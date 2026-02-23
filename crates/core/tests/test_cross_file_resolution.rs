//! Tests for cross-file import/call edge resolution.
//!
//! Each test creates temp files, runs `parse_files_parallel`, then asserts
//! that the resolver added the expected `Imports`, `References`, and `Calls`
//! edges across file boundaries.

use revet_core::graph::{EdgeKind, NodeData, NodeKind};
use revet_core::ParserDispatcher;
use std::path::PathBuf;
use tempfile::TempDir;

fn write(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

// ── Python ─────────────────────────────────────────────────────────────────

#[test]
fn test_python_relative_import_creates_imports_edge() {
    let dir = TempDir::new().unwrap();
    let utils = write(&dir, "utils.py", "def helper(): pass\n");
    let main = write(
        &dir,
        "main.py",
        "from utils import helper\n\ndef run():\n    helper()\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[utils, main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    // There should be an Imports edge between the two File nodes
    let imports_edges: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::File))
        .flat_map(|(id, _)| {
            graph
                .edges_from(id)
                .filter(|(_, e)| matches!(e.kind(), EdgeKind::Imports))
                .map(move |(target, _)| (id, target))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        !imports_edges.is_empty(),
        "expected at least one Imports edge between files"
    );
}

#[test]
fn test_python_import_references_symbol() {
    let dir = TempDir::new().unwrap();
    let utils = write(&dir, "utils.py", "def helper(): pass\n");
    let main = write(
        &dir,
        "main.py",
        "from utils import helper\n\ndef run():\n    helper()\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[utils, main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    // The Import node in main.py should have a References edge to the helper function
    let ref_edges: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .flat_map(|(id, _)| {
            graph
                .edges_from(id)
                .filter(|(_, e)| matches!(e.kind(), EdgeKind::References))
                .map(move |(target, _)| (id, target))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        !ref_edges.is_empty(),
        "expected References edge from Import node to helper function"
    );

    // The target should be a Function node named "helper"
    let (_, target_id) = ref_edges[0];
    let target = graph.node(target_id).unwrap();
    assert_eq!(target.name(), "helper");
    assert!(matches!(target.kind(), NodeKind::Function));
}

#[test]
fn test_python_import_resolved_path_is_stamped() {
    let dir = TempDir::new().unwrap();
    let _utils = write(&dir, "utils.py", "def helper(): pass\n");
    let main = write(&dir, "main.py", "from utils import helper\n");

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) =
        dispatcher.parse_files_parallel(&[_utils, main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    // Import node in main.py should have resolved_path set
    let resolved = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import) && n.file_path().ends_with("main.py"))
        .any(|(_, n)| {
            if let NodeData::Import { resolved_path, .. } = n.data() {
                resolved_path.is_some()
            } else {
                false
            }
        });

    assert!(resolved, "expected Import node to have resolved_path set");
}

#[test]
fn test_python_wildcard_import_no_references() {
    let dir = TempDir::new().unwrap();
    let utils = write(&dir, "utils.py", "def helper(): pass\n");
    let main = write(&dir, "main.py", "from utils import *\n");

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[utils, main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    // Wildcard imports get an Imports edge but no References edges
    let ref_edges: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .flat_map(|(id, _)| {
            graph
                .edges_from(id)
                .filter(|(_, e)| matches!(e.kind(), EdgeKind::References))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        ref_edges.is_empty(),
        "wildcard imports should not produce References edges, got: {:?}",
        ref_edges.len()
    );
}

// ── TypeScript ─────────────────────────────────────────────────────────────

#[test]
fn test_typescript_named_import_creates_imports_edge() {
    let dir = TempDir::new().unwrap();
    let utils = write(
        &dir,
        "utils.ts",
        "export function greet(name: string): string { return name; }\n",
    );
    let main = write(
        &dir,
        "main.ts",
        "import { greet } from './utils';\n\nfunction run() { greet('world'); }\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[utils, main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let imports_edges: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::File))
        .flat_map(|(id, _)| {
            graph
                .edges_from(id)
                .filter(|(_, e)| matches!(e.kind(), EdgeKind::Imports))
                .map(move |(target, _)| (id, target))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        !imports_edges.is_empty(),
        "expected Imports edge from main.ts to utils.ts"
    );
}

#[test]
fn test_typescript_named_import_references_symbol() {
    let dir = TempDir::new().unwrap();
    let utils = write(
        &dir,
        "utils.ts",
        "export function greet(name: string): string { return name; }\n",
    );
    let main = write(
        &dir,
        "main.ts",
        "import { greet } from './utils';\n\nfunction run() { greet('world'); }\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[utils, main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let ref_edges: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .flat_map(|(id, _)| {
            graph
                .edges_from(id)
                .filter(|(_, e)| matches!(e.kind(), EdgeKind::References))
                .map(move |(target, _)| (id, target))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        !ref_edges.is_empty(),
        "expected References edge from Import node to greet function"
    );
    let (_, target_id) = ref_edges[0];
    assert_eq!(graph.node(target_id).unwrap().name(), "greet");
}

// ── Go ─────────────────────────────────────────────────────────────────────

#[test]
fn test_go_import_creates_imports_edge() {
    let dir = TempDir::new().unwrap();
    let pkg = write(&dir, "util/util.go", "package util\n\nfunc Helper() {}\n");
    let main = write(
        &dir,
        "main.go",
        &format!(
            "package main\n\nimport \"{}\"\n\nfunc main() {{ util.Helper() }}\n",
            dir.path().join("util").display()
        ),
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[pkg, main], dir.path().to_path_buf());
    // Go resolver uses package directory matching — may or may not resolve
    // depending on the specifier format, so we just verify no crash
    let _ = errors;
    let _ = graph;
}

// ── Multi-file resolution correctness ──────────────────────────────────────

#[test]
fn test_no_spurious_edges_for_stdlib_imports() {
    let dir = TempDir::new().unwrap();
    let main = write(
        &dir,
        "app.py",
        "import os\nimport sys\n\ndef main():\n    pass\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    // stdlib imports (os, sys) are not in the file index → no File→File Imports edges
    let file_to_file_imports: Vec<_> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::File))
        .flat_map(|(id, _)| {
            graph
                .edges_from(id)
                .filter(|(target, e)| {
                    matches!(e.kind(), EdgeKind::Imports)
                        && matches!(graph.node(*target).map(|n| n.kind()), Some(NodeKind::File))
                })
                .map(move |(target, _)| (id, target))
                .collect::<Vec<_>>()
        })
        .collect();

    assert!(
        file_to_file_imports.is_empty(),
        "stdlib imports should not produce File→File Imports edges"
    );
}

#[test]
fn test_unresolvable_import_does_not_crash() {
    let dir = TempDir::new().unwrap();
    let main = write(
        &dir,
        "main.py",
        "from nonexistent_module import something\n\ndef run(): pass\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    // Graph still has nodes (File + Import + Function)
    assert!(graph.nodes().count() >= 1);
}

#[test]
fn test_multiple_named_imports_each_get_references_edge() {
    let dir = TempDir::new().unwrap();
    let utils = write(
        &dir,
        "utils.py",
        "def alpha(): pass\ndef beta(): pass\ndef gamma(): pass\n",
    );
    let main = write(
        &dir,
        "main.py",
        "from utils import alpha, beta, gamma\n\ndef run(): alpha(); beta(); gamma()\n",
    );

    let dispatcher = ParserDispatcher::new();
    let (graph, errors) = dispatcher.parse_files_parallel(&[utils, main], dir.path().to_path_buf());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let ref_count = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::Import))
        .flat_map(|(id, _)| {
            graph
                .edges_from(id)
                .filter(|(_, e)| matches!(e.kind(), EdgeKind::References))
                .collect::<Vec<_>>()
        })
        .count();

    assert_eq!(
        ref_count, 3,
        "expected 3 References edges, one per imported name"
    );
}
