//! Graph storage abstraction layer
//!
//! Provides a `GraphStore` trait that decouples graph queries from the underlying
//! storage engine. Two implementations:
//! - `MemoryStore`: in-memory, wraps `CodeGraph` (always available)
//! - `CozoStore`: CozoDB-backed persistent storage (behind `cozo-store` feature)

pub mod memory;

#[cfg(feature = "cozo-store")]
pub mod cozo;

pub use memory::MemoryStore;

#[cfg(feature = "cozo-store")]
pub use cozo::CozoStore;

use std::collections::HashMap;
use std::path::Path;

use crate::graph::{Edge, EdgeKind, Node, NodeId, NodeKind};
use crate::CodeGraph;
use anyhow::{Context, Result};

/// Storage-agnostic node identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoreNodeId(pub u64);

impl From<NodeId> for StoreNodeId {
    fn from(id: NodeId) -> Self {
        StoreNodeId(id.index() as u64)
    }
}

impl StoreNodeId {
    /// Convert back to a petgraph NodeIndex (for MemoryStore)
    pub fn to_node_index(self) -> NodeId {
        petgraph::graph::NodeIndex::new(self.0 as usize)
    }
}

/// Snapshot metadata
#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

/// An edge result with source, target, and edge data
#[derive(Debug, Clone)]
pub struct EdgeResult {
    pub from: StoreNodeId,
    pub to: StoreNodeId,
    pub edge: Edge,
}

/// Abstract graph storage backend
///
/// All methods take a `snapshot` parameter to support multiple graph versions.
/// Returns owned values so callers don't hold locks.
pub trait GraphStore: Send + Sync {
    // -- Lifecycle --

    /// Flush a CodeGraph into the store under the given snapshot name
    fn flush(&self, graph: &CodeGraph, snapshot: &str) -> Result<()>;

    /// List all snapshots
    fn snapshots(&self) -> Result<Vec<SnapshotInfo>>;

    /// Delete a snapshot and all its data
    fn delete_snapshot(&self, snapshot: &str) -> Result<()>;

    // -- Node queries --

    /// Get a single node by its store ID
    fn node(&self, id: StoreNodeId, snapshot: &str) -> Result<Option<Node>>;

    /// Get all nodes in a snapshot
    fn nodes(&self, snapshot: &str) -> Result<Vec<(StoreNodeId, Node)>>;

    /// Find nodes by file path and optional name
    fn find_nodes(
        &self,
        file_path: &str,
        name: Option<&str>,
        snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Node)>>;

    /// Find nodes by kind
    fn find_nodes_by_kind(
        &self,
        kind: NodeKind,
        snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Node)>>;

    /// Count nodes in a snapshot
    fn node_count(&self, snapshot: &str) -> Result<usize>;

    // -- Edge queries --

    /// Get all outgoing edges from a node
    fn edges_from(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<EdgeResult>>;

    /// Get all incoming edges to a node
    fn edges_to(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<EdgeResult>>;

    // -- Traversals --

    /// Find nodes that directly depend on the given node (reverse edge lookup)
    fn direct_dependents(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<StoreNodeId>>;

    /// Find all transitive dependents (BFS through reverse edges)
    fn transitive_dependents(
        &self,
        node: StoreNodeId,
        max_depth: Option<usize>,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>>;

    /// Find nodes that the given node depends on (forward edge lookup)
    fn dependencies(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<StoreNodeId>>;

    /// Find all transitive dependencies (BFS through forward edges)
    fn transitive_dependencies(
        &self,
        node: StoreNodeId,
        max_depth: Option<usize>,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>>;

    /// Find nodes connected by a specific edge kind (outgoing)
    fn find_by_edge_kind(
        &self,
        node: StoreNodeId,
        kind: EdgeKind,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>>;

    // -- Cross-snapshot --

    /// Find changed nodes between two snapshots
    /// Returns (new_id, Option<old_id>) — None means added, Some means modified
    fn find_changed_nodes(
        &self,
        old_snapshot: &str,
        new_snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Option<StoreNodeId>)>>;
}

/// Reconstruct a `CodeGraph` from a store snapshot.
///
/// Loads all nodes (sorted by StoreNodeId for deterministic NodeId assignment),
/// maps them into a new CodeGraph, then adds all edges.
pub fn reconstruct_graph(
    store: &dyn GraphStore,
    snapshot: &str,
    root_path: &Path,
) -> Result<CodeGraph> {
    let mut graph = CodeGraph::new(root_path.to_path_buf());

    // Load nodes sorted by store ID for deterministic petgraph NodeId assignment
    let mut nodes = store.nodes(snapshot)?;
    nodes.sort_by_key(|(id, _)| id.0);

    // Map StoreNodeId → petgraph NodeId
    let mut id_map: HashMap<StoreNodeId, NodeId> = HashMap::new();
    for (store_id, node) in &nodes {
        let graph_id = graph.add_node(node.clone());
        id_map.insert(*store_id, graph_id);
    }

    // Add edges
    for (store_id, _) in &nodes {
        let edges = store.edges_from(*store_id, snapshot)?;
        for edge_result in edges {
            if let (Some(&from), Some(&to)) =
                (id_map.get(&edge_result.from), id_map.get(&edge_result.to))
            {
                graph.add_edge(from, to, edge_result.edge);
            }
        }
    }

    Ok(graph)
}

/// Create a SQLite-backed CozoStore at `.revet-cache/graph.db` under the given repo root.
#[cfg(feature = "cozo-store")]
pub fn create_store(repo_root: &Path) -> Result<CozoStore> {
    let cache_dir = repo_root.join(".revet-cache");
    std::fs::create_dir_all(&cache_dir).context("Failed to create .revet-cache directory")?;
    let db_path = cache_dir.join("graph.db");
    CozoStore::new_sqlite(&db_path)
}
