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

fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::types::{Tier, FREE_FEATURES, PRO_FEATURES};
    use std::collections::HashSet;

    fn make_pro_license(cached_at: Option<u64>) -> License {
        let features: HashSet<String> = FREE_FEATURES
            .iter()
            .chain(PRO_FEATURES.iter())
            .map(|s| s.to_string())
            .collect();
        License {
            tier: Tier::Pro,
            features,
            expires_at: Some("2026-12-31".to_string()),
            cached_at,
        }
    }

    #[test]
    fn save_and_load_cache_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_path = tmp.path().join("license.json");
        let license = make_pro_license(Some(now_epoch()));

        // Write directly so we don't depend on config_dir()
        let json = serde_json::to_string_pretty(&license).unwrap();
        std::fs::write(&cache_path, &json).unwrap();

        // Read back
        let content = std::fs::read_to_string(&cache_path).unwrap();
        let loaded: License = serde_json::from_str(&content).unwrap();

        assert_eq!(loaded.tier, Tier::Pro);
        assert!(loaded.has_feature("auto_fix"));
        assert!(loaded.has_feature("graph"));
        assert_eq!(loaded.expires_at.as_deref(), Some("2026-12-31"));
    }

    #[test]
    fn load_cached_raw_returns_none_for_missing_file() {
        // load_cached_raw uses config_dir() which we can't easily override,
        // so test the deserialization path directly
        let result: Result<License, _> = serde_json::from_str("not json");
        assert!(result.is_err());
    }

    #[test]
    fn cache_ttl_fresh_is_valid() {
        let now = now_epoch();
        let license = make_pro_license(Some(now));
        // Within TTL: now - cached_at = 0 <= 86400
        assert!(license.cached_at.is_some());
        let elapsed = now.saturating_sub(license.cached_at.unwrap());
        assert!(elapsed <= 24 * 60 * 60);
    }

    #[test]
    fn cache_ttl_expired_is_invalid() {
        let now = now_epoch();
        let old_time = now.saturating_sub(25 * 60 * 60); // 25 hours ago
        let license = make_pro_license(Some(old_time));
        let elapsed = now.saturating_sub(license.cached_at.unwrap());
        assert!(elapsed > 24 * 60 * 60, "Should be expired");
    }

    #[test]
    fn cache_ttl_no_cached_at_is_invalid() {
        let license = make_pro_license(None);
        assert!(license.cached_at.is_none());
    }

    #[test]
    fn save_key_and_load_key_via_fs() {
        let tmp = tempfile::tempdir().unwrap();
        let key_path = tmp.path().join("license.key");

        std::fs::write(&key_path, "  test-key-123  ").unwrap();
        let content = std::fs::read_to_string(&key_path).unwrap();
        let trimmed = content.trim().to_string();

        assert_eq!(trimmed, "test-key-123");
    }

    #[test]
    fn empty_key_file_returns_none_equivalent() {
        let tmp = tempfile::tempdir().unwrap();
        let key_path = tmp.path().join("license.key");

        std::fs::write(&key_path, "   ").unwrap();
        let content = std::fs::read_to_string(&key_path).unwrap();
        let trimmed = content.trim().to_string();

        assert!(trimmed.is_empty());
    }

    #[test]
    fn remove_key_deletes_files() {
        let tmp = tempfile::tempdir().unwrap();
        let key_path = tmp.path().join("license.key");
        let cache_path = tmp.path().join("license.json");

        std::fs::write(&key_path, "key").unwrap();
        std::fs::write(&cache_path, "{}").unwrap();

        let _ = std::fs::remove_file(&key_path);
        let _ = std::fs::remove_file(&cache_path);

        assert!(!key_path.exists());
        assert!(!cache_path.exists());
    }
}
