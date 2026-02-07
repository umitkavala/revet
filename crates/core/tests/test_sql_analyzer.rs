//! Integration tests for SqlInjectionAnalyzer

use revet_core::analyzer::sql_injection::SqlInjectionAnalyzer;
use revet_core::analyzer::Analyzer;
use revet_core::config::RevetConfig;
use revet_core::finding::Severity;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper: create a temp file with given content and return its path
fn write_temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

fn default_config() -> RevetConfig {
    RevetConfig::default()
}

// ── Detection tests: DB execution calls (Error) ────────────────

#[test]
fn test_detects_fstring_in_execute() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.py",
        r#"cursor.execute(f"SELECT * FROM users WHERE id = {uid}")
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0]
        .message
        .contains("f-string SQL in database call"));
    assert_eq!(findings[0].line, 1);
}

#[test]
fn test_detects_concat_in_execute() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.py",
        r#"cursor.execute("SELECT * FROM users WHERE id = " + uid)
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0]
        .message
        .contains("string concatenation SQL in database call"));
}

#[test]
fn test_detects_format_in_execute() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.py",
        r#"cursor.execute("SELECT * FROM users WHERE id = {}".format(uid))
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0]
        .message
        .contains(".format() SQL in database call"));
}

#[test]
fn test_detects_percent_format_in_execute() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.py",
        r#"cursor.execute("SELECT * FROM users WHERE id = %s" % uid)
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0]
        .message
        .contains("%-format SQL in database call"));
}

#[test]
fn test_detects_template_literal_in_query() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "routes.js",
        "db.query(`SELECT * FROM users WHERE id = ${uid}`)\n",
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0]
        .message
        .contains("template literal SQL in database call"));
}

#[test]
fn test_detects_django_raw_query() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "views.py",
        r#"users = User.objects.raw(f"SELECT * FROM auth_user WHERE name = '{name}'")
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0]
        .message
        .contains("ORM raw query with interpolation"));
}

// ── Detection tests: standalone SQL strings (Warning) ──────────

#[test]
fn test_detects_fstring_sql_assignment() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "queries.py",
        r#"query = f"SELECT * FROM users WHERE id = {uid}"
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("f-string SQL assignment"));
}

#[test]
fn test_detects_concat_sql_standalone() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "queries.py",
        r#"query = "SELECT * FROM users WHERE id = " + uid
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("string concatenation SQL"));
}

#[test]
fn test_detects_template_literal_sql_assignment() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "queries.ts",
        "const q = `SELECT * FROM users WHERE id = ${uid}`\n",
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("template literal SQL"));
}

// ── False positive tests ────────────────────────────────────────

#[test]
fn test_no_match_parameterized_query_tuple() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "safe.py",
        r#"cursor.execute("SELECT * FROM users WHERE id = %s", (uid,))
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Parameterized queries should not trigger findings, got: {:?}",
        findings
    );
}

#[test]
fn test_no_match_parameterized_query_placeholder() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "safe.js",
        r#"db.query("SELECT * FROM users WHERE id = ?", [uid])
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Placeholder queries should not trigger findings, got: {:?}",
        findings
    );
}

#[test]
fn test_no_match_clean_code() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "clean.py",
        r#"
import sqlite3

def get_user(uid):
    """Fetch user safely."""
    conn = sqlite3.connect("mydb.sqlite")
    cursor = conn.cursor()
    cursor.execute("SELECT * FROM users WHERE id = ?", (uid,))
    return cursor.fetchone()

class UserRepository:
    def find_all(self):
        return self.session.query(User).all()
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Clean code should not trigger findings, got: {:?}",
        findings
    );
}

#[test]
fn test_skips_comment_lines() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "commented.py",
        r#"# cursor.execute(f"SELECT * FROM users WHERE id = {uid}")
// db.query(`SELECT * FROM users WHERE id = ${uid}`)
* execute(f"DELETE FROM table WHERE id = {id}")
-- SELECT * FROM users + variable
"#,
    );

    let analyzer = SqlInjectionAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Comment lines should not trigger findings, got: {:?}",
        findings
    );
}

// ── Infrastructure tests ────────────────────────────────────────

#[test]
fn test_respects_config_disabled() {
    let mut config = default_config();
    config.modules.security = false;

    let analyzer = SqlInjectionAnalyzer::new();
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_sequential_via_dispatcher() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "multi.py",
        r#"cursor.execute(f"SELECT * FROM users WHERE id = {uid}")
cursor.execute("DELETE FROM orders WHERE id = " + oid)
query = f"UPDATE items SET name = {name} WHERE id = {iid}"
"#,
    );

    let config = default_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    // Filter to SQL findings only
    let sql_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("SQL"))
        .collect();

    assert_eq!(sql_findings.len(), 3);
    assert_eq!(sql_findings[0].id, "SQL-001");
    assert_eq!(sql_findings[1].id, "SQL-002");
    assert_eq!(sql_findings[2].id, "SQL-003");
}
