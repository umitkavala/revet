//! Graph caching for incremental analysis

use crate::graph::CodeGraph;
use anyhow::{Result, Context};
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
        std::fs::create_dir_all(&self.cache_dir)
            .context("Failed to create cache directory")?;
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
    fn compute_file_checksum(file_path: &Path) -> Result<String> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_creation() {
        let temp_dir = TempDir::new().unwrap();
        let cache = GraphCache::new(temp_dir.path());
        assert!(cache.ensure_cache_dir().is_ok());
    }
}
