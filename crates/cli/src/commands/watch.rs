//! Watch command — monitor files and re-analyze on changes

use anyhow::Result;
use colored::Colorize;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use revet_core::{
    apply_fixes, discover_files_extended, filter_findings, filter_findings_by_inline,
    AnalyzerDispatcher, Baseline, Finding, ParserDispatcher, RevetConfig, Severity,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::review::{
    build_summary, has_extension, has_filename, print_github, print_json, print_no_files,
    print_sarif, print_terminal, resolve_format, Format,
};

pub fn run(path: Option<&Path>, cli: &crate::Cli, debounce_ms: u64, no_clear: bool) -> Result<()> {
    let repo_path = path.unwrap_or_else(|| Path::new("."));
    let repo_path = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());

    eprintln!(
        "{}",
        format!("  revet v{} — watch mode", revet_core::VERSION).bold()
    );
    eprintln!();

    // ── Initial run ────────────────────────────────────────────
    run_analysis(&repo_path, cli)?;
    eprintln!();
    eprintln!("  {}", "Watching for changes... (Ctrl-C to stop)".dimmed());

    // ── Ctrl-C handler ─────────────────────────────────────────
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })?;

    // ── Collect supported file types ───────────────────────────
    let config = RevetConfig::find_and_load(&repo_path).unwrap_or_default();
    let dispatcher = ParserDispatcher::new();
    let analyzer_dispatcher = AnalyzerDispatcher::new_with_config(&config);

    let extensions = dispatcher.supported_extensions();
    let extra_exts = analyzer_dispatcher.extra_extensions(&config);
    let extra_names = analyzer_dispatcher.extra_filenames(&config);
    let mut all_extensions: Vec<&str> = extensions;
    for ext in &extra_exts {
        if !all_extensions.contains(ext) {
            all_extensions.push(ext);
        }
    }

    // ── Set up file watcher ────────────────────────────────────
    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(Duration::from_millis(debounce_ms), tx)?;

    use notify::RecursiveMode;
    debouncer
        .watcher()
        .watch(repo_path.as_ref(), RecursiveMode::Recursive)?;

    // ── Event loop ─────────────────────────────────────────────
    while running.load(Ordering::SeqCst) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(events)) => {
                let dominated = events.iter().any(|ev| {
                    if ev.kind != DebouncedEventKind::Any {
                        return false;
                    }
                    let p = &ev.path;
                    // Skip .git/ and .revet-cache/ directories
                    if path_contains_segment(p, ".git") || path_contains_segment(p, ".revet-cache")
                    {
                        return false;
                    }
                    has_extension(p, &all_extensions) || has_filename(p, &extra_names)
                });

                if dominated {
                    if !no_clear {
                        clear_screen();
                    } else {
                        eprintln!();
                        eprintln!("  {}", "\u{2500}".repeat(60).dimmed());
                        eprintln!();
                    }

                    match run_analysis(&repo_path, cli) {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("  {}: {}", "analysis error".red(), e);
                        }
                    }
                    eprintln!();
                    eprintln!("  {}", "Watching for changes... (Ctrl-C to stop)".dimmed());
                }
            }
            Ok(Err(errs)) => {
                eprintln!("  {}: {:?}", "watch error".red(), errs);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Normal timeout — check if we should keep running
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    eprintln!();
    eprintln!("  {}", "Stopped watching.".bold());
    Ok(())
}

fn run_analysis(repo_path: &Path, cli: &crate::Cli) -> Result<()> {
    let start = Instant::now();

    // ── 1. Config (re-load each run) ──────────────────────────
    let config = match RevetConfig::find_and_load(repo_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  {}: {}", "config error".red(), e);
            RevetConfig::default()
        }
    };
    let format = resolve_format(cli, &config);

    // ── 2. File discovery (full scan) ─────────────────────────
    let dispatcher = ParserDispatcher::new();
    let analyzer_dispatcher = AnalyzerDispatcher::new_with_config(&config);

    let extensions = dispatcher.supported_extensions();
    let extra_exts = analyzer_dispatcher.extra_extensions(&config);
    let extra_names = analyzer_dispatcher.extra_filenames(&config);
    let mut all_extensions: Vec<&str> = extensions;
    for ext in &extra_exts {
        if !all_extensions.contains(ext) {
            all_extensions.push(ext);
        }
    }

    eprint!("  Discovering files... ");
    let files = discover_files_extended(
        repo_path,
        &all_extensions,
        &extra_names,
        &config.ignore.paths,
    )?;
    eprintln!("{} ({} files)", "done".green(), files.len());

    if files.is_empty() {
        print_no_files(format, start);
        return Ok(());
    }

    // ── 3. Parse (parallel) ────────────────────────────────────
    eprint!("  Building code graph... ");
    let graph_start = Instant::now();

    let (graph, parse_errors) = dispatcher.parse_files_parallel(&files, repo_path.to_path_buf());

    let node_count: usize = graph.nodes().count();
    eprintln!(
        "{} \u{2014} {} files, {} nodes ({:.1}s)",
        "done".green(),
        files.len(),
        node_count,
        graph_start.elapsed().as_secs_f64()
    );

    // ── 4. Domain analyzers ───────────────────────────────────
    let mut findings: Vec<Finding> = Vec::new();

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
    let analyzer_findings = analyzer_dispatcher.run_all_parallel(&files, repo_path, &config);
    let analyzer_count = analyzer_findings.len();
    findings.extend(analyzer_findings);
    eprintln!(
        "{} \u{2014} {} finding(s) ({:.1}s)",
        "done".green(),
        analyzer_count,
        analyzer_start.elapsed().as_secs_f64()
    );

    // ── 5. Apply fixes ────────────────────────────────────────
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

    // ── 6. Inline suppression ─────────────────────────────────
    let (new_findings, inline_suppressed) = filter_findings_by_inline(findings);
    findings = new_findings;

    // ── 7. Baseline suppression ───────────────────────────────
    let mut suppressed_count = 0usize;
    if !cli.no_baseline {
        if let Some(baseline) = Baseline::load(repo_path)? {
            let (new_findings, suppressed) = filter_findings(findings, &baseline, repo_path);
            findings = new_findings;
            suppressed_count = suppressed;
        }
    }

    // ── 8. Output ─────────────────────────────────────────────
    let summary = build_summary(&findings, files.len(), node_count);

    match format {
        Format::Json => print_json(&findings, &summary),
        Format::Sarif => print_sarif(&findings, repo_path),
        Format::Github => print_github(&findings, repo_path),
        Format::Terminal => print_terminal(
            &findings,
            &summary,
            repo_path,
            start,
            suppressed_count,
            inline_suppressed,
        ),
    }

    Ok(())
}

fn path_contains_segment(path: &Path, segment: &str) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_str() == Some(segment))
}

fn clear_screen() {
    // ANSI escape: clear screen + move cursor to top-left
    eprint!("\x1B[2J\x1B[H");
}
