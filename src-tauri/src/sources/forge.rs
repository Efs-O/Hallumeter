// Forge VS Code extension — reads ~/.forge/hallumeter-bridge.json written by the
// Forge extension on every postTokenBudget() call. Format:
//   { "model": "gemma4-e4b-it-ud-q4kxl", "used_tokens": 12500,
//     "max_tokens": 98304, "timestamp_ms": 1747000000000 }

use serde::Deserialize;
use std::time::UNIX_EPOCH;

use super::{home_dir, recent_cutoff_ms};

const BRIDGE_FILE: &str = "hallumeter-bridge.json";

#[derive(Deserialize)]
struct ForgeBridge {
    model: String,
    used_tokens: u64,
    max_tokens: u64,
    timestamp_ms: i64,
}

fn forge_bridge_path() -> Option<std::path::PathBuf> {
    Some(home_dir()?.join(".forge").join(BRIDGE_FILE))
}

/// Active Forge VS Code extension session from ~/.forge/hallumeter-bridge.json.
/// Returns `(model, fill_pct, session, tokens, last_active_ms)`.
pub fn read_forge_usage(activity_secs: u64) -> Option<(String, f64, String, u64, i64)> {
    let path = forge_bridge_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let bridge: ForgeBridge = serde_json::from_str(&content).ok()?;

    if bridge.max_tokens == 0 {
        return None;
    }

    let cutoff_ms = recent_cutoff_ms(activity_secs as i64);
    if bridge.timestamp_ms < cutoff_ms {
        return None;
    }

    // Sanity-check: file must also be recent on disk (guards against stale writes)
    let mtime_ms = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if mtime_ms < cutoff_ms {
        return None;
    }

    let fill = (bridge.used_tokens as f64 / bridge.max_tokens as f64 * 100.0).clamp(0.0, 100.0);
    let session = format!("Forge · {}", bridge.model);
    Some((
        bridge.model,
        fill,
        session,
        bridge.used_tokens,
        bridge.timestamp_ms,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bridge_json() {
        let json = r#"{"model":"gemma4-e4b-it-ud-q4kxl","used_tokens":12500,"max_tokens":98304,"timestamp_ms":1747000000000}"#;
        let b: ForgeBridge = serde_json::from_str(json).expect("parses");
        assert_eq!(b.used_tokens, 12500);
        assert_eq!(b.max_tokens, 98304);
        let fill = b.used_tokens as f64 / b.max_tokens as f64 * 100.0;
        assert!((fill - 12.716).abs() < 0.01);
    }
}
