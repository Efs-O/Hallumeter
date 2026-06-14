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
    pub session: String, // project folder name extracted from JSONL path
    pub variant: u8,     // 1–5: which voice line just played; 0: no new line this cycle
    pub tokens: u64,     // raw input token count for the current session
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

/// Looks up model in curves.json and interpolates risk score for given fill %.
/// Falls back to the first model's curve if model not found.
pub fn interpolate_curve(model: &str, fill_pct: f64) -> f64 {
    let curves = load_curves();
    let mc = curves
        .models
        .iter()
        .find(|m| m.id == model)
        .or_else(|| curves.models.first());
    let Some(mc) = mc else {
        return (fill_pct / 100.0).clamp(0.0, 1.0);
    };
    let pts = &mc.degradation_curve;
    if pts.is_empty() {
        return 0.0;
    }
    if fill_pct <= pts[0].fill_pct {
        return pts[0].risk_score;
    }
    let last = pts.last().unwrap();
    if fill_pct >= last.fill_pct {
        return last.risk_score;
    }
    for i in 1..pts.len() {
        let lo = &pts[i - 1];
        let hi = &pts[i];
        if fill_pct <= hi.fill_pct {
            let t = (fill_pct - lo.fill_pct) / (hi.fill_pct - lo.fill_pct);
            return lo.risk_score + t * (hi.risk_score - lo.risk_score);
        }
    }
    last.risk_score
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
