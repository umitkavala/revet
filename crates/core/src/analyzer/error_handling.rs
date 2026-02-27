//! Error Handling analyzer — detects error handling anti-patterns across languages
//!
//! Scans `.py`, `.js`, `.ts`, `.jsx`, `.tsx`, `.rs`, `.go`, `.java`, `.kt`, `.cs`
//! files line-by-line for patterns like empty catch blocks, bare except, unwrap chains,
//! and swallowed errors. Only one finding per line (first matching pattern wins).

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled error handling pattern detection rule
struct ErrorPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    /// If set, skip the match when the line contains this substring
    reject_if_contains: Option<&'static str>,
    /// File extensions this pattern applies to (empty = all scanned extensions)
    extensions: &'static [&'static str],
    /// If true, skip this pattern when the file is a test file
    skip_in_test_files: bool,
    suggestion: &'static str,
    fix_kind: FixKind,
}

/// Returns all error handling patterns in priority order
fn patterns() -> &'static [ErrorPattern] {
    static PATTERNS: OnceLock<Vec<ErrorPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ERR-001: Empty catch/except block (multi-language)
            ErrorPattern {
                name: "Empty catch/except block",
                regex: Regex::new(r"(?:catch\s*(?:\([^)]*\))?\s*\{\s*\}|except[^:]*:\s*(?:pass\s*$))").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &[],
                skip_in_test_files: false,
                suggestion: "Handle the error or add a comment explaining why it is safe to ignore",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-002: Bare except: (no exception type) in Python
            ErrorPattern {
                name: "Bare except without exception type",
                regex: Regex::new(r"^\s*except\s*:").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &["py"],
                skip_in_test_files: false,
                suggestion: "Specify an exception type: except ValueError: or except Exception as e:",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-003: .unwrap() in Rust (skip test files)
            ErrorPattern {
                name: ".unwrap() call",
                regex: Regex::new(r"\.unwrap\(\)").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &["rs"],
                skip_in_test_files: true,
                suggestion: "Use ? operator, .unwrap_or(), .unwrap_or_else(), or .expect() with a descriptive message",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-004: panic!()/todo!()/unimplemented!() in non-test Rust code
            ErrorPattern {
                name: "panic!/todo!/unimplemented! in non-test code",
                regex: Regex::new(r"\b(?:panic!|todo!|unimplemented!)\s*\(").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &["rs"],
                skip_in_test_files: true,
                suggestion: "Return a Result with a descriptive error instead of panicking",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-005: .expect() with a non-descriptive message in Rust
            ErrorPattern {
                name: ".expect() with non-descriptive message",
                regex: Regex::new(
                    r#"\.expect\s*\(\s*["'](?:|error|err|failed|fail|failure|oops|todo|fixme|ok|bad|wrong|none|panic|crash|broken|invalid|unexpected|unreachable|missing|unknown|test|no|yes|x|temp|hack)\s*["']\s*\)"#,
                )
                .unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &["rs"],
                skip_in_test_files: true,
                suggestion: "Use a descriptive message: .expect(\"Failed to open config file\")",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-006: Catch that only logs (swallowed error)
            ErrorPattern {
                name: "Catch block only logs error",
                regex: Regex::new(r"catch\s*\([^)]*\)\s*\{\s*(?:console\.(?:log|warn|error|info)|System\.(?:out|err)\.print|log(?:ger)?\.(?:error|warn|info|debug))\s*\(").unwrap(),
                severity: Severity::Info,
                reject_if_contains: Some("throw"),
                extensions: &["js", "ts", "jsx", "tsx", "java"],
                skip_in_test_files: false,
                suggestion: "Re-throw the error or handle it properly after logging",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-007: except Exception or except BaseException (too broad)
            ErrorPattern {
                name: "Too-broad exception catch",
                regex: Regex::new(r"^\s*except\s+(?:Exception|BaseException)\b").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &["py"],
                skip_in_test_files: false,
                suggestion: "Catch a more specific exception type (e.g. ValueError, KeyError)",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-008: Empty .catch() callback in JS/TS
            ErrorPattern {
                name: "Empty .catch() callback",
                regex: Regex::new(r"\.catch\s*\(\s*(?:\(\s*[^)]*\)\s*=>\s*\{\s*\}|\w+\s*=>\s*\{\s*\}|\(\s*\)\s*\{\s*\}|function\s*\(\s*[^)]*\)\s*\{\s*\})\s*\)").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &["js", "ts", "jsx", "tsx"],
                skip_in_test_files: false,
                suggestion: "Handle or re-throw the error in the .catch() callback",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-009: Discarded error in Go (_ = err)
            ErrorPattern {
                name: "Discarded error in Go",
                regex: Regex::new(r"_\s*=\s*err\b").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                extensions: &["go"],
                skip_in_test_files: false,
                suggestion: "Handle the error: if err != nil { return err }",
                fix_kind: FixKind::Suggestion,
            },
        ]
    })
}

/// File extensions to scan for error handling patterns
const ERROR_EXTENSIONS: &[&str] = &[
    "py", "js", "ts", "jsx", "tsx", "rs", "go", "java", "kt", "cs",
];

/// Analyzer that detects error handling anti-patterns
pub struct ErrorHandlingAnalyzer;

impl ErrorHandlingAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned based on its extension
    fn should_scan(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| ERROR_EXTENSIONS.contains(&e))
            .unwrap_or(false)
    }

    /// Check if a line is a comment (covers JS/TS/Java/C#/Kotlin/Rust/Go and Python)
    fn is_comment_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("//")
            || trimmed.starts_with('*')
            || trimmed.starts_with("/*")
            // '#' is a comment in Python/shell, but '#[' is a Rust attribute — don't skip those
            || (trimmed.starts_with('#') && !trimmed.starts_with("#["))
    }

    /// Check if a file is a test file — skips Rust-specific patterns
    fn is_test_file(path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Common test file naming conventions
        if name.starts_with("test_")
            || name.ends_with("_test.rs")
            || name.ends_with("_test.go")
            || name.ends_with("_spec.rs")
            || name.ends_with("_spec.js")
            || name.ends_with("_spec.ts")
        {
            return true;
        }
        // Files under a `tests/` directory (e.g. crates/core/tests/*.rs)
        path.components()
            .any(|c| c.as_os_str() == "tests" || c.as_os_str() == "__tests__")
    }

    /// Scan a single file for error handling issues
    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let is_test = Self::is_test_file(path);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let mut findings = Vec::new();

        // Track Rust test context via brace depth so entire #[test]/#[cfg(test)]
        // function/module bodies are excluded from Rust-specific patterns.
        let mut pending_test_scope = false; // just saw a test attribute
        let mut test_brace_depth: i32 = 0; // brace depth inside a test scope
        let mut in_test_scope = false; // currently inside a test fn/mod body

        for (line_num, line) in content.lines().enumerate() {
            if Self::is_comment_line(line) {
                continue;
            }

            // Detect Rust test attribute lines
            if ext == "rs" {
                let t = line.trim();
                if t == "#[test]"
                    || t.starts_with("#[cfg(test)]")
                    || t.starts_with("#[tokio::test]")
                    || t.starts_with("#[async_std::test]")
                {
                    pending_test_scope = true;
                }

                // Once we've seen a test attribute, track brace depth to mark scope
                if pending_test_scope || in_test_scope {
                    let open = line.chars().filter(|&c| c == '{').count() as i32;
                    let close = line.chars().filter(|&c| c == '}').count() as i32;
                    if open > 0 && !in_test_scope {
                        // First opening brace after the test attribute — enter scope
                        in_test_scope = true;
                        pending_test_scope = false;
                        test_brace_depth = open - close;
                    } else if in_test_scope {
                        test_brace_depth += open - close;
                        if test_brace_depth <= 0 {
                            in_test_scope = false;
                            test_brace_depth = 0;
                        }
                    }
                }
            }

            let line_in_test_context = in_test_scope;

            // First matching pattern wins for this line
            for pat in all_patterns.iter() {
                // Extension gate
                if !pat.extensions.is_empty() && !pat.extensions.contains(&ext) {
                    continue;
                }
                // Test-file gate
                if pat.skip_in_test_files && (is_test || line_in_test_context) {
                    continue;
                }

                if pat.regex.is_match(line) {
                    if let Some(reject) = pat.reject_if_contains {
                        if line.contains(reject) {
                            continue;
                        }
                    }

                    findings.push(make_finding(
                        pat.severity,
                        pat.name.to_string(),
                        path.to_path_buf(),
                        line_num + 1,
                        Some(pat.suggestion.to_string()),
                        Some(pat.fix_kind.clone()),
                    ));
                    break;
                }
            }
        }

        findings
    }
}

impl Default for ErrorHandlingAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for ErrorHandlingAnalyzer {
    fn name(&self) -> &str {
        "Error Handling"
    }

    fn finding_prefix(&self) -> &str {
        "ERR"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.error_handling
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
