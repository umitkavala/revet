//! Initialize .revet.toml configuration

use anyhow::Result;
use revet_core::RevetConfig;
use std::path::Path;

pub fn run(path: Option<&Path>) -> Result<()> {
    let target_path = path.unwrap_or_else(|| Path::new("."));
    let config_path = target_path.join(".revet.toml");

    if config_path.exists() {
        println!("⚠️  .revet.toml already exists at {:?}", config_path);
        return Ok(());
    }

    let config = RevetConfig::default();
    config.save(&config_path)?;

    println!("✅ Created .revet.toml at {:?}", config_path);
    println!("\nYou can now customize the configuration and run:");
    println!("  revet");

    Ok(())
}
