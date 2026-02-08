//! ML pipeline analyzer — detects common ML anti-patterns
//!
//! Scans raw file content line-by-line for patterns indicating data leakage,
//! non-reproducible experiments, insecure serialization, and deprecated imports.
//! Only targets Python ML code (`.py`, `.ipynb` files).

use crate::analyzer::{make_finding, Analyzer};
use crate::config::RevetConfig;
use crate::finding::{Finding, Severity};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A compiled ML pipeline detection pattern
struct MlPattern {
    name: &'static str,
    regex: Regex,
    severity: Severity,
    /// If set, the line must NOT contain this substring (negative filter)
    reject_if_contains: Option<&'static str>,
    /// If set, the line MUST also contain this substring (positive filter)
    require_contains: Option<&'static str>,
}

/// Returns all ML pipeline patterns in priority order (Error → Warning → Info)
fn patterns() -> &'static [MlPattern] {
    static PATTERNS: OnceLock<Vec<MlPattern>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            // ── Error: clear bugs ──────────────────────────────────────
            // Pattern 1: Fit on test data — fitting scaler/model on test set
            MlPattern {
                name: "fit on test data (data leakage)",
                regex: Regex::new(
                    r"\.fit(?:_transform)?\s*\(.*(?:X_test|test_X|x_test|test_x|test_data|test_features)",
                )
                .unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                require_contains: None,
            },
            // Pattern 2: Fit on test labels — fitting on test labels
            MlPattern {
                name: "fit on test labels (data leakage)",
                regex: Regex::new(
                    r"\.fit(?:_transform)?\s*\(.*(?:y_test|test_y|test_labels|test_target)",
                )
                .unwrap(),
                severity: Severity::Error,
                reject_if_contains: None,
                require_contains: None,
            },
            // ── Warning: likely problematic ────────────────────────────
            // Pattern 3: train_test_split without random_state
            MlPattern {
                name: "train_test_split without random_state (non-reproducible)",
                regex: Regex::new(r"train_test_split\s*\(").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("random_state"),
                require_contains: None,
            },
            // Pattern 4: fit_transform on full dataset before split
            MlPattern {
                name: "fit_transform on full dataset (possible data leakage)",
                regex: Regex::new(
                    r"\.fit_transform\s*\(\s*(?:X|data|df|features|dataset)\s*[\),]",
                )
                .unwrap(),
                severity: Severity::Warning,
                reject_if_contains: Some("_train"),
                require_contains: None,
            },
            // Pattern 5: Pickle for model serialization
            MlPattern {
                name: "pickle for model serialization (insecure, non-portable)",
                regex: Regex::new(r"pickle\.(?:dump|loads?)\s*\(").unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                require_contains: None,
            },
            // Pattern 6: Hardcoded absolute data path
            MlPattern {
                name: "hardcoded absolute data path (non-reproducible)",
                regex: Regex::new(r#"\.read_(?:csv|parquet|excel|json|feather)\s*\(\s*["']/"#)
                    .unwrap(),
                severity: Severity::Warning,
                reject_if_contains: None,
                require_contains: None,
            },
            // ── Info: best practices ───────────────────────────────────
            // Pattern 7: No stratify in classification split
            // Only flag if random_state IS present (to avoid duplicate with pattern 3)
            MlPattern {
                name: "train_test_split without stratify (imbalanced data risk)",
                regex: Regex::new(r"train_test_split\s*\(").unwrap(),
                severity: Severity::Info,
                reject_if_contains: Some("stratify"),
                require_contains: Some("random_state"),
            },
            // Pattern 8: Deprecated sklearn import
            MlPattern {
                name: "deprecated sklearn import (use model_selection instead)",
                regex: Regex::new(r"from\s+sklearn\.(?:cross_validation|grid_search)").unwrap(),
                severity: Severity::Info,
                reject_if_contains: None,
                require_contains: None,
            },
        ]
    })
}

/// File extensions to scan for ML patterns
const ML_EXTENSIONS: &[&str] = &["py", "ipynb"];

/// Binary file extensions to skip
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg", "webp", "woff", "woff2", "ttf", "eot",
    "otf", "zip", "gz", "tar", "bz2", "xz", "7z", "rar", "pdf", "doc", "docx", "xls", "xlsx",
    "ppt", "pptx", "exe", "dll", "so", "dylib", "o", "a", "pyc", "pyo", "class", "lock", "mp3",
    "mp4", "avi", "mov", "wav", "flac", "sqlite", "db",
];

/// Analyzer that detects ML pipeline anti-patterns
pub struct MlPipelineAnalyzer;

impl MlPipelineAnalyzer {
    /// Create a new ML pipeline analyzer
    pub fn new() -> Self {
        Self
    }

    /// Check if a file should be scanned (must be .py or .ipynb, not binary)
    fn should_scan(path: &Path) -> bool {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e.to_lowercase(),
            None => return false,
        };

        if BINARY_EXTENSIONS.contains(&ext.as_str()) {
            return false;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name.ends_with(".min.js") || file_name.ends_with(".min.css") {
            return false;
        }

        ML_EXTENSIONS.contains(&ext.as_str())
    }

    /// Check if a line is a comment (should be skipped)
    fn is_comment_line(line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.starts_with('*')
    }

    /// Scan a single file for ML pipeline patterns
    fn scan_file(path: &Path) -> Vec<Finding> {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let all_patterns = patterns();
        let mut findings = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            if Self::is_comment_line(line) {
                continue;
            }

            // First matching pattern wins for this line
            for pat in all_patterns {
                if !pat.regex.is_match(line) {
                    continue;
                }

                // Apply negative filter: skip if line contains rejected substring
                if let Some(reject) = pat.reject_if_contains {
                    if line.contains(reject) {
                        continue;
                    }
                }

                // Apply positive filter: skip if line does NOT contain required substring
                if let Some(require) = pat.require_contains {
                    if !line.contains(require) {
                        continue;
                    }
                }

                findings.push(make_finding(
                    pat.severity,
                    format!("ML pipeline issue: {}", pat.name),
                    path.to_path_buf(),
                    line_num + 1,
                ));
                break;
            }
        }

        findings
    }
}

impl Default for MlPipelineAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for MlPipelineAnalyzer {
    fn name(&self) -> &str {
        "ML Pipeline"
    }

    fn finding_prefix(&self) -> &str {
        "ML"
    }

    fn is_enabled(&self, config: &RevetConfig) -> bool {
        config.modules.ml
    }

    fn analyze_files(&self, files: &[PathBuf], _repo_root: &Path) -> Vec<Finding> {
        let mut findings = Vec::new();

        for file in files {
            if !Self::should_scan(file) {
                continue;
            }
            findings.extend(Self::scan_file(file));
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_should_scan_python_files() {
        assert!(MlPipelineAnalyzer::should_scan(&PathBuf::from("train.py")));
        assert!(MlPipelineAnalyzer::should_scan(&PathBuf::from(
            "notebook.ipynb"
        )));
    }

    #[test]
    fn test_should_skip_non_python_files() {
        assert!(!MlPipelineAnalyzer::should_scan(&PathBuf::from("app.js")));
        assert!(!MlPipelineAnalyzer::should_scan(&PathBuf::from(
            "routes.ts"
        )));
        assert!(!MlPipelineAnalyzer::should_scan(&PathBuf::from(
            "data.csv"
        )));
    }

    #[test]
    fn test_should_skip_binary_files() {
        assert!(!MlPipelineAnalyzer::should_scan(&PathBuf::from(
            "model.pkl"
        )));
        assert!(!MlPipelineAnalyzer::should_scan(&PathBuf::from(
            "logo.png"
        )));
    }

    #[test]
    fn test_fit_on_test_data_pattern() {
        let pats = patterns();
        let pat = &pats[0];
        assert!(pat.regex.is_match("scaler.fit(X_test)"));
        assert!(pat.regex.is_match("scaler.fit_transform(X_test)"));
        assert!(pat.regex.is_match("model.fit(test_data)"));
        assert!(!pat.regex.is_match("scaler.fit(X_train)"));
    }

    #[test]
    fn test_fit_on_test_labels_pattern() {
        let pats = patterns();
        let pat = &pats[1];
        assert!(pat.regex.is_match("encoder.fit(y_test)"));
        assert!(pat.regex.is_match("encoder.fit_transform(test_labels)"));
        assert!(!pat.regex.is_match("encoder.fit(y_train)"));
    }

    #[test]
    fn test_train_test_split_no_random_state_pattern() {
        let pats = patterns();
        let pat = &pats[2];
        assert!(pat.regex.is_match("train_test_split(X, y)"));
        // regex matches, but reject_if_contains would filter it:
        assert!(pat.regex.is_match("train_test_split(X, y, random_state=42)"));
    }

    #[test]
    fn test_fit_transform_full_dataset_pattern() {
        let pats = patterns();
        let pat = &pats[3];
        assert!(pat.regex.is_match("scaler.fit_transform(X)"));
        assert!(pat.regex.is_match("scaler.fit_transform(data)"));
        assert!(pat.regex.is_match("scaler.fit_transform(df)"));
        assert!(!pat.regex.is_match("scaler.fit_transform(X_train)"));
    }

    #[test]
    fn test_pickle_pattern() {
        let pats = patterns();
        let pat = &pats[4];
        assert!(pat.regex.is_match("pickle.dump(model, f)"));
        assert!(pat.regex.is_match("pickle.loads(data)"));
        assert!(pat.regex.is_match("pickle.load(f)"));
        assert!(!pat.regex.is_match("joblib.dump(model, f)"));
    }

    #[test]
    fn test_hardcoded_path_pattern() {
        let pats = patterns();
        let pat = &pats[5];
        assert!(pat.regex.is_match(r#"pd.read_csv("/data/train.csv")"#));
        assert!(pat
            .regex
            .is_match(r#"pd.read_parquet("/home/user/data.parquet")"#));
        assert!(!pat.regex.is_match(r#"pd.read_csv("data/train.csv")"#));
        assert!(!pat.regex.is_match(r#"pd.read_csv("./data/train.csv")"#));
    }

    #[test]
    fn test_deprecated_sklearn_pattern() {
        let pats = patterns();
        let pat = &pats[7];
        assert!(pat
            .regex
            .is_match("from sklearn.cross_validation import train_test_split"));
        assert!(pat
            .regex
            .is_match("from sklearn.grid_search import GridSearchCV"));
        assert!(!pat
            .regex
            .is_match("from sklearn.model_selection import train_test_split"));
    }

    #[test]
    fn test_is_comment_line() {
        assert!(MlPipelineAnalyzer::is_comment_line(
            "# scaler.fit(X_test)"
        ));
        assert!(MlPipelineAnalyzer::is_comment_line(
            "// train_test_split(X, y)"
        ));
        assert!(!MlPipelineAnalyzer::is_comment_line("scaler.fit(X_test)"));
    }
}
