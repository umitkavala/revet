//! HTTP client for license validation against api.revet.dev

use super::types::{License, LicenseError, Tier, FREE_FEATURES, PRO_FEATURES, TEAM_FEATURES};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::Duration;

const VALIDATE_URL: &str = "https://api.revet.dev/v1/license/validate";
const TIMEOUT_SECS: u64 = 5;

#[derive(Serialize)]
struct ValidateRequest<'a> {
    key: &'a str,
    machine_id: &'a str,
}

#[derive(Deserialize)]
struct ValidateResponse {
    valid: bool,
    tier: Option<String>,
    features: Option<Vec<String>>,
    expires_at: Option<String>,
}

/// Validates a license key against the remote API.
///
/// Returns the validated `License` on success.
/// On network/timeout errors, returns `LicenseError::NetworkError`.
/// On invalid key, returns `LicenseError::InvalidKey`.
pub fn validate_key(key: &str, machine_id: &str) -> Result<License, LicenseError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| LicenseError::NetworkError(e.to_string()))?;

    let body = ValidateRequest { key, machine_id };

    let resp = client
        .post(VALIDATE_URL)
        .json(&body)
        .send()
        .map_err(|e| LicenseError::NetworkError(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(LicenseError::NetworkError(format!(
            "HTTP {}",
            resp.status()
        )));
    }

    let data: ValidateResponse = resp
        .json()
        .map_err(|e| LicenseError::NetworkError(e.to_string()))?;

    if !data.valid {
        return Err(LicenseError::InvalidKey);
    }

    let tier = match data.tier.as_deref() {
        Some("team") => Tier::Team,
        Some("pro") => Tier::Pro,
        _ => Tier::Free,
    };

    let features = match data.features {
        Some(f) => f.into_iter().collect::<HashSet<String>>(),
        None => build_default_features(tier),
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(License {
        tier,
        features,
        expires_at: data.expires_at,
        cached_at: Some(now),
    })
}

/// Builds the default feature set for a given tier.
fn build_default_features(tier: Tier) -> HashSet<String> {
    let mut features: HashSet<String> = FREE_FEATURES.iter().map(|s| s.to_string()).collect();
    if matches!(tier, Tier::Pro | Tier::Team) {
        features.extend(PRO_FEATURES.iter().map(|s| s.to_string()));
    }
    if tier == Tier::Team {
        features.extend(TEAM_FEATURES.iter().map(|s| s.to_string()));
    }
    features
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_default_features_free() {
        let features = build_default_features(Tier::Free);
        for f in FREE_FEATURES {
            assert!(features.contains(*f), "Free should include {f}");
        }
        for f in PRO_FEATURES {
            assert!(!features.contains(*f), "Free should not include {f}");
        }
        for f in TEAM_FEATURES {
            assert!(!features.contains(*f), "Free should not include {f}");
        }
    }

    #[test]
    fn build_default_features_pro() {
        let features = build_default_features(Tier::Pro);
        for f in FREE_FEATURES {
            assert!(features.contains(*f), "Pro should include Free feature {f}");
        }
        for f in PRO_FEATURES {
            assert!(features.contains(*f), "Pro should include {f}");
        }
        for f in TEAM_FEATURES {
            assert!(
                !features.contains(*f),
                "Pro should not include Team feature {f}"
            );
        }
    }

    #[test]
    fn build_default_features_team() {
        let features = build_default_features(Tier::Team);
        for f in FREE_FEATURES {
            assert!(
                features.contains(*f),
                "Team should include Free feature {f}"
            );
        }
        for f in PRO_FEATURES {
            assert!(features.contains(*f), "Team should include Pro feature {f}");
        }
        for f in TEAM_FEATURES {
            assert!(features.contains(*f), "Team should include {f}");
        }
    }

    #[test]
    fn build_default_features_counts() {
        let free = build_default_features(Tier::Free);
        let pro = build_default_features(Tier::Pro);
        let team = build_default_features(Tier::Team);

        assert_eq!(free.len(), FREE_FEATURES.len());
        assert_eq!(pro.len(), FREE_FEATURES.len() + PRO_FEATURES.len());
        assert_eq!(
            team.len(),
            FREE_FEATURES.len() + PRO_FEATURES.len() + TEAM_FEATURES.len()
        );
    }

    #[test]
    fn validate_response_deserialization() {
        let json = r#"{"valid": true, "tier": "pro", "features": ["auto_fix", "ml_module"], "expires_at": "2026-12-31"}"#;
        let resp: ValidateResponse = serde_json::from_str(json).unwrap();
        assert!(resp.valid);
        assert_eq!(resp.tier.as_deref(), Some("pro"));
        assert_eq!(resp.features.as_ref().unwrap().len(), 2);
        assert_eq!(resp.expires_at.as_deref(), Some("2026-12-31"));
    }

    #[test]
    fn validate_response_minimal() {
        let json = r#"{"valid": false}"#;
        let resp: ValidateResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.valid);
        assert!(resp.tier.is_none());
        assert!(resp.features.is_none());
        assert!(resp.expires_at.is_none());
    }

    #[test]
    fn validate_response_unknown_tier_maps_to_free() {
        // Simulates the logic in validate_key
        let tier_str: Option<&str> = Some("enterprise");
        let tier = match tier_str {
            Some("team") => Tier::Team,
            Some("pro") => Tier::Pro,
            _ => Tier::Free,
        };
        assert_eq!(tier, Tier::Free);
    }

    #[test]
    fn validate_response_none_features_uses_defaults() {
        // When API returns no features, build_default_features is used
        let features: Option<Vec<String>> = None;
        let result = match features {
            Some(f) => f.into_iter().collect::<HashSet<String>>(),
            None => build_default_features(Tier::Pro),
        };
        assert!(result.contains("auto_fix"));
        assert!(result.contains("graph"));
    }

    #[test]
    fn validate_request_serialization() {
        let req = ValidateRequest {
            key: "test-key",
            machine_id: "abc123",
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("test-key"));
        assert!(json.contains("abc123"));
    }
}
