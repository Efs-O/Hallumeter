// Codex JSONL session reader.

use std::collections::HashMap;
use std::fs;
use std::time::UNIX_EPOCH;

use super::{collect_jsonl, home_dir, recent_cutoff, truncate40, UsageResult};

/// Load ~/.codex/session_index.jsonl and return a map of session-id → thread_name.
fn codex_session_index() -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    let Some(path) = home_dir().map(|h| h.join(".codex").join("session_index.jsonl")) else {
        return Ok(map);
    };
    if !path.exists() {
        return Ok(map);
    };
    let content = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let (Some(id), Some(name)) = (
            v.get("id").and_then(|s| s.as_str()),
            v.get("thread_name").and_then(|s| s.as_str()),
        ) {
            map.insert(id.to_string(), name.to_string());
        }
    }
    Ok(map)
}

/// Extract the user's request text from a Codex user_message payload (fallback only).
fn codex_user_request(msg: &str) -> Option<String> {
    const MARKER: &str = "## My request for Codex:\n";
    let text = if let Some(pos) = msg.find(MARKER) {
        &msg[pos + MARKER.len()..]
    } else {
        msg
    };
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    Some(truncate40(text))
}

fn parse_codex_session(
    content: &str,
    index: &HashMap<String, String>,
) -> Option<(String, f64, String, u64)> {
    let mut session_id: Option<String> = None;
    let mut model: Option<String> = None;
    let mut input_tokens: f64 = 0.0;
    let mut context_window: f64 = 0.0;
    let mut first_user_msg: Option<String> = None;

    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("session_meta") if session_id.is_none() => {
                session_id = v
                    .get("payload")
                    .and_then(|p| p.get("id"))
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
            }
            Some("turn_context") => {
                if let Some(m) = v
                    .get("payload")
                    .and_then(|p| p.get("model"))
                    .and_then(|m| m.as_str())
                {
                    // Normalise "gpt-5.4-mini" → "gpt-5-4-mini" to match curves.json keys
                    model = Some(m.replace('.', "-"));
                }
            }
            Some("event_msg") => {
                let Some(payload) = v.get("payload") else {
                    continue;
                };
                match payload.get("type").and_then(|t| t.as_str()) {
                    Some("token_count") => {
                        let Some(info) = payload.get("info") else {
                            continue;
                        };
                        if info.is_null() {
                            continue;
                        }
                        // last_token_usage = current turn's context size (not cumulative)
                        if let Some(t) = info
                            .get("last_token_usage")
                            .and_then(|u| u.get("input_tokens"))
                            .and_then(|t| t.as_f64())
                        {
                            input_tokens = t;
                        }
                        if let Some(cw) = info.get("model_context_window").and_then(|c| c.as_f64())
                        {
                            context_window = cw;
                        }
                    }
                    Some("user_message") if first_user_msg.is_none() => {
                        if let Some(msg) = payload.get("message").and_then(|m| m.as_str()) {
                            first_user_msg = codex_user_request(msg);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    let model = model?;
    if context_window <= 0.0 || input_tokens <= 0.0 {
        return None;
    }
    let fill_pct = (input_tokens / context_window * 100.0).clamp(0.0, 100.0);
    // Prefer session_index thread_name → first user message → fallback
    let session = session_id
        .as_deref()
        .and_then(|id| index.get(id))
        .cloned()
        .or(first_user_msg)
        .unwrap_or_else(|| "Codex".to_string());
    Some((model, fill_pct, session, input_tokens as u64))
}

/// Highest-fill active Codex session among the `max_files` most recently modified
/// files, limited to files touched within the last `activity_secs` seconds.
/// Returns (model, fill_pct, session, tokens, last_active_ms, session_id).
/// `session_id` is the session file path — stable, unlike the display title
/// which can upgrade once the session_index thread_name appears.
pub fn read_codex_jsonl_usage(activity_secs: u64, max_files: usize) -> UsageResult {
    let Some(home) = home_dir() else {
        return Ok(None);
    };
    let sessions_dir = home.join(".codex").join("sessions");
    if !sessions_dir.exists() {
        return Ok(None);
    }
    let cutoff = recent_cutoff(activity_secs);
    let index = codex_session_index()?;
    let recent: Vec<_> = collect_jsonl(&sessions_dir)?
        .iter()
        .take(max_files)
        .filter(|(mtime, _)| *mtime >= cutoff)
        .cloned()
        .collect();
    let mut usages = Vec::new();
    for (mtime, path) in &recent {
        let content = fs::read_to_string(path)
            .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
        let Some((model, fill_pct, session, tokens)) = parse_codex_session(&content, &index) else {
            continue;
        };
        let last_active_ms = mtime
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let session_id = path.to_string_lossy().into_owned();
        usages.push((model, fill_pct, session, tokens, last_active_ms, session_id));
    }
    if recent.is_empty() {
        return Ok(None);
    }
    usages
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(Some)
        .ok_or_else(|| "No recognizable Codex usage record in recent session files".to_string())
}
