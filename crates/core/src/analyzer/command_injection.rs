//! Command injection analyzer — detects user-input flowing into shell/exec calls
//!
//! Scans raw file content line-by-line for patterns that indicate code paths
//! where shell commands may be constructed from untrusted input.
//! Covers Python, JavaScript/TypeScript, Go, Ruby, and shell scripts.

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled command-injection detection pattern
struct CmdPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    suggestion: &'static str,
    /// File extensions this pattern applies to (empty = all)
    extensions: &'static [&'static str],
}

fn patterns() -> &'static [CmdPattern] {
    static PATTERNS: OnceLock<Vec<CmdPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Python ────────────────────────────────────────────────────────
            CmdPattern {
                name: "subprocess call with shell=True",
                regex: Regex::new(r"subprocess\.\w+\s*\([^)]*shell\s*=\s*True").unwrap(),
                severity: Severity::Error,
                suggestion: "Pass a list of arguments to subprocess instead of shell=True; \
                             never interpolate user input into shell commands",
                extensions: &["py"],
            },
            CmdPattern {
                name: "os.system() call",
                regex: Regex::new(r"\bos\.system\s*\(").unwrap(),
                severity: Severity::Error,
                suggestion: "Replace os.system() with subprocess.run() using an argument list",
                extensions: &["py"],
            },
            CmdPattern {
                name: "os.popen() call",
                regex: Regex::new(r"\bos\.popen\s*\(").unwrap(),
                severity: Severity::Error,
                suggestion: "Replace os.popen() with subprocess.run() using an argument list",
                extensions: &["py"],
            },
            CmdPattern {
                name: "commands.getoutput() call (deprecated shell exec)",
                regex: Regex::new(r"\bcommands\.get(?:output|status(?:output)?)\s*\(").unwrap(),
                severity: Severity::Error,
                suggestion:
                    "Replace commands.getoutput() with subprocess.run() using an argument list",
                extensions: &["py"],
            },
            // ── JavaScript / TypeScript ───────────────────────────────────────
            CmdPattern {
                name: "child_process exec with template literal",
                regex: Regex::new(r"\bexec(?:Sync)?\s*\(\s*`[^`]*\$\{").unwrap(),
                severity: Severity::Error,
                suggestion: "Use execFile() or spawn() with an argument array instead of \
                             interpolating variables into a shell command string",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            CmdPattern {
                name: "child_process exec with string concatenation",
                regex: Regex::new(r#"\bexec(?:Sync)?\s*\(\s*["'][^"']*["']\s*\+"#).unwrap(),
                severity: Severity::Error,
                suggestion: "Use execFile() or spawn() with an argument array instead of \
                             concatenating user input into a shell command",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            CmdPattern {
                name: "spawn() with shell: true",
                regex: Regex::new(r"\bspawn\s*\([^)]*shell\s*:\s*true").unwrap(),
                severity: Severity::Error,
                suggestion: "Remove shell: true from spawn() and pass arguments as an array",
                extensions: &["js", "ts", "jsx", "tsx", "mjs", "cjs"],
            },
            // ── Go ───────────────────────────────────────────────────────────
            CmdPattern {
                name: "exec.Command with explicit shell invocation",
                regex: Regex::new(r#"exec\.Command\s*\(\s*"(?:sh|bash|zsh|cmd|powershell)"\s*,"#)
                    .unwrap(),
                severity: Severity::Error,
                suggestion: "Invoke the target binary directly with exec.Command instead of \
                             routing through a shell; validate all arguments",
                extensions: &["go"],
            },
            // ── Ruby ─────────────────────────────────────────────────────────
            CmdPattern {
                name: "backtick shell execution with string interpolation",
                regex: Regex::new(r"`[^`]*#\{").unwrap(),
                severity: Severity::Error,
                suggestion: "Use Open3.capture2e or IO.popen with an argument array instead of \
                             interpolating variables into a backtick shell command",
                extensions: &["rb", "rake", "gemspec"],
            },
            CmdPattern {
                name: "%x{} shell execution with string interpolation",
                regex: Regex::new(r"%x\{[^}]*#\{").unwrap(),
                severity: Severity::Error,
                suggestion: "Use Open3.capture2e or IO.popen with an argument array instead of \
                             interpolating variables into a shell command",
                extensions: &["rb", "rake", "gemspec"],
            },
            CmdPattern {
                name: "Kernel#system() with interpolated string",
                regex: Regex::new(r#"\bsystem\s*\(\s*"[^"]*#\{"#).unwrap(),
                severity: Severity::Error,
                suggestion: "Pass arguments as separate strings to system() rather than \
                             interpolating into a single shell string",
                extensions: &["rb", "rake", "gemspec"],
            },
            // ── Shell scripts ─────────────────────────────────────────────────
            CmdPattern {
                name: "eval with variable expansion",
                regex: Regex::new(r"\beval\s+\$\w+").unwrap(),
                severity: Severity::Error,
                suggestion: "Avoid eval with variables; restructure logic to avoid dynamic \
                             command construction from untrusted input",
                extensions: &["sh", "bash", "zsh"],
            },
        ]
    })
}

/// Extensions this analyzer handles beyond what the parsers cover
const EXTRA_EXTENSIONS: &[&str] = &[".sh", ".bash", ".zsh", ".mjs", ".cjs"];

/// Binary-like extensions to skip
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "woff", "woff2", "ttf", "eot", "otf",
    "zip", "gz", "tar", "bz2", "xz", "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx", "ppt",
    "pptx", "exe", "dll", "so", "dylib", "o", "a", "pyc", "pyo", "class", "lock", "mp3", "mp4",
    "avi", "mov", "wav", "flac", "sqlite", "db",
];

/// Analyzer that detects command injection patterns across multiple languages
pub struct CommandInjectionAnalyzer;

impl CommandInjectionAnalyzer {
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
                // Skip patterns that don't apply to this file's extension
                if !pat.extensions.is_empty() && !pat.extensions.contains(&ext.as_str()) {
                    continue;
                }
                if pat.regex.is_match(line) {
                    findings.push(make_finding(
                        pat.severity,
                        format!("Possible command injection: {}", pat.name),
                        path.to_path_buf(),
                        line_num + 1,
                        Some(pat.suggestion.to_string()),
                        Some(FixKind::Suggestion),
                    ));
                    break; // One finding per line
                }
            }
        }

        findings
    }
}

impl Default for CommandInjectionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for CommandInjectionAnalyzer {
    fn name(&self) -> &str {
        "Command Injection"
    }

    fn finding_prefix(&self) -> &str {
        "CMD"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.security
    }

    fn extra_extensions(&self) -> &[&str] {
        EXTRA_EXTENSIONS
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
