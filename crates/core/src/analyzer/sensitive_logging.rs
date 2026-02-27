//! Sensitive logging analyzer — detects secrets/credentials passed to log or print calls
//!
//! Logging sensitive data such as passwords, tokens, or API keys exposes secrets in
//! log files, which are often stored or forwarded to third-party aggregators.
//! This analyzer flags log/print calls where a sensitive-named variable appears as
//! an argument (CWE-532: Insertion of Sensitive Information into Log File).
//!
//! Covers Python, JavaScript/TypeScript, PHP, Go, Java, and Ruby.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct LogPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    suggestion: &'static str,
    extensions: &'static [&'static str],
}

/// Sensitive identifier names (word-boundary matched in each regex)
///
/// Covers common naming conventions across languages. PHP patterns additionally
/// require the `$` sigil. Go/Java patterns include camelCase variants.
const SENSITIVE: &str =
    r"password|passwd|pwd|secret|token|api[_-]?key|credential|auth[_-]?key|private[_-]?key";

const SENSITIVE_CAMEL: &str = r"password|passwd|pwd|secret|token|apiKey|api[_-]?key|credential|authKey|auth[_-]?key|privateKey|private[_-]?key";

fn patterns() -> &'static [LogPattern] {
    static PATTERNS: OnceLock<Vec<LogPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Python ────────────────────────────────────────────────────────
            LogPattern {
                name: "sensitive variable passed to Python logging call",
                regex: Regex::new(&format!(
                    r"(?i)\b(?:logging|log|logger)\.(?:debug|info|warning|warn|error|critical|exception)\s*\(.*\b(?:{SENSITIVE})\b"
                ))
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Never log raw credentials; redact or omit the sensitive field before logging",
                extensions: &["py"],
            },
            LogPattern {
                name: "sensitive variable passed to Python print()",
                regex: Regex::new(&format!(
                    r"(?i)\bprint\s*\(.*\b(?:{SENSITIVE})\b"
                ))
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Remove the sensitive value from the print() call",
                extensions: &["py"],
            },
            // ── JavaScript / TypeScript ───────────────────────────────────────
            LogPattern {
                name: "sensitive variable passed to console.*",
                regex: Regex::new(&format!(
                    r"(?i)\bconsole\.(?:log|info|warn|error|debug|trace)\s*\(.*\b(?:{SENSITIVE})\b"
                ))
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Remove sensitive values from console calls; use a redaction helper",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            LogPattern {
                name: "sensitive variable passed to logger.* (JS/TS)",
                regex: Regex::new(&format!(
                    r"(?i)\blogger\.(?:log|info|warn|error|debug|trace|fatal)\s*\(.*\b(?:{SENSITIVE})\b"
                ))
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Redact or omit the sensitive field before passing it to the logger",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            // ── Go ────────────────────────────────────────────────────────────
            LogPattern {
                name: "sensitive variable passed to fmt.Print / log.Print (Go)",
                regex: Regex::new(&format!(
                    r"\b(?:fmt\.Print(?:f|ln)?|log\.Print(?:f|ln)?|log\.Fatal(?:f|ln)?|log\.Panic(?:f|ln)?)\s*\(.*\b(?:{SENSITIVE_CAMEL})\b"
                ))
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Redact the sensitive field before logging; use structured logging with redaction",
                extensions: &["go"],
            },
            // ── Java ──────────────────────────────────────────────────────────
            LogPattern {
                name: "sensitive variable passed to System.out.println or logger (Java)",
                regex: Regex::new(&format!(
                    r"(?i)(?:System\.out\.print(?:ln)?|(?:log|logger)\.(?:info|debug|warn|error|trace|fatal))\s*\(.*\b(?:{SENSITIVE_CAMEL})\b"
                ))
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Do not log raw credentials; mask or omit the sensitive field",
                extensions: &["java"],
            },
            // ── PHP ───────────────────────────────────────────────────────────
            LogPattern {
                name: "sensitive variable passed to error_log / var_dump (PHP)",
                regex: Regex::new(
                    r"(?i)\b(?:error_log|var_dump|print_r)\s*\(.*\$(?:password|passwd|secret|token|api_key|credential|auth_key|private_key)\b",
                )
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Never dump raw credentials to error_log or var_dump",
                extensions: &["php"],
            },
            // ── Ruby ──────────────────────────────────────────────────────────
            LogPattern {
                name: "sensitive variable passed to puts / p (Ruby)",
                regex: Regex::new(&format!(
                    r"(?i)\b(?:puts|p|pp|print)\s+(?:\w+\.)?\b(?:{SENSITIVE})\b"
                ))
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Remove the sensitive variable from the puts/p call",
                extensions: &["rb"],
            },
        ]
    })
}

const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "woff", "woff2", "ttf", "eot", "otf",
    "zip", "gz", "tar", "bz2", "xz", "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx", "ppt",
    "pptx", "exe", "dll", "so", "dylib", "o", "a", "pyc", "pyo", "class", "lock", "mp3", "mp4",
    "avi", "mov", "wav", "flac", "sqlite", "db",
];

/// Analyzer that detects sensitive data (credentials, secrets) passed to log/print calls
pub struct SensitiveLoggingAnalyzer;

impl SensitiveLoggingAnalyzer {
    pub fn new() -> Self {
        Self
    }

    fn should_scan(path: &Path) -> bool {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return true,
        };
        !BINARY_EXTENSIONS.contains(&ext.as_str())
    }

    fn scan_file(path: &Path) -> Vec<Finding> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            for pat in all_patterns {
                if !pat.extensions.is_empty() && !pat.extensions.contains(&ext.as_str()) {
                    continue;
                }
                if !pat.regex.is_match(line) {
                    continue;
                }
                findings.push(make_finding(
                    pat.severity,
                    format!("Sensitive data in log: {}", pat.name),
                    path.to_path_buf(),
                    line_num + 1,
                    Some(pat.suggestion.to_string()),
                    Some(FixKind::Suggestion),
                ));
                break; // One finding per line
            }
        }

        findings
    }
}

impl Default for SensitiveLoggingAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for SensitiveLoggingAnalyzer {
    fn name(&self) -> &str {
        "Sensitive Logging"
    }

    fn finding_prefix(&self) -> &str {
        "LOG"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.security
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
