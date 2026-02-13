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
