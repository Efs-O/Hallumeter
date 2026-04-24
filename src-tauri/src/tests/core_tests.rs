// HalluMeter

use crate::core::{interpolate_curve, load_curves, risk_to_state, AMBER_THRESHOLD, RED_THRESHOLD};

#[test]
fn interpolates_midpoint() {
    // Sonnet knots: 25%=0.10, 40%=0.16. Midpoint 32.5% interpolates linearly.
    let risk = interpolate_curve("claude-sonnet-4-6", 32.5);
    assert!((risk - 0.13).abs() < 0.001, "got {risk}");
}

#[test]
fn clamps_at_zero() {
    let risk = interpolate_curve("claude-sonnet-4-6", 0.0);
    assert!((risk - 0.0).abs() < f64::EPSILON);
}

#[test]
fn baseline_knot_at_session_start() {
    let risk = interpolate_curve("claude-sonnet-4-6", 5.0);
    assert!((risk - 0.03).abs() < f64::EPSILON, "got {risk}");

    let risk_opus = interpolate_curve("claude-opus-4-6", 5.0);
    assert!((risk_opus - 0.02).abs() < f64::EPSILON, "got {risk_opus}");

    let risk_gpt = interpolate_curve("gpt-5-4", 0.0);
    assert!((risk_gpt - 0.00).abs() < f64::EPSILON, "got {risk_gpt}");
}

#[test]
fn clamps_at_hundred() {
    let risk = interpolate_curve("claude-sonnet-4-6", 100.0);
    assert!((risk - 0.45).abs() < f64::EPSILON);
}

#[test]
fn exact_knot_point() {
    let risk = interpolate_curve("claude-sonnet-4-6", 64.0);
    assert!((risk - 0.24).abs() < f64::EPSILON);
}

#[test]
fn unknown_model_uses_generic_curve() {
    let risk = interpolate_curve("gpt-99-unknown", 50.0);
    assert!((0.0..=1.0).contains(&risk));
}

#[test]
fn low_risk_is_green() {
    assert_eq!(
        risk_to_state(AMBER_THRESHOLD - 0.01, AMBER_THRESHOLD, RED_THRESHOLD),
        "green"
    );
}

#[test]
fn amber_threshold_is_amber() {
    assert_eq!(
        risk_to_state(AMBER_THRESHOLD, AMBER_THRESHOLD, RED_THRESHOLD),
        "amber"
    );
}

#[test]
fn red_threshold_is_red() {
    assert_eq!(
        risk_to_state(RED_THRESHOLD, AMBER_THRESHOLD, RED_THRESHOLD),
        "red"
    );
}

#[test]
fn boundary_just_below_red_is_amber() {
    assert_eq!(
        risk_to_state(RED_THRESHOLD - 0.001, AMBER_THRESHOLD, RED_THRESHOLD),
        "amber"
    );
}

#[test]
fn zero_risk_is_green() {
    assert_eq!(risk_to_state(0.0, AMBER_THRESHOLD, RED_THRESHOLD), "green");
}

#[test]
fn full_risk_is_red() {
    assert_eq!(risk_to_state(1.0, AMBER_THRESHOLD, RED_THRESHOLD), "red");
}

#[test]
fn curves_json_parses_without_panic() {
    let config = load_curves();
    assert!(
        !config.models.is_empty(),
        "curves.json must contain at least one model"
    );
    for model in &config.models {
        assert!(!model.id.is_empty());
        assert!(!model.degradation_curve.is_empty());
        let last = model.degradation_curve.last().unwrap();
        assert!(
            (last.fill_pct - 100.0).abs() < f64::EPSILON,
            "last knot must be fill=100"
        );
    }
}
