//! Integration tests for HardcodedEndpointsAnalyzer

use revet_core::analyzer::hardcoded_endpoints::HardcodedEndpointsAnalyzer;
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

fn analyzer() -> HardcodedEndpointsAnalyzer {
    HardcodedEndpointsAnalyzer::new()
}

// ── Private IPs (RFC 1918) ────────────────────────────────────────

#[test]
fn test_class_a_private_ip() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "config.py", "DB_HOST = '10.0.1.25'\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("10.x.x.x"));
}

#[test]
fn test_class_a_private_ip_in_go() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "client.go", "addr := \"10.128.4.200:8080\"\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

#[test]
fn test_class_c_private_ip() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "deploy.sh", "SERVER=192.168.1.100\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("192.168.x.x"));
}

#[test]
fn test_class_b_private_ip_172_16() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "infra.tf", "host = \"172.16.0.5\"\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("172.16-31.x.x"));
}

#[test]
fn test_class_b_private_ip_172_31() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "infra.tf", "host = \"172.31.255.1\"\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

#[test]
fn test_class_b_outside_range_no_finding() {
    let dir = TempDir::new().unwrap();
    // 172.32.x.x is public — should not be flagged by the private IP pattern
    // (may be caught by other patterns if in URL form, but not as private IP)
    let file = write_temp_file(&dir, "config.py", "host = '172.32.0.1'\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "172.32.x.x is not a private range; got: {findings:?}"
    );
}

// ── IP address in URL ─────────────────────────────────────────────

#[test]
fn test_ip_in_http_url() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "client.py", "BASE_URL = 'http://203.0.113.5/api'\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("IP address in URL"));
}

#[test]
fn test_ip_in_https_url() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.ts",
        "const endpoint = 'https://198.51.100.42/v1/data';\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

// ── Production URLs ───────────────────────────────────────────────

#[test]
fn test_prod_subdomain_url() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "config.js",
        "const API = 'https://api.prod.example.com';\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("production URL"));
}

#[test]
fn test_production_subdomain_url() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.py",
        "BASE = 'https://production.myapp.io/api'\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("production URL"));
}

// ── Staging URLs ──────────────────────────────────────────────────

#[test]
fn test_staging_subdomain_url() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "env.ts",
        "export const API_URL = 'https://staging.api.example.com';\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("staging URL"));
}

#[test]
fn test_stage_subdomain_url() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "config.go",
        "baseURL := \"https://stage.internal.co/graphql\"\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
}

// ── No-finding cases ──────────────────────────────────────────────

#[test]
fn test_localhost_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "server.py", "HOST = '127.0.0.1'\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "localhost must not be flagged; got: {findings:?}"
    );
}

#[test]
fn test_public_https_no_ip_no_prod_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.py",
        "BASE_URL = 'https://api.example.com/v1'\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "clean named URL must not be flagged; got: {findings:?}"
    );
}

#[test]
fn test_version_number_not_ip() {
    let dir = TempDir::new().unwrap();
    // 1.2.3.4 style but not in a private range
    let file = write_temp_file(&dir, "version.py", "VERSION = '1.2.3.4'\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "version number must not be flagged; got: {findings:?}"
    );
}

#[test]
fn test_productionready_url_no_finding() {
    let dir = TempDir::new().unwrap();
    // 'production' must be a whole subdomain component, not a substring
    let file = write_temp_file(
        &dir,
        "links.py",
        "url = 'https://productionready.io/docs'\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "'productionready.io' must not be flagged; got: {findings:?}"
    );
}

// ── Cross-cutting ─────────────────────────────────────────────────

#[test]
fn test_binary_files_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "logo.png", "DB_HOST = '10.0.0.1'\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "binary files must be skipped");
}

#[test]
fn test_one_finding_per_line() {
    let dir = TempDir::new().unwrap();
    // Line has both a private IP and a prod URL — only first match wins
    let file = write_temp_file(
        &dir,
        "config.py",
        "url = 'http://10.0.0.1' # also https://prod.example.com\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1, "one finding per line; got: {findings:?}");
}

#[test]
fn test_disabled_by_default() {
    let config = RevetConfig::default();
    assert!(
        !analyzer().is_enabled(&config),
        "hardcoded_endpoints must be off by default"
    );
}

#[test]
fn test_enabled_via_config() {
    let mut config = RevetConfig::default();
    config.modules.hardcoded_endpoints = true;
    assert!(analyzer().is_enabled(&config));
}
