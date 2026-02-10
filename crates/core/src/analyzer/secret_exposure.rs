//! Secret exposure analyzer — detects hardcoded secrets, API keys, and credentials
//!
//! Scans raw file content line-by-line for patterns that indicate exposed secrets.
//! Only one finding per line (first matching pattern wins) to reduce noise.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled secret detection pattern
struct SecretPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    suggestion: &'static str,
    fix_kind: FixKind,
}

/// Returns all secret patterns in priority order (Error patterns first)
fn patterns() -> &'static [SecretPattern] {
    static PATTERNS: OnceLock<Vec<SecretPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            SecretPattern {
                name: "AWS Access Key ID",
                regex: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
                severity: Severity::Error,
                suggestion: "Use environment variable AWS_ACCESS_KEY_ID instead",
                fix_kind: FixKind::CommentOut,
            },
            SecretPattern {
                name: "AWS Secret Access Key",
                regex: Regex::new(r#"(?i)aws.{0,20}['"][0-9a-zA-Z/+=]{40}['"]"#).unwrap(),
                severity: Severity::Error,
                suggestion: "Use environment variable AWS_SECRET_ACCESS_KEY instead",
                fix_kind: FixKind::CommentOut,
            },
            SecretPattern {
                name: "GitHub Token",
                regex: Regex::new(r"gh[pousr]_[A-Za-z0-9_]{36,}").unwrap(),
                severity: Severity::Error,
                suggestion: "Use environment variable GITHUB_TOKEN instead",
                fix_kind: FixKind::CommentOut,
            },
            SecretPattern {
                name: "Private Key (PEM)",
                regex: Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----")
                    .unwrap(),
                severity: Severity::Error,
                suggestion: "Store private key in a file outside the repo and reference via path",
                fix_kind: FixKind::CommentOut,
            },
            SecretPattern {
                name: "Database Connection String",
                regex: Regex::new(r#"(?i)(?:mongodb|postgres|mysql|redis)://[^\s'"]+:[^\s'"]+@"#)
                    .unwrap(),
                severity: Severity::Error,
                suggestion: "Store connection string in .env file or use a secrets manager",
                fix_kind: FixKind::CommentOut,
            },
            SecretPattern {
                name: "Generic API Key",
                regex: Regex::new(r#"(?i)api[_\-]?key\s*[:=]\s*['"][a-zA-Z0-9]{20,}['"]"#).unwrap(),
                severity: Severity::Warning,
                suggestion: "Store API key in environment variable or .env file",
                fix_kind: FixKind::CommentOut,
            },
            SecretPattern {
                name: "Generic Secret Key",
                regex: Regex::new(r#"(?i)secret[_\-]?key\s*[:=]\s*['"][a-zA-Z0-9]{20,}['"]"#)
                    .unwrap(),
                severity: Severity::Warning,
                suggestion: "Store secret key in environment variable or .env file",
                fix_kind: FixKind::CommentOut,
            },
            SecretPattern {
                name: "Hardcoded Password",
                regex: Regex::new(r#"(?i)password\s*[:=]\s*['"][^'"]{8,}['"]"#).unwrap(),
                severity: Severity::Warning,
                suggestion: "Store password in environment variable or use a secrets manager",
                fix_kind: FixKind::CommentOut,
            },
        ]
    })
}

/// Binary file extensions to skip
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "woff", "woff2", "ttf", "eot", "otf",
    "zip", "gz", "tar", "bz2", "xz", "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx", "ppt",
    "pptx", "exe", "dll", "so", "dylib", "o", "a", "pyc", "pyo", "class", "lock", "mp3", "mp4",
    "avi", "mov", "wav", "flac", "sqlite", "db",
];

/// Analyzer that detects hardcoded secrets in source files
pub struct SecretExposureAnalyzer;

impl SecretExposureAnalyzer {
    /// Create a new secret exposure analyzer
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned based on its extension
    fn should_scan(path: &Path) -> bool {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return true, // No extension — scan it
        };

        // Check compound extension like .min.js
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.ends_with(".min.js") || file_name.ends_with(".min.css") {
            return false;
        }

        !BINARY_EXTENSIONS.contains(&ext.as_str())
    }

    /// Scan a single file for secrets
    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(), // Skip unreadable files
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // First matching pattern wins for this line
            for pat in all_patterns {
                if pat.regex.is_match(line) {
                    findings.push(make_finding(
                        pat.severity,
                        format!("Possible {} detected", pat.name),
                        path.to_path_buf(),
                        line_num + 1, // 1-indexed
                        Some(pat.suggestion.to_string()),
                        Some(pat.fix_kind.clone()),
                    ));
                    break; // One finding per line
                }
            }
        }

        findings
    }
}

impl Default for SecretExposureAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for SecretExposureAnalyzer {
    fn name(&self) -> &str {
        "Secret Exposure"
    }

    fn finding_prefix(&self) -> &str {
        "SEC"
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
