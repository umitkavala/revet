//! In-memory graph store wrapping CodeGraph

use std::collections::HashMap;
use std::sync::RwLock;

use anyhow::{Context, Result};

use crate::graph::{EdgeKind, Node, NodeKind};
use crate::CodeGraph;

use super::{EdgeResult, GraphStore, SnapshotInfo, StoreNodeId};

/// In-memory graph store backed by `HashMap<String, CodeGraph>`
pub struct MemoryStore {
    graphs: RwLock<HashMap<String, CodeGraph>>,
}

impl MemoryStore {
    /// Create a new empty MemoryStore
    pub fn new() -> Self {
        Self {
            graphs: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphStore for MemoryStore {
    fn flush(&self, graph: &CodeGraph, snapshot: &str) -> Result<()> {
        let mut graphs = self
            .graphs
            .write()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        graphs.insert(snapshot.to_string(), graph.clone());
        Ok(())
    }

    fn snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        Ok(graphs
            .iter()
            .map(|(name, g)| {
                let node_count = g.nodes().count();
                let edge_count = g
                    .nodes()
                    .flat_map(|(id, _)| g.edges_from(id).map(|_| ()))
                    .count();
                SnapshotInfo {
                    name: name.clone(),
                    node_count,
                    edge_count,
                }
            })
            .collect())
    }

    fn delete_snapshot(&self, snapshot: &str) -> Result<()> {
        let mut graphs = self
            .graphs
            .write()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        graphs.remove(snapshot);
        Ok(())
    }

    fn node(&self, id: StoreNodeId, snapshot: &str) -> Result<Option<Node>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph.node(id.to_node_index()).cloned())
    }

    fn nodes(&self, snapshot: &str) -> Result<Vec<(StoreNodeId, Node)>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .nodes()
            .map(|(id, node)| (StoreNodeId::from(id), node.clone()))
            .collect())
    }

    fn find_nodes(
        &self,
        file_path: &str,
        name: Option<&str>,
        snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Node)>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        let path = std::path::Path::new(file_path);
        let ids = graph.find_nodes(path, name);
        Ok(ids
            .into_iter()
            .filter_map(|id| graph.node(id).map(|n| (StoreNodeId::from(id), n.clone())))
            .collect())
    }

    fn find_nodes_by_kind(
        &self,
        kind: NodeKind,
        snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Node)>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .nodes()
            .filter(|(_, node)| *node.kind() == kind)
            .map(|(id, node)| (StoreNodeId::from(id), node.clone()))
            .collect())
    }

    fn node_count(&self, snapshot: &str) -> Result<usize> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph.nodes().count())
    }

    fn edges_from(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<EdgeResult>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .edges_from(node.to_node_index())
            .map(|(target, edge)| EdgeResult {
                from: node,
                to: StoreNodeId::from(target),
                edge: edge.clone(),
            })
            .collect())
    }

    fn edges_to(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<EdgeResult>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .edges_to(node.to_node_index())
            .into_iter()
            .map(|(source, edge)| EdgeResult {
                from: StoreNodeId::from(source),
                to: node,
                edge: edge.clone(),
            })
            .collect())
    }

    fn direct_dependents(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<StoreNodeId>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .query()
            .direct_dependents(node.to_node_index())
            .into_iter()
            .map(StoreNodeId::from)
            .collect())
    }

    fn transitive_dependents(
        &self,
        node: StoreNodeId,
        max_depth: Option<usize>,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .query()
            .transitive_dependents(node.to_node_index(), max_depth)
            .into_iter()
            .map(StoreNodeId::from)
            .collect())
    }

    fn dependencies(&self, node: StoreNodeId, snapshot: &str) -> Result<Vec<StoreNodeId>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .query()
            .dependencies(node.to_node_index())
            .into_iter()
            .map(StoreNodeId::from)
            .collect())
    }

    fn transitive_dependencies(
        &self,
        node: StoreNodeId,
        max_depth: Option<usize>,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .query()
            .transitive_dependencies(node.to_node_index(), max_depth)
            .into_iter()
            .map(StoreNodeId::from)
            .collect())
    }

    fn find_by_edge_kind(
        &self,
        node: StoreNodeId,
        kind: EdgeKind,
        snapshot: &str,
    ) -> Result<Vec<StoreNodeId>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let graph = graphs.get(snapshot).context("snapshot not found")?;
        Ok(graph
            .query()
            .find_by_edge_kind(node.to_node_index(), kind)
            .into_iter()
            .map(StoreNodeId::from)
            .collect())
    }

    fn find_changed_nodes(
        &self,
        old_snapshot: &str,
        new_snapshot: &str,
    ) -> Result<Vec<(StoreNodeId, Option<StoreNodeId>)>> {
        let graphs = self
            .graphs
            .read()
            .map_err(|e| anyhow::anyhow!("lock poisoned: {e}"))?;
        let old_graph = graphs.get(old_snapshot).context("old snapshot not found")?;
        let new_graph = graphs.get(new_snapshot).context("new snapshot not found")?;

        let mut changed = Vec::new();

        for (new_id, new_node) in new_graph.nodes() {
            // Find matching node in old graph by file_path + name + kind
            let old_match = old_graph.nodes().find(|(_, old_node)| {
                old_node.file_path() == new_node.file_path()
                    && old_node.name() == new_node.name()
                    && old_node.kind() == new_node.kind()
            });

            match old_match {
                Some((old_id, old_node)) => {
                    // Check if data or line changed
                    if old_node.data() != new_node.data() || old_node.line() != new_node.line() {
                        changed.push((StoreNodeId::from(new_id), Some(StoreNodeId::from(old_id))));
                    }
                }
                None => {
                    // New node (added)
                    changed.push((StoreNodeId::from(new_id), None));
                }
            }
        }

        Ok(changed)
    }
}
