//! Dead imports analyzer — detects imported symbols never used within the same file.
//!
//! Unlike `UnusedExportsAnalyzer` (which tracks cross-file callers via the graph),
//! this analyzer asks: "you imported X into this file — do you ever *use* X here?"
//!
//! **Algorithm:**
//! 1. Enumerate all `Import` nodes from the graph (which carry `imported_names`).
//! 2. Read the import's source line to detect local aliases (`import X as Y` → use `Y`).
//! 3. Count word-boundary occurrences of the local name in the whole file.
//! 4. If count ≤ 1 the name only appears on the import line itself — flag it.
//!
//! **Limitations (known, acceptable for v1):**
//! - Names appearing in comments or string literals are counted as "used".
//! - Implicit usages (e.g. React in JSX with old transform) may produce false positives.
//! - Multi-line import statements use the first line for alias detection.

use crate::analyzer::GraphAnalyzer;
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use crate::graph::{CodeGraph, NodeData, NodeKind};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// ── Public struct ─────────────────────────────────────────────────────────────

pub struct DeadImportsAnalyzer;

impl DeadImportsAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeadImportsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Names that should never be flagged as "unused" regardless of occurrence count.
/// - `self` / `super` / `crate` are Rust meta-keywords in use-paths
/// - `_` is a conventional "ignore" identifier
const ALWAYS_SKIP: &[&str] = &["self", "super", "crate", "_"];

/// Try to extract the local alias from an import line for a given original name.
///
/// Handles patterns like:
/// - `import pandas as pd`                       → alias for "pandas" is "pd"
/// - `from os import path as p`                  → alias for "path" is "p"
/// - `import { Foo as Bar } from './mod'`         → alias for "Foo" is "Bar"
/// - `import alias "net/http"` (Go)              → Go parser already stores alias in imported_names
fn extract_alias(line: &str, name: &str) -> Option<String> {
    // Look for `<name> as <ident>` anywhere in the line.
    let search = format!("{name} as ");
    let pos = line.find(&search)?;
    let after_as = &line[pos + search.len()..];
    let alias: String = after_as
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if alias.is_empty() {
        None
    } else {
        Some(alias)
    }
}

/// Count non-overlapping, word-boundary occurrences of `word` in `content`.
///
/// A "word boundary" means the byte before and after the match must not be
/// an ASCII alphanumeric character or underscore.
fn count_word(content: &str, word: &str) -> usize {
    if word.is_empty() {
        return 0;
    }
    let bytes = content.as_bytes();
    let wbytes = word.as_bytes();
    let wlen = wbytes.len();
    let clen = bytes.len();
    let mut count = 0usize;
    let mut i = 0usize;

    while i + wlen <= clen {
        if bytes[i..i + wlen] == *wbytes {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_ok = i + wlen >= clen || !is_ident_byte(bytes[i + wlen]);
            if before_ok && after_ok {
                count += 1;
            }
            i += wlen;
        } else {
            i += 1;
        }
    }
    count
}

#[inline]
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// ── GraphAnalyzer impl ────────────────────────────────────────────────────────

impl GraphAnalyzer for DeadImportsAnalyzer {
    fn name(&self) -> &str {
        "Dead Imports"
    }

    fn finding_prefix(&self) -> &str {
        "IMP"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.dead_imports
    }

    fn analyze_graph(&self, graph: &CodeGraph, _config: &RevetConfig) -> Vec<Finding> {
        // Collect all Import nodes grouped by file path.
        // Each entry: (line_number, imported_names)
        let mut by_file: HashMap<PathBuf, Vec<(usize, Vec<String>)>> = HashMap::new();

        for (_, node) in graph.nodes() {
            if !matches!(node.kind(), NodeKind::Import) {
                continue;
            }
            let NodeData::Import { imported_names, .. } = node.data() else {
                continue;
            };
            by_file
                .entry(node.file_path().clone())
                .or_default()
                .push((node.line(), imported_names.clone()));
        }

        let mut findings = Vec::new();

        for (file_path, imports) in &by_file {
            let content = match fs::read_to_string(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();

            for (import_line_no, imported_names) in imports {
                // Retrieve the raw import line for alias detection (1-indexed).
                let import_line_str = if *import_line_no > 0 && *import_line_no <= lines.len() {
                    lines[import_line_no - 1]
                } else {
                    ""
                };

                for name in imported_names {
                    // Skip wildcard imports — we can't know what they bring in.
                    if name == "*" || name.is_empty() {
                        continue;
                    }

                    // Skip language meta-keywords that are never real usages.
                    if ALWAYS_SKIP.contains(&name.as_str()) {
                        continue;
                    }

                    // Determine the local binding: prefer an explicit alias.
                    let local_name =
                        extract_alias(import_line_str, name).unwrap_or_else(|| name.clone());

                    // Count word-boundary occurrences across the whole file.
                    // A count of ≤ 1 means the name only appears on the import
                    // line itself and is never referenced in the file body.
                    let occurrences = count_word(&content, &local_name);

                    if occurrences <= 1 {
                        findings.push(Finding {
                            id: String::new(),
                            severity: Severity::Warning,
                            message: format!("`{local_name}` is imported but never used"),
                            file: file_path.clone(),
                            line: *import_line_no,
                            affected_dependents: 0,
                            suggestion: Some(format!("Remove the unused import of `{local_name}`")),
                            fix_kind: None,
                        });
                    }
                }
            }
        }

        findings
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_alias_simple() {
        assert_eq!(
            extract_alias("import pandas as pd", "pandas"),
            Some("pd".to_string())
        );
    }

    #[test]
    fn test_extract_alias_from_import() {
        assert_eq!(
            extract_alias("from os import path as p", "path"),
            Some("p".to_string())
        );
    }

    #[test]
    fn test_extract_alias_none() {
        assert_eq!(extract_alias("from utils import helper", "helper"), None);
    }

    #[test]
    fn test_extract_alias_ts_named() {
        assert_eq!(
            extract_alias("import { Foo as Bar } from './mod'", "Foo"),
            Some("Bar".to_string())
        );
    }

    #[test]
    fn test_count_word_basic() {
        assert_eq!(count_word("let x = pandas.read_csv()", "pandas"), 1);
        assert_eq!(count_word("pandas pandas pandas", "pandas"), 3);
    }

    #[test]
    fn test_count_word_boundary() {
        // "DataFrame" inside "DataFrameExtra" should NOT match
        assert_eq!(count_word("DataFrameExtra", "DataFrame"), 0);
        // But "DataFrame" with a dot after should match
        assert_eq!(count_word("DataFrame.from_dict()", "DataFrame"), 1);
    }

    #[test]
    fn test_count_word_underscore_boundary() {
        // _helper and helper_ are different words
        assert_eq!(count_word("_helper is not helper", "helper"), 1);
    }
}
