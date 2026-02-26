//! Inline suppression comments — `revet-ignore PREFIX` silences findings at source

use crate::Finding;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::sync::LazyLock;

static SUPPRESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"revet-ignore\s+([\w\-\*]+(?:\s+[\w\-\*]+)*)").unwrap());

/// Parse inline suppression comments from file content.
///
/// Returns a map of `line_number → vec_of_prefixes` (1-indexed).
/// Recognises any comment style (`#`, `//`, `--`, `/*`) — we simply search for
/// the `revet-ignore` token anywhere on the line.
pub fn parse_suppressions(content: &str) -> HashMap<usize, Vec<String>> {
    let mut map = HashMap::new();
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1; // 1-indexed
        if let Some(caps) = SUPPRESS_RE.captures(line) {
            let prefixes: Vec<String> = caps[1].split_whitespace().map(String::from).collect();
            map.insert(line_no, prefixes);
        }
    }
    map
}

/// Check whether a finding ID matches any of the given suppression prefixes.
///
/// - `*` matches everything
/// - `SEC` matches `SEC-001`, `SEC-002`, etc.
pub fn matches_suppression(finding_id: &str, prefixes: &[String]) -> bool {
    let finding_prefix = finding_id.split('-').next().unwrap_or(finding_id);
    prefixes.iter().any(|p| p == "*" || p == finding_prefix)
}

/// A finding that was suppressed, paired with the reason for suppression.
#[derive(Debug, Clone)]
pub struct SuppressedFinding {
    pub finding: Finding,
    /// Human-readable suppression source: `"inline"`, `"per-path rule"`, `"baseline"`.
    pub reason: String,
}

/// Filter findings by inline `revet-ignore` comments in source files.
///
/// For each finding at line N, checks for suppression comments at line N
/// (same-line) and line N-1 (line-before).
///
/// Returns `(kept_findings, suppressed)`.
pub fn filter_findings_by_inline(findings: Vec<Finding>) -> (Vec<Finding>, Vec<SuppressedFinding>) {
    // Group findings by file to read each file only once
    let mut by_file: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, f) in findings.iter().enumerate() {
        let key = f.file.to_string_lossy().into_owned();
        by_file.entry(key).or_default().push(i);
    }

    // Parse suppressions for each unique file
    let mut file_suppressions: HashMap<String, HashMap<usize, Vec<String>>> = HashMap::new();
    for file_path in by_file.keys() {
        if let Ok(content) = fs::read_to_string(file_path) {
            let sups = parse_suppressions(&content);
            if !sups.is_empty() {
                file_suppressions.insert(file_path.clone(), sups);
            }
        }
    }

    let mut kept = Vec::new();
    let mut suppressed: Vec<SuppressedFinding> = Vec::new();

    for finding in findings {
        let key = finding.file.to_string_lossy().into_owned();
        let is_suppressed = if let Some(sups) = file_suppressions.get(&key) {
            let line = finding.line;
            // Check same-line
            let same_line = sups
                .get(&line)
                .map(|p| matches_suppression(&finding.id, p))
                .unwrap_or(false);
            // Check line-before (only if line > 1)
            let line_before = if line > 1 {
                sups.get(&(line - 1))
                    .map(|p| matches_suppression(&finding.id, p))
                    .unwrap_or(false)
            } else {
                false
            };
            same_line || line_before
        } else {
            false
        };

        if is_suppressed {
            suppressed.push(SuppressedFinding {
                finding,
                reason: "inline".to_string(),
            });
        } else {
            kept.push(finding);
        }
    }

    (kept, suppressed)
}

/// Filter findings using per-path suppression rules from `.revet.toml`.
///
/// `per_path` maps glob patterns (e.g. `"**/tests/**"`) to lists of finding
/// ID prefixes (e.g. `["SEC", "SQL"]` or `["*"]` for all).
///
/// Returns `(kept_findings, suppressed)`.
pub fn filter_findings_by_path_rules(
    findings: Vec<Finding>,
    per_path: &std::collections::HashMap<String, Vec<String>>,
    repo_root: &std::path::Path,
) -> (Vec<Finding>, Vec<SuppressedFinding>) {
    if per_path.is_empty() {
        return (findings, vec![]);
    }

    // Pre-compile glob patterns (keep original pattern string for the reason)
    let rules: Vec<(glob::Pattern, &str, &Vec<String>)> = per_path
        .iter()
        .filter_map(|(pattern, prefixes)| {
            glob::Pattern::new(pattern)
                .ok()
                .map(|p| (p, pattern.as_str(), prefixes))
        })
        .collect();

    let mut kept = Vec::new();
    let mut suppressed: Vec<SuppressedFinding> = Vec::new();

    for finding in findings {
        // Match against the path relative to repo root for consistent glob behaviour
        let rel_path = finding
            .file
            .strip_prefix(repo_root)
            .unwrap_or(&finding.file);
        let path_str = rel_path.to_string_lossy();

        let matched = rules.iter().find(|(pattern, _, prefixes)| {
            pattern.matches(&path_str) && matches_suppression(&finding.id, prefixes)
        });

        if let Some((_, pattern_str, _)) = matched {
            suppressed.push(SuppressedFinding {
                finding,
                reason: format!("per-path rule: {}", pattern_str),
            });
        } else {
            kept.push(finding);
        }
    }

    (kept, suppressed)
}
