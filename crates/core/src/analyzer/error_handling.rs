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
    suggestion: &'static str,
    fix_kind: FixKind,
}

/// Returns all error handling patterns in priority order
fn patterns() -> &'static [ErrorPattern] {
    static PATTERNS: OnceLock<Vec<ErrorPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ERR-001: Empty catch/except block (multi-language)
            // Matches: catch (...) { } or except ...: pass or catch { } etc.
            ErrorPattern {
                name: "Empty catch/except block",
                regex: Regex::new(r"(?:catch\s*(?:\([^)]*\))?\s*\{\s*\}|except[^:]*:\s*(?:pass\s*$))").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion: "Handle the error or add a comment explaining why it is safe to ignore",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-002: Bare except: (no exception type) in Python
            ErrorPattern {
                name: "Bare except without exception type",
                regex: Regex::new(r"^\s*except\s*:").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion: "Specify an exception type: except ValueError: or except Exception as e:",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-003: .unwrap() in Rust
            ErrorPattern {
                name: ".unwrap() call",
                regex: Regex::new(r"\.unwrap\(\)").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion: "Use ? operator, .unwrap_or(), .unwrap_or_else(), or .expect() with a message",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-004: panic!()/todo!()/unimplemented!() in non-test Rust code
            ErrorPattern {
                name: "panic!/todo!/unimplemented! in non-test code",
                regex: Regex::new(r"\b(?:panic!|todo!|unimplemented!)\s*\(").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("#[test]"),
                suggestion: "Return a Result with a descriptive error instead of panicking",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-005: Catch that only logs (swallowed error)
            ErrorPattern {
                name: "Catch block only logs error",
                regex: Regex::new(r"catch\s*\([^)]*\)\s*\{\s*(?:console\.(?:log|warn|error|info)|System\.(?:out|err)\.print|log(?:ger)?\.(?:error|warn|info|debug))\s*\(").unwrap(),
                severity: Severity::Info,
                reject_if_contains: Some("throw"),
                suggestion: "Re-throw the error or handle it properly after logging",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-006: except Exception or except BaseException (too broad)
            ErrorPattern {
                name: "Too-broad exception catch",
                regex: Regex::new(r"^\s*except\s+(?:Exception|BaseException)\b").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion: "Catch a more specific exception type (e.g. ValueError, KeyError)",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-007: Empty .catch() callback in JS/TS
            ErrorPattern {
                name: "Empty .catch() callback",
                regex: Regex::new(r"\.catch\s*\(\s*(?:\(\s*[^)]*\)\s*=>\s*\{\s*\}|\w+\s*=>\s*\{\s*\}|\(\s*\)\s*\{\s*\}|function\s*\(\s*[^)]*\)\s*\{\s*\})\s*\)").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion: "Handle or re-throw the error in the .catch() callback",
                fix_kind: FixKind::Suggestion,
            },
            // ERR-008: Discarded error in Go (_ = err)
            ErrorPattern {
                name: "Discarded error in Go",
                regex: Regex::new(r"_\s*=\s*err\b").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
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
            || trimmed.starts_with('#')
    }

    /// Check if a file is a test file (for ERR-004 filtering)
    fn is_test_file(path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        name.starts_with("test_")
            || name.ends_with("_test.rs")
            || name.ends_with("_test.go")
            || name.contains("test")
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

        for (line_num, line) in content.lines().enumerate() {
            if Self::is_comment_line(line) {
                continue;
            }

            // First matching pattern wins for this line
            for (idx, pat) in all_patterns.iter().enumerate() {
                // ERR-003 (.unwrap()): only for Rust files
                if idx == 2 && ext != "rs" {
                    continue;
                }
                // ERR-004 (panic!/todo!/unimplemented!): only Rust, skip test files
                if idx == 3 && (ext != "rs" || is_test) {
                    continue;
                }
                // ERR-005 (catch-only-logs): only JS/TS/Java
                if idx == 4 && !matches!(ext, "js" | "ts" | "jsx" | "tsx" | "java") {
                    continue;
                }
                // ERR-002/ERR-006: only Python
                if (idx == 1 || idx == 5) && ext != "py" {
                    continue;
                }
                // ERR-007 (empty .catch()): only JS/TS
                if idx == 6 && !matches!(ext, "js" | "ts" | "jsx" | "tsx") {
                    continue;
                }
                // ERR-008 (discarded err): only Go
                if idx == 7 && ext != "go" {
                    continue;
                }

                if pat.regex.is_match(line) {
                    // Check reject filter
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
