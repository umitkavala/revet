//! Unused exports analyzer — detects top-level symbols with no callers or references.
//!
//! Reports symbols (functions, classes, variables) that are exported from a file but
//! never imported or called by any other file in the graph.

use crate::analyzer::GraphAnalyzer;
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use crate::graph::{CodeGraph, EdgeKind, NodeId, NodeKind};

/// Names commonly used as entry points — never flagged as unused.
const ENTRY_POINT_NAMES: &[&str] = &[
    "main", "__init__", "__main__", "new", "index", "handler", "default",
];

pub struct UnusedExportsAnalyzer;

impl UnusedExportsAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

/// Returns true if `node_id` has a `Contains` edge incoming from a `File` node.
fn is_top_level(graph: &CodeGraph, node_id: NodeId) -> bool {
    graph.edges_to(node_id).iter().any(|(src, e)| {
        matches!(e.kind(), EdgeKind::Contains)
            && matches!(graph.node(*src).map(|n| n.kind()), Some(NodeKind::File))
    })
}

/// Returns true if any node has a `Calls` or `References` edge pointing to `node_id`.
fn has_callers(graph: &CodeGraph, node_id: NodeId) -> bool {
    graph
        .edges_to(node_id)
        .iter()
        .any(|(_, e)| matches!(e.kind(), EdgeKind::Calls | EdgeKind::References))
}

impl GraphAnalyzer for UnusedExportsAnalyzer {
    fn name(&self) -> &str {
        "Unused Exports"
    }

    fn finding_prefix(&self) -> &str {
        "DEAD"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.dead_code
    }

    fn analyze_graph(&self, graph: &CodeGraph, _config: &RevetConfig) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (node_id, node) in graph.nodes() {
            let kind = node.kind();
            if !matches!(
                kind,
                NodeKind::Function | NodeKind::Class | NodeKind::Variable
            ) {
                continue;
            }

            if ENTRY_POINT_NAMES.contains(&node.name()) {
                continue;
            }

            if !is_top_level(graph, node_id) {
                continue;
            }

            if has_callers(graph, node_id) {
                continue;
            }

            findings.push(Finding {
                id: String::new(), // renumbered by dispatcher
                severity: Severity::Warning,
                message: format!(
                    "Exported `{}` ({:?}) has no callers or references",
                    node.name(),
                    kind
                ),
                file: node.file_path().clone(),
                line: node.line(),
                affected_dependents: 0,
                suggestion: Some(
                    "Remove this symbol or add an import/call to suppress this warning".to_string(),
                ),
                fix_kind: None,
            });
        }

        findings
    }
}
