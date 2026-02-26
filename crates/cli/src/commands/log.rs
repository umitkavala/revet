//! `revet log` — list past runs or show a specific run's JSON

use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::run_log;

pub fn run(repo_path: &Path, show: Option<&str>) -> Result<()> {
    match show {
        Some(id) => show_run(repo_path, id),
        None => list_runs(repo_path),
    }
}

fn list_runs(repo_path: &Path) -> Result<()> {
    let entries = run_log::list_runs(repo_path)?;

    if entries.is_empty() {
        println!(
            "  {}",
            "No run logs found. Run `revet review` to create one.".dimmed()
        );
        return Ok(());
    }

    println!(
        "  {}",
        format!(
            "  {:<20} {:<12} {:<8} {:<10} {:<12} {}",
            "ID", "Date", "Files", "Findings", "Suppressed", "Duration"
        )
        .bold()
    );
    println!("  {}", "\u{2500}".repeat(72).dimmed());

    for entry in &entries {
        let date = format_timestamp(entry.timestamp);
        println!(
            "  {:<20} {:<12} {:<8} {:<10} {:<12} {:.1}s",
            entry.id,
            date,
            entry.files_analyzed,
            entry.findings_kept,
            entry.suppressed,
            entry.duration_secs,
        );
    }

    println!();
    println!(
        "  {}",
        "Use `revet log --show <id>` to view a specific run.".dimmed()
    );
    Ok(())
}

fn show_run(repo_path: &Path, id: &str) -> Result<()> {
    let log = run_log::load_run_log(repo_path, id)?;
    let json = serde_json::to_string_pretty(&log)?;
    println!("{}", json);
    Ok(())
}

fn format_timestamp(ts: u64) -> String {
    let days = ts / 86400;
    let (year, month, day) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let leap = is_leap(year);
        let days_in_year = if leap { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap(year);
    let months = [
        31u64,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u64;
    for &m in &months {
        if days < m {
            break;
        }
        days -= m;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap(y: u64) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}
