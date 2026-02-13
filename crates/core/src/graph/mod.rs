//! Code dependency graph data structures and operations

pub mod edges;
pub mod nodes;
pub mod query;

pub use edges::{Edge, EdgeKind, EdgeMetadata};
pub use nodes::{Node, NodeData, NodeKind, Parameter};
pub use query::GraphQuery;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Result of merging another graph into this one.
/// Maps old NodeIds (from the source graph) to new NodeIds (in the target graph).
pub type MergeMap = HashMap<NodeId, NodeId>;

/// Unique identifier for a node in the code graph
pub type NodeId = NodeIndex;

/// The core code dependency graph
///
/// This graph represents the structure and relationships of a codebase:
/// - Nodes represent code entities (files, functions, classes, types, etc.)
/// - Edges represent relationships (imports, calls, inheritance, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraph {
    /// The underlying directed graph
    graph: DiGraph<Node, Edge>,

    /// Index for fast node lookup by file path and name
    node_index: HashMap<String, Vec<NodeId>>,

    /// Root directory of the analyzed codebase
    root_path: PathBuf,
}

impl CodeGraph {
    /// Create a new empty code graph
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            graph: DiGraph::new(),
            node_index: HashMap::new(),
            root_path,
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, node: Node) -> NodeId {
        let node_id = self.graph.add_node(node.clone());

        // Update the index
        let key = format!("{}:{}", node.file_path().display(), node.name());
        self.node_index.entry(key).or_default().push(node_id);

        node_id
    }

    /// Add an edge between two nodes
    pub fn add_edge(&mut self, from: NodeId, to: NodeId, edge: Edge) {
        self.graph.add_edge(from, to, edge);
    }

    /// Get a node by its ID
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.graph.node_weight(id)
    }

    /// Get a mutable reference to a node
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.graph.node_weight_mut(id)
    }

    /// Find nodes by file path and optional name pattern
    pub fn find_nodes(&self, file_path: &Path, name_pattern: Option<&str>) -> Vec<NodeId> {
        if let Some(pattern) = name_pattern {
            let key = format!("{}:{}", file_path.display(), pattern);
            self.node_index.get(&key).cloned().unwrap_or_default()
        } else {
            // Return all nodes in the file
            self.node_index
                .iter()
                .filter(|(k, _)| k.starts_with(&format!("{}:", file_path.display())))
                .flat_map(|(_, ids)| ids.clone())
                .collect()
        }
    }

    /// Get all nodes in the graph
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.graph
            .node_indices()
            .map(move |id| (id, &self.graph[id]))
    }

    /// Get all edges from a node
    pub fn edges_from(&self, node: NodeId) -> impl Iterator<Item = (NodeId, &Edge)> + '_ {
        self.graph
            .edges(node)
            .map(|edge| (edge.target(), edge.weight()))
    }

    /// Get all edges to a node (reverse dependencies)
    pub fn edges_to(&self, node: NodeId) -> Vec<(NodeId, &Edge)> {
        self.graph
            .node_indices()
            .flat_map(|source| {
                self.graph
                    .edges(source)
                    .filter(move |edge| edge.target() == node)
                    .map(move |edge| (source, edge.weight()))
            })
            .collect()
    }

    /// Remove a node from the graph
    pub fn remove_node(&mut self, node: NodeId) -> Option<Node> {
        if let Some(node_data) = self.graph.node_weight(node) {
            let key = format!("{}:{}", node_data.file_path().display(), node_data.name());
            if let Some(ids) = self.node_index.get_mut(&key) {
                ids.retain(|&id| id != node);
            }
        }
        self.graph.remove_node(node)
    }

    /// Get the root path of the codebase
    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }

    /// Get the underlying petgraph
    pub fn inner_graph(&self) -> &DiGraph<Node, Edge> {
        &self.graph
    }

    /// Merge another graph into this one.
    ///
    /// All nodes and edges from `other` are added to `self`. Node IDs are
    /// remapped so there are no collisions. The returned [`MergeMap`] maps
    /// old IDs (in `other`) to their new IDs (in `self`).
    ///
    /// This is the key enabler for parallel parse-then-merge: each thread
    /// builds its own small `CodeGraph`, then they are merged sequentially
    /// into one authoritative graph with zero contention during the expensive
    /// parse phase.
    pub fn merge(&mut self, other: CodeGraph) -> MergeMap {
        let mut id_map: MergeMap = HashMap::new();

        // 1. Re-add all nodes from `other`
        for old_id in other.graph.node_indices() {
            let node = other.graph[old_id].clone();
            let new_id = self.add_node(node);
            id_map.insert(old_id, new_id);
        }

        // 2. Re-add all edges, remapping source/target
        for old_edge in other.graph.edge_indices() {
            if let Some((src, tgt)) = other.graph.edge_endpoints(old_edge) {
                let new_src = id_map[&src];
                let new_tgt = id_map[&tgt];
                let edge = other.graph[old_edge].clone();
                self.add_edge(new_src, new_tgt, edge);
            }
        }

        id_map
    }

    /// Get a query interface for complex graph operations
    pub fn query(&self) -> GraphQuery<'_> {
        GraphQuery::new(self)
    }
}
