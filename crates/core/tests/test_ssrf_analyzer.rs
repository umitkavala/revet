//! Integration tests for SsrfAnalyzer

use revet_core::analyzer::ssrf::SsrfAnalyzer;
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

fn analyzer() -> SsrfAnalyzer {
    SsrfAnalyzer::new()
}

// ── Python: requests ──────────────────────────────────────────────────────────

#[test]
fn test_python_requests_get_fstring() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.py",
        r#"resp = requests.get(f"https://api.example.com/users/{user_id}")"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("f-string"));
}

#[test]
fn test_python_requests_post_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.py",
        "resp = requests.post(target_url, json=payload)\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("variable URL"));
}

#[test]
fn test_python_requests_get_attr_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.py",
        "resp = requests.get(self.base_url, timeout=5)\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_python_requests_hardcoded_url_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.py",
        r#"resp = requests.get("https://api.example.com/data", timeout=5)"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded URL must not be flagged; got: {findings:?}"
    );
}

#[test]
fn test_python_requests_session_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "client.py", "session = requests.Session()\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "requests.Session() must not be flagged; got: {findings:?}"
    );
}

// ── Python: urllib ────────────────────────────────────────────────────────────

#[test]
fn test_python_urllib_urlopen_fstring() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "fetch.py",
        r#"response = urllib.request.urlopen(f"http://internal/{path}")"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_python_urllib_urlopen_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "fetch.py", "response = urllib.request.urlopen(url)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_python_urllib_hardcoded_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "fetch.py",
        r#"response = urllib.request.urlopen("https://api.example.com/")"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded URL must not be flagged; got: {findings:?}"
    );
}

// ── Python: httpx ─────────────────────────────────────────────────────────────

#[test]
fn test_python_httpx_get_fstring() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.py",
        r#"r = httpx.get(f"http://service/{endpoint}")"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_python_httpx_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "client.py", "r = httpx.post(target_url)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

// ── JavaScript / TypeScript: fetch ───────────────────────────────────────────

#[test]
fn test_js_fetch_template_literal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.js",
        "const res = await fetch(`http://internal/${path}`);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("template literal"));
}

#[test]
fn test_js_fetch_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.ts",
        "const response = await fetch(targetUrl, { method: 'GET' });\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_js_fetch_hardcoded_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.js",
        r#"const res = await fetch("https://api.example.com/v1/data");"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded URL must not be flagged; got: {findings:?}"
    );
}

// ── JavaScript / TypeScript: axios ────────────────────────────────────────────

#[test]
fn test_js_axios_template_literal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "service.ts",
        "const res = await axios.get(`http://service/${id}`);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_js_axios_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "service.ts",
        "const res = await axios.post(endpoint, body);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

// ── Go ────────────────────────────────────────────────────────────────────────

#[test]
fn test_go_http_get_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "client.go", "resp, err := http.Get(targetURL)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_go_http_get_sprintf() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.go",
        r#"resp, err := http.Get(fmt.Sprintf("http://internal/%s", path))"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_go_http_get_hardcoded_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "client.go",
        r#"resp, err := http.Get("https://api.example.com/health")"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded URL must not be flagged; got: {findings:?}"
    );
}

// ── Java ──────────────────────────────────────────────────────────────────────

#[test]
fn test_java_new_url_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "Client.java", "URL url = new URL(targetUrl);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_java_new_url_concat() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Client.java",
        r#"URL url = new URL("https://api.internal/" + userPath);"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

// ── Cross-cutting ─────────────────────────────────────────────────────────────

#[test]
fn test_binary_files_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "icon.png", "requests.get(url)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "binary files must be skipped");
}

#[test]
fn test_python_patterns_not_fired_on_go_file() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "main.go", "// requests.get(url)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "Python pattern must not fire on Go files"
    );
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
    // Line that could match both f-string and variable patterns — only one finding
    let file = write_temp_file(&dir, "client.py", r#"r = requests.get(f"{base_url}/api")"#);
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "only one finding per line; got: {findings:?}"
    );
}
