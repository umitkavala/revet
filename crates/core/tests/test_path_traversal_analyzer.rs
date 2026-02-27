//! Integration tests for PathTraversalAnalyzer

use revet_core::analyzer::path_traversal::PathTraversalAnalyzer;
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

fn analyzer() -> PathTraversalAnalyzer {
    PathTraversalAnalyzer::new()
}

// ── Python: open() ────────────────────────────────────────────────────────────

#[test]
fn test_python_open_fstring() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "handler.py",
        r#"with open(f"/data/{filename}") as f:"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("f-string path"));
}

#[test]
fn test_python_open_dotdot_concat() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "handler.py", r#"f = open(base_dir + "../secret")"#);
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("'../'"));
}

#[test]
fn test_python_open_hardcoded_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "handler.py",
        r#"with open("config/settings.toml") as f:
    data = f.read()
"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded path must not be flagged; got: {findings:?}"
    );
}

// ── Python: os.path.join ──────────────────────────────────────────────────────

#[test]
fn test_python_os_path_join_variable_first_arg() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "serve.py",
        "full_path = os.path.join(user_dir, filename)\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("os.path.join()"));
}

#[test]
fn test_python_os_path_join_hardcoded_base_no_finding() {
    let dir = TempDir::new().unwrap();
    // First arg is a string literal — base is fixed, less risky
    let file = write_temp_file(
        &dir,
        "serve.py",
        r#"full_path = os.path.join("/var/data", filename)"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded base path must not be flagged; got: {findings:?}"
    );
}

// ── Python: pathlib.Path ──────────────────────────────────────────────────────

#[test]
fn test_python_pathlib_fstring() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "serve.py", r#"p = Path(f"/uploads/{user_file}")"#);
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("pathlib.Path()"));
}

// ── JavaScript / TypeScript ───────────────────────────────────────────────────

#[test]
fn test_js_fs_readfile_template_literal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.js",
        "fs.readFile(`/uploads/${req.params.name}`, callback);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("template literal"));
}

#[test]
fn test_js_fs_writefile_template_literal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "upload.ts",
        "fs.writeFileSync(`/tmp/${filename}`, data);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_js_fs_readfile_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.js",
        "fs.readFile(filePath, 'utf8', callback);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_js_fs_readfile_hardcoded_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.js",
        r#"fs.readFile("./config/settings.json", "utf8", callback);"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded path must not be flagged; got: {findings:?}"
    );
}

#[test]
fn test_js_path_join_template_literal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "serve.js",
        "const full = path.join(base, `user/${req.query.file}`);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("path.join()"));
}

#[test]
fn test_js_path_join_dotdot() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "serve.js",
        r#"const p = path.join(root, '../', userInput);"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

// ── PHP ───────────────────────────────────────────────────────────────────────

#[test]
fn test_php_include_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "page.php", "include($page);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("include/require"));
}

#[test]
fn test_php_require_once_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "router.php", "require_once($module_path);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_php_file_get_contents_superglobal() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "read.php",
        "$content = file_get_contents($_GET['file']);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("superglobal"));
}

#[test]
fn test_php_file_get_contents_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "read.php",
        "$content = file_get_contents($filepath);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

// ── Go ────────────────────────────────────────────────────────────────────────

#[test]
fn test_go_os_open_sprintf() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "handler.go",
        r#"f, err := os.Open(fmt.Sprintf("/data/%s", filename))"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("fmt.Sprintf"));
}

#[test]
fn test_go_os_open_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "handler.go", "f, err := os.Open(filePath)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_go_os_open_hardcoded_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "handler.go",
        r#"f, err := os.Open("config/app.yaml")"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "hardcoded path must not be flagged; got: {findings:?}"
    );
}

// ── Java ──────────────────────────────────────────────────────────────────────

#[test]
fn test_java_new_file_concat() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "FileHandler.java",
        r#"File f = new File("/uploads/" + fileName);"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("concatenation"));
}

#[test]
fn test_java_paths_get_variable() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "FileHandler.java", "Path p = Paths.get(userPath);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

// ── Cross-cutting ─────────────────────────────────────────────────────────────

#[test]
fn test_binary_files_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "img.png", "open(f\"/data/{name}\")\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "binary files must be skipped");
}

#[test]
fn test_python_patterns_not_fired_on_php_file() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "page.php", "# open(f\"/data/{name}\")\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "Python pattern must not fire on PHP files"
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
    let file = write_temp_file(
        &dir,
        "handler.py",
        r#"with open(f"/data/{os.path.join(base, name)}") as f:"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "only one finding per line; got: {findings:?}"
    );
}
