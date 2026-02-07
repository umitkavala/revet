//! Main review command

use anyhow::Result;
use colored::Colorize;
use revet_core::{CodeGraph, ParserDispatcher, RevetConfig};
use std::path::Path;
use std::time::Instant;

pub fn run(path: Option<&Path>, _cli: &crate::Cli) -> Result<()> {
    let start = Instant::now();
    let repo_path = path.unwrap_or_else(|| Path::new("."));

    println!(
        "{}",
        format!("  revet v{} — analyzing repository", revet_core::VERSION).bold()
    );
    println!();

    // Load configuration
    let _config = RevetConfig::find_and_load(repo_path)?;

    // Initialize code graph
    let _graph = CodeGraph::new(repo_path.to_path_buf());

    // Build code graph
    print!("  Building code graph... ");
    let graph_start = Instant::now();

    let dispatcher = ParserDispatcher::new();

    // For now, just count supported files
    // TODO: Actually parse files
    let _supported_extensions = dispatcher.supported_extensions();
    println!(
        "{} ({:.1}s)",
        "done".green(),
        graph_start.elapsed().as_secs_f64()
    );

    // Run analyzers
    print!("  Running analyzers... ");
    let analyzer_start = Instant::now();
    // TODO: Run domain analyzers
    println!(
        "{} ({:.1}s)",
        "done".green(),
        analyzer_start.elapsed().as_secs_f64()
    );

    println!();
    println!("  {}", "─".repeat(60).dimmed());
    println!(
        "  {} · {} · {}",
        "0 errors".green(),
        "0 warnings".yellow(),
        "0 info".blue()
    );
    println!(
        "  {} deterministic · {} LLM",
        "0 checks".dimmed(),
        "0".dimmed()
    );
    println!("  Time: {:.1}s", start.elapsed().as_secs_f64());

    Ok(())
}
