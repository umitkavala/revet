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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::types::{Tier, FREE_FEATURES, PRO_FEATURES, TEAM_FEATURES};
    use std::collections::HashSet;

    fn make_license(tier: Tier) -> License {
        let mut features: HashSet<String> = FREE_FEATURES.iter().map(|s| s.to_string()).collect();
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
        config.modules.error_handling = true;
        config
    }

    // --- apply_license_gates (now a no-op) ---

    #[test]
    fn free_tier_keeps_all_modules() {
        let license = make_license(Tier::Free);
        let mut config = all_modules_enabled();

        apply_license_gates(&mut config, &license);

        // All deterministic modules are free — nothing gets gated
        assert!(config.modules.security);
        assert!(config.modules.ml);
        assert!(config.modules.infra);
        assert!(config.modules.react);
        assert!(config.modules.async_patterns);
        assert!(config.modules.dependency);
        assert!(config.modules.error_handling);
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
        assert!(config.modules.error_handling);
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
        assert!(config.modules.error_handling);
    }

    #[test]
    fn gates_do_not_touch_disabled_modules() {
        let license = make_license(Tier::Free);
        let mut config = RevetConfig::default();
        config.modules.ml = false;
        config.modules.infra = false;

        apply_license_gates(&mut config, &license);

        assert!(!config.modules.ml);
        assert!(!config.modules.infra);
    }

    // --- require_feature ---

    #[test]
    fn require_feature_ok_when_licensed() {
        let license = make_license(Tier::Pro);
        assert!(require_feature("auto_fix", &license).is_ok());
        assert!(require_feature("ai_reasoning", &license).is_ok());
        assert!(require_feature("graph", &license).is_ok());
    }

    #[test]
    fn require_feature_err_when_not_licensed() {
        let license = make_license(Tier::Free);
        let err = require_feature("ai_reasoning", &license).unwrap_err();
        match err {
            LicenseError::FeatureNotLicensed {
                feature,
                required_tier,
            } => {
                assert_eq!(feature, "ai_reasoning");
                assert_eq!(required_tier, Tier::Pro);
            }
            _ => panic!("Expected FeatureNotLicensed"),
        }
    }

    #[test]
    fn require_feature_free_features_always_pass() {
        let license = License::default(); // Free tier
        for f in FREE_FEATURES {
            assert!(
                require_feature(f, &license).is_ok(),
                "Free feature {f} should pass"
            );
        }
    }

    #[test]
    fn auto_fix_and_explain_are_free() {
        let license = License::default(); // Free tier
        assert!(require_feature("auto_fix", &license).is_ok());
        assert!(require_feature("explain", &license).is_ok());
    }

    // --- check_and_warn ---

    #[test]
    fn check_and_warn_returns_true_for_free_features() {
        let license = make_license(Tier::Free);
        assert!(check_and_warn("auto_fix", "Auto-fix", &license));
        assert!(check_and_warn("explain", "explain", &license));
    }

    #[test]
    fn check_and_warn_returns_false_for_pro_features_on_free() {
        let license = make_license(Tier::Free);
        assert!(!check_and_warn("ai_reasoning", "AI reasoning", &license));
    }
}
