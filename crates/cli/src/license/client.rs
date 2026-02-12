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
