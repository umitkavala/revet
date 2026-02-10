//! Baseline command — snapshot current findings so future reviews only report new ones

use anyhow::Result;
use colored::Colorize;
use revet_core::{
    discover_files, discover_files_extended, AnalyzerDispatcher, Baseline, CodeGraph, GraphCache,
    ParserDispatcher, RevetConfig, Severity,
};
use std::path::Path;
use std::time::Instant;

pub fn run(path: Option<&Path>, clear: bool) -> Result<()> {
    let repo_path = path.unwrap_or_else(|| Path::new("."));
    let repo_path = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());

    if clear {
        let removed = Baseline::clear(&repo_path)?;
        if removed {
            eprintln!("  {}", "Baseline cleared.".green());
        } else {
            eprintln!("  {}", "No baseline to clear.".dimmed());
        }
        return Ok(());
    }

    let start = Instant::now();
    eprintln!(
        "{}",
        format!("  revet v{} — creating baseline", revet_core::VERSION).bold()
    );
    eprintln!();

    // ── 1. Config ────────────────────────────────────────────────
    let config = RevetConfig::find_and_load(&repo_path)?;

    // ── 2. File Discovery (always full scan for baseline) ────────
    let dispatcher = ParserDispatcher::new();
    let analyzer_dispatcher = AnalyzerDispatcher::new();
    let extensions = dispatcher.supported_extensions();

    let extra_exts = analyzer_dispatcher.extra_extensions(&config);
    let extra_names = analyzer_dispatcher.extra_filenames(&config);
    let mut all_extensions: Vec<&str> = extensions.clone();
    for ext in &extra_exts {
        if !all_extensions.contains(ext) {
            all_extensions.push(ext);
        }
    }

    eprint!("  Discovering files (full scan)... ");
    let files = if extra_names.is_empty() {
        discover_files(&repo_path, &all_extensions, &config.ignore.paths)?
    } else {
        discover_files_extended(
            &repo_path,
            &all_extensions,
            &extra_names,
            &config.ignore.paths,
        )?
    };
    eprintln!("{} ({} files)", "done".green(), files.len());

    // ── 3. Parse ─────────────────────────────────────────────────
    eprint!("  Building code graph... ");
    let mut graph = CodeGraph::new(repo_path.clone());
    for file in &files {
        let _ = dispatcher.parse_file(file, &mut graph);
    }
    let node_count: usize = graph.nodes().count();
    eprintln!("{} ({} nodes)", "done".green(), node_count);

    // ── 4. Domain Analyzers ──────────────────────────────────────
    eprint!("  Running domain analyzers... ");
    let findings = analyzer_dispatcher.run_all(&files, &repo_path, &config);
    eprintln!("{} ({} findings)", "done".green(), findings.len());

    // ── 5. Save Baseline ─────────────────────────────────────────
    let commit = GraphCache::get_git_commit_hash(&repo_path);
    let baseline = Baseline::from_findings(&findings, &repo_path, commit);
    baseline.save(&repo_path)?;

    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warnings = findings
        .iter()
        .filter(|f| f.severity == Severity::Warning)
        .count();
    let info = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();

    eprintln!();
    eprintln!(
        "  {} ({} errors, {} warnings, {} info)",
        format!("Baselined {} finding(s)", findings.len())
            .green()
            .bold(),
        errors,
        warnings,
        info,
    );
    eprintln!("  Time: {:.1}s", start.elapsed().as_secs_f64());

    Ok(())
}
