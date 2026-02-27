//! Node.js bindings for Revet via NAPI-RS
//!
//! Exposes revet-core's domain analysis to JavaScript/TypeScript via N-API.
//! All analysis runs on a thread-pool task (AsyncTask) so it never blocks
//! the Node.js event loop.
//!
//! # JavaScript API
//!
//! ```js
//! const { analyzeRepository, analyzeFiles, analyzeGraph, suppress, getVersion } = require('./index');
//!
//! // Full repository scan
//! const result = await analyzeRepository('/path/to/repo');
//! console.log(result.summary);  // { total, errors, warnings, info, filesScanned }
//! result.findings.forEach(f => console.log(f.id, f.severity, f.message));
//!
//! // Targeted file scan
//! const result2 = await analyzeFiles(['/path/to/repo/src/auth.py'], '/path/to/repo');
//!
//! // Graph statistics
//! const stats = await analyzeGraph('/path/to/repo');
//! console.log(stats.nodeCount, stats.edgeCount);
//!
//! // Suppress a finding in .revet.toml
//! const added = await suppress('SEC-001', '/path/to/repo');
//! ```

#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use revet_core::{
    analyzer::AnalyzerDispatcher, cache::FileGraphCache, config::RevetConfig,
    discovery::discover_files_extended, finding::Severity, parser::ParserDispatcher,
};
use std::path::PathBuf;

// ── Shared output types ───────────────────────────────────────────────────────

/// Options for `analyzeRepository` and `analyzeFiles`.
#[napi(object)]
pub struct AnalyzeOptions {
    /// Reserved for future use. Currently all scans are full-repository scans.
    pub full: Option<bool>,
}

/// A single analysis finding.
#[napi(object)]
pub struct JsFinding {
    /// Finding identifier, e.g. `"SEC-001"`.
    pub id: String,
    /// Severity: `"error"`, `"warning"`, or `"info"`.
    pub severity: String,
    /// Human-readable description of the finding.
    pub message: String,
    /// File path relative to the repository root.
    pub file: String,
    /// 1-indexed line number.
    pub line: u32,
    /// Optional remediation hint.
    pub suggestion: Option<String>,
}

/// High-level counts returned alongside findings.
#[napi(object)]
pub struct AnalyzeSummary {
    pub total: u32,
    pub errors: u32,
    pub warnings: u32,
    pub info: u32,
    pub files_scanned: u32,
}

/// Return value of `analyzeRepository` and `analyzeFiles`.
#[napi(object)]
pub struct AnalyzeResult {
    pub findings: Vec<JsFinding>,
    pub summary: AnalyzeSummary,
}

/// Statistics about the code graph for a repository.
#[napi(object)]
pub struct GraphStats {
    /// Total number of nodes (files, functions, classes, imports, …).
    pub node_count: u32,
    /// Total number of edges (calls, imports, contains, …).
    pub edge_count: u32,
    /// Number of source files parsed or loaded from cache.
    pub files_scanned: u32,
    /// Number of files that could not be parsed.
    pub parse_errors: u32,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn to_js_findings(
    findings: &[revet_core::finding::Finding],
    repo_path: &PathBuf,
) -> Vec<JsFinding> {
    findings
        .iter()
        .map(|f| {
            let severity_str = match f.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            };
            JsFinding {
                id: f.id.clone(),
                severity: severity_str.to_string(),
                message: f.message.clone(),
                file: f
                    .file
                    .strip_prefix(repo_path)
                    .unwrap_or(&f.file)
                    .to_string_lossy()
                    .to_string(),
                line: f.line as u32,
                suggestion: f.suggestion.clone(),
            }
        })
        .collect()
}

fn summarize(findings: &[revet_core::finding::Finding], files_scanned: u32) -> AnalyzeSummary {
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count() as u32;
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count() as u32;
    let info = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count() as u32;
    AnalyzeSummary {
        total: findings.len() as u32,
        errors,
        warnings,
        info,
        files_scanned,
    }
}

fn canonicalize_repo(path: &str) -> napi::Result<PathBuf> {
    std::fs::canonicalize(path).map_err(|e| {
        napi::Error::from_reason(format!("Cannot resolve repository path '{}': {}", path, e))
    })
}

// ── analyzeRepository ─────────────────────────────────────────────────────────

pub struct AnalyzeTask {
    repo_path: String,
}

impl Task for AnalyzeTask {
    type Output = AnalyzeResult;
    type JsValue = AnalyzeResult;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        run_full_analysis(&self.repo_path)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

fn run_full_analysis(path: &str) -> napi::Result<AnalyzeResult> {
    let repo_path = canonicalize_repo(path)?;
    let config = RevetConfig::find_and_load(&repo_path).unwrap_or_default();

    let parser_dispatcher = ParserDispatcher::new();
    let analyzer_dispatcher = AnalyzerDispatcher::new_with_config(&config);

    let parser_exts: Vec<&str> = parser_dispatcher.supported_extensions();
    let extra_exts: Vec<&str> = analyzer_dispatcher.extra_extensions(&config);
    let extra_names: Vec<&str> = analyzer_dispatcher.extra_filenames(&config);

    let mut all_extensions: Vec<&str> = parser_exts;
    for ext in &extra_exts {
        if !all_extensions.contains(ext) {
            all_extensions.push(ext);
        }
    }

    let files = discover_files_extended(
        &repo_path,
        &all_extensions,
        &extra_names,
        &config.ignore.paths,
    )
    .map_err(|e| napi::Error::from_reason(format!("File discovery failed: {}", e)))?;

    let files_scanned = files.len() as u32;
    let findings = analyzer_dispatcher.run_all_parallel(&files, &repo_path, &config);

    Ok(AnalyzeResult {
        findings: to_js_findings(&findings, &repo_path),
        summary: summarize(&findings, files_scanned),
    })
}

/// Scan a repository and return all findings from enabled domain analyzers.
///
/// Runs on a thread-pool task — returns a `Promise<AnalyzeResult>`.
/// Config is loaded from `.revet.toml` in the repository root (or defaults).
///
/// @param repoPath - Absolute or relative path to the repository root.
/// @param options  - Optional scan options (currently unused, reserved for future).
#[napi(js_name = "analyzeRepository")]
pub fn analyze_repository(
    repo_path: String,
    _options: Option<AnalyzeOptions>,
) -> AsyncTask<AnalyzeTask> {
    AsyncTask::new(AnalyzeTask { repo_path })
}

// ── analyzeFiles ──────────────────────────────────────────────────────────────

pub struct AnalyzeFilesTask {
    files: Vec<String>,
    repo_root: String,
}

impl Task for AnalyzeFilesTask {
    type Output = AnalyzeResult;
    type JsValue = AnalyzeResult;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        run_files_analysis(&self.files, &self.repo_root)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

fn run_files_analysis(files: &[String], root: &str) -> napi::Result<AnalyzeResult> {
    let repo_path = canonicalize_repo(root)?;
    let config = RevetConfig::find_and_load(&repo_path).unwrap_or_default();
    let analyzer_dispatcher = AnalyzerDispatcher::new_with_config(&config);

    let paths: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();

    let files_scanned = paths.len() as u32;
    let findings = analyzer_dispatcher.run_all_parallel(&paths, &repo_path, &config);

    Ok(AnalyzeResult {
        findings: to_js_findings(&findings, &repo_path),
        summary: summarize(&findings, files_scanned),
    })
}

/// Run domain analyzers on a specific list of files.
///
/// Useful for editor integrations or incremental CI checks where only changed
/// files need to be re-scanned. Config is loaded from `.revet.toml` under
/// `repoRoot` (or defaults if absent).
///
/// @param files    - Array of file paths (absolute or relative) to analyze.
/// @param repoRoot - Repository root for config loading and path relativization.
/// @param options  - Optional scan options (reserved for future use).
#[napi(js_name = "analyzeFiles")]
pub fn analyze_files(
    files: Vec<String>,
    repo_root: String,
    _options: Option<AnalyzeOptions>,
) -> AsyncTask<AnalyzeFilesTask> {
    AsyncTask::new(AnalyzeFilesTask { files, repo_root })
}

// ── analyzeGraph ──────────────────────────────────────────────────────────────

pub struct AnalyzeGraphTask {
    repo_path: String,
}

impl Task for AnalyzeGraphTask {
    type Output = GraphStats;
    type JsValue = GraphStats;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        run_graph_analysis(&self.repo_path)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

fn run_graph_analysis(path: &str) -> napi::Result<GraphStats> {
    let repo_path = canonicalize_repo(path)?;
    let config = RevetConfig::find_and_load(&repo_path).unwrap_or_default();
    let parser_dispatcher = ParserDispatcher::new();

    let parser_exts: Vec<&str> = parser_dispatcher.supported_extensions();
    let files = discover_files_extended(&repo_path, &parser_exts, &[], &config.ignore.paths)
        .map_err(|e| napi::Error::from_reason(format!("File discovery failed: {}", e)))?;

    let files_scanned = files.len() as u32;

    let cache_dir = repo_path.join(".revet-cache");
    let file_cache = FileGraphCache::new(&cache_dir);

    let (graph, errors, _cached, _parsed) =
        parser_dispatcher.parse_files_incremental(&files, repo_path, &file_cache);

    let node_count = graph.nodes().count() as u32;
    let edge_count = graph.inner_graph().edge_count() as u32;

    Ok(GraphStats {
        node_count,
        edge_count,
        files_scanned,
        parse_errors: errors.len() as u32,
    })
}

/// Parse the repository and return code graph statistics.
///
/// Uses the incremental parser with on-disk cache (`.revet-cache/`) for speed.
/// Returns node/edge counts useful for dependency analysis dashboards.
///
/// @param repoPath - Absolute or relative path to the repository root.
#[napi(js_name = "analyzeGraph")]
pub fn analyze_graph(repo_path: String) -> AsyncTask<AnalyzeGraphTask> {
    AsyncTask::new(AnalyzeGraphTask { repo_path })
}

// ── suppress ─────────────────────────────────────────────────────────────────

pub struct SuppressTask {
    finding_id: String,
    repo_path: String,
}

impl Task for SuppressTask {
    type Output = bool;
    type JsValue = bool;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        run_suppress(&self.finding_id, &self.repo_path)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

fn run_suppress(finding_id: &str, path: &str) -> napi::Result<bool> {
    let repo_path = canonicalize_repo(path)?;
    let toml_path = repo_path.join(".revet.toml");

    // Read existing TOML or start from empty table
    let raw = if toml_path.exists() {
        std::fs::read_to_string(&toml_path)
            .map_err(|e| napi::Error::from_reason(format!("Cannot read .revet.toml: {}", e)))?
    } else {
        String::new()
    };

    let mut doc: toml::Table = raw
        .parse()
        .map_err(|e| napi::Error::from_reason(format!("Cannot parse .revet.toml: {}", e)))?;

    // Navigate/create [ignore].findings
    let ignore = doc
        .entry("ignore")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));

    let findings_arr = ignore
        .as_table_mut()
        .ok_or_else(|| napi::Error::from_reason("[ignore] is not a table".to_string()))?
        .entry("findings")
        .or_insert_with(|| toml::Value::Array(Vec::new()));

    let arr = findings_arr
        .as_array_mut()
        .ok_or_else(|| napi::Error::from_reason("[ignore].findings is not an array".to_string()))?;

    // Check for duplicate
    let already_present = arr.iter().any(|v| v.as_str() == Some(finding_id));

    if already_present {
        return Ok(false);
    }

    arr.push(toml::Value::String(finding_id.to_string()));

    let serialized = toml::to_string_pretty(&doc)
        .map_err(|e| napi::Error::from_reason(format!("Cannot serialize .revet.toml: {}", e)))?;

    std::fs::write(&toml_path, serialized)
        .map_err(|e| napi::Error::from_reason(format!("Cannot write .revet.toml: {}", e)))?;

    Ok(true)
}

/// Add a finding ID to `[ignore].findings` in `.revet.toml`.
///
/// Creates `.revet.toml` if it does not exist. Returns `true` if the ID was
/// added, `false` if it was already present (idempotent).
///
/// @param findingId - Finding ID to suppress, e.g. `"SEC-001"`.
/// @param repoPath  - Repository root where `.revet.toml` lives (or will be created).
#[napi(js_name = "suppress")]
pub fn suppress_finding(finding_id: String, repo_path: String) -> AsyncTask<SuppressTask> {
    AsyncTask::new(SuppressTask {
        finding_id,
        repo_path,
    })
}

// ── getVersion ────────────────────────────────────────────────────────────────

/// Return the revet-core library version string.
#[napi(js_name = "getVersion")]
pub fn get_version() -> String {
    revet_core::VERSION.to_string()
}
