//! Run log — persists every review run to `.revet-cache/runs/<id>.json`.

use anyhow::{Context, Result};
use revet_core::{Finding, ReviewSummary, SuppressedFinding};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const RUNS_DIR: &str = ".revet-cache/runs";

// ── On-disk structures ───────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct RunLog {
    pub id: String,
    pub version: String,
    pub timestamp: u64,
    pub duration_secs: f64,
    pub files_analyzed: usize,
    pub nodes_parsed: usize,
    pub summary: RunSummary,
    pub findings: Vec<RunFinding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub suppressed: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RunFinding {
    pub id: String,
    pub severity: String,
    pub message: String,
    pub file: String,
    pub line: usize,
    pub suppressed: bool,
    pub suppression_reason: Option<String>,
}

/// A brief entry shown in `revet log` listings.
#[derive(Debug)]
pub struct RunEntry {
    pub id: String,
    pub path: PathBuf,
    pub timestamp: u64,
    pub files_analyzed: usize,
    pub findings_kept: usize,
    pub suppressed: usize,
    pub duration_secs: f64,
}

// ── Write ────────────────────────────────────────────────────────

/// Persist a completed review run to `.revet-cache/runs/<id>.json`.
///
/// The `id` is the millisecond Unix timestamp at the start of the run.
pub fn save_run_log(
    repo_path: &Path,
    id: &str,
    duration_secs: f64,
    findings: &[Finding],
    suppressed: &[SuppressedFinding],
    summary: &ReviewSummary,
    repo_root: &Path,
) -> Result<()> {
    let runs_dir = repo_path.join(RUNS_DIR);
    std::fs::create_dir_all(&runs_dir)
        .with_context(|| format!("create runs dir {}", runs_dir.display()))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut run_findings: Vec<RunFinding> = findings
        .iter()
        .map(|f| RunFinding {
            id: f.id.clone(),
            severity: f.severity.to_string(),
            message: f.message.clone(),
            file: f
                .file
                .strip_prefix(repo_root)
                .unwrap_or(&f.file)
                .display()
                .to_string(),
            line: f.line,
            suppressed: false,
            suppression_reason: None,
        })
        .collect();

    for sf in suppressed {
        run_findings.push(RunFinding {
            id: sf.finding.id.clone(),
            severity: sf.finding.severity.to_string(),
            message: sf.finding.message.clone(),
            file: sf
                .finding
                .file
                .strip_prefix(repo_root)
                .unwrap_or(&sf.finding.file)
                .display()
                .to_string(),
            line: sf.finding.line,
            suppressed: true,
            suppression_reason: Some(sf.reason.clone()),
        });
    }

    let log = RunLog {
        id: id.to_string(),
        version: revet_core::VERSION.to_string(),
        timestamp,
        duration_secs,
        files_analyzed: summary.files_analyzed,
        nodes_parsed: summary.nodes_parsed,
        summary: RunSummary {
            errors: summary.errors,
            warnings: summary.warnings,
            info: summary.info,
            suppressed: suppressed.len(),
        },
        findings: run_findings,
    };

    let path = runs_dir.join(format!("{}.json", id));
    let json = serde_json::to_string_pretty(&log)?;
    std::fs::write(&path, json).with_context(|| format!("write run log {}", path.display()))?;

    Ok(())
}

// ── Read ─────────────────────────────────────────────────────────

/// List all run log entries, sorted newest-first.
pub fn list_runs(repo_path: &Path) -> Result<Vec<RunEntry>> {
    let runs_dir = repo_path.join(RUNS_DIR);
    if !runs_dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&runs_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(log) = load_run_log_from_path(&path) {
            let findings_kept = log.findings.iter().filter(|f| !f.suppressed).count();
            entries.push(RunEntry {
                id: log.id,
                path,
                timestamp: log.timestamp,
                files_analyzed: log.files_analyzed,
                findings_kept,
                suppressed: log.summary.suppressed,
                duration_secs: log.duration_secs,
            });
        }
    }

    // Newest first
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(entries)
}

/// Load a run log by its ID from the given repo.
pub fn load_run_log(repo_path: &Path, id: &str) -> Result<RunLog> {
    let path = repo_path.join(RUNS_DIR).join(format!("{}.json", id));
    load_run_log_from_path(&path)
}

fn load_run_log_from_path(path: &Path) -> Result<RunLog> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("read run log {}", path.display()))?;
    let log: RunLog =
        serde_json::from_str(&json).with_context(|| format!("parse {}", path.display()))?;
    Ok(log)
}

/// Generate a run ID from the current time (millisecond Unix timestamp).
pub fn new_run_id() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
