//! Graph query operations for impact analysis and dependency traversal

use super::{CodeGraph, EdgeKind, NodeId};
use std::collections::{HashSet, VecDeque};

/// A query interface for complex graph operations
pub struct GraphQuery<'a> {
    graph: &'a CodeGraph,
}

impl<'a> GraphQuery<'a> {
    pub fn new(graph: &'a CodeGraph) -> Self {
        Self { graph }
    }

    /// Find all nodes that directly depend on the given node
    pub fn direct_dependents(&self, node: NodeId) -> Vec<NodeId> {
        self.graph
            .edges_to(node)
            .into_iter()
            .map(|(source, _)| source)
            .collect()
    }

    /// Find all nodes that transitively depend on the given node
    /// (i.e., all nodes reachable by following reverse edges)
    pub fn transitive_dependents(&self, node: NodeId, max_depth: Option<usize>) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((node, 0));
        visited.insert(node);

        while let Some((current, depth)) = queue.pop_front() {
            if let Some(max) = max_depth {
                if depth >= max {
                    continue;
                }
            }

            for (dependent, _) in self.graph.edges_to(current) {
                if visited.insert(dependent) {
                    result.push(dependent);
                    queue.push_back((dependent, depth + 1));
                }
            }
        }

        result
    }

    /// Find all nodes that the given node depends on
    pub fn dependencies(&self, node: NodeId) -> Vec<NodeId> {
        self.graph
            .edges_from(node)
            .map(|(target, _)| target)
            .collect()
    }

    /// Find all nodes that the given node transitively depends on
    pub fn transitive_dependencies(&self, node: NodeId, max_depth: Option<usize>) -> Vec<NodeId> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        queue.push_back((node, 0));
        visited.insert(node);

        while let Some((current, depth)) = queue.pop_front() {
            if let Some(max) = max_depth {
                if depth >= max {
                    continue;
                }
            }

            for (dependency, _) in self.graph.edges_from(current) {
                if visited.insert(dependency) {
                    result.push(dependency);
                    queue.push_back((dependency, depth + 1));
                }
            }
        }

        result
    }

    /// Find all paths from one node to another
    pub fn find_paths(&self, from: NodeId, to: NodeId, max_length: usize) -> Vec<Vec<NodeId>> {
        let mut paths = Vec::new();
        let mut current_path = vec![from];
        let mut visited = HashSet::new();
        visited.insert(from);

        self.find_paths_recursive(
            from,
            to,
            max_length,
            &mut current_path,
            &mut visited,
            &mut paths,
        );

        paths
    }

    fn find_paths_recursive(
        &self,
        current: NodeId,
        target: NodeId,
        max_length: usize,
        current_path: &mut Vec<NodeId>,
        visited: &mut HashSet<NodeId>,
        paths: &mut Vec<Vec<NodeId>>,
    ) {
        if current == target {
            paths.push(current_path.clone());
            return;
        }

        if current_path.len() >= max_length {
            return;
        }

        for (next, _) in self.graph.edges_from(current) {
            if !visited.contains(&next) {
                visited.insert(next);
                current_path.push(next);

                self.find_paths_recursive(next, target, max_length, current_path, visited, paths);

                current_path.pop();
                visited.remove(&next);
            }
        }
    }

    /// Find all nodes of a specific edge kind from the given node
    pub fn find_by_edge_kind(&self, node: NodeId, kind: EdgeKind) -> Vec<NodeId> {
        self.graph
            .edges_from(node)
            .filter(|(_, edge)| edge.kind() == &kind)
            .map(|(target, _)| target)
            .collect()
    }

    /// Find all nodes that reach this node via a specific edge kind
    pub fn find_by_edge_kind_reverse(&self, node: NodeId, kind: EdgeKind) -> Vec<NodeId> {
        self.graph
            .edges_to(node)
            .into_iter()
            .filter(|(_, edge)| edge.kind() == &kind)
            .map(|(source, _)| source)
            .collect()
    }
}
