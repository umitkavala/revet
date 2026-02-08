//! Integration tests for MlPipelineAnalyzer

use revet_core::analyzer::ml_pipeline::MlPipelineAnalyzer;
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

fn default_config() -> RevetConfig {
    RevetConfig::default()
}

// ── Error-level: data leakage ──────────────────────────────────

#[test]
fn test_detects_fit_on_x_test() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "train.py", "scaler.fit(X_test)\n");

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("fit on test data"));
    assert_eq!(findings[0].line, 1);
}

#[test]
fn test_detects_fit_transform_on_test_data() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "pipeline.py", "scaler.fit_transform(test_features)\n");

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("fit on test data"));
}

#[test]
fn test_detects_fit_on_test_labels() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "train.py", "encoder.fit(y_test)\n");

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Error);
    assert!(findings[0].message.contains("fit on test labels"));
}

// ── Warning-level ──────────────────────────────────────────────

#[test]
fn test_detects_train_test_split_without_random_state() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "train.py",
        "X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2)\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("without random_state"));
}

#[test]
fn test_no_warning_train_test_split_with_random_state() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "train.py",
        "X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42)\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // Should NOT get a Warning for missing random_state
    // May get Info for missing stratify (pattern 7)
    let warnings: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .collect();
    assert!(
        warnings.is_empty(),
        "Should not warn when random_state is present, got: {:?}",
        warnings
    );
}

#[test]
fn test_detects_fit_transform_on_full_dataset() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "preprocess.py",
        "X_scaled = scaler.fit_transform(X)\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0]
        .message
        .contains("fit_transform on full dataset"));
}

#[test]
fn test_no_warning_fit_transform_on_train() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "preprocess.py",
        "X_scaled = scaler.fit_transform(X_train)\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "fit_transform on X_train should not trigger, got: {:?}",
        findings
    );
}

#[test]
fn test_detects_pickle_serialization() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "save_model.py",
        "pickle.dump(model, open('model.pkl', 'wb'))\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("pickle"));
}

#[test]
fn test_detects_hardcoded_absolute_path() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "load_data.py",
        r#"df = pd.read_csv("/data/train.csv")
"#,
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Warning);
    assert!(findings[0].message.contains("hardcoded absolute data path"));
}

#[test]
fn test_no_warning_relative_data_path() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "load_data.py",
        r#"df = pd.read_csv("data/train.csv")
"#,
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Relative path should not trigger, got: {:?}",
        findings
    );
}

// ── Info-level ─────────────────────────────────────────────────

#[test]
fn test_detects_missing_stratify() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "train.py",
        "X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42)\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    // Pattern 3 (Warning, no random_state) won't fire because random_state IS present
    // Pattern 7 (Info, no stratify) SHOULD fire because random_state is present but no stratify
    let info: Vec<_> = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .collect();
    assert_eq!(info.len(), 1);
    assert!(info[0].message.contains("without stratify"));
}

#[test]
fn test_no_info_when_stratify_present() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "train.py",
        "train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Should not flag when stratify is present, got: {:?}",
        findings
    );
}

#[test]
fn test_detects_deprecated_sklearn_import() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "old_code.py",
        "from sklearn.cross_validation import train_test_split\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].severity, Severity::Info);
    assert!(findings[0].message.contains("deprecated sklearn"));
}

// ── False positive / skip tests ────────────────────────────────

#[test]
fn test_skips_non_python_files() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(&dir, "train.js", "scaler.fit(X_test)\n");

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "JS files should be skipped, got: {:?}",
        findings
    );
}

#[test]
fn test_skips_comment_lines() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "train.py",
        "# scaler.fit(X_test)\n# pickle.dump(model, f)\n",
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Comment lines should not trigger findings, got: {:?}",
        findings
    );
}

#[test]
fn test_no_match_clean_ml_code() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "clean_ml.py",
        r#"from sklearn.model_selection import train_test_split
import joblib

X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42, stratify=y)

scaler = StandardScaler()
X_train_scaled = scaler.fit_transform(X_train)
X_test_scaled = scaler.transform(X_test)

model.fit(X_train_scaled, y_train)
joblib.dump(model, "model.joblib")
df = pd.read_csv("data/train.csv")
"#,
    );

    let analyzer = MlPipelineAnalyzer::new();
    let findings = analyzer.analyze_files(&[file], dir.path());

    assert!(
        findings.is_empty(),
        "Clean ML code should not trigger findings, got: {:?}",
        findings
    );
}

// ── Infrastructure tests ───────────────────────────────────────

#[test]
fn test_respects_config_disabled() {
    let mut config = default_config();
    config.modules.ml = false;

    let analyzer = MlPipelineAnalyzer::new();
    assert!(!analyzer.is_enabled(&config));
}

#[test]
fn test_enabled_by_default() {
    let config = default_config();
    let analyzer = MlPipelineAnalyzer::new();
    assert!(analyzer.is_enabled(&config));
}

#[test]
fn test_finding_ids_sequential_via_dispatcher() {
    let dir = TempDir::new().unwrap();
    let file = write_temp_file(
        &dir,
        "leaky.py",
        r#"scaler.fit(X_test)
encoder.fit(y_test)
pickle.dump(model, f)
"#,
    );

    let config = default_config();
    let dispatcher = revet_core::AnalyzerDispatcher::new();
    let findings = dispatcher.run_all(&[file], dir.path(), &config);

    let ml_findings: Vec<_> = findings.iter().filter(|f| f.id.starts_with("ML")).collect();

    assert_eq!(ml_findings.len(), 3);
    assert_eq!(ml_findings[0].id, "ML-001");
    assert_eq!(ml_findings[1].id, "ML-002");
    assert_eq!(ml_findings[2].id, "ML-003");
}
