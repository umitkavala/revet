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

#[cfg(test)]
mod tests {
    use super::*;
    use revet_core::{Finding, Severity};
    use std::path::PathBuf;

    fn make_finding(
        id: &str,
        severity: Severity,
        message: &str,
        file: &str,
        line: usize,
    ) -> Finding {
        Finding {
            id: id.to_string(),
            severity,
            message: message.to_string(),
            file: PathBuf::from(file),
            line,
            affected_dependents: 0,
            suggestion: None,
            fix_kind: None,
        }
    }

    #[test]
    fn test_empty_findings() {
        let log = build_sarif_log(&[], Path::new("/repo"));
        assert_eq!(log.version, "2.1.0");
        assert_eq!(log.runs.len(), 1);
        assert!(log.runs[0].results.is_empty());
        assert!(log.runs[0].tool.driver.rules.is_empty());

        let json = serde_json::to_string_pretty(&log).unwrap();
        assert!(json.contains("\"version\": \"2.1.0\""));
    }

    #[test]
    fn test_single_finding() {
        let findings = vec![make_finding(
            "SEC-001",
            Severity::Error,
            "Hardcoded AWS key",
            "/repo/src/config.py",
            42,
        )];
        let log = build_sarif_log(&findings, Path::new("/repo"));

        assert_eq!(log.runs[0].results.len(), 1);
        let result = &log.runs[0].results[0];
        assert_eq!(result.rule_id, "SEC");
        assert_eq!(result.level, "error");
        assert_eq!(result.message.text, "Hardcoded AWS key");
        assert_eq!(
            result.locations[0].physical_location.artifact_location.uri,
            "src/config.py"
        );
        assert_eq!(result.locations[0].physical_location.region.start_line, 42);
    }

    #[test]
    fn test_severity_mapping() {
        let findings = vec![
            make_finding("SEC-001", Severity::Error, "err", "/repo/a.py", 1),
            make_finding("SEC-002", Severity::Warning, "warn", "/repo/b.py", 2),
            make_finding("SEC-003", Severity::Info, "info", "/repo/c.py", 3),
        ];
        let log = build_sarif_log(&findings, Path::new("/repo"));

        assert_eq!(log.runs[0].results[0].level, "error");
        assert_eq!(log.runs[0].results[1].level, "warning");
        assert_eq!(log.runs[0].results[2].level, "note");
    }

    #[test]
    fn test_rules_deduplication() {
        let findings = vec![
            make_finding("SEC-001", Severity::Error, "a", "/repo/a.py", 1),
            make_finding("SEC-002", Severity::Warning, "b", "/repo/b.py", 2),
            make_finding("SQL-001", Severity::Error, "c", "/repo/c.py", 3),
            make_finding("SEC-003", Severity::Info, "d", "/repo/d.py", 4),
            make_finding("SQL-002", Severity::Warning, "e", "/repo/e.py", 5),
        ];
        let log = build_sarif_log(&findings, Path::new("/repo"));

        assert_eq!(log.runs[0].tool.driver.rules.len(), 2);
        assert_eq!(log.runs[0].tool.driver.rules[0].id, "SEC");
        assert_eq!(log.runs[0].tool.driver.rules[1].id, "SQL");
    }

    #[test]
    fn test_file_path_normalization() {
        let findings = vec![make_finding(
            "IMPACT-001",
            Severity::Warning,
            "change",
            "/home/user/project/src/lib/utils.rs",
            10,
        )];
        let log = build_sarif_log(&findings, Path::new("/home/user/project"));

        assert_eq!(
            log.runs[0].results[0].locations[0]
                .physical_location
                .artifact_location
                .uri,
            "src/lib/utils.rs"
        );
        assert_eq!(
            log.runs[0].results[0].locations[0]
                .physical_location
                .artifact_location
                .uri_base_id,
            "%SRCROOT%"
        );
    }

    #[test]
    fn test_rule_index_matches() {
        let findings = vec![
            make_finding("ML-001", Severity::Warning, "a", "/repo/a.py", 1),
            make_finding("SEC-001", Severity::Error, "b", "/repo/b.py", 2),
            make_finding("ML-002", Severity::Info, "c", "/repo/c.py", 3),
        ];
        let log = build_sarif_log(&findings, Path::new("/repo"));

        // BTreeMap ordering: ML=0, SEC=1
        let rules = &log.runs[0].tool.driver.rules;
        assert_eq!(rules[0].id, "ML");
        assert_eq!(rules[1].id, "SEC");

        assert_eq!(log.runs[0].results[0].rule_index, 0); // ML
        assert_eq!(log.runs[0].results[1].rule_index, 1); // SEC
        assert_eq!(log.runs[0].results[2].rule_index, 0); // ML
    }

    #[test]
    fn test_roundtrip_serialization() {
        let findings = vec![
            make_finding(
                "SEC-001",
                Severity::Error,
                "secret found",
                "/repo/src/main.rs",
                15,
            ),
            make_finding(
                "SQL-001",
                Severity::Warning,
                "sql issue",
                "/repo/src/db.rs",
                30,
            ),
        ];
        let log = build_sarif_log(&findings, Path::new("/repo"));

        let json = serde_json::to_string_pretty(&log).unwrap();
        let deserialized: SarifLog = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.version, "2.1.0");
        assert_eq!(deserialized.runs.len(), 1);
        assert_eq!(deserialized.runs[0].results.len(), 2);
        assert_eq!(deserialized.runs[0].tool.driver.rules.len(), 2);
        assert_eq!(deserialized.runs[0].results[0].rule_id, "SEC");
        assert_eq!(deserialized.runs[0].results[1].rule_id, "SQL");
    }

    #[test]
    fn test_unknown_prefix() {
        let findings = vec![make_finding(
            "CUSTOM-001",
            Severity::Info,
            "custom finding",
            "/repo/file.txt",
            1,
        )];
        let log = build_sarif_log(&findings, Path::new("/repo"));

        assert_eq!(log.runs[0].tool.driver.rules[0].id, "CUSTOM");
        assert_eq!(
            log.runs[0].tool.driver.rules[0].short_description.text,
            "Code review finding"
        );
    }
}
