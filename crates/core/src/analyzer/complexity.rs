//! Complexity analyzer — detects overly complex functions via structural and content metrics.
//!
//! Checks four metrics per function node:
//! - **Length**: lines between opening and closing (from graph `end_line`)
//! - **Parameters**: argument count (from graph `NodeData::Function`)
//! - **Cyclomatic complexity**: branch-counting heuristic on the function body
//! - **Nesting depth**: max brace/indentation depth within the function body

use crate::analyzer::GraphAnalyzer;
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use crate::graph::{CodeGraph, NodeData, NodeKind};
use std::fs;
use std::path::Path;

// ── Thresholds ────────────────────────────────────────────────────────────────

const FN_LEN_WARN: usize = 50;
const FN_LEN_ERROR: usize = 100;

const PARAM_WARN: usize = 5;
const PARAM_ERROR: usize = 8;

const COMPLEXITY_WARN: usize = 10;
const COMPLEXITY_ERROR: usize = 20;

const NESTING_WARN: usize = 4;
const NESTING_ERROR: usize = 6;

// ── Public struct ─────────────────────────────────────────────────────────────

pub struct ComplexityAnalyzer;

impl ComplexityAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ComplexityAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Map a file extension to a language key used in branch counting.
fn lang_from_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("py") => "python",
        Some("ts") | Some("tsx") => "typescript",
        Some("js") | Some("jsx") => "javascript",
        Some("rs") => "rust",
        Some("go") => "go",
        Some("java") => "java",
        Some("cs") => "csharp",
        Some("kt") | Some("kts") => "kotlin",
        Some("rb") => "ruby",
        Some("php") => "php",
        Some("swift") => "swift",
        _ => "generic",
    }
}

/// Count cyclomatic complexity for a slice of source lines.
///
/// Starts at 1 (base path), then adds 1 per decision point. Comments are skipped.
fn cyclomatic_complexity(lines: &[&str], lang: &str) -> usize {
    let mut complexity = 1usize; // base path

    for line in lines {
        let t = line.trim();
        // Skip comment lines (rough heuristic — good enough for branch counting)
        if t.starts_with("//") || t.starts_with('#') || t.starts_with('*') || t.starts_with("/*") {
            continue;
        }

        complexity += branches_in_line(t, lang);
    }

    complexity
}

/// Count branch points on a single trimmed line for the given language.
fn branches_in_line(t: &str, lang: &str) -> usize {
    let mut n = 0usize;

    match lang {
        "python" => {
            for kw in &["if ", "elif ", "for ", "while ", " and ", " or ", "except"] {
                if t.contains(kw) {
                    n += 1;
                }
            }
            // Inline `else:` on its own line
            if t == "else:" || t.starts_with("else:") {
                n += 1;
            }
        }
        "rust" => {
            for kw in &[
                "if ", "} else {", "else if ", "for ", "while ", "loop {", "match ",
            ] {
                if t.contains(kw) {
                    n += 1;
                }
            }
            // Match arms: standalone `=> ` (avoid counting closures double)
            n += t.matches("=>").count();
            n += t.matches("&&").count();
            n += t.matches("||").count();
            // ? operator — each is a potential early-return branch
            n += t.matches('?').count();
        }
        "go" => {
            for kw in &[
                "if ", "} else {", "else if ", "for ", "switch ", "select {", "case ",
            ] {
                if t.contains(kw) {
                    n += 1;
                }
            }
            n += t.matches("&&").count();
            n += t.matches("||").count();
        }
        // TypeScript, JavaScript, Java, C#, Kotlin, PHP, Swift, Ruby share similar syntax
        _ => {
            for kw in &[
                "if (",
                "if(",
                "else if (",
                "else if(",
                "} else {",
                "for (",
                "for(",
                "while (",
                "while(",
                "switch (",
                "switch(",
                "case ",
                "catch (",
                "catch(",
                "catch {",
            ] {
                if t.contains(kw) {
                    n += 1;
                }
            }
            n += t.matches("&&").count();
            n += t.matches("||").count();
            n += t.matches("??").count();
            // Ternary `condition ? a : b`
            n += t.matches(" ? ").count();
        }
    }

    n
}

/// Compute maximum nesting depth within a function's source lines.
///
/// For brace-based languages, tracks `{` / `}`. Depth 1 = inside the function
/// body itself; each nested block adds one more.
///
/// For Python, measures indentation depth relative to the function's own
/// indentation (first non-empty line defines the baseline).
fn max_nesting_depth(lines: &[&str], lang: &str) -> usize {
    if lang == "python" {
        return python_max_nesting(lines);
    }

    let mut depth = 0i32;
    let mut max_depth = 0usize;

    for line in lines {
        let t = line.trim();
        // Skip comment lines
        if t.starts_with("//") || t.starts_with('*') || t.starts_with("/*") {
            continue;
        }
        for ch in t.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    max_depth = max_depth.max(depth as usize);
                }
                '}' => {
                    depth = (depth - 1).max(0);
                }
                _ => {}
            }
        }
    }

    // Depth 1 = inside the function's own braces — subtract baseline of 1
    // so we report "depth of nested blocks *within* the body"
    max_depth.saturating_sub(1)
}

/// Python-specific nesting: count indent levels relative to the function body.
fn python_max_nesting(lines: &[&str]) -> usize {
    // Find the function body's baseline indent (first non-empty line after def)
    let baseline = lines
        .iter()
        .skip(1) // skip the `def foo():` line itself
        .find(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or(0);

    let mut max_extra = 0usize;
    for line in lines.iter().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent >= baseline {
            let extra = (indent - baseline) / 4; // assume 4-space indent per level
            max_extra = max_extra.max(extra);
        }
    }
    max_extra
}

// ── GraphAnalyzer impl ────────────────────────────────────────────────────────

impl GraphAnalyzer for ComplexityAnalyzer {
    fn name(&self) -> &str {
        "Complexity"
    }

    fn finding_prefix(&self) -> &str {
        "CMPLX"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.complexity
    }

    fn analyze_graph(&self, graph: &CodeGraph, _config: &RevetConfig) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (_, node) in graph.nodes() {
            if !matches!(node.kind(), NodeKind::Function) {
                continue;
            }

            let NodeData::Function { parameters, .. } = node.data() else {
                continue;
            };

            let file_path = node.file_path();
            let start_line = node.line();
            let end_line = node.end_line().unwrap_or(start_line);
            let fn_length = end_line.saturating_sub(start_line);
            let param_count = parameters.len();
            let lang = lang_from_path(file_path);

            // ── 1. Function length ──────────────────────────────────────────
            if fn_length >= FN_LEN_ERROR {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Error,
                    message: format!(
                        "Function `{}` is {} lines long (max recommended: {})",
                        node.name(),
                        fn_length,
                        FN_LEN_ERROR
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Break this function into smaller, focused functions".to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
            } else if fn_length >= FN_LEN_WARN {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Warning,
                    message: format!(
                        "Function `{}` is {} lines long (recommended: <{})",
                        node.name(),
                        fn_length,
                        FN_LEN_WARN
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Consider breaking this function into smaller, focused functions"
                            .to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
            }

            // ── 2. Parameter count ──────────────────────────────────────────
            if param_count >= PARAM_ERROR {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Error,
                    message: format!(
                        "Function `{}` has {} parameters (max recommended: {})",
                        node.name(),
                        param_count,
                        PARAM_ERROR
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Group related parameters into a struct or configuration object"
                            .to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
            } else if param_count >= PARAM_WARN {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Warning,
                    message: format!(
                        "Function `{}` has {} parameters (recommended: <{})",
                        node.name(),
                        param_count,
                        PARAM_WARN
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Consider grouping related parameters into a struct or object".to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
            }

            // ── 3 & 4. Cyclomatic complexity + nesting (require file content) ──
            if start_line == 0 || end_line < start_line {
                continue;
            }

            let Ok(content) = fs::read_to_string(file_path) else {
                continue;
            };

            let all_lines: Vec<&str> = content.lines().collect();
            let line_count = all_lines.len();

            // tree-sitter lines are 0-indexed; graph stores 1-based line numbers
            let start_idx = start_line.saturating_sub(1);
            let end_idx = end_line.min(line_count);

            if start_idx >= end_idx {
                continue;
            }

            let fn_lines = &all_lines[start_idx..end_idx];

            // Cyclomatic complexity
            let complexity = cyclomatic_complexity(fn_lines, lang);
            if complexity >= COMPLEXITY_ERROR {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Error,
                    message: format!(
                        "Function `{}` has cyclomatic complexity of {} (max recommended: {})",
                        node.name(),
                        complexity,
                        COMPLEXITY_ERROR
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Reduce branching by extracting helper functions or simplifying logic"
                            .to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
            } else if complexity >= COMPLEXITY_WARN {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Warning,
                    message: format!(
                        "Function `{}` has cyclomatic complexity of {} (recommended: <{})",
                        node.name(),
                        complexity,
                        COMPLEXITY_WARN
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Consider reducing branching by extracting helper functions".to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
            }

            // Nesting depth
            let nesting = max_nesting_depth(fn_lines, lang);
            if nesting >= NESTING_ERROR {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Error,
                    message: format!(
                        "Function `{}` has nesting depth of {} (max recommended: {})",
                        node.name(),
                        nesting,
                        NESTING_ERROR
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Reduce nesting using early returns or helper functions".to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
            } else if nesting >= NESTING_WARN {
                findings.push(Finding {
                    id: String::new(),
                    severity: Severity::Warning,
                    message: format!(
                        "Function `{}` has nesting depth of {} (recommended: <{})",
                        node.name(),
                        nesting,
                        NESTING_WARN
                    ),
                    file: file_path.clone(),
                    line: start_line,
                    affected_dependents: 0,
                    suggestion: Some(
                        "Consider reducing nesting using early returns or helper functions"
                            .to_string(),
                    ),
                    fix_kind: None,
                    ..Default::default()
                });
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
    fn test_cyclomatic_complexity_baseline() {
        let lines = vec!["fn foo() {", "    let x = 1;", "}"];
        assert_eq!(cyclomatic_complexity(&lines, "rust"), 1);
    }

    #[test]
    fn test_cyclomatic_complexity_branches() {
        let lines = vec![
            "fn foo(x: i32) -> i32 {",
            "    if x > 0 {",
            "        if x > 10 {",
            "            return x * 2;",
            "        }",
            "    } else {",
            "        return 0;",
            "    }",
            "    x",
            "}",
        ];
        // base(1) + if(1) + inner if(1) + } else {(1) = 4
        let cc = cyclomatic_complexity(&lines, "rust");
        assert!(cc >= 3, "Expected cc >= 3, got {cc}");
    }

    #[test]
    fn test_nesting_depth_brace() {
        let lines = vec![
            "fn foo() {", // depth 1
            "    if x {", // depth 2
            "        {",  // depth 3
            "        }",
            "    }",
            "}",
        ];
        // max raw = 3, subtract baseline 1 → nesting = 2
        assert_eq!(max_nesting_depth(&lines, "rust"), 2);
    }

    #[test]
    fn test_nesting_depth_flat() {
        let lines = vec!["fn foo() {", "    let x = 1;", "}"];
        // max raw = 1, subtract 1 → nesting = 0
        assert_eq!(max_nesting_depth(&lines, "rust"), 0);
    }

    #[test]
    fn test_python_nesting() {
        let lines = vec![
            "def foo():",
            "    for i in range(10):",
            "        if i > 5:",
            "            print(i)",
        ];
        // baseline indent = 4; deepest indent = 12; extra = (12-4)/4 = 2
        assert_eq!(max_nesting_depth(&lines, "python"), 2);
    }
}
