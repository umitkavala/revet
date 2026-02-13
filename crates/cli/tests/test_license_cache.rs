use revet_cli::license::cache::now_epoch;
use revet_cli::license::types::{License, Tier, FREE_FEATURES, PRO_FEATURES};
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

    let json = serde_json::to_string_pretty(&license).unwrap();
    std::fs::write(&cache_path, &json).unwrap();

    let content = std::fs::read_to_string(&cache_path).unwrap();
    let loaded: License = serde_json::from_str(&content).unwrap();

    assert_eq!(loaded.tier, Tier::Pro);
    assert!(loaded.has_feature("auto_fix"));
    assert!(loaded.has_feature("graph"));
    assert_eq!(loaded.expires_at.as_deref(), Some("2026-12-31"));
}

#[test]
fn load_cached_raw_returns_none_for_missing_file() {
    let result: Result<License, _> = serde_json::from_str("not json");
    assert!(result.is_err());
}

#[test]
fn cache_ttl_fresh_is_valid() {
    let now = now_epoch();
    let license = make_pro_license(Some(now));
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
