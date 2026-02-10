//! Infrastructure analyzer — detects Terraform, Kubernetes, and Docker misconfigurations
//!
//! Scans raw file content line-by-line for patterns indicating security issues,
//! overly permissive configs, and non-reproducible builds.
//! Targets: `.tf`, `.tfvars`, `.yaml`, `.yml`, `Dockerfile`.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled infrastructure detection pattern
struct InfraPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    /// If set, the line must NOT contain this substring (negative filter)
    reject_if_contains: Option<&'static str>,
    /// File extensions this pattern targets (e.g., "tf", "yaml")
    target_extensions: &'static [&'static str],
    /// Exact filenames this pattern targets (e.g., "Dockerfile")
    target_filenames: &'static [&'static str],
    suggestion: &'static str,
    fix_kind: FixKind,
}

/// Returns all infrastructure patterns in priority order (Error → Warning → Info)
fn patterns() -> &'static [InfraPattern] {
    static PATTERNS: OnceLock<Vec<InfraPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Error: critical security issues ──────────────────────────
            // Pattern 1: Public S3 bucket ACL
            InfraPattern {
                name: "public S3 bucket ACL (exposes bucket to internet)",
                regex: Regex::new(r#"acl\s*=\s*["']public-read(?:-write)?["']"#).unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                target_extensions: &["tf"],
                target_filenames: &[],
                suggestion: "Set ACL to \"private\" to restrict bucket access",
                fix_kind: FixKind::ReplacePattern {
                    find: r#"public-read(?:-write)?"#.to_string(),
                    replace: "private".to_string(),
                },
            },
            // Pattern 2: Open security group (0.0.0.0/0)
            InfraPattern {
                name: "open security group 0.0.0.0/0 (exposes service to internet)",
                regex: Regex::new(r#"cidr_blocks\s*=\s*\[.*["']0\.0\.0\.0/0["']"#).unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                target_extensions: &["tf"],
                target_filenames: &[],
                suggestion: "Restrict CIDR block to specific IP ranges instead of 0.0.0.0/0",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 3: Hardcoded provider credentials
            InfraPattern {
                name: "hardcoded provider credentials in Terraform",
                regex: Regex::new(r#"(?:access_key|secret_key)\s*=\s*["'][A-Za-z0-9/+=]{16,}["']"#)
                    .unwrap(),
                severity: Severity::Error,
                reject_if_contains: Some("var."),
                target_extensions: &["tf", "tfvars"],
                target_filenames: &[],
                suggestion: "Use Terraform variables or environment variables for credentials",
                fix_kind: FixKind::Suggestion,
            },
            // ── Warning: likely problematic ──────────────────────────────
            // Pattern 4: Wildcard IAM actions
            InfraPattern {
                name: "wildcard IAM action (violates least-privilege)",
                regex: Regex::new(r#"["']?(?:actions|Action)["']?\s*[:=]\s*\[?\s*["']\*["']"#)
                    .unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("NotAction"),
                target_extensions: &["tf", "json"],
                target_filenames: &[],
                suggestion: "Specify explicit IAM actions instead of using wildcard \"*\"",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 5: Docker FROM :latest or no tag
            InfraPattern {
                name: "Docker FROM :latest or untagged (non-reproducible build)",
                regex: Regex::new(r"(?i)^FROM\s+[^\s:]+(?::latest\s*$|\s*$)").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("scratch"),
                target_extensions: &[],
                target_filenames: &["Dockerfile"],
                suggestion: "Pin Docker image to a specific version tag for reproducible builds",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 6: Privileged container
            InfraPattern {
                name: "privileged container (root access to host)",
                regex: Regex::new(r"privileged:\s*true").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &["yaml", "yml"],
                target_filenames: &[],
                suggestion: "Set privileged: false unless root access is strictly required",
                fix_kind: FixKind::ReplacePattern {
                    find: r"privileged:\s*true".to_string(),
                    replace: "privileged: false".to_string(),
                },
            },
            // Pattern 7: HostPath volume mount
            InfraPattern {
                name: "hostPath volume mount (container escape vector)",
                regex: Regex::new(r"hostPath:\s*$").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &["yaml", "yml"],
                target_filenames: &[],
                suggestion: "Use emptyDir, configMap, or PVC instead of hostPath volumes",
                fix_kind: FixKind::Suggestion,
            },
            // ── Info: best practice ──────────────────────────────────────
            // Pattern 8: HTTP backend/source URL
            InfraPattern {
                name: "HTTP URL in Terraform config (use HTTPS)",
                regex: Regex::new(r#"(?:source|endpoint|url)\s*=\s*["']http://"#).unwrap(),
                severity: Severity::Info,
                reject_if_contains: Some("localhost"),
                target_extensions: &["tf"],
                target_filenames: &[],
                suggestion: "Use HTTPS instead of HTTP for secure communication",
                fix_kind: FixKind::ReplacePattern {
                    find: r"http://".to_string(),
                    replace: "https://".to_string(),
                },
            },
        ]
    })
}

/// All file extensions the infra analyzer may scan
const INFRA_EXTENSIONS: &[&str] = &["tf", "tfvars", "yaml", "yml", "json"];

/// Exact filenames the infra analyzer may scan
const INFRA_FILENAMES: &[&str] = &["Dockerfile"];

/// Binary file extensions to skip
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "woff", "woff2", "ttf", "eot", "otf",
    "zip", "gz", "tar", "bz2", "xz", "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx", "ppt",
    "pptx", "exe", "dll", "so", "dylib", "o", "a", "pyc", "pyo", "class", "lock", "mp3", "mp4",
    "avi", "mov", "wav", "flac", "sqlite", "db",
];

/// Analyzer that detects infrastructure misconfigurations
pub struct InfraAnalyzer;

impl InfraAnalyzer {
    /// Create a new infrastructure analyzer
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned (must match infra file types, not binary)
    fn should_scan(path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check exact filename match (e.g., "Dockerfile")
        if INFRA_FILENAMES.contains(&file_name) {
            return true;
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return false,
        };

        if BINARY_EXTENSIONS.contains(&ext.as_str()) {
            return false;
        }

        INFRA_EXTENSIONS.contains(&ext.as_str())
    }

    /// Check if a pattern applies to a given file
    fn pattern_matches_file(pattern: &InfraPattern, path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check exact filename match
        if !pattern.target_filenames.is_empty() && pattern.target_filenames.contains(&file_name) {
            return true;
        }

        // Check extension match
        if !pattern.target_extensions.is_empty() {
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e.to_lowercase(),
                None => return false,
            };
            return pattern.target_extensions.contains(&ext.as_str());
        }

        false
    }

    /// Check if a line is a comment (should be skipped)
    fn is_comment_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.starts_with('*')
    }

    /// Scan a single file for infrastructure patterns
    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        // Filter patterns applicable to this file
        let applicable: Vec<&InfraPattern> = all_patterns
            .iter()
            .filter(|p| Self::pattern_matches_file(p, path))
            .collect();

        if applicable.is_empty() {
            return findings;
        }

        for (line_num, line) in content.lines().enumerate() {
            if Self::is_comment_line(line) {
                continue;
            }

            // First matching pattern wins for this line
            for pat in &applicable {
                if !pat.regex.is_match(line) {
                    continue;
                }

                // Apply negative filter: skip if line contains rejected substring
                if let Some(reject) = pat.reject_if_contains {
                    if line.contains(reject) {
                        continue;
                    }
                }

                findings.push(make_finding(
                    pat.severity,
                    format!("Infrastructure issue: {}", pat.name),
                    path.to_path_buf(),
                    line_num + 1,
                    Some(pat.suggestion.to_string()),
                    Some(pat.fix_kind.clone()),
                ));
                break;
            }
        }

        findings
    }
}

impl Default for InfraAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for InfraAnalyzer {
    fn name(&self) -> &str {
        "Infrastructure"
    }

    fn finding_prefix(&self) -> &str {
        "INFRA"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.infra
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

    fn extra_extensions(&self) -> &[&str] {
        &[".tf", ".tfvars", ".yaml", ".yml", ".json"]
    }

    fn extra_filenames(&self) -> &[&str] {
        &["Dockerfile"]
    }
}
