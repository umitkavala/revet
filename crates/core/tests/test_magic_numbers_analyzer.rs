use revet_core::analyzer::magic_numbers::MagicNumbersAnalyzer;
use revet_core::analyzer::Analyzer;
use revet_core::config::RevetConfig;
use std::io::Write;
use tempfile::NamedTempFile;

fn run(source: &str) -> Vec<revet_core::Finding> {
    let mut f = NamedTempFile::with_suffix(".rs").unwrap();
    f.write_all(source.as_bytes()).unwrap();
    let path = f.path().to_path_buf();
    let analyzer = MagicNumbersAnalyzer::new();
    let mut config = RevetConfig::default();
    config.modules.magic_numbers = true;
    analyzer.analyze_files(&[path], std::path::Path::new("/tmp"))
}

#[test]
fn test_flags_magic_number_in_condition() {
    let findings = run("if retries > 3 { panic!(); }");
    assert!(!findings.is_empty(), "should flag magic number 3");
    assert!(findings[0].message.contains("3"));
}

#[test]
fn test_flags_magic_number_in_expression() {
    let findings = run("let timeout = duration * 60;");
    assert!(!findings.is_empty(), "should flag magic number 60");
}

#[test]
fn test_flags_float_literal() {
    let findings = run("let rate = value * 0.07;");
    assert!(!findings.is_empty(), "should flag 0.07");
}

#[test]
fn test_allows_zero_and_one() {
    let findings = run("if x == 0 || x == 1 { return; }");
    assert!(findings.is_empty(), "0 and 1 are universally understood");
}

#[test]
fn test_allows_minus_one() {
    let findings = run("if result == -1 { return; }");
    assert!(findings.is_empty(), "-1 is universally understood");
}

#[test]
fn test_skips_const_declaration() {
    let findings = run("const MAX_RETRIES: u32 = 3;");
    assert!(
        findings.is_empty(),
        "const declaration should not be flagged"
    );
}

#[test]
fn test_skips_comment_lines() {
    let findings = run("// retry up to 5 times");
    assert!(findings.is_empty(), "comment lines should not be flagged");
}

#[test]
fn test_skips_non_source_extension() {
    // .json files should be skipped
    let mut f = NamedTempFile::with_suffix(".json").unwrap();
    f.write_all(b"{ \"timeout\": 3000 }").unwrap();
    let path = f.path().to_path_buf();
    let analyzer = MagicNumbersAnalyzer::new();
    let mut config = RevetConfig::default();
    config.modules.magic_numbers = true;
    let findings = analyzer.analyze_files(&[path], std::path::Path::new("/tmp"));
    assert!(findings.is_empty(), "json files should be skipped");
}

#[test]
fn test_disabled_by_default() {
    let mut f = NamedTempFile::with_suffix(".rs").unwrap();
    f.write_all(b"if x > 42 {}").unwrap();
    let path = f.path().to_path_buf();
    let analyzer = MagicNumbersAnalyzer::new();
    let config = RevetConfig::default(); // magic_numbers = false
    assert!(
        !analyzer.is_enabled(&config),
        "magic numbers should be disabled by default"
    );
    let _ = analyzer.analyze_files(&[path], std::path::Path::new("/tmp"));
}

#[test]
fn test_finding_metadata() {
    let findings = run("while count < 42 { count += 1; }");
    assert!(!findings.is_empty());
    let f = &findings[0];
    assert!(f.suggestion.is_some());
    assert!(f.message.contains("42"));
}
