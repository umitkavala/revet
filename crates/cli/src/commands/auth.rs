//! Auth command — manage license key and authentication

use anyhow::Result;
use colored::Colorize;

use crate::license;

/// Runs `revet auth` with the given action.
///
/// - No args, no --key: open browser to sign in
/// - `--key <KEY>`: save key and validate
/// - `status`: show current license
/// - `logout`: remove stored credentials
pub fn run(action: Option<&AuthAction>, key: Option<&str>) -> Result<()> {
    if let Some(k) = key {
        return run_set_key(k);
    }

    match action {
        Some(AuthAction::Status) => run_status(),
        Some(AuthAction::Logout) => run_logout(),
        None => run_browser(),
    }
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum AuthAction {
    /// Show current license status
    Status,
    /// Remove stored credentials
    Logout,
}

fn run_browser() -> Result<()> {
    eprintln!("{}", "  Opening browser to sign in...".bold());
    eprintln!();

    if let Err(e) = open::that("https://revet.dev/auth?cli=true") {
        eprintln!("  {} Could not open browser: {}", "Error:".red().bold(), e);
        eprintln!();
        eprintln!(
            "  Visit {} to get your license key, then run:",
            "https://revet.dev/auth".bold()
        );
        eprintln!("    {}", "revet auth --key <YOUR_KEY>".bold());
        return Ok(());
    }

    eprintln!("  After signing in, copy your license key and run:");
    eprintln!("    {}", "revet auth --key <YOUR_KEY>".bold());
    eprintln!();

    Ok(())
}

fn run_set_key(key: &str) -> Result<()> {
    eprint!("  Saving license key... ");
    license::cache::save_key(key)?;
    eprintln!("{}", "done".green());

    eprint!("  Validating... ");
    let machine = license::machine::machine_id();
    match license::client::validate_key(key, &machine) {
        Ok(lic) => {
            let _ = license::cache::save_cache(&lic);
            eprintln!("{}", "done".green());
            eprintln!();
            print_license_info(&lic);
        }
        Err(license::types::LicenseError::NetworkError(e)) => {
            eprintln!("{}", "offline".yellow());
            eprintln!(
                "  Key saved. Validation will happen on next run. ({})",
                e.dimmed()
            );
        }
        Err(e) => {
            eprintln!("{}", "failed".red());
            eprintln!("  {}", e);
            // Remove invalid key
            let _ = license::cache::remove_key();
        }
    }

    Ok(())
}

fn run_status() -> Result<()> {
    let lic = license::load_license();
    print_license_info(&lic);
    Ok(())
}

fn run_logout() -> Result<()> {
    license::cache::remove_key()?;
    eprintln!("  {} Credentials removed.", "\u{2713}".green());
    Ok(())
}

fn print_license_info(license: &license::License) {
    eprintln!("  {}: {}", "Tier".bold(), license.tier.to_string().cyan());

    if let Some(ref expires) = license.expires_at {
        eprintln!("  {}: {}", "Expires".bold(), expires);
    }

    let mut features: Vec<&str> = license.features.iter().map(|s| s.as_str()).collect();
    features.sort();
    eprintln!("  {}: {}", "Features".bold(), features.join(", ").dimmed());
    eprintln!();
}
