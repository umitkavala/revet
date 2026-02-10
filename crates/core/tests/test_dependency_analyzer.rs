//! Integration tests for DependencyAnalyzer

use revet_core::analyzer::dependency::DependencyAnalyzer;
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

fn dep_enabled_config() -> RevetConfig {
    let mut config = RevetConfig::default();
    config.modules.dependency = true;
    config
}

// ── Pattern 1: Python wildcard import ────────────────────────

#[test]
fn test_python_wildcard_import() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.py", "from utils import *\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("wildcard import in Python"));
    assert_eq!(findings[0].line, 1);
}

#[test]
fn test_python_specific_import_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.py", "from utils import parse_config\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Specific import should not trigger, got: {:?}",
        findings
    );
}

// ── Pattern 2: Java wildcard import ─────────────────────────

#[test]
fn test_java_wildcard_import() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "App.java", "import java.util.*;\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("wildcard import in Java"));
}

#[test]
fn test_java_specific_import_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "App.java", "import java.util.List;\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Specific Java import should not trigger, got: {:?}",
        findings
    );
}

// ── Pattern 3: Deprecated Python import ─────────────────────

#[test]
fn test_deprecated_python_import() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "old.py",
        "import optparse\nfrom distutils import core\n",
    );

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.severity == Severity::Warning));
    assert!(findings[0].message.contains("deprecated Python module"));
}

// ── Pattern 4: require() instead of import ──────────────────

#[test]
fn test_require_instead_of_import() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "app.ts", "const fs = require('fs');\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("require()"));
}

#[test]
fn test_require_jest_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "test.js", "const { expect } = require('jest');\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "require with jest should not trigger, got: {:?}",
        findings
    );
}

// ── Pattern 5: Deeply nested relative import ────────────────

#[test]
fn test_deeply_nested_relative_import() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "deep.ts",
        "import { foo } from '../../../utils/foo';\n",
    );

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("deeply nested"));
}

#[test]
fn test_shallow_relative_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "shallow.ts", "import { bar } from '../utils/bar';\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Shallow relative import should not trigger, got: {:?}",
        findings
    );
}

// ── Pattern 6: Circular import workaround ───────────────────

#[test]
fn test_circular_import_workaround() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "circ.py",
        "from models import User  # type: ignore[import\n",
    );

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("circular import"));
}

// ── Pattern 7: Unpinned/wildcard dep version ────────────────

#[test]
fn test_unpinned_wildcard_version() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "package.json",
        r#"{
  "dependencies": {
    "lodash": "*",
    "react": "latest"
  }
}
"#,
    );

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| f.severity == Severity::Warning));
    assert!(findings[0].message.contains("unpinned"));
}

#[test]
fn test_pinned_version_no_finding() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "package.json",
        r#"{
  "dependencies": {
    "lodash": "^4.17.21",
    "react": "~18.2.0"
  }
}
"#,
    );

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Pinned versions should not trigger, got: {:?}",
        findings
    );
}

// ── Pattern 8: Git dependency ───────────────────────────────

#[test]
fn test_git_dependency() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "package.json",
        r#"{
  "dependencies": {
    "my-lib": "git+https://github.com/user/repo.git"
  }
}
"#,
    );

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("git dependency"));
}

// ── Skipping / config tests ─────────────────────────────────

#[test]
fn test_non_target_file_skipped() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "main.rs", "from utils import *\n");

    let analyzer = DependencyAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Non-target files should be skipped, got: {:?}",
        findings
    );
}

#[test]
fn test_config_disabled_by_default() {
    let config = RevetConfig::default();
    let analyzer = DependencyAnalyzer::new();
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_via_dispatcher() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "bad.py", "from os import *\nimport distutils\n");

    let config = dep_enabled_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    let dep_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.id.starts_with("DEP"))
        .collect();

    assert_eq!(dep_findings.len(), 2);
    assert_eq!(dep_findings[0].id, "DEP-001");
    assert_eq!(dep_findings[1].id, "DEP-002");
}
