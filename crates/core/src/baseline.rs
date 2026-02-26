//! Baseline/suppression — snapshot findings so only new ones are reported

use crate::suppress::SuppressedFinding;
use crate::Finding;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

const BASELINE_FILE: &str = ".revet-cache/baseline.json";

/// A single baselined finding, keyed by file + message (line-independent).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct BaselineEntry {
    pub file: String,
    pub message: String,
}

/// Full baseline document stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub version: String,
    pub created_at: String,
    pub commit: Option<String>,
    pub count: usize,
    pub entries: Vec<BaselineEntry>,
}

impl Baseline {
    /// Build a baseline from a set of findings, relativizing paths against `repo_root`.
    pub fn from_findings(findings: &[Finding], repo_root: &Path, commit: Option<String>) -> Self {
        let entries: Vec<BaselineEntry> = findings
            .iter()
            .map(|f| BaselineEntry {
                file: f
                    .file
                    .strip_prefix(repo_root)
                    .unwrap_or(&f.file)
                    .to_string_lossy()
                    .into_owned(),
                message: f.message.clone(),
            })
            .collect();

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| format!("{}", d.as_secs()))
            .unwrap_or_default();

        Baseline {
            version: "1".to_string(),
            created_at: now,
            commit,
            count: entries.len(),
            entries,
        }
    }

    /// Save the baseline to `.revet-cache/baseline.json`.
    pub fn save(&self, repo_root: &Path) -> Result<()> {
        let path = repo_root.join(BASELINE_FILE);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating cache dir {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self).context("serializing baseline")?;
        fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Load a baseline from disk, returning `None` if the file doesn't exist.
    pub fn load(repo_root: &Path) -> Result<Option<Self>> {
        let path = repo_root.join(BASELINE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let data =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let baseline: Baseline =
            serde_json::from_str(&data).with_context(|| format!("parsing {}", path.display()))?;
        Ok(Some(baseline))
    }

    /// Delete the baseline file. Returns `true` if a file was actually removed.
    pub fn clear(repo_root: &Path) -> Result<bool> {
        let path = repo_root.join(BASELINE_FILE);
        if path.exists() {
            fs::remove_file(&path).with_context(|| format!("removing {}", path.display()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Filter findings against a baseline.
///
/// Returns `(new_findings, suppressed)`.
pub fn filter_findings(
    findings: Vec<Finding>,
    baseline: &Baseline,
    repo_root: &Path,
) -> (Vec<Finding>, Vec<SuppressedFinding>) {
    let lookup: HashSet<(&str, &str)> = baseline
        .entries
        .iter()
        .map(|e| (e.file.as_str(), e.message.as_str()))
        .collect();

    let mut new_findings = Vec::new();
    let mut suppressed: Vec<SuppressedFinding> = Vec::new();

    for f in findings {
        let rel = f
            .file
            .strip_prefix(repo_root)
            .unwrap_or(&f.file)
            .to_string_lossy();
        if lookup.contains(&(rel.as_ref(), f.message.as_str())) {
            suppressed.push(SuppressedFinding {
                finding: f,
                reason: "baseline".to_string(),
            });
        } else {
            new_findings.push(f);
        }
    }

    (new_findings, suppressed)
}
