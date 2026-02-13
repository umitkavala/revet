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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::types::{
        Tier, FREE_FEATURES, PRO_FEATURES, TEAM_FEATURES,
    };
    use std::collections::HashSet;

    fn make_license(tier: Tier) -> License {
        let mut features: HashSet<String> =
            FREE_FEATURES.iter().map(|s| s.to_string()).collect();
        if matches!(tier, Tier::Pro | Tier::Team) {
            features.extend(PRO_FEATURES.iter().map(|s| s.to_string()));
        }
        if tier == Tier::Team {
            features.extend(TEAM_FEATURES.iter().map(|s| s.to_string()));
        }
        License {
            tier,
            features,
            expires_at: None,
            cached_at: None,
        }
    }

    fn all_modules_enabled() -> RevetConfig {
        let mut config = RevetConfig::default();
        config.modules.ml = true;
        config.modules.security = true;
        config.modules.infra = true;
        config.modules.react = true;
        config.modules.async_patterns = true;
        config.modules.dependency = true;
        config
    }

    // --- apply_license_gates ---

    #[test]
    fn free_tier_disables_pro_modules() {
        let license = make_license(Tier::Free);
        let mut config = all_modules_enabled();

        apply_license_gates(&mut config, &license);

        // Free tier keeps security (basic_security is Free)
        assert!(config.modules.security);
        // Free tier disables all Pro modules
        assert!(!config.modules.ml, "ml should be gated on Free");
        assert!(!config.modules.infra, "infra should be gated on Free");
        assert!(!config.modules.react, "react should be gated on Free");
        assert!(!config.modules.async_patterns, "async should be gated on Free");
        assert!(!config.modules.dependency, "dependency should be gated on Free");
    }

    #[test]
    fn pro_tier_keeps_all_modules() {
        let license = make_license(Tier::Pro);
        let mut config = all_modules_enabled();

        apply_license_gates(&mut config, &license);

        assert!(config.modules.security);
        assert!(config.modules.ml);
        assert!(config.modules.infra);
        assert!(config.modules.react);
        assert!(config.modules.async_patterns);
        assert!(config.modules.dependency);
    }

    #[test]
    fn team_tier_keeps_all_modules() {
        let license = make_license(Tier::Team);
        let mut config = all_modules_enabled();

        apply_license_gates(&mut config, &license);

        assert!(config.modules.security);
        assert!(config.modules.ml);
        assert!(config.modules.infra);
        assert!(config.modules.react);
        assert!(config.modules.async_patterns);
        assert!(config.modules.dependency);
    }

    #[test]
    fn free_tier_does_not_gate_already_disabled_modules() {
        let license = make_license(Tier::Free);
        let mut config = RevetConfig::default();
        // All extra modules default to false already
        config.modules.ml = false;
        config.modules.infra = false;
        config.modules.react = false;
        config.modules.async_patterns = false;
        config.modules.dependency = false;

        apply_license_gates(&mut config, &license);

        // Nothing should change — modules were already off
        assert!(!config.modules.ml);
        assert!(!config.modules.infra);
        assert!(!config.modules.react);
        assert!(!config.modules.async_patterns);
        assert!(!config.modules.dependency);
    }

    #[test]
    fn gate_only_disables_unlicensed_modules() {
        // License has only ml_module and react_module enabled (custom feature set)
        let mut features: HashSet<String> =
            FREE_FEATURES.iter().map(|s| s.to_string()).collect();
        features.insert("ml_module".to_string());
        features.insert("react_module".to_string());

        let license = License {
            tier: Tier::Pro,
            features,
            expires_at: None,
            cached_at: None,
        };

        let mut config = all_modules_enabled();
        apply_license_gates(&mut config, &license);

        assert!(config.modules.ml, "ml_module is licensed");
        assert!(config.modules.react, "react_module is licensed");
        assert!(!config.modules.infra, "infra_module is not licensed");
        assert!(!config.modules.async_patterns, "async_module is not licensed");
        assert!(!config.modules.dependency, "dependency_module is not licensed");
    }

    // --- require_feature ---

    #[test]
    fn require_feature_ok_when_licensed() {
        let license = make_license(Tier::Pro);
        assert!(require_feature("auto_fix", &license).is_ok());
        assert!(require_feature("graph", &license).is_ok());
    }

    #[test]
    fn require_feature_err_when_not_licensed() {
        let license = make_license(Tier::Free);
        let err = require_feature("auto_fix", &license).unwrap_err();
        match err {
            LicenseError::FeatureNotLicensed { feature, required_tier } => {
                assert_eq!(feature, "auto_fix");
                assert_eq!(required_tier, Tier::Pro);
            }
            _ => panic!("Expected FeatureNotLicensed"),
        }
    }

    #[test]
    fn require_feature_free_features_always_pass() {
        let license = License::default(); // Free tier
        for f in FREE_FEATURES {
            assert!(require_feature(f, &license).is_ok(), "Free feature {f} should pass");
        }
    }

    // --- check_and_warn ---

    #[test]
    fn check_and_warn_returns_true_when_licensed() {
        let license = make_license(Tier::Pro);
        assert!(check_and_warn("auto_fix", "Auto-fix", &license));
    }

    #[test]
    fn check_and_warn_returns_false_when_not_licensed() {
        let license = make_license(Tier::Free);
        assert!(!check_and_warn("auto_fix", "Auto-fix", &license));
    }
}
