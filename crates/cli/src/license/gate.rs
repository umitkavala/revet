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
/// Prints one combined warning line if any modules were gated.
pub fn apply_license_gates(config: &mut RevetConfig, license: &License) {
    let mut gated: Vec<&str> = Vec::new();

    if config.modules.ml && !license.has_feature("ml_module") {
        config.modules.ml = false;
        gated.push("ml");
    }
    if config.modules.infra && !license.has_feature("infra_module") {
        config.modules.infra = false;
        gated.push("infra");
    }
    if config.modules.react && !license.has_feature("react_module") {
        config.modules.react = false;
        gated.push("react");
    }
    if config.modules.async_patterns && !license.has_feature("async_module") {
        config.modules.async_patterns = false;
        gated.push("async_patterns");
    }
    if config.modules.dependency && !license.has_feature("dependency_module") {
        config.modules.dependency = false;
        gated.push("dependency");
    }

    if !gated.is_empty() {
        eprintln!(
            "  {} modules [{}] require {}. Run '{}' to upgrade.",
            "\u{26a1}".yellow(),
            gated.join(", ").bold(),
            "Pro".cyan(),
            "revet auth".bold(),
        );
    }
}
