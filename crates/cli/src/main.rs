//! Revet CLI - Code review agent

mod commands;
mod license;
mod output;
#[allow(dead_code)]
mod progress;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "revet")]
#[command(about = "Code review that understands your architecture", long_about = None)]
#[command(version = revet_core::VERSION)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Diff base (default: main)
    #[arg(long, global = true)]
    diff: Option<String>,

    /// Analyze entire repo
    #[arg(long, global = true)]
    full: bool,

    /// Enable LLM reasoning (opt-in)
    #[arg(long, global = true)]
    ai: bool,

    /// Specific domain modules to run
    #[arg(long, value_delimiter = ',', global = true)]
    module: Option<Vec<String>>,

    /// Output format
    #[arg(long, value_enum, global = true)]
    format: Option<OutputFormat>,

    /// Severity threshold for non-zero exit: error, warning, info, never
    #[arg(long, global = true)]
    fail_on: Option<String>,

    /// Apply automatic fixes
    #[arg(long, global = true)]
    fix: bool,

    /// Ignore baseline — show all findings
    #[arg(long, global = true)]
    no_baseline: bool,

    /// Max cost for LLM calls in USD
    #[arg(long, global = true)]
    max_cost: Option<f64>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize .revet.toml configuration
    Init {
        /// Path to initialize (default: current directory)
        path: Option<PathBuf>,
    },

    /// Explain a specific finding in detail
    Explain {
        /// Finding ID to explain
        finding_id: String,

        /// Use AI for explanation
        #[arg(long)]
        ai: bool,
    },

    /// Review code changes (default command)
    Review {
        /// Path to repository (default: current directory)
        path: Option<PathBuf>,
    },

    /// Show findings only on changed lines
    Diff {
        /// Base branch or commit to diff against
        base: String,
    },

    /// Snapshot current findings as a baseline
    Baseline {
        /// Path to repository (default: current directory)
        path: Option<PathBuf>,

        /// Remove the existing baseline
        #[arg(long)]
        clear: bool,
    },

    /// Watch for file changes and analyze continuously
    Watch {
        /// Path to repository (default: current directory)
        path: Option<PathBuf>,

        /// Debounce duration in milliseconds
        #[arg(long, default_value = "300")]
        debounce: u64,

        /// Don't clear screen between runs
        #[arg(long)]
        no_clear: bool,
    },

    /// Manage license and authentication
    Auth {
        #[command(subcommand)]
        action: Option<commands::auth::AuthAction>,

        /// Set license key directly
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum OutputFormat {
    Terminal,
    Json,
    Sarif,
    Github,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Auth command doesn't need license loading
    if let Some(Commands::Auth {
        ref action,
        ref key,
    }) = cli.command
    {
        return commands::auth::run(action.as_ref(), key.as_deref());
    }

    let lic = license::load_license();

    match cli.command {
        Some(Commands::Init { path }) => {
            commands::init::run(path.as_deref())?;
        }
        Some(Commands::Explain { finding_id, ai }) => {
            commands::explain::run(&finding_id, ai, &lic)?;
        }
        Some(Commands::Review { ref path }) => {
            let exit_code = commands::review::run(path.as_deref(), &cli, &lic)?;
            if exit_code == commands::review::ReviewExitCode::FindingsExceedThreshold {
                std::process::exit(1);
            }
        }
        Some(Commands::Diff { ref base }) => {
            let exit_code = commands::diff::run(base, &cli, &lic)?;
            if exit_code == commands::review::ReviewExitCode::FindingsExceedThreshold {
                std::process::exit(1);
            }
        }
        Some(Commands::Baseline { ref path, clear }) => {
            commands::baseline::run(path.as_deref(), clear)?;
        }
        Some(Commands::Watch {
            ref path,
            debounce,
            no_clear,
        }) => {
            commands::watch::run(path.as_deref(), &cli, debounce, no_clear, &lic)?;
        }
        Some(Commands::Auth { .. }) => unreachable!(),
        None => {
            let exit_code = commands::review::run(None, &cli, &lic)?;
            if exit_code == commands::review::ReviewExitCode::FindingsExceedThreshold {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
