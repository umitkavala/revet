//! JSON output formatter.

use serde::{Deserialize, Serialize};

use revet_core::{BlastRadiusSummary, Finding, ReviewSummary, SuppressedFinding};
use std::path::Path;
use std::time::Duration;

use super::OutputFormatter;

// ── JSON document structs (kept public for tests) ─────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blast_radius: Option<BlastRadiusSummary>,
    pub findings: Vec<JsonFinding>,
    pub summary: JsonSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonFinding {
    pub id: String,
    pub severity: String,
    pub message: String,
    pub file: String,
    pub line: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
}

// ── Formatter struct ─────────────────────────────────────────────────────────

/// Accumulates findings in memory and serialises the whole JSON document on
/// [`finalize`](OutputFormatter::finalize).
pub struct JsonFormatter {
    blast_radius: Option<BlastRadiusSummary>,
    findings: Vec<JsonFinding>,
    summary: JsonSummary,
}

impl JsonFormatter {
    pub fn new() -> Self {
        Self {
            blast_radius: None,
            findings: Vec::new(),
            summary: JsonSummary {
                errors: 0,
                warnings: 0,
                info: 0,
            },
        }
    }
}

impl Default for JsonFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputFormatter for JsonFormatter {
    fn write_blast_radius(&mut self, summary: &BlastRadiusSummary) {
        self.blast_radius = Some(summary.clone());
    }

    fn write_finding(&mut self, finding: &Finding, _repo_path: &Path) {
        self.findings.push(JsonFinding {
            id: finding.id.clone(),
            severity: finding.severity.to_string(),
            message: finding.message.clone(),
            file: finding.file.display().to_string(),
            line: finding.line,
        });
    }

    fn write_summary(
        &mut self,
        summary: &ReviewSummary,
        _suppressed: &[SuppressedFinding],
        _elapsed: Duration,
        _run_id: Option<&str>,
    ) {
        self.summary = JsonSummary {
            errors: summary.errors,
            warnings: summary.warnings,
            info: summary.info,
        };
    }

    fn write_no_files(&mut self, _elapsed: Duration) {
        // Leave findings empty and summary zeroed — finalize will emit valid JSON.
    }

    fn finalize(&mut self) {
        let out = JsonOutput {
            blast_radius: self.blast_radius.take(),
            findings: std::mem::take(&mut self.findings),
            summary: JsonSummary {
                errors: self.summary.errors,
                warnings: self.summary.warnings,
                info: self.summary.info,
            },
        };
        match serde_json::to_string_pretty(&out) {
            Ok(json) => println!("{}", json),
            Err(e) => eprintln!("Failed to serialize JSON: {}", e),
        }
    }
}
