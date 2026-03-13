//! PR blast radius summary — scope and risk scoring for a diff.

use crate::diff::impact::ImpactReport;
use crate::graph::CodeGraph;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Risk level for a PR based on how many callers are affected and whether
/// changes cross module/package boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "LOW"),
            RiskLevel::Medium => write!(f, "MEDIUM"),
            RiskLevel::High => write!(f, "HIGH"),
        }
    }
}

/// At-a-glance summary of a PR's blast radius, shown before individual findings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusSummary {
    /// Number of symbols directly modified in the diff
    pub directly_modified: usize,
    /// Number of unique transitive call-site nodes affected across all changed symbols
    pub transitively_affected: usize,
    /// How many affected nodes live in a different top-level module than their changed symbol
    pub cross_module_crossings: usize,
    /// Overall risk classification
    pub risk: RiskLevel,
}

impl BlastRadiusSummary {
    /// Compute the blast radius from an impact report and the new code graph.
    pub fn from_impact_report(report: &ImpactReport, graph: &CodeGraph, repo_root: &Path) -> Self {
        let directly_modified = report.changes.len();

        // Collect all unique affected node IDs across all changes
        let mut all_affected: HashSet<crate::graph::NodeId> = HashSet::new();
        for change in &report.changes {
            for &id in &change.direct_dependents {
                all_affected.insert(id);
            }
            for &id in &change.transitive_dependents {
                all_affected.insert(id);
            }
        }
        let transitively_affected = all_affected.len();

        // Detect cross-module crossings: an affected node is cross-module if its
        // top-level path component differs from the changed symbol's component.
        let mut cross_module_crossings = 0usize;
        for change in &report.changes {
            let changed_node = match graph.node(change.node_id) {
                Some(n) => n,
                None => continue,
            };
            let changed_module = top_level_component(changed_node.file_path(), repo_root);

            let all_deps = change
                .direct_dependents
                .iter()
                .chain(change.transitive_dependents.iter());

            for &dep_id in all_deps {
                if let Some(dep_node) = graph.node(dep_id) {
                    let dep_module = top_level_component(dep_node.file_path(), repo_root);
                    if dep_module != changed_module {
                        cross_module_crossings += 1;
                        break; // count once per changed symbol
                    }
                }
            }
        }

        let risk = score_risk(transitively_affected, cross_module_crossings);

        Self {
            directly_modified,
            transitively_affected,
            cross_module_crossings,
            risk,
        }
    }
}

/// Extract the first path component relative to repo root (e.g. "crates" or "src").
fn top_level_component(abs_path: &Path, repo_root: &Path) -> String {
    abs_path
        .strip_prefix(repo_root)
        .unwrap_or(abs_path)
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Compute risk level from counts.
///
/// Thresholds:
/// - LOW:    ≤ 5 affected AND zero cross-module crossings
/// - HIGH:   > 20 affected with any cross-module, OR > 50 affected regardless
/// - MEDIUM: everything else
fn score_risk(affected: usize, cross_module: usize) -> RiskLevel {
    if affected > 50 || (affected > 20 && cross_module > 0) {
        RiskLevel::High
    } else if affected > 5 || cross_module > 0 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}
