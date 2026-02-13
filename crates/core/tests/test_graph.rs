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

// ── Merge tests ─────────────────────────────────────────────────

#[test]
fn test_merge_basic() {
    let mut a = CodeGraph::new(PathBuf::from("/root"));
    a.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("a.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let mut b = CodeGraph::new(PathBuf::from("/root"));
    b.add_node(Node::new(
        NodeKind::Function,
        "func_b".to_string(),
        PathBuf::from("b.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let map = a.merge(b);
    assert_eq!(map.len(), 1);
    assert_eq!(a.nodes().count(), 2);
}

#[test]
fn test_merge_edges_integrity() {
    let mut main = CodeGraph::new(PathBuf::from("/root"));

    let mut sub = CodeGraph::new(PathBuf::from("/root"));
    let s1 = sub.add_node(Node::new(
        NodeKind::Function,
        "caller".to_string(),
        PathBuf::from("x.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));
    let s2 = sub.add_node(Node::new(
        NodeKind::Function,
        "callee".to_string(),
        PathBuf::from("x.py"),
        10,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));
    sub.add_edge(s1, s2, Edge::new(EdgeKind::Calls));

    let map = main.merge(sub);

    // Verify the edge was remapped correctly
    let new_caller = map[&s1];
    let edges: Vec<_> = main.edges_from(new_caller).collect();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].0, map[&s2]);

    // Query should work on merged graph
    let query = main.query();
    let deps = query.direct_dependents(map[&s2]);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0], map[&s1]);
}
