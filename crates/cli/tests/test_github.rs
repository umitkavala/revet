use revet_cli::output::github::format_finding;
use revet_core::{Finding, Severity};
use std::path::{Path, PathBuf};

fn make_finding(severity: Severity, file: &str, line: usize) -> Finding {
    Finding {
        id: "SEC-001".to_string(),
        severity,
        message: "Hardcoded secret detected".to_string(),
        file: PathBuf::from(format!("/repo/{}", file)),
        line,
        affected_dependents: 0,
        suggestion: None,
        fix_kind: None,
    }
}

#[test]
fn error_finding() {
    let f = make_finding(Severity::Error, "src/config.ts", 9);
    let out = format_finding(&f, Path::new("/repo"));
    assert_eq!(
        out,
        "::error file=src/config.ts,line=9,title=SEC-001::Hardcoded secret detected"
    );
}

#[test]
fn warning_finding() {
    let f = make_finding(Severity::Warning, "src/lib.rs", 42);
    let out = format_finding(&f, Path::new("/repo"));
    assert_eq!(
        out,
        "::warning file=src/lib.rs,line=42,title=SEC-001::Hardcoded secret detected"
    );
}

#[test]
fn info_finding() {
    let f = make_finding(Severity::Info, "README.md", 1);
    let out = format_finding(&f, Path::new("/repo"));
    assert_eq!(
        out,
        "::notice file=README.md,line=1,title=SEC-001::Hardcoded secret detected"
    );
}
