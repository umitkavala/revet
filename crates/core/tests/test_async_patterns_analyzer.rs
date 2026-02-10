//! Integration tests for AsyncPatternsAnalyzer

use revet_core::analyzer::async_patterns::AsyncPatternsAnalyzer;
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

fn async_config() -> RevetConfig {
    let mut config = RevetConfig::default();
    config.modules.async_patterns = true;
    config
}

// ── Error: Always wrong ─────────────────────────────────────────

#[test]
fn test_async_promise_executor() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.ts",
        r#"
const result = new Promise(async (resolve, reject) => {
  const data = await fetch("/api");
  resolve(data);
});
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("Async Promise executor"));
}

#[test]
fn test_await_in_foreach() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "process.js",
        r#"
items.forEach(async (item) => {
  await processItem(item);
});
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("Await in forEach"));
}

// ── Warning: Usually problematic ────────────────────────────────

#[test]
fn test_unhandled_then() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "fetch.ts",
        r#"
fetch("/api/data").then(response => response.json());
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Unhandled .then() chain"));
}

#[test]
fn test_then_with_catch_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "fetch.ts",
        r#"
fetch("/api/data").then(r => r.json()).catch(err => console.error(err));
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        ".then() with .catch() should not trigger, got: {:?}",
        findings
    );
}

#[test]
fn test_async_map_without_promise_all() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "batch.ts",
        r#"
const results = items.map(async (item) => {
  const val = await transform(item);
  return val;
});
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0]
        .message
        .contains("Async map without Promise.all"));
}

#[test]
fn test_async_map_with_promise_all_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "batch.ts",
        r#"
const results = await Promise.all(items.map(async (item) => transform(item)));
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        ".map(async) with Promise.all should not trigger, got: {:?}",
        findings
    );
}

#[test]
fn test_async_timer_callback() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "timer.js",
        r#"
setTimeout(async () => {
  await doWork();
}, 1000);
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Async timer callback"));
}

#[test]
fn test_floating_python_coroutine() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.py",
        r#"
async def handler():
    asyncio.sleep(1)
    return "done"
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Floating Python coroutine"));
}

#[test]
fn test_python_coroutine_with_await_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "server.py",
        r#"
async def handler():
    await asyncio.sleep(1)
    return "done"
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "asyncio with await should not trigger, got: {:?}",
        findings
    );
}

// ── Info: Code smell / style ────────────────────────────────────

#[test]
fn test_swallowed_catch() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.ts",
        r#"
fetch("/api").then(r => r.json()).catch(() => {});
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // The line contains .catch so .then() won't trigger, but swallowed catch will
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("Swallowed error in catch"));
}

#[test]
fn test_redundant_return_await() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "service.ts",
        r#"
async function getData() {
  return await fetchData();
}
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("Redundant return await"));
}

// ── Edge cases ──────────────────────────────────────────────────

#[test]
fn test_non_async_file_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "main.rs",
        "new Promise(async (resolve) => resolve());\n",
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Non-async-relevant files (.rs) should be skipped"
    );
}

#[test]
fn test_comment_lines_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "api.ts",
        r#"
// new Promise(async (resolve) => resolve());
/* items.forEach(async (x) => await x); */
* .then(something)
# asyncio.sleep(1)
function clean() {}
"#,
    );

    let analyzer = AsyncPatternsAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Comment lines should be skipped, got: {:?}",
        findings
    );
}

#[test]
fn test_config_disabled_by_default() {
    let analyzer = AsyncPatternsAnalyzer::new();
    let config = RevetConfig::default();
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_config_enabled() {
    let analyzer = AsyncPatternsAnalyzer::new();
    let config = async_config();
    assert!(analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_via_dispatcher() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "bad.ts",
        r#"
new Promise(async (resolve) => resolve());
items.forEach(async (item) => { await item; });
fetch("/api").then(r => r.json());
"#,
    );

    let config = async_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    let async_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("ASYNC-"))
        .collect();
    assert!(
        async_findings.len() >= 3,
        "Expected at least 3 ASYNC findings, got: {:?}",
        async_findings
    );
    assert_eq!(async_findings[0].id, "ASYNC-001");
    assert_eq!(async_findings[1].id, "ASYNC-002");
    assert_eq!(async_findings[2].id, "ASYNC-003");
}
