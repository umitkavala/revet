use revet_core::finding::{Finding, FixKind};
use revet_core::fixer::apply_fixes;
use revet_core::Severity;
use std::path::PathBuf;
use tempfile::NamedTempFile;

fn make_finding(file: PathBuf, line: usize, suggestion: &str, fix_kind: FixKind) -> Finding {
    Finding {
        id: "TEST-001".to_string(),
        severity: Severity::Warning,
        message: "Test finding".to_string(),
        file,
        line,
        affected_dependents: 0,
        suggestion: Some(suggestion.to_string()),
        fix_kind: Some(fix_kind),
    }
}

// ── CommentOut tests ─────────────────────────────────────────────

#[test]
fn test_comment_out_python_secret() {
    let tmp = NamedTempFile::with_suffix(".py").unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(&path, "API_KEY = 'AKIA1234567890123456'\nprint('hello')\n").unwrap();

    let findings = vec![make_finding(
        path.clone(),
        1,
        "Use environment variable instead",
        FixKind::CommentOut,
    )];

    let report = apply_fixes(&findings).unwrap();
    assert_eq!(report.applied, 1);
    assert_eq!(report.skipped, 0);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# FIXME(revet): Use environment variable instead"));
    assert!(content.contains("# API_KEY = 'AKIA1234567890123456'"));
    // Other lines untouched
    assert!(content.contains("print('hello')"));
}

#[test]
fn test_comment_out_typescript_secret() {
    let tmp = NamedTempFile::with_suffix(".ts").unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(
        &path,
        "const token = 'ghp_abc123abc123abc123abc123abc123abc123abc1';\n",
    )
    .unwrap();

    let findings = vec![make_finding(
        path.clone(),
        1,
        "Use environment variable GITHUB_TOKEN instead",
        FixKind::CommentOut,
    )];

    let report = apply_fixes(&findings).unwrap();
    assert_eq!(report.applied, 1);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("// FIXME(revet): Use environment variable GITHUB_TOKEN instead"));
    assert!(content.contains("// const token ="));
}

// ── ReplacePattern tests ─────────────────────────────────────────

#[test]
fn test_replace_pattern_infra_acl() {
    let tmp = NamedTempFile::with_suffix(".tf").unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(
        &path,
        "resource \"aws_s3_bucket\" \"data\" {\n  acl = \"public-read\"\n}\n",
    )
    .unwrap();

    let findings = vec![make_finding(
        path.clone(),
        2,
        "Set ACL to private",
        FixKind::ReplacePattern {
            find: r#"public-read(?:-write)?"#.to_string(),
            replace: "private".to_string(),
        },
    )];

    let report = apply_fixes(&findings).unwrap();
    assert_eq!(report.applied, 1);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("acl = \"private\""));
    assert!(!content.contains("public-read"));
}

#[test]
fn test_replace_pattern_infra_privileged() {
    let tmp = NamedTempFile::with_suffix(".yaml").unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(
        &path,
        "spec:\n  containers:\n    securityContext:\n      privileged: true\n",
    )
    .unwrap();

    let findings = vec![make_finding(
        path.clone(),
        4,
        "Set privileged: false",
        FixKind::ReplacePattern {
            find: r"privileged:\s*true".to_string(),
            replace: "privileged: false".to_string(),
        },
    )];

    let report = apply_fixes(&findings).unwrap();
    assert_eq!(report.applied, 1);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("privileged: false"));
    assert!(!content.contains("privileged: true"));
}

#[test]
fn test_replace_pickle_with_joblib() {
    let tmp = NamedTempFile::with_suffix(".py").unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(
        &path,
        "import pickle\nmodel = pickle.load(open('model.pkl', 'rb'))\n",
    )
    .unwrap();

    let findings = vec![make_finding(
        path.clone(),
        2,
        "Use joblib instead",
        FixKind::ReplacePattern {
            find: r"pickle\.".to_string(),
            replace: "joblib.".to_string(),
        },
    )];

    let report = apply_fixes(&findings).unwrap();
    assert_eq!(report.applied, 1);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("joblib.load("));
    assert!(!content.lines().nth(1).unwrap().contains("pickle."));
}

#[test]
fn test_replace_deprecated_sklearn_import() {
    let tmp = NamedTempFile::with_suffix(".py").unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(
        &path,
        "from sklearn.cross_validation import KFold\nfrom sklearn.grid_search import GridSearchCV\n",
    )
    .unwrap();

    let f1 = Finding {
        id: "ML-001".to_string(),
        severity: Severity::Info,
        message: "deprecated sklearn import".to_string(),
        file: path.clone(),
        line: 1,
        affected_dependents: 0,
        suggestion: Some("Use sklearn.model_selection instead".to_string()),
        fix_kind: Some(FixKind::ReplacePattern {
            find: r"sklearn\.(?:cross_validation|grid_search)".to_string(),
            replace: "sklearn.model_selection".to_string(),
        }),
    };
    let f2 = Finding {
        id: "ML-002".to_string(),
        severity: Severity::Info,
        message: "deprecated sklearn import".to_string(),
        file: path.clone(),
        line: 2,
        affected_dependents: 0,
        suggestion: Some("Use sklearn.model_selection instead".to_string()),
        fix_kind: Some(FixKind::ReplacePattern {
            find: r"sklearn\.(?:cross_validation|grid_search)".to_string(),
            replace: "sklearn.model_selection".to_string(),
        }),
    };

    let report = apply_fixes(&[f1, f2]).unwrap();
    assert_eq!(report.applied, 2);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("from sklearn.model_selection import KFold"));
    assert!(content.contains("from sklearn.model_selection import GridSearchCV"));
    assert!(!content.contains("cross_validation"));
    assert!(!content.contains("grid_search"));
}

// ── Suggestion-only tests ────────────────────────────────────────

#[test]
fn test_suggestion_only_no_modification() {
    let tmp = NamedTempFile::with_suffix(".py").unwrap();
    let path = tmp.path().to_path_buf();
    let original = "cursor.execute(f\"SELECT * FROM users WHERE id = {user_id}\")\n";
    std::fs::write(&path, original).unwrap();

    let findings = vec![make_finding(
        path.clone(),
        1,
        "Use parameterized queries",
        FixKind::Suggestion,
    )];

    let report = apply_fixes(&findings).unwrap();
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped, 1);

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, original);
}

// ── Multi-fix tests ─────────────────────────────────────────────

#[test]
fn test_multiple_fixes_same_file() {
    let tmp = NamedTempFile::with_suffix(".py").unwrap();
    let path = tmp.path().to_path_buf();
    std::fs::write(
        &path,
        "API_KEY = 'AKIA1234567890123456'\nSECRET = 'supersecretvalue1234'\nprint('ok')\n",
    )
    .unwrap();

    let f1 = make_finding(
        path.clone(),
        1,
        "Use env var for API key",
        FixKind::CommentOut,
    );
    let mut f2 = make_finding(
        path.clone(),
        2,
        "Use env var for secret",
        FixKind::CommentOut,
    );
    f2.id = "TEST-002".to_string();

    let report = apply_fixes(&[f1, f2]).unwrap();
    assert_eq!(report.applied, 2);

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# FIXME(revet): Use env var for API key"));
    assert!(content.contains("# FIXME(revet): Use env var for secret"));
    assert!(content.contains("print('ok')"));
}

// ── Line preservation test ──────────────────────────────────────

#[test]
fn test_fix_preserves_other_lines() {
    let tmp = NamedTempFile::with_suffix(".tf").unwrap();
    let path = tmp.path().to_path_buf();
    let original = "# This is a comment\nresource \"aws_s3_bucket\" \"data\" {\n  acl = \"public-read-write\"\n  tags = {}\n}\n";
    std::fs::write(&path, original).unwrap();

    let findings = vec![make_finding(
        path.clone(),
        3,
        "Set ACL to private",
        FixKind::ReplacePattern {
            find: r#"public-read(?:-write)?"#.to_string(),
            replace: "private".to_string(),
        },
    )];

    let report = apply_fixes(&findings).unwrap();
    assert_eq!(report.applied, 1);

    let content = std::fs::read_to_string(&path).unwrap();
    // Fixed line
    assert!(content.contains("acl = \"private\""));
    // All other lines preserved
    assert!(content.contains("# This is a comment"));
    assert!(content.contains("resource \"aws_s3_bucket\" \"data\""));
    assert!(content.contains("tags = {}"));
}
