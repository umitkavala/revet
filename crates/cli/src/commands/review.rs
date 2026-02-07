//! Main review command — wires parser, graph, impact analysis, and cache together

use anyhow::Result;
use colored::Colorize;
use revet_core::{
    discover_files, CodeGraph, DiffAnalyzer, Finding, GraphCache, GraphCacheMeta, ImpactAnalysis,
    ParserDispatcher, RevetConfig, ReviewSummary, Severity,
};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::output;

pub fn run(path: Option<&Path>, cli: &crate::Cli) -> Result<()> {
    let start = Instant::now();
    let repo_path = path.unwrap_or_else(|| Path::new("."));
    let repo_path = std::fs::canonicalize(repo_path).unwrap_or_else(|_| repo_path.to_path_buf());

    println!(
        "{}",
        format!("  revet v{} — analyzing repository", revet_core::VERSION).bold()
    );
    println!();

    // ── 1. Config ────────────────────────────────────────────────
    let config = RevetConfig::find_and_load(&repo_path)?;
    let format = resolve_format(cli, &config);

    // ── 2. File Discovery ────────────────────────────────────────
    let dispatcher = ParserDispatcher::new();
    let extensions = dispatcher.supported_extensions();

    let files = discover_review_files(&repo_path, cli, &config, &extensions)?;

    if files.is_empty() {
        print_no_files(format, start);
        return Ok(());
    }

    // ── 3. Parse ─────────────────────────────────────────────────
    print!("  Building code graph... ");
    let graph_start = Instant::now();

    let mut graph = CodeGraph::new(repo_path.clone());
    let mut parse_errors: Vec<String> = Vec::new();

    for file in &files {
        match dispatcher.parse_file(file, &mut graph) {
            Ok(_) => {}
            Err(e) => parse_errors.push(format!("{}: {}", file.display(), e)),
        }
    }

    let node_count: usize = graph.nodes().count();
    println!(
        "{} — {} files, {} nodes ({:.1}s)",
        "done".green(),
        files.len(),
        node_count,
        graph_start.elapsed().as_secs_f64()
    );

    // ── 4. Impact Analysis ───────────────────────────────────────
    let mut findings: Vec<Finding> = Vec::new();

    let cache = GraphCache::new(&repo_path);
    let old_graph = cache.load().ok().flatten();

    if let Some((cached_graph, _meta)) = old_graph {
        print!("  Running impact analysis... ");
        let impact_start = Instant::now();

        let analysis = ImpactAnalysis::new(cached_graph, graph.clone());
        let report = analysis.analyze_impact();

        for change in &report.changes {
            let severity = match change.classification {
                revet_core::ChangeClassification::Breaking => Severity::Error,
                revet_core::ChangeClassification::PotentiallyBreaking => Severity::Warning,
                revet_core::ChangeClassification::Safe => {
                    if !cli.full {
                        continue; // skip safe changes unless --full
                    }
                    Severity::Info
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
            });
        }

        println!(
            "{} ({:.1}s)",
            "done".green(),
            impact_start.elapsed().as_secs_f64()
        );
    } else {
        println!(
            "  {} — run again to compare changes",
            "No cached graph found, skipping impact analysis".dimmed()
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
        });
    }

    // ── 5. Save Cache ────────────────────────────────────────────
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

    if let Err(e) = cache.save(&graph, &meta) {
        eprintln!("  {}: failed to save cache: {}", "warn".yellow(), e);
    }

    // ── 6. Output ────────────────────────────────────────────────
    let summary = build_summary(&findings, files.len(), node_count);

    match format {
        Format::Json => print_json(&findings, &summary),
        _ => print_terminal(&findings, &summary, &repo_path, start),
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
enum Format {
    Terminal,
    Json,
}

fn resolve_format(cli: &crate::Cli, config: &RevetConfig) -> Format {
    if let Some(ref f) = cli.format {
        return match f {
            crate::OutputFormat::Json => Format::Json,
            _ => Format::Terminal,
        };
    }
    match config.output.format.as_str() {
        "json" => Format::Json,
        _ => Format::Terminal,
    }
}

fn discover_review_files(
    repo_path: &Path,
    cli: &crate::Cli,
    config: &RevetConfig,
    extensions: &[&str],
) -> Result<Vec<PathBuf>> {
    if cli.full {
        print!("  Discovering files (full scan)... ");
        let files = discover_files(repo_path, extensions, &config.ignore.paths)?;
        println!("{} ({} files)", "done".green(), files.len());
        return Ok(files);
    }

    // Try diff-based discovery
    let base = cli.diff.as_deref().unwrap_or(&config.general.diff_base);

    match DiffAnalyzer::new(repo_path) {
        Ok(analyzer) => {
            print!("  Discovering changed files (diff vs {})... ", base);
            match analyzer.get_diff(base, None) {
                Ok(diff) => {
                    let changed = analyzer.get_changed_files(&diff)?;
                    let files: Vec<PathBuf> = changed
                        .into_iter()
                        .filter_map(|cf| {
                            let abs = repo_path.join(&cf.path);
                            if abs.exists() && has_extension(&cf.path, extensions) {
                                Some(abs)
                            } else {
                                None
                            }
                        })
                        .collect();
                    println!("{} ({} files)", "done".green(), files.len());

                    if files.is_empty() {
                        println!(
                            "  {} — falling back to full scan",
                            "No supported changed files".dimmed()
                        );
                        return full_scan(repo_path, extensions, config);
                    }

                    Ok(files)
                }
                Err(_) => {
                    println!(
                        "{}",
                        format!(
                            "  Could not diff against '{}', falling back to full scan",
                            base
                        )
                        .dimmed()
                    );
                    full_scan(repo_path, extensions, config)
                }
            }
        }
        Err(_) => {
            println!("  {} — running full scan", "Not a git repository".dimmed());
            full_scan(repo_path, extensions, config)
        }
    }
}

fn full_scan(repo_path: &Path, extensions: &[&str], config: &RevetConfig) -> Result<Vec<PathBuf>> {
    print!("  Discovering files (full scan)... ");
    let files = discover_files(repo_path, extensions, &config.ignore.paths)?;
    println!("{} ({} files)", "done".green(), files.len());
    Ok(files)
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    let with_dot = format!(".{}", ext);
    extensions.contains(&with_dot.as_str())
}

fn build_summary(
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

fn print_terminal(findings: &[Finding], summary: &ReviewSummary, repo_path: &Path, start: Instant) {
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
    println!(
        "  {} files analyzed \u{00b7} {} nodes parsed",
        summary.files_analyzed, summary.nodes_parsed
    );
    println!("  Time: {:.1}s", start.elapsed().as_secs_f64());
}

fn print_json(findings: &[Finding], summary: &ReviewSummary) {
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

fn print_no_files(format: Format, start: Instant) {
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
        _ => {
            println!("  {}", "No supported files found.".dimmed());
            println!("  Time: {:.1}s", start.elapsed().as_secs_f64());
        }
    }
}
