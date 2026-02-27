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

// ── Compiled regexes for file-level K8s / Dockerfile checks ──────────────────

fn re_k8s_containers() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*containers:\s*$").unwrap())
}

fn re_k8s_readiness() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"readinessProbe:").unwrap())
}

fn re_k8s_liveness() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"livenessProbe:").unwrap())
}

fn re_k8s_resources() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*resources:").unwrap())
}

fn re_dockerfile_user() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^USER\s+").unwrap())
}

fn re_dockerfile_user_root() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^USER\s+(?:root|0)\s*$").unwrap())
}

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
            // Pattern 8: K8s image :latest tag in pod spec
            InfraPattern {
                name: "K8s container image pinned to :latest (non-reproducible deploy)",
                regex: Regex::new(r"^\s*image:\s+\S+:latest\s*$").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &["yaml", "yml"],
                target_filenames: &[],
                suggestion: "Pin the image to a specific digest or version tag (e.g. nginx:1.25.3)",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 9: Docker ADD instruction (implicit tar extraction / remote URL risk)
            InfraPattern {
                name: "Docker ADD instruction (use COPY unless tar-extraction is needed)",
                regex: Regex::new(r"(?i)^ADD\s+").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &[],
                target_filenames: &["Dockerfile"],
                suggestion: "Use COPY instead of ADD; ADD silently extracts tarballs and can fetch remote URLs",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 10: Docker USER root
            InfraPattern {
                name: "Docker USER root (container runs as root)",
                regex: Regex::new(r"(?i)^USER\s+(?:root|0)\s*$").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &[],
                target_filenames: &["Dockerfile"],
                suggestion: "Create and switch to a non-root user: RUN useradd -m appuser && USER appuser",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 11: Docker COPY . . (copies .env, secrets, entire repo into image)
            InfraPattern {
                name: "Docker COPY . . copies entire context (may include .env or secrets)",
                regex: Regex::new(r"(?i)^COPY\s+\.\s+[./]").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &[],
                target_filenames: &["Dockerfile"],
                suggestion: "Use a .dockerignore file to exclude .env, secrets, and other sensitive files",
                fix_kind: FixKind::Suggestion,
            },
            // ── Info: best practice ──────────────────────────────────────
            // Pattern 12: HTTP backend/source URL
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

        let lines: Vec<&str> = content.lines().collect();

        if !applicable.is_empty() {
            for (line_num, &line) in lines.iter().enumerate() {
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
        }

        // ── File-level: K8s missing probes and resource limits ────────────────
        let is_k8s_yaml = matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yaml") | Some("yml")
        );
        if is_k8s_yaml {
            let has_containers = lines.iter().any(|l| re_k8s_containers().is_match(l));
            if has_containers {
                if !lines.iter().any(|l| re_k8s_readiness().is_match(l)) {
                    if let Some((ln, _)) = lines
                        .iter()
                        .enumerate()
                        .find(|(_, l)| re_k8s_containers().is_match(l))
                    {
                        findings.push(make_finding(
                            Severity::Warning,
                            "Infrastructure issue: K8s Deployment missing readinessProbe (pod receives traffic before ready)".to_string(),
                            path.to_path_buf(),
                            ln + 1,
                            Some("Add a readinessProbe to each container so the pod is only sent traffic when healthy".to_string()),
                            Some(FixKind::Suggestion),
                        ));
                    }
                }
                if !lines.iter().any(|l| re_k8s_liveness().is_match(l)) {
                    if let Some((ln, _)) = lines
                        .iter()
                        .enumerate()
                        .find(|(_, l)| re_k8s_containers().is_match(l))
                    {
                        findings.push(make_finding(
                            Severity::Warning,
                            "Infrastructure issue: K8s Deployment missing livenessProbe (stuck pods won't be restarted)".to_string(),
                            path.to_path_buf(),
                            ln + 1,
                            Some("Add a livenessProbe to each container so Kubernetes can restart unhealthy pods".to_string()),
                            Some(FixKind::Suggestion),
                        ));
                    }
                }
                if !lines.iter().any(|l| re_k8s_resources().is_match(l)) {
                    if let Some((ln, _)) = lines
                        .iter()
                        .enumerate()
                        .find(|(_, l)| re_k8s_containers().is_match(l))
                    {
                        findings.push(make_finding(
                            Severity::Warning,
                            "Infrastructure issue: K8s Deployment missing resource limits/requests (noisy-neighbour risk)".to_string(),
                            path.to_path_buf(),
                            ln + 1,
                            Some("Set resources.requests and resources.limits for CPU and memory on each container".to_string()),
                            Some(FixKind::Suggestion),
                        ));
                    }
                }
            }
        }

        // ── File-level: Dockerfile missing USER instruction ───────────────────
        let is_dockerfile = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "Dockerfile" || n.starts_with("Dockerfile."))
            .unwrap_or(false);
        if is_dockerfile {
            // FROM scratch is a special base with no shell; USER is not applicable
            let all_scratch = lines
                .iter()
                .filter(|l| {
                    let t = l.trim_start().to_lowercase();
                    t.starts_with("from ")
                })
                .all(|l| l.trim_start().to_lowercase().contains("scratch"));

            let has_user = lines.iter().any(|l| re_dockerfile_user().is_match(l));
            let has_user_root = lines.iter().any(|l| re_dockerfile_user_root().is_match(l));
            // Flag only when there is no USER at all, or only USER root (and not a scratch-only image)
            if !all_scratch && (!has_user || has_user_root) {
                // Report on the last FROM line (base image), or line 1
                let from_line = lines
                    .iter()
                    .enumerate()
                    .rfind(|(_, l)| l.trim_start().to_lowercase().starts_with("from "))
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                findings.push(make_finding(
                    Severity::Warning,
                    "Infrastructure issue: Dockerfile runs as root (no non-root USER instruction)"
                        .to_string(),
                    path.to_path_buf(),
                    from_line + 1,
                    Some("Add a non-root user: RUN useradd -m appuser && USER appuser".to_string()),
                    Some(FixKind::Suggestion),
                ));
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
