//! Revet CLI - Code review agent

use anyhow::Result;
use clap::Parser;
use revet_cli::{commands, license, Cli, Commands};

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
