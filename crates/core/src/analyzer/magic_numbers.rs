//! Magic numbers analyzer — detects unnamed numeric literals in source code
//!
//! "Magic numbers" are bare numeric constants that appear in logic without a
//! named constant to explain their meaning. They harm readability, make
//! refactoring error-prone, and hide business rules inside implementation details.
//!
//! This analyzer flags integer and float literals that appear in expressions
//! while ignoring universally unambiguous values (0, 1, -1, 2) and common
//! false-positive contexts (array indices in tests, version tuples, port 80/443,
//! HTTP status codes with clear surrounding context, etc.).
//!
//! Disabled by default (`modules.magic_numbers = false`).

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// ── Skip list ─────────────────────────────────────────────────────────────────

/// Numeric literals universally understood without a name.
const ALLOWED_LITERALS: &[&str] = &["0", "1", "-1", "2", "0.0", "1.0", "0.5", "100", "1000"];

/// Skip files with these extensions (binary / generated / data files).
const SKIP_EXTENSIONS: &[&str] = &[
    "lock", "sum", "mod", "toml", "json", "yaml", "yml", "xml", "html", "md", "txt", "csv", "tsv",
    "sql", "png", "jpg", "jpeg", "gif", "svg", "ico", "woff", "woff2", "ttf", "eot", "otf", "mp3",
    "mp4", "avi", "mov", "wav", "pdf", "zip", "gz", "tar", "exe", "dll", "so", "dylib", "o", "a",
    "pyc", "class", "db", "sqlite",
];

// ── Patterns ──────────────────────────────────────────────────────────────────

fn magic_number_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Numeric literal preceded by an operator/paren/comma and followed by a
    // non-identifier character, to avoid matching `x64`, `v1`, `0x1f`, etc.
    RE.get_or_init(|| {
        Regex::new(
            r"(?:^|[(\[,=<>!+\-*/%&|^~\s])(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)(?:[)\]},;:\s]|$)",
        )
        .unwrap()
    })
}

/// Lines that are almost certainly not magic numbers (comments, const/let defs,
/// version strings, enum discriminants, etc.).
fn skip_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*(?://|#|/\*|\*|<!--|$)|const\s+\w+\s*[=:]|\bdefine\b|\benum\b|\bversion\b|\bstatus_code\b|\bport\s*[=:]\s*\d+|\bstride\b|\boffset\b|\bpadding\b|^\s*\[|\#\[",
        )
        .unwrap()
    })
}

// ── Analyzer ──────────────────────────────────────────────────────────────────

pub struct MagicNumbersAnalyzer;

impl MagicNumbersAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn should_scan(path: &Path) -> bool {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return false,
        };
        !SKIP_EXTENSIONS.contains(&ext.as_str())
    }

    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let re = magic_number_re();
        let skip = skip_line_re();
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Skip comment lines, constant declarations, and other low-noise contexts
            if skip.is_match(line) {
                continue;
            }

            // Collect all distinct numeric literals on this line
            let mut flagged: Vec<&str> = re
                .captures_iter(line)
                .filter_map(|cap| cap.get(1))
                .map(|m| m.as_str())
                .filter(|n| !ALLOWED_LITERALS.contains(n))
                .collect();

            flagged.dedup();

            if flagged.is_empty() {
                continue;
            }

            let numbers = flagged.join(", ");
            findings.push(make_finding(
                Severity::Info,
                format!("Magic number(s): {numbers} — consider extracting to a named constant"),
                path.to_path_buf(),
                line_num + 1,
                Some(
                    "Replace the literal with a named constant (e.g. `const MAX_RETRIES: u32 = 3`)"
                        .to_string(),
                ),
                Some(FixKind::Suggestion),
            ));
        }

        findings
    }
}

impl Default for MagicNumbersAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for MagicNumbersAnalyzer {
    fn name(&self) -> &str {
        "Magic Numbers"
    }

    fn finding_prefix(&self) -> &str {
        "MAGIC"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.magic_numbers
    }

    fn analyze_files(&self, files: &[PathBuf], _repo_root: &Path) -> Vec<Finding> {
        let mut findings = Vec::new();
        for file in files {
            if !Self::should_scan(file) {
                continue;
            }
            findings.extend(Self::scan_file(file));
        }
        findings
    }
}
