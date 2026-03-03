//! Diff command — show findings only on changed lines

use anyhow::Result;
use colored::Colorize;
use revet_core::{
    apply_fixes, filter_findings, filter_findings_by_diff, filter_findings_by_inline,
    AnalyzerDispatcher, Baseline, DiffAnalyzer, Finding, ParserDispatcher, RevetConfig, Severity,
    SuppressedFinding,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::review::{build_summary, has_extension, has_filename, ReviewExitCode};
use crate::output::{make_formatter, resolve_format};

pub fn run(base: &str, cli: &crate::Cli) -> Result<ReviewExitCode> {
    let start = Instant::now();
    let repo_path = std::fs::canonicalize(Path::new(".")).unwrap_or_else(|_| PathBuf::from("."));

    eprintln!(
        "{}",
        format!(
            "  revet v{} — diff analysis vs {}",
            revet_core::VERSION,
            base
        )
        .bold()
    );
    eprintln!();

    // ── 1. Config ────────────────────────────────────────────────
    let config = RevetConfig::find_and_load(&repo_path)?;
    let format = resolve_format(cli, &config);

    // ── 2. Diff discovery ────────────────────────────────────────
    let diff_analyzer = DiffAnalyzer::new(&repo_path)?;

    eprint!("  Discovering changed files (diff vs {})... ", base);
    let diff = diff_analyzer.get_diff(base, None)?;
    let changed = diff_analyzer.get_changed_files(&diff)?;

    let dispatcher = ParserDispatcher::new();
    let analyzer_dispatcher = AnalyzerDispatcher::new_with_config(&config);
    let extensions = dispatcher.supported_extensions();

    let extra_exts = analyzer_dispatcher.extra_extensions(&config);
    let extra_names = analyzer_dispatcher.extra_filenames(&config);
    let mut all_extensions: Vec<&str> = extensions.clone();
    for ext in &extra_exts {
        if !all_extensions.contains(ext) {
            all_extensions.push(ext);
        }
    }

    let files: Vec<PathBuf> = changed
        .into_iter()
        .filter_map(|cf| {
            if cf.change_type == revet_core::diff::ChangeType::Deleted {
                return None;
            }
            let abs = repo_path.join(&cf.path);
            if abs.exists()
                && (has_extension(&cf.path, &all_extensions)
                    || has_filename(&cf.path, &extra_names))
            {
                Some(abs)
            } else {
                None
            }
        })
        .collect();
    eprintln!("{} ({} files)", "done".green(), files.len());

    if files.is_empty() {
        let mut out = make_formatter(format, &repo_path, false);
        out.write_no_files(start.elapsed());
        out.finalize();
        return Ok(ReviewExitCode::Success);
    }

    // ── 3. Build diff line map ───────────────────────────────────
    eprint!("  Building diff line map... ");
    let diff_map = diff_analyzer.get_all_changed_lines(base)?;
    let changed_line_count: usize = diff_map
        .values()
        .map(|v| match v {
            revet_core::DiffFileLines::AllNew => 0, // can't count without reading
            revet_core::DiffFileLines::Lines(set) => set.len(),
        })
        .sum();
    eprintln!(
        "{} ({} files, {} changed lines tracked)",
        "done".green(),
        diff_map.len(),
        changed_line_count
    );

    // ── 4. Parse (parallel) ────────────────────────────────────
    eprint!("  Building code graph... ");
    let graph_start = Instant::now();

    let (graph, parse_errors) = dispatcher.parse_files_parallel(&files, repo_path.clone());

    let node_count: usize = graph.nodes().count();
    eprintln!(
        "{} — {} files, {} nodes ({:.1}s)",
        "done".green(),
        files.len(),
        node_count,
        graph_start.elapsed().as_secs_f64()
    );

    // ── 5. Domain Analyzers ──────────────────────────────────────
    let mut findings: Vec<Finding> = Vec::new();

    // Add parse errors as findings
    for err_msg in &parse_errors {
        findings.push(Finding {
            id: format!("PARSE-{:03}", findings.len() + 1),
            severity: Severity::Warning,
            message: format!("Parse error: {}", err_msg),
            file: PathBuf::new(),
            line: 0,
            affected_dependents: 0,
            suggestion: None,
            fix_kind: None,
            ..Default::default()
        });
    }

    eprint!("  Running domain analyzers... ");
    let analyzer_start = Instant::now();
    let analyzer_findings = analyzer_dispatcher.run_all_parallel(&files, &repo_path, &config);
    let analyzer_count = analyzer_findings.len();
    findings.extend(analyzer_findings);
    eprintln!(
        "{} — {} finding(s) ({:.1}s)",
        "done".green(),
        analyzer_count,
        analyzer_start.elapsed().as_secs_f64()
    );

    // ── 6. Apply fixes (before filtering) ────────────────────────
    if cli.fix {
        eprint!("  Applying fixes... ");
        match apply_fixes(&findings) {
            Ok(report) => eprintln!(
                "{} ({} applied, {} suggestion-only)",
                "done".green(),
                report.applied,
                report.skipped
            ),
            Err(e) => eprintln!("{}: {}", "failed".red(), e),
        }
    }

    // ── 7. Filter by diff lines ──────────────────────────────────
    let (new_findings, diff_filtered) = filter_findings_by_diff(findings, &diff_map, &repo_path);
    findings = new_findings;

    // ── 8. Inline suppression ────────────────────────────────────
    let mut all_suppressed: Vec<SuppressedFinding> = Vec::new();
    let (new_findings, inline_suppressed) = filter_findings_by_inline(findings);
    findings = new_findings;
    all_suppressed.extend(inline_suppressed);

    // ── 9. Baseline suppression ──────────────────────────────────
    if !cli.no_baseline {
        if let Some(baseline) = Baseline::load(&repo_path)? {
            let (new_findings, baseline_suppressed) =
                filter_findings(findings, &baseline, &repo_path);
            findings = new_findings;
            all_suppressed.extend(baseline_suppressed);
        }
    }

    // ── 10. Output ───────────────────────────────────────────────
    let summary = build_summary(&findings, files.len(), node_count);

    let mut out = make_formatter(format, &repo_path, cli.show_suppressed);
    for f in &findings {
        out.write_finding(f, &repo_path);
    }
    if cli.show_suppressed {
        for sf in &all_suppressed {
            out.write_suppressed(sf, &repo_path);
        }
    }
    out.write_summary(&summary, &all_suppressed, start.elapsed(), None);
    out.finalize();

    if diff_filtered > 0 {
        println!(
            "  {}",
            format!(
                "{} finding(s) on unchanged lines filtered out",
                diff_filtered
            )
            .dimmed()
        );
    }

    let fail_on = cli.fail_on.as_deref().unwrap_or(&config.general.fail_on);
    if summary.exceeds_threshold(fail_on) {
        Ok(ReviewExitCode::FindingsExceedThreshold)
    } else {
        Ok(ReviewExitCode::Success)
    }
}
