//! Test coverage gaps analyzer — detects public symbols with no test coverage.
//!
//! Uses the code graph to find top-level functions and classes defined in
//! non-test source files, then checks whether any test file in the repo
//! references each symbol by name. Symbols whose names never appear in a test
//! file are reported as potential coverage gaps.
//!
//! This is a heuristic, not a precise coverage tool. It catches the obvious
//! case where a public symbol has zero test mentions, which is a strong signal
//! of missing tests. False positives can be suppressed per-finding in
//! `.revet.toml`.
//!
//! Disabled by default (`modules.test_coverage = false`).

use crate::analyzer::GraphAnalyzer;
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use crate::graph::{CodeGraph, EdgeKind, NodeId, NodeKind};
use std::collections::HashSet;
use std::path::Path;

// ── Heuristics ────────────────────────────────────────────────────────────────

/// Symbol names that are universal entry points / lifecycle hooks — skip them.
const SKIP_NAMES: &[&str] = &[
    "main", "new", "default", "init", "__init__", "__main__", "index", "handler", "setup",
    "teardown", "setUp", "tearDown",
];

/// Minimum symbol name length — very short names produce too many false positives.
const MIN_NAME_LEN: usize = 3;

/// Path components that identify a test file.
const TEST_MARKERS: &[&str] = &[
    "/test/",
    "/tests/",
    "/spec/",
    "/specs/",
    "/__tests__/",
    "_test.",
    ".test.",
    "_spec.",
    ".spec.",
    "test_",
];

fn is_test_file(path: &Path) -> bool {
    let s = path.to_string_lossy();
    // Check the filename itself for test markers too
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    TEST_MARKERS.iter().any(|m| s.contains(m))
        || file_name.starts_with("test_")
        || file_name.starts_with("spec_")
        || file_name.ends_with("_test.rs")
        || file_name.ends_with("_test.go")
        || file_name.ends_with("_spec.rb")
        || file_name.ends_with(".test.ts")
        || file_name.ends_with(".test.js")
        || file_name.ends_with(".spec.ts")
        || file_name.ends_with(".spec.js")
}

fn is_top_level(graph: &CodeGraph, node_id: NodeId) -> bool {
    graph.edges_to(node_id).iter().any(|(src, e)| {
        matches!(e.kind(), EdgeKind::Contains)
            && matches!(graph.node(*src).map(|n| n.kind()), Some(NodeKind::File))
    })
}

// ── Analyzer ──────────────────────────────────────────────────────────────────

pub struct TestCoverageAnalyzer;

impl TestCoverageAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TestCoverageAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphAnalyzer for TestCoverageAnalyzer {
    fn name(&self) -> &str {
        "Test Coverage Gaps"
    }

    fn finding_prefix(&self) -> &str {
        "COV"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.test_coverage
    }

    fn analyze_graph(&self, graph: &CodeGraph, _config: &RevetConfig) -> Vec<Finding> {
        // 1. Partition File nodes into test files and source files
        let mut test_files: Vec<std::path::PathBuf> = Vec::new();

        for (_, node) in graph.nodes() {
            if !matches!(node.kind(), NodeKind::File) {
                continue;
            }
            let path = node.file_path();
            if is_test_file(path) {
                test_files.push(path.clone());
            }
        }

        // No test files at all — nothing to compare against, skip.
        if test_files.is_empty() {
            return Vec::new();
        }

        // 2. Read all test file content and collect every word that appears
        let mut tested_names: HashSet<String> = HashSet::new();
        for path in &test_files {
            if let Ok(content) = std::fs::read_to_string(path) {
                // Extract word-like tokens (identifiers) from the test file.
                // Using a simple split on non-alphanumeric/underscore characters.
                for word in content.split(|c: char| !c.is_alphanumeric() && c != '_') {
                    if word.len() >= MIN_NAME_LEN {
                        tested_names.insert(word.to_string());
                    }
                }
            }
        }

        // 3. Find public top-level functions/classes in non-test source files
        let mut findings = Vec::new();

        for (node_id, node) in graph.nodes() {
            let kind = node.kind();
            if !matches!(kind, NodeKind::Function | NodeKind::Class) {
                continue;
            }

            let name = node.name();

            // Skip private-by-convention names
            if name.starts_with('_') || name.starts_with("__") {
                continue;
            }

            // Skip universally known entry points and very short names
            if SKIP_NAMES.contains(&name) || name.len() < MIN_NAME_LEN {
                continue;
            }

            // Skip nodes in test files
            if is_test_file(node.file_path()) {
                continue;
            }

            // Must be directly under a File node (top-level, not nested)
            if !is_top_level(graph, node_id) {
                continue;
            }

            // Flag if name never appears in any test file
            if !tested_names.contains(name) {
                findings.push(Finding {
                    id: String::new(), // renumbered by dispatcher
                    severity: Severity::Info,
                    message: format!(
                        "`{}` ({:?}) has no test coverage — name not found in any test file",
                        name, kind
                    ),
                    file: node.file_path().clone(),
                    line: node.line(),
                    affected_dependents: 0,
                    suggestion: Some(format!(
                        "Add a test that exercises `{name}` to improve coverage"
                    )),
                    fix_kind: None,
                    ..Default::default()
                });
            }
        }

        findings
    }
}
