//! Node.js bindings for Revet via NAPI-RS
//!
//! Exposes revet-core's domain analysis to JavaScript/TypeScript via N-API.
//! All analysis runs on a thread-pool task (AsyncTask) so it never blocks
//! the Node.js event loop.
//!
//! # JavaScript API
//!
//! ```js
//! const { analyzeRepository, getVersion } = require('./index');
//!
//! const result = await analyzeRepository('/path/to/repo');
//! console.log(result.summary);  // { total, errors, warnings, info, filesScanned }
//! result.findings.forEach(f => console.log(f.id, f.severity, f.message));
//! ```

#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use revet_core::{
    analyzer::AnalyzerDispatcher, config::RevetConfig, discovery::discover_files_extended,
    finding::Severity, parser::ParserDispatcher,
};

// ── Public types (auto-generate TypeScript declarations) ─────────────────────

/// Options for `analyzeRepository`.
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

/// Return value of `analyzeRepository`.
#[napi(object)]
pub struct AnalyzeResult {
    pub findings: Vec<JsFinding>,
    pub summary: AnalyzeSummary,
}

// ── Async task ────────────────────────────────────────────────────────────────

pub struct AnalyzeTask {
    repo_path: String,
}

impl Task for AnalyzeTask {
    type Output = AnalyzeResult;
    type JsValue = AnalyzeResult;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        run_analysis(&self.repo_path)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

fn run_analysis(path: &str) -> napi::Result<AnalyzeResult> {
    let raw = std::path::Path::new(path);
    let repo_path = std::fs::canonicalize(raw).map_err(|e| {
        napi::Error::from_reason(format!("Cannot resolve repository path '{}': {}", path, e))
    })?;

    // Load config from .revet.toml (falls back to defaults if not found)
    let config = RevetConfig::find_and_load(&repo_path).unwrap_or_default();

    // Build dispatchers
    let parser_dispatcher = ParserDispatcher::new();
    let analyzer_dispatcher = AnalyzerDispatcher::new_with_config(&config);

    // Merge parser extensions (.py, .js, ...) with analyzer-specific extras (.sh, .tf, ...)
    let parser_exts: Vec<&str> = parser_dispatcher.supported_extensions();
    let extra_exts: Vec<&str> = analyzer_dispatcher.extra_extensions(&config);
    let extra_names: Vec<&str> = analyzer_dispatcher.extra_filenames(&config);

    let mut all_extensions: Vec<&str> = parser_exts;
    for ext in &extra_exts {
        if !all_extensions.contains(ext) {
            all_extensions.push(ext);
        }
    }

    // Discover files, respecting .gitignore and config ignore patterns
    let files = discover_files_extended(
        &repo_path,
        &all_extensions,
        &extra_names,
        &config.ignore.paths,
    )
    .map_err(|e| napi::Error::from_reason(format!("File discovery failed: {}", e)))?;

    let files_scanned = files.len() as u32;

    // Run domain analyzers in parallel (rayon)
    let findings = analyzer_dispatcher.run_all_parallel(&files, &repo_path, &config);

    // Build JS-friendly finding objects
    let js_findings: Vec<JsFinding> = findings
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
                    .strip_prefix(&repo_path)
                    .unwrap_or(&f.file)
                    .to_string_lossy()
                    .to_string(),
                line: f.line as u32,
                suggestion: f.suggestion.clone(),
            }
        })
        .collect();

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
    let total = findings.len() as u32;

    Ok(AnalyzeResult {
        findings: js_findings,
        summary: AnalyzeSummary {
            total,
            errors,
            warnings,
            info,
            files_scanned,
        },
    })
}

// ── Exported functions ────────────────────────────────────────────────────────

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

/// Return the revet-core library version string.
#[napi(js_name = "getVersion")]
pub fn get_version() -> String {
    revet_core::VERSION.to_string()
}
