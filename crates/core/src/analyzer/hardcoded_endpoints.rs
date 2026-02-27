//! Hardcoded endpoints analyzer — detects IP addresses and prod/staging URLs baked into code
//!
//! Hardcoding internal IPs or environment-specific URLs couples code to infrastructure,
//! breaks across environments, and leaks topology. Production URLs in source are also
//! an accidental exposure risk.
//!
//! Detects:
//! - Private IPv4 ranges (RFC 1918): 10.x.x.x, 192.168.x.x, 172.16-31.x.x
//! - Hardcoded IP addresses in URLs: `http://1.2.3.4/api`
//! - Production/staging URLs with explicit env in subdomain: `https://api.prod.example.com`
//!
//! Disabled by default (`modules.hardcoded_endpoints = false`).

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct EndpointPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    suggestion: &'static str,
}

fn patterns() -> &'static [EndpointPattern] {
    static PATTERNS: OnceLock<Vec<EndpointPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Private IP ranges (RFC 1918) ──────────────────────────────────
            EndpointPattern {
                name: "hardcoded private IP (10.x.x.x)",
                regex: Regex::new(r"\b10\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap(),
                severity: Severity::Warning,
                suggestion: "Use an environment variable or config file for internal IP addresses",
            },
            EndpointPattern {
                name: "hardcoded private IP (192.168.x.x)",
                regex: Regex::new(r"\b192\.168\.\d{1,3}\.\d{1,3}\b").unwrap(),
                severity: Severity::Warning,
                suggestion: "Use an environment variable or config file for internal IP addresses",
            },
            EndpointPattern {
                name: "hardcoded private IP (172.16-31.x.x)",
                regex: Regex::new(r"\b172\.(1[6-9]|2\d|3[01])\.\d{1,3}\.\d{1,3}\b").unwrap(),
                severity: Severity::Warning,
                suggestion: "Use an environment variable or config file for internal IP addresses",
            },
            // ── IP address in URL ─────────────────────────────────────────────
            EndpointPattern {
                name: "hardcoded IP address in URL",
                regex: Regex::new(r"https?://\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap(),
                severity: Severity::Warning,
                suggestion: "Replace the hardcoded IP with a named host configured via environment variable",
            },
            // ── Production / staging URLs ─────────────────────────────────────
            EndpointPattern {
                name: "hardcoded production URL",
                // Matches URLs where a subdomain component is exactly 'prod' or 'production'
                // e.g. https://api.prod.example.com, https://production.myapp.io
                regex: Regex::new(
                    r"(?i)https?://(?:[a-zA-Z0-9-]+\.)*(?:prod|production)\b[a-zA-Z0-9.-]*/?"
                )
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Store production URLs in environment variables; use a config abstraction layer",
            },
            EndpointPattern {
                name: "hardcoded staging URL",
                regex: Regex::new(
                    r"(?i)https?://(?:[a-zA-Z0-9-]+\.)*(?:staging|stage)\b[a-zA-Z0-9.-]*/?"
                )
                .unwrap(),
                severity: Severity::Warning,
                suggestion: "Store staging URLs in environment variables; do not couple code to environments",
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

/// Analyzer that detects hardcoded IP addresses and environment-specific URLs
pub struct HardcodedEndpointsAnalyzer;

impl HardcodedEndpointsAnalyzer {
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
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            for pat in all_patterns {
                if !pat.regex.is_match(line) {
                    continue;
                }
                findings.push(make_finding(
                    pat.severity,
                    format!("Hardcoded endpoint: {}", pat.name),
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

impl Default for HardcodedEndpointsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for HardcodedEndpointsAnalyzer {
    fn name(&self) -> &str {
        "Hardcoded Endpoints"
    }

    fn finding_prefix(&self) -> &str {
        "ENDPT"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.hardcoded_endpoints
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
