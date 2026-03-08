//! `revet config check` — validate `.revet.toml` without running analysis.

use anyhow::Result;
use colored::Colorize;
use revet_core::RevetConfig;
use std::path::{Path, PathBuf};

pub fn run(repo_path: &Path) -> Result<()> {
    // ── 1. Find config file ──────────────────────────────────────
    let config_path = find_config(repo_path);

    match &config_path {
        Some(p) => println!("  {} {}", "Config:".bold(), p.display().to_string().green()),
        None => println!(
            "  {} {}",
            "Config:".bold(),
            "no .revet.toml found — using defaults".dimmed()
        ),
    }

    // ── 2. Load (catches TOML syntax errors) ────────────────────
    let config = match &config_path {
        Some(p) => match RevetConfig::from_file(p) {
            Ok(c) => c,
            Err(e) => {
                println!();
                println!("  {} TOML parse error:", "✗".red().bold());
                println!("    {}", e.to_string().red());
                println!();
                std::process::exit(1);
            }
        },
        None => RevetConfig::default(),
    };

    // ── 3. Validate (catches semantic errors) ───────────────────
    let (errors, warnings) = config.validate();

    // ── 4. Print config summary ──────────────────────────────────
    println!();
    print_modules(&config);
    print_custom_rules(&config);
    print_gate(&config);

    // ── 5. Print validation results ──────────────────────────────
    println!();
    for w in &warnings {
        println!("  {} {}", "warn:".yellow().bold(), w);
    }
    for e in &errors {
        println!("  {} {}", "✗".red().bold(), e);
    }

    if errors.is_empty() {
        println!("  {} Config is valid.", "✓".green().bold());
    } else {
        std::process::exit(1);
    }

    Ok(())
}

// ── Display helpers ──────────────────────────────────────────────────────────

fn print_modules(config: &RevetConfig) {
    let m = &config.modules;
    let modules = [
        ("security", m.security),
        ("ml-pipeline", m.ml),
        ("cycles", m.cycles),
        ("complexity", m.complexity),
        ("infra", m.infra),
        ("react", m.react),
        ("async-patterns", m.async_patterns),
        ("dependency", m.dependency),
        ("error-handling", m.error_handling),
        ("dead-code", m.dead_code),
        ("dead-imports", m.dead_imports),
        ("toolchain", m.toolchain),
        ("hardcoded-endpoints", m.hardcoded_endpoints),
        ("magic-numbers", m.magic_numbers),
        ("test-coverage", m.test_coverage),
        ("duplication", m.duplication),
    ];

    let on: Vec<&str> = modules
        .iter()
        .filter(|(_, e)| *e)
        .map(|(n, _)| *n)
        .collect();
    let off: Vec<&str> = modules
        .iter()
        .filter(|(_, e)| !*e)
        .map(|(n, _)| *n)
        .collect();

    println!("  {}", "Modules".bold());
    println!("    {} {}", "on: ".green(), on.join(", ").green());
    if !off.is_empty() {
        println!("    {} {}", "off:".dimmed(), off.join(", ").dimmed());
    }
}

fn print_custom_rules(config: &RevetConfig) {
    if config.rules.is_empty() {
        return;
    }
    println!();
    println!(
        "  {} {} custom rule(s)",
        "Rules:".bold(),
        config.rules.len()
    );
    for (i, rule) in config.rules.iter().enumerate() {
        let label = rule
            .id
            .as_deref()
            .map(|id| id.to_string())
            .unwrap_or_else(|| format!("[{}]", i));
        let paths = if rule.paths.is_empty() {
            "all files".dimmed().to_string()
        } else {
            rule.paths.join(", ")
        };
        println!(
            "    {} {} ({}, {})",
            "·".dimmed(),
            label.bold(),
            rule.severity,
            paths
        );
    }
}

fn print_gate(config: &RevetConfig) {
    let g = &config.gate;
    if g.is_empty() {
        return;
    }
    let mut parts = Vec::new();
    if let Some(n) = g.error_max {
        parts.push(format!("error ≤ {}", n));
    }
    if let Some(n) = g.warning_max {
        parts.push(format!("warning ≤ {}", n));
    }
    if let Some(n) = g.info_max {
        parts.push(format!("info ≤ {}", n));
    }
    println!();
    println!("  {} {}", "Gate:".bold(), parts.join(", ").yellow());
}

fn find_config(start: &Path) -> Option<PathBuf> {
    let mut current = start;
    loop {
        let p = current.join(".revet.toml");
        if p.exists() {
            return Some(p);
        }
        current = current.parent()?;
    }
}
