//! Integration tests for SensitiveLoggingAnalyzer

use revet_core::analyzer::sensitive_logging::SensitiveLoggingAnalyzer;
use revet_core::analyzer::Analyzer;
use revet_core::config::RevetConfig;
use revet_core::finding::Severity;
use std::path::PathBuf;
use tempfile::TempDir;

fn write_temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

fn analyzer() -> SensitiveLoggingAnalyzer {
    SensitiveLoggingAnalyzer::new()
}

// ── Python logging ────────────────────────────────────────────────

#[test]
fn test_python_logging_debug_password() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "auth.py", "logging.debug(password)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Python logging call"));
}

#[test]
fn test_python_logging_info_token() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "auth.py", "logging.info(f\"token={token}\")\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_python_logging_error_secret() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "auth.py", "logger.error(\"secret: %s\", secret)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

#[test]
fn test_python_print_api_key() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "debug.py", "print(api_key)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("print()"));
}

#[test]
fn test_python_logging_no_sensitive_name_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.py",
        "logging.info(\"Server started on port %d\", port)\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "Non-sensitive variable must not be flagged; got: {findings:?}"
    );
}

#[test]
fn test_python_print_username_no_finding() {
    let dir = TempDir::new().unwrap();
    // 'username' is not in the sensitive names list
    let file = write_temp_file(&dir, "auth.py", "print(username)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "username is not sensitive; got: {findings:?}"
    );
}

// ── JavaScript / TypeScript ───────────────────────────────────────

#[test]
fn test_js_console_log_password() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "auth.js", "console.log(password);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("console.*"));
}

#[test]
fn test_ts_console_error_token() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "auth.ts",
        "console.error(`Auth failed, token=${token}`);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

#[test]
fn test_js_logger_info_secret() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.js", "logger.info({ secret });\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("logger.*"));
}

#[test]
fn test_js_console_log_no_sensitive_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.js",
        "console.log(\"Request received\", req.url);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "got: {findings:?}");
}

// ── Go ────────────────────────────────────────────────────────────

#[test]
fn test_go_fmt_println_password() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "auth.go", "fmt.Println(\"debug:\", password)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("fmt.Print"));
}

#[test]
fn test_go_log_printf_api_key() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "debug.go", "log.Printf(\"apiKey=%s\", apiKey)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

#[test]
fn test_go_fmt_no_sensitive_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "main.go", "fmt.Println(\"Starting server...\")\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "got: {findings:?}");
}

// ── Java ──────────────────────────────────────────────────────────

#[test]
fn test_java_system_out_password() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Auth.java",
        "System.out.println(\"pwd: \" + password);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_java_logger_debug_token() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "Service.java", "logger.debug(\"token={}\", token);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

// ── PHP ───────────────────────────────────────────────────────────

#[test]
fn test_php_error_log_password() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "auth.php", "error_log($password);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("error_log"));
}

#[test]
fn test_php_var_dump_token() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "debug.php", "var_dump($token);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

// ── Ruby ──────────────────────────────────────────────────────────

#[test]
fn test_ruby_puts_password() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "auth.rb", "puts password\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("puts"));
}

#[test]
fn test_ruby_p_secret() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "debug.rb", "p secret\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

// ── Cross-cutting ─────────────────────────────────────────────────

#[test]
fn test_binary_files_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "img.png", "console.log(password)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "binary files must be skipped");
}

#[test]
fn test_python_pattern_not_fired_on_js_file() {
    let dir = TempDir::new().unwrap();
    // Python logging pattern should not fire on a .js file
    let file = write_temp_file(&dir, "app.js", "logging.debug(password)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    // The JS logger pattern won't match `logging.debug` either (expects `logger.`)
    // Only the console.* and logger.* patterns apply to .js
    assert!(findings.is_empty(), "got: {findings:?}");
}

#[test]
fn test_respects_security_module_disabled() {
    let mut config = RevetConfig::default();
    config.modules.security = false;
    assert!(!analyzer().is_enabled(&config));
}

#[test]
fn test_one_finding_per_line() {
    let dir = TempDir::new().unwrap();
    // Line matches both logging + print patterns — only first wins
    let file = write_temp_file(&dir, "auth.py", "logging.debug(password); print(secret)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1, "one finding per line; got: {findings:?}");
}
