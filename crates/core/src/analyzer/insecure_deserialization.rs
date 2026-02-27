//! Insecure deserialization analyzer — detects unsafe deserialization of untrusted data
//!
//! Deserializing data from untrusted sources with unsafe formats can lead to
//! Remote Code Execution (RCE). This analyzer detects the most common patterns
//! across Python, PHP, Java, and Ruby.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

struct DeserPattern {
    name: &'static str,
    regex: Regex,
    /// If this regex also matches the same line, the finding is suppressed (safe usage)
    safe_pattern: Option<Regex>,
    severity: Severity,
    suggestion: &'static str,
    /// File extensions this pattern applies to (empty = all text files)
    extensions: &'static [&'static str],
}

fn patterns() -> &'static [DeserPattern] {
    static PATTERNS: OnceLock<Vec<DeserPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Python ────────────────────────────────────────────────────────
            DeserPattern {
                name: "yaml.load() without safe Loader",
                // Matches yaml.load( — safe_pattern suppresses if SafeLoader/BaseLoader is present
                regex: Regex::new(r"yaml\.load\s*\(").unwrap(),
                safe_pattern: Some(
                    Regex::new(r"(?:Safe|Base)Loader").unwrap(),
                ),
                severity: Severity::Error,
                suggestion: "Use yaml.safe_load() or pass Loader=yaml.SafeLoader to yaml.load()",
                extensions: &["py"],
            },
            DeserPattern {
                name: "pickle.load() — arbitrary code execution on untrusted data",
                regex: Regex::new(r"\bpickle\.loads?\s*\(").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion:
                    "Never deserialize pickle data from untrusted sources; use JSON or msgpack instead",
                extensions: &["py"],
            },
            DeserPattern {
                name: "cPickle.load() — arbitrary code execution on untrusted data",
                regex: Regex::new(r"\bcPickle\.loads?\s*\(").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion:
                    "Never deserialize cPickle data from untrusted sources; use JSON or msgpack instead",
                extensions: &["py"],
            },
            DeserPattern {
                name: "marshal.loads() — arbitrary code execution on untrusted data",
                regex: Regex::new(r"\bmarshal\.loads?\s*\(").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion:
                    "Never deserialize marshal data from untrusted sources; use JSON or msgpack instead",
                extensions: &["py"],
            },
            DeserPattern {
                name: "jsonpickle.decode() — arbitrary code execution on untrusted data",
                regex: Regex::new(r"\bjsonpickle\.decode\s*\(").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion:
                    "Never call jsonpickle.decode() on untrusted input; use json.loads() instead",
                extensions: &["py"],
            },
            // ── PHP ───────────────────────────────────────────────────────────
            DeserPattern {
                name: "PHP unserialize() — object injection on untrusted data",
                regex: Regex::new(r"\bunserialize\s*\(").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion:
                    "Use json_decode() instead of unserialize(); if you must unserialize, \
                     validate the data with a whitelist via the allowed_classes option",
                extensions: &["php"],
            },
            // ── Java ──────────────────────────────────────────────────────────
            DeserPattern {
                name: "Java ObjectInputStream — deserialization gadget chain risk",
                regex: Regex::new(r"\bnew\s+ObjectInputStream\s*\(").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion:
                    "Avoid Java native deserialization with untrusted data; use JSON (Jackson) \
                     or a serialization filter (ObjectInputFilter) to allowlist safe types",
                extensions: &["java"],
            },
            // ── Ruby ──────────────────────────────────────────────────────────
            DeserPattern {
                name: "Marshal.load() — arbitrary code execution on untrusted data",
                regex: Regex::new(r"\bMarshal\.load\s*\(").unwrap(),
                safe_pattern: None,
                severity: Severity::Error,
                suggestion:
                    "Never call Marshal.load() on untrusted input; use JSON.parse() instead",
                extensions: &["rb", "rake", "gemspec"],
            },
            DeserPattern {
                name: "YAML.load() — may deserialize arbitrary Ruby objects",
                // safe_pattern suppresses if YAML.safe_load is present on the same line
                regex: Regex::new(r"\bYAML\.load\s*\(").unwrap(),
                safe_pattern: Some(Regex::new(r"YAML\.safe_load").unwrap()),
                severity: Severity::Warning,
                suggestion:
                    "Use YAML.safe_load() instead of YAML.load() to prevent arbitrary object \
                     deserialization; YAML.load() can instantiate arbitrary Ruby classes",
                extensions: &["rb", "rake", "gemspec"],
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

/// Analyzer that detects insecure deserialization of untrusted data
pub struct InsecureDeserializationAnalyzer;

impl InsecureDeserializationAnalyzer {
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
                // Suppress if a safe usage pattern is also present on this line
                if let Some(safe) = &pat.safe_pattern {
                    if safe.is_match(line) {
                        break;
                    }
                }
                findings.push(make_finding(
                    pat.severity,
                    format!("Insecure deserialization: {}", pat.name),
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

impl Default for InsecureDeserializationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for InsecureDeserializationAnalyzer {
    fn name(&self) -> &str {
        "Insecure Deserialization"
    }

    fn finding_prefix(&self) -> &str {
        "DESER"
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
