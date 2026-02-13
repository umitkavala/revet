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

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_id_is_deterministic() {
        let id1 = machine_id();
        let id2 = machine_id();
        assert_eq!(id1, id2);
    }

    #[test]
    fn machine_id_is_16_hex_chars() {
        let id = machine_id();
        assert_eq!(id.len(), 16, "Expected 16 hex chars, got: {id}");
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit()),
            "Non-hex char in: {id}"
        );
    }

    #[test]
    fn hex_encode_empty() {
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn hex_encode_known_values() {
        assert_eq!(hex_encode(&[0x00]), "00");
        assert_eq!(hex_encode(&[0xff]), "ff");
        assert_eq!(hex_encode(&[0xde, 0xad, 0xbe, 0xef]), "deadbeef");
    }
}
