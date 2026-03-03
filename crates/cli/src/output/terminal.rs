//! Terminal output formatter — rich coloured block layout.
//!
//! Each finding renders as a two-part block:
//!
//! ```text
//!   ✗  BREAKING   src/foo.rs:42
//!   |  Function signature changed — 3 dependent(s) affected
//!   |  Fix: update all call sites
//! ```

use colored::Colorize;
use revet_core::{Finding, ReviewSummary, Severity, SuppressedFinding};
use std::path::Path;
use std::time::Duration;

use super::OutputFormatter;

// ── Formatter struct ─────────────────────────────────────────────────────────

pub struct TerminalFormatter {
    show_suppressed: bool,
    printed: usize, // total blocks printed so far (for blank-line spacing)
}

impl TerminalFormatter {
    pub fn new(show_suppressed: bool) -> Self {
        Self {
            show_suppressed,
            printed: 0,
        }
    }
}

impl Default for TerminalFormatter {
    fn default() -> Self {
        Self::new(false)
    }
}

// ── OutputFormatter impl ─────────────────────────────────────────────────────

impl OutputFormatter for TerminalFormatter {
    fn write_finding(&mut self, finding: &Finding, repo_path: &Path) {
        if self.printed > 0 {
            println!();
        }
        self.printed += 1;
        println!("{}", finding_block(finding, repo_path));
    }

    fn write_suppressed(&mut self, sf: &SuppressedFinding, repo_path: &Path) {
        if !self.show_suppressed {
            return;
        }
        if self.printed > 0 {
            println!();
        }
        self.printed += 1;
        println!("{}", suppressed_block(sf, repo_path));
    }

    fn write_summary(
        &mut self,
        summary: &ReviewSummary,
        suppressed: &[SuppressedFinding],
        elapsed: Duration,
        run_id: Option<&str>,
    ) {
        if self.printed > 0 {
            println!();
        }

        println!("  {}", "\u{2500}".repeat(60).dimmed());

        // Error / warning / info counts
        let errors_str = if summary.errors > 0 {
            format!(
                "{} {}",
                summary.errors,
                if summary.errors == 1 {
                    "error"
                } else {
                    "errors"
                }
            )
            .red()
            .to_string()
        } else {
            "0 errors".dimmed().to_string()
        };

        let warnings_str = if summary.warnings > 0 {
            format!(
                "{} {}",
                summary.warnings,
                if summary.warnings == 1 {
                    "warning"
                } else {
                    "warnings"
                }
            )
            .yellow()
            .to_string()
        } else {
            "0 warnings".dimmed().to_string()
        };

        let info_str = format!("{} info", summary.info).dimmed().to_string();

        println!(
            "  {} \u{00b7} {} \u{00b7} {}",
            errors_str, warnings_str, info_str
        );

        // Suppression breakdown
        if !suppressed.is_empty() {
            let baseline = suppressed.iter().filter(|s| s.reason == "baseline").count();
            let inline = suppressed.iter().filter(|s| s.reason == "inline").count();
            let per_path = suppressed
                .iter()
                .filter(|s| s.reason.starts_with("per-path"))
                .count();

            let mut parts = Vec::new();
            if baseline > 0 {
                parts.push(format!("{} baselined", baseline));
            }
            if inline > 0 {
                parts.push(format!("{} inline", inline));
            }
            if per_path > 0 {
                parts.push(format!("{} per-path", per_path));
            }

            println!(
                "  {}",
                format!(
                    "{} finding(s) suppressed ({})",
                    suppressed.len(),
                    parts.join(", ")
                )
                .dimmed()
            );
        }

        println!(
            "  {}",
            format!(
                "{} files analyzed \u{00b7} {} nodes parsed",
                summary.files_analyzed, summary.nodes_parsed
            )
            .dimmed()
        );

        println!(
            "  {}",
            format!("Time: {:.1}s", elapsed.as_secs_f64()).green()
        );

        if let Some(id) = run_id {
            println!("  {}", format!("Run log: revet log --show {}", id).dimmed());
        }
    }

    fn write_no_files(&mut self, elapsed: Duration) {
        println!("  {}", "No supported files found.".dimmed());
        println!(
            "  {}",
            format!("Time: {:.1}s", elapsed.as_secs_f64()).green()
        );
    }
}

// ── Rendering helpers ────────────────────────────────────────────────────────

fn finding_block(f: &Finding, repo_path: &Path) -> String {
    let label = f.id.split('-').next().unwrap_or(&f.id);

    let (icon, colored_label) = match f.severity {
        Severity::Error => ("✗".red().bold().to_string(), label.red().bold().to_string()),
        Severity::Warning => (
            "⚠".yellow().bold().to_string(),
            label.yellow().bold().to_string(),
        ),
        Severity::Info => ("·".blue().to_string(), label.blue().to_string()),
    };

    let display = f.file.strip_prefix(repo_path).unwrap_or(&f.file);
    let file_line = if f.line > 0 {
        format!("{}:{}", display.display(), f.line)
            .cyan()
            .to_string()
    } else {
        display.display().to_string().cyan().to_string()
    };

    let pipe = "|".dimmed();
    let mut lines = vec![format!("  {}  {}   {}", icon, colored_label, file_line)];

    for msg_line in f.message.lines() {
        // Lines starting with "→" are caller/path references — highlight in cyan
        let trimmed = msg_line.trim_start();
        if trimmed.starts_with('\u{2192}') {
            lines.push(format!("  {}  {}", pipe, trimmed.cyan()));
        } else {
            lines.push(format!("  {}  {}", pipe, msg_line.dimmed()));
        }
    }

    // Render caller locations as cyan arrow lines
    for caller in &f.callers {
        lines.push(format!(
            "  {}  {}",
            pipe,
            format!("\u{2192} {}", caller).cyan()
        ));
    }

    if let Some(s) = &f.suggestion {
        lines.push(format!("  {}  {}", pipe, format!("Fix: {}", s).dimmed()));
    }

    if let Some(note) = &f.ai_note {
        let prefix = if f.ai_false_positive {
            "🤖 [likely false positive] "
        } else {
            "🤖 "
        };
        lines.push(format!(
            "  {}  {}",
            pipe,
            format!("{}{}", prefix, note).dimmed()
        ));
    }

    lines.join("\n")
}

fn suppressed_block(sf: &SuppressedFinding, repo_path: &Path) -> String {
    let f = &sf.finding;
    let label = f.id.split('-').next().unwrap_or(&f.id);

    let icon = match f.severity {
        Severity::Error => "✗",
        Severity::Warning => "⚠",
        Severity::Info => "·",
    };

    let display = f.file.strip_prefix(repo_path).unwrap_or(&f.file);
    let file_line = if f.line > 0 {
        format!("{}:{}", display.display(), f.line)
    } else {
        display.display().to_string()
    };

    let header = format!("  {}  {}   {}", icon, label, file_line).dimmed();
    let pipe = "|".dimmed();

    format!(
        "{}\n  {}  {}\n  {}  {}",
        header,
        pipe,
        f.message.as_str().dimmed(),
        pipe,
        format!("[suppressed: {}]", sf.reason).dimmed()
    )
}
