//! Circular imports analyzer — detects import cycles between files.
//!
//! Performs DFS cycle detection over `Imports` edges between `File` nodes.
//! Each unique cycle is reported once (canonicalized by rotating to the smallest NodeId).

use crate::analyzer::GraphAnalyzer;
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use crate::graph::{CodeGraph, EdgeKind, NodeId, NodeKind};
use std::collections::{HashMap, HashSet};

pub struct CircularImportsAnalyzer;

impl CircularImportsAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

/// DFS coloring states
#[derive(Clone, Copy, PartialEq)]
enum Color {
    White, // unvisited
    Gray,  // in current DFS stack
    Black, // fully visited
}

/// Find all import cycles among File nodes.
///
/// Returns a list of cycles; each cycle is a `Vec<NodeId>` of file nodes
/// in the order they form the cycle (not including the repeated start node).
fn find_import_cycles(graph: &CodeGraph) -> Vec<Vec<NodeId>> {
    // Collect only File node IDs
    let file_nodes: Vec<NodeId> = graph
        .nodes()
        .filter(|(_, n)| matches!(n.kind(), NodeKind::File))
        .map(|(id, _)| id)
        .collect();

    let mut color: HashMap<NodeId, Color> =
        file_nodes.iter().map(|&id| (id, Color::White)).collect();

    let mut cycles: Vec<Vec<NodeId>> = Vec::new();
    let mut seen_cycles: HashSet<Vec<NodeId>> = HashSet::new();

    for &start in &file_nodes {
        if color[&start] == Color::White {
            let mut stack: Vec<NodeId> = Vec::new();
            dfs(
                graph,
                start,
                &mut color,
                &mut stack,
                &mut cycles,
                &mut seen_cycles,
            );
        }
    }

    cycles
}

fn dfs(
    graph: &CodeGraph,
    node: NodeId,
    color: &mut HashMap<NodeId, Color>,
    stack: &mut Vec<NodeId>,
    cycles: &mut Vec<Vec<NodeId>>,
    seen_cycles: &mut HashSet<Vec<NodeId>>,
) {
    color.insert(node, Color::Gray);
    stack.push(node);

    // Follow Imports edges to other File nodes only
    let neighbors: Vec<NodeId> = graph
        .edges_from(node)
        .filter(|(target, edge)| {
            matches!(edge.kind(), EdgeKind::Imports)
                && matches!(graph.node(*target).map(|n| n.kind()), Some(NodeKind::File))
        })
        .map(|(target, _)| target)
        .collect();

    for neighbor in neighbors {
        match color.get(&neighbor).copied() {
            Some(Color::White) => {
                dfs(graph, neighbor, color, stack, cycles, seen_cycles);
            }
            Some(Color::Gray) => {
                // Found a cycle — extract the cycle from the stack
                if let Some(pos) = stack.iter().position(|&n| n == neighbor) {
                    let cycle = stack[pos..].to_vec();
                    let canonical = canonicalize_cycle(cycle);
                    if seen_cycles.insert(canonical.clone()) {
                        cycles.push(canonical);
                    }
                }
            }
            _ => {}
        }
    }

    stack.pop();
    color.insert(node, Color::Black);
}

/// Rotate cycle to start at the smallest NodeId for deduplication.
fn canonicalize_cycle(mut cycle: Vec<NodeId>) -> Vec<NodeId> {
    if cycle.is_empty() {
        return cycle;
    }
    let min_pos = cycle
        .iter()
        .enumerate()
        .min_by_key(|(_, &id)| id)
        .map(|(i, _)| i)
        .unwrap_or(0);
    cycle.rotate_left(min_pos);
    cycle
}

impl GraphAnalyzer for CircularImportsAnalyzer {
    fn name(&self) -> &str {
        "Circular Imports"
    }

    fn finding_prefix(&self) -> &str {
        "CYCLE"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.cycles
    }

    fn analyze_graph(&self, graph: &CodeGraph, _config: &RevetConfig) -> Vec<Finding> {
        let cycles = find_import_cycles(graph);
        let mut findings = Vec::new();

        for cycle in cycles {
            if cycle.is_empty() {
                continue;
            }

            // Report the finding on the first (canonical) node in the cycle
            let reporting_node_id = cycle[0];
            let node = match graph.node(reporting_node_id) {
                Some(n) => n,
                None => continue,
            };

            // Build a human-readable cycle description
            let cycle_path: Vec<String> = cycle
                .iter()
                .filter_map(|&id| graph.node(id))
                .map(|n| {
                    n.file_path()
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(n.name())
                        .to_string()
                })
                .collect();

            let cycle_str = if cycle_path.len() > 1 {
                format!("{} → {}", cycle_path.join(" → "), cycle_path[0])
            } else {
                format!("{} → {}", cycle_path[0], cycle_path[0])
            };

            findings.push(Finding {
                id: String::new(), // renumbered by dispatcher
                severity: Severity::Warning,
                message: format!("Circular import detected: {}", cycle_str),
                file: node.file_path().clone(),
                line: node.line(),
                affected_dependents: 0,
                suggestion: Some(
                    "Break the cycle by extracting shared code to a separate module".to_string(),
                ),
                fix_kind: None,
            });
        }

        findings
    }
}
