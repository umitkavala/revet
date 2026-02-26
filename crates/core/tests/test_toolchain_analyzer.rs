//! Integration tests for the ToolchainAnalyzer.

use revet_core::analyzer::Analyzer;
use revet_core::RevetConfig;
use revet_core::ToolchainAnalyzer;
use std::fs;
use tempfile::TempDir;

fn make_config_with_toolchain() -> RevetConfig {
    let mut config = RevetConfig::default();
    config.modules.toolchain = true;
    config
}

fn run(repo: &TempDir) -> Vec<String> {
    let analyzer = ToolchainAnalyzer::new();
    let config = make_config_with_toolchain();
    let findings = analyzer.analyze_files(&[], repo.path());
    assert!(analyzer.is_enabled(&config));
    findings.into_iter().map(|f| f.message).collect()
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn write(repo: &TempDir, rel: &str, content: &str) {
    let path = repo.path().join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_no_ci_files_no_findings() {
    let repo = TempDir::new().unwrap();
    assert!(run(&repo).is_empty());
}

#[test]
fn test_clippy_without_rust_toolchain_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/ci.yml",
        "steps:\n  - run: cargo clippy -- -D warnings\n",
    );
    let msgs = run(&repo);
    assert!(
        msgs.iter().any(|m| m.contains("clippy")),
        "expected clippy finding, got: {:?}",
        msgs
    );
}

#[test]
fn test_clippy_declared_in_rust_toolchain_not_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/ci.yml",
        "steps:\n  - run: cargo clippy -- -D warnings\n",
    );
    write(
        &repo,
        "rust-toolchain.toml",
        "[toolchain]\nchannel = \"stable\"\ncomponents = [\"clippy\", \"rustfmt\"]\n",
    );
    let msgs = run(&repo);
    assert!(
        !msgs.iter().any(|m| m.contains("clippy")),
        "unexpected clippy finding: {:?}",
        msgs
    );
}

#[test]
fn test_rustfmt_in_makefile_without_declaration_flagged() {
    let repo = TempDir::new().unwrap();
    write(&repo, "Makefile", "fmt:\n\tcargo fmt --check\n");
    let msgs = run(&repo);
    assert!(
        msgs.iter().any(|m| m.contains("rustfmt")),
        "expected rustfmt finding, got: {:?}",
        msgs
    );
}

#[test]
fn test_rustfmt_declared_not_flagged() {
    let repo = TempDir::new().unwrap();
    write(&repo, "Makefile", "fmt:\n\tcargo fmt --check\n");
    write(
        &repo,
        "rust-toolchain.toml",
        "[toolchain]\ncomponents = [\"rustfmt\"]\n",
    );
    let msgs = run(&repo);
    assert!(
        !msgs.iter().any(|m| m.contains("rustfmt")),
        "unexpected rustfmt finding: {:?}",
        msgs
    );
}

#[test]
fn test_eslint_in_ci_without_package_json_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/lint.yml",
        "steps:\n  - run: npx eslint src/\n",
    );
    let msgs = run(&repo);
    assert!(
        msgs.iter().any(|m| m.contains("eslint")),
        "expected eslint finding, got: {:?}",
        msgs
    );
}

#[test]
fn test_eslint_in_package_json_dev_deps_not_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/lint.yml",
        "steps:\n  - run: npx eslint src/\n",
    );
    write(
        &repo,
        "package.json",
        r#"{"devDependencies": {"eslint": "^8.0.0", "prettier": "^3.0.0"}}"#,
    );
    let msgs = run(&repo);
    assert!(
        !msgs.iter().any(|m| m.contains("eslint")),
        "unexpected eslint finding: {:?}",
        msgs
    );
}

#[test]
fn test_pytest_in_ci_without_requirements_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/test.yml",
        "steps:\n  - run: pytest tests/\n",
    );
    let msgs = run(&repo);
    assert!(
        msgs.iter().any(|m| m.contains("pytest")),
        "expected pytest finding, got: {:?}",
        msgs
    );
}

#[test]
fn test_pytest_in_requirements_dev_not_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/test.yml",
        "steps:\n  - run: pytest tests/\n",
    );
    write(
        &repo,
        "requirements-dev.txt",
        "pytest==7.4.0\nblack==23.0.0\n",
    );
    let msgs = run(&repo);
    assert!(
        !msgs.iter().any(|m| m.contains("pytest")),
        "unexpected pytest finding: {:?}",
        msgs
    );
}

#[test]
fn test_ruff_in_ci_without_pyproject_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/lint.yml",
        "steps:\n  - run: ruff check .\n",
    );
    let msgs = run(&repo);
    assert!(
        msgs.iter().any(|m| m.contains("ruff")),
        "expected ruff finding, got: {:?}",
        msgs
    );
}

#[test]
fn test_ruff_in_pyproject_not_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/lint.yml",
        "steps:\n  - run: ruff check .\n",
    );
    write(
        &repo,
        "pyproject.toml",
        "[tool.ruff]\nline-length = 88\n\n[project.optional-dependencies]\ndev = [\"ruff\"]\n",
    );
    let msgs = run(&repo);
    assert!(
        !msgs.iter().any(|m| m.contains("ruff")),
        "unexpected ruff finding: {:?}",
        msgs
    );
}

#[test]
fn test_golangci_lint_in_ci_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/go.yml",
        "steps:\n  - run: golangci-lint run ./...\n",
    );
    let msgs = run(&repo);
    assert!(
        msgs.iter().any(|m| m.contains("golangci-lint")),
        "expected golangci-lint finding, got: {:?}",
        msgs
    );
}

#[test]
fn test_golangci_lint_in_tools_go_not_flagged() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/go.yml",
        "steps:\n  - run: golangci-lint run ./...\n",
    );
    write(
        &repo,
        "tools.go",
        "//go:build tools\npackage tools\nimport _ \"github.com/golangci/golangci-lint/cmd/golangci-lint\"\n",
    );
    let msgs = run(&repo);
    assert!(
        !msgs.iter().any(|m| m.contains("golangci-lint")),
        "unexpected golangci-lint finding: {:?}",
        msgs
    );
}

#[test]
fn test_multiple_tools_one_ci_file() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/ci.yml",
        "steps:\n  - run: cargo clippy\n  - run: cargo fmt --check\n  - run: pytest\n",
    );
    let msgs = run(&repo);
    assert!(msgs.iter().any(|m| m.contains("clippy")));
    assert!(msgs.iter().any(|m| m.contains("rustfmt")));
    assert!(msgs.iter().any(|m| m.contains("pytest")));
}

#[test]
fn test_comment_lines_ignored() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/ci.yml",
        "steps:\n  # - run: cargo clippy\n  - run: echo hello\n",
    );
    let msgs = run(&repo);
    assert!(
        !msgs.iter().any(|m| m.contains("clippy")),
        "comment line should not trigger finding: {:?}",
        msgs
    );
}

#[test]
fn test_disabled_by_default() {
    let repo = TempDir::new().unwrap();
    write(
        &repo,
        ".github/workflows/ci.yml",
        "steps:\n  - run: cargo clippy\n",
    );
    let analyzer = ToolchainAnalyzer::new();
    let config = RevetConfig::default(); // toolchain: false
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_shell_script_at_root_scanned() {
    let repo = TempDir::new().unwrap();
    write(&repo, "ci.sh", "#!/bin/bash\ncargo clippy -- -D warnings\n");
    let msgs = run(&repo);
    assert!(
        msgs.iter().any(|m| m.contains("clippy")),
        "expected clippy from shell script, got: {:?}",
        msgs
    );
}
