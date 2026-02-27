//! SSRF (Server-Side Request Forgery) analyzer
//!
//! Detects HTTP client calls where the URL argument is not a hardcoded string
//! literal — i.e., the URL is a variable, f-string, or template literal that
//! could be influenced by user input.
//!
//! Covers Python (requests, urllib, httpx), JavaScript/TypeScript (fetch, axios),
//! Go (net/http), and Java (java.net.URL).

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct SsrfPattern {
    name: &'static str,
    /// Matches lines that are potentially SSRF-vulnerable
    regex: Regex,
    /// If this regex also matches the line it is considered safe (e.g. literal URL)
    safe_pattern: Option<Regex>,
    severity: Severity,
    suggestion: &'static str,
    extensions: &'static [&'static str],
}

fn patterns() -> &'static [SsrfPattern] {
    static PATTERNS: OnceLock<Vec<SsrfPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Python: requests ─────────────────────────────────────────────
            SsrfPattern {
                name: "requests call with f-string URL (interpolated)",
                regex: Regex::new(r#"\brequests\.\w+\s*\(\s*f["']"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate and allowlist URLs before passing to requests; \
                             never interpolate user input directly into the URL",
                extensions: &["py"],
            },
            SsrfPattern {
                name: "requests call with variable URL",
                // Matches requests.get(some_var) or requests.get(obj.attr)
                // Safe pattern suppresses when the first arg is a plain string literal
                regex: Regex::new(
                    r"\brequests\.\w+\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*[,)\[]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Validate and allowlist the URL before making this request; \
                             ensure it cannot be controlled by external input",
                extensions: &["py"],
            },
            // ── Python: urllib ────────────────────────────────────────────────
            SsrfPattern {
                name: "urllib.urlopen with f-string URL",
                regex: Regex::new(
                    r#"\burllib(?:2)?\.(?:request\.)?urlopen\s*\(\s*f["']"#,
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate and allowlist URLs before opening; \
                             never interpolate user input into the URL",
                extensions: &["py"],
            },
            SsrfPattern {
                name: "urllib.urlopen with variable URL",
                regex: Regex::new(
                    r"\burllib(?:2)?\.(?:request\.)?urlopen\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*[,)\[]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Validate and allowlist the URL before opening",
                extensions: &["py"],
            },
            // ── Python: httpx ─────────────────────────────────────────────────
            SsrfPattern {
                name: "httpx call with f-string URL",
                regex: Regex::new(r#"\bhttpx\.\w+\s*\(\s*f["']"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate and allowlist URLs before making httpx requests",
                extensions: &["py"],
            },
            SsrfPattern {
                name: "httpx call with variable URL",
                regex: Regex::new(
                    r"\bhttpx\.\w+\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*[,)\[]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Validate and allowlist the URL before making this httpx request",
                extensions: &["py"],
            },
            // ── JavaScript / TypeScript: fetch ────────────────────────────────
            SsrfPattern {
                name: "fetch() with template literal URL (interpolated)",
                regex: Regex::new(r"\bfetch\s*\(\s*`[^`]*\$\{").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate and allowlist URLs before calling fetch(); \
                             never interpolate user input into the URL",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            SsrfPattern {
                name: "fetch() with variable URL",
                regex: Regex::new(
                    r"\bfetch\s*\(\s*[a-zA-Z_$][a-zA-Z0-9_.$]*\s*[,)\[]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Validate and allowlist the URL before calling fetch()",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            // ── JavaScript / TypeScript: axios ────────────────────────────────
            SsrfPattern {
                name: "axios call with template literal URL",
                regex: Regex::new(
                    r"\baxios\.(?:get|post|put|delete|patch|head|request)\s*\(\s*`[^`]*\$\{",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate and allowlist URLs before making axios requests",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            SsrfPattern {
                name: "axios call with variable URL",
                regex: Regex::new(
                    r"\baxios\.(?:get|post|put|delete|patch|head|request)\s*\(\s*[a-zA-Z_$][a-zA-Z0-9_.$]*\s*[,)\[]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Validate and allowlist the URL before making this axios request",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            // ── Go: net/http ──────────────────────────────────────────────────
            SsrfPattern {
                name: "http.Get/Post/Head with variable URL",
                regex: Regex::new(
                    r"\bhttp\.(?:Get|Post|Head)\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*[,)\[]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Validate and allowlist the URL before making this HTTP request; \
                             parse and inspect the URL with url.Parse() before use",
                extensions: &["go"],
            },
            SsrfPattern {
                name: "http.Get/Post with fmt.Sprintf URL (interpolated)",
                regex: Regex::new(
                    r#"\bhttp\.(?:Get|Post|Head)\s*\(\s*fmt\.Sprintf\s*\("#,
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate and allowlist the URL; never construct URLs via \
                             fmt.Sprintf with user-controlled values",
                extensions: &["go"],
            },
            // ── Java: java.net.URL ────────────────────────────────────────────
            SsrfPattern {
                name: "new URL() with variable (potential SSRF)",
                regex: Regex::new(
                    r"\bnew\s+URL\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*[,)\[]",
                )
                .unwrap(),
                // Suppress if it's clearly a local file URL or hardcoded
                safe_pattern: Some(Regex::new(r#"new\s+URL\s*\(\s*"(?:file|classpath):"#).unwrap()),
                severity: Severity::Warning,
                suggestion: "Validate and allowlist the URL before opening a connection; \
                             use a strict allowlist of permitted hosts",
                extensions: &["java"],
            },
            SsrfPattern {
                name: "new URL() with string concatenation",
                regex: Regex::new(r#"\bnew\s+URL\s*\(\s*"[^"]*"\s*\+"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Never build URLs by concatenating user input; \
                             use a strict allowlist of permitted hosts",
                extensions: &["java"],
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

/// Analyzer that detects potential SSRF vulnerabilities
pub struct SsrfAnalyzer;

impl SsrfAnalyzer {
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
                if let Some(safe) = &pat.safe_pattern {
                    if safe.is_match(line) {
                        break;
                    }
                }
                findings.push(make_finding(
                    pat.severity,
                    format!("Possible SSRF: {}", pat.name),
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

impl Default for SsrfAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for SsrfAnalyzer {
    fn name(&self) -> &str {
        "SSRF"
    }

    fn finding_prefix(&self) -> &str {
        "SSRF"
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
