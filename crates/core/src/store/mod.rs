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

use crate::graph::{Edge, EdgeKind, Node, NodeId, NodeKind};
use crate::CodeGraph;
use anyhow::Result;

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
