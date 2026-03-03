use revet_core::analyzer::test_coverage::TestCoverageAnalyzer;
use revet_core::analyzer::GraphAnalyzer;
use revet_core::config::RevetConfig;
use revet_core::graph::{CodeGraph, Edge, EdgeKind, Node, NodeData, NodeKind};
use std::io::Write;
use std::path::PathBuf;
use tempfile::{tempdir, NamedTempFile};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn enabled_config() -> RevetConfig {
    let mut cfg = RevetConfig::default();
    cfg.modules.test_coverage = true;
    cfg
}

fn add_file_node(graph: &mut CodeGraph, path: &str) -> revet_core::graph::NodeId {
    graph.add_node(Node::new(
        NodeKind::File,
        path.to_string(),
        PathBuf::from(path),
        0,
        NodeData::File {
            language: "rust".to_string(),
        },
    ))
}

fn add_function_node(
    graph: &mut CodeGraph,
    name: &str,
    file: &str,
    line: usize,
) -> revet_core::graph::NodeId {
    graph.add_node(Node::new(
        NodeKind::Function,
        name.to_string(),
        PathBuf::from(file),
        line,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ))
}

fn add_class_node(graph: &mut CodeGraph, name: &str, file: &str) -> revet_core::graph::NodeId {
    graph.add_node(Node::new(
        NodeKind::Class,
        name.to_string(),
        PathBuf::from(file),
        1,
        NodeData::Class {
            base_classes: vec![],
            methods: vec![],
            fields: vec![],
        },
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_untested_function_flagged() {
    let dir = tempdir().unwrap();

    // Create a test file that does NOT mention the function
    let test_path = dir.path().join("tests/test_other.rs");
    std::fs::create_dir_all(test_path.parent().unwrap()).unwrap();
    std::fs::write(&test_path, "fn test_something() { assert!(true); }").unwrap();

    let src_path = dir.path().join("src/lib.rs");
    std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();

    let mut graph = CodeGraph::new(dir.path().to_path_buf());

    let test_file_id = add_file_node(&mut graph, &test_path.to_string_lossy());
    let src_file_id = add_file_node(&mut graph, &src_path.to_string_lossy());
    let func_id = add_function_node(&mut graph, "compute_hash", &src_path.to_string_lossy(), 5);
    graph.add_edge(src_file_id, func_id, Edge::new(EdgeKind::Contains));
    let _ = test_file_id;

    let analyzer = TestCoverageAnalyzer::new();
    let findings = analyzer.analyze_graph(&graph, &enabled_config());

    assert!(
        findings.iter().any(|f| f.message.contains("compute_hash")),
        "should flag untested function"
    );
}

#[test]
fn test_tested_function_not_flagged() {
    let dir = tempdir().unwrap();

    // Create a test file that DOES mention the function
    let test_path = dir.path().join("tests/test_lib.rs");
    std::fs::create_dir_all(test_path.parent().unwrap()).unwrap();
    std::fs::write(
        &test_path,
        "fn test_compute_hash() { let _ = compute_hash(b\"data\"); }",
    )
    .unwrap();

    let src_path = dir.path().join("src/lib.rs");
    std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();

    let mut graph = CodeGraph::new(dir.path().to_path_buf());
    let _test_id = add_file_node(&mut graph, &test_path.to_string_lossy());
    let src_file_id = add_file_node(&mut graph, &src_path.to_string_lossy());
    let func_id = add_function_node(&mut graph, "compute_hash", &src_path.to_string_lossy(), 5);
    graph.add_edge(src_file_id, func_id, Edge::new(EdgeKind::Contains));

    let analyzer = TestCoverageAnalyzer::new();
    let findings = analyzer.analyze_graph(&graph, &enabled_config());

    assert!(
        findings.iter().all(|f| !f.message.contains("compute_hash")),
        "tested function should not be flagged"
    );
}

#[test]
fn test_private_function_skipped() {
    let dir = tempdir().unwrap();

    let test_path = dir.path().join("tests/test_lib.rs");
    std::fs::create_dir_all(test_path.parent().unwrap()).unwrap();
    std::fs::write(&test_path, "fn test_pub() {}").unwrap();

    let src_path = dir.path().join("src/lib.rs");
    std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();

    let mut graph = CodeGraph::new(dir.path().to_path_buf());
    let _test_id = add_file_node(&mut graph, &test_path.to_string_lossy());
    let src_file_id = add_file_node(&mut graph, &src_path.to_string_lossy());
    // Private-by-convention: leading underscore
    let func_id = add_function_node(
        &mut graph,
        "_internal_helper",
        &src_path.to_string_lossy(),
        10,
    );
    graph.add_edge(src_file_id, func_id, Edge::new(EdgeKind::Contains));

    let analyzer = TestCoverageAnalyzer::new();
    let findings = analyzer.analyze_graph(&graph, &enabled_config());

    assert!(
        findings
            .iter()
            .all(|f| !f.message.contains("_internal_helper")),
        "private functions should not be flagged"
    );
}

#[test]
fn test_no_test_files_produces_no_findings() {
    let dir = tempdir().unwrap();
    let src_path = dir.path().join("src/lib.rs");
    std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();

    let mut graph = CodeGraph::new(dir.path().to_path_buf());
    let src_file_id = add_file_node(&mut graph, &src_path.to_string_lossy());
    let func_id = add_function_node(&mut graph, "untested_fn", &src_path.to_string_lossy(), 1);
    graph.add_edge(src_file_id, func_id, Edge::new(EdgeKind::Contains));

    let analyzer = TestCoverageAnalyzer::new();
    let findings = analyzer.analyze_graph(&graph, &enabled_config());

    assert!(
        findings.is_empty(),
        "no test files — cannot determine coverage, should produce no findings"
    );
}

#[test]
fn test_class_without_tests_flagged() {
    let dir = tempdir().unwrap();

    let test_path = dir.path().join("tests/test_other.rs");
    std::fs::create_dir_all(test_path.parent().unwrap()).unwrap();
    std::fs::write(&test_path, "fn test_unrelated() {}").unwrap();

    let src_path = dir.path().join("src/models.rs");
    std::fs::create_dir_all(src_path.parent().unwrap()).unwrap();

    let mut graph = CodeGraph::new(dir.path().to_path_buf());
    let _test_id = add_file_node(&mut graph, &test_path.to_string_lossy());
    let src_file_id = add_file_node(&mut graph, &src_path.to_string_lossy());
    let class_id = add_class_node(&mut graph, "UserAccount", &src_path.to_string_lossy());
    graph.add_edge(src_file_id, class_id, Edge::new(EdgeKind::Contains));

    let analyzer = TestCoverageAnalyzer::new();
    let findings = analyzer.analyze_graph(&graph, &enabled_config());

    assert!(
        findings.iter().any(|f| f.message.contains("UserAccount")),
        "should flag class with no test mention"
    );
}

#[test]
fn test_disabled_by_default() {
    let mut graph = CodeGraph::new(PathBuf::from("/tmp"));
    let config = RevetConfig::default(); // test_coverage = false
    let analyzer = TestCoverageAnalyzer::new();
    assert!(!analyzer.is_enabled(&config));
    let findings = analyzer.analyze_graph(&graph, &config);
    // Even with nodes present, disabled analyzer returns nothing useful
    let _ = findings;
}

#[test]
fn test_symbols_in_test_files_not_flagged() {
    let dir = tempdir().unwrap();

    let test_path = dir.path().join("tests/test_utils.rs");
    std::fs::create_dir_all(test_path.parent().unwrap()).unwrap();
    std::fs::write(&test_path, "fn helper_in_test() {}").unwrap();

    let mut graph = CodeGraph::new(dir.path().to_path_buf());
    let test_file_id = add_file_node(&mut graph, &test_path.to_string_lossy());
    // Function defined inside the test file itself
    let func_id = add_function_node(
        &mut graph,
        "helper_in_test",
        &test_path.to_string_lossy(),
        1,
    );
    graph.add_edge(test_file_id, func_id, Edge::new(EdgeKind::Contains));

    let analyzer = TestCoverageAnalyzer::new();
    let findings = analyzer.analyze_graph(&graph, &enabled_config());

    assert!(
        findings
            .iter()
            .all(|f| !f.message.contains("helper_in_test")),
        "functions defined in test files should not be flagged"
    );
}
