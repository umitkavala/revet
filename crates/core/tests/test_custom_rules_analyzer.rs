//! Tests for the custom rules analyzer

use revet_core::{AnalyzerDispatcher, Finding, RevetConfig, Severity};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper: parse a TOML config string into RevetConfig
fn config_from_toml(toml_str: &str) -> RevetConfig {
    toml::from_str(toml_str).expect("should parse TOML")
}

/// Helper: create a temp file with given content, return its absolute path
fn write_temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    path
}

/// Helper: run the custom rules analyzer via AnalyzerDispatcher on given files
fn run_custom(
    config: &RevetConfig,
    files: &[PathBuf],
    repo_root: &std::path::Path,
) -> Vec<Finding> {
    let dispatcher = AnalyzerDispatcher::new_with_config(config);
    dispatcher.run_all(files, repo_root, config)
}

// ── Basic matching ──────────────────────────────────────────────────────

#[test]
fn test_basic_match() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('debug');\nlet x = 1;\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log in production"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].message, "No console.log in production");
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].id.starts_with("CUSTOM-"));
}

#[test]
fn test_no_match() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "logger.info('hello');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert!(findings.is_empty());
}

#[test]
fn test_wrong_file_type() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.py", "console.log('debug');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert!(
        findings.is_empty(),
        "should not match .py file when paths = [\"*.ts\"]"
    );
}

// ── Multiple matches & rules ────────────────────────────────────────────

#[test]
fn test_multiple_matches_same_rule() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.ts",
        "console.log('a');\nlet x = 1;\nconsole.log('b');\n",
    );

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].line, 1);
    assert_eq!(findings[1].line, 3);
}

#[test]
fn test_multiple_rules() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('a');\n// TODO: fix this\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]

[[rules]]
pattern = "TODO|FIXME"
message = "Unresolved TODO"
severity = "info"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0].message, "No console.log");
    assert_eq!(findings[1].message, "Unresolved TODO");
}

#[test]
fn test_first_rule_wins_per_line() {
    let dir = TempDir::new().unwrap();
    // This line matches both rules
    let file = write_temp_file(&dir, "app.ts", "console.log('TODO: fix');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]

[[rules]]
pattern = "TODO"
message = "Unresolved TODO"
severity = "info"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "first rule wins, only one finding per line"
    );
    assert_eq!(findings[0].message, "No console.log");
}

// ── Severities ──────────────────────────────────────────────────────────

#[test]
fn test_severity_error() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "eval('code');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'eval\('
message = "No eval"
severity = "error"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_severity_info() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "// TODO\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = "TODO"
message = "TODO found"
severity = "info"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings[0].severity, Severity::Info);
}

#[test]
fn test_invalid_severity_defaults_to_warning() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('x');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "critical"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings[0].severity, Severity::Warning);
}

// ── Suggestion ──────────────────────────────────────────────────────────

#[test]
fn test_suggestion_present() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('x');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
suggestion = "Use logger.info() instead"
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(
        findings[0].suggestion.as_deref(),
        Some("Use logger.info() instead")
    );
}

#[test]
fn test_suggestion_absent() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('x');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert!(findings[0].suggestion.is_none());
}

// ── reject_if_contains ──────────────────────────────────────────────────

#[test]
fn test_reject_if_contains() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.ts",
        "console.log('a');\nconsole.log('b'); // eslint-disable\n",
    );

    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
reject_if_contains = "// eslint-disable"
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "second line should be rejected by negative filter"
    );
    assert_eq!(findings[0].line, 1);
}

// ── Invalid regex ───────────────────────────────────────────────────────

#[test]
fn test_invalid_regex_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('x');\n");

    let config = config_from_toml(
        r#"
[[rules]]
pattern = '[invalid'
message = "Should be skipped"
severity = "warning"
paths = ["*.ts"]

[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings.len(), 1, "invalid regex rule should be skipped");
    assert_eq!(findings[0].message, "No console.log");
}

// ── Empty rules ─────────────────────────────────────────────────────────

#[test]
fn test_empty_rules_disabled() {
    let config = config_from_toml("");

    // With no rules, the custom analyzer should not be enabled
    let dispatcher = AnalyzerDispatcher::new_with_config(&config);
    let findings = dispatcher.run_all(&[], std::path::Path::new("."), &config);
    assert!(findings.is_empty());
}

// ── Dispatcher renumbering ──────────────────────────────────────────────

#[test]
fn test_dispatcher_renumbering() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.ts",
        "console.log('a');\nconsole.log('b');\nconsole.log('c');\n",
    );

    let config = config_from_toml(
        r#"
[modules]
security = false
ml = false

[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings.len(), 3);
    assert_eq!(findings[0].id, "CUSTOM-001");
    assert_eq!(findings[1].id, "CUSTOM-002");
    assert_eq!(findings[2].id, "CUSTOM-003");
}

// ── Glob path matching ─────────────────────────────────────────────────

#[test]
fn test_glob_matches_multiple_extensions() {
    let dir = TempDir::new().unwrap();
    let ts_file = write_temp_file(&dir, "app.ts", "console.log('a');\n");
    let js_file = write_temp_file(&dir, "app.js", "console.log('b');\n");
    let py_file = write_temp_file(&dir, "app.py", "console.log('c');\n");

    let config = config_from_toml(
        r#"
[modules]
security = false
ml = false

[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts", "*.js"]
"#,
    );

    let findings = run_custom(&config, &[ts_file, js_file, py_file], dir.path());
    assert_eq!(findings.len(), 2, "should match .ts and .js but not .py");
}

// ── No paths means match all files ──────────────────────────────────────

#[test]
fn test_no_paths_matches_all_files() {
    let dir = TempDir::new().unwrap();
    let ts_file = write_temp_file(&dir, "app.ts", "TODO: fix\n");
    let py_file = write_temp_file(&dir, "app.py", "# TODO: fix\n");
    let rs_file = write_temp_file(&dir, "app.rs", "// TODO: fix\n");

    let config = config_from_toml(
        r#"
[modules]
security = false
ml = false

[[rules]]
pattern = "TODO"
message = "Unresolved TODO"
severity = "info"
"#,
    );

    let findings = run_custom(&config, &[ts_file, py_file, rs_file], dir.path());
    assert_eq!(findings.len(), 3, "no paths filter should match all files");
}

// ── Optional id field ───────────────────────────────────────────────────

#[test]
fn test_id_is_optional() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('x');\n");

    // No id field in the rule
    let config = config_from_toml(
        r#"
[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings.len(), 1);
}

#[test]
fn test_id_present() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "console.log('x');\n");

    let config = config_from_toml(
        r#"
[[rules]]
id = "no-console-log"
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    assert_eq!(findings.len(), 1);
    // id field is for human reference only, finding ID uses CUSTOM-NNN
    assert!(findings[0].id.starts_with("CUSTOM-"));
}

// ── Coexists with built-in analyzers ────────────────────────────────────

#[test]
fn test_coexists_with_builtin() {
    let dir = TempDir::new().unwrap();
    // This file triggers both SEC (hardcoded password) and custom rule
    let file = write_temp_file(
        &dir,
        "config.ts",
        "const password = \"supersecret123\";\nconsole.log('debug');\n",
    );

    let config = config_from_toml(
        r#"
[modules]
security = true
ml = false

[[rules]]
pattern = 'console\.log'
message = "No console.log"
severity = "warning"
paths = ["*.ts"]
"#,
    );

    let findings = run_custom(&config, &[file], dir.path());
    let sec_count = findings.iter().filter(|f| f.id.starts_with("SEC-")).count();
    let custom_count = findings
        .iter()
        .filter(|f| f.id.starts_with("CUSTOM-"))
        .count();
    assert!(sec_count >= 1, "should have SEC findings");
    assert_eq!(custom_count, 1, "should have 1 CUSTOM finding");
}
