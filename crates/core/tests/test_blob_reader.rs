//! Integration tests for GitTreeReader (git blob reading)

use git2::{Repository, Signature};
use revet_core::diff::blob::GitTreeReader;
use revet_core::parser::ParserDispatcher;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Helper: create a temp git repo with an initial commit containing the given files.
/// Returns (TempDir, Repository).
fn create_test_repo(files: &[(&str, &str)]) -> (TempDir, Repository) {
    let dir = TempDir::new().unwrap();
    let repo = Repository::init(dir.path()).unwrap();

    // Write files to the working directory
    for (path, content) in files {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, content).unwrap();
    }

    // Stage all files
    let mut index = repo.index().unwrap();
    for (path, _) in files {
        index.add_path(Path::new(path)).unwrap();
    }
    index.write().unwrap();

    // Create initial commit (scoped to drop tree before returning repo)
    let tree_oid = index.write_tree().unwrap();
    {
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = Signature::now("test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap();
    }

    (dir, repo)
}

/// Helper: add a new commit on top of HEAD with the given file changes.
fn add_commit(repo: &Repository, dir: &TempDir, files: &[(&str, &str)], message: &str) {
    for (path, content) in files {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full, content).unwrap();
    }

    let mut index = repo.index().unwrap();
    for (path, _) in files {
        index.add_path(Path::new(path)).unwrap();
    }
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = Signature::now("test", "test@example.com").unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&head])
        .unwrap();
}

// ── read_files_at_ref tests ─────────────────────────────────────

#[test]
fn test_read_files_at_ref() {
    let (dir, _repo) = create_test_repo(&[
        ("hello.py", "def hello():\n    print('hi')\n"),
        ("util.py", "def util(): pass\n"),
    ]);

    let reader = GitTreeReader::new(dir.path()).unwrap();
    let files = reader.read_files_at_ref("HEAD", &[".py"]).unwrap();

    assert_eq!(files.len(), 2);

    let names: Vec<&str> = files.iter().map(|f| f.path.to_str().unwrap()).collect();
    assert!(names.contains(&"hello.py"));
    assert!(names.contains(&"util.py"));

    let hello = files
        .iter()
        .find(|f| f.path.to_str() == Some("hello.py"))
        .unwrap();
    assert!(hello.content.contains("def hello()"));
}

#[test]
fn test_read_files_filters_extensions() {
    let (dir, _repo) = create_test_repo(&[
        ("app.py", "x = 1\n"),
        ("index.ts", "const x = 1;\n"),
        ("readme.md", "# Hello\n"),
    ]);

    let reader = GitTreeReader::new(dir.path()).unwrap();

    // Only .py files
    let py_files = reader.read_files_at_ref("HEAD", &[".py"]).unwrap();
    assert_eq!(py_files.len(), 1);
    assert_eq!(py_files[0].path, PathBuf::from("app.py"));

    // Only .ts files
    let ts_files = reader.read_files_at_ref("HEAD", &[".ts"]).unwrap();
    assert_eq!(ts_files.len(), 1);
    assert_eq!(ts_files[0].path, PathBuf::from("index.ts"));

    // Multiple extensions
    let both = reader.read_files_at_ref("HEAD", &[".py", ".ts"]).unwrap();
    assert_eq!(both.len(), 2);
}

#[test]
fn test_read_files_in_subdirectories() {
    let (dir, _repo) = create_test_repo(&[
        ("src/main.py", "def main(): pass\n"),
        ("src/utils/helpers.py", "def helper(): pass\n"),
    ]);

    let reader = GitTreeReader::new(dir.path()).unwrap();
    let files = reader.read_files_at_ref("HEAD", &[".py"]).unwrap();

    assert_eq!(files.len(), 2);
    let paths: Vec<String> = files.iter().map(|f| f.path.display().to_string()).collect();
    assert!(paths.contains(&"src/main.py".to_string()));
    assert!(paths.contains(&"src/utils/helpers.py".to_string()));
}

// ── read_file_at_ref tests ──────────────────────────────────────

#[test]
fn test_read_single_file() {
    let (dir, _repo) = create_test_repo(&[("app.py", "x = 42\n"), ("other.py", "y = 99\n")]);

    let reader = GitTreeReader::new(dir.path()).unwrap();
    let content = reader
        .read_file_at_ref("HEAD", Path::new("app.py"))
        .unwrap();

    assert_eq!(content, Some("x = 42\n".to_string()));
}

#[test]
fn test_read_nonexistent_file() {
    let (dir, _repo) = create_test_repo(&[("app.py", "x = 1\n")]);

    let reader = GitTreeReader::new(dir.path()).unwrap();
    let content = reader
        .read_file_at_ref("HEAD", Path::new("missing.py"))
        .unwrap();

    assert_eq!(content, None);
}

#[test]
fn test_read_empty_tree() {
    // Create a repo with only a non-matching file
    let (dir, _repo) = create_test_repo(&[("readme.md", "# Hello\n")]);

    let reader = GitTreeReader::new(dir.path()).unwrap();
    let files = reader.read_files_at_ref("HEAD", &[".py"]).unwrap();

    assert!(files.is_empty());
}

// ── build_graph_at_ref tests ────────────────────────────────────

#[test]
fn test_build_graph_at_ref() {
    let (dir, _repo) = create_test_repo(&[(
        "app.py",
        "def greet(name: str) -> str:\n    return f'Hello, {name}'\n\nclass User:\n    def __init__(self, name):\n        self.name = name\n",
    )]);

    let reader = GitTreeReader::new(dir.path()).unwrap();
    let dispatcher = ParserDispatcher::new();
    let graph = reader
        .build_graph_at_ref("HEAD", dir.path(), &dispatcher)
        .unwrap();

    // Should have parsed the function and class
    let node_count: usize = graph.nodes().count();
    assert!(
        node_count >= 2,
        "Expected at least 2 nodes, got {}",
        node_count
    );

    // Verify paths are absolute (matching live graph convention)
    for (_id, node) in graph.nodes() {
        assert!(
            node.file_path().is_absolute(),
            "Expected absolute path, got: {:?}",
            node.file_path()
        );
    }
}

#[test]
fn test_build_graph_two_commits_impact() {
    let (dir, repo) = create_test_repo(&[(
        "api.py",
        "def get_user(user_id: int) -> dict:\n    return {}\n",
    )]);

    // Build old graph from initial commit
    let reader = GitTreeReader::new(dir.path()).unwrap();
    let dispatcher = ParserDispatcher::new();
    let old_graph = reader
        .build_graph_at_ref("HEAD", dir.path(), &dispatcher)
        .unwrap();

    // Make a breaking change: change the parameter name and return type
    add_commit(
        &repo,
        &dir,
        &[(
            "api.py",
            "def get_user(uid: str, include_email: bool = False) -> list:\n    return []\n",
        )],
        "breaking change",
    );

    // Build new graph from HEAD
    let new_graph = reader
        .build_graph_at_ref("HEAD", dir.path(), &dispatcher)
        .unwrap();

    // Run impact analysis between old and new
    let analysis = revet_core::ImpactAnalysis::new(old_graph, new_graph);
    let report = analysis.analyze_impact();

    // Should detect the breaking change in get_user
    assert!(
        !report.changes.is_empty(),
        "Expected at least one change detected"
    );

    let get_user_change = report.changes.iter().find(|c| {
        analysis
            .new_graph()
            .node(c.node_id)
            .map(|n| n.name() == "get_user")
            .unwrap_or(false)
    });

    assert!(
        get_user_change.is_some(),
        "Expected a change for get_user function"
    );

    let change = get_user_change.unwrap();
    assert_eq!(
        change.classification,
        revet_core::ChangeClassification::Breaking,
        "Parameter change should be classified as Breaking"
    );
}

// ── Binary file handling ────────────────────────────────────────

#[test]
fn test_skips_binary_files() {
    // Create a repo with a binary-ish .py file (null bytes make git2 detect it as binary)
    let (dir, _repo) = create_test_repo(&[("normal.py", "x = 1\n")]);

    // Write a binary file and commit it
    let bin_path = dir.path().join("binary.py");
    std::fs::write(&bin_path, b"x = 1\0\0\0binary data").unwrap();

    let repo = Repository::open(dir.path()).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("binary.py")).unwrap();
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = Signature::now("test", "test@example.com").unwrap();
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "add binary", &tree, &[&head])
        .unwrap();

    let reader = GitTreeReader::new(dir.path()).unwrap();
    let files = reader.read_files_at_ref("HEAD", &[".py"]).unwrap();

    // Only the normal file should be returned (binary skipped)
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, PathBuf::from("normal.py"));
}
