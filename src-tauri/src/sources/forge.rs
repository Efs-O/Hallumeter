// Forge VS Code extension — reads ~/.forge/hallumeter-bridge.json written by the
// Forge extension on every postTokenBudget() call. Format:
//   { "model": "gemma4-e4b-it-ud-q4kxl", "used_tokens": 12500,
//     "max_tokens": 98304, "timestamp_ms": 1747000000000 }

use serde::Deserialize;
use std::time::UNIX_EPOCH;

use super::{home_dir, recent_cutoff_ms, UsageResult};

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
/// Returns `(model, fill_pct, session, tokens, last_active_ms, session_id)`.
pub fn read_forge_usage(activity_secs: u64) -> UsageResult {
    let Some(path) = forge_bridge_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let bridge: ForgeBridge = serde_json::from_str(&content)
        .map_err(|error| format!("Invalid Forge bridge {}: {error}", path.display()))?;

    if bridge.max_tokens == 0 {
        return Err(format!(
            "Forge bridge {} has max_tokens = 0",
            path.display()
        ));
    }

    let cutoff_ms = recent_cutoff_ms(activity_secs as i64);
    if bridge.timestamp_ms < cutoff_ms {
        return Ok(None);
    }

    // Sanity-check: file must also be recent on disk (guards against stale writes)
    let modified = std::fs::metadata(&path)
        .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?
        .modified()
        .map_err(|error| {
            format!(
                "Could not read modification time for {}: {error}",
                path.display()
            )
        })?;
    let mtime_ms = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("Invalid modification time for {}: {error}", path.display()))?
        .as_millis() as i64;
    if mtime_ms < cutoff_ms {
        return Ok(None);
    }

    let fill = (bridge.used_tokens as f64 / bridge.max_tokens as f64 * 100.0).clamp(0.0, 100.0);
    let session = format!("Forge · {}", bridge.model);
    // Single bridge file — the model id is the most stable identity available.
    let session_id = format!("forge:{}", bridge.model);
    Ok(Some((
        bridge.model,
        fill,
        session,
        bridge.used_tokens,
        bridge.timestamp_ms,
        session_id,
    )))
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
