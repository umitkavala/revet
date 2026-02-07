//! Secret exposure analyzer — detects hardcoded secrets, API keys, and credentials
//!
//! Scans raw file content line-by-line for patterns that indicate exposed secrets.
//! Only one finding per line (first matching pattern wins) to reduce noise.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled secret detection pattern
struct SecretPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
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
            },
            SecretPattern {
                name: "AWS Secret Access Key",
                regex: Regex::new(r#"(?i)aws.{0,20}['"][0-9a-zA-Z/+=]{40}['"]"#).unwrap(),
                severity: Severity::Error,
            },
            SecretPattern {
                name: "GitHub Token",
                regex: Regex::new(r"gh[pousr]_[A-Za-z0-9_]{36,}").unwrap(),
                severity: Severity::Error,
            },
            SecretPattern {
                name: "Private Key (PEM)",
                regex: Regex::new(r"-----BEGIN (?:RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----")
                    .unwrap(),
                severity: Severity::Error,
            },
            SecretPattern {
                name: "Database Connection String",
                regex: Regex::new(r#"(?i)(?:mongodb|postgres|mysql|redis)://[^\s'"]+:[^\s'"]+@"#)
                    .unwrap(),
                severity: Severity::Error,
            },
            SecretPattern {
                name: "Generic API Key",
                regex: Regex::new(r#"(?i)api[_\-]?key\s*[:=]\s*['"][a-zA-Z0-9]{20,}['"]"#).unwrap(),
                severity: Severity::Warning,
            },
            SecretPattern {
                name: "Generic Secret Key",
                regex: Regex::new(r#"(?i)secret[_\-]?key\s*[:=]\s*['"][a-zA-Z0-9]{20,}['"]"#)
                    .unwrap(),
                severity: Severity::Warning,
            },
            SecretPattern {
                name: "Hardcoded Password",
                regex: Regex::new(r#"(?i)password\s*[:=]\s*['"][^'"]{8,}['"]"#).unwrap(),
                severity: Severity::Warning,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_should_scan_source_files() {
        assert!(SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "main.py"
        )));
        assert!(SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "config.ts"
        )));
        assert!(SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "Makefile"
        )));
    }

    #[test]
    fn test_should_skip_binary_files() {
        assert!(!SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "logo.png"
        )));
        assert!(!SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "font.woff2"
        )));
        assert!(!SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "archive.zip"
        )));
        assert!(!SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "data.db"
        )));
    }

    #[test]
    fn test_should_skip_minified_files() {
        assert!(!SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "bundle.min.js"
        )));
        assert!(!SecretExposureAnalyzer::should_scan(&PathBuf::from(
            "style.min.css"
        )));
    }

    #[test]
    fn test_aws_key_pattern() {
        let pats = patterns();
        let aws_pat = &pats[0]; // AWS Access Key ID
        assert!(aws_pat.regex.is_match("AKIAIOSFODNN7EXAMPLE"));
        assert!(!aws_pat.regex.is_match("not_an_aws_key"));
    }

    #[test]
    fn test_github_token_pattern() {
        let pats = patterns();
        let gh_pat = &pats[2]; // GitHub Token
        assert!(gh_pat
            .regex
            .is_match("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijkl"));
        assert!(!gh_pat.regex.is_match("gh_tooshort"));
    }

    #[test]
    fn test_private_key_pattern() {
        let pats = patterns();
        let pem_pat = &pats[3]; // Private Key (PEM)
        assert!(pem_pat.regex.is_match("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(pem_pat.regex.is_match("-----BEGIN PRIVATE KEY-----"));
        assert!(pem_pat.regex.is_match("-----BEGIN EC PRIVATE KEY-----"));
        assert!(!pem_pat.regex.is_match("-----BEGIN PUBLIC KEY-----"));
    }

    #[test]
    fn test_connection_string_pattern() {
        let pats = patterns();
        let conn_pat = &pats[4]; // Database Connection String
        assert!(conn_pat
            .regex
            .is_match("postgres://admin:secret123@db.example.com:5432/mydb"));
        assert!(conn_pat
            .regex
            .is_match("mongodb://user:pass@mongo.host/dbname"));
        assert!(!conn_pat.regex.is_match("postgres://localhost/mydb")); // No credentials
    }

    #[test]
    fn test_password_pattern() {
        let pats = patterns();
        let pw_pat = &pats[7]; // Hardcoded Password
        assert!(pw_pat.regex.is_match(r#"password = "my_super_secret""#));
        assert!(pw_pat.regex.is_match(r#"PASSWORD: 'longpassword123'"#));
        assert!(!pw_pat.regex.is_match(r#"password = "short""#)); // Too short (<8)
    }
}
