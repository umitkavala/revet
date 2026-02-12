//! License/entitlement system for revet CLI
//!
//! Handles key storage, remote validation, caching, and feature gating.
//! Core library (revet-core) stays completely license-unaware.

pub mod cache;
pub mod client;
pub mod gate;
pub mod machine;
pub mod types;

pub use types::License;

/// Loads the current license.
///
/// Flow:
/// 1. Read stored key — if none, return Free tier
/// 2. Check cache (24h TTL) — if valid, return cached license
/// 3. Validate key against API — cache result on success
/// 4. On network error, use expired cache (offline grace)
/// 5. On invalid/expired key, return Free tier + print warning
///
/// Never blocks startup for more than 5s (reqwest timeout).
pub fn load_license() -> License {
    // 1. Load key
    let key = match cache::load_key() {
        Some(k) => k,
        None => return License::default(),
    };

    // 2. Check cache (within TTL)
    if let Some(cached) = cache::load_cached() {
        return cached;
    }

    // 3. Validate against API
    let machine = machine::machine_id();
    match client::validate_key(&key, &machine) {
        Ok(license) => {
            let _ = cache::save_cache(&license);
            license
        }
        Err(types::LicenseError::NetworkError(_)) => {
            // Offline grace: use expired cache if available
            if let Some(cached) = cache::load_cached_any() {
                eprintln!("  \u{26a1} Using cached license (API unreachable)");
                return cached;
            }
            License::default()
        }
        Err(types::LicenseError::InvalidKey) => {
            eprintln!("  \u{26a1} License key is invalid. Run 'revet auth' to re-authenticate.");
            License::default()
        }
        Err(types::LicenseError::Expired) => {
            eprintln!("  \u{26a1} License has expired. Run 'revet auth' to renew.");
            License::default()
        }
        Err(_) => License::default(),
    }
}
