//! Quick benchmark: parallel vs sequential parsing and analysis
//!
//! Run with: cargo test --test bench_parallel -- --nocapture --ignored

use revet_core::{AnalyzerDispatcher, CodeGraph, ParserDispatcher, RevetConfig};
use std::path::PathBuf;
use std::time::Instant;

fn collect_source_files() -> (Vec<PathBuf>, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let dispatcher = ParserDispatcher::new();
    let extensions = dispatcher.supported_extensions();

    let mut files = Vec::new();
    collect_files_recursive(&root.join("crates"), &extensions, &mut files);
    // Also include test fixtures if present
    collect_files_recursive(&root.join("tests"), &extensions, &mut files);

    (files, root)
}

fn collect_files_recursive(dir: &std::path::Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
    if !dir.exists() {
        return;
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .map_or(false, |n| n == "target" || n == ".git")
            {
                continue;
            }
            collect_files_recursive(&path, extensions, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let with_dot = format!(".{}", ext);
            if extensions.contains(&with_dot.as_str()) {
                out.push(path);
            }
        }
    }
}

#[test]
#[ignore] // Run explicitly with --ignored
fn bench_parse_sequential_vs_parallel() {
    let (files, root) = collect_source_files();
    let dispatcher = ParserDispatcher::new();

    println!("\n=== Parse Benchmark ({} files) ===", files.len());

    // Warm up tree-sitter
    let _ = dispatcher.parse_files_parallel(&files, root.clone());

    // Sequential
    let iterations = 5;
    let mut seq_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let mut graph = CodeGraph::new(root.clone());
        for file in &files {
            let _ = dispatcher.parse_file(file, &mut graph);
        }
        let elapsed = start.elapsed();
        seq_times.push(elapsed);
    }

    // Parallel
    let mut par_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = dispatcher.parse_files_parallel(&files, root.clone());
        let elapsed = start.elapsed();
        par_times.push(elapsed);
    }

    let seq_avg = seq_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / iterations as f64;
    let par_avg = par_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / iterations as f64;
    let speedup = seq_avg / par_avg;

    println!("  Sequential: {:.3}s (avg of {})", seq_avg, iterations);
    println!("  Parallel:   {:.3}s (avg of {})", par_avg, iterations);
    println!("  Speedup:    {:.2}x", speedup);
    println!();
}

#[test]
#[ignore]
fn bench_analyzers_sequential_vs_parallel() {
    let (files, root) = collect_source_files();
    let config = RevetConfig::default();
    let dispatcher = AnalyzerDispatcher::new();

    println!("\n=== Analyzer Benchmark ({} files) ===", files.len());

    // Warm up
    let _ = dispatcher.run_all_parallel(&files, &root, &config);

    let iterations = 5;

    // Sequential
    let mut seq_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = dispatcher.run_all(&files, &root, &config);
        seq_times.push(start.elapsed());
    }

    // Parallel
    let mut par_times = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        let _ = dispatcher.run_all_parallel(&files, &root, &config);
        par_times.push(start.elapsed());
    }

    let seq_avg = seq_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / iterations as f64;
    let par_avg = par_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / iterations as f64;
    let speedup = seq_avg / par_avg;

    println!("  Sequential: {:.3}s (avg of {})", seq_avg, iterations);
    println!("  Parallel:   {:.3}s (avg of {})", par_avg, iterations);
    println!("  Speedup:    {:.2}x", speedup);
    println!();
}
