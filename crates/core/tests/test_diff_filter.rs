//! Tests for filter_findings_by_diff

use revet_core::diff::{DiffFileLines, DiffLineMap};
use revet_core::{filter_findings_by_diff, Finding, Severity};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn make_finding(file: &str, line: usize) -> Finding {
    Finding {
        id: format!("TEST-{:03}", line),
        severity: Severity::Warning,
        message: format!("test finding at line {}", line),
        file: PathBuf::from(file),
        line,
        affected_dependents: 0,
        suggestion: None,
        fix_kind: None,
        ..Default::default()
    }
}

#[test]
fn finding_on_changed_line_is_kept() {
    let mut map = DiffLineMap::new();
    map.insert(
        PathBuf::from("src/main.py"),
        DiffFileLines::Lines(HashSet::from([10, 20, 30])),
    );

    let findings = vec![make_finding("src/main.py", 10)];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 1);
    assert_eq!(filtered, 0);
}

#[test]
fn finding_on_unchanged_line_is_filtered() {
    let mut map = DiffLineMap::new();
    map.insert(
        PathBuf::from("src/main.py"),
        DiffFileLines::Lines(HashSet::from([10, 20])),
    );

    let findings = vec![make_finding("src/main.py", 15)];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 0);
    assert_eq!(filtered, 1);
}

#[test]
fn finding_in_new_file_allnew_is_kept() {
    let mut map = DiffLineMap::new();
    map.insert(PathBuf::from("src/new_file.py"), DiffFileLines::AllNew);

    let findings = vec![
        make_finding("src/new_file.py", 1),
        make_finding("src/new_file.py", 50),
        make_finding("src/new_file.py", 100),
    ];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 3);
    assert_eq!(filtered, 0);
}

#[test]
fn finding_in_file_not_in_map_is_filtered() {
    let map = DiffLineMap::new(); // empty map

    let findings = vec![make_finding("src/untouched.py", 5)];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 0);
    assert_eq!(filtered, 1);
}

#[test]
fn mixed_findings_kept_and_filtered() {
    let mut map = DiffLineMap::new();
    map.insert(
        PathBuf::from("src/app.py"),
        DiffFileLines::Lines(HashSet::from([10, 20])),
    );
    map.insert(PathBuf::from("src/new.py"), DiffFileLines::AllNew);

    let findings = vec![
        make_finding("src/app.py", 10),  // kept — changed line
        make_finding("src/app.py", 15),  // filtered — unchanged
        make_finding("src/new.py", 5),   // kept — new file
        make_finding("src/other.py", 1), // filtered — not in map
    ];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 2);
    assert_eq!(filtered, 2);
    assert_eq!(kept[0].file, PathBuf::from("src/app.py"));
    assert_eq!(kept[1].file, PathBuf::from("src/new.py"));
}

#[test]
fn empty_diff_map_filters_everything() {
    let map = DiffLineMap::new();
    let findings = vec![make_finding("src/a.py", 1), make_finding("src/b.py", 2)];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 0);
    assert_eq!(filtered, 2);
}

#[test]
fn empty_findings_returns_empty() {
    let mut map = DiffLineMap::new();
    map.insert(
        PathBuf::from("src/main.py"),
        DiffFileLines::Lines(HashSet::from([10])),
    );

    let findings: Vec<Finding> = vec![];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 0);
    assert_eq!(filtered, 0);
}

#[test]
fn relative_path_normalization_with_repo_root() {
    let mut map = DiffLineMap::new();
    map.insert(
        PathBuf::from("src/main.py"),
        DiffFileLines::Lines(HashSet::from([10])),
    );

    // Finding has absolute path that needs to be relativized
    let findings = vec![make_finding("/repo/src/main.py", 10)];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new("/repo"));

    assert_eq!(kept.len(), 1);
    assert_eq!(filtered, 0);
}

#[test]
fn filtered_count_is_correct_with_many_findings() {
    let mut map = DiffLineMap::new();
    map.insert(
        PathBuf::from("src/main.py"),
        DiffFileLines::Lines(HashSet::from([1, 5, 10])),
    );

    let findings = vec![
        make_finding("src/main.py", 1),  // kept
        make_finding("src/main.py", 2),  // filtered
        make_finding("src/main.py", 3),  // filtered
        make_finding("src/main.py", 5),  // kept
        make_finding("src/main.py", 7),  // filtered
        make_finding("src/main.py", 10), // kept
    ];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 3);
    assert_eq!(filtered, 3);
    assert_eq!(kept[0].line, 1);
    assert_eq!(kept[1].line, 5);
    assert_eq!(kept[2].line, 10);
}

#[test]
fn finding_with_line_zero_in_new_file_is_kept() {
    // Parse errors have line 0 — should be kept if file is AllNew
    let mut map = DiffLineMap::new();
    map.insert(PathBuf::from("src/bad.py"), DiffFileLines::AllNew);

    let findings = vec![make_finding("src/bad.py", 0)];
    let (kept, filtered) = filter_findings_by_diff(findings, &map, Path::new(""));

    assert_eq!(kept.len(), 1);
    assert_eq!(filtered, 0);
}
