//! `revet stats` — aggregate run history into code quality metrics.
//!
//! Reads all run logs from `.revet-cache/runs/` and surfaces:
//! - Clean run rate (% of runs with zero findings)
//! - Week-over-week finding trend
//! - Average findings per run by severity
//! - Noisiest rules (most frequent across all runs)
//! - Most suppressed rules

use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::run_log::{self, RunLog};

pub fn run(repo_path: &Path, last_n: Option<usize>) -> Result<()> {
    let entries = run_log::list_runs(repo_path)?;

    if entries.is_empty() {
        println!(
            "  {}",
            "No run history found. Run `revet review` a few times to build up stats.".dimmed()
        );
        return Ok(());
    }

    // Load the full log for each entry (we need finding details for rule stats)
    let mut logs: Vec<RunLog> = entries
        .iter()
        .filter_map(|e| run_log::load_run_log(repo_path, &e.id).ok())
        .collect();

    // Newest first (list_runs already returns newest-first, but reload may shuffle)
    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Optionally limit to last N runs
    if let Some(n) = last_n {
        logs.truncate(n);
    }

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    print_summary_header(logs.len(), last_n);
    print_clean_rate(&logs);
    print_severity_averages(&logs);
    print_trend(now_secs, &logs);
    print_noisiest_rules(&logs);
    print_suppression_stats(&logs);

    Ok(())
}

// ── Sections ─────────────────────────────────────────────────────────────────

fn print_summary_header(count: usize, last_n: Option<usize>) {
    println!();
    println!("  {}", "revet stats".bold().yellow());
    if let Some(n) = last_n {
        println!(
            "  {}",
            format!("Showing last {} of {} run(s)", count.min(n), count).dimmed()
        );
    } else {
        println!("  {}", format!("{} run(s) in history", count).dimmed());
    }
    println!();
}

fn print_clean_rate(logs: &[RunLog]) {
    let clean = logs
        .iter()
        .filter(|l| l.summary.errors == 0 && l.summary.warnings == 0 && l.summary.info == 0)
        .count();
    let pct = 100.0 * clean as f64 / logs.len() as f64;
    let bar = progress_bar(pct, 30);
    let label = format!("{:.0}% clean ({}/{})", pct, clean, logs.len());
    let colored_label = if pct >= 80.0 {
        label.green()
    } else if pct >= 50.0 {
        label.yellow()
    } else {
        label.red()
    };
    println!("  {} {}", "Clean run rate".bold(), colored_label);
    println!("  {}", bar);
    println!();
}

fn print_severity_averages(logs: &[RunLog]) {
    let n = logs.len() as f64;
    let avg_err = logs.iter().map(|l| l.summary.errors).sum::<usize>() as f64 / n;
    let avg_warn = logs.iter().map(|l| l.summary.warnings).sum::<usize>() as f64 / n;
    let avg_info = logs.iter().map(|l| l.summary.info).sum::<usize>() as f64 / n;
    let avg_supp = logs.iter().map(|l| l.summary.suppressed).sum::<usize>() as f64 / n;

    println!("  {}", "Average findings per run".bold());
    println!(
        "    {} errors   {} warnings   {} info   {} suppressed",
        format!("{:.1}", avg_err).red(),
        format!("{:.1}", avg_warn).yellow(),
        format!("{:.1}", avg_info).blue(),
        format!("{:.1}", avg_supp).dimmed(),
    );
    println!();
}

fn print_trend(now_secs: u64, logs: &[RunLog]) {
    const WEEK: u64 = 7 * 24 * 3600;
    let this_week: Vec<_> = logs
        .iter()
        .filter(|l| now_secs.saturating_sub(l.timestamp) < WEEK)
        .collect();
    let last_week: Vec<_> = logs
        .iter()
        .filter(|l| {
            let age = now_secs.saturating_sub(l.timestamp);
            (WEEK..2 * WEEK).contains(&age)
        })
        .collect();

    if this_week.is_empty() && last_week.is_empty() {
        return;
    }

    let total_findings = |runs: &[&RunLog]| -> usize {
        runs.iter()
            .map(|l| l.summary.errors + l.summary.warnings + l.summary.info)
            .sum()
    };

    let this_total = total_findings(&this_week);
    let last_total = total_findings(&last_week);

    println!("  {}", "Week-over-week trend".bold());
    println!(
        "    This week: {} finding(s) across {} run(s)",
        this_total.to_string().bold(),
        this_week.len()
    );

    if !last_week.is_empty() {
        let arrow = if this_total < last_total {
            "↓".green()
        } else if this_total > last_total {
            "↑".red()
        } else {
            "→".dimmed()
        };
        let diff = this_total.abs_diff(last_total);
        println!(
            "    Last week:  {} finding(s) across {} run(s)  {} {}",
            last_total,
            last_week.len(),
            arrow,
            format!("{} vs last week", diff).dimmed(),
        );
    }
    println!();
}

fn print_noisiest_rules(logs: &[RunLog]) {
    let mut rule_counts: HashMap<String, usize> = HashMap::new();
    for log in logs {
        for f in &log.findings {
            if !f.suppressed {
                *rule_counts.entry(rule_prefix(&f.id)).or_default() += 1;
            }
        }
    }

    if rule_counts.is_empty() {
        return;
    }

    let mut rules: Vec<(String, usize)> = rule_counts.into_iter().collect();
    rules.sort_by(|a, b| b.1.cmp(&a.1));
    rules.truncate(5);

    println!("  {}", "Noisiest rules (top 5)".bold());
    let max = rules[0].1;
    for (rule, count) in &rules {
        let bar = mini_bar(*count, max, 20);
        println!("    {:<20} {} {}", rule, bar, count.to_string().bold());
    }
    println!();
}

fn print_suppression_stats(logs: &[RunLog]) {
    let mut sup_counts: HashMap<String, usize> = HashMap::new();
    for log in logs {
        for f in &log.findings {
            if f.suppressed {
                *sup_counts.entry(rule_prefix(&f.id)).or_default() += 1;
            }
        }
    }

    if sup_counts.is_empty() {
        return;
    }

    let mut rules: Vec<(String, usize)> = sup_counts.into_iter().collect();
    rules.sort_by(|a, b| b.1.cmp(&a.1));
    rules.truncate(5);

    println!("  {}", "Most suppressed rules (top 5)".bold());
    println!(
        "  {}",
        "High suppression may indicate noisy or misconfigured rules.".dimmed()
    );
    let max = rules[0].1;
    for (rule, count) in &rules {
        let bar = mini_bar(*count, max, 20);
        println!(
            "    {:<20} {} {} suppressed",
            rule,
            bar,
            count.to_string().bold()
        );
    }
    println!();
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract the rule prefix from a finding ID (e.g. "SEC" from "SEC-001").
fn rule_prefix(id: &str) -> String {
    id.split('-')
        .next()
        .unwrap_or(id)
        .to_uppercase()
        .to_string()
}

/// ASCII progress bar for a percentage (0–100).
fn progress_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    format!(
        "  [{}{}] {:.0}%",
        "█".repeat(filled).green(),
        "░".repeat(empty).dimmed(),
        pct
    )
}

/// Compact bar for relative comparison (filled proportional to max).
fn mini_bar(value: usize, max: usize, width: usize) -> String {
    let filled = if max == 0 {
        0
    } else {
        ((value as f64 / max as f64) * width as f64).round() as usize
    };
    let filled = filled.min(width);
    let empty = width - filled;
    format!(
        "[{}{}]",
        "█".repeat(filled).yellow(),
        "░".repeat(empty).dimmed()
    )
}
