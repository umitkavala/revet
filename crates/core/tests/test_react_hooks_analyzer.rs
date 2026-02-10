//! Integration tests for ReactHooksAnalyzer

use revet_core::analyzer::react_hooks::ReactHooksAnalyzer;
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

fn react_config() -> RevetConfig {
    let mut config = RevetConfig::default();
    config.modules.react = true;
    config
}

// ── Error: Rules of Hooks violations ────────────────────────────

#[test]
fn test_hook_inside_condition() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "App.tsx",
        r#"
function App() {
  if (isLoggedIn) useState(0);
  return <div />;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("Hook inside condition"));
}

#[test]
fn test_hook_inside_loop() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "List.tsx",
        r#"
function List({ items }) {
  for (let i = 0; i < items.length; i++) useEffect(() => {});
  return <ul />;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("Hook inside loop"));
}

// ── Warning: Common anti-patterns ───────────────────────────────

#[test]
fn test_useeffect_no_deps() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Timer.tsx",
        r#"
function Timer() {
  useEffect(() => {
    console.log("runs every render");
  });
  return <div />;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0]
        .message
        .contains("useEffect without dependency array"));
}

#[test]
fn test_useeffect_with_deps_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Counter.tsx",
        r#"
function Counter({ count }) {
  useEffect(() => { document.title = count; }, [count]);
  return <div>{count}</div>;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // useEffect with deps should not trigger "no deps" warning
    // But document.title is not document.getElementById, so no DOM finding either
    assert!(
        findings.is_empty(),
        "Expected no findings, got: {:?}",
        findings
    );
}

#[test]
fn test_direct_dom_manipulation() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Modal.tsx",
        r#"
function Modal() {
  const el = document.getElementById("modal-root");
  return <div />;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Direct DOM manipulation"));
}

#[test]
fn test_missing_key_in_map() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "List.jsx",
        r#"
function List({ items }) {
  return items.map(item => <Item name={item.name} />);
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Missing key prop"));
}

#[test]
fn test_map_with_key_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "List.jsx",
        r#"
function List({ items }) {
  return items.map(item => <Item key={item.id} name={item.name} />);
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Map with key= should not trigger finding, got: {:?}",
        findings
    );
}

#[test]
fn test_dangerously_set_inner_html() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Render.tsx",
        r#"
function Render({ html }) {
  return <div dangerouslySetInnerHTML={{ __html: html }} />;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("dangerouslySetInnerHTML"));
}

// ── Info: Performance hints ─────────────────────────────────────

#[test]
fn test_inline_handler() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Button.tsx",
        r#"
function Button() {
  return <button onClick={() => doStuff()}>Click</button>;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("Inline function"));
}

#[test]
fn test_useeffect_empty_deps() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Init.tsx",
        r#"
function Init() {
  useEffect(() => { fetchData(); }, []);
  return <div />;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("empty dependency array"));
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn test_non_react_file_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "utils.py", "if (x) useState(0)\n");

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Non-React files (.py) should be skipped"
    );
}

#[test]
fn test_comment_lines_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "App.tsx",
        r#"
// if (x) useState(0)
/* dangerouslySetInnerHTML */
* useEffect(() => {
{/* document.getElementById("foo") */}
function App() {
  return <div />;
}
"#,
    );

    let analyzer = ReactHooksAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Comment lines should be skipped, got: {:?}",
        findings
    );
}

#[test]
fn test_respects_config_disabled() {
    let analyzer = ReactHooksAnalyzer::new();
    let config = RevetConfig::default(); // react defaults to false
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_respects_config_enabled() {
    let analyzer = ReactHooksAnalyzer::new();
    let config = react_config();
    assert!(analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_via_dispatcher() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "App.tsx",
        r#"
function App() {
  if (ready) useState(0);
  for (let i = 0; i < 3; i++) useEffect(() => {});
  return <div dangerouslySetInnerHTML={{ __html: "" }} />;
}
"#,
    );

    let config = react_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    // Should have HOOKS-prefixed findings
    let hooks_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("HOOKS-"))
        .collect();
    assert!(
        hooks_findings.len() >= 3,
        "Expected at least 3 HOOKS findings, got: {:?}",
        hooks_findings
    );
    assert_eq!(hooks_findings[0].id, "HOOKS-001");
    assert_eq!(hooks_findings[1].id, "HOOKS-002");
    assert_eq!(hooks_findings[2].id, "HOOKS-003");
}
