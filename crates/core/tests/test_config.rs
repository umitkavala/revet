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
