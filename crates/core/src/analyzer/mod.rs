//! Domain analyzers for detecting patterns beyond AST-level analysis
//!
//! Analyzers operate on raw file content (not the code graph) and produce
//! [`Finding`]s. Each analyzer is independent and can be enabled/disabled
//! via `.revet.toml`.

pub mod async_patterns;
pub mod circular_imports;
pub mod complexity;
pub mod custom_rules;
pub mod dead_imports;
pub mod dependency;
pub mod error_handling;
pub mod infra;
pub mod ml_pipeline;
pub mod react_hooks;
pub mod secret_exposure;
pub mod sql_injection;
pub mod unused_exports;

use crate::config::RevetConfig;
use crate::finding::{Finding, FixKind, Severity};
use crate::graph::CodeGraph;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

/// Trait for domain-specific analyzers
///
/// Analyzers scan raw file content for patterns that don't require AST parsing
/// (e.g., hardcoded secrets, configuration issues).
pub trait Analyzer: Send + Sync {
    /// Human-readable name of this analyzer
    fn name(&self) -> &str;

    /// Finding ID prefix (e.g., "SEC" produces "SEC-001", "SEC-002", ...)
    fn finding_prefix(&self) -> &str;

    /// Whether this analyzer is enabled given the current config
    fn is_enabled(&self, config: &RevetConfig) -> bool;

    /// Analyze the given files and return findings
    ///
    /// `repo_root` is the absolute path to the repository root, used to
    /// produce relative file paths in findings.
    fn analyze_files(&self, files: &[PathBuf], repo_root: &Path) -> Vec<Finding>;

    /// Additional file extensions this analyzer needs beyond parser extensions.
    /// Returns extensions with leading dot (e.g., `[".tf", ".yaml"]`).
    fn extra_extensions(&self) -> &[&str] {
        &[]
    }

    /// Additional exact filenames this analyzer needs (e.g., `["Dockerfile"]`).
    fn extra_filenames(&self) -> &[&str] {
        &[]
    }
}

/// Trait for graph-based analyzers that query the CodeGraph.
pub trait GraphAnalyzer: Send + Sync {
    /// Human-readable name of this analyzer
    fn name(&self) -> &str;

    /// Finding ID prefix (e.g., "DEAD" produces "DEAD-001", "DEAD-002", ...)
    fn finding_prefix(&self) -> &str;

    /// Whether this analyzer is enabled given the current config
    fn is_enabled(&self, config: &RevetConfig) -> bool;

    /// Analyze the code graph and return findings
    fn analyze_graph(&self, graph: &CodeGraph, config: &RevetConfig) -> Vec<Finding>;
}

/// Dispatches analysis across all registered analyzers
pub struct AnalyzerDispatcher {
    analyzers: Vec<Box<dyn Analyzer>>,
    graph_analyzers: Vec<Box<dyn GraphAnalyzer>>,
}

impl AnalyzerDispatcher {
    /// Create a new dispatcher with all built-in analyzers
    pub fn new() -> Self {
        Self {
            analyzers: vec![
                Box::new(secret_exposure::SecretExposureAnalyzer::new()),
                Box::new(sql_injection::SqlInjectionAnalyzer::new()),
                Box::new(ml_pipeline::MlPipelineAnalyzer::new()),
                Box::new(infra::InfraAnalyzer::new()),
                Box::new(react_hooks::ReactHooksAnalyzer::new()),
                Box::new(async_patterns::AsyncPatternsAnalyzer::new()),
                Box::new(dependency::DependencyAnalyzer::new()),
                Box::new(error_handling::ErrorHandlingAnalyzer::new()),
            ],
            graph_analyzers: vec![
                Box::new(unused_exports::UnusedExportsAnalyzer::new()),
                Box::new(circular_imports::CircularImportsAnalyzer::new()),
                Box::new(complexity::ComplexityAnalyzer::new()),
                Box::new(dead_imports::DeadImportsAnalyzer::new()),
            ],
        }
    }

    /// Create a dispatcher with built-in analyzers plus custom rules from config
    pub fn new_with_config(config: &RevetConfig) -> Self {
        let mut dispatcher = Self::new();
        let custom = custom_rules::CustomRulesAnalyzer::from_config(config);
        if custom.is_enabled(config) {
            dispatcher.analyzers.push(Box::new(custom));
        }
        dispatcher
    }

    /// Run all enabled graph analyzers and return combined findings.
    ///
    /// Finding IDs are renumbered per-prefix to ensure sequential ordering
    /// (e.g., DEAD-001, DEAD-002, ...).
    pub fn run_graph_analyzers(&self, graph: &CodeGraph, config: &RevetConfig) -> Vec<Finding> {
        let mut all_findings = Vec::new();

        for analyzer in &self.graph_analyzers {
            if !analyzer.is_enabled(config) {
                continue;
            }

            let mut findings = analyzer.analyze_graph(graph, config);
            let prefix = analyzer.finding_prefix();

            for (i, finding) in findings.iter_mut().enumerate() {
                finding.id = format!("{}-{:03}", prefix, i + 1);
            }

            let findings: Vec<Finding> = findings
                .into_iter()
                .filter(|f| !config.ignore.findings.contains(&f.id))
                .collect();

            all_findings.extend(findings);
        }

        all_findings
    }

    /// Collect extra file extensions needed by enabled analyzers.
    /// Returns extensions with leading dot (e.g., `".tf"`).
    pub fn extra_extensions(&self, config: &RevetConfig) -> Vec<&str> {
        let mut exts = Vec::new();
        for analyzer in &self.analyzers {
            if analyzer.is_enabled(config) {
                exts.extend_from_slice(analyzer.extra_extensions());
            }
        }
        exts.sort();
        exts.dedup();
        exts
    }

    /// Collect extra filenames needed by enabled analyzers (e.g., `"Dockerfile"`).
    pub fn extra_filenames(&self, config: &RevetConfig) -> Vec<&str> {
        let mut names = Vec::new();
        for analyzer in &self.analyzers {
            if analyzer.is_enabled(config) {
                names.extend_from_slice(analyzer.extra_filenames());
            }
        }
        names.sort();
        names.dedup();
        names
    }

    /// Run all enabled analyzers and return combined findings
    ///
    /// Finding IDs are renumbered per-prefix to ensure sequential ordering
    /// (e.g., SEC-001, SEC-002, ...).
    pub fn run_all(
        &self,
        files: &[PathBuf],
        repo_root: &Path,
        config: &RevetConfig,
    ) -> Vec<Finding> {
        let mut all_findings = Vec::new();

        for analyzer in &self.analyzers {
            if !analyzer.is_enabled(config) {
                continue;
            }

            let mut findings = analyzer.analyze_files(files, repo_root);
            let prefix = analyzer.finding_prefix();

            // Renumber finding IDs sequentially
            for (i, finding) in findings.iter_mut().enumerate() {
                finding.id = format!("{}-{:03}", prefix, i + 1);
            }

            // Filter out suppressed findings
            let findings: Vec<Finding> = findings
                .into_iter()
                .filter(|f| !config.ignore.findings.contains(&f.id))
                .collect();

            all_findings.extend(findings);
        }

        all_findings
    }

    /// Run all enabled analyzers in parallel and return combined findings.
    ///
    /// Each analyzer runs on its own rayon task. Finding IDs are renumbered
    /// per-prefix after collection to ensure sequential ordering.
    pub fn run_all_parallel(
        &self,
        files: &[PathBuf],
        repo_root: &Path,
        config: &RevetConfig,
    ) -> Vec<Finding> {
        // Collect enabled analyzers
        let enabled: Vec<&dyn Analyzer> = self
            .analyzers
            .iter()
            .filter(|a| a.is_enabled(config))
            .map(|a| &**a)
            .collect();

        // Run all analyzers in parallel
        let per_analyzer: Vec<(String, Vec<Finding>)> = enabled
            .par_iter()
            .map(|analyzer| {
                let findings = analyzer.analyze_files(files, repo_root);
                (analyzer.finding_prefix().to_string(), findings)
            })
            .collect();

        // Sequential post-processing: renumber and filter
        let mut all_findings = Vec::new();
        for (prefix, mut findings) in per_analyzer {
            for (i, finding) in findings.iter_mut().enumerate() {
                finding.id = format!("{}-{:03}", prefix, i + 1);
            }
            let findings: Vec<Finding> = findings
                .into_iter()
                .filter(|f| !config.ignore.findings.contains(&f.id))
                .collect();
            all_findings.extend(findings);
        }

        all_findings
    }
}

impl Default for AnalyzerDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create a finding with common defaults
pub(crate) fn make_finding(
    severity: Severity,
    message: String,
    file: PathBuf,
    line: usize,
    suggestion: Option<String>,
    fix_kind: Option<FixKind>,
) -> Finding {
    Finding {
        id: String::new(), // Renumbered by dispatcher
        severity,
        message,
        file,
        line,
        affected_dependents: 0,
        suggestion,
        fix_kind,
    }
}
