//! License key and cache file management (~/.config/revet/)

use super::types::License;
use anyhow::Result;
use std::path::PathBuf;

const KEY_FILENAME: &str = "license.key";
const CACHE_FILENAME: &str = "license.json";
const CACHE_TTL_SECS: u64 = 24 * 60 * 60; // 24 hours

/// Returns `~/.config/revet/`, creating it if needed.
pub fn config_dir() -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("revet");
    if !dir.exists() {
        std::fs::create_dir_all(&dir).ok()?;
    }
    Some(dir)
}

/// Reads the stored license key, trimmed.
pub fn load_key() -> Option<String> {
    let path = config_dir()?.join(KEY_FILENAME);
    let content = std::fs::read_to_string(path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Saves a license key to disk.
pub fn save_key(key: &str) -> Result<()> {
    let dir =
        config_dir().ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    std::fs::write(dir.join(KEY_FILENAME), key.trim())?;
    Ok(())
}

/// Removes the stored key and cache files.
pub fn remove_key() -> Result<()> {
    if let Some(dir) = config_dir() {
        let _ = std::fs::remove_file(dir.join(KEY_FILENAME));
        let _ = std::fs::remove_file(dir.join(CACHE_FILENAME));
    }
    Ok(())
}

/// Loads cached license if present and within TTL (24h).
pub fn load_cached() -> Option<License> {
    let license = load_cached_raw()?;
    let now = now_epoch();
    if let Some(cached_at) = license.cached_at {
        if now.saturating_sub(cached_at) <= CACHE_TTL_SECS {
            return Some(license);
        }
    }
    None
}

/// Loads cached license regardless of TTL (for offline grace period).
pub fn load_cached_any() -> Option<License> {
    load_cached_raw()
}

/// Saves a license to the cache file.
pub fn save_cache(license: &License) -> Result<()> {
    let dir =
        config_dir().ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
    let json = serde_json::to_string_pretty(license)?;
    std::fs::write(dir.join(CACHE_FILENAME), json)?;
    Ok(())
}

fn load_cached_raw() -> Option<License> {
    let path = config_dir()?.join(CACHE_FILENAME);
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
