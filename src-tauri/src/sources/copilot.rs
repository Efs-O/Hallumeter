// GitHub Copilot CLI — reads session events from ~/.copilot/session-state/.
// Optional: COPILOT_HOME env overrides ~/.copilot.

use crate::core::load_curves;
use serde_json::Value;
use std::cmp::Reverse;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::continue_types::{continue_normalize_model_id, continue_parse_timestamp_ms};
use super::{home_dir, recent_cutoff, truncate40};

fn copilot_session_state_root() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("COPILOT_HOME") {
        let p = PathBuf::from(h.trim());
        if p.is_dir() {
            return Some(p.join("session-state"));
        }
    }
    Some(home_dir()?.join(".copilot").join("session-state"))
}

/// Session dirs under session-state, most recently modified `events.jsonl` first.
fn collect_copilot_session_dirs(session_state: &Path) -> Vec<(SystemTime, PathBuf)> {
    let mut out: Vec<(SystemTime, PathBuf)> = Vec::new();
    let Ok(entries) = fs::read_dir(session_state) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let events = dir.join("events.jsonl");
        if !events.is_file() {
            continue;
        }
        if let Ok(meta) = events.metadata() {
            if let Ok(mtime) = meta.modified() {
                out.push((mtime, dir));
            }
        }
    }
    out.sort_by_key(|(t, _)| Reverse(*t));
    out
}

fn json_u64(obj: &Value, key: &str) -> Option<u64> {
    let v = obj.get(key)?;
    v.as_u64().or_else(|| v.as_f64().map(|x| x as u64))
}

/// Map Copilot model string to curves.json ids (slashes, dots → dashes).
fn copilot_normalize_model_id(raw: &str) -> String {
    let trimmed = raw.trim();
    let after_slash = trimmed.rsplit_once('/').map(|(_, r)| r).unwrap_or(trimmed);
    continue_normalize_model_id(after_slash)
        .chars()
        .map(|ch| if ch == '.' { '-' } else { ch })
        .collect()
}

fn session_label(dir: &Path) -> String {
    let fallback = "-";
    let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or(fallback);
    format!("Copilot · {}", truncate40(name))
}

#[derive(Default)]
struct CopilotScan {
    model: Option<String>,
    current_tokens: Option<u64>,
    token_limit: Option<u64>,
    shutdown_tokens: Option<u64>,
    shutdown_model: Option<String>,
}

fn parse_events_jsonl(content: &str) -> Option<(String, f64, u64)> {
    let mut scan = CopilotScan::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(t) = v.get("type").and_then(|t| t.as_str()) else {
            continue;
        };
        let data = v.get("data").cloned().unwrap_or(Value::Null);

        match t {
            "session.model_change" => {
                if let Some(m) = data.get("newModel").and_then(|x| x.as_str()) {
                    scan.model = Some(m.to_string());
                }
            }
            "session.usage_info" => {
                if let Some(ct) = json_u64(&data, "currentTokens") {
                    scan.current_tokens = Some(ct);
                }
                if let Some(lim) = json_u64(&data, "tokenLimit") {
                    scan.token_limit = Some(lim);
                }
            }
            "session.shutdown" => {
                if let Some(ct) = json_u64(&data, "currentTokens") {
                    scan.shutdown_tokens = Some(ct);
                }
                if let Some(m) = data.get("currentModel").and_then(|x| x.as_str()) {
                    scan.shutdown_model = Some(m.to_string());
                }
            }
            "assistant.usage" => {
                if let (None, Some(m)) = (
                    scan.model.as_ref(),
                    data.get("model").and_then(|x| x.as_str()),
                ) {
                    scan.model = Some(m.to_string());
                }
            }
            _ => {}
        }
    }

    let (tokens, limit_u64, model_raw) =
        if let (Some(ct), Some(lim)) = (scan.current_tokens, scan.token_limit) {
            (ct, lim, scan.model.or(scan.shutdown_model)?)
        } else if let Some(ct) = scan.shutdown_tokens {
            let m_raw = scan.shutdown_model.or(scan.model)?;
            let curves = load_curves();
            let normalized = copilot_normalize_model_id(&m_raw);
            let lim = curves
                .models
                .iter()
                .find(|m| m.id == normalized)
                .map(|m| m.context_window)
                .unwrap_or(128_000);
            (ct, lim, m_raw)
        } else {
            return None;
        };

    if limit_u64 == 0 {
        return None;
    }
    let normalized = copilot_normalize_model_id(&model_raw);
    let fill = (tokens as f64 / limit_u64 as f64 * 100.0).clamp(0.0, 100.0);
    Some((normalized, fill, tokens))
}

fn max_timestamp_ms_in_file(content: &str, file_mtime_ms: i64) -> i64 {
    let mut max_ts = file_mtime_ms;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(ms) = v
            .get("timestamp")
            .and_then(|t| t.as_str())
            .and_then(continue_parse_timestamp_ms)
        else {
            continue;
        };
        max_ts = max_ts.max(ms);
    }
    max_ts
}

/// Active GitHub Copilot CLI sessions under ~/.copilot/session-state.
/// Returns `(model, fill_pct, session, tokens, last_active_ms)`.
pub fn read_copilot_usage(
    activity_secs: u64,
    max_files: usize,
) -> Option<(String, f64, String, u64, i64)> {
    let session_state = copilot_session_state_root()?;
    if !session_state.is_dir() {
        return None;
    }
    let cutoff = recent_cutoff(activity_secs);

    collect_copilot_session_dirs(&session_state)
        .into_iter()
        .take(max_files)
        .filter(|(mtime, _)| *mtime >= cutoff)
        .filter_map(|(mtime, dir)| {
            let path = dir.join("events.jsonl");
            let content = fs::read_to_string(&path).ok()?;
            let (model, fill_pct, tokens) = parse_events_jsonl(&content)?;
            let file_mtime_ms = mtime
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let last_active_ms = max_timestamp_ms_in_file(&content, file_mtime_ms);
            let session = session_label(&dir);
            Some((model, fill_pct, session, tokens, last_active_ms))
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usage_info_line() {
        let jsonl = r#"{"type":"session.model_change","timestamp":"2026-04-01T12:00:00.000Z","id":"a","parentId":null,"data":{"newModel":"gpt-4o"}}
{"type":"session.usage_info","timestamp":"2026-04-01T12:01:00.000Z","id":"b","parentId":"a","ephemeral":true,"data":{"currentTokens":50000,"tokenLimit":128000,"messagesLength":4}}"#;
        let (model, fill, _) = parse_events_jsonl(jsonl).expect("parses");
        assert!(model.contains("gpt-4"));
        let expected = 50_000.0_f64 / 128_000.0 * 100.0;
        assert!((fill - expected).abs() < 0.01);
    }

    #[test]
    fn normalizes_deploy_name_dots() {
        assert_eq!(
            copilot_normalize_model_id("my.gpt-4.1-deployment"),
            "my-gpt-4-1-deployment"
        );
    }
}
