//! Cross-file impact analysis

use crate::graph::{CodeGraph, NodeId};
use serde::{Deserialize, Serialize};

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
    old_graph: CodeGraph,
    new_graph: CodeGraph,
}

impl ImpactAnalysis {
    /// Create a new impact analyzer with old and new graphs
    pub fn new(old_graph: CodeGraph, new_graph: CodeGraph) -> Self {
        Self { old_graph, new_graph }
    }

    /// Find all changed nodes by comparing old and new graphs
    pub fn find_changed_nodes(&self) -> Vec<(NodeId, Option<NodeId>)> {
        let mut changed = Vec::new();

        // Find nodes by file path and name
        for (new_node_id, new_node) in self.new_graph.nodes() {
            // Look for corresponding node in old graph
            let old_node_id = self.find_matching_node_in_old(&new_node);

            if let Some(old_id) = old_node_id {
                // Node exists in both - check if it changed
                if let Some(old_node) = self.old_graph.node(old_id) {
                    if self.node_changed(old_node, new_node) {
                        changed.push((new_node_id, Some(old_id)));
                    }
                }
            } else {
                // New node (added)
                changed.push((new_node_id, None));
            }
        }

        changed
    }

    /// Find a matching node in the old graph by file path and name
    fn find_matching_node_in_old(&self, new_node: &crate::graph::Node) -> Option<NodeId> {
        // Look through old graph for node with same path and name
        for (old_id, old_node) in self.old_graph.nodes() {
            if old_node.file_path() == new_node.file_path() 
                && old_node.name() == new_node.name()
                && old_node.kind() == new_node.kind() {
                return Some(old_id);
            }
        }
        None
    }

    /// Check if a node has changed between old and new versions
    fn node_changed(&self, old_node: &crate::graph::Node, new_node: &crate::graph::Node) -> bool {
        // Compare node data
        old_node.data() != new_node.data() || old_node.line() != new_node.line()
    }

    /// Analyze the impact of changes
    pub fn analyze_impact(&self) -> ImpactReport {
        let mut report = ImpactReport::new();
        let changed_nodes = self.find_changed_nodes();

        for (new_node_id, old_node_id) in changed_nodes {
            let classification = self.classify_change(new_node_id, old_node_id);

            // Find direct dependents in the NEW graph
            let direct_deps = self.new_graph.query().direct_dependents(new_node_id);

            // Find transitive dependents (up to depth 3) in the NEW graph
            let transitive_deps = self.new_graph.query().transitive_dependents(new_node_id, Some(3));

            report.add_changed_node(new_node_id, classification, direct_deps, transitive_deps);
        }

        report
    }

    /// Classify a change by comparing old and new node versions
    fn classify_change(&self, new_node_id: NodeId, old_node_id: Option<NodeId>) -> ChangeClassification {
        let new_node = match self.new_graph.node(new_node_id) {
            Some(n) => n,
            None => return ChangeClassification::Safe,
        };

        // If no old node, this is a new addition
        let old_node = match old_node_id.and_then(|id| self.old_graph.node(id)) {
            Some(n) => n,
            None => return ChangeClassification::Safe, // New additions are safe
        };

        // Compare based on node type
        match (old_node.data(), new_node.data()) {
            (
                crate::graph::NodeData::Function { parameters: old_params, return_type: old_ret },
                crate::graph::NodeData::Function { parameters: new_params, return_type: new_ret },
            ) => self.compare_function_signatures(old_params, old_ret, new_params, new_ret),

            (
                crate::graph::NodeData::Class { base_classes: old_bases, methods: old_methods, fields: old_fields },
                crate::graph::NodeData::Class { base_classes: new_bases, methods: new_methods, fields: new_fields },
            ) => self.compare_classes(old_bases, old_methods, old_fields, new_bases, new_methods, new_fields),

            (crate::graph::NodeData::Type { .. }, crate::graph::NodeData::Type { .. }) => {
                // Type changes are always breaking
                ChangeClassification::Breaking
            }

            (
                crate::graph::NodeData::Variable { is_constant: true, .. },
                crate::graph::NodeData::Variable { .. }
            ) => {
                // Constant changes are breaking
                ChangeClassification::Breaking
            }

            _ => ChangeClassification::Safe,
        }
    }

    /// Compare function signatures to detect breaking changes
    fn compare_function_signatures(
        &self,
        old_params: &[crate::graph::Parameter],
        old_return_type: &Option<String>,
        new_params: &[crate::graph::Parameter],
        new_return_type: &Option<String>,
    ) -> ChangeClassification {
        // Return type changed
        if old_return_type != new_return_type {
            return ChangeClassification::Breaking;
        }

        // Number of parameters changed
        if old_params.len() != new_params.len() {
            // If new parameters are added at the end with defaults, it might be safe
            if new_params.len() > old_params.len() {
                let new_params_have_defaults = new_params[old_params.len()..]
                    .iter()
                    .all(|p| p.default_value.is_some());
                
                if new_params_have_defaults {
                    return ChangeClassification::PotentiallyBreaking;
                }
            }
            return ChangeClassification::Breaking;
        }

        // Compare each parameter
        for (old_param, new_param) in old_params.iter().zip(new_params.iter()) {
            // Parameter name changed (could affect keyword arguments)
            if old_param.name != new_param.name {
                return ChangeClassification::Breaking;
            }

            // Parameter type changed
            if old_param.param_type != new_param.param_type {
                return ChangeClassification::Breaking;
            }

            // Default value added or removed
            if old_param.default_value.is_some() != new_param.default_value.is_some() {
                return ChangeClassification::PotentiallyBreaking;
            }
        }

        // No breaking changes detected
        ChangeClassification::Safe
    }

    /// Compare classes to detect breaking changes
    fn compare_classes(
        &self,
        old_bases: &[String],
        old_methods: &[String],
        old_fields: &[String],
        new_bases: &[String],
        new_methods: &[String],
        new_fields: &[String],
    ) -> ChangeClassification {
        // Base classes changed (inheritance)
        if old_bases != new_bases {
            return ChangeClassification::Breaking;
        }

        // Check for removed methods (breaking)
        for old_method in old_methods {
            if !new_methods.contains(old_method) {
                return ChangeClassification::Breaking;
            }
        }

        // Check for removed fields (breaking)
        for old_field in old_fields {
            if !new_fields.contains(old_field) {
                return ChangeClassification::Breaking;
            }
        }

        // Check for added methods/fields (safe)
        let methods_added = new_methods.len() > old_methods.len();
        let fields_added = new_fields.len() > old_fields.len();

        if methods_added || fields_added {
            return ChangeClassification::Safe; // Additions are safe
        }

        ChangeClassification::Safe
    }

    /// Get the new graph reference
    pub fn new_graph(&self) -> &CodeGraph {
        &self.new_graph
    }

    /// Get the old graph reference
    pub fn old_graph(&self) -> &CodeGraph {
        &self.old_graph
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
    use crate::graph::{Node, NodeData, NodeKind, Edge, EdgeKind};
    use std::path::PathBuf;

    #[test]
    fn test_impact_analysis_basic() {
        // Create old graph
        let mut old_graph = CodeGraph::new(PathBuf::from("/test"));

        let old_func_a = old_graph.add_node(Node::new(
            NodeKind::Function,
            "func_a".to_string(),
            PathBuf::from("a.py"),
            1,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        ));

        let old_func_b = old_graph.add_node(Node::new(
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
        old_graph.add_edge(old_func_b, old_func_a, Edge::new(EdgeKind::Calls));

        // Create new graph with modified func_a
        let mut new_graph = CodeGraph::new(PathBuf::from("/test"));

        let new_func_a = new_graph.add_node(Node::new(
            NodeKind::Function,
            "func_a".to_string(),
            PathBuf::from("a.py"),
            1,
            NodeData::Function {
                parameters: vec![crate::graph::Parameter {
                    name: "x".to_string(),
                    param_type: Some("int".to_string()),
                    default_value: None,
                }],
                return_type: Some("int".to_string()),
            },
        ));

        let new_func_b = new_graph.add_node(Node::new(
            NodeKind::Function,
            "func_b".to_string(),
            PathBuf::from("b.py"),
            1,
            NodeData::Function {
                parameters: vec![],
                return_type: None,
            },
        ));

        new_graph.add_edge(new_func_b, new_func_a, Edge::new(EdgeKind::Calls));

        let analyzer = ImpactAnalysis::new(old_graph, new_graph);
        let report = analyzer.analyze_impact();

        // Should detect func_a as changed
        assert!(report.changes.len() >= 1);
        
        // Should classify as breaking (signature changed)
        let breaking_changes: Vec<_> = report.breaking_changes().collect();
        assert!(breaking_changes.len() >= 1);
        
        // Should find func_b as a dependent
        let func_a_impact = &report.changes.iter()
            .find(|c| {
                analyzer.new_graph().node(c.node_id)
                    .map(|n| n.name() == "func_a")
                    .unwrap_or(false)
            });
        
        if let Some(impact) = func_a_impact {
            assert!(impact.direct_dependents.len() >= 1);
        }
    }

    #[test]
    fn test_compare_function_signatures() {
        let old_graph = CodeGraph::new(PathBuf::from("/test"));
        let new_graph = CodeGraph::new(PathBuf::from("/test"));
        let analyzer = ImpactAnalysis::new(old_graph, new_graph);

        // Test 1: Adding parameter without default = BREAKING
        let old_params = vec![];
        let new_params = vec![crate::graph::Parameter {
            name: "x".to_string(),
            param_type: Some("int".to_string()),
            default_value: None,
        }];

        let result = analyzer.compare_function_signatures(&old_params, &None, &new_params, &None);
        assert_eq!(result, ChangeClassification::Breaking);

        // Test 2: Adding parameter with default = POTENTIALLY BREAKING
        let new_params_with_default = vec![crate::graph::Parameter {
            name: "x".to_string(),
            param_type: Some("int".to_string()),
            default_value: Some("0".to_string()),
        }];

        let result = analyzer.compare_function_signatures(&old_params, &None, &new_params_with_default, &None);
        assert_eq!(result, ChangeClassification::PotentiallyBreaking);

        // Test 3: Return type changed = BREAKING
        let result = analyzer.compare_function_signatures(&old_params, &None, &old_params, &Some("int".to_string()));
        assert_eq!(result, ChangeClassification::Breaking);

        // Test 4: No changes = SAFE
        let result = analyzer.compare_function_signatures(&old_params, &None, &old_params, &None);
        assert_eq!(result, ChangeClassification::Safe);
    }

    #[test]
    fn test_compare_classes() {
        let old_graph = CodeGraph::new(PathBuf::from("/test"));
        let new_graph = CodeGraph::new(PathBuf::from("/test"));
        let analyzer = ImpactAnalysis::new(old_graph, new_graph);

        let old_bases = vec!["Base".to_string()];
        let old_methods = vec!["method_a".to_string(), "method_b".to_string()];
        let old_fields = vec!["field_x".to_string()];

        // Test 1: Removed method = BREAKING
        let new_methods_removed = vec!["method_a".to_string()];
        let result = analyzer.compare_classes(
            &old_bases, &old_methods, &old_fields,
            &old_bases, &new_methods_removed, &old_fields
        );
        assert_eq!(result, ChangeClassification::Breaking);

        // Test 2: Added method = SAFE
        let new_methods_added = vec!["method_a".to_string(), "method_b".to_string(), "method_c".to_string()];
        let result = analyzer.compare_classes(
            &old_bases, &old_methods, &old_fields,
            &old_bases, &new_methods_added, &old_fields
        );
        assert_eq!(result, ChangeClassification::Safe);

        // Test 3: Changed base class = BREAKING
        let new_bases = vec!["NewBase".to_string()];
        let result = analyzer.compare_classes(
            &old_bases, &old_methods, &old_fields,
            &new_bases, &old_methods, &old_fields
        );
        assert_eq!(result, ChangeClassification::Breaking);

        // Test 4: Removed field = BREAKING
        let new_fields_removed = vec![];
        let result = analyzer.compare_classes(
            &old_bases, &old_methods, &old_fields,
            &old_bases, &old_methods, &new_fields_removed
        );
        assert_eq!(result, ChangeClassification::Breaking);
    }
}
