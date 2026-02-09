//! Integration tests for GraphStore implementations
//!
//! Tests run against MemoryStore (always) and CozoStore (when cozo-store feature enabled).

use std::path::PathBuf;

use revet_core::graph::{Edge, EdgeKind, EdgeMetadata, Node, NodeData, NodeKind, Parameter};
use revet_core::store::{reconstruct_graph, GraphStore, MemoryStore, StoreNodeId};
use revet_core::CodeGraph;

#[cfg(feature = "cozo-store")]
use revet_core::store::CozoStore;

/// Create all store implementations to test against
fn create_stores() -> Vec<(&'static str, Box<dyn GraphStore>)> {
    #[allow(unused_mut)]
    let mut stores: Vec<(&'static str, Box<dyn GraphStore>)> =
        vec![("memory", Box::new(MemoryStore::new()))];

    #[cfg(feature = "cozo-store")]
    stores.push(("cozo", Box::new(CozoStore::new_memory().unwrap())));

    stores
}

/// Build a sample 3-node graph: A --Calls--> B --Calls--> C
fn build_sample_graph() -> CodeGraph {
    let mut graph = CodeGraph::new(PathBuf::from("/repo"));

    let a = graph.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("src/a.py"),
        10,
        NodeData::Function {
            parameters: vec![Parameter {
                name: "x".to_string(),
                param_type: Some("int".to_string()),
                default_value: None,
            }],
            return_type: Some("str".to_string()),
        },
    ));

    let b = graph.add_node(Node::new(
        NodeKind::Function,
        "func_b".to_string(),
        PathBuf::from("src/b.py"),
        20,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let c = graph.add_node(Node::new(
        NodeKind::Class,
        "MyClass".to_string(),
        PathBuf::from("src/c.py"),
        1,
        NodeData::Class {
            base_classes: vec!["Base".to_string()],
            methods: vec!["run".to_string()],
            fields: vec!["name".to_string()],
        },
    ));

    graph.add_edge(a, b, Edge::new(EdgeKind::Calls));
    graph.add_edge(
        b,
        c,
        Edge::with_metadata(
            EdgeKind::Calls,
            EdgeMetadata::Call {
                line: 25,
                is_direct: true,
            },
        ),
    );

    graph
}

#[test]
fn test_flush_and_node_count() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();
        let count = store.node_count("v1").unwrap();
        assert_eq!(count, 3, "[{name}] expected 3 nodes, got {count}");
    }
}

#[test]
fn test_flush_and_retrieve_node() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // Node 0 should be func_a
        let node = store.node(StoreNodeId(0), "v1").unwrap();
        assert!(node.is_some(), "[{name}] node 0 should exist");
        let node = node.unwrap();
        assert_eq!(node.name(), "func_a", "[{name}]");
        assert_eq!(*node.kind(), NodeKind::Function, "[{name}]");
        assert_eq!(node.file_path(), &PathBuf::from("src/a.py"), "[{name}]");
        assert_eq!(node.line(), 10, "[{name}]");

        match node.data() {
            NodeData::Function {
                parameters,
                return_type,
            } => {
                assert_eq!(parameters.len(), 1, "[{name}]");
                assert_eq!(parameters[0].name, "x", "[{name}]");
                assert_eq!(return_type, &Some("str".to_string()), "[{name}]");
            }
            other => panic!("[{name}] expected Function data, got {other:?}"),
        }
    }
}

#[test]
fn test_nodes_returns_all() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let all_nodes = store.nodes("v1").unwrap();
        assert_eq!(all_nodes.len(), 3, "[{name}]");

        let names: Vec<_> = all_nodes
            .iter()
            .map(|(_, n)| n.name().to_string())
            .collect();
        assert!(names.contains(&"func_a".to_string()), "[{name}]");
        assert!(names.contains(&"func_b".to_string()), "[{name}]");
        assert!(names.contains(&"MyClass".to_string()), "[{name}]");
    }
}

#[test]
fn test_find_nodes_by_path() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let found = store.find_nodes("src/a.py", None, "v1").unwrap();
        assert_eq!(found.len(), 1, "[{name}]");
        assert_eq!(found[0].1.name(), "func_a", "[{name}]");
    }
}

#[test]
fn test_find_nodes_by_path_and_name() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let found = store.find_nodes("src/b.py", Some("func_b"), "v1").unwrap();
        assert_eq!(found.len(), 1, "[{name}]");
        assert_eq!(found[0].1.name(), "func_b", "[{name}]");

        // No match
        let empty = store
            .find_nodes("src/b.py", Some("nonexist"), "v1")
            .unwrap();
        assert!(empty.is_empty(), "[{name}]");
    }
}

#[test]
fn test_find_nodes_by_kind() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let funcs = store.find_nodes_by_kind(NodeKind::Function, "v1").unwrap();
        assert_eq!(funcs.len(), 2, "[{name}] expected 2 functions");

        let classes = store.find_nodes_by_kind(NodeKind::Class, "v1").unwrap();
        assert_eq!(classes.len(), 1, "[{name}] expected 1 class");
        assert_eq!(classes[0].1.name(), "MyClass", "[{name}]");
    }
}

#[test]
fn test_edges_from() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // func_a (id=0) has one outgoing edge to func_b (id=1)
        let edges = store.edges_from(StoreNodeId(0), "v1").unwrap();
        assert_eq!(edges.len(), 1, "[{name}]");
        assert_eq!(edges[0].to, StoreNodeId(1), "[{name}]");
        assert_eq!(*edges[0].edge.kind(), EdgeKind::Calls, "[{name}]");
    }
}

#[test]
fn test_edges_to() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // func_b (id=1) has one incoming edge from func_a (id=0)
        let edges = store.edges_to(StoreNodeId(1), "v1").unwrap();
        assert_eq!(edges.len(), 1, "[{name}]");
        assert_eq!(edges[0].from, StoreNodeId(0), "[{name}]");
    }
}

#[test]
fn test_edges_with_metadata() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // Edge from func_b (1) -> MyClass (2) has Call metadata
        let edges = store.edges_from(StoreNodeId(1), "v1").unwrap();
        assert_eq!(edges.len(), 1, "[{name}]");
        match edges[0].edge.metadata() {
            Some(EdgeMetadata::Call { line, is_direct }) => {
                assert_eq!(*line, 25, "[{name}]");
                assert!(*is_direct, "[{name}]");
            }
            other => panic!("[{name}] expected Call metadata, got {other:?}"),
        }
    }
}

#[test]
fn test_direct_dependents() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // func_b (id=1) has one direct dependent: func_a (id=0)
        let deps = store.direct_dependents(StoreNodeId(1), "v1").unwrap();
        assert_eq!(deps.len(), 1, "[{name}]");
        assert_eq!(deps[0], StoreNodeId(0), "[{name}]");

        // func_a (id=0) has no dependents
        let deps = store.direct_dependents(StoreNodeId(0), "v1").unwrap();
        assert!(deps.is_empty(), "[{name}]");
    }
}

#[test]
fn test_transitive_dependents() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // MyClass (id=2) is called by func_b, which is called by func_a
        // Transitive dependents of MyClass: {func_b, func_a}
        let deps = store
            .transitive_dependents(StoreNodeId(2), None, "v1")
            .unwrap();
        assert_eq!(deps.len(), 2, "[{name}] expected 2 transitive dependents");
        let dep_set: std::collections::HashSet<_> = deps.into_iter().collect();
        assert!(
            dep_set.contains(&StoreNodeId(0)),
            "[{name}] should contain func_a"
        );
        assert!(
            dep_set.contains(&StoreNodeId(1)),
            "[{name}] should contain func_b"
        );
    }
}

#[test]
fn test_transitive_dependents_with_depth() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // With max_depth=1, only direct dependents of MyClass (id=2): {func_b}
        let deps = store
            .transitive_dependents(StoreNodeId(2), Some(1), "v1")
            .unwrap();
        assert_eq!(deps.len(), 1, "[{name}] expected 1 dependent at depth 1");
        assert_eq!(deps[0], StoreNodeId(1), "[{name}] should be func_b");
    }
}

#[test]
fn test_dependencies() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // func_a (id=0) depends on func_b (id=1)
        let deps = store.dependencies(StoreNodeId(0), "v1").unwrap();
        assert_eq!(deps.len(), 1, "[{name}]");
        assert_eq!(deps[0], StoreNodeId(1), "[{name}]");
    }
}

#[test]
fn test_transitive_dependencies() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        // func_a depends on func_b, which depends on MyClass
        let deps = store
            .transitive_dependencies(StoreNodeId(0), None, "v1")
            .unwrap();
        assert_eq!(deps.len(), 2, "[{name}]");
        let dep_set: std::collections::HashSet<_> = deps.into_iter().collect();
        assert!(dep_set.contains(&StoreNodeId(1)), "[{name}]");
        assert!(dep_set.contains(&StoreNodeId(2)), "[{name}]");
    }
}

#[test]
fn test_find_by_edge_kind() {
    // Build a graph with mixed edge kinds
    let mut graph = CodeGraph::new(PathBuf::from("/repo"));

    let a = graph.add_node(Node::new(
        NodeKind::Function,
        "caller".to_string(),
        PathBuf::from("main.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let b = graph.add_node(Node::new(
        NodeKind::Function,
        "callee".to_string(),
        PathBuf::from("lib.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let c = graph.add_node(Node::new(
        NodeKind::Import,
        "os".to_string(),
        PathBuf::from("main.py"),
        1,
        NodeData::Import {
            module: "os".to_string(),
            imported_names: vec!["path".to_string()],
        },
    ));

    graph.add_edge(a, b, Edge::new(EdgeKind::Calls));
    graph.add_edge(a, c, Edge::new(EdgeKind::Imports));

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let calls = store
            .find_by_edge_kind(StoreNodeId(0), EdgeKind::Calls, "v1")
            .unwrap();
        assert_eq!(calls.len(), 1, "[{name}]");
        assert_eq!(calls[0], StoreNodeId(1), "[{name}]");

        let imports = store
            .find_by_edge_kind(StoreNodeId(0), EdgeKind::Imports, "v1")
            .unwrap();
        assert_eq!(imports.len(), 1, "[{name}]");
        assert_eq!(imports[0], StoreNodeId(2), "[{name}]");
    }
}

#[test]
fn test_find_changed_nodes_modified() {
    let mut old_graph = CodeGraph::new(PathBuf::from("/repo"));
    old_graph.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("a.py"),
        10,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let mut new_graph = CodeGraph::new(PathBuf::from("/repo"));
    new_graph.add_node(Node::new(
        NodeKind::Function,
        "func_a".to_string(),
        PathBuf::from("a.py"),
        10,
        NodeData::Function {
            parameters: vec![Parameter {
                name: "x".to_string(),
                param_type: None,
                default_value: None,
            }],
            return_type: None,
        },
    ));

    for (name, store) in create_stores() {
        store.flush(&old_graph, "old").unwrap();
        store.flush(&new_graph, "new").unwrap();

        let changed = store.find_changed_nodes("old", "new").unwrap();
        assert_eq!(changed.len(), 1, "[{name}] expected 1 changed node");
        // old_id should be Some (modified, not added)
        assert!(
            changed[0].1.is_some(),
            "[{name}] should be modified, not added"
        );
    }
}

#[test]
fn test_find_changed_nodes_added() {
    let old_graph = CodeGraph::new(PathBuf::from("/repo"));

    let mut new_graph = CodeGraph::new(PathBuf::from("/repo"));
    new_graph.add_node(Node::new(
        NodeKind::Function,
        "new_func".to_string(),
        PathBuf::from("new.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    for (name, store) in create_stores() {
        store.flush(&old_graph, "old").unwrap();
        store.flush(&new_graph, "new").unwrap();

        let changed = store.find_changed_nodes("old", "new").unwrap();
        assert_eq!(changed.len(), 1, "[{name}] expected 1 added node");
        assert!(changed[0].1.is_none(), "[{name}] should be None (added)");
    }
}

#[test]
fn test_snapshots_lifecycle() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        // Initially no snapshots
        let snaps = store.snapshots().unwrap();
        assert!(snaps.is_empty(), "[{name}] should start empty");

        // Flush two snapshots
        store.flush(&graph, "v1").unwrap();
        store.flush(&graph, "v2").unwrap();

        let snaps = store.snapshots().unwrap();
        assert_eq!(snaps.len(), 2, "[{name}]");
        let names: Vec<_> = snaps.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"v1"), "[{name}]");
        assert!(names.contains(&"v2"), "[{name}]");
        assert_eq!(snaps[0].node_count, 3, "[{name}]");

        // Delete one
        store.delete_snapshot("v1").unwrap();
        let snaps = store.snapshots().unwrap();
        assert_eq!(snaps.len(), 1, "[{name}]");
        assert_eq!(snaps[0].name, "v2", "[{name}]");
    }
}

#[test]
fn test_multiple_edges_same_pair() {
    let mut graph = CodeGraph::new(PathBuf::from("/repo"));

    let a = graph.add_node(Node::new(
        NodeKind::Function,
        "f".to_string(),
        PathBuf::from("a.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    let b = graph.add_node(Node::new(
        NodeKind::Function,
        "g".to_string(),
        PathBuf::from("b.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    // Two edges between same pair with different kinds
    graph.add_edge(a, b, Edge::new(EdgeKind::Calls));
    graph.add_edge(a, b, Edge::new(EdgeKind::References));

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let edges = store.edges_from(StoreNodeId(0), "v1").unwrap();
        assert_eq!(
            edges.len(),
            2,
            "[{name}] expected 2 edges between same pair"
        );

        let kinds: Vec<_> = edges.iter().map(|e| *e.edge.kind()).collect();
        assert!(kinds.contains(&EdgeKind::Calls), "[{name}]");
        assert!(kinds.contains(&EdgeKind::References), "[{name}]");
    }
}

#[test]
fn test_reconstruct_graph_round_trip() {
    let graph = build_sample_graph();

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let reconstructed = reconstruct_graph(&*store, "v1", &PathBuf::from("/repo")).unwrap();

        // Same node count
        let orig_count: usize = graph.nodes().count();
        let recon_count: usize = reconstructed.nodes().count();
        assert_eq!(recon_count, orig_count, "[{name}] node count mismatch");

        // Verify node names and kinds match
        let mut orig_names: Vec<_> = graph.nodes().map(|(_, n)| n.name().to_string()).collect();
        let mut recon_names: Vec<_> = reconstructed
            .nodes()
            .map(|(_, n)| n.name().to_string())
            .collect();
        orig_names.sort();
        recon_names.sort();
        assert_eq!(recon_names, orig_names, "[{name}] node names mismatch");

        // Verify edges: func_a -> func_b, func_b -> MyClass
        let edges_from_a: Vec<_> = reconstructed
            .edges_from(petgraph::graph::NodeIndex::new(0))
            .collect();
        assert_eq!(
            edges_from_a.len(),
            1,
            "[{name}] func_a should have 1 outgoing edge"
        );
    }
}

#[test]
fn test_reconstruct_graph_preserves_decorators() {
    let mut graph = CodeGraph::new(PathBuf::from("/repo"));

    let mut node = Node::new(
        NodeKind::Function,
        "decorated_func".to_string(),
        PathBuf::from("app.py"),
        5,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    );
    node.set_decorators(vec![
        "@app.route".to_string(),
        "@login_required".to_string(),
    ]);
    graph.add_node(node);

    for (name, store) in create_stores() {
        store.flush(&graph, "v1").unwrap();

        let reconstructed = reconstruct_graph(&*store, "v1", &PathBuf::from("/repo")).unwrap();

        let (_, recon_node) = reconstructed.nodes().next().unwrap();
        assert_eq!(
            recon_node.decorators(),
            &["@app.route", "@login_required"],
            "[{name}] decorators should round-trip"
        );
    }
}

#[test]
fn test_reconstruct_empty_graph() {
    let graph = CodeGraph::new(PathBuf::from("/repo"));

    for (name, store) in create_stores() {
        store.flush(&graph, "empty").unwrap();

        let reconstructed = reconstruct_graph(&*store, "empty", &PathBuf::from("/repo")).unwrap();

        let count: usize = reconstructed.nodes().count();
        assert_eq!(count, 0, "[{name}] empty graph should have 0 nodes");
    }
}
