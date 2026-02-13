//! Anonymous machine ID for license validation

use sha2::{Digest, Sha256};

/// Returns a stable anonymous machine identifier (first 16 hex chars of SHA-256).
///
/// Input: username + hostname. Falls back to config dir path if either is unavailable.
/// No PII is sent — just a stable hash.
pub fn machine_id() -> String {
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default();

    let host = hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_default();

    let seed = if !user.is_empty() || !host.is_empty() {
        format!("{}@{}", user, host)
    } else {
        // Fallback: use config dir path as seed
        dirs::config_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "revet-unknown".to_string())
    };

    let hash = Sha256::digest(seed.as_bytes());
    hex_encode(&hash[..8])
}

pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
