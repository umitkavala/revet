//! SQL injection analyzer — detects SQL queries built with string interpolation/concatenation
//!
//! Scans raw file content line-by-line for patterns where SQL keywords co-occur with
//! string interpolation or concatenation, indicating potential SQL injection vulnerabilities.
//! Only one finding per line (first matching pattern wins) to reduce noise.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled SQL injection detection pattern
struct SqlPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
}

/// Returns all SQL injection patterns in priority order (Error patterns first)
fn patterns() -> &'static [SqlPattern] {
    static PATTERNS: OnceLock<Vec<SqlPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        // Shared SQL keyword group
        let kw = r"(?:SELECT|INSERT|UPDATE|DELETE|DROP|ALTER|CREATE|REPLACE|MERGE|TRUNCATE|EXEC)\b";
        // Common DB execution method names
        let exec = r"(?:execute|executemany|executescript|raw|rawquery|query|prepare)";

        vec![
            // ── Error: interpolation inside DB execution calls ──────────
            // ORM-specific patterns first (more specific than generic exec patterns)

            // Pattern 1: ORM raw with interpolation — .objects.raw(f"...") / .text(f"...")
            SqlPattern {
                name: "ORM raw query with interpolation",
                regex: Regex::new(&format!(r#"\.(?:objects\.raw|text)\s*\(\s*f["'].*{kw}"#))
                    .unwrap(),
                severity: Severity::Error,
            },
            // Pattern 2: f-string SQL in DB call — .execute(f"...SQL...")
            SqlPattern {
                name: "f-string SQL in database call",
                regex: Regex::new(&format!(r#"\.{exec}\s*\(\s*f["'].*{kw}"#)).unwrap(),
                severity: Severity::Error,
            },
            // Pattern 3: String concat SQL in DB call — .execute("...SQL..." + var)
            SqlPattern {
                name: "string concatenation SQL in database call",
                regex: Regex::new(&format!(r#"\.{exec}\s*\(\s*["'].*{kw}.*["']\s*\+"#)).unwrap(),
                severity: Severity::Error,
            },
            // Pattern 4: .format() SQL in DB call — .execute("...SQL...".format())
            SqlPattern {
                name: ".format() SQL in database call",
                regex: Regex::new(&format!(
                    r#"\.{exec}\s*\(\s*["'].*{kw}.*["']\s*\.format\s*\("#
                ))
                .unwrap(),
                severity: Severity::Error,
            },
            // Pattern 5: % format SQL in DB call — .execute("...SQL..." % var)
            // Note: parameterized queries like execute("...%s", (var,)) won't match
            // because the comma after the closing quote prevents the " % pattern
            SqlPattern {
                name: "%-format SQL in database call",
                regex: Regex::new(&format!(r#"\.{exec}\s*\(\s*["'].*{kw}.*["']\s*%\s*\w"#))
                    .unwrap(),
                severity: Severity::Error,
            },
            // Pattern 6: Template literal SQL in DB call — .query(`...SQL...${var}`)
            SqlPattern {
                name: "template literal SQL in database call",
                regex: Regex::new(&format!(r#"\.{exec}\s*\(\s*`[^`]*{kw}[^`]*\$\{{[^`]*`"#))
                    .unwrap(),
                severity: Severity::Error,
            },
            // ── Warning: standalone SQL strings with interpolation ──────

            // Pattern 7: f-string SQL assignment — var = f"...SQL...{}"
            SqlPattern {
                name: "f-string SQL assignment",
                regex: Regex::new(&format!(r#"=\s*f["'].*{kw}.*\{{"#)).unwrap(),
                severity: Severity::Warning,
            },
            // Pattern 8: String concat SQL — "...SQL..." + var
            SqlPattern {
                name: "string concatenation SQL",
                regex: Regex::new(&format!(r#"["'].*{kw}.*["']\s*\+\s*\w"#)).unwrap(),
                severity: Severity::Warning,
            },
            // Pattern 9: .format() SQL string — "...SQL...{}".format()
            SqlPattern {
                name: ".format() SQL string",
                regex: Regex::new(&format!(r#"["'].*{kw}.*["']\s*\.format\s*\("#)).unwrap(),
                severity: Severity::Warning,
            },
            // Pattern 10: % format SQL string — "...SQL...%s" % var
            SqlPattern {
                name: "%-format SQL string",
                regex: Regex::new(&format!(r#"["'].*{kw}.*["']\s*%\s*\w"#)).unwrap(),
                severity: Severity::Warning,
            },
            // Pattern 11: Template literal SQL — var = `...SQL...${}`
            SqlPattern {
                name: "template literal SQL",
                regex: Regex::new(&format!(r#"`[^`]*{kw}[^`]*\$\{{[^`]*`"#)).unwrap(),
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

/// Analyzer that detects SQL injection via string interpolation/concatenation
pub struct SqlInjectionAnalyzer;

impl SqlInjectionAnalyzer {
    /// Create a new SQL injection analyzer
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned based on its extension
    fn should_scan(path: &Path) -> bool {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return true,
        };

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.ends_with(".min.js") || file_name.ends_with(".min.css") {
            return false;
        }

        !BINARY_EXTENSIONS.contains(&ext.as_str())
    }

    /// Check if a line is a comment (should be skipped)
    fn is_comment_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with('#')
            || trimmed.starts_with("//")
            || trimmed.starts_with('*')
            || trimmed.starts_with("--")
    }

    /// Scan a single file for SQL injection patterns
    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            // Skip comment lines
            if Self::is_comment_line(line) {
                continue;
            }

            // First matching pattern wins for this line
            for pat in all_patterns {
                if pat.regex.is_match(line) {
                    findings.push(make_finding(
                        pat.severity,
                        format!("Possible SQL injection: {}", pat.name),
                        path.to_path_buf(),
                        line_num + 1,
                    ));
                    break;
                }
            }
        }

        findings
    }
}

impl Default for SqlInjectionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for SqlInjectionAnalyzer {
    fn name(&self) -> &str {
        "SQL Injection"
    }

    fn finding_prefix(&self) -> &str {
        "SQL"
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
