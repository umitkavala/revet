//! Git blob reading for building code graphs from historical commits
//!
//! This module allows constructing a [`CodeGraph`] from any git ref (branch, tag, commit)
//! by reading file contents directly from git blobs — no checkout required.

use anyhow::{Context, Result};
use git2::{ObjectType, Oid, Repository};
use std::path::{Path, PathBuf};

use crate::graph::CodeGraph;
use crate::parser::ParserDispatcher;

/// A file read from a git tree
#[derive(Debug, Clone)]
pub struct GitFile {
    /// Relative path within the repository
    pub path: PathBuf,
    /// UTF-8 file content
    pub content: String,
}

/// Reads file contents from git trees without checking out
pub struct GitTreeReader {
    repo: Repository,
}

impl GitTreeReader {
    /// Open a git repository at the given path
    pub fn new(repo_path: &Path) -> Result<Self> {
        let repo = Repository::open(repo_path)
            .context("Failed to open git repository for blob reading")?;
        Ok(Self { repo })
    }

    /// Read all files at a given ref, filtered by extension
    ///
    /// `extensions` should be in the form `[".py", ".ts", ".js"]`.
    /// Binary files and non-UTF-8 files are silently skipped.
    pub fn read_files_at_ref(&self, ref_spec: &str, extensions: &[&str]) -> Result<Vec<GitFile>> {
        let tree = self.resolve_tree(ref_spec)?;

        // Pass 1: collect (relative_path, oid) pairs to avoid borrow issues
        let mut entries: Vec<(PathBuf, Oid)> = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() != Some(ObjectType::Blob) {
                return git2::TreeWalkResult::Ok;
            }

            let name = match entry.name() {
                Some(n) => n,
                None => return git2::TreeWalkResult::Ok,
            };

            let rel_path = if dir.is_empty() {
                PathBuf::from(name)
            } else {
                PathBuf::from(dir).join(name)
            };

            // Filter by extension
            if !has_matching_extension(&rel_path, extensions) {
                return git2::TreeWalkResult::Ok;
            }

            entries.push((rel_path, entry.id()));
            git2::TreeWalkResult::Ok
        })?;

        // Pass 2: read blobs
        let mut files = Vec::with_capacity(entries.len());
        for (rel_path, oid) in entries {
            if let Ok(blob) = self.repo.find_blob(oid) {
                if blob.is_binary() {
                    continue;
                }
                if let Ok(content) = std::str::from_utf8(blob.content()) {
                    files.push(GitFile {
                        path: rel_path,
                        content: content.to_string(),
                    });
                }
            }
        }

        Ok(files)
    }

    /// Read a single file at a given ref
    ///
    /// Returns `None` if the file doesn't exist at that ref or is binary.
    pub fn read_file_at_ref(&self, ref_spec: &str, file_path: &Path) -> Result<Option<String>> {
        let tree = self.resolve_tree(ref_spec)?;

        let entry = match tree.get_path(file_path) {
            Ok(e) => e,
            Err(_) => return Ok(None),
        };

        let blob = self
            .repo
            .find_blob(entry.id())
            .context("Failed to read blob")?;

        if blob.is_binary() {
            return Ok(None);
        }

        match std::str::from_utf8(blob.content()) {
            Ok(s) => Ok(Some(s.to_string())),
            Err(_) => Ok(None),
        }
    }

    /// Build a [`CodeGraph`] from files at a given ref
    ///
    /// Reads all parseable files from the git tree, parses each with the
    /// appropriate language parser, and returns a complete graph. File paths
    /// in the graph use absolute paths (`repo_root.join(relative_path)`) to
    /// match the convention used by the live graph.
    pub fn build_graph_at_ref(
        &self,
        ref_spec: &str,
        repo_root: &Path,
        dispatcher: &ParserDispatcher,
    ) -> Result<CodeGraph> {
        let extensions = dispatcher.supported_extensions();
        let files = self.read_files_at_ref(ref_spec, &extensions)?;

        let mut graph = CodeGraph::new(repo_root.to_path_buf());

        for git_file in &files {
            // Use absolute path to match the live graph's convention
            let abs_path = repo_root.join(&git_file.path);

            if let Some(parser) = dispatcher.find_parser(&abs_path) {
                // Ignore parse errors for individual files — the graph is best-effort
                let _ = parser.parse_source(&git_file.content, &abs_path, &mut graph);
            }
        }

        Ok(graph)
    }

    fn resolve_tree(&self, spec: &str) -> Result<git2::Tree<'_>> {
        let obj = self
            .repo
            .revparse_single(spec)
            .with_context(|| format!("Failed to resolve git ref '{}'", spec))?;
        let commit = obj
            .peel_to_commit()
            .with_context(|| format!("'{}' does not point to a commit", spec))?;
        commit.tree().context("Failed to get tree from commit")
    }
}

fn has_matching_extension(path: &Path, extensions: &[&str]) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    let with_dot = format!(".{}", ext);
    extensions.contains(&with_dot.as_str())
}
