use revet_cli::output::sarif::build_sarif_log;
use revet_core::{Finding, Severity};
use std::path::{Path, PathBuf};

fn make_finding(id: &str, severity: Severity, message: &str, file: &str, line: usize) -> Finding {
    Finding {
        id: id.to_string(),
        severity,
        message: message.to_string(),
        file: PathBuf::from(file),
        line,
        affected_dependents: 0,
        suggestion: None,
        fix_kind: None,
        ..Default::default()
    }
}

#[test]
fn test_empty_findings() {
    let log = build_sarif_log(&[], Path::new("/repo"));
    assert_eq!(log.version, "2.1.0");
    assert_eq!(log.runs.len(), 1);
    assert!(log.runs[0].results.is_empty());
    assert!(log.runs[0].tool.driver.rules.is_empty());

    let json = serde_json::to_string_pretty(&log).unwrap();
    assert!(json.contains("\"version\": \"2.1.0\""));
}

#[test]
fn test_single_finding() {
    let findings = vec![make_finding(
        "SEC-001",
        Severity::Error,
        "Hardcoded AWS key",
        "/repo/src/config.py",
        42,
    )];
    let log = build_sarif_log(&findings, Path::new("/repo"));

    assert_eq!(log.runs[0].results.len(), 1);
    let result = &log.runs[0].results[0];
    assert_eq!(result.rule_id, "SEC");
    assert_eq!(result.level, "error");
    assert_eq!(result.message.text, "Hardcoded AWS key");
    assert_eq!(
        result.locations[0].physical_location.artifact_location.uri,
        "src/config.py"
    );
    assert_eq!(result.locations[0].physical_location.region.start_line, 42);
}

#[test]
fn test_severity_mapping() {
    let findings = vec![
        make_finding("SEC-001", Severity::Error, "err", "/repo/a.py", 1),
        make_finding("SEC-002", Severity::Warning, "warn", "/repo/b.py", 2),
        make_finding("SEC-003", Severity::Info, "info", "/repo/c.py", 3),
    ];
    let log = build_sarif_log(&findings, Path::new("/repo"));

    assert_eq!(log.runs[0].results[0].level, "error");
    assert_eq!(log.runs[0].results[1].level, "warning");
    assert_eq!(log.runs[0].results[2].level, "note");
}

#[test]
fn test_rules_deduplication() {
    let findings = vec![
        make_finding("SEC-001", Severity::Error, "a", "/repo/a.py", 1),
        make_finding("SEC-002", Severity::Warning, "b", "/repo/b.py", 2),
        make_finding("SQL-001", Severity::Error, "c", "/repo/c.py", 3),
        make_finding("SEC-003", Severity::Info, "d", "/repo/d.py", 4),
        make_finding("SQL-002", Severity::Warning, "e", "/repo/e.py", 5),
    ];
    let log = build_sarif_log(&findings, Path::new("/repo"));

    assert_eq!(log.runs[0].tool.driver.rules.len(), 2);
    assert_eq!(log.runs[0].tool.driver.rules[0].id, "SEC");
    assert_eq!(log.runs[0].tool.driver.rules[1].id, "SQL");
}

#[test]
fn test_file_path_normalization() {
    let findings = vec![make_finding(
        "IMPACT-001",
        Severity::Warning,
        "change",
        "/home/user/project/src/lib/utils.rs",
        10,
    )];
    let log = build_sarif_log(&findings, Path::new("/home/user/project"));

    assert_eq!(
        log.runs[0].results[0].locations[0]
            .physical_location
            .artifact_location
            .uri,
        "src/lib/utils.rs"
    );
    assert_eq!(
        log.runs[0].results[0].locations[0]
            .physical_location
            .artifact_location
            .uri_base_id,
        "%SRCROOT%"
    );
}

#[test]
fn test_rule_index_matches() {
    let findings = vec![
        make_finding("ML-001", Severity::Warning, "a", "/repo/a.py", 1),
        make_finding("SEC-001", Severity::Error, "b", "/repo/b.py", 2),
        make_finding("ML-002", Severity::Info, "c", "/repo/c.py", 3),
    ];
    let log = build_sarif_log(&findings, Path::new("/repo"));

    // BTreeMap ordering: ML=0, SEC=1
    let rules = &log.runs[0].tool.driver.rules;
    assert_eq!(rules[0].id, "ML");
    assert_eq!(rules[1].id, "SEC");

    assert_eq!(log.runs[0].results[0].rule_index, 0); // ML
    assert_eq!(log.runs[0].results[1].rule_index, 1); // SEC
    assert_eq!(log.runs[0].results[2].rule_index, 0); // ML
}

#[test]
fn test_roundtrip_serialization() {
    use revet_cli::output::sarif::SarifLog;

    let findings = vec![
        make_finding(
            "SEC-001",
            Severity::Error,
            "secret found",
            "/repo/src/main.rs",
            15,
        ),
        make_finding(
            "SQL-001",
            Severity::Warning,
            "sql issue",
            "/repo/src/db.rs",
            30,
        ),
    ];
    let log = build_sarif_log(&findings, Path::new("/repo"));

    let json = serde_json::to_string_pretty(&log).unwrap();
    let deserialized: SarifLog = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.version, "2.1.0");
    assert_eq!(deserialized.runs.len(), 1);
    assert_eq!(deserialized.runs[0].results.len(), 2);
    assert_eq!(deserialized.runs[0].tool.driver.rules.len(), 2);
    assert_eq!(deserialized.runs[0].results[0].rule_id, "SEC");
    assert_eq!(deserialized.runs[0].results[1].rule_id, "SQL");
}

#[test]
fn test_unknown_prefix() {
    let findings = vec![make_finding(
        "CUSTOM-001",
        Severity::Info,
        "custom finding",
        "/repo/file.txt",
        1,
    )];
    let log = build_sarif_log(&findings, Path::new("/repo"));

    assert_eq!(log.runs[0].tool.driver.rules[0].id, "CUSTOM");
    assert_eq!(
        log.runs[0].tool.driver.rules[0].short_description.text,
        "Code review finding"
    );
}
