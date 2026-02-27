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
    // Include USER so only the :latest finding fires
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM node:latest\nRUN useradd -m app\nUSER app\n",
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Docker FROM"));
}

#[test]
fn test_detects_docker_no_tag() {
    let dir = TempDir::new().unwrap();
    // Include USER so only the untagged FROM finding fires
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM ubuntu\nRUN useradd -m app\nUSER app\n",
    );

    let analyzer = InfraAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("Docker FROM"));
}

#[test]
fn test_no_warning_docker_scratch() {
    let dir = TempDir::new().unwrap();
    // FROM scratch is a special case: no USER needed (scratch has no shell)
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
    // Include USER so no findings are expected
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM node:18-alpine\nRUN adduser -D app\nUSER app\n",
    );

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

    // The privileged finding plus missing-probe/resource findings will all fire;
    // assert the privileged finding is present with correct severity.
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("privileged container")),
        "privileged container must be flagged; got: {findings:?}"
    );
    let priv_finding = findings
        .iter()
        .find(|f| f.message.contains("privileged container"))
        .unwrap();
    assert_eq!(priv_finding.severity, Severity::Warning);
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

// ── K8s image :latest ──────────────────────────────────────────

#[test]
fn test_k8s_image_latest_tag() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deployment.yaml",
        "spec:\n  containers:\n  - name: app\n    image: nginx:latest\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert_eq!(
        findings.len(),
        4,
        "expected image:latest + missing probes/resources; got: {findings:?}"
    );
    assert!(findings.iter().any(|f| f.message.contains(":latest")));
}

#[test]
fn test_k8s_image_pinned_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deployment.yaml",
        "spec:\n  containers:\n  - name: app\n    image: nginx:1.25.3\n    readinessProbe:\n      httpGet:\n        path: /health\n    livenessProbe:\n      httpGet:\n        path: /health\n    resources:\n      limits:\n        cpu: 500m\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert!(
        findings.iter().all(|f| !f.message.contains(":latest")),
        "pinned image must not be flagged; got: {findings:?}"
    );
}

// ── K8s missing probes / resource limits ───────────────────────

#[test]
fn test_k8s_missing_readiness_probe() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deploy.yaml",
        "spec:\n  containers:\n  - name: app\n    image: myapp:1.0\n    livenessProbe:\n      httpGet:\n        path: /health\n    resources:\n      limits:\n        cpu: 500m\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "expected only missing readinessProbe; got: {findings:?}"
    );
    assert!(findings[0].message.contains("readinessProbe"));
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_k8s_missing_liveness_probe() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deploy.yaml",
        "spec:\n  containers:\n  - name: app\n    image: myapp:1.0\n    readinessProbe:\n      httpGet:\n        path: /health\n    resources:\n      limits:\n        cpu: 500m\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "expected only missing livenessProbe; got: {findings:?}"
    );
    assert!(findings[0].message.contains("livenessProbe"));
}

#[test]
fn test_k8s_missing_resource_limits() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deploy.yaml",
        "spec:\n  containers:\n  - name: app\n    image: myapp:1.0\n    readinessProbe:\n      httpGet:\n        path: /health\n    livenessProbe:\n      httpGet:\n        path: /health\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert_eq!(
        findings.len(),
        1,
        "expected only missing resources; got: {findings:?}"
    );
    assert!(findings[0].message.contains("resource"));
}

#[test]
fn test_k8s_no_containers_no_probe_finding() {
    // Non-deployment YAML (e.g. ConfigMap) should not trigger missing-probe findings
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "configmap.yaml",
        "apiVersion: v1\nkind: ConfigMap\ndata:\n  key: value\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert!(
        findings
            .iter()
            .all(|f| !f.message.contains("Probe") && !f.message.contains("resource")),
        "ConfigMap must not trigger missing-probe findings; got: {findings:?}"
    );
}

// ── Docker ADD / USER / COPY . . ───────────────────────────────

#[test]
fn test_docker_add_instruction() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM ubuntu:22.04\nADD ./app /app\nUSER appuser\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert!(
        findings.iter().any(|f| f.message.contains("ADD")),
        "ADD must be flagged; got: {findings:?}"
    );
    assert_eq!(
        findings
            .iter()
            .filter(|f| f.message.contains("ADD"))
            .count(),
        1
    );
}

#[test]
fn test_docker_user_root() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM ubuntu:22.04\nRUN apt-get update\nUSER root\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert!(
        findings.iter().any(|f| f.message.contains("root")),
        "USER root must be flagged; got: {findings:?}"
    );
}

#[test]
fn test_docker_copy_dot_dot() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM node:20\nCOPY . .\nRUN npm install\nUSER node\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert!(
        findings.iter().any(|f| f.message.contains("COPY . .")),
        "COPY . . must be flagged; got: {findings:?}"
    );
}

#[test]
fn test_docker_missing_user_instruction() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM python:3.11\nRUN pip install flask\nCMD [\"python\", \"app.py\"]\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert!(
        findings.iter().any(|f| f.message.contains("non-root USER")),
        "missing USER must be flagged; got: {findings:?}"
    );
    assert_eq!(findings[0].severity, Severity::Warning);
}

#[test]
fn test_docker_non_root_user_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "Dockerfile",
        "FROM python:3.11\nRUN useradd -m appuser\nUSER appuser\nCMD [\"python\", \"app.py\"]\n",
    );
    let findings = InfraAnalyzer::new().analyze_files(&[file], dir.path());
    assert!(
        findings
            .iter()
            .all(|f| !f.message.contains("non-root USER")),
        "non-root USER must not trigger missing-user finding; got: {findings:?}"
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
