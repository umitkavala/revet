//! Integration tests for SecretExposureAnalyzer

use revet_core::analyzer::secret_exposure::SecretExposureAnalyzer;
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

// ── Detection tests ──────────────────────────────────────────────

#[test]
fn test_detects_aws_key() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "config.py", "AWS_KEY = 'AKIAIOSFODNN7EXAMPLE'\n");

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("AWS Access Key ID"));
    assert_eq!(findings[0].line, 1);
}

#[test]
fn test_detects_github_token() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deploy.sh",
        "TOKEN=ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijkl\n",
    );

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("GitHub Token"));
}

#[test]
fn test_detects_private_key() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "key.pem",
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQ...\n-----END RSA PRIVATE KEY-----\n",
    );

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("Private Key"));
}

#[test]
fn test_detects_password() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "settings.py",
        r#"DB_PASSWORD = "real_secret_password_123"
"#,
    );

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Password"));
}

#[test]
fn test_detects_connection_string() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "db.py",
        "DATABASE_URL = 'postgres://admin:supersecret@db.prod.com:5432/app'\n",
    );

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("Connection String"));
}

#[test]
fn test_no_false_positive_clean_code() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "clean.py",
        r#"
import os

def get_config():
    """Load configuration from environment."""
    api_key = os.environ.get("API_KEY")
    password = os.environ.get("DB_PASSWORD")
    return {"api_key": api_key, "password": password}

class DatabaseClient:
    def __init__(self, host, port):
        self.host = host
        self.port = port
"#,
    );

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Expected no findings, got: {:?}",
        findings
    );
}

#[test]
fn test_skips_binary_files() {
    let dir = TempDir::new().unwrap();
    // Even if binary file somehow contains a pattern, it should be skipped
    let file = write_temp_file(&dir, "logo.png", "AKIAIOSFODNN7EXAMPLE\n");

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(findings.is_empty(), "Binary files should be skipped");
}

#[test]
fn test_respects_config_disabled() {
    let dir = TempDir::new().unwrap();
    let _file = write_temp_file(&dir, "secrets.py", "AWS_KEY = 'AKIAIOSFODNN7EXAMPLE'\n");

    let mut config = default_config();
    config.modules.security = false;

    let analyzer = SecretExposureAnalyzer::new();
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_sequential() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "multi.py",
        r#"AWS_KEY = 'AKIAIOSFODNN7EXAMPLE'
TOKEN = 'ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijkl'
DB = 'postgres://user:pass@host/db'
"#,
    );

    let config = default_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    assert_eq!(findings.len(), 3);
    assert_eq!(findings[0].id, "SEC-001");
    assert_eq!(findings[1].id, "SEC-002");
    assert_eq!(findings[2].id, "SEC-003");
}

#[test]
fn test_one_finding_per_line() {
    let dir = TempDir::new().unwrap();
    // This line matches both AWS key and generic API key patterns
    let file = write_temp_file(&dir, "overlap.py", "api_key = 'AKIAIOSFODNN7EXAMPLE'\n");

    let analyzer = SecretExposureAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // AWS key pattern is checked first and wins
    assert_eq!(
        findings.len(),
        1,
        "Should produce exactly one finding per line"
    );
    assert!(findings[0].message.contains("AWS Access Key ID"));
}
