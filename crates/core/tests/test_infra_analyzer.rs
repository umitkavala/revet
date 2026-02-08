//! Integration tests for InfraAnalyzer

use revet_core::analyzer::infra::InfraAnalyzer;
use revet_core::analyzer::Analyzer;
use revet_core::config::RevetConfig;
use revet_core::finding::Severity;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper: create a temp file with given content and return its path
fn write_temp_file(dir: &TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

fn infra_enabled_config() -> RevetConfig {
    let mut config = RevetConfig::default();
    config.modules.infra = true;
    config
}

// ── Error-level: critical security issues ──────────────────────

#[test]
fn test_detects_public_s3_acl() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "s3.tf",
        r#"resource "aws_s3_bucket" "data" {
  acl = "public-read"
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("public S3 bucket ACL"));
    assert_eq!(findings[0].line, 2);
}

#[test]
fn test_detects_public_read_write_acl() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "s3.tf",
        r#"resource "aws_s3_bucket" "data" {
  acl = "public-read-write"
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("public S3 bucket ACL"));
}

#[test]
fn test_detects_open_security_group() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "sg.tf",
        r#"resource "aws_security_group_rule" "allow_all" {
  cidr_blocks = ["0.0.0.0/0"]
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("open security group"));
}

#[test]
fn test_detects_hardcoded_credentials() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "provider.tf",
        r#"provider "aws" {
  access_key = "AKIAIOSFODNN7EXAMPLE"
  secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.severity == Severity::Error));
    assert!(findings[0]
        .message
        .contains("hardcoded provider credentials"));
}

#[test]
fn test_no_error_variable_reference_credentials() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "provider.tf",
        r#"provider "aws" {
  access_key = var.aws_access_key
  secret_key = var.aws_secret_key
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Variable references should not trigger, got: {:?}",
        findings
    );
}

// ── Warning-level ──────────────────────────────────────────────

#[test]
fn test_detects_wildcard_iam_action_tf() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "iam.tf",
        r#"resource "aws_iam_policy" "admin" {
  actions = ["*"]
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("wildcard IAM action"));
}

#[test]
fn test_detects_wildcard_iam_action_json() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "policy.json",
        r#"{
  "Statement": [{
    "Action": "*",
    "Effect": "Allow"
  }]
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_no_warning_not_action() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "iam.tf",
        r#"  NotAction = ["*"]
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "NotAction should not trigger, got: {:?}",
        findings
    );
}

#[test]
fn test_detects_docker_latest_tag() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "Dockerfile", "FROM node:latest\n");

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Docker FROM"));
}

#[test]
fn test_detects_docker_no_tag() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "Dockerfile", "FROM ubuntu\n");

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Docker FROM"));
}

#[test]
fn test_no_warning_docker_scratch() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "Dockerfile", "FROM scratch\n");

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "FROM scratch should not trigger, got: {:?}",
        findings
    );
}

#[test]
fn test_no_warning_docker_pinned_tag() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "Dockerfile", "FROM node:18-alpine\n");

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Pinned tag should not trigger, got: {:?}",
        findings
    );
}

#[test]
fn test_detects_privileged_container() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "pod.yaml",
        "spec:\n  containers:\n    - name: app\n      securityContext:\n        privileged: true\n",
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("privileged container"));
}

#[test]
fn test_detects_hostpath_volume() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deployment.yml",
        "volumes:\n  - name: host-vol\n    hostPath:\n      path: /var/run/docker.sock\n",
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("hostPath"));
}

// ── Info-level ─────────────────────────────────────────────────

#[test]
fn test_detects_http_source_url() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "modules.tf",
        r#"module "vpc" {
  source = "http://example.com/modules/vpc.zip"
}
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("HTTP URL"));
}

#[test]
fn test_no_info_localhost_http() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "backend.tf",
        r#"  endpoint = "http://localhost:8080/api"
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "localhost HTTP should not trigger, got: {:?}",
        findings
    );
}

// ── False positive / skip tests ────────────────────────────────

#[test]
fn test_skips_non_infra_files() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "main.py",
        r#"acl = "public-read"
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Non-infra files should be skipped, got: {:?}",
        findings
    );
}

#[test]
fn test_skips_comment_lines() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "main.tf",
        r#"# acl = "public-read"
// cidr_blocks = ["0.0.0.0/0"]
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Comment lines should not trigger, got: {:?}",
        findings
    );
}

// ── Config / dispatcher tests ──────────────────────────────────

#[test]
fn test_disabled_by_default() {
    let config = RevetConfig::default();
    let analyzer = InfraAnalyzer::new();
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_enabled_when_configured() {
    let config = infra_enabled_config();
    let analyzer = InfraAnalyzer::new();
    assert!(analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_sequential_via_dispatcher() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "bad.tf",
        r#"acl = "public-read"
cidr_blocks = ["0.0.0.0/0"]
access_key = "AKIAIOSFODNN7EXAMPLE"
actions = ["*"]
"#,
    );

    let config = infra_enabled_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    let infra_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("INFRA"))
        .collect();

    assert_eq!(infra_findings.len(), 4);
    assert_eq!(infra_findings[0].id, "INFRA-001");
    assert_eq!(infra_findings[1].id, "INFRA-002");
    assert_eq!(infra_findings[2].id, "INFRA-003");
    assert_eq!(infra_findings[3].id, "INFRA-004");
}

#[test]
fn test_tf_pattern_does_not_match_yaml() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "config.yaml",
        r#"acl = "public-read"
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "TF patterns should not match YAML files, got: {:?}",
        findings
    );
}

#[test]
fn test_detects_hardcoded_creds_in_tfvars() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "secrets.tfvars",
        r#"access_key = "AKIAIOSFODNN7EXAMPLE"
"#,
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0]
        .message
        .contains("hardcoded provider credentials"));
}
