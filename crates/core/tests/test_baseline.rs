use revet_core::{filter_findings, Baseline, BaselineEntry, Finding, Severity};
use std::path::PathBuf;
use tempfile::TempDir;

fn make_finding(file: &str, message: &str, line: usize) -> Finding {
    Finding {
        id: "TEST-001".to_string(),
        severity: Severity::Warning,
        message: message.to_string(),
        file: PathBuf::from(file),
        line,
        affected_dependents: 0,
        suggestion: None,
        fix_kind: None,
    }
}

#[test]
fn test_save_load_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let findings = vec![
        make_finding("src/main.py", "Hardcoded AWS access key detected", 10),
        make_finding("src/db.py", "SQL injection risk", 25),
    ];

    let baseline = Baseline::from_findings(&findings, root, Some("abc123".to_string()));
    assert_eq!(baseline.count, 2);
    assert_eq!(baseline.version, "1");
    assert_eq!(baseline.commit, Some("abc123".to_string()));

    baseline.save(root).unwrap();

    let loaded = Baseline::load(root)
        .unwrap()
        .expect("baseline should exist");
    assert_eq!(loaded.count, 2);
    assert_eq!(loaded.entries.len(), 2);
    assert_eq!(loaded.entries[0].file, "src/main.py");
    assert_eq!(
        loaded.entries[0].message,
        "Hardcoded AWS access key detected"
    );
    assert_eq!(loaded.entries[1].file, "src/db.py");
}

#[test]
fn test_filter_findings() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let baseline = Baseline {
        version: "1".to_string(),
        created_at: "0".to_string(),
        commit: None,
        count: 1,
        entries: vec![BaselineEntry {
            file: "src/main.py".to_string(),
            message: "Hardcoded AWS access key detected".to_string(),
        }],
    };

    let findings = vec![
        make_finding(
            &root.join("src/main.py").to_string_lossy(),
            "Hardcoded AWS access key detected",
            10,
        ),
        make_finding(&root.join("src/new.py").to_string_lossy(), "New finding", 5),
    ];

    let (new, suppressed) = filter_findings(findings, &baseline, root);
    assert_eq!(suppressed, 1);
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].message, "New finding");
}

#[test]
fn test_filter_empty_baseline() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let baseline = Baseline {
        version: "1".to_string(),
        created_at: "0".to_string(),
        commit: None,
        count: 0,
        entries: vec![],
    };

    let findings = vec![make_finding(
        &root.join("src/main.py").to_string_lossy(),
        "Hardcoded AWS access key detected",
        10,
    )];

    let (new, suppressed) = filter_findings(findings, &baseline, root);
    assert_eq!(suppressed, 0);
    assert_eq!(new.len(), 1);
}

#[test]
fn test_clear_baseline() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    // Clear when no baseline exists → false
    assert!(!Baseline::clear(root).unwrap());

    // Save a baseline, then clear → true
    let baseline = Baseline::from_findings(&[], root, None);
    baseline.save(root).unwrap();
    assert!(Baseline::clear(root).unwrap());

    // Verify it's gone
    assert!(Baseline::load(root).unwrap().is_none());
}

#[test]
fn test_fingerprint_ignores_line_number() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let baseline = Baseline {
        version: "1".to_string(),
        created_at: "0".to_string(),
        commit: None,
        count: 1,
        entries: vec![BaselineEntry {
            file: "src/main.py".to_string(),
            message: "Hardcoded AWS access key detected".to_string(),
        }],
    };

    // Same file and message, different line number → still suppressed
    let findings = vec![make_finding(
        &root.join("src/main.py").to_string_lossy(),
        "Hardcoded AWS access key detected",
        999, // different line number
    )];

    let (new, suppressed) = filter_findings(findings, &baseline, root);
    assert_eq!(suppressed, 1);
    assert_eq!(new.len(), 0);
}

#[test]
fn test_different_message_not_matched() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();

    let baseline = Baseline {
        version: "1".to_string(),
        created_at: "0".to_string(),
        commit: None,
        count: 1,
        entries: vec![BaselineEntry {
            file: "src/main.py".to_string(),
            message: "Hardcoded AWS access key detected".to_string(),
        }],
    };

    // Same file, different message → not suppressed
    let findings = vec![make_finding(
        &root.join("src/main.py").to_string_lossy(),
        "SQL injection risk",
        10,
    )];

    let (new, suppressed) = filter_findings(findings, &baseline, root);
    assert_eq!(suppressed, 0);
    assert_eq!(new.len(), 1);
}

#[test]
fn test_load_nonexistent() {
    let tmp = TempDir::new().unwrap();
    assert!(Baseline::load(tmp.path()).unwrap().is_none());
}
