//! Feature gating — checks license entitlements and downgrades config

use super::types::{required_tier, License, LicenseError};
use colored::Colorize;
use revet_core::RevetConfig;

/// Returns `Ok(())` if the license includes the feature, otherwise `Err(FeatureNotLicensed)`.
#[allow(dead_code)]
pub fn require_feature(feature: &str, license: &License) -> Result<(), LicenseError> {
    if license.has_feature(feature) {
        Ok(())
    } else {
        Err(LicenseError::FeatureNotLicensed {
            feature: feature.to_string(),
            required_tier: required_tier(feature),
        })
    }
}

/// Returns `true` if the feature is allowed.
/// If not, prints a warning to stderr and returns `false`. Never panics or exits.
#[allow(dead_code)]
pub fn check_and_warn(feature: &str, label: &str, license: &License) -> bool {
    if license.has_feature(feature) {
        return true;
    }
    let tier = required_tier(feature);
    eprintln!(
        "  {} {} requires {}. Run '{}' to upgrade. Continuing without {}...",
        "\u{26a1}".yellow(),
        label.bold(),
        tier.to_string().cyan(),
        "revet auth".bold(),
        label,
    );
    false
}

/// Disables config modules that the current license doesn't cover.
/// Currently a no-op: all deterministic modules are free.
/// Retained for future LLM/cloud feature gating.
pub fn apply_license_gates(_config: &mut RevetConfig, _license: &License) {
    // All deterministic features (analyzers, --fix, explain) are free.
    // This function will gate LLM/cloud features when Layer 3 is added.
}
