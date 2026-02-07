//! JSON output formatting

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonOutput {
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
