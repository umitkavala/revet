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

/// How a finding can be automatically fixed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixKind {
    /// Comment out the offending line with a FIXME annotation
    CommentOut,
    /// Replace a regex pattern on the offending line
    ReplacePattern { find: String, replace: String },
    /// Suggestion only — no auto-fix available
    Suggestion,
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

    /// Human-readable remediation suggestion
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,

    /// How this finding can be auto-fixed (None = no fix metadata)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_kind: Option<FixKind>,
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

impl ReviewSummary {
    /// Check whether findings exceed the configured severity threshold.
    ///
    /// - `"error"` → fail if errors > 0
    /// - `"warning"` → fail if errors or warnings > 0
    /// - `"info"` → fail if any findings
    /// - `"never"` → always pass
    pub fn exceeds_threshold(&self, fail_on: &str) -> bool {
        match fail_on {
            "error" => self.errors > 0,
            "warning" => self.errors > 0 || self.warnings > 0,
            "info" => self.errors > 0 || self.warnings > 0 || self.info > 0,
            "never" => false,
            _ => self.errors > 0, // default to "error" for unknown values
        }
    }
}
