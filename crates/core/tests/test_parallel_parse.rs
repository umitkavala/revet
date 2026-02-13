//! Tests for parallel parsing and analysis

use revet_core::graph::{CodeGraph, Edge, EdgeKind, Node, NodeData, NodeKind};
use revet_core::{AnalyzerDispatcher, ParserDispatcher, RevetConfig};
use std::collections::HashSet;
use std::path::PathBuf;
use tempfile::TempDir;

// ── CodeGraph::merge() tests ────────────────────────────────────

#[test]
fn test_merge_empty_into_empty() {
    let mut a = CodeGraph::new(PathBuf::from("/root"));
    let b = CodeGraph::new(PathBuf::from("/root"));
    let map = a.merge(b);
    assert!(map.is_empty());
    assert_eq!(a.nodes().count(), 0);
}

#[test]
fn test_merge_nodes_are_added() {
    let mut a = CodeGraph::new(PathBuf::from("/root"));
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
    b.add_node(Node::new(
        NodeKind::Class,
        "ClassB".to_string(),
        PathBuf::from("b.py"),
        10,
        NodeData::Class {
            base_classes: vec![],
            methods: vec![],
            fields: vec![],
        },
    ));

    let map = a.merge(b);
    assert_eq!(map.len(), 2);
    assert_eq!(a.nodes().count(), 2);
}

#[test]
fn test_merge_edges_are_remapped() {
    let mut a = CodeGraph::new(PathBuf::from("/root"));
    let mut b = CodeGraph::new(PathBuf::from("/root"));

    let b_func = b.add_node(Node::new(
        NodeKind::Function,
        "func_b".to_string(),
        PathBuf::from("b.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));
    let b_class = b.add_node(Node::new(
        NodeKind::Class,
        "ClassB".to_string(),
        PathBuf::from("b.py"),
        10,
        NodeData::Class {
            base_classes: vec![],
            methods: vec![],
            fields: vec![],
        },
    ));
    b.add_edge(b_func, b_class, Edge::new(EdgeKind::Calls));

    let map = a.merge(b);

    // Verify remapped edge exists
    let new_func = map[&b_func];
    let edges: Vec<_> = a.edges_from(new_func).collect();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].0, map[&b_class]);
    assert_eq!(edges[0].1.kind(), &EdgeKind::Calls);
}

#[test]
fn test_merge_preserves_existing_nodes() {
    let mut a = CodeGraph::new(PathBuf::from("/root"));
    let a_id = a.add_node(Node::new(
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

    a.merge(b);

    // Original node still accessible
    assert!(a.node(a_id).is_some());
    assert_eq!(a.node(a_id).unwrap().name(), "func_a");
    // Total: 1 original + 1 merged
    assert_eq!(a.nodes().count(), 2);
}

#[test]
fn test_merge_name_index_works_after_merge() {
    let mut a = CodeGraph::new(PathBuf::from("/root"));
    let mut b = CodeGraph::new(PathBuf::from("/root"));

    b.add_node(Node::new(
        NodeKind::Function,
        "find_me".to_string(),
        PathBuf::from("target.py"),
        42,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    a.merge(b);

    let found = a.find_nodes(&PathBuf::from("target.py"), Some("find_me"));
    assert_eq!(found.len(), 1);
    assert_eq!(a.node(found[0]).unwrap().name(), "find_me");
}

#[test]
fn test_merge_multiple_graphs() {
    let mut main = CodeGraph::new(PathBuf::from("/root"));

    for i in 0..5 {
        let mut sub = CodeGraph::new(PathBuf::from("/root"));
        sub.add_node(Node::new(
            NodeKind::Function,
            format!("func_{}", i),
            PathBuf::from(format!("file_{}.py", i)),
            i + 1,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        ));
        main.merge(sub);
    }

    assert_eq!(main.nodes().count(), 5);
}

// ── parse_files_parallel() tests ────────────────────────────────

fn write_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_parallel_parse_empty_files() {
    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[], PathBuf::from("/root"));
    assert_eq!(graph.nodes().count(), 0);
    assert!(errors.is_empty());
}

#[test]
fn test_parallel_parse_single_file() {
    let dir = TempDir::new().unwrap();
    let py = write_file(
        &dir,
        "hello.py",
        "def greet(name):\n    return f'Hello, {name}'\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[py], dir.path().to_path_buf());

    assert!(errors.is_empty());
    assert!(graph.nodes().count() >= 1); // at least the function
}

#[test]
fn test_parallel_parse_multiple_languages() {
    let dir = TempDir::new().unwrap();
    let py = write_file(
        &dir,
        "app.py",
        "def main():\n    pass\n\nclass App:\n    pass\n",
    );
    let ts = write_file(
        &dir,
        "index.ts",
        "function hello(): string { return 'hi'; }\n",
    );
    let go = write_file(
        &dir,
        "main.go",
        "package main\n\nfunc Run() {}\n\nfunc Helper() {}\n",
    );

    let dispatcher = ParserDispatcher::new();
    let files = vec![py, ts, go];
    let (graph, errors) = dispatcher.parse_files_parallel(&files, dir.path().to_path_buf());

    assert!(errors.is_empty());
    // Should have nodes from all three languages
    assert!(graph.nodes().count() >= 5);
}

#[test]
fn test_parallel_parse_matches_sequential() {
    let dir = TempDir::new().unwrap();
    let py1 = write_file(
        &dir,
        "a.py",
        "def alpha():\n    pass\n\ndef beta():\n    pass\n",
    );
    let py2 = write_file(
        &dir,
        "b.py",
        "class Gamma:\n    def delta(self):\n        pass\n",
    );

    let dispatcher = ParserDispatcher::new();
    let files = vec![py1.clone(), py2.clone()];

    // Parallel
    let (par_graph, par_errors) = dispatcher.parse_files_parallel(&files, dir.path().to_path_buf());

    // Sequential
    let mut seq_graph = CodeGraph::new(dir.path().to_path_buf());
    let mut seq_errors = Vec::new();
    for file in &files {
        match dispatcher.parse_file(file, &mut seq_graph) {
            Ok(_) => {}
            Err(e) => seq_errors.push(format!("{}: {}", file.display(), e)),
        }
    }

    // Same number of nodes and errors
    assert_eq!(par_graph.nodes().count(), seq_graph.nodes().count());
    assert_eq!(par_errors.len(), seq_errors.len());

    // Same node names (order may differ)
    let par_names: HashSet<String> = par_graph
        .nodes()
        .map(|(_, n)| n.name().to_string())
        .collect();
    let seq_names: HashSet<String> = seq_graph
        .nodes()
        .map(|(_, n)| n.name().to_string())
        .collect();
    assert_eq!(par_names, seq_names);
}

#[test]
fn test_parallel_parse_collects_errors() {
    let dir = TempDir::new().unwrap();
    let bad = dir.path().join("nonexistent.py");

    let dispatcher = ParserDispatcher::new();
    let (_graph, errors) = dispatcher.parse_files_parallel(&[bad], dir.path().to_path_buf());

    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("nonexistent.py"));
}

// ── run_all_parallel() tests ────────────────────────────────────

#[test]
fn test_parallel_analyzers_match_sequential() {
    let dir = TempDir::new().unwrap();
    let _py = write_file(
        &dir,
        "bad.py",
        r#"
password = "hunter2"
api_key = "AKIA1234567890ABCDEF"
query = f"SELECT * FROM users WHERE id = {user_id}"
"#,
    );

    let files = vec![dir.path().join("bad.py")];
    let config = RevetConfig::default();

    let dispatcher = AnalyzerDispatcher::new();

    // Sequential
    let seq_findings = dispatcher.run_all(&files, dir.path(), &config);

    // Parallel
    let par_findings = dispatcher.run_all_parallel(&files, dir.path(), &config);

    // Same count
    assert_eq!(seq_findings.len(), par_findings.len());

    // Same finding IDs (order may differ across analyzers, but within each prefix it's the same)
    let seq_ids: HashSet<String> = seq_findings.iter().map(|f| f.id.clone()).collect();
    let par_ids: HashSet<String> = par_findings.iter().map(|f| f.id.clone()).collect();
    assert_eq!(seq_ids, par_ids);
}

#[test]
fn test_parallel_analyzers_empty_files() {
    let config = RevetConfig::default();
    let dispatcher = AnalyzerDispatcher::new();

    let findings = dispatcher.run_all_parallel(&[], std::path::Path::new("/tmp"), &config);
    assert!(findings.is_empty());
}

#[test]
fn test_parallel_analyzers_finding_ids_are_numbered() {
    let dir = TempDir::new().unwrap();
    let _py = write_file(
        &dir,
        "secrets.py",
        r#"
password = "pass1234"
api_key = "AKIA1234567890ABCDEF"
secret_key = "sk_live_abcdefgh12345678"
"#,
    );

    let files = vec![dir.path().join("secrets.py")];
    let config = RevetConfig::default();
    let dispatcher = AnalyzerDispatcher::new();

    let findings = dispatcher.run_all_parallel(&files, dir.path(), &config);

    // All findings have properly formatted IDs (PREFIX-NNN)
    for f in &findings {
        assert!(
            f.id.contains('-'),
            "Finding ID should contain a dash: {}",
            f.id
        );
        let parts: Vec<&str> = f.id.split('-').collect();
        assert!(
            parts.len() == 2,
            "Finding ID should be PREFIX-NNN: {}",
            f.id
        );
        assert!(
            parts[1].parse::<u32>().is_ok(),
            "Finding number should be numeric: {}",
            f.id
        );
    }
}
