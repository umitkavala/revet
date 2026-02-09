use revet_core::ReviewSummary;

#[test]
fn threshold_error_fails_on_errors() {
    let summary = ReviewSummary {
        errors: 1,
        warnings: 0,
        info: 0,
        ..Default::default()
    };
    assert!(summary.exceeds_threshold("error"));
}

#[test]
fn threshold_error_passes_on_warnings_only() {
    let summary = ReviewSummary {
        errors: 0,
        warnings: 5,
        info: 3,
        ..Default::default()
    };
    assert!(!summary.exceeds_threshold("error"));
}

#[test]
fn threshold_warning_fails_on_warnings() {
    let summary = ReviewSummary {
        errors: 0,
        warnings: 2,
        info: 0,
        ..Default::default()
    };
    assert!(summary.exceeds_threshold("warning"));
}

#[test]
fn threshold_info_fails_on_any_finding() {
    let summary = ReviewSummary {
        errors: 0,
        warnings: 0,
        info: 1,
        ..Default::default()
    };
    assert!(summary.exceeds_threshold("info"));
}

#[test]
fn threshold_never_always_passes() {
    let summary = ReviewSummary {
        errors: 10,
        warnings: 20,
        info: 30,
        ..Default::default()
    };
    assert!(!summary.exceeds_threshold("never"));
}
