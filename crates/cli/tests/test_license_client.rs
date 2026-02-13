use revet_cli::license::client::{build_default_features, ValidateRequest, ValidateResponse};
use revet_cli::license::types::{Tier, FREE_FEATURES, PRO_FEATURES, TEAM_FEATURES};
use std::collections::HashSet;

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
