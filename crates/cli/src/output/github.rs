//! GitHub Actions workflow command output format
//!
//! Produces `::error`, `::warning`, and `::notice` annotations for inline PR feedback.

use revet_core::{Finding, Severity};
use std::path::Path;

/// Format a finding as a GitHub Actions workflow command.
///
/// See: https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions
pub fn format_finding(finding: &Finding, repo_path: &Path) -> String {
    let level = match finding.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "notice",
    };
    let rel_path = finding
        .file
        .strip_prefix(repo_path)
        .unwrap_or(&finding.file);
    format!(
        "::{level} file={},line={},title={}::{msg}",
        rel_path.display(),
        finding.line,
        finding.id,
        level = level,
        msg = finding.message,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_finding(severity: Severity, file: &str, line: usize) -> Finding {
        Finding {
            id: "SEC-001".to_string(),
            severity,
            message: "Hardcoded secret detected".to_string(),
            file: PathBuf::from(format!("/repo/{}", file)),
            line,
            affected_dependents: 0,
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
}
