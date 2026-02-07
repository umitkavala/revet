//! Cross-file impact analysis

use crate::graph::{CodeGraph, NodeId, NodeKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Classifies the type and severity of a code change
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeClassification {
    /// Breaking change - signature or contract changed
    Breaking,
    /// Potentially breaking - behavior may have changed
    PotentiallyBreaking,
    /// Safe change - new addition or internal implementation
    Safe,
}

/// Analyzes the impact of code changes across the codebase
pub struct ImpactAnalysis {
    graph: CodeGraph,
}

impl ImpactAnalysis {
    /// Create a new impact analyzer
    pub fn new(graph: CodeGraph) -> Self {
        Self { graph }
    }

    /// Analyze the impact of changed nodes
    pub fn analyze_impact(&self, changed_nodes: &[NodeId]) -> ImpactReport {
        let mut report = ImpactReport::new();

        for &node_id in changed_nodes {
            let classification = self.classify_change(node_id);

            // Find direct dependents
            let direct_deps = self.graph.query().direct_dependents(node_id);

            // Find transitive dependents (up to depth 3)
            let transitive_deps = self.graph.query().transitive_dependents(node_id, Some(3));

            report.add_changed_node(node_id, classification, direct_deps, transitive_deps);
        }

        report
    }

    /// Classify a change based on the node type and change characteristics
    fn classify_change(&self, node_id: NodeId) -> ChangeClassification {
        let node = match self.graph.node(node_id) {
            Some(n) => n,
            None => return ChangeClassification::Safe,
        };

        match node.kind() {
            NodeKind::Function => {
                // TODO: Compare old and new signatures
                // For now, conservatively mark as potentially breaking
                ChangeClassification::PotentiallyBreaking
            }
            NodeKind::Class | NodeKind::Interface => {
                // Class/interface changes are often breaking
                ChangeClassification::Breaking
            }
            NodeKind::Type => ChangeClassification::Breaking,
            NodeKind::Variable if matches!(node.data(), crate::graph::NodeData::Variable { is_constant: true, .. }) => {
                ChangeClassification::Breaking
            }
            _ => ChangeClassification::Safe,
        }
    }

    /// Get the graph reference
    pub fn graph(&self) -> &CodeGraph {
        &self.graph
    }
}

/// Report of impact analysis
#[derive(Debug, Clone)]
pub struct ImpactReport {
    /// Changed nodes and their impacts
    pub changes: Vec<ChangeImpact>,

    /// Summary statistics
    pub summary: ImpactSummary,
}

impl ImpactReport {
    fn new() -> Self {
        Self {
            changes: Vec::new(),
            summary: ImpactSummary::default(),
        }
    }

    fn add_changed_node(
        &mut self,
        node_id: NodeId,
        classification: ChangeClassification,
        direct_dependents: Vec<NodeId>,
        transitive_dependents: Vec<NodeId>,
    ) {
        // Update summary before moving values
        let total_affected = direct_dependents.len() + transitive_dependents.len();

        self.changes.push(ChangeImpact {
            node_id,
            classification,
            direct_dependents,
            transitive_dependents,
        });

        // Update summary
        match classification {
            ChangeClassification::Breaking => self.summary.breaking_changes += 1,
            ChangeClassification::PotentiallyBreaking => {
                self.summary.potentially_breaking_changes += 1
            }
            ChangeClassification::Safe => self.summary.safe_changes += 1,
        }

        self.summary.total_affected_nodes += total_affected;
    }

    /// Get all breaking changes
    pub fn breaking_changes(&self) -> impl Iterator<Item = &ChangeImpact> {
        self.changes
            .iter()
            .filter(|c| c.classification == ChangeClassification::Breaking)
    }

    /// Get all potentially breaking changes
    pub fn potentially_breaking_changes(&self) -> impl Iterator<Item = &ChangeImpact> {
        self.changes
            .iter()
            .filter(|c| c.classification == ChangeClassification::PotentiallyBreaking)
    }
}

/// Impact of a single change
#[derive(Debug, Clone)]
pub struct ChangeImpact {
    pub node_id: NodeId,
    pub classification: ChangeClassification,
    pub direct_dependents: Vec<NodeId>,
    pub transitive_dependents: Vec<NodeId>,
}

/// Summary statistics for an impact report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImpactSummary {
    pub breaking_changes: usize,
    pub potentially_breaking_changes: usize,
    pub safe_changes: usize,
    pub total_affected_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Node, NodeData, Edge, EdgeKind};
    use std::path::PathBuf;

    #[test]
    fn test_impact_analysis() {
        let mut graph = CodeGraph::new(PathBuf::from("/test"));

        let node_a = graph.add_node(Node::new(
            NodeKind::Function,
            "func_a".to_string(),
            PathBuf::from("a.py"),
            1,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        ));

        let node_b = graph.add_node(Node::new(
            NodeKind::Function,
            "func_b".to_string(),
            PathBuf::from("b.py"),
            1,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        ));

        // B depends on A
        graph.add_edge(node_b, node_a, Edge::new(EdgeKind::Calls));

        let analyzer = ImpactAnalysis::new(graph);
        let report = analyzer.analyze_impact(&[node_a]);

        assert_eq!(report.changes.len(), 1);
        assert_eq!(report.changes[0].direct_dependents.len(), 1);
    }
}
