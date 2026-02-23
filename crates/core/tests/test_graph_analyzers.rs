//! Integration tests for graph-based analyzers (GraphAnalyzer trait)

use revet_core::config::RevetConfig;
use revet_core::graph::{CodeGraph, Edge, EdgeKind, Node, NodeData, NodeKind};
use revet_core::AnalyzerDispatcher;
use std::path::PathBuf;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn config_with(dead_code: bool, cycles: bool) -> RevetConfig {
    let mut cfg = RevetConfig::default();
    cfg.modules.dead_code = dead_code;
    cfg.modules.cycles = cycles;
    cfg
}

fn add_file_node(graph: &mut CodeGraph, path: &str) -> revet_core::graph::NodeId {
    graph.add_node(Node::new(
        NodeKind::File,
        path.to_string(),
        PathBuf::from(path),
        0,
        NodeData::File {
            language: "python".to_string(),
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

// ── UnusedExportsAnalyzer tests ───────────────────────────────────────────────

#[test]
fn test_unused_export_detected() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_id = add_file_node(&mut graph, "src/utils.py");
    let func_id = add_function_node(&mut graph, "helper", "src/utils.py", 1);

    // File contains the function (top-level)
    graph.add_edge(file_id, func_id, Edge::new(EdgeKind::Contains));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(true, false);
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    assert!(
        findings
            .iter()
            .any(|f| f.id.starts_with("DEAD") && f.message.contains("helper")),
        "Expected DEAD finding for unused `helper`, got: {:?}",
        findings
    );
}

#[test]
fn test_used_export_not_flagged() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_a = add_file_node(&mut graph, "src/utils.py");
    let func_id = add_function_node(&mut graph, "helper", "src/utils.py", 1);
    graph.add_edge(file_a, func_id, Edge::new(EdgeKind::Contains));

    // Another node calls the function
    let file_b = add_file_node(&mut graph, "src/main.py");
    let caller_id = add_function_node(&mut graph, "run", "src/main.py", 1);
    graph.add_edge(file_b, caller_id, Edge::new(EdgeKind::Contains));
    graph.add_edge(caller_id, func_id, Edge::new(EdgeKind::Calls));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(true, false);
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    let dead_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("DEAD") && f.message.contains("helper"))
        .collect();
    assert!(
        dead_findings.is_empty(),
        "Did not expect DEAD finding for `helper` (it is called)"
    );
}

#[test]
fn test_entry_point_main_not_flagged() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_id = add_file_node(&mut graph, "src/app.py");
    let main_id = add_function_node(&mut graph, "main", "src/app.py", 1);
    graph.add_edge(file_id, main_id, Edge::new(EdgeKind::Contains));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(true, false);
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    let dead_main: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("DEAD") && f.message.contains("main"))
        .collect();
    assert!(dead_main.is_empty(), "Should not flag `main` as unused");
}

#[test]
fn test_dead_code_disabled_no_findings() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_id = add_file_node(&mut graph, "src/utils.py");
    let func_id = add_function_node(&mut graph, "helper", "src/utils.py", 1);
    graph.add_edge(file_id, func_id, Edge::new(EdgeKind::Contains));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(false, false); // dead_code = false
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    let dead: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("DEAD"))
        .collect();
    assert!(dead.is_empty(), "No DEAD findings when dead_code=false");
}

// ── CircularImportsAnalyzer tests ─────────────────────────────────────────────

#[test]
fn test_cycle_two_files_detected() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_a = add_file_node(&mut graph, "src/a.py");
    let file_b = add_file_node(&mut graph, "src/b.py");

    // A imports B, B imports A
    graph.add_edge(file_a, file_b, Edge::new(EdgeKind::Imports));
    graph.add_edge(file_b, file_a, Edge::new(EdgeKind::Imports));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(false, true);
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    let cycle_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("CYCLE"))
        .collect();
    assert!(
        !cycle_findings.is_empty(),
        "Expected CYCLE finding for A↔B, got: {:?}",
        findings
    );
}

#[test]
fn test_no_cycle_no_finding() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_a = add_file_node(&mut graph, "src/a.py");
    let file_b = add_file_node(&mut graph, "src/b.py");
    let file_c = add_file_node(&mut graph, "src/c.py");

    // A→B→C (no cycle)
    graph.add_edge(file_a, file_b, Edge::new(EdgeKind::Imports));
    graph.add_edge(file_b, file_c, Edge::new(EdgeKind::Imports));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(false, true);
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    let cycle_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("CYCLE"))
        .collect();
    assert!(cycle_findings.is_empty(), "No cycle in A→B→C");
}

#[test]
fn test_three_file_cycle_detected() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_a = add_file_node(&mut graph, "src/a.py");
    let file_b = add_file_node(&mut graph, "src/b.py");
    let file_c = add_file_node(&mut graph, "src/c.py");

    // A→B→C→A
    graph.add_edge(file_a, file_b, Edge::new(EdgeKind::Imports));
    graph.add_edge(file_b, file_c, Edge::new(EdgeKind::Imports));
    graph.add_edge(file_c, file_a, Edge::new(EdgeKind::Imports));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(false, true);
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    let cycle_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("CYCLE"))
        .collect();
    assert_eq!(
        cycle_findings.len(),
        1,
        "Expected exactly 1 CYCLE finding for A→B→C→A, got: {:?}",
        findings
    );
}

#[test]
fn test_cycles_disabled_no_findings() {
    let mut graph = CodeGraph::new(PathBuf::from("."));
    let file_a = add_file_node(&mut graph, "src/a.py");
    let file_b = add_file_node(&mut graph, "src/b.py");
    graph.add_edge(file_a, file_b, Edge::new(EdgeKind::Imports));
    graph.add_edge(file_b, file_a, Edge::new(EdgeKind::Imports));

    let dispatcher = AnalyzerDispatcher::new();
    let config = config_with(false, false); // cycles = false
    let findings = dispatcher.run_graph_analyzers(&graph, &config);

    let cycle_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("CYCLE"))
        .collect();
    assert!(
        cycle_findings.is_empty(),
        "No CYCLE findings when cycles=false"
    );
}
