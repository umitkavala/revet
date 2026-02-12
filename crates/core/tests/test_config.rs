//! Tests for configuration parsing

use revet_core::RevetConfig;

#[test]
fn test_default_config() {
    let config = RevetConfig::default();
    assert_eq!(config.general.diff_base, "main");
    assert!(config.modules.security);
}

#[test]
fn test_serialize_config() {
    let config = RevetConfig::default();
    let toml_str = toml::to_string(&config).unwrap();
    assert!(toml_str.contains("diff_base"));
}

#[test]
fn test_parse_custom_rules() {
    let toml_str = r#"
[[rules]]
id = "no-console-log"
pattern = 'console\.log'
message = "console.log should not be used in production code"
severity = "warning"
paths = ["*.ts", "*.js", "*.tsx"]
suggestion = "Use the logger utility instead"
reject_if_contains = "// eslint-disable"

[[rules]]
pattern = "TODO|FIXME|HACK"
message = "Unresolved TODO/FIXME/HACK comment found"
severity = "info"
paths = ["*.rs", "*.py", "*.ts"]
"#;

    let config: RevetConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.rules.len(), 2);

    let r0 = &config.rules[0];
    assert_eq!(r0.id.as_deref(), Some("no-console-log"));
    assert_eq!(r0.pattern, r"console\.log");
    assert_eq!(
        r0.message,
        "console.log should not be used in production code"
    );
    assert_eq!(r0.severity, "warning");
    assert_eq!(r0.paths, vec!["*.ts", "*.js", "*.tsx"]);
    assert_eq!(
        r0.suggestion.as_deref(),
        Some("Use the logger utility instead")
    );
    assert_eq!(r0.reject_if_contains.as_deref(), Some("// eslint-disable"));

    let r1 = &config.rules[1];
    assert!(r1.id.is_none());
    assert_eq!(r1.severity, "info");
    assert!(r1.suggestion.is_none());
    assert!(r1.reject_if_contains.is_none());
}

#[test]
fn test_default_config_has_no_rules() {
    let config = RevetConfig::default();
    assert!(config.rules.is_empty());
}

#[test]
fn test_parse_custom_rules_with_fix() {
    let toml_str = r#"
[[rules]]
pattern = 'console\.log'
message = "Use logger"
severity = "warning"
paths = ["*.ts"]
suggestion = "Replace console.log with logger.info"
fix_find = 'console\.log\('
fix_replace = 'logger.info('
"#;

    let config: RevetConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(config.rules.len(), 1);

    let r = &config.rules[0];
    assert_eq!(r.fix_find.as_deref(), Some(r"console\.log\("));
    assert_eq!(r.fix_replace.as_deref(), Some("logger.info("));
}

#[test]
fn test_parse_custom_rules_fix_fields_optional() {
    let toml_str = r#"
[[rules]]
pattern = 'TODO'
message = "Unresolved TODO"
severity = "info"
"#;

    let config: RevetConfig = toml::from_str(toml_str).unwrap();
    let r = &config.rules[0];
    assert!(r.fix_find.is_none());
    assert!(r.fix_replace.is_none());
}
