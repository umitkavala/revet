//! GitHub Actions workflow command output formatter.
//!
//! Produces `::error`, `::warning`, and `::notice` annotations for inline
//! PR feedback. Each finding is printed immediately — no buffering needed.
//!
//! See: <https://docs.github.com/en/actions/using-workflows/workflow-commands-for-github-actions>

use revet_core::{BlastRadiusSummary, Finding, ReviewSummary, Severity, SuppressedFinding};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::OutputFormatter;

pub struct GithubFormatter {
    repo_path: PathBuf,
}

impl GithubFormatter {
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }
}

impl OutputFormatter for GithubFormatter {
    fn write_blast_radius(&mut self, summary: &BlastRadiusSummary) {
        // Emit a GitHub Actions notice annotation with the blast radius summary
        println!(
            "::notice title=PR Blast Radius::Risk: {} | {} symbol(s) modified | {} caller(s) affected | {} module {}",
            summary.risk,
            summary.directly_modified,
            summary.transitively_affected,
            summary.cross_module_crossings,
            if summary.cross_module_crossings == 1 { "boundary crossed" } else { "boundaries crossed" },
        );
    }

    fn write_finding(&mut self, finding: &Finding, _repo_path: &Path) {
        println!("{}", format_finding(finding, &self.repo_path));
    }

    fn write_summary(
        &mut self,
        _summary: &ReviewSummary,
        _suppressed: &[SuppressedFinding],
        _elapsed: Duration,
        _run_id: Option<&str>,
    ) {
        // GitHub annotations don't have a summary section.
    }

    fn write_no_files(&mut self, _elapsed: Duration) {
        // Nothing to annotate.
    }
}

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
