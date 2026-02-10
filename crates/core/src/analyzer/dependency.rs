//! Dependency hygiene analyzer — detects import anti-patterns and manifest issues
//!
//! Scans raw file content line-by-line for patterns indicating wildcard imports,
//! deprecated modules, circular dependency workarounds, and unpinned versions.
//! Targets: `.py`, `.java`, `.ts`, `.js`, `.tsx`, `.jsx`, `package.json`,
//! `requirements.txt`, `Cargo.toml`, `pyproject.toml`.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled dependency detection pattern
struct DepPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    /// If set, the line must NOT contain this substring (negative filter)
    reject_if_contains: Option<&'static str>,
    /// File extensions this pattern targets (e.g., "py", "java")
    target_extensions: &'static [&'static str],
    /// Exact filenames this pattern targets (e.g., "package.json")
    target_filenames: &'static [&'static str],
    suggestion: &'static str,
    fix_kind: FixKind,
}

/// Returns all dependency patterns in priority order (Warning → Info)
fn patterns() -> &'static [DepPattern] {
    static PATTERNS: OnceLock<Vec<DepPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Warning: likely problematic ──────────────────────────────
            // Pattern 1: Wildcard import (Python)
            DepPattern {
                name: "wildcard import in Python (pollutes namespace)",
                regex: Regex::new(r"^\s*from\s+\S+\s+import\s+\*").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &["py"],
                target_filenames: &[],
                suggestion: "Import specific names instead of using wildcard import",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 2: Wildcard import (Java)
            DepPattern {
                name: "wildcard import in Java (pollutes namespace)",
                regex: Regex::new(r"^\s*import\s+[\w.]+\.\*\s*;").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &["java"],
                target_filenames: &[],
                suggestion: "Import specific classes instead of using wildcard import",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 3: Deprecated Python import
            DepPattern {
                name: "deprecated Python module import (removed in 3.12+)",
                regex: Regex::new(
                    r"^\s*(?:import\s+(?:imp|optparse|distutils|aifc|audioop|cgi|cgitb|smtpd|pipes|sndhdr|sunau|nntplib|xdrlib|msilib|imghdr|formatter)|from\s+(?:imp|optparse|distutils|aifc|audioop|cgi|cgitb|smtpd|pipes|sndhdr|sunau|nntplib|xdrlib|msilib|imghdr|formatter)\s+import)"
                ).unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &["py"],
                target_filenames: &[],
                suggestion: "This module is deprecated/removed in Python 3.12+; use its modern replacement",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 6: Circular import workaround
            DepPattern {
                name: "circular import workaround annotation",
                regex: Regex::new(
                    r"(?:#\s*noqa:\s*circular|#\s*type:\s*ignore\[import|//\s*@ts-ignore|//\s*eslint-disable.*import)"
                ).unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &["py", "ts", "js", "tsx", "jsx"],
                target_filenames: &[],
                suggestion: "Resolve the circular dependency instead of suppressing the lint",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 7: Unpinned/wildcard dep version
            DepPattern {
                name: "unpinned or wildcard dependency version",
                regex: Regex::new(r#"["']\s*:\s*["'](?:\*|latest)["']"#).unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                target_extensions: &[],
                target_filenames: &["package.json"],
                suggestion: "Pin dependency to a specific version or semver range",
                fix_kind: FixKind::Suggestion,
            },
            // ── Info: best practice ──────────────────────────────────────
            // Pattern 4: require() instead of import
            DepPattern {
                name: "require() instead of ES import",
                regex: Regex::new(r"(?:const|let|var)\s+\w+\s*=\s*require\s*\(").unwrap(),
                severity: Severity::Info,
                reject_if_contains: Some("jest"),
                target_extensions: &["ts", "js", "tsx", "jsx"],
                target_filenames: &[],
                suggestion: "Use ES module import syntax instead of require()",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 5: Deeply nested relative import (3+)
            DepPattern {
                name: "deeply nested relative import (3+ levels)",
                regex: Regex::new(
                    r#"(?:from\s+\.{3,}\S*\s+import|(?:from|import|require)\s*\(?['"](?:\.\./){3,}|require\s*\(\s*['"](?:\.\./){3,})"#
                ).unwrap(),
                severity: Severity::Info,
                reject_if_contains: None,
                target_extensions: &["py", "ts", "js", "tsx", "jsx"],
                target_filenames: &[],
                suggestion: "Use absolute imports or path aliases instead of deep relative imports",
                fix_kind: FixKind::Suggestion,
            },
            // Pattern 8: Git dependency
            DepPattern {
                name: "git dependency (non-reproducible, breaks offline installs)",
                regex: Regex::new(
                    r#"(?:["']git\+https?://|["']github:|git\s*=\s*["']https?://|git\+https?://)"#
                ).unwrap(),
                severity: Severity::Info,
                reject_if_contains: None,
                target_extensions: &["toml"],
                target_filenames: &["package.json", "requirements.txt"],
                suggestion: "Use a published package version instead of a git dependency",
                fix_kind: FixKind::Suggestion,
            },
        ]
    })
}

/// Code file extensions the dependency analyzer may scan
const CODE_EXTENSIONS: &[&str] = &["py", "ts", "js", "tsx", "jsx", "java"];

/// Manifest filenames the dependency analyzer may scan
const MANIFEST_FILENAMES: &[&str] = &[
    "package.json",
    "requirements.txt",
    "Cargo.toml",
    "pyproject.toml",
];

/// Binary file extensions to skip
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "woff", "woff2", "ttf", "eot", "otf",
    "zip", "gz", "tar", "bz2", "xz", "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx", "ppt",
    "pptx", "exe", "dll", "so", "dylib", "o", "a", "pyc", "pyo", "class", "lock", "mp3", "mp4",
    "avi", "mov", "wav", "flac", "sqlite", "db",
];

/// Analyzer that detects dependency hygiene issues
pub struct DependencyAnalyzer;

impl DependencyAnalyzer {
    /// Create a new dependency analyzer
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned (must match dependency file types, not binary)
    fn should_scan(path: &Path) -> bool {
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check exact filename match (e.g., "package.json")
        if MANIFEST_FILENAMES.contains(&file_name) {
            return true;
        }

        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return false,
        };

        if BINARY_EXTENSIONS.contains(&ext.as_str()) {
            return false;
        }

        CODE_EXTENSIONS.contains(&ext.as_str())
    }

    /// Check if a pattern applies to a given file
    fn pattern_matches_file(pattern: &DepPattern, path: &Path) -> bool {
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
        trimmed.starts_with('#')
            || trimmed.starts_with("//")
            || trimmed.starts_with('*')
            || trimmed.starts_with("/*")
    }

    /// Scan a single file for dependency patterns
    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        // Filter patterns applicable to this file
        let applicable: Vec<&DepPattern> = all_patterns
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
                    format!("Dependency issue: {}", pat.name),
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

impl Default for DependencyAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for DependencyAnalyzer {
    fn name(&self) -> &str {
        "Dependency Hygiene"
    }

    fn finding_prefix(&self) -> &str {
        "DEP"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.dependency
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
        &[".toml", ".txt", ".json"]
    }

    fn extra_filenames(&self) -> &[&str] {
        &[
            "package.json",
            "requirements.txt",
            "Cargo.toml",
            "pyproject.toml",
        ]
    }
}
