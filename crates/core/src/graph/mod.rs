//! Code dependency graph data structures and operations

pub mod nodes;
pub mod edges;
pub mod query;

pub use nodes::{Node, NodeKind, NodeData, Parameter};
pub use edges::{Edge, EdgeKind, EdgeMetadata};
pub use query::GraphQuery;

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Serialize, Deserialize};

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
        self.node_index.entry(key).or_insert_with(Vec::new).push(node_id);

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
    pub fn find_nodes(&self, file_path: &PathBuf, name_pattern: Option<&str>) -> Vec<NodeId> {
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
        self.graph.node_indices().map(move |id| (id, &self.graph[id]))
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

    /// Get a query interface for complex graph operations
    pub fn query(&self) -> GraphQuery {
        GraphQuery::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_create_graph() {
        let graph = CodeGraph::new(PathBuf::from("/test"));
        assert_eq!(graph.root_path(), &PathBuf::from("/test"));
    }

    #[test]
    fn test_add_node() {
        let mut graph = CodeGraph::new(PathBuf::from("/test"));
        let node = Node::new(
            NodeKind::Function,
            "test_func".to_string(),
            PathBuf::from("/test/file.py"),
            10,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        );
        let id = graph.add_node(node);
        assert!(graph.node(id).is_some());
    }
}
