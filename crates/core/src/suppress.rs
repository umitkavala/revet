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
fn matches_suppression(finding_id: &str, prefixes: &[String]) -> bool {
    let finding_prefix = finding_id.split('-').next().unwrap_or(finding_id);
    prefixes.iter().any(|p| p == "*" || p == finding_prefix)
}

/// Filter findings by inline `revet-ignore` comments in source files.
///
/// For each finding at line N, checks for suppression comments at line N
/// (same-line) and line N-1 (line-before).
///
/// Returns `(kept_findings, suppressed_count)`.
pub fn filter_findings_by_inline(findings: Vec<Finding>) -> (Vec<Finding>, usize) {
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
    let mut suppressed = 0usize;

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
            suppressed += 1;
        } else {
            kept.push(finding);
        }
    }

    (kept, suppressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_prefix() {
        let sups = parse_suppressions("# revet-ignore SEC\npassword = 'abc'");
        assert_eq!(sups.get(&1).unwrap(), &["SEC"]);
        assert!(!sups.contains_key(&2));
    }

    #[test]
    fn test_parse_multiple_prefixes() {
        let sups = parse_suppressions("// revet-ignore SEC SQL");
        assert_eq!(sups.get(&1).unwrap(), &["SEC", "SQL"]);
    }

    #[test]
    fn test_parse_wildcard() {
        let sups = parse_suppressions("# revet-ignore *");
        assert_eq!(sups.get(&1).unwrap(), &["*"]);
    }

    #[test]
    fn test_matches_suppression_prefix() {
        assert!(matches_suppression("SEC-001", &["SEC".into()]));
        assert!(matches_suppression("SEC-042", &["SEC".into()]));
        assert!(!matches_suppression("SQL-001", &["SEC".into()]));
    }

    #[test]
    fn test_matches_suppression_wildcard() {
        assert!(matches_suppression("SEC-001", &["*".into()]));
        assert!(matches_suppression("ML-001", &["*".into()]));
    }

    #[test]
    fn test_matches_suppression_multiple() {
        let prefixes = vec!["SEC".into(), "SQL".into()];
        assert!(matches_suppression("SEC-001", &prefixes));
        assert!(matches_suppression("SQL-003", &prefixes));
        assert!(!matches_suppression("ML-001", &prefixes));
    }
}
