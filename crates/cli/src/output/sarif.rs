//! SARIF 2.1.0 output formatting
//!
//! Produces Static Analysis Results Interchange Format for GitHub Code Scanning
//! and IDE consumption (VS Code, IntelliJ).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use revet_core::{Finding, Severity};

// ── SARIF 2.1.0 structs ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLog {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifDriver {
    pub name: String,
    pub semantic_version: String,
    pub information_uri: String,
    pub rules: Vec<SarifReportingDescriptor>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifReportingDescriptor {
    pub id: String,
    pub short_description: SarifMessage,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifResult {
    pub rule_id: String,
    pub rule_index: usize,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifLocation {
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifPhysicalLocation {
    pub artifact_location: SarifArtifactLocation,
    pub region: SarifRegion,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifArtifactLocation {
    pub uri: String,
    pub uri_base_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifRegion {
    pub start_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SarifMessage {
    pub text: String,
}

// ── Rule descriptions ────────────────────────────────────────────

fn rule_description(prefix: &str) -> &'static str {
    match prefix {
        "SEC" => "Secret exposure detected",
        "SQL" => "SQL injection vulnerability",
        "ML" => "ML pipeline anti-pattern",
        "INFRA" => "Infrastructure misconfiguration",
        "IMPACT" => "Breaking change impact",
        "PARSE" => "Parse error",
        _ => "Code review finding",
    }
}

fn severity_to_level(severity: &Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

/// Extract the rule prefix from a finding ID (e.g. "SEC" from "SEC-001").
fn extract_prefix(id: &str) -> &str {
    id.split('-').next().unwrap_or(id)
}

/// Make a relative path with forward slashes from a potentially absolute file path.
fn relative_uri(file: &Path, repo_path: &Path) -> String {
    let rel = file.strip_prefix(repo_path).unwrap_or(file);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

// ── Public API ───────────────────────────────────────────────────

/// Build a complete SARIF 2.1.0 log from a list of findings.
pub fn build_sarif_log(findings: &[Finding], repo_path: &Path) -> SarifLog {
    // 1. Collect unique rule prefixes in stable order
    let mut prefix_set: BTreeMap<String, &'static str> = BTreeMap::new();
    for f in findings {
        let prefix = extract_prefix(&f.id).to_string();
        prefix_set
            .entry(prefix.clone())
            .or_insert_with(|| rule_description(&prefix));
    }

    // 2. Build rules array
    let rules: Vec<SarifReportingDescriptor> = prefix_set
        .iter()
        .map(|(prefix, desc)| SarifReportingDescriptor {
            id: prefix.clone(),
            short_description: SarifMessage {
                text: desc.to_string(),
            },
        })
        .collect();

    // Build prefix → index lookup
    let prefix_index: BTreeMap<&str, usize> = prefix_set
        .keys()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();

    // 3. Build results array
    let results: Vec<SarifResult> = findings
        .iter()
        .map(|f| {
            let prefix = extract_prefix(&f.id);
            let rule_index = prefix_index.get(prefix).copied().unwrap_or(0);

            SarifResult {
                rule_id: prefix.to_string(),
                rule_index,
                level: severity_to_level(&f.severity).to_string(),
                message: SarifMessage {
                    text: f.message.clone(),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: relative_uri(&f.file, repo_path),
                            uri_base_id: "%SRCROOT%".to_string(),
                        },
                        region: SarifRegion {
                            start_line: f.line.max(1),
                        },
                    },
                }],
            }
        })
        .collect();

    // 4. Assemble
    SarifLog {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json".to_string(),
        version: "2.1.0".to_string(),
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "Revet".to_string(),
                    semantic_version: revet_core::VERSION.to_string(),
                    information_uri: "https://github.com/anthropics/revet".to_string(),
                    rules,
                },
            },
            results,
        }],
    }
}
