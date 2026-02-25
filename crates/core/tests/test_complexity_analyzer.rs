//! Integration tests for the ComplexityAnalyzer.
//!
//! Most checks require real file content (the analyzer reads function bodies),
//! so tests write temporary source files and point graph nodes at them.

use revet_core::config::RevetConfig;
use revet_core::graph::{CodeGraph, Node, NodeData, NodeKind, Parameter};
use revet_core::AnalyzerDispatcher;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn config_complexity() -> RevetConfig {
    let mut cfg = RevetConfig::default();
    cfg.modules.complexity = true;
    cfg.modules.cycles = false;
    cfg
}

fn add_fn_node(
    graph: &mut CodeGraph,
    name: &str,
    file: &str,
    line: usize,
    end_line: usize,
    params: usize,
) -> revet_core::graph::NodeId {
    let mut node = Node::new(
        NodeKind::Function,
        name.to_string(),
        PathBuf::from(file),
        line,
        NodeData::Function {
            parameters: (0..params)
                .map(|i| Parameter {
                    name: format!("p{i}"),
                    param_type: None,
                    default_value: None,
                })
                .collect(),
            return_type: None,
        },
    );
    node.set_end_line(end_line);
    graph.add_node(node)
}

/// Write source content to a NamedTempFile and return it.
/// The caller must keep the file alive for the duration of the test.
fn write_temp_src(content: &str, suffix: &str) -> NamedTempFile {
    let mut f = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("temp file");
    f.write_all(content.as_bytes()).expect("write temp file");
    f
}

// ── Function length tests ─────────────────────────────────────────────────────

#[test]
fn test_short_function_no_finding() {
    let src = "fn short() {\n    let x = 1;\n}\n";
    let tmp = write_temp_src(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "short", &path, 1, 3, 0);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    assert!(
        findings.is_empty(),
        "Short function should have no findings, got: {findings:?}"
    );
}

#[test]
fn test_long_function_warning() {
    // Build a function that is exactly FN_LEN_WARN (50) lines
    let mut lines: Vec<&str> = vec!["fn long_fn() {"];
    let body: Vec<String> = (0..49).map(|i| format!("    let _{i} = {i};")).collect();
    for l in &body {
        lines.push(l.as_str());
    }
    lines.push("}");
    let src = lines.join("\n");

    let tmp = write_temp_src(&src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();
    let total = lines.len();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "long_fn", &path, 1, total, 0);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("long_fn") && f.message.contains("lines")),
        "Expected length warning for long_fn, got: {findings:?}"
    );
}

#[test]
fn test_very_long_function_error() {
    // FN_LEN_ERROR = 100 lines
    let mut lines: Vec<String> = vec!["fn huge_fn() {".to_string()];
    for i in 0..100 {
        lines.push(format!("    let _{i} = {i};"));
    }
    lines.push("}".to_string());
    let src = lines.join("\n");

    let tmp = write_temp_src(&src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();
    let total = lines.len();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "huge_fn", &path, 1, total, 0);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    assert!(
        findings.iter().any(|f| {
            f.message.contains("huge_fn")
                && f.message.contains("lines")
                && f.severity == revet_core::finding::Severity::Error
        }),
        "Expected length Error for huge_fn, got: {findings:?}"
    );
}

// ── Parameter count tests ─────────────────────────────────────────────────────

#[test]
fn test_few_params_no_finding() {
    let src = "fn ok(a: i32, b: i32) -> i32 { a + b }\n";
    let tmp = write_temp_src(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "ok", &path, 1, 1, 2);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    assert!(
        !findings.iter().any(|f| f.message.contains("parameters")),
        "2 params should have no parameter finding"
    );
}

#[test]
fn test_many_params_warning() {
    // PARAM_WARN = 5
    let src = "fn wide(a: i32, b: i32, c: i32, d: i32, e: i32) {}\n";
    let tmp = write_temp_src(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "wide", &path, 1, 1, 5);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("wide") && f.message.contains("parameters")),
        "5 params should trigger parameter warning"
    );
}

#[test]
fn test_too_many_params_error() {
    // PARAM_ERROR = 8
    let src = "fn bloated(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32, h: i32) {}\n";
    let tmp = write_temp_src(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "bloated", &path, 1, 1, 8);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    assert!(
        findings.iter().any(|f| {
            f.message.contains("bloated")
                && f.message.contains("parameters")
                && f.severity == revet_core::finding::Severity::Error
        }),
        "8 params should trigger parameter Error"
    );
}

// ── Cyclomatic complexity tests ───────────────────────────────────────────────

#[test]
fn test_high_complexity_warning() {
    // Build a function with >10 branches (COMPLEXITY_WARN)
    let src = r#"fn complex(x: i32) -> i32 {
    if x > 0 {
        if x > 1 {
            if x > 2 {
                if x > 3 {
                    if x > 4 {
                        if x > 5 {
                            if x > 6 {
                                if x > 7 {
                                    if x > 8 {
                                        return x;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    0
}
"#;
    let tmp = write_temp_src(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();
    let line_count = src.lines().count();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "complex", &path, 1, line_count, 1);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    let has_complexity_finding = findings
        .iter()
        .any(|f| f.message.contains("complex") && f.message.contains("complexity"));

    assert!(
        has_complexity_finding,
        "Deeply nested function should have complexity finding, got: {findings:?}"
    );
}

// ── Nesting depth tests ───────────────────────────────────────────────────────

#[test]
fn test_deep_nesting_warning() {
    // Build a function with nesting depth >= NESTING_WARN (4)
    let src = r#"fn deeply_nested(x: bool) {
    if x {
        if x {
            if x {
                if x {
                    println!("deep");
                }
            }
        }
    }
}
"#;
    let tmp = write_temp_src(src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();
    let line_count = src.lines().count();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "deeply_nested", &path, 1, line_count, 1);

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &config_complexity());

    let has_nesting = findings
        .iter()
        .any(|f| f.message.contains("deeply_nested") && f.message.contains("nesting"));

    assert!(
        has_nesting,
        "Deeply nested function should have nesting finding, got: {findings:?}"
    );
}

// ── Disabled module test ──────────────────────────────────────────────────────

#[test]
fn test_disabled_produces_no_findings() {
    // Build a huge function but with complexity disabled
    let mut lines: Vec<String> = vec!["fn huge() {".to_string()];
    for i in 0..200 {
        lines.push(format!("    let _{i} = {i};"));
    }
    lines.push("}".to_string());
    let src = lines.join("\n");

    let tmp = write_temp_src(&src, ".rs");
    let path = tmp.path().to_str().unwrap().to_string();
    let total = lines.len();

    let mut graph = CodeGraph::new(PathBuf::from("."));
    add_fn_node(&mut graph, "huge", &path, 1, total, 9); // 9 params, huge length

    let mut cfg = RevetConfig::default();
    cfg.modules.complexity = false; // disabled
    cfg.modules.cycles = false;
    cfg.modules.dead_code = false;

    let findings = AnalyzerDispatcher::new().run_graph_analyzers(&graph, &cfg);

    assert!(
        findings.is_empty(),
        "Disabled complexity module should produce no findings"
    );
}
