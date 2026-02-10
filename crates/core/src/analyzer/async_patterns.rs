//! Async Patterns analyzer — detects async/await anti-patterns in JS/TS and Python
//!
//! Scans `.js`, `.ts`, `.jsx`, `.tsx`, and `.py` files line-by-line for patterns
//! that cause unhandled rejections, silent failures, and race conditions.
//! Only one finding per line (first matching pattern wins) to reduce noise.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled async pattern detection rule
struct AsyncPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    /// If set, skip the match when the line contains this substring
    reject_if_contains: Option<&'static str>,
    suggestion: &'static str,
    fix_kind: FixKind,
}

/// Returns all async patterns in priority order (Error patterns first)
fn patterns() -> &'static [AsyncPattern] {
    static PATTERNS: OnceLock<Vec<AsyncPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // --- Error: Always wrong ---
            AsyncPattern {
                name: "Async Promise executor",
                regex: Regex::new(r"new\s+Promise\s*\(\s*async").unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                suggestion: "Remove async from Promise executor; use resolve/reject callbacks instead",
                fix_kind: FixKind::Suggestion,
            },
            AsyncPattern {
                name: "Await in forEach",
                regex: Regex::new(r"\.forEach\s*\(\s*async").unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                suggestion: "Use for...of loop or Promise.all(items.map(...)) instead of forEach with async",
                fix_kind: FixKind::Suggestion,
            },
            // --- Warning: Usually problematic ---
            AsyncPattern {
                name: "Unhandled .then() chain",
                regex: Regex::new(r"\.then\s*\(").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some(".catch"),
                suggestion: "Add .catch() handler or use async/await with try/catch",
                fix_kind: FixKind::Suggestion,
            },
            AsyncPattern {
                name: "Async map without Promise.all",
                regex: Regex::new(r"\.map\s*\(\s*async").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("Promise.all"),
                suggestion: "Wrap with await Promise.all(...) to collect async map results",
                fix_kind: FixKind::Suggestion,
            },
            AsyncPattern {
                name: "Async timer callback",
                regex: Regex::new(r"(?:setTimeout|setInterval)\s*\(\s*async").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion: "Extract async logic and add error handling inside the callback",
                fix_kind: FixKind::Suggestion,
            },
            AsyncPattern {
                name: "Floating Python coroutine",
                regex: Regex::new(r"asyncio\.(?:sleep|gather|wait_for|create_task|ensure_future)\s*\(").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("await"),
                suggestion: "Add await before asyncio call",
                fix_kind: FixKind::Suggestion,
            },
            // --- Info: Code smell / style ---
            AsyncPattern {
                name: "Swallowed error in catch",
                regex: Regex::new(r"\.catch\s*\([^{]*\{\s*\}\s*\)").unwrap(),
                severity: Severity::Info,
                reject_if_contains: None,
                suggestion: "Handle or log the error instead of swallowing it",
                fix_kind: FixKind::Suggestion,
            },
            AsyncPattern {
                name: "Redundant return await",
                regex: Regex::new(r"return\s+await\s+").unwrap(),
                severity: Severity::Info,
                reject_if_contains: None,
                suggestion: "Remove await from return statement (unless inside try/catch)",
                fix_kind: FixKind::Suggestion,
            },
        ]
    })
}

/// File extensions to scan for async patterns
const ASYNC_EXTENSIONS: &[&str] = &["js", "ts", "jsx", "tsx", "py"];

/// Analyzer that detects async/await anti-patterns
pub struct AsyncPatternsAnalyzer;

impl AsyncPatternsAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned based on its extension
    fn should_scan(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| ASYNC_EXTENSIONS.contains(&e))
            .unwrap_or(false)
    }

    /// Check if a line is a comment (covers JS/TS and Python)
    fn is_comment_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("//")
            || trimmed.starts_with('*')
            || trimmed.starts_with("/*")
            || trimmed.starts_with('#')
    }

    /// Scan a single file for async pattern issues
    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if Self::is_comment_line(line) {
                continue;
            }

            // First matching pattern wins for this line
            for pat in all_patterns {
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

impl Default for AsyncPatternsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for AsyncPatternsAnalyzer {
    fn name(&self) -> &str {
        "Async Patterns"
    }

    fn finding_prefix(&self) -> &str {
        "ASYNC"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.async_patterns
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
