//! `revet report --html` — generate a self-contained HTML quality report.
//!
//! Reads run history from `.revet-cache/runs/` and produces a portable HTML
//! file with CSS charts. No external dependencies — the file works offline.

use anyhow::{bail, Result};
use std::collections::HashMap;
use std::path::Path;

use crate::run_log::{self, RunLog};

pub fn run(repo_path: &Path, output: &str, last_n: Option<usize>) -> Result<()> {
    let entries = run_log::list_runs(repo_path)?;

    if entries.is_empty() {
        bail!("No run history found. Run `revet review` first to build history.");
    }

    let mut logs: Vec<RunLog> = entries
        .iter()
        .filter_map(|e| run_log::load_run_log(repo_path, &e.id).ok())
        .collect();

    // Newest first
    logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    if let Some(n) = last_n {
        logs.truncate(n);
    }

    let html = render_html(&logs);
    std::fs::write(output, &html)?;
    eprintln!("  Report written to: {}", output);
    Ok(())
}

// ── Data aggregation ─────────────────────────────────────────────────────────

struct ReportData<'a> {
    logs: &'a [RunLog],
    total_errors: usize,
    total_warnings: usize,
    total_info: usize,
    debt_minutes: usize,
    clean_runs: usize,
    rule_counts: Vec<(String, usize)>,
    file_counts: Vec<(String, usize)>,
    trend: Vec<(String, usize)>, // (date, total_findings)
}

impl<'a> ReportData<'a> {
    fn build(logs: &'a [RunLog]) -> Self {
        let latest = logs.first();
        let total_errors = latest.map(|l| l.summary.errors).unwrap_or(0);
        let total_warnings = latest.map(|l| l.summary.warnings).unwrap_or(0);
        let total_info = latest.map(|l| l.summary.info).unwrap_or(0);
        let debt_minutes = total_errors * 60 + total_warnings * 30 + total_info * 10;

        let clean_runs = logs
            .iter()
            .filter(|l| l.summary.errors == 0 && l.summary.warnings == 0 && l.summary.info == 0)
            .count();

        // Rule counts from latest run's active findings
        let mut rule_map: HashMap<String, usize> = HashMap::new();
        if let Some(latest) = latest {
            for f in &latest.findings {
                if !f.suppressed {
                    let prefix = f.id.split('-').next().unwrap_or(&f.id).to_string();
                    *rule_map.entry(prefix).or_default() += 1;
                }
            }
        }
        let mut rule_counts: Vec<(String, usize)> = rule_map.into_iter().collect();
        rule_counts.sort_by(|a, b| b.1.cmp(&a.1));
        rule_counts.truncate(10);

        // File counts from latest run's active findings
        let mut file_map: HashMap<String, usize> = HashMap::new();
        if let Some(latest) = latest {
            for f in &latest.findings {
                if !f.suppressed && !f.file.is_empty() {
                    *file_map.entry(f.file.clone()).or_default() += 1;
                }
            }
        }
        let mut file_counts: Vec<(String, usize)> = file_map.into_iter().collect();
        file_counts.sort_by(|a, b| b.1.cmp(&a.1));
        file_counts.truncate(10);

        // Trend — last 14 runs, oldest first
        let mut trend: Vec<(String, usize)> = logs
            .iter()
            .take(14)
            .map(|l| {
                let date = ts_to_date(l.timestamp);
                let total = l.summary.errors + l.summary.warnings + l.summary.info;
                (date, total)
            })
            .collect();
        trend.reverse(); // oldest first for left-to-right chart

        Self {
            logs,
            total_errors,
            total_warnings,
            total_info,
            debt_minutes,
            clean_runs,
            rule_counts,
            file_counts,
            trend,
        }
    }
}

// ── HTML rendering ────────────────────────────────────────────────────────────

fn render_html(logs: &[RunLog]) -> String {
    let d = ReportData::build(logs);
    let date = if let Some(l) = logs.first() {
        ts_to_date(l.timestamp)
    } else {
        "—".to_string()
    };
    let clean_pct = if d.logs.is_empty() {
        0
    } else {
        d.clean_runs * 100 / d.logs.len()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Revet Quality Report — {date}</title>
<style>
  :root {{
    --bg: #0f1117; --card: #1a1d27; --border: #2a2d3a;
    --text: #e2e8f0; --muted: #64748b; --accent: #6366f1;
    --red: #ef4444; --yellow: #f59e0b; --blue: #3b82f6; --green: #22c55e;
  }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ background: var(--bg); color: var(--text); font-family: system-ui, sans-serif; padding: 2rem; }}
  h1 {{ font-size: 1.5rem; margin-bottom: 0.25rem; }}
  h2 {{ font-size: 1rem; font-weight: 600; color: var(--muted); text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 1rem; }}
  .subtitle {{ color: var(--muted); font-size: 0.875rem; margin-bottom: 2rem; }}
  .grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 1rem; margin-bottom: 2rem; }}
  .card {{ background: var(--card); border: 1px solid var(--border); border-radius: 0.75rem; padding: 1.25rem; }}
  .stat-value {{ font-size: 2rem; font-weight: 700; line-height: 1; }}
  .stat-label {{ color: var(--muted); font-size: 0.8rem; margin-top: 0.25rem; }}
  .red {{ color: var(--red); }} .yellow {{ color: var(--yellow); }}
  .blue {{ color: var(--blue); }} .green {{ color: var(--green); }}
  .section {{ margin-bottom: 2rem; }}
  .bar-row {{ display: flex; align-items: center; gap: 0.75rem; margin-bottom: 0.5rem; font-size: 0.85rem; }}
  .bar-label {{ width: 160px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; color: var(--text); }}
  .bar-track {{ flex: 1; background: var(--border); border-radius: 4px; height: 8px; }}
  .bar-fill {{ height: 8px; border-radius: 4px; background: var(--accent); }}
  .bar-count {{ width: 2.5rem; text-align: right; color: var(--muted); }}
  .trend {{ display: flex; align-items: flex-end; gap: 4px; height: 60px; }}
  .trend-bar {{ flex: 1; background: var(--accent); border-radius: 2px 2px 0 0; min-height: 2px; position: relative; }}
  .trend-bar:hover .trend-tip {{ display: block; }}
  .trend-tip {{ display: none; position: absolute; bottom: 110%; left: 50%; transform: translateX(-50%);
    background: var(--card); border: 1px solid var(--border); padding: 0.25rem 0.5rem;
    border-radius: 4px; font-size: 0.75rem; white-space: nowrap; z-index: 10; }}
  .trend-labels {{ display: flex; gap: 4px; margin-top: 0.25rem; }}
  .trend-label {{ flex: 1; font-size: 0.65rem; color: var(--muted); text-align: center; overflow: hidden; }}
  table {{ width: 100%; border-collapse: collapse; font-size: 0.85rem; }}
  th {{ text-align: left; color: var(--muted); padding: 0.5rem; border-bottom: 1px solid var(--border); }}
  td {{ padding: 0.5rem; border-bottom: 1px solid var(--border); vertical-align: top; }}
  tr:last-child td {{ border-bottom: none; }}
  .badge {{ display: inline-block; padding: 0.1rem 0.4rem; border-radius: 4px; font-size: 0.75rem; font-weight: 600; }}
  .badge-error {{ background: #450a0a; color: var(--red); }}
  .badge-warning {{ background: #431407; color: var(--yellow); }}
  .badge-info {{ background: #0c1a3a; color: var(--blue); }}
  .debt {{ font-size: 1.25rem; font-weight: 700; color: var(--yellow); }}
  footer {{ color: var(--muted); font-size: 0.75rem; margin-top: 3rem; text-align: center; }}
</style>
</head>
<body>
<h1>Revet Quality Report</h1>
<p class="subtitle">Generated {date} &nbsp;·&nbsp; {run_count} run(s) in history</p>

<div class="grid">
  <div class="card"><div class="stat-value {err_class}">{total_errors}</div><div class="stat-label">Errors</div></div>
  <div class="card"><div class="stat-value {warn_class}">{total_warnings}</div><div class="stat-label">Warnings</div></div>
  <div class="card"><div class="stat-value">{total_info}</div><div class="stat-label">Info</div></div>
  <div class="card"><div class="stat-value {green_class}">{clean_pct}%</div><div class="stat-label">Clean run rate ({clean_runs}/{run_count})</div></div>
  <div class="card"><div class="debt">{debt_str}</div><div class="stat-label">Technical debt</div></div>
</div>

{trend_section}

{rules_section}

{files_section}

{findings_section}

<footer>Generated by <strong>revet</strong> &nbsp;·&nbsp; <a href="https://github.com/umitk/revet" style="color: var(--accent)">github.com/umitk/revet</a></footer>
</body>
</html>"#,
        date = date,
        run_count = d.logs.len(),
        total_errors = d.total_errors,
        total_warnings = d.total_warnings,
        total_info = d.total_info,
        err_class = if d.total_errors > 0 { "red" } else { "" },
        warn_class = if d.total_warnings > 0 { "yellow" } else { "" },
        green_class = if clean_pct >= 80 {
            "green"
        } else if clean_pct >= 50 {
            "yellow"
        } else {
            "red"
        },
        clean_pct = clean_pct,
        clean_runs = d.clean_runs,
        debt_str = format_debt(d.debt_minutes),
        trend_section = render_trend(&d),
        rules_section = render_rules(&d),
        files_section = render_files(&d),
        findings_section = render_findings(logs),
    )
}

fn render_trend(d: &ReportData) -> String {
    if d.trend.is_empty() {
        return String::new();
    }
    let max = d.trend.iter().map(|(_, v)| *v).max().unwrap_or(1).max(1);
    let bars: String = d
        .trend
        .iter()
        .map(|(date, count)| {
            let pct = (*count as f64 / max as f64 * 100.0) as usize;
            format!(
                r#"<div class="trend-bar" style="height:{}%"><span class="trend-tip">{}<br/>{} findings</span></div>"#,
                pct.max(2),
                date,
                count
            )
        })
        .collect();
    let labels: String = d
        .trend
        .iter()
        .map(|(date, _)| {
            let short = &date[5..]; // MM-DD
            format!(r#"<div class="trend-label">{}</div>"#, short)
        })
        .collect();

    format!(
        r#"<div class="section card">
<h2>Finding trend (last {} runs)</h2>
<div class="trend">{}</div>
<div class="trend-labels">{}</div>
</div>"#,
        d.trend.len(),
        bars,
        labels
    )
}

fn render_rules(d: &ReportData) -> String {
    if d.rule_counts.is_empty() {
        return String::new();
    }
    let max = d.rule_counts.iter().map(|(_, v)| *v).max().unwrap_or(1);
    let rows: String = d
        .rule_counts
        .iter()
        .map(|(rule, count)| {
            let pct = (*count as f64 / max as f64 * 100.0) as usize;
            format!(
                r#"<div class="bar-row">
  <div class="bar-label">{rule}</div>
  <div class="bar-track"><div class="bar-fill" style="width:{pct}%"></div></div>
  <div class="bar-count">{count}</div>
</div>"#
            )
        })
        .collect();

    format!(
        r#"<div class="section card">
<h2>Top rules (latest run)</h2>
{rows}
</div>"#
    )
}

fn render_files(d: &ReportData) -> String {
    if d.file_counts.is_empty() {
        return String::new();
    }
    let max = d.file_counts.iter().map(|(_, v)| *v).max().unwrap_or(1);
    let rows: String = d
        .file_counts
        .iter()
        .map(|(file, count)| {
            let pct = (*count as f64 / max as f64 * 100.0) as usize;
            // Show only the last 2 path components for readability
            let short = short_path(file);
            format!(
                r#"<div class="bar-row">
  <div class="bar-label" title="{file}">{short}</div>
  <div class="bar-track"><div class="bar-fill" style="width:{pct}%; background:#f59e0b"></div></div>
  <div class="bar-count">{count}</div>
</div>"#
            )
        })
        .collect();

    format!(
        r#"<div class="section card">
<h2>Top files by findings (latest run)</h2>
{rows}
</div>"#
    )
}

fn render_findings(logs: &[RunLog]) -> String {
    let latest = match logs.first() {
        Some(l) => l,
        None => return String::new(),
    };

    let active: Vec<_> = latest.findings.iter().filter(|f| !f.suppressed).collect();
    if active.is_empty() {
        return r#"<div class="section card"><h2>Findings (latest run)</h2><p style="color:var(--green);margin-top:.5rem">✓ No active findings</p></div>"#.to_string();
    }

    let rows: String = active
        .iter()
        .map(|f| {
            let badge = match f.severity.as_str() {
                "error" => r#"<span class="badge badge-error">error</span>"#,
                "warning" => r#"<span class="badge badge-warning">warn</span>"#,
                _ => r#"<span class="badge badge-info">info</span>"#,
            };
            let loc = if f.line > 0 {
                format!("{}:{}", f.file, f.line)
            } else {
                f.file.clone()
            };
            format!(
                "<tr><td>{}</td><td>{}</td><td style='color:var(--muted);font-family:monospace;font-size:0.8rem'>{}</td><td>{}</td></tr>",
                f.id, badge, html_escape(&loc), html_escape(&f.message)
            )
        })
        .collect();

    format!(
        r#"<div class="section card">
<h2>Findings (latest run — {} active)</h2>
<table>
<thead><tr><th>ID</th><th>Severity</th><th>Location</th><th>Message</th></tr></thead>
<tbody>{rows}</tbody>
</table>
</div>"#,
        active.len()
    )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn format_debt(minutes: usize) -> String {
    if minutes == 0 {
        return "0m".to_string();
    }
    if minutes < 60 {
        format!("{}m", minutes)
    } else {
        let h = minutes / 60;
        let m = minutes % 60;
        if m == 0 {
            format!("{}h", h)
        } else {
            format!("{}h {}m", h, m)
        }
    }
}

fn ts_to_date(ts: u64) -> String {
    let days = ts / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, mo, d)
}

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

fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 {
        path.to_string()
    } else {
        format!("…/{}", parts[parts.len() - 2..].join("/"))
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
