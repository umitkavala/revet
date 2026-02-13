//! Integration tests for ErrorHandlingAnalyzer

use revet_core::analyzer::error_handling::ErrorHandlingAnalyzer;
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

fn error_config() -> RevetConfig {
    let mut config = RevetConfig::default();
    config.modules.error_handling = true;
    config
}

// ── ERR-001: Empty catch/except block ───────────────────────────

#[test]
fn test_empty_catch_block_js() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "handler.js",
        r#"
try {
  riskyOperation();
} catch (e) {}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Empty catch/except block"));
}

#[test]
fn test_empty_except_pass_python() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "handler.py",
        "try:\n    risky()\nexcept ValueError: pass\n",
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Empty catch/except block"));
}

#[test]
fn test_non_empty_catch_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "handler.js",
        r#"
try {
  riskyOperation();
} catch (e) { retryWithBackoff(e); }
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Non-empty catch should not trigger, got: {:?}",
        findings
    );
}

// ── ERR-002: Bare except ────────────────────────────────────────

#[test]
fn test_bare_except_python() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.py",
        "try:\n    connect()\nexcept:\n    retry()\n",
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Bare except"));
}

#[test]
fn test_typed_except_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.py",
        "try:\n    connect()\nexcept ConnectionError:\n    retry()\n",
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // typed except should not trigger bare except
    let bare = findings.iter().any(|f| f.message.contains("Bare except"));
    assert!(!bare, "Typed except should not trigger bare except");
}

// ── ERR-003: .unwrap() ─────────────────────────────────────────

#[test]
fn test_unwrap_rust() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "main.rs",
        r#"
fn main() {
    let data = std::fs::read_to_string("file.txt").unwrap();
    println!("{}", data);
}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains(".unwrap()"));
}

#[test]
fn test_unwrap_or_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "main.rs",
        r#"
fn main() {
    let data = std::fs::read_to_string("file.txt").unwrap_or_default();
}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        ".unwrap_or() should not trigger, got: {:?}",
        findings
    );
}

#[test]
fn test_unwrap_only_in_rust_files() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "script.py", "result.unwrap()\n");

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        ".unwrap() should only trigger in .rs files, got: {:?}",
        findings
    );
}

// ── ERR-004: panic!/todo!/unimplemented! ────────────────────────

#[test]
fn test_panic_in_non_test_rust() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "lib.rs",
        r#"
fn process(data: &str) {
    if data.is_empty() {
        panic!("data cannot be empty");
    }
}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("panic!"));
}

#[test]
fn test_todo_in_non_test_rust() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "lib.rs",
        r#"
fn process() -> Result<(), String> {
    todo!("implement later")
}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("panic!") || findings[0].message.contains("todo!"));
}

#[test]
fn test_panic_in_test_file_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "test_lib.rs",
        r#"
#[test]
fn test_something() {
    panic!("expected failure");
}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "panic! in test files should not trigger, got: {:?}",
        findings
    );
}

// ── ERR-005: Catch that only logs ───────────────────────────────

#[test]
fn test_catch_only_logs_js() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.ts",
        r#"
try { fetch("/api"); } catch (e) { console.error(e);
}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("only logs"));
}

#[test]
fn test_catch_logs_and_throws_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.ts",
        r#"
try { fetch("/api"); } catch (e) { console.error(e); throw e; }
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // "throw" on the line should suppress via reject_if_contains
    let log_only = findings.iter().any(|f| f.message.contains("only logs"));
    assert!(!log_only, "catch with throw should not trigger log-only");
}

// ── ERR-006: Too-broad exception ────────────────────────────────

#[test]
fn test_except_exception_python() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.py",
        "try:\n    run()\nexcept Exception as e:\n    handle(e)\n",
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Too-broad exception"));
}

#[test]
fn test_except_base_exception_python() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.py",
        "try:\n    run()\nexcept BaseException:\n    handle()\n",
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("Too-broad exception"));
}

// ── ERR-007: Empty .catch() callback ────────────────────────────

#[test]
fn test_empty_catch_callback_js() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "fetch.ts",
        r#"
fetch("/api").catch(() => {});
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Empty .catch() callback"));
}

#[test]
fn test_non_empty_catch_callback_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "fetch.ts",
        r#"
fetch("/api").catch((err) => { console.error(err); });
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // Non-empty .catch() callback should not trigger ERR-007
    let empty_catch = findings
        .iter()
        .any(|f| f.message.contains("Empty .catch()"));
    assert!(
        !empty_catch,
        "Non-empty .catch() should not trigger, got: {:?}",
        findings
    );
}

// ── ERR-008: Discarded error in Go ──────────────────────────────

#[test]
fn test_discarded_error_go() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "main.go",
        r#"
func main() {
    result, err := doSomething()
    _ = err
    fmt.Println(result)
}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Discarded error"));
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn test_non_supported_file_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "styles.css", "catch (e) {}\n");

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Non-supported files (.css) should be skipped"
    );
}

#[test]
fn test_comment_lines_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "app.js",
        r#"
// catch (e) {}
/* .unwrap() */
* panic!("test")
# except:
function clean() {}
"#,
    );

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Comment lines should be skipped, got: {:?}",
        findings
    );
}

#[test]
fn test_config_disabled_by_default() {
    let analyzer = ErrorHandlingAnalyzer::new();
    let config = RevetConfig::default();
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_config_enabled() {
    let analyzer = ErrorHandlingAnalyzer::new();
    let config = error_config();
    assert!(analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_via_dispatcher() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "bad.rs",
        r#"
fn main() {
    let x = foo().unwrap();
    let y = bar().unwrap();
    panic!("oops");
}
"#,
    );

    let config = error_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    let err_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("ERR-"))
        .collect();
    assert!(
        err_findings.len() >= 3,
        "Expected at least 3 ERR findings, got: {:?}",
        err_findings
    );
    assert_eq!(err_findings[0].id, "ERR-001");
    assert_eq!(err_findings[1].id, "ERR-002");
    assert_eq!(err_findings[2].id, "ERR-003");
}

#[test]
fn test_multi_language_file_detection() {
    let dir = TempDir::new().unwrap();
    let py = write_temp_file(&dir, "app.py", "except:\n    pass\n");
    let rs = write_temp_file(&dir, "lib.rs", "let x = val.unwrap();\n");
    let go = write_temp_file(&dir, "main.go", "_ = err\n");

    let analyzer = ErrorHandlingAnalyzer::new();
    let findings = analyzer.analyze_files(&[py, rs, go], dir.path());

    assert_eq!(
        findings.len(),
        3,
        "Expected one finding per language file, got: {:?}",
        findings
    );
}
