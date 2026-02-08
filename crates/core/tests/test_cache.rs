//! Tests for graph caching

use revet_core::graph::{CodeGraph, Node, NodeData, NodeKind};
use revet_core::{GraphCache, GraphCacheMeta};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::SystemTime;
use tempfile::TempDir;

#[test]
fn test_cache_creation() {
    let temp_dir = TempDir::new().unwrap();
    let cache = GraphCache::new(temp_dir.path());

    // Saving an empty graph implicitly tests ensure_cache_dir
    let graph = CodeGraph::new(temp_dir.path().to_path_buf());
    let meta = GraphCacheMeta {
        commit_hash: None,
        timestamp: SystemTime::now(),
        file_checksums: HashMap::new(),
        revet_version: env!("CARGO_PKG_VERSION").to_string(),
    };
    assert!(cache.save(&graph, &meta).is_ok());
}

#[test]
fn test_save_and_load_graph() {
    let temp_dir = TempDir::new().unwrap();
    let cache = GraphCache::new(temp_dir.path());

    // Create a graph with some nodes
    let mut graph = CodeGraph::new(temp_dir.path().to_path_buf());
    let _node = graph.add_node(Node::new(
        NodeKind::Function,
        "test_func".to_string(),
        PathBuf::from("test.py"),
        1,
        NodeData::Function {
            parameters: vec![],
            return_type: None,
        },
    ));

    // Create metadata
    let meta = GraphCacheMeta {
        commit_hash: Some("abc123".to_string()),
        timestamp: SystemTime::now(),
        file_checksums: HashMap::new(),
        revet_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    // Save
    assert!(cache.save(&graph, &meta).is_ok());

    // Load
    let loaded = cache.load().unwrap();
    assert!(loaded.is_some());

    let (loaded_graph, loaded_meta) = loaded.unwrap();
    assert_eq!(loaded_meta.commit_hash, Some("abc123".to_string()));

    // Verify the node was preserved
    let nodes: Vec<_> = loaded_graph.nodes().collect();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].1.name(), "test_func");
}

#[test]
fn test_cache_clear() {
    let temp_dir = TempDir::new().unwrap();
    let cache = GraphCache::new(temp_dir.path());

    // Create and save a graph
    let graph = CodeGraph::new(temp_dir.path().to_path_buf());
    let meta = GraphCacheMeta {
        commit_hash: None,
        timestamp: SystemTime::now(),
        file_checksums: HashMap::new(),
        revet_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    cache.save(&graph, &meta).unwrap();
    assert!(cache.load().unwrap().is_some());

    // Clear cache
    assert!(cache.clear().is_ok());
    assert!(cache.load().unwrap().is_none());
}

#[test]
fn test_file_checksum_computation() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");

    std::fs::write(&file_path, "Hello, World!").unwrap();

    let checksum1 = GraphCache::compute_file_checksum(&file_path).unwrap();
    let checksum2 = GraphCache::compute_file_checksum(&file_path).unwrap();

    // Same file should have same checksum
    assert_eq!(checksum1, checksum2);

    // Modify file
    std::fs::write(&file_path, "Hello, Rust!").unwrap();
    let checksum3 = GraphCache::compute_file_checksum(&file_path).unwrap();

    // Modified file should have different checksum
    assert_ne!(checksum1, checksum3);
}

#[test]
fn test_build_file_checksums() {
    let temp_dir = TempDir::new().unwrap();

    // Create test files
    let file1 = temp_dir.path().join("file1.py");
    let file2 = temp_dir.path().join("file2.py");
    std::fs::write(&file1, "print('file1')").unwrap();
    std::fs::write(&file2, "print('file2')").unwrap();

    let file_paths = vec![PathBuf::from("file1.py"), PathBuf::from("file2.py")];

    let checksums = GraphCache::build_file_checksums(temp_dir.path(), &file_paths).unwrap();

    assert_eq!(checksums.len(), 2);
    assert!(checksums.contains_key(&PathBuf::from("file1.py")));
    assert!(checksums.contains_key(&PathBuf::from("file2.py")));
}

#[test]
fn test_find_changed_files() {
    let temp_dir = TempDir::new().unwrap();
    let cache = GraphCache::new(temp_dir.path());

    // Create initial files
    let file1 = temp_dir.path().join("file1.py");
    let file2 = temp_dir.path().join("file2.py");
    std::fs::write(&file1, "version 1").unwrap();
    std::fs::write(&file2, "unchanged").unwrap();

    // Build initial checksums
    let file_paths = vec![PathBuf::from("file1.py"), PathBuf::from("file2.py")];
    let checksums = GraphCache::build_file_checksums(temp_dir.path(), &file_paths).unwrap();

    let meta = GraphCacheMeta {
        commit_hash: None,
        timestamp: SystemTime::now(),
        file_checksums: checksums,
        revet_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    // No files changed yet
    let changed = cache.find_changed_files(&meta).unwrap();
    assert_eq!(changed.len(), 0);

    // Modify file1
    std::fs::write(&file1, "version 2").unwrap();

    // Should detect file1 as changed
    let changed = cache.find_changed_files(&meta).unwrap();
    assert_eq!(changed.len(), 1);
    assert!(changed.contains(&PathBuf::from("file1.py")));
}

#[test]
fn test_cache_validation() {
    let temp_dir = TempDir::new().unwrap();
    let cache = GraphCache::new(temp_dir.path());

    // Create a file
    let file1 = temp_dir.path().join("file1.py");
    std::fs::write(&file1, "original").unwrap();

    let checksums =
        GraphCache::build_file_checksums(temp_dir.path(), &[PathBuf::from("file1.py")]).unwrap();

    let meta = GraphCacheMeta {
        commit_hash: None,
        timestamp: SystemTime::now(),
        file_checksums: checksums,
        revet_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    // Cache should be valid
    assert!(cache.is_cache_valid(&meta).unwrap());

    // Modify the file
    std::fs::write(&file1, "modified").unwrap();

    // Cache should now be invalid
    assert!(!cache.is_cache_valid(&meta).unwrap());
}

#[test]
fn test_cache_version_validation() {
    let temp_dir = TempDir::new().unwrap();
    let cache = GraphCache::new(temp_dir.path());

    let meta = GraphCacheMeta {
        commit_hash: None,
        timestamp: SystemTime::now(),
        file_checksums: HashMap::new(),
        revet_version: "0.0.0".to_string(), // Wrong version
    };

    // Cache should be invalid due to version mismatch
    assert!(!cache.is_cache_valid(&meta).unwrap());
}
