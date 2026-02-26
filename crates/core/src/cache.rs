//! Graph caching for incremental analysis

use crate::graph::CodeGraph;
use crate::parser::ParseState;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Metadata about a cached graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCacheMeta {
    /// Git commit hash when the graph was built
    pub commit_hash: Option<String>,

    /// Timestamp when the graph was built
    pub timestamp: SystemTime,

    /// File checksums (path -> checksum)
    pub file_checksums: HashMap<PathBuf, String>,

    /// Revet version that created this cache
    pub revet_version: String,
}

/// Manages graph caching for incremental analysis
pub struct GraphCache {
    cache_dir: PathBuf,
}

impl GraphCache {
    /// Create a new graph cache manager
    pub fn new(repo_root: &Path) -> Self {
        Self {
            cache_dir: repo_root.join(".revet-cache"),
        }
    }

    /// Ensure the cache directory exists
    fn ensure_cache_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir).context("Failed to create cache directory")?;
        Ok(())
    }

    /// Load a cached graph if it exists
    pub fn load(&self) -> Result<Option<(CodeGraph, GraphCacheMeta)>> {
        let graph_path = self.cache_dir.join("graph.msgpack");
        let meta_path = self.cache_dir.join("graph.meta.json");

        if !graph_path.exists() || !meta_path.exists() {
            return Ok(None);
        }

        // Load metadata
        let meta_contents = std::fs::read_to_string(&meta_path)?;
        let meta: GraphCacheMeta = serde_json::from_str(&meta_contents)?;

        // Load graph
        let graph_contents = std::fs::read(&graph_path)?;
        let graph: CodeGraph = rmp_serde::from_slice(&graph_contents)?;

        Ok(Some((graph, meta)))
    }

    /// Save a graph to the cache
    pub fn save(&self, graph: &CodeGraph, meta: &GraphCacheMeta) -> Result<()> {
        self.ensure_cache_dir()?;

        let graph_path = self.cache_dir.join("graph.msgpack");
        let meta_path = self.cache_dir.join("graph.meta.json");

        // Save metadata
        let meta_contents = serde_json::to_string_pretty(meta)?;
        std::fs::write(&meta_path, meta_contents)?;

        // Save graph
        let graph_contents = rmp_serde::to_vec(graph)?;
        std::fs::write(&graph_path, graph_contents)?;

        Ok(())
    }

    /// Save only the metadata JSON (when graph is persisted via CozoStore)
    pub fn save_meta(&self, meta: &GraphCacheMeta) -> Result<()> {
        self.ensure_cache_dir()?;
        let meta_path = self.cache_dir.join("graph.meta.json");
        let meta_contents = serde_json::to_string_pretty(meta)?;
        std::fs::write(&meta_path, meta_contents)?;
        Ok(())
    }

    /// Clear the cache
    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }

    /// Check if a file has changed based on its checksum
    pub fn file_changed(&self, file_path: &Path, meta: &GraphCacheMeta) -> Result<bool> {
        let current_checksum = Self::compute_file_checksum(file_path)?;

        Ok(meta
            .file_checksums
            .get(file_path)
            .map(|cached| cached != &current_checksum)
            .unwrap_or(true))
    }

    /// Compute a checksum for a file
    pub fn compute_file_checksum(file_path: &Path) -> Result<String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let contents = std::fs::read(file_path)?;
        let mut hasher = DefaultHasher::new();
        contents.hash(&mut hasher);
        Ok(format!("{:x}", hasher.finish()))
    }

    /// Get the current Git commit hash
    pub fn get_git_commit_hash(repo_root: &Path) -> Option<String> {
        use git2::Repository;

        Repository::open(repo_root)
            .ok()?
            .head()
            .ok()?
            .peel_to_commit()
            .ok()
            .map(|commit| commit.id().to_string())
    }

    /// Build file checksums for all files in the repository
    pub fn build_file_checksums(
        repo_root: &Path,
        file_paths: &[PathBuf],
    ) -> Result<HashMap<PathBuf, String>> {
        let mut checksums = HashMap::new();

        for file_path in file_paths {
            let full_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                repo_root.join(file_path)
            };

            if full_path.exists() {
                let checksum = Self::compute_file_checksum(&full_path)?;
                checksums.insert(file_path.clone(), checksum);
            }
        }

        Ok(checksums)
    }

    /// Find files that have changed since the cached version
    pub fn find_changed_files(&self, meta: &GraphCacheMeta) -> Result<Vec<PathBuf>> {
        let mut changed_files = Vec::new();

        for (file_path, cached_checksum) in &meta.file_checksums {
            let full_path = if file_path.is_absolute() {
                file_path.clone()
            } else {
                self.cache_dir
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(file_path)
            };

            if !full_path.exists() {
                // File was deleted
                changed_files.push(file_path.clone());
                continue;
            }

            let current_checksum = Self::compute_file_checksum(&full_path)?;
            if &current_checksum != cached_checksum {
                changed_files.push(file_path.clone());
            }
        }

        Ok(changed_files)
    }

    /// Check if the cache is valid for the current repository state.
    ///
    /// Returns `true` only when the revet version, git commit, and all file
    /// checksums are unchanged — i.e. a full re-parse can be skipped entirely.
    pub fn is_cache_valid(&self, meta: &GraphCacheMeta) -> Result<bool> {
        // Check if revet version matches
        let current_version = env!("CARGO_PKG_VERSION");
        if meta.revet_version != current_version {
            return Ok(false);
        }

        // Check if git commit hash matches (if available)
        if let Some(cached_hash) = &meta.commit_hash {
            if let Some(current_hash) =
                Self::get_git_commit_hash(self.cache_dir.parent().unwrap_or_else(|| Path::new(".")))
            {
                if cached_hash != &current_hash {
                    return Ok(false);
                }
            }
        }

        // Check if any files have changed
        let changed_files = self.find_changed_files(meta)?;
        Ok(changed_files.is_empty())
    }
}

// ── Per-file graph fragment cache ────────────────────────────────────────────

/// Serialized payload for a single file's parse result.
#[derive(Serialize, Deserialize)]
struct FileFragment {
    graph: CodeGraph,
    state: ParseState,
}

/// Cache for per-file `(CodeGraph, ParseState)` fragments.
///
/// Each entry is keyed by the hex content-hash of the source file and stored
/// as a msgpack file in `.revet-cache/files/`. On a subsequent run, unchanged
/// files are loaded from cache instead of being re-parsed by tree-sitter,
/// giving near-instant re-runs for large repos where only a few files changed.
pub struct FileGraphCache {
    cache_dir: PathBuf,
}

impl FileGraphCache {
    /// Create a file graph cache rooted at `<repo_root>/.revet-cache/files/`.
    pub fn new(repo_root: &Path) -> Self {
        Self {
            cache_dir: repo_root.join(".revet-cache").join("files"),
        }
    }

    fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.cache_dir)
            .context("Failed to create file graph cache directory")?;
        Ok(())
    }

    fn path_for(&self, content_hash: &str) -> PathBuf {
        self.cache_dir.join(format!("{}.msgpack", content_hash))
    }

    /// Load a cached `(CodeGraph, ParseState)` for a file with the given
    /// content hash. Returns `None` on any miss or deserialization failure
    /// so callers always fall back to a fresh parse.
    pub fn load(&self, content_hash: &str) -> Option<(CodeGraph, ParseState)> {
        let path = self.path_for(content_hash);
        let bytes = std::fs::read(&path).ok()?;
        let fragment: FileFragment = rmp_serde::from_slice(&bytes).ok()?;
        Some((fragment.graph, fragment.state))
    }

    /// Persist a `(CodeGraph, ParseState)` fragment keyed by content hash.
    /// Failures are silently ignored — caching is best-effort.
    pub fn save(&self, content_hash: &str, graph: &CodeGraph, state: &ParseState) {
        if self.ensure_dir().is_err() {
            return;
        }
        let fragment = FileFragment {
            graph: graph.clone(),
            state: state.clone(),
        };
        if let Ok(bytes) = rmp_serde::to_vec(&fragment) {
            let _ = std::fs::write(self.path_for(content_hash), bytes);
        }
    }
}
