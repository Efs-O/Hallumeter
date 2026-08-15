// HalluMeter

use crate::core::{
    context_window_for, estimate_risk, find_model_curve, interpolate_curve, load_curves,
    risk_to_state, AMBER_THRESHOLD, RED_THRESHOLD,
};

#[test]
fn interpolates_midpoint() {
    // Sonnet knots: 25%=0.10, 40%=0.16. Midpoint 32.5% interpolates linearly.
    let risk = interpolate_curve("claude-sonnet-4-6", 32.5).expect("known model curve");
    assert!((risk - 0.13).abs() < 0.001, "got {risk}");
}

#[test]
fn clamps_at_zero() {
    let risk = interpolate_curve("claude-sonnet-4-6", 0.0).expect("known model curve");
    assert!((risk - 0.0).abs() < f64::EPSILON);
}

#[test]
fn baseline_knot_at_session_start() {
    let risk = interpolate_curve("claude-sonnet-4-6", 5.0).expect("known model curve");
    assert!((risk - 0.03).abs() < f64::EPSILON, "got {risk}");

    let risk_opus = interpolate_curve("claude-opus-4-6", 5.0).expect("known model curve");
    assert!((risk_opus - 0.02).abs() < f64::EPSILON, "got {risk_opus}");

    let risk_gpt = interpolate_curve("gpt-5-4", 0.0).expect("known model curve");
    assert!((risk_gpt - 0.00).abs() < f64::EPSILON, "got {risk_gpt}");
}

#[test]
fn clamps_at_hundred() {
    let risk = interpolate_curve("claude-sonnet-4-6", 100.0).expect("known model curve");
    assert!((risk - 0.45).abs() < f64::EPSILON);
}

#[test]
fn exact_knot_point() {
    let risk = interpolate_curve("claude-sonnet-4-6", 64.0).expect("known model curve");
    assert!((risk - 0.24).abs() < f64::EPSILON);
}

#[test]
fn unknown_model_has_no_risk_score() {
    assert_eq!(interpolate_curve("gpt-99-unknown", 50.0), None);
}

#[test]
fn opus_5_has_its_own_curve() {
    // Regression: claude-opus-5 was missing from curves.json, and 0.1.6's
    // `context_window_for(&model)?` turned that into a hard "Source error".
    assert!(interpolate_curve("claude-opus-5", 50.0).is_some());
    assert_eq!(context_window_for("claude-opus-5"), Some(200_000));
}

#[test]
fn unknown_model_falls_back_and_is_flagged_approximate() {
    // Local Forge models (qwen/gemma GGUFs) match nothing in `models`.
    let est = estimate_risk("qwen38-27b-mtp-q3km", 50.0).expect("fallback curve");
    assert!(est.approximate, "fallback use must be flagged");
    assert!(est.risk_score > 0.0);
}

#[test]
fn known_model_is_not_flagged_approximate() {
    let est = estimate_risk("claude-sonnet-4-6", 64.0).expect("known curve");
    assert!(!est.approximate);
    assert!((est.risk_score - 0.24).abs() < f64::EPSILON);
}

#[test]
fn fallback_is_never_selected_by_family_prefix_matching() {
    // The fallback lives outside `models`, so it can never be matched as a family
    // prefix — otherwise an id like "fallback-x" would silently resolve to it.
    assert!(find_model_curve("fallback").is_none());
    assert_eq!(interpolate_curve("fallback", 50.0), None);
}

#[test]
fn date_suffixed_model_matches_family_curve() {
    // Prefix match on a '-' boundary: date-suffixed ids resolve to the family curve,
    // not the generic first-curve fallback.
    let haiku = interpolate_curve("claude-haiku-4-5-20251001", 25.0).expect("family curve");
    let haiku_exact = interpolate_curve("claude-haiku-4-5", 25.0).expect("exact curve");
    assert!((haiku - haiku_exact).abs() < f64::EPSILON, "got {haiku}");

    let sonnet = interpolate_curve("claude-sonnet-4-6", 25.0).expect("known model curve");
    assert!(
        (haiku - sonnet).abs() > 0.001,
        "haiku must not fall back to sonnet"
    );
}

#[test]
fn prefix_match_requires_dash_boundary() {
    // "claude-sonnet-55" must not match "claude-sonnet-5".
    use crate::core::find_model_curve;
    assert!(find_model_curve("claude-sonnet-5").is_some());
    assert!(find_model_curve("claude-sonnet-55").is_none());
    assert!(find_model_curve("claude-fable-5-20260101").is_some());
}

#[test]
fn context_window_honors_prefix_match() {
    use crate::core::context_window_for;
    assert_eq!(
        context_window_for("claude-haiku-4-5-20251001"),
        Some(200_000)
    );
    assert_eq!(context_window_for("totally-unknown"), None);
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
