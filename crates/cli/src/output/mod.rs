//! Output formatters for review findings.
//!
//! Every output format implements [`OutputFormatter`]. The caller drives it:
//! 1. `write_finding` for each active finding
//! 2. `write_suppressed` for each suppressed finding (only when `--show-suppressed`)
//! 3. `write_summary` once with final stats
//! 4. `finalize` to flush any buffered output (e.g. JSON serialises the whole
//!    document at once)

pub mod github;
pub mod github_comment;
pub mod json;
pub mod sarif;
pub mod terminal;

use revet_core::{Finding, ReviewSummary, SuppressedFinding};
use std::path::Path;
use std::time::Duration;

use crate::Cli;
use revet_core::RevetConfig;

// ── Output format enum ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub enum Format {
    Terminal,
    Json,
    Sarif,
    Github,
}

pub fn resolve_format(cli: &Cli, config: &RevetConfig) -> Format {
    if let Some(ref f) = cli.format {
        return match f {
            crate::OutputFormat::Json => Format::Json,
            crate::OutputFormat::Sarif => Format::Sarif,
            crate::OutputFormat::Github => Format::Github,
            crate::OutputFormat::Terminal => Format::Terminal,
        };
    }
    match config.output.format.as_str() {
        "json" => Format::Json,
        "sarif" => Format::Sarif,
        "github" => Format::Github,
        _ => Format::Terminal,
    }
}

// ── Trait ─────────────────────────────────────────────────────────────────────

pub trait OutputFormatter {
    /// Write one active finding.
    fn write_finding(&mut self, finding: &Finding, repo_path: &Path);

    /// Write one suppressed finding. Default: no-op (most formats ignore these).
    fn write_suppressed(&mut self, _sf: &SuppressedFinding, _repo_path: &Path) {}

    /// Write the final summary line(s) after all findings have been written.
    fn write_summary(
        &mut self,
        summary: &ReviewSummary,
        suppressed: &[SuppressedFinding],
        elapsed: Duration,
        run_id: Option<&str>,
    );

    /// Called instead of the normal flow when no files were discovered.
    fn write_no_files(&mut self, elapsed: Duration);

    /// Flush/finalize output. Called once, after `write_summary`.
    /// Formatters that buffer (JSON, SARIF) emit their document here.
    fn finalize(&mut self) {}
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Create the right formatter for the requested format.
pub fn make_formatter(
    format: Format,
    repo_path: &Path,
    show_suppressed: bool,
) -> Box<dyn OutputFormatter> {
    match format {
        Format::Terminal => Box::new(terminal::TerminalFormatter::new(show_suppressed)),
        Format::Json => Box::new(json::JsonFormatter::new()),
        Format::Sarif => Box::new(sarif::SarifFormatter::new(repo_path.to_path_buf())),
        Format::Github => Box::new(github::GithubFormatter::new(repo_path.to_path_buf())),
    }
}
