//! Main review command — wires parser, graph, impact analysis, and cache together

use anyhow::Result;
use colored::Colorize;
use revet_core::{
    apply_fixes, create_store, discover_files, discover_files_extended, filter_findings,
    filter_findings_by_diff, filter_findings_by_inline, filter_findings_by_path_rules,
    reconstruct_graph, AnalyzerDispatcher, Baseline, CodeGraph, DiffAnalyzer, FileGraphCache,
    Finding, GitTreeReader, GraphCache, GraphCacheMeta, GraphStore, ImpactAnalysis,
    ParserDispatcher, RevetConfig, ReviewSummary, Severity,
};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::ai::AiReasoner;
use crate::output;
use crate::output::github_comment;

/// Exit status from the review command
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewExitCode {
    /// No findings exceeding the configured threshold
    Success,
    /// Findings exceeded the configured threshold
    FindingsExceedThreshold,
}

pub fn run(path: Option<&Path>, cli: &crate::Cli) -> Result<ReviewExitCode> {
    let start = Instant::now();
    let repo_path = path.unwrap_or_else(|| Path::new("."));
    let repo_path = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());

    eprintln!(
        "{}",
        format!("  revet v{} — analyzing repository", revet_core::VERSION).bold()
    );
    eprintln!();

    // ── 1. Config ────────────────────────────────────────────────
    let config = RevetConfig::find_and_load(&repo_path)?;
    let format = resolve_format(cli, &config);

    // ── 2. File Discovery ────────────────────────────────────────
    let dispatcher = ParserDispatcher::new();
    let analyzer_dispatcher = AnalyzerDispatcher::new_with_config(&config);
    let extensions = dispatcher.supported_extensions();

    // Merge parser extensions with analyzer-specific extensions
    let extra_exts = analyzer_dispatcher.extra_extensions(&config);
    let extra_names = analyzer_dispatcher.extra_filenames(&config);
    let mut all_extensions: Vec<&str> = extensions.clone();
    for ext in &extra_exts {
        if !all_extensions.contains(ext) {
            all_extensions.push(ext);
        }
    }

    let files = discover_review_files(&repo_path, cli, &config, &all_extensions, &extra_names)?;

    if files.is_empty() {
        print_no_files(format, start);
        return Ok(ReviewExitCode::Success);
    }

    // ── 3. Parse (incremental, cache-aware) ──────────────────────
    eprint!("  Building code graph... ");
    let _ = std::io::stderr().flush();
    let graph_start = Instant::now();

    let file_cache = FileGraphCache::new(&repo_path);
    let (graph, parse_errors, cached_count, parsed_count) =
        dispatcher.parse_files_incremental(&files, repo_path.clone(), &file_cache);

    let node_count: usize = graph.nodes().count();
    eprintln!(
        "{} — {} files ({} cached, {} parsed), {} nodes ({:.1}s)",
        "done".green(),
        files.len(),
        cached_count,
        parsed_count,
        node_count,
        graph_start.elapsed().as_secs_f64()
    );

    // ── 4. Impact Analysis ───────────────────────────────────────
    let mut findings: Vec<Finding> = Vec::new();

    let old_graph = load_old_graph(&repo_path, cli, &config, &dispatcher);

    if let Some(baseline) = old_graph {
        eprint!("  Running impact analysis... ");
        let _ = std::io::stderr().flush();
        let impact_start = Instant::now();

        let analysis = ImpactAnalysis::new(baseline, graph.clone());
        let report = analysis.analyze_impact();

        for change in &report.changes {
            let severity = match change.classification {
                revet_core::ChangeClassification::Breaking => Severity::Error,
                revet_core::ChangeClassification::PotentiallyBreaking => Severity::Warning,
                revet_core::ChangeClassification::Safe => {
                    continue; // safe changes are never reported — not actionable
                }
            };

            let node = match analysis.new_graph().node(change.node_id) {
                Some(n) => n,
                None => continue,
            };

            let total_deps = change.direct_dependents.len() + change.transitive_dependents.len();

            findings.push(Finding {
                id: format!("IMPACT-{:03}", findings.len() + 1),
                severity,
                message: format!(
                    "{:?} change in `{}` — {} dependent(s) affected",
                    change.classification,
                    node.name(),
                    total_deps,
                ),
                file: node.file_path().clone(),
                line: node.line(),
                affected_dependents: total_deps,
                suggestion: None,
                fix_kind: None,
                ..Default::default()
            });
        }

        eprintln!(
            "{} ({:.1}s)",
            "done".green(),
            impact_start.elapsed().as_secs_f64()
        );
    } else {
        eprintln!(
            "  {} — run again to compare changes",
            "No baseline graph available, skipping impact analysis".dimmed()
        );
    }

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

    // ── 4b. Domain Analyzers ─────────────────────────────────────
    eprint!("  Running domain analyzers... ");
    let _ = std::io::stderr().flush();
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

    // ── 4b'. Graph analyzers ─────────────────────────────────────────
    eprint!("  Running graph analyzers... ");
    let _ = std::io::stderr().flush();
    let ga_start = Instant::now();
    let graph_findings = analyzer_dispatcher.run_graph_analyzers(&graph, &config);
    let graph_count = graph_findings.len();
    findings.extend(graph_findings);
    eprintln!(
        "{} — {} finding(s) ({:.1}s)",
        "done".green(),
        graph_count,
        ga_start.elapsed().as_secs_f64()
    );

    // ── 4c. AI reasoning ─────────────────────────────────────────
    if cli.ai {
        let eligible = findings
            .iter()
            .filter(|f| {
                matches!(f.severity, Severity::Warning | Severity::Error) && f.suggestion.is_none()
            })
            .count();
        eprint!("  Running AI reasoning ({} findings)... ", eligible);
        let _ = std::io::stderr().flush();
        let ai_start = Instant::now();
        let reasoner = AiReasoner::new(config.ai.clone(), cli.max_cost);
        match reasoner.enrich(&mut findings, &repo_path) {
            Ok(stats) => eprintln!(
                "{} — {} enriched, {} false positives (${:.4}, {:.1}s)",
                "done".green(),
                stats.findings_enriched,
                stats.false_positives,
                stats.cost_usd,
                ai_start.elapsed().as_secs_f64()
            ),
            Err(e) => eprintln!("{}: {}", "warn".yellow(), e),
        }
    }

    // ── 4d. Apply fixes ───────────────────────────────────────────
    if cli.fix {
        eprint!("  Applying fixes... ");
        let _ = std::io::stderr().flush();
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

    // ── 4d. Inline suppression ───────────────────────────────────
    let (new_findings, inline_suppressed) = filter_findings_by_inline(findings);
    findings = new_findings;

    // ── 4e. Per-path rule suppression ────────────────────────────
    if !config.ignore.per_path.is_empty() {
        let (new_findings, path_suppressed) =
            filter_findings_by_path_rules(findings, &config.ignore.per_path, &repo_path);
        findings = new_findings;
        if path_suppressed > 0 {
            eprintln!(
                "  {} finding(s) suppressed by per-path rules",
                path_suppressed
            );
        }
    }

    // ── 4f. Baseline suppression ───────────────────────────────────
    let mut suppressed_count = 0usize;
    if !cli.no_baseline {
        if let Some(baseline) = Baseline::load(&repo_path)? {
            let (new_findings, suppressed) = filter_findings(findings, &baseline, &repo_path);
            findings = new_findings;
            suppressed_count = suppressed;
        }
    }

    // ── 5. Save Cache (CozoStore + metadata) ─────────────────────
    let file_paths: Vec<PathBuf> = files
        .iter()
        .map(|f| f.strip_prefix(&repo_path).unwrap_or(f).to_path_buf())
        .collect();

    let checksums = GraphCache::build_file_checksums(&repo_path, &file_paths).unwrap_or_default();

    let meta = GraphCacheMeta {
        commit_hash: GraphCache::get_git_commit_hash(&repo_path),
        timestamp: SystemTime::now(),
        file_checksums: checksums,
        revet_version: revet_core::VERSION.to_string(),
    };

    match create_store(&repo_path) {
        Ok(store) => {
            let _ = store.delete_snapshot("cached");
            if let Err(e) = store.flush(&graph, "cached") {
                eprintln!(
                    "  {}: failed to save graph to store: {}",
                    "warn".yellow(),
                    e
                );
            }
        }
        Err(e) => {
            eprintln!("  {}: failed to create store: {}", "warn".yellow(), e);
        }
    }

    let cache = GraphCache::new(&repo_path);
    if let Err(e) = cache.save(&graph, &meta) {
        eprintln!("  {}: failed to save graph cache: {}", "warn".yellow(), e);
    }

    // ── 5b. Post GitHub PR comments ──────────────────────────────
    if cli.post_comment {
        post_github_comments(&findings, &repo_path, cli);
    }

    // ── 6. Output ────────────────────────────────────────────────
    let summary = build_summary(&findings, files.len(), node_count);

    match format {
        Format::Json => print_json(&findings, &summary),
        Format::Sarif => print_sarif(&findings, &repo_path),
        Format::Github => print_github(&findings, &repo_path),
        Format::Terminal => print_terminal(
            &findings,
            &summary,
            &repo_path,
            start,
            suppressed_count,
            inline_suppressed,
        ),
    }

    let fail_on = cli.fail_on.as_deref().unwrap_or(&config.general.fail_on);
    if summary.exceeds_threshold(fail_on) {
        Ok(ReviewExitCode::FindingsExceedThreshold)
    } else {
        Ok(ReviewExitCode::Success)
    }
}

// ── Helpers ──────────────────────────────────────────────────────

/// Load the old (baseline) graph for impact analysis.
///
/// Fallback chain: CozoStore → git blobs → None
fn load_old_graph(
    repo_path: &Path,
    cli: &crate::Cli,
    config: &RevetConfig,
    dispatcher: &ParserDispatcher,
) -> Option<CodeGraph> {
    // 1. Try msgpack cache (fast path — serialized whole graph)
    let cache = GraphCache::new(repo_path);
    eprint!("  Loading baseline graph from cache... ");
    let _ = std::io::stderr().flush();
    let baseline_start = Instant::now();
    match cache.load() {
        Ok(Some((cached_graph, _))) => {
            eprintln!(
                "{} ({} nodes, {:.1}s)",
                "done".green(),
                cached_graph.nodes().count(),
                baseline_start.elapsed().as_secs_f64()
            );
            return Some(cached_graph);
        }
        Ok(None) => eprintln!("{}", "not found — will build from git".dimmed()),
        Err(e) => eprintln!("{}: {}", "warn".yellow(), e),
    }

    // 2. Try CozoStore (slower fallback)
    if let Ok(store) = create_store(repo_path) {
        let snaps = store.snapshots().unwrap_or_default();
        if snaps.iter().any(|s| s.name == "cached") {
            eprint!("  Loading baseline graph from store... ");
            let _ = std::io::stderr().flush();
            let baseline_start = Instant::now();
            match reconstruct_graph(&store, "cached", repo_path) {
                Ok(graph) => {
                    eprintln!(
                        "{} ({} nodes, {:.1}s)",
                        "done".green(),
                        graph.nodes().count(),
                        baseline_start.elapsed().as_secs_f64()
                    );
                    return Some(graph);
                }
                Err(e) => {
                    eprintln!("{}: {}", "warn".yellow(), e);
                }
            }
        }
    }

    // 2. Try building from git blobs at the base ref
    let base = cli.diff.as_deref().unwrap_or(&config.general.diff_base);
    match GitTreeReader::new(repo_path) {
        Ok(reader) => {
            eprint!("  Building baseline graph from git ({})... ", base);
            let _ = std::io::stderr().flush();
            let baseline_start = Instant::now();
            match reader.build_graph_at_ref(base, repo_path, dispatcher) {
                Ok(blob_graph) => {
                    let node_count: usize = blob_graph.nodes().count();
                    eprintln!(
                        "{} ({} nodes, {:.1}s)",
                        "done".green(),
                        node_count,
                        baseline_start.elapsed().as_secs_f64()
                    );
                    Some(blob_graph)
                }
                Err(e) => {
                    eprintln!("{}", format!("failed: {}", e).dimmed());
                    None
                }
            }
        }
        Err(_) => None,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Format {
    Terminal,
    Json,
    Sarif,
    Github,
}

pub(crate) fn resolve_format(cli: &crate::Cli, config: &RevetConfig) -> Format {
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

fn discover_review_files(
    repo_path: &Path,
    cli: &crate::Cli,
    config: &RevetConfig,
    all_extensions: &[&str],
    extra_filenames: &[&str],
) -> Result<Vec<PathBuf>> {
    if cli.full {
        return full_scan(repo_path, all_extensions, extra_filenames, config);
    }

    // Try diff-based discovery
    let base = cli.diff.as_deref().unwrap_or(&config.general.diff_base);

    match DiffAnalyzer::new(repo_path) {
        Ok(analyzer) => {
            eprint!("  Discovering changed files (diff vs {})... ", base);
            match analyzer.get_diff(base, None) {
                Ok(diff) => {
                    let changed = analyzer.get_changed_files(&diff)?;
                    let files: Vec<PathBuf> = changed
                        .into_iter()
                        .filter_map(|cf| {
                            let abs = repo_path.join(&cf.path);
                            if abs.exists()
                                && (has_extension(&cf.path, all_extensions)
                                    || has_filename(&cf.path, extra_filenames))
                            {
                                Some(abs)
                            } else {
                                None
                            }
                        })
                        .collect();
                    eprintln!("{} ({} files)", "done".green(), files.len());

                    if files.is_empty() {
                        eprintln!(
                            "  {} — falling back to full scan",
                            "No supported changed files".dimmed()
                        );
                        return full_scan(repo_path, all_extensions, extra_filenames, config);
                    }

                    Ok(files)
                }
                Err(_) => {
                    eprintln!(
                        "{}",
                        format!(
                            "  Could not diff against '{}', falling back to full scan",
                            base
                        )
                        .dimmed()
                    );
                    full_scan(repo_path, all_extensions, extra_filenames, config)
                }
            }
        }
        Err(_) => {
            eprintln!("  {} — running full scan", "Not a git repository".dimmed());
            full_scan(repo_path, all_extensions, extra_filenames, config)
        }
    }
}

fn full_scan(
    repo_path: &Path,
    extensions: &[&str],
    filenames: &[&str],
    config: &RevetConfig,
) -> Result<Vec<PathBuf>> {
    eprint!("  Discovering files (full scan)... ");
    let _ = std::io::stderr().flush();
    let files = if filenames.is_empty() {
        discover_files(repo_path, extensions, &config.ignore.paths)?
    } else {
        discover_files_extended(repo_path, extensions, filenames, &config.ignore.paths)?
    };
    eprintln!("{} ({} files)", "done".green(), files.len());
    Ok(files)
}

pub(crate) fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    let with_dot = format!(".{}", ext);
    extensions.contains(&with_dot.as_str())
}

pub(crate) fn has_filename(path: &Path, filenames: &[&str]) -> bool {
    if filenames.is_empty() {
        return false;
    }
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => filenames.contains(&name),
        None => false,
    }
}

pub(crate) fn build_summary(
    findings: &[Finding],
    files_analyzed: usize,
    nodes_parsed: usize,
) -> ReviewSummary {
    let mut summary = ReviewSummary {
        files_analyzed,
        nodes_parsed,
        ..Default::default()
    };
    for f in findings {
        match f.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warning => summary.warnings += 1,
            Severity::Info => summary.info += 1,
        }
    }
    summary
}

pub(crate) fn print_terminal(
    findings: &[Finding],
    summary: &ReviewSummary,
    repo_path: &Path,
    start: Instant,
    suppressed_count: usize,
    inline_suppressed: usize,
) {
    println!();

    // Print findings
    for f in findings {
        let display_path = f.file.strip_prefix(repo_path).unwrap_or(&f.file).display();

        println!(
            "{}",
            output::terminal::format_finding(
                &f.severity.to_string(),
                &f.message,
                &display_path.to_string(),
                f.line,
                f.suggestion.as_deref(),
                f.ai_note.as_deref(),
                f.ai_false_positive,
            )
        );
    }

    if !findings.is_empty() {
        println!();
    }

    println!("  {}", "\u{2500}".repeat(60).dimmed());
    println!(
        "  {} \u{00b7} {} \u{00b7} {}",
        format!("{} error(s)", summary.errors).green(),
        format!("{} warning(s)", summary.warnings).yellow(),
        format!("{} info", summary.info).blue()
    );
    if suppressed_count > 0 || inline_suppressed > 0 {
        let mut parts = Vec::new();
        if suppressed_count > 0 {
            parts.push(format!("{} baselined", suppressed_count));
        }
        if inline_suppressed > 0 {
            parts.push(format!("{} inline", inline_suppressed));
        }
        println!(
            "  {}",
            format!("{} finding(s) suppressed", parts.join(" + ")).dimmed()
        );
    }
    println!(
        "  {} files analyzed \u{00b7} {} nodes parsed",
        summary.files_analyzed, summary.nodes_parsed
    );
    println!("  Time: {:.1}s", start.elapsed().as_secs_f64());
}

pub(crate) fn print_json(findings: &[Finding], summary: &ReviewSummary) {
    let json_findings: Vec<output::json::JsonFinding> = findings
        .iter()
        .map(|f| output::json::JsonFinding {
            id: f.id.clone(),
            severity: f.severity.to_string(),
            message: f.message.clone(),
            file: f.file.display().to_string(),
            line: f.line,
        })
        .collect();

    let out = output::json::JsonOutput {
        findings: json_findings,
        summary: output::json::JsonSummary {
            errors: summary.errors,
            warnings: summary.warnings,
            info: summary.info,
        },
    };

    match serde_json::to_string_pretty(&out) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Failed to serialize JSON: {}", e),
    }
}

pub(crate) fn print_sarif(findings: &[Finding], repo_path: &Path) {
    let log = output::sarif::build_sarif_log(findings, repo_path);
    match serde_json::to_string_pretty(&log) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Failed to serialize SARIF: {}", e),
    }
}

pub(crate) fn print_no_files(format: Format, start: Instant) {
    match format {
        Format::Json => {
            let out = output::json::JsonOutput {
                findings: vec![],
                summary: output::json::JsonSummary {
                    errors: 0,
                    warnings: 0,
                    info: 0,
                },
            };
            if let Ok(json) = serde_json::to_string_pretty(&out) {
                println!("{}", json);
            }
        }
        Format::Sarif => {
            let log = output::sarif::build_sarif_log(&[], Path::new("."));
            if let Ok(json) = serde_json::to_string_pretty(&log) {
                println!("{}", json);
            }
        }
        Format::Github => {
            // No files, no annotations — nothing to output
        }
        Format::Terminal => {
            println!("  {}", "No supported files found.".dimmed());
            println!("  Time: {:.1}s", start.elapsed().as_secs_f64());
        }
    }
}

pub(crate) fn print_github(findings: &[Finding], repo_path: &Path) {
    for f in findings {
        println!("{}", output::github::format_finding(f, repo_path));
    }
}

/// Post findings as inline GitHub PR review comments.
///
/// Filters to diff-only findings, deduplicates against existing comments,
/// and logs a summary. Exits gracefully if GitHub context is not available.
fn post_github_comments(findings: &[Finding], repo_path: &Path, cli: &crate::Cli) {
    let ctx = match github_comment::GitHubContext::from_env() {
        Some(c) => c,
        None => {
            eprintln!(
                "  {}: --post-comment requires GITHUB_TOKEN, GITHUB_REPOSITORY, \
                 GITHUB_PR_NUMBER and GITHUB_SHA environment variables",
                "warn".yellow()
            );
            return;
        }
    };

    // Filter to only findings on changed lines
    let diff_findings = match DiffAnalyzer::new(repo_path) {
        Ok(analyzer) => {
            let base = cli.diff.as_deref().unwrap_or("main");
            match analyzer.get_all_changed_lines(base) {
                Ok(diff_map) => {
                    let (kept, _) =
                        filter_findings_by_diff(findings.to_vec(), &diff_map, repo_path);
                    kept
                }
                Err(_) => findings.to_vec(), // fallback: post all findings
            }
        }
        Err(_) => findings.to_vec(), // not a git repo or no diff — post all
    };

    eprint!(
        "  Posting {} finding(s) to GitHub PR #{}... ",
        diff_findings.len(),
        ctx.pr_number
    );
    let _ = std::io::stderr().flush();

    match github_comment::post_review_comments(&diff_findings, repo_path, &ctx) {
        Ok((posted, off_diff, dupes)) => {
            eprintln!("{}", "done".green());
            if posted > 0 {
                eprintln!("  {} new comment(s) posted", posted);
            }
            if dupes > 0 {
                eprintln!("  {} duplicate(s) skipped", dupes);
            }
            if off_diff > 0 {
                eprintln!("  {} finding(s) skipped (no line info)", off_diff);
            }
        }
        Err(e) => {
            eprintln!("{}: {}", "failed".red(), e);
        }
    }
}
