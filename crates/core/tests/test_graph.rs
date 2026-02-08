//! Tests for graph data structures (nodes, edges, queries)

use revet_core::graph::{
    CodeGraph, Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeKind, Parameter,
};
use std::path::PathBuf;

// ── Node tests ─────────────────────────────────────────────────

#[test]
fn test_create_function_node() {
    let node = Node::new(
        NodeKind::Function,
        "test_func".to_string(),
        PathBuf::from("test.py"),
        10,
        NodeData::Function {
            parameters: vec![Parameter {
                name: "x".to_string(),
                param_type: Some("int".to_string()),
                default_value: None,
            }],
            return_type: Some("str".to_string()),
        },
    );

    assert_eq!(node.name(), "test_func");
    assert_eq!(node.line(), 10);
    assert_eq!(node.kind(), &NodeKind::Function);
}

// ── Edge tests ─────────────────────────────────────────────────

#[test]
fn test_create_edge() {
    let edge = Edge::new(EdgeKind::Calls);
    assert_eq!(edge.kind(), &EdgeKind::Calls);
    assert!(edge.metadata().is_none());
}

#[test]
fn test_edge_with_metadata() {
    let edge = Edge::with_metadata(
        EdgeKind::Calls,
        EdgeMetadata::Call {
            line: 42,
            is_direct: true,
        },
    );
    assert!(edge.metadata().is_some());
}

// ── CodeGraph tests ────────────────────────────────────────────

#[test]
fn test_create_graph() {
    let graph = CodeGraph::new(PathBuf::from("/test"));
    assert_eq!(graph.root_path(), &PathBuf::from("/test"));
}

#[test]
fn test_add_node() {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));
    let node = Node::new(
        NodeKind::Function,
        "test_func".to_string(),
        PathBuf::from("/test/file.py"),
        10,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    );
    let id = graph.add_node(node);
    assert!(graph.node(id).is_some());
}

// ── Query tests ────────────────────────────────────────────────

#[test]
fn test_direct_dependents() {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));

    let node_a = graph.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("a.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let node_b = graph.add_node(Node::new(
        NodeKind::Function,
        "func_b".to_string(),
        PathBuf::from("b.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    // B calls A
    graph.add_edge(node_b, node_a, Edge::new(EdgeKind::Calls));

    let query = graph.query();
    let dependents = query.direct_dependents(node_a);

    assert_eq!(dependents.len(), 1);
    assert_eq!(dependents[0], node_b);
}

#[test]
fn test_transitive_dependents() {
    let mut graph = CodeGraph::new(PathBuf::from("/test"));

    let node_a = graph.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("a.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let node_b = graph.add_node(Node::new(
        NodeKind::Function,
        "func_b".to_string(),
        PathBuf::from("b.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let node_c = graph.add_node(Node::new(
        NodeKind::Function,
        "func_c".to_string(),
        PathBuf::from("c.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    // B calls A, C calls B
    graph.add_edge(node_b, node_a, Edge::new(EdgeKind::Calls));
    graph.add_edge(node_c, node_b, Edge::new(EdgeKind::Calls));

    let query = graph.query();
    let dependents = query.transitive_dependents(node_a, None);

    assert_eq!(dependents.len(), 2);
    assert!(dependents.contains(&node_b));
    assert!(dependents.contains(&node_c));
}
