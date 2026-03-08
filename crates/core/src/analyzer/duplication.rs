//! Duplication analyzer — detects copy-paste code blocks across files.
//!
//! Algorithm:
//! 1. For each file, extract all normalized N-line windows (sliding window).
//! 2. Normalize lines: strip comments, collapse whitespace, replace literals.
//! 3. Hash each window. Group identical hashes across different locations.
//! 4. Emit one finding per duplicate group pointing to the first occurrence's
//!    location, listing all duplicate sites.
//!
//! Enabled via `[modules] duplication = true` in `.revet.toml`.
//! Threshold: `duplication_min_lines` (default: 6).

use crate::analyzer::Analyzer;
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub struct DuplicationAnalyzer {
    min_lines: usize,
}

impl DuplicationAnalyzer {
    pub fn new() -> Self {
        Self { min_lines: 6 }
    }

    pub fn with_min_lines(min_lines: usize) -> Self {
        Self {
            min_lines: min_lines.max(3),
        }
    }
}

impl Default for DuplicationAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for DuplicationAnalyzer {
    fn name(&self) -> &str {
        "Duplication"
    }

    fn finding_prefix(&self) -> &str {
        "DUP"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.duplication
    }

    fn analyze_files(&self, files: &[PathBuf], repo_root: &Path) -> Vec<Finding> {
        self.detect(files, repo_root, self.min_lines)
    }
}

impl DuplicationAnalyzer {
    fn detect(&self, files: &[PathBuf], repo_root: &Path, min_lines: usize) -> Vec<Finding> {
        // Map: hash → list of (file, start_line, preview)
        let mut buckets: HashMap<u64, Vec<(PathBuf, usize, String)>> = HashMap::new();

        for file in files {
            let content = match std::fs::read_to_string(file) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let normalized: Vec<(usize, String)> = content
                .lines()
                .enumerate()
                .filter_map(|(i, line)| {
                    let norm = normalize_line(line);
                    if norm.is_empty() {
                        None
                    } else {
                        Some((i + 1, norm)) // 1-indexed line numbers
                    }
                })
                .collect();

            if normalized.len() < min_lines {
                continue;
            }

            // Sliding window over non-blank lines
            for window_start in 0..=(normalized.len() - min_lines) {
                let window = &normalized[window_start..window_start + min_lines];
                let hash = hash_window(window);
                let (line_num, preview) = &window[0];
                buckets
                    .entry(hash)
                    .or_default()
                    .push((file.clone(), *line_num, preview.clone()));
            }
        }

        // Collect findings: only buckets with occurrences in 2+ distinct locations
        let mut findings = Vec::new();

        for (_hash, mut locations) in buckets {
            // Deduplicate: keep only one entry per (file, line) pair
            locations.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            locations.dedup_by(|a, b| a.0 == b.0 && a.1.abs_diff(b.1) < min_lines);

            // Must span at least 2 distinct files or 2 non-overlapping locations
            let distinct_files: std::collections::HashSet<&PathBuf> =
                locations.iter().map(|(f, _, _)| f).collect();
            if locations.len() < 2 || (distinct_files.len() == 1 && locations.len() < 2) {
                continue;
            }

            // Report at the first (alphabetically earliest) location
            let (primary_file, primary_line, preview) = &locations[0];

            let others: Vec<String> = locations[1..]
                .iter()
                .map(|(f, line, _)| {
                    let rel = f.strip_prefix(repo_root).unwrap_or(f);
                    format!("{}:{}", rel.display(), line)
                })
                .collect();

            let primary_rel = primary_file.strip_prefix(repo_root).unwrap_or(primary_file);

            findings.push(Finding {
                id: String::new(), // renumbered by dispatcher
                severity: Severity::Info,
                message: format!(
                    "Duplicate block ({} line{}) also found at: {}",
                    min_lines,
                    if min_lines == 1 { "" } else { "s" },
                    others.join(", ")
                ),
                file: primary_file.clone(),
                line: *primary_line,
                suggestion: Some(format!(
                    "Extract `{}…` into a shared function or module",
                    truncate(preview, 40)
                )),
                affected_dependents: others.len(),
                ..Default::default()
            });

            // Suppress noisy output: if primary_rel appears in error already skip it
            let _ = primary_rel; // used above via strip_prefix
        }

        findings
    }
}

// ── Normalization ─────────────────────────────────────────────────────────────

/// Normalize a source line for duplicate detection:
/// - Trim whitespace
/// - Strip single-line comments (// and #)
/// - Replace string literals with __STR__
/// - Replace numeric literals with __NUM__
/// - Collapse internal whitespace
/// - Skip structural-only lines ({ } ( ))
fn normalize_line(line: &str) -> String {
    let trimmed = line.trim();

    // Skip blank or comment-only lines
    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
        return String::new();
    }

    // Skip lines that are purely structural punctuation
    if matches!(trimmed, "{" | "}" | "(" | ")" | "{}" | "[]" | "," | ";") {
        return String::new();
    }

    // Strip trailing single-line comments
    let without_comment = strip_comment(trimmed);

    // Replace string literals (single and double quoted)
    let s = replace_strings(without_comment);

    // Replace numeric literals
    let s = replace_numbers(&s);

    // Collapse whitespace
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_comment(s: &str) -> &str {
    // Naive: find // outside strings
    let bytes = s.as_bytes();
    let mut in_str: Option<u8> = None;
    for i in 0..bytes.len() {
        match (in_str, bytes[i]) {
            (None, b'"' | b'\'') => in_str = Some(bytes[i]),
            (Some(q), c) if c == q && (i == 0 || bytes[i - 1] != b'\\') => in_str = None,
            (None, b'/') if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                return s[..i].trim_end();
            }
            _ => {}
        }
    }
    s
}

fn replace_strings(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' || c == '\'' {
            let q = c;
            out.push_str("__STR__");
            let mut escaped = false;
            for inner in chars.by_ref() {
                if escaped {
                    escaped = false;
                } else if inner == '\\' {
                    escaped = true;
                } else if inner == q {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn replace_numbers(s: &str) -> String {
    // Replace sequences of digits (possibly with . in between) with __NUM__
    let mut out = String::with_capacity(s.len());
    let mut prev_alnum = false;
    let mut i = 0;
    let bytes = s.as_bytes();
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() && !prev_alnum {
            out.push_str("__NUM__");
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            prev_alnum = false;
        } else {
            let c = bytes[i] as char;
            prev_alnum = c.is_alphanumeric() || c == '_';
            out.push(c);
            i += 1;
        }
    }
    out
}

fn hash_window(window: &[(usize, String)]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (_, line) in window {
        line.hash(&mut hasher);
    }
    hasher.finish()
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
