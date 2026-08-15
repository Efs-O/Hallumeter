// Note: variant field added for audio/scrollbar sync (Phase 4)
// Pure logic functions — shell parsing, curve interpolation, state mapping.

use serde::Deserialize;
use std::sync::OnceLock;

// Absolute risk score thresholds — model-agnostic, applied to the interpolated risk_score
// emitted by the curve for whichever model is active. Curves do the per-model heavy lifting
// (including per-model baseline knots for Claude Code overhead); these just define what
// "amber" and "red" mean on the 0.0–1.0 risk scale.
// Amber @ 0.15 ≈ Sonnet at ~38% fill / Opus at ~64% fill.
// Red   @ 0.30 ≈ Sonnet at ~75% fill / Opus at ~90% fill.
pub const AMBER_THRESHOLD: f64 = 0.15;
pub const RED_THRESHOLD: f64 = 0.30;

// Emitted to frontend on every poll cycle
#[derive(serde::Serialize, Clone, Debug)]
pub struct ContextPayload {
    pub fill_pct: f64,
    pub risk_score: f64,
    pub state: String,
    pub model: String,
    pub session: String, // display title — can upgrade mid-session (ai-title, custom-title)
    pub session_id: String, // stable identity (file path / session uuid) — one-shot bookkeeping
    pub variant: u8,     // 1–5: which voice line just played; 0: no new line this cycle
    pub tokens: u64,     // raw input token count for the current session
    /// A user-visible explanation for unavailable/degraded monitoring.
    pub diagnostic: Option<String>,
    /// True when `risk_score` came from the generic fallback curve rather than a
    /// curve measured for this model. The UI must mark the reading as approximate.
    pub approximate: bool,
}

// --- Curve data structures ---

#[derive(Deserialize, Debug)]
pub struct CurvePoint {
    pub fill_pct: f64,
    pub risk_score: f64,
}

#[derive(Deserialize, Debug)]
pub struct ModelCurve {
    pub id: String,
    pub context_window: u64,
    pub degradation_curve: Vec<CurvePoint>,
}

#[derive(Deserialize, Debug)]
pub struct CurvesConfig {
    pub models: Vec<ModelCurve>,
    /// Generic curve used when a model id matches nothing in `models`. Deliberately
    /// kept out of `models` so family-prefix matching can never select it by accident,
    /// and so every use of it is explicitly flagged as approximate to the user.
    pub fallback: Option<ModelCurve>,
}

// Embedded at compile time — single source of truth
static CURVES_JSON: &str = include_str!("../assets/curves.json");

// Parsed once on first use, then reused for the process lifetime. The data is
// immutable (compile-time embedded), so re-parsing it on every poll/JSONL line
// is pure waste in an always-on background app.
static CURVES: OnceLock<CurvesConfig> = OnceLock::new();

pub fn load_curves() -> &'static CurvesConfig {
    CURVES.get_or_init(|| serde_json::from_str(CURVES_JSON).expect("curves.json is malformed"))
}

// --- Implementations ---

/// Resolves a model id to its curve: exact match first, then the longest
/// family-prefix match on a `-` boundary, so date-suffixed ids like
/// `claude-haiku-4-5-20251001` hit the `claude-haiku-4-5` curve.
pub fn find_model_curve(model: &str) -> Option<&'static ModelCurve> {
    let curves = load_curves();
    curves.models.iter().find(|m| m.id == model).or_else(|| {
        curves
            .models
            .iter()
            .filter(|m| model.starts_with(&m.id) && model.as_bytes().get(m.id.len()) == Some(&b'-'))
            .max_by_key(|m| m.id.len())
    })
}

/// Context window for a model, honoring family-prefix matching.
pub fn context_window_for(model: &str) -> Option<u64> {
    find_model_curve(model).map(|m| m.context_window)
}

/// Linear interpolation across a curve's knots, clamped at both ends.
fn interpolate_points(pts: &[CurvePoint], fill_pct: f64) -> Option<f64> {
    if pts.is_empty() {
        return None;
    }
    if fill_pct <= pts[0].fill_pct {
        return Some(pts[0].risk_score);
    }
    let last = pts.last().unwrap();
    if fill_pct >= last.fill_pct {
        return Some(last.risk_score);
    }
    for i in 1..pts.len() {
        let lo = &pts[i - 1];
        let hi = &pts[i];
        if fill_pct <= hi.fill_pct {
            let t = (fill_pct - lo.fill_pct) / (hi.fill_pct - lo.fill_pct);
            return Some(lo.risk_score + t * (hi.risk_score - lo.risk_score));
        }
    }
    Some(last.risk_score)
}

/// Looks up model in curves.json and interpolates risk score for given fill %.
/// Exact/family match only — unknown models return `None`. Callers that want the
/// generic fallback must use `estimate_risk`, which reports when it was used.
pub fn interpolate_curve(model: &str, fill_pct: f64) -> Option<f64> {
    interpolate_points(&find_model_curve(model)?.degradation_curve, fill_pct)
}

/// A risk score plus whether it came from the generic fallback curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiskEstimate {
    pub risk_score: f64,
    /// True when no curve matched the model id and the generic fallback was used.
    /// Callers MUST surface this — an unlabelled fallback is a synthetic score,
    /// which is exactly what the 0.1.5 audit set out to eliminate.
    pub approximate: bool,
}

/// Risk for a model, falling back to the generic curve for unknown ids.
/// Returns `None` only when the id is unknown AND no fallback curve is defined.
pub fn estimate_risk(model: &str, fill_pct: f64) -> Option<RiskEstimate> {
    if let Some(risk_score) = interpolate_curve(model, fill_pct) {
        return Some(RiskEstimate {
            risk_score,
            approximate: false,
        });
    }
    let fallback = load_curves().fallback.as_ref()?;
    interpolate_points(&fallback.degradation_curve, fill_pct).map(|risk_score| RiskEstimate {
        risk_score,
        approximate: true,
    })
}

/// Maps a risk score to a state string using the provided thresholds.
pub fn risk_to_state(risk: f64, amber: f64, red: f64) -> &'static str {
    if risk >= red {
        "red"
    } else if risk >= amber {
        "amber"
    } else {
        "green"
    }
}

// JSONL readers live in sources.rs
