//! React Hooks analyzer — detects Rules of Hooks violations and common anti-patterns
//!
//! Scans `.tsx`, `.jsx`, `.ts`, and `.js` files line-by-line for patterns that indicate
//! misuse of React hooks or common React anti-patterns. Only one finding per line
//! (first matching pattern wins) to reduce noise.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled React hooks detection pattern
struct HookPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    /// If set, skip the match when the line contains this substring
    reject_if_contains: Option<&'static str>,
    suggestion: &'static str,
    fix_kind: FixKind,
}

/// Returns all hook patterns in priority order (Error patterns first)
fn patterns() -> &'static [HookPattern] {
    static PATTERNS: OnceLock<Vec<HookPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // --- Error: Rules of Hooks violations ---
            HookPattern {
                name: "Hook inside condition",
                regex: Regex::new(r"if\s*\(.*\).*\buse[A-Z]\w+\s*\(").unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                suggestion: "Move hook call to top level of the component",
                fix_kind: FixKind::Suggestion,
            },
            HookPattern {
                name: "Hook inside loop",
                regex: Regex::new(r"(?:for\s*\(|while\s*\().*\buse[A-Z]\w+\s*\(").unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                suggestion: "Move hook call to top level of the component",
                fix_kind: FixKind::Suggestion,
            },
            // --- Warning: Common anti-patterns ---
            HookPattern {
                name: "useEffect without dependency array",
                regex: Regex::new(r"useEffect\s*\(").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some(", ["),
                suggestion: "Add dependency array: useEffect(() => { ... }, [deps])",
                fix_kind: FixKind::Suggestion,
            },
            HookPattern {
                name: "Direct DOM manipulation",
                regex: Regex::new(
                    r"document\.(?:getElementById|querySelector|querySelectorAll|getElementsBy)\s*\(",
                )
                .unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion: "Use useRef() hook instead of direct DOM manipulation",
                fix_kind: FixKind::Suggestion,
            },
            HookPattern {
                name: "Missing key prop in map",
                regex: Regex::new(r"\.map\s*\(.*=>\s*<[A-Z]").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("key="),
                suggestion: "Add a unique key prop: <Component key={item.id} />",
                fix_kind: FixKind::Suggestion,
            },
            HookPattern {
                name: "dangerouslySetInnerHTML usage",
                regex: Regex::new(r"dangerouslySetInnerHTML").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                suggestion:
                    "Avoid dangerouslySetInnerHTML; use a sanitization library like DOMPurify if needed",
                fix_kind: FixKind::Suggestion,
            },
            // --- Info: Performance hints ---
            HookPattern {
                name: "Inline function in JSX event handler",
                regex: Regex::new(r"on[A-Z]\w+=\{.*=>").unwrap(),
                severity: Severity::Info,
                reject_if_contains: None,
                suggestion:
                    "Extract handler to useCallback() to prevent unnecessary re-renders",
                fix_kind: FixKind::Suggestion,
            },
            HookPattern {
                name: "useEffect with empty dependency array",
                regex: Regex::new(r"useEffect\s*\(.*,\s*\[\s*\]\s*\)").unwrap(),
                severity: Severity::Info,
                reject_if_contains: None,
                suggestion: "Empty dependency array means this runs once on mount; ensure it does not reference props or state that may change",
                fix_kind: FixKind::Suggestion,
            },
        ]
    })
}

/// React file extensions to scan
const REACT_EXTENSIONS: &[&str] = &["tsx", "jsx", "ts", "js"];

/// Analyzer that detects React hooks misuse and common anti-patterns
pub struct ReactHooksAnalyzer;

impl ReactHooksAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned based on its extension
    fn should_scan(path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| REACT_EXTENSIONS.contains(&e))
            .unwrap_or(false)
    }

    /// Check if a line is a comment
    fn is_comment_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with("//")
            || trimmed.starts_with('*')
            || trimmed.starts_with("/*")
            || trimmed.starts_with("{/*")
    }

    /// Scan a single file for React hooks issues
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

impl Default for ReactHooksAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for ReactHooksAnalyzer {
    fn name(&self) -> &str {
        "React Hooks"
    }

    fn finding_prefix(&self) -> &str {
        "HOOKS"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.react
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
