//! Main review command — wires parser, graph, impact analysis, and cache together

use anyhow::Result;
use colored::Colorize;
use revet_core::{
    apply_fixes, create_store, discover_files, discover_files_extended, filter_findings,
    filter_findings_by_diff, filter_findings_by_inline, filter_findings_by_path_rules,
    reconstruct_graph, AnalyzerDispatcher, AnalyzerTiming, Baseline, BlastRadiusSummary, CodeGraph,
    DiffAnalyzer, FileGraphCache, Finding, GateConfig, GitTreeReader, GraphCache, GraphCacheMeta,
    GraphStore, ImpactAnalysis, ParserDispatcher, RevetConfig, ReviewSummary, Severity,
    SuppressedFinding,
};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::ai::AiReasoner;
use crate::output::github_comment;
use crate::output::{make_formatter, resolve_format};
use crate::progress::Step;
use crate::run_log;

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
        format!("  revet v{} — analyzing repository", revet_core::VERSION)
            .bold()
            .yellow()
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
        let mut out = make_formatter(format, &repo_path, false);
        out.write_no_files(start.elapsed());
        out.finalize();
        return Ok(ReviewExitCode::Success);
    }

    // ── 3. Parse (incremental, cache-aware) ──────────────────────
    let step = Step::new("Building code graph");
    let graph_start = Instant::now();

    let file_cache = FileGraphCache::new(&repo_path);
    let (graph, parse_errors, cached_count, parsed_count) =
        dispatcher.parse_files_incremental(&files, repo_path.clone(), &file_cache);

    let node_count: usize = graph.nodes().count();
    step.finish(&format!(
        "{} files ({} cached, {} parsed), {} nodes ({:.1}s)",
        files.len(),
        cached_count,
        parsed_count,
        node_count,
        graph_start.elapsed().as_secs_f64()
    ));

    // ── 4. Impact Analysis ───────────────────────────────────────
    let mut findings: Vec<Finding> = Vec::new();
    let mut blast_radius: Option<BlastRadiusSummary> = None;

    let old_graph = load_old_graph(&repo_path, cli, &config, &dispatcher);

    if let Some(baseline) = old_graph {
        let step = Step::new("Running impact analysis");
        let impact_start = Instant::now();

        let analysis = ImpactAnalysis::new(baseline, graph.clone())
            .with_depth(config.modules.call_graph_depth);
        let report = analysis.analyze_impact();

        // Compute blast radius summary for at-a-glance output
        blast_radius = Some(BlastRadiusSummary::from_impact_report(
            &report,
            analysis.new_graph(),
            &repo_path,
        ));

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

            let id_prefix = match change.classification {
                revet_core::ChangeClassification::Breaking => "BREAKING",
                revet_core::ChangeClassification::PotentiallyBreaking => "IMPACT",
                revet_core::ChangeClassification::Safe => unreachable!(),
            };

            // Collect caller locations using the call-site line from EdgeMetadata.
            // Direct callers first (with precise call-site line), then transitives.
            let mut callers: Vec<String> = Vec::new();

            // Direct callers — use EdgeMetadata::Call { line } for call-site precision
            for (caller_id, edge) in analysis
                .new_graph()
                .edges_to(change.node_id)
                .into_iter()
                .filter(|(_, e)| e.kind() == &revet_core::EdgeKind::Calls)
            {
                if let Some(caller_node) = analysis.new_graph().node(caller_id) {
                    let rel = caller_node
                        .file_path()
                        .strip_prefix(&repo_path)
                        .unwrap_or(caller_node.file_path());
                    let call_line = match edge.metadata() {
                        Some(revet_core::EdgeMetadata::Call { line, .. }) => *line,
                        _ => caller_node.line(),
                    };
                    callers.push(if call_line > 0 {
                        format!("{}:{}", rel.display(), call_line)
                    } else {
                        rel.display().to_string()
                    });
                }
            }

            // Transitive callers (beyond the direct set) — annotated as transitive
            let direct_set: std::collections::HashSet<_> =
                change.direct_dependents.iter().copied().collect();
            for &t_id in &change.transitive_dependents {
                if direct_set.contains(&t_id) {
                    continue; // already listed above
                }
                if let Some(t_node) = analysis.new_graph().node(t_id) {
                    let rel = t_node
                        .file_path()
                        .strip_prefix(&repo_path)
                        .unwrap_or(t_node.file_path());
                    callers.push(format!(
                        "{} (transitive)",
                        if t_node.line() > 0 {
                            format!("{}:{}", rel.display(), t_node.line())
                        } else {
                            rel.display().to_string()
                        }
                    ));
                }
            }

            findings.push(Finding {
                id: format!("{}-{:03}", id_prefix, findings.len() + 1),
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
                callers,
                suggestion: None,
                fix_kind: None,
                ..Default::default()
            });
        }

        step.finish(&format!(
            "{} impact finding(s) ({:.1}s)",
            findings.len(),
            impact_start.elapsed().as_secs_f64()
        ));

        // ── 4b. Diff-scoped dead code (rvt-59) ───────────────────
        if config.modules.dead_code {
            // Symbols added in this diff that have zero callers in the full repo
            let new_node_names: std::collections::HashSet<(&str, &std::path::Path)> = analysis
                .new_graph()
                .nodes()
                .map(|(_, n)| (n.name(), n.file_path().as_path()))
                .collect();

            for (node_id, node) in analysis.new_graph().nodes() {
                if !matches!(
                    node.kind(),
                    revet_core::NodeKind::Function
                        | revet_core::NodeKind::Class
                        | revet_core::NodeKind::Variable
                ) {
                    continue;
                }
                if diff_dead_is_entry_point(node.name()) {
                    continue;
                }
                if diff_is_test_file(node.file_path()) {
                    continue;
                }
                // Only flag symbols that are NEW (not in old graph)
                let in_old = analysis.old_graph().nodes().any(|(_, on)| {
                    on.name() == node.name()
                        && on.file_path() == node.file_path()
                        && on.kind() == node.kind()
                });
                if in_old {
                    continue;
                }
                // Check for callers in the new graph
                let has_callers = analysis.new_graph().edges_to(node_id).iter().any(|(_, e)| {
                    matches!(
                        e.kind(),
                        revet_core::EdgeKind::Calls | revet_core::EdgeKind::References
                    )
                });
                if has_callers {
                    continue;
                }
                let severity = if node.is_public() {
                    Severity::Warning
                } else {
                    Severity::Info
                };
                findings.push(Finding {
                    id: String::new(),
                    severity,
                    message: format!(
                        "`{}` added in this diff but has no callers in the codebase",
                        node.name()
                    ),
                    file: node.file_path().clone(),
                    line: node.line(),
                    affected_dependents: 0,
                    suggestion: Some("Add a call site or remove the symbol".to_string()),
                    fix_kind: None,
                    ..Default::default()
                });
            }
            let _ = new_node_names; // suppress unused warning

            // ── 4c. Deletion orphan check (rvt-60) ────────────────
            for (_, old_node) in analysis.old_graph().nodes() {
                if !matches!(
                    old_node.kind(),
                    revet_core::NodeKind::Function
                        | revet_core::NodeKind::Class
                        | revet_core::NodeKind::Variable
                ) {
                    continue;
                }
                if diff_is_test_file(old_node.file_path()) {
                    continue;
                }
                // Only examine deleted symbols (absent in new graph)
                let still_exists = analysis.new_graph().nodes().any(|(_, nn)| {
                    nn.name() == old_node.name()
                        && nn.file_path() == old_node.file_path()
                        && nn.kind() == old_node.kind()
                });
                if still_exists {
                    continue;
                }

                // Find what this deleted node was calling/referencing in the old graph
                // If the deletion removes the only reference to another symbol S,
                // emit a dead-code finding on S in the new graph.
                for (old_caller_id, _) in analysis.old_graph().nodes() {
                    if analysis.old_graph().node(old_caller_id).map(|n| n.name())
                        != Some(old_node.name())
                    {
                        continue;
                    }
                    for (target_id, edge) in analysis.old_graph().edges_from(old_caller_id) {
                        if !matches!(
                            edge.kind(),
                            revet_core::EdgeKind::Calls | revet_core::EdgeKind::References
                        ) {
                            continue;
                        }
                        let target_node = match analysis.old_graph().node(target_id) {
                            Some(n) => n,
                            None => continue,
                        };
                        if diff_is_test_file(target_node.file_path()) {
                            continue;
                        }
                        // Find the same target in the new graph
                        let new_target = analysis.new_graph().nodes().find(|(_, nn)| {
                            nn.name() == target_node.name()
                                && nn.file_path() == target_node.file_path()
                                && nn.kind() == target_node.kind()
                        });
                        let (new_target_id, new_target_node) = match new_target {
                            Some(t) => t,
                            None => continue, // also deleted
                        };
                        // Check if now has zero callers in new graph
                        let still_has_callers = analysis
                            .new_graph()
                            .edges_to(new_target_id)
                            .iter()
                            .any(|(_, e)| {
                                matches!(
                                    e.kind(),
                                    revet_core::EdgeKind::Calls | revet_core::EdgeKind::References
                                )
                            });
                        if still_has_callers {
                            continue;
                        }
                        if diff_dead_is_entry_point(new_target_node.name()) {
                            continue;
                        }
                        let severity = if new_target_node.is_public() {
                            Severity::Warning
                        } else {
                            Severity::Info
                        };
                        let rel = new_target_node
                            .file_path()
                            .strip_prefix(&repo_path)
                            .unwrap_or(new_target_node.file_path());
                        findings.push(Finding {
                            id: String::new(),
                            severity,
                            message: format!(
                                "`{}` is now unreachable — its only caller `{}` was removed in this diff",
                                new_target_node.name(),
                                old_node.name()
                            ),
                            file: new_target_node.file_path().clone(),
                            line: new_target_node.line(),
                            affected_dependents: 0,
                            suggestion: Some(format!(
                                "Remove `{}` or add a new call site at {}",
                                new_target_node.name(),
                                rel.display()
                            )),
                            fix_kind: None,
                            ..Default::default()
                        });
                    }
                    break; // matched the deleted node — no need to continue
                }
            }
        }
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
    let step = Step::new("Running domain analyzers");
    let analyzer_start = Instant::now();
    let (analyzer_findings, domain_timings) =
        analyzer_dispatcher.run_all_parallel_timed(&files, &repo_path, &config);
    let analyzer_count = analyzer_findings.len();
    findings.extend(analyzer_findings);
    step.finish(&format!(
        "{} finding(s) ({:.1}s)",
        analyzer_count,
        analyzer_start.elapsed().as_secs_f64()
    ));

    // ── 4b'. Graph analyzers ─────────────────────────────────────────
    let step = Step::new("Running graph analyzers");
    let ga_start = Instant::now();
    let (graph_findings, graph_timings) =
        analyzer_dispatcher.run_graph_analyzers_timed(&graph, &config);
    let graph_count = graph_findings.len();
    findings.extend(graph_findings);
    step.finish(&format!(
        "{} finding(s) ({:.1}s)",
        graph_count,
        ga_start.elapsed().as_secs_f64()
    ));

    // ── 4c. AI reasoning ─────────────────────────────────────────
    if cli.ai {
        let eligible = findings
            .iter()
            .filter(|f| {
                matches!(f.severity, Severity::Warning | Severity::Error) && f.suggestion.is_none()
            })
            .count();
        let step = Step::new(format!("Running AI reasoning ({} findings)", eligible));
        let ai_start = Instant::now();
        let reasoner = AiReasoner::new(config.ai.clone(), cli.max_cost);
        match reasoner.enrich(&mut findings, &repo_path) {
            Ok(stats) => step.finish(&format!(
                "{} enriched, {} false positives (${:.4}, {:.1}s)",
                stats.findings_enriched,
                stats.false_positives,
                stats.cost_usd,
                ai_start.elapsed().as_secs_f64()
            )),
            Err(e) => step.warn(e),
        }
    }

    // ── 4d. Apply fixes ───────────────────────────────────────────
    if cli.fix {
        let step = Step::new("Applying fixes");
        match apply_fixes(&findings) {
            Ok(report) => step.finish(&format!(
                "{} applied, {} suggestion-only",
                report.applied, report.skipped
            )),
            Err(e) => step.warn(format!("failed: {}", e)),
        }
    }

    // ── 4d. Inline suppression ───────────────────────────────────
    let mut all_suppressed: Vec<SuppressedFinding> = Vec::new();
    let (new_findings, inline_suppressed) = filter_findings_by_inline(findings);
    findings = new_findings;
    all_suppressed.extend(inline_suppressed);

    // ── 4e. Per-path rule suppression ────────────────────────────
    if !config.ignore.per_path.is_empty() {
        let (new_findings, path_suppressed) =
            filter_findings_by_path_rules(findings, &config.ignore.per_path, &repo_path);
        findings = new_findings;
        all_suppressed.extend(path_suppressed);
    }

    // ── 4f. Baseline suppression ───────────────────────────────────
    if !cli.no_baseline {
        if let Some(baseline) = Baseline::load(&repo_path)? {
            let (new_findings, baseline_suppressed) =
                filter_findings(findings, &baseline, &repo_path);
            findings = new_findings;
            all_suppressed.extend(baseline_suppressed);
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
    let summary = build_summary(&findings, &files, node_count);

    // Write run log (best-effort — don't fail the review on log errors)
    let run_id = run_log::new_run_id();
    let run_log_saved = run_log::save_run_log(
        &repo_path,
        &run_id,
        start.elapsed().as_secs_f64(),
        &findings,
        &all_suppressed,
        &summary,
        &repo_path,
    )
    .is_ok();

    let mut out = make_formatter(format, &repo_path, cli.show_suppressed);
    if let Some(ref br) = blast_radius {
        out.write_blast_radius(br);
    }
    for f in &findings {
        out.write_finding(f, &repo_path);
    }
    if cli.show_suppressed {
        for sf in &all_suppressed {
            out.write_suppressed(sf, &repo_path);
        }
    }
    out.write_summary(
        &summary,
        &all_suppressed,
        start.elapsed(),
        if run_log_saved { Some(&run_id) } else { None },
    );
    out.finalize();

    // ── 7. Timings (optional) ────────────────────────────────────
    if cli.timings {
        print_timings(&domain_timings, &graph_timings);
    }

    // Quality gate (--gate) takes precedence over --fail-on
    let gate = cli
        .gate
        .as_deref()
        .map(GateConfig::from_flag)
        .unwrap_or_else(|| config.gate.clone());

    let exceeded = if !gate.is_empty() {
        summary.exceeds_gate(&gate)
    } else {
        let fail_on = cli.fail_on.as_deref().unwrap_or(&config.general.fail_on);
        summary.exceeds_threshold(fail_on)
    };

    if exceeded {
        Ok(ReviewExitCode::FindingsExceedThreshold)
    } else {
        Ok(ReviewExitCode::Success)
    }
}

// ── Helpers ──────────────────────────────────────────────────────

/// Load the old (baseline) graph for impact analysis.
///
/// Tries: msgpack cache → CozoStore → git blobs → None.
/// A single spinner covers all attempts; its message is updated between tries.
fn load_old_graph(
    repo_path: &Path,
    cli: &crate::Cli,
    config: &RevetConfig,
    dispatcher: &ParserDispatcher,
) -> Option<CodeGraph> {
    let step = Step::new("Loading baseline graph");
    let baseline_start = Instant::now();

    // 1. Try msgpack cache (fast path — serialized whole graph)
    let cache = GraphCache::new(repo_path);
    match cache.load() {
        Ok(Some((cached_graph, _))) => {
            step.finish(&format!(
                "{} nodes from cache ({:.1}s)",
                cached_graph.nodes().count(),
                baseline_start.elapsed().as_secs_f64()
            ));
            return Some(cached_graph);
        }
        Ok(None) => {} // not found — try next source
        Err(e) => step.warn(e),
    }

    // 2. Try CozoStore (slower fallback)
    if let Ok(store) = create_store(repo_path) {
        let snaps = store.snapshots().unwrap_or_default();
        if snaps.iter().any(|s| s.name == "cached") {
            step.update("Loading baseline graph from store...");
            match reconstruct_graph(&store, "cached", repo_path) {
                Ok(graph) => {
                    step.finish(&format!(
                        "{} nodes from store ({:.1}s)",
                        graph.nodes().count(),
                        baseline_start.elapsed().as_secs_f64()
                    ));
                    return Some(graph);
                }
                Err(e) => step.warn(e),
            }
        }
    }

    // 3. Try building from git blobs at the base ref
    let base = cli.diff.as_deref().unwrap_or(&config.general.diff_base);
    match GitTreeReader::new(repo_path) {
        Ok(reader) => {
            step.update(format!("Building baseline graph from git ({})...", base));
            match reader.build_graph_at_ref(base, repo_path, dispatcher) {
                Ok(blob_graph) => {
                    let node_count: usize = blob_graph.nodes().count();
                    step.finish(&format!(
                        "{} nodes from git ({:.1}s)",
                        node_count,
                        baseline_start.elapsed().as_secs_f64()
                    ));
                    Some(blob_graph)
                }
                Err(e) => {
                    step.skip(&format!(
                        "No baseline available ({}), skipping impact analysis",
                        e
                    ));
                    None
                }
            }
        }
        Err(_) => {
            step.skip("No baseline graph available — run again to compare changes");
            None
        }
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
            let step = Step::new(format!("Discovering changed files (diff vs {})", base));
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

                    if files.is_empty() {
                        step.skip("No supported changed files — falling back to full scan");
                        return full_scan(repo_path, all_extensions, extra_filenames, config);
                    }

                    step.finish(&format!("{} files", files.len()));
                    Ok(files)
                }
                Err(_) => {
                    step.skip(&format!(
                        "Could not diff against '{}' — falling back to full scan",
                        base
                    ));
                    full_scan(repo_path, all_extensions, extra_filenames, config)
                }
            }
        }
        Err(_) => {
            eprintln!("  {}", "Not a git repository — running full scan".dimmed());
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
    let step = Step::new("Discovering files (full scan)");
    let files = if filenames.is_empty() {
        discover_files(repo_path, extensions, &config.ignore.paths)?
    } else {
        discover_files_extended(repo_path, extensions, filenames, &config.ignore.paths)?
    };
    step.finish(&format!("{} files", files.len()));
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
    files: &[PathBuf],
    nodes_parsed: usize,
) -> ReviewSummary {
    let mut summary = ReviewSummary {
        files_analyzed: files.len(),
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
    for path in files {
        let lang = ext_to_language(path);
        *summary.files_by_language.entry(lang).or_default() += 1;
    }
    summary
}

fn ext_to_language(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "Rust",
        Some("py") => "Python",
        Some("ts") | Some("tsx") => "TypeScript",
        Some("js") | Some("jsx") => "JavaScript",
        Some("go") => "Go",
        Some("java") => "Java",
        Some("kt") => "Kotlin",
        Some("rb") => "Ruby",
        Some("cs") => "C#",
        Some("cpp") | Some("cc") | Some("cxx") => "C++",
        Some("c") | Some("h") => "C",
        Some("swift") => "Swift",
        Some("toml") => "TOML",
        Some("yaml") | Some("yml") => "YAML",
        Some("json") => "JSON",
        Some("tf") => "Terraform",
        Some("sh") | Some("bash") => "Shell",
        Some(e) => e,
        None => match path.file_name().and_then(|n| n.to_str()) {
            Some("Dockerfile") => "Dockerfile",
            Some("Makefile") => "Makefile",
            _ => "other",
        },
    }
    .to_string()
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

    let step = Step::new(format!(
        "Posting {} finding(s) to GitHub PR #{}",
        diff_findings.len(),
        ctx.pr_number
    ));

    match github_comment::post_review_comments(&diff_findings, repo_path, &ctx) {
        Ok((posted, off_diff, dupes)) => {
            step.finish(&format!(
                "{} posted, {} duplicate(s) skipped, {} off-diff skipped",
                posted, dupes, off_diff
            ));
        }
        Err(e) => {
            step.warn(format!("failed: {}", e));
        }
    }
}

/// Print a per-analyzer timing breakdown table to stderr.
fn print_timings(domain: &[AnalyzerTiming], graph: &[AnalyzerTiming]) {
    let all: Vec<&AnalyzerTiming> = domain.iter().chain(graph.iter()).collect();
    if all.is_empty() {
        return;
    }

    eprintln!();
    eprintln!("  {}", "Analyzer timings".bold());
    eprintln!(
        "  {:<30} {:>8}  {:>8}  {}",
        "Analyzer".dimmed(),
        "Time".dimmed(),
        "Findings".dimmed(),
        "Bar".dimmed()
    );
    eprintln!("  {}", "─".repeat(60).dimmed());

    let max_ms = all
        .iter()
        .map(|t| t.duration.as_millis())
        .max()
        .unwrap_or(1)
        .max(1);

    for t in &all {
        let ms = t.duration.as_millis();
        let bar_len = ((ms as f64 / max_ms as f64) * 20.0).round() as usize;
        let bar = format!("{}{}", "█".repeat(bar_len), "░".repeat(20 - bar_len));
        let time_str = if ms < 1 {
            format!("{:.2}ms", t.duration.as_secs_f64() * 1000.0)
        } else {
            format!("{:.0}ms", ms)
        };
        eprintln!(
            "  {:<30} {:>8}  {:>8}  {}",
            t.name,
            time_str.yellow(),
            t.findings,
            bar.dimmed()
        );
    }

    let total_ms: u128 = all.iter().map(|t| t.duration.as_millis()).sum();
    eprintln!("  {}", "─".repeat(60).dimmed());
    eprintln!(
        "  {:<30} {:>8}",
        "Total".bold(),
        format!("{:.0}ms", total_ms).yellow().bold()
    );
    eprintln!();
}

/// Entry-point names never flagged as dead code (diff-scoped checks).
fn diff_dead_is_entry_point(name: &str) -> bool {
    matches!(
        name,
        "main" | "__init__" | "__main__" | "new" | "index" | "handler" | "default"
    )
}

/// Returns true if the path looks like a test file.
fn diff_is_test_file(path: &std::path::Path) -> bool {
    path.components()
        .any(|c| c.as_os_str() == "tests" || c.as_os_str() == "__tests__")
        || path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|name| {
                name.ends_with("_test.rs")
                    || name.starts_with("test_")
                    || name.ends_with("_test.py")
                    || name.ends_with(".test.ts")
                    || name.ends_with(".spec.ts")
                    || name.ends_with(".test.js")
                    || name.ends_with(".spec.js")
            })
            .unwrap_or(false)
}
