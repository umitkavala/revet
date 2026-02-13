use revet_cli::commands::explain::{extract_prefix, get_explanation};

#[test]
fn test_extract_prefix_standard() {
    assert_eq!(extract_prefix("SEC-001"), "SEC");
    assert_eq!(extract_prefix("SQL-123"), "SQL");
    assert_eq!(extract_prefix("ML-042"), "ML");
}

#[test]
fn test_extract_prefix_no_number() {
    assert_eq!(extract_prefix("SEC"), "SEC");
    assert_eq!(extract_prefix("IMPACT"), "IMPACT");
}

#[test]
fn test_all_known_prefixes() {
    let known = [
        "SEC", "SQL", "ML", "INFRA", "HOOKS", "ASYNC", "DEP", "ERR", "CUSTOM", "SUPPRESS",
        "IMPACT", "PARSE",
    ];
    for prefix in &known {
        assert!(
            get_explanation(prefix).is_some(),
            "Missing explanation for prefix: {}",
            prefix
        );
    }
}

#[test]
fn test_unknown_prefix() {
    assert!(get_explanation("FOOBAR").is_none());
    assert!(get_explanation("XYZ").is_none());
    assert!(get_explanation("").is_none());
}

#[test]
fn test_explanation_has_content() {
    let known = [
        "SEC", "SQL", "ML", "INFRA", "HOOKS", "ASYNC", "DEP", "ERR", "CUSTOM", "SUPPRESS",
        "IMPACT", "PARSE",
    ];
    for prefix in &known {
        let exp = get_explanation(prefix).unwrap();
        assert!(!exp.prefix.is_empty(), "prefix is empty");
        assert!(!exp.name.is_empty(), "name is empty for {}", exp.prefix);
        assert!(
            !exp.description.is_empty(),
            "description is empty for {}",
            exp.prefix
        );
        assert!(
            !exp.why_it_matters.is_empty(),
            "why_it_matters is empty for {}",
            exp.prefix
        );
        assert!(
            !exp.how_to_fix.is_empty(),
            "how_to_fix is empty for {}",
            exp.prefix
        );
        assert!(
            !exp.example_bad.is_empty(),
            "example_bad is empty for {}",
            exp.prefix
        );
        assert!(
            !exp.example_good.is_empty(),
            "example_good is empty for {}",
            exp.prefix
        );
        assert!(
            !exp.references.is_empty(),
            "references is empty for {}",
            exp.prefix
        );
    }
}

#[test]
fn test_sec_explanation() {
    let exp = get_explanation("SEC").unwrap();
    assert!(
        exp.description.to_lowercase().contains("secret"),
        "SEC description should mention 'secret'"
    );
}

#[test]
fn test_sql_explanation() {
    let exp = get_explanation("SQL").unwrap();
    assert!(
        exp.description.to_lowercase().contains("injection"),
        "SQL description should mention 'injection'"
    );
}

#[test]
fn test_ml_explanation() {
    let exp = get_explanation("ML").unwrap();
    let desc = exp.description.to_lowercase();
    assert!(
        desc.contains("pipeline") || desc.contains("leakage"),
        "ML description should mention 'pipeline' or 'leakage'"
    );
}
