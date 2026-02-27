//! Path traversal analyzer — detects unsanitized user input in file system operations
//!
//! Path traversal (CWE-22) allows attackers to access files outside the intended
//! directory by injecting `../` sequences or absolute paths. This analyzer flags
//! file system calls where the path argument is interpolated or derived from a
//! variable that could be user-controlled.
//!
//! Covers Python, JavaScript/TypeScript, PHP, Go, and Java.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct PathPattern {
    name: &'static str,
    regex: Regex,
    /// If this regex also matches, the line is considered safe
    safe_pattern: Option<Regex>,
    severity: Severity,
    suggestion: &'static str,
    extensions: &'static [&'static str],
}

fn patterns() -> &'static [PathPattern] {
    static PATTERNS: OnceLock<Vec<PathPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Python ────────────────────────────────────────────────────────
            PathPattern {
                name: "open() with f-string path (interpolated)",
                regex: Regex::new(r#"\bopen\s*\(\s*f["']"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate and sanitize the path; use os.path.realpath() and verify \
                             the result starts with the expected base directory",
                extensions: &["py"],
            },
            PathPattern {
                name: "open() with '../' traversal sequence",
                regex: Regex::new(r#"\bopen\s*\([^)]*(?:['"]|\+\s*['"])\s*\.\./"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Never allow '../' in file paths from user input; \
                             use os.path.realpath() and allowlist the expected base directory",
                extensions: &["py"],
            },
            PathPattern {
                name: "os.path.join() with variable first argument",
                // Matches os.path.join(var, ...) where first arg is not a string literal
                regex: Regex::new(
                    r"\bos\.path\.join\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*,",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "After joining paths, call os.path.realpath() and verify the result \
                             starts with the intended base directory to prevent traversal",
                extensions: &["py"],
            },
            PathPattern {
                name: "pathlib.Path() with f-string argument",
                regex: Regex::new(r#"\bPath\s*\(\s*f["']"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Resolve and validate the path: use Path(base).resolve() and check \
                             that the result is relative to the expected root",
                extensions: &["py"],
            },
            // ── JavaScript / TypeScript ───────────────────────────────────────
            PathPattern {
                name: "fs.readFile/writeFile/appendFile with template literal path",
                regex: Regex::new(
                    r"\bfs\.(?:read|write|append)(?:File|FileSync)\s*\(\s*`[^`]*\$\{",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate the path with path.resolve() and verify it starts with \
                             the expected base directory before passing to fs methods",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            PathPattern {
                name: "fs.readFile/writeFile with variable path",
                regex: Regex::new(
                    r"\bfs\.(?:read|write|append)(?:File|FileSync)\s*\(\s*[a-zA-Z_$][a-zA-Z0-9_.$]*\s*[,)]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Validate the path with path.resolve() and verify it stays within \
                             the expected directory before reading or writing",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            PathPattern {
                name: "path.join() with template literal segment",
                regex: Regex::new(r"\bpath\.join\s*\([^)]*`[^`]*\$\{").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Validate that path.resolve(path.join(...)) stays within the \
                             expected base directory",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            PathPattern {
                name: "path.join() with '../' sequence",
                regex: Regex::new(r#"\bpath\.join\s*\([^)]*['"]\.\.[/\\]['"]"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Do not allow '../' in path segments from user input; \
                             use path.resolve() and verify the result is within the base dir",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            // ── PHP ───────────────────────────────────────────────────────────
            PathPattern {
                name: "include/require with variable path (LFI risk)",
                regex: Regex::new(r"\b(?:include|require)(?:_once)?\s*\(\s*\$").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Never pass user-controlled values to include/require; \
                             use an allowlist of permitted file names",
                extensions: &["php"],
            },
            PathPattern {
                name: "file_get_contents() with superglobal input",
                // Flags direct use of $_GET, $_POST, $_REQUEST, $_COOKIE in file ops
                regex: Regex::new(
                    r"\bfile_get_contents\s*\(\s*\$_(?:GET|POST|REQUEST|COOKIE|SERVER)",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Never pass raw user input to file_get_contents(); \
                             validate and sanitize the path, then use realpath() to prevent traversal",
                extensions: &["php"],
            },
            PathPattern {
                name: "file_get_contents() with variable path",
                // Matches any $variable — safe_pattern suppresses when it's a superglobal
                // (those are already caught by the higher-severity pattern above)
                regex: Regex::new(r"\bfile_get_contents\s*\(\s*\$[a-zA-Z_]\w*").unwrap(),
                safe_pattern: Some(
                    Regex::new(r"\$_(?:GET|POST|REQUEST|COOKIE|SERVER)").unwrap(),
                ),
                severity: Severity::Warning,
                suggestion: "Validate and sanitize the path before passing to file_get_contents(); \
                             use realpath() and verify the result is within the expected directory",
                extensions: &["php"],
            },
            // ── Go ────────────────────────────────────────────────────────────
            PathPattern {
                name: "os.Open/ReadFile with fmt.Sprintf path (interpolated)",
                regex: Regex::new(
                    r"\b(?:os\.Open|os\.ReadFile|ioutil\.ReadFile)\s*\(\s*fmt\.Sprintf\s*\(",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Use filepath.Clean() and verify the cleaned path starts with the \
                             expected base directory; never build paths with fmt.Sprintf from user input",
                extensions: &["go"],
            },
            PathPattern {
                name: "os.Open/ReadFile with variable path",
                regex: Regex::new(
                    r"\b(?:os\.Open|os\.ReadFile|ioutil\.ReadFile)\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*[,)]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Use filepath.Clean() and verify the path stays within the expected \
                             base directory before opening",
                extensions: &["go"],
            },
            // ── Java ──────────────────────────────────────────────────────────
            PathPattern {
                name: "new File() with string concatenation",
                regex: Regex::new(r#"\bnew\s+File\s*\(\s*"[^"]*"\s*\+"#).unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion: "Use File.getCanonicalPath() and verify the result starts with the \
                             expected base path; never build file paths by string concatenation",
                extensions: &["java"],
            },
            PathPattern {
                name: "Paths.get() with variable argument",
                regex: Regex::new(
                    r"\bPaths\.get\s*\(\s*[a-zA-Z_][a-zA-Z0-9_.]*\s*[,)]",
                )
                .unwrap(),
                safe_pattern: None,
                severity: Severity::Warning,
                suggestion: "Call .toRealPath() and verify the result starts with the expected \
                             base directory to prevent path traversal",
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

/// Analyzer that detects path traversal vulnerabilities
pub struct PathTraversalAnalyzer;

impl PathTraversalAnalyzer {
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
                    format!("Possible path traversal: {}", pat.name),
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

impl Default for PathTraversalAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for PathTraversalAnalyzer {
    fn name(&self) -> &str {
        "Path Traversal"
    }

    fn finding_prefix(&self) -> &str {
        "PATH"
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
