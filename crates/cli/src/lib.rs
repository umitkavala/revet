//! Revet CLI library — exposed for integration tests

pub mod commands;
pub mod license;
pub mod output;
#[allow(dead_code)]
pub mod progress;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub use license::License;

#[derive(Parser)]
#[command(name = "revet")]
#[command(about = "Code review that understands your architecture", long_about = None)]
#[command(version = revet_core::VERSION)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Diff base (default: main)
    #[arg(long, global = true)]
    pub diff: Option<String>,

    /// Analyze entire repo
    #[arg(long, global = true)]
    pub full: bool,

    /// Enable LLM reasoning (opt-in)
    #[arg(long, global = true)]
    pub ai: bool,

    /// Specific domain modules to run
    #[arg(long, value_delimiter = ',', global = true)]
    pub module: Option<Vec<String>>,

    /// Output format
    #[arg(long, value_enum, global = true)]
    pub format: Option<OutputFormat>,

    /// Severity threshold for non-zero exit: error, warning, info, never
    #[arg(long, global = true)]
    pub fail_on: Option<String>,

    /// Apply automatic fixes
    #[arg(long, global = true)]
    pub fix: bool,

    /// Ignore baseline — show all findings
    #[arg(long, global = true)]
    pub no_baseline: bool,

    /// Max cost for LLM calls in USD
    #[arg(long, global = true)]
    pub max_cost: Option<f64>,
}

#[derive(Subcommand)]
pub enum Commands {
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
pub enum OutputFormat {
    Terminal,
    Json,
    Sarif,
    Github,
}
