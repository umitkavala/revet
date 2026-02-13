//! Core types for the license/entitlement system

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

/// Feature constants — Free tier
pub const FREE_FEATURES: &[&str] = &[
    "graph",
    "cross_file_impact",
    "dead_code",
    "circular_deps",
    "basic_security",
    "sarif_output",
    "auto_fix",
    "ml_module",
    "infra_module",
    "react_module",
    "async_module",
    "dependency_module",
    "error_handling_module",
    "explain",
];

/// Feature constants — Pro tier (includes Free)
pub const PRO_FEATURES: &[&str] = &["ai_reasoning"];

/// Feature constants — Team tier (includes Pro + Free)
pub const TEAM_FEATURES: &[&str] = &["shared_config", "github_action", "team_dashboard"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    #[default]
    Free,
    Pro,
    Team,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tier::Free => write!(f, "Free"),
            Tier::Pro => write!(f, "Pro"),
            Tier::Team => write!(f, "Team"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub tier: Tier,
    pub features: HashSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_at: Option<u64>,
}

impl License {
    pub fn has_feature(&self, feature: &str) -> bool {
        self.features.contains(feature)
    }
}

impl Default for License {
    fn default() -> Self {
        Self {
            tier: Tier::Free,
            features: FREE_FEATURES.iter().map(|s| s.to_string()).collect(),
            expires_at: None,
            cached_at: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum LicenseError {
    #[error("\u{26a1} {feature} requires {required_tier}. Run 'revet auth' to upgrade.")]
    FeatureNotLicensed {
        feature: String,
        required_tier: Tier,
    },
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Invalid license key")]
    InvalidKey,
    #[error("License has expired")]
    Expired,
}

/// Returns the minimum tier required for a given feature.
pub fn required_tier(feature: &str) -> Tier {
    if FREE_FEATURES.contains(&feature) {
        return Tier::Free;
    }
    if PRO_FEATURES.contains(&feature) {
        return Tier::Pro;
    }
    if TEAM_FEATURES.contains(&feature) {
        return Tier::Team;
    }
    // Unknown features default to Pro
    Tier::Pro
}
