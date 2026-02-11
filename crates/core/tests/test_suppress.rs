use revet_core::finding::{Finding, Severity};
use revet_core::suppress::{filter_findings_by_inline, parse_suppressions};
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

// ── parse_suppressions ──────────────────────────────────────────

#[test]
fn test_parse_python_comment() {
    let content = "# revet-ignore SEC\npassword = 'abc'\n";
    let sups = parse_suppressions(content);
    assert_eq!(sups.len(), 1);
    assert_eq!(sups.get(&1).unwrap(), &["SEC"]);
}

#[test]
fn test_parse_js_comment() {
    let content = "const key = 'secret'; // revet-ignore SEC\n";
    let sups = parse_suppressions(content);
    assert_eq!(sups.len(), 1);
    assert_eq!(sups.get(&1).unwrap(), &["SEC"]);
}

#[test]
fn test_parse_multiple_prefixes() {
    let content = "// revet-ignore SEC SQL\nconst q = `SELECT * FROM ${t}`;\n";
    let sups = parse_suppressions(content);
    assert_eq!(sups.get(&1).unwrap(), &["SEC", "SQL"]);
}

#[test]
fn test_parse_wildcard() {
    let content = "# revet-ignore *\nsome_code()\n";
    let sups = parse_suppressions(content);
    assert_eq!(sups.get(&1).unwrap(), &["*"]);
}

#[test]
fn test_parse_no_suppressions() {
    let content = "let x = 1;\nlet y = 2;\n";
    let sups = parse_suppressions(content);
    assert!(sups.is_empty());
}

#[test]
fn test_parse_multiple_lines() {
    let content = "# revet-ignore SEC\npassword = 'abc'\n# revet-ignore ML\nfit(X_test)\n";
    let sups = parse_suppressions(content);
    assert_eq!(sups.len(), 2);
    assert_eq!(sups.get(&1).unwrap(), &["SEC"]);
    assert_eq!(sups.get(&3).unwrap(), &["ML"]);
}

#[test]
fn test_parse_dash_in_prefix() {
    let content = "// revet-ignore MY-CUSTOM\ncode()\n";
    let sups = parse_suppressions(content);
    assert_eq!(sups.get(&1).unwrap(), &["MY-CUSTOM"]);
}

// ── filter_findings_by_inline ──────────────────────────────────

fn make_finding(id: &str, file: PathBuf, line: usize) -> Finding {
    Finding {
        id: id.to_string(),
        severity: Severity::Warning,
        message: format!("Test finding {}", id),
        file,
        line,
        affected_dependents: 0,
        suggestion: None,
        fix_kind: None,
    }
}

#[test]
fn test_same_line_suppression() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "password = 'abc' # revet-ignore SEC").unwrap();
    let path = f.path().to_path_buf();

    let findings = vec![make_finding("SEC-001", path, 1)];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 1);
    assert!(kept.is_empty());
}

#[test]
fn test_line_before_suppression() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "# revet-ignore SEC").unwrap();
    writeln!(f, "password = 'abc'").unwrap();
    let path = f.path().to_path_buf();

    let findings = vec![make_finding("SEC-001", path, 2)];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 1);
    assert!(kept.is_empty());
}

#[test]
fn test_no_suppression_wrong_line() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "# revet-ignore SEC").unwrap();
    writeln!(f, "clean_line()").unwrap();
    writeln!(f, "password = 'abc'").unwrap();
    let path = f.path().to_path_buf();

    // Finding on line 3, suppression on line 1 — too far away
    let findings = vec![make_finding("SEC-001", path, 3)];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 0);
    assert_eq!(kept.len(), 1);
}

#[test]
fn test_prefix_matching() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "// revet-ignore SEC").unwrap();
    writeln!(f, "const key = 'secret';").unwrap();
    let path = f.path().to_path_buf();

    // SEC prefix should match SEC-001, SEC-042 etc.
    let findings = vec![
        make_finding("SEC-001", path.clone(), 2),
        make_finding("SEC-042", path.clone(), 2),
        make_finding("SQL-001", path, 2), // should NOT be suppressed
    ];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 2);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id, "SQL-001");
}

#[test]
fn test_wildcard_suppresses_all() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "// revet-ignore *").unwrap();
    writeln!(f, "const key = 'secret';").unwrap();
    let path = f.path().to_path_buf();

    let findings = vec![
        make_finding("SEC-001", path.clone(), 2),
        make_finding("SQL-001", path.clone(), 2),
        make_finding("ML-001", path, 2),
    ];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 3);
    assert!(kept.is_empty());
}

#[test]
fn test_multiple_prefixes_on_one_line() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "// revet-ignore SEC SQL").unwrap();
    writeln!(f, "const q = db.query(`SELECT ${{key}}`);").unwrap();
    let path = f.path().to_path_buf();

    let findings = vec![
        make_finding("SEC-001", path.clone(), 2),
        make_finding("SQL-001", path.clone(), 2),
        make_finding("ML-001", path, 2), // should NOT be suppressed
    ];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 2);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id, "ML-001");
}

#[test]
fn test_no_file_on_disk() {
    // Finding points to a file that doesn't exist — should not crash, just keep it
    let findings = vec![make_finding(
        "SEC-001",
        PathBuf::from("/nonexistent/file.py"),
        1,
    )];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 0);
    assert_eq!(kept.len(), 1);
}

#[test]
fn test_finding_on_line_1_no_crash() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "password = 'abc'").unwrap();
    let path = f.path().to_path_buf();

    // Line 1 finding — line-before check (line 0) should not panic
    let findings = vec![make_finding("SEC-001", path, 1)];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 0);
    assert_eq!(kept.len(), 1);
}

#[test]
fn test_mixed_suppressed_and_kept() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "clean_line()").unwrap();
    writeln!(f, "# revet-ignore SEC").unwrap();
    writeln!(f, "password = 'abc'").unwrap();
    writeln!(f, "another_secret = 'xyz'").unwrap();
    let path = f.path().to_path_buf();

    let findings = vec![
        make_finding("SEC-001", path.clone(), 3), // suppressed (line-before)
        make_finding("SEC-002", path, 4),         // NOT suppressed (line 3 has no comment)
    ];
    let (kept, suppressed) = filter_findings_by_inline(findings);
    assert_eq!(suppressed, 1);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].id, "SEC-002");
}
