//! Integration tests for InsecureDeserializationAnalyzer

use revet_core::analyzer::insecure_deserialization::InsecureDeserializationAnalyzer;
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

fn analyzer() -> InsecureDeserializationAnalyzer {
    InsecureDeserializationAnalyzer::new()
}

// ── Python: yaml ──────────────────────────────────────────────────────────────

#[test]
fn test_python_yaml_load_bare() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "config.py", "data = yaml.load(stream)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("yaml.load()"));
}

#[test]
fn test_python_yaml_load_unsafe_loader() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "config.py",
        "data = yaml.load(stream, Loader=yaml.Loader)\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_python_yaml_safe_load_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "config.py", "data = yaml.safe_load(stream)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "yaml.safe_load() must not be flagged; got: {findings:?}"
    );
}

#[test]
fn test_python_yaml_load_safe_loader_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "config.py",
        "data = yaml.load(stream, Loader=yaml.SafeLoader)\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "yaml.load with SafeLoader must not be flagged; got: {findings:?}"
    );
}

// ── Python: pickle ────────────────────────────────────────────────────────────

#[test]
fn test_python_pickle_load() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "model.py", "model = pickle.load(f)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("pickle.load()"));
}

#[test]
fn test_python_pickle_loads() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "decode.py", "obj = pickle.loads(data)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
}

#[test]
fn test_python_cpickle_load() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "legacy.py", "obj = cPickle.load(f)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("cPickle"));
}

#[test]
fn test_python_marshal_loads() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "run.py", "code = marshal.loads(raw_bytes)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("marshal"));
}

#[test]
fn test_python_jsonpickle_decode() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "api.py", "obj = jsonpickle.decode(request.body)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("jsonpickle"));
}

#[test]
fn test_python_safe_json_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.py",
        r#"import json
data = json.loads(request.body)
config = json.load(open("config.json"))
"#,
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "json.loads/load must not be flagged; got: {findings:?}"
    );
}

// ── PHP ───────────────────────────────────────────────────────────────────────

#[test]
fn test_php_unserialize() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "user.php", "$user = unserialize($_COOKIE['user']);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("unserialize()"));
}

#[test]
fn test_php_safe_json_decode_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "api.php", "$data = json_decode($input, true);\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "json_decode must not be flagged; got: {findings:?}"
    );
}

// ── Java ──────────────────────────────────────────────────────────────────────

#[test]
fn test_java_object_input_stream() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Deserializer.java",
        "ObjectInputStream ois = new ObjectInputStream(socket.getInputStream());\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("ObjectInputStream"));
}

#[test]
fn test_java_safe_json_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Api.java",
        "MyDto dto = objectMapper.readValue(json, MyDto.class);\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "Jackson readValue must not be flagged; got: {findings:?}"
    );
}

// ── Ruby ──────────────────────────────────────────────────────────────────────

#[test]
fn test_ruby_marshal_load() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "session.rb", "obj = Marshal.load(data)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("Marshal.load()"));
}

#[test]
fn test_ruby_yaml_load() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "config.rb",
        "config = YAML.load(File.read('app.yml'))\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("YAML.load()"));
}

#[test]
fn test_ruby_yaml_safe_load_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "config.rb",
        "config = YAML.safe_load(File.read('app.yml'))\n",
    );
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "YAML.safe_load must not be flagged; got: {findings:?}"
    );
}

// ── Cross-cutting ─────────────────────────────────────────────────────────────

#[test]
fn test_binary_files_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "data.png", "pickle.load(f)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(findings.is_empty(), "binary files must be skipped");
}

#[test]
fn test_python_patterns_not_fired_on_java_file() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "Util.java", "// pickle.load(f)\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert!(
        findings.is_empty(),
        "Python pattern must not fire on Java files"
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
    // A contrived line that matches multiple patterns — only one finding expected
    let file = write_temp_file(&dir, "weird.py", "x = pickle.load(marshal.loads(data))\n");
    let findings = analyzer().analyze_files(&[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "only one finding per line; got: {findings:?}"
    );
}
