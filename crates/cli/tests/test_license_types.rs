use revet_cli::license::types::{
    required_tier, License, LicenseError, Tier, FREE_FEATURES, PRO_FEATURES, TEAM_FEATURES,
};

// --- Tier ---

#[test]
fn tier_default_is_free() {
    assert_eq!(Tier::default(), Tier::Free);
}

#[test]
fn tier_display() {
    assert_eq!(Tier::Free.to_string(), "Free");
    assert_eq!(Tier::Pro.to_string(), "Pro");
    assert_eq!(Tier::Team.to_string(), "Team");
}

#[test]
fn tier_serde_roundtrip() {
    for tier in [Tier::Free, Tier::Pro, Tier::Team] {
        let json = serde_json::to_string(&tier).unwrap();
        let back: Tier = serde_json::from_str(&json).unwrap();
        assert_eq!(tier, back);
    }
}

#[test]
fn tier_serde_lowercase() {
    assert_eq!(serde_json::to_string(&Tier::Free).unwrap(), "\"free\"");
    assert_eq!(serde_json::to_string(&Tier::Pro).unwrap(), "\"pro\"");
    assert_eq!(serde_json::to_string(&Tier::Team).unwrap(), "\"team\"");
}

// --- License ---

#[test]
fn license_default_is_free_with_free_features() {
    let lic = License::default();
    assert_eq!(lic.tier, Tier::Free);
    for f in FREE_FEATURES {
        assert!(lic.has_feature(f), "Free license missing feature: {f}");
    }
    for f in PRO_FEATURES {
        assert!(!lic.has_feature(f), "Free license should not have: {f}");
    }
    for f in TEAM_FEATURES {
        assert!(!lic.has_feature(f), "Free license should not have: {f}");
    }
}

#[test]
fn license_has_feature() {
    let lic = License::default();
    assert!(lic.has_feature("graph"));
    assert!(lic.has_feature("auto_fix"));
    assert!(!lic.has_feature("ai_reasoning"));
    assert!(!lic.has_feature("nonexistent"));
}

#[test]
fn license_serde_roundtrip() {
    let lic = License {
        tier: Tier::Pro,
        features: PRO_FEATURES
            .iter()
            .chain(FREE_FEATURES.iter())
            .map(|s| s.to_string())
            .collect(),
        expires_at: Some("2026-12-31".to_string()),
        cached_at: Some(1234567890),
    };
    let json = serde_json::to_string(&lic).unwrap();
    let back: License = serde_json::from_str(&json).unwrap();
    assert_eq!(back.tier, Tier::Pro);
    assert!(back.has_feature("auto_fix"));
    assert!(back.has_feature("graph"));
    assert_eq!(back.expires_at.as_deref(), Some("2026-12-31"));
    assert_eq!(back.cached_at, Some(1234567890));
}

#[test]
fn license_serde_optional_fields_omitted() {
    let lic = License::default();
    let json = serde_json::to_string(&lic).unwrap();
    assert!(!json.contains("expires_at"));
    assert!(!json.contains("cached_at"));
}

// --- required_tier ---

#[test]
fn required_tier_free_features() {
    for f in FREE_FEATURES {
        assert_eq!(required_tier(f), Tier::Free, "Expected Free for {f}");
    }
}

#[test]
fn required_tier_pro_features() {
    for f in PRO_FEATURES {
        assert_eq!(required_tier(f), Tier::Pro, "Expected Pro for {f}");
    }
}

#[test]
fn required_tier_team_features() {
    for f in TEAM_FEATURES {
        assert_eq!(required_tier(f), Tier::Team, "Expected Team for {f}");
    }
}

#[test]
fn required_tier_unknown_defaults_to_pro() {
    assert_eq!(required_tier("unknown_feature"), Tier::Pro);
    assert_eq!(required_tier(""), Tier::Pro);
}

// --- Feature lists are non-empty and disjoint ---

#[test]
fn feature_lists_are_non_empty() {
    assert!(!FREE_FEATURES.is_empty());
    assert!(!PRO_FEATURES.is_empty());
    assert!(!TEAM_FEATURES.is_empty());
}

#[test]
fn feature_lists_are_disjoint() {
    for f in PRO_FEATURES {
        assert!(!FREE_FEATURES.contains(f), "Overlap: {f} in Free and Pro");
    }
    for f in TEAM_FEATURES {
        assert!(!FREE_FEATURES.contains(f), "Overlap: {f} in Free and Team");
        assert!(!PRO_FEATURES.contains(f), "Overlap: {f} in Pro and Team");
    }
}

// --- LicenseError ---

#[test]
fn license_error_display() {
    let err = LicenseError::FeatureNotLicensed {
        feature: "auto_fix".to_string(),
        required_tier: Tier::Pro,
    };
    let msg = err.to_string();
    assert!(msg.contains("auto_fix"));
    assert!(msg.contains("Pro"));

    assert!(LicenseError::InvalidKey.to_string().contains("Invalid"));
    assert!(LicenseError::Expired.to_string().contains("expired"));
    assert!(LicenseError::NetworkError("timeout".into())
        .to_string()
        .contains("timeout"));
}
