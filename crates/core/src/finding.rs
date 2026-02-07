//! Finding types that bridge analysis results to output formatters

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Severity level of a finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "error"),
            Severity::Warning => write!(f, "warning"),
            Severity::Info => write!(f, "info"),
        }
    }
}

/// A single finding from analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Unique identifier (e.g. "IMPACT-001")
    pub id: String,

    /// Severity level
    pub severity: Severity,

    /// Human-readable message
    pub message: String,

    /// File where the finding was detected
    pub file: PathBuf,

    /// Line number in the file
    pub line: usize,

    /// Number of downstream dependents affected
    pub affected_dependents: usize,
}

/// Summary of an entire review run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReviewSummary {
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub files_analyzed: usize,
    pub nodes_parsed: usize,
}
