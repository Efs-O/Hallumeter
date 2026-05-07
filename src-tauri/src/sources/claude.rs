// Claude Code JSONL session reader.

use crate::core::load_curves;
use crate::sources::continue_types::continue_parse_timestamp_ms;
use std::fs;

use super::{collect_jsonl, home_dir, recent_cutoff, truncate40};

/// Session title from Claude Code JSONL.
/// Priority: custom-title → ai-title → raw first user message.
fn claude_session_title(content: &str) -> Option<String> {
    let mut custom_title: Option<String> = None;
    let mut ai_title: Option<String> = None;
    let mut first_message: Option<String> = None;

    for line in content.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("custom-title") => {
                if let Some(t) = v.get("customTitle").and_then(|t| t.as_str()) {
                    custom_title = Some(t.to_string());
                }
            }
            Some("ai-title") => {
                if let Some(t) = v.get("aiTitle").and_then(|t| t.as_str()) {
                    ai_title = Some(t.to_string());
                }
            }
            _ => {
                if first_message.is_none() {
                    let Some(msg) = v.get("message") else {
                        continue;
                    };
                    if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
                        continue;
                    }
                    let Some(items) = msg.get("content").and_then(|c| c.as_array()) else {
                        continue;
                    };
                    if let Some(text) = items
                        .iter()
                        .rev()
                        .filter_map(|item| {
                            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                                item.get("text").and_then(|t| t.as_str())
                            } else {
                                None
                            }
                        })
                        .find(|t| {
                            let trimmed = t.trim();
                            !trimmed.is_empty() && !trimmed.starts_with('<')
                        })
                    {
                        first_message = Some(truncate40(text));
                    }
                }
            }
        }
    }
    custom_title.or(ai_title).or(first_message)
}

fn parse_claude_usage_line(line: &str) -> Option<(String, f64, u64, i64)> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let msg = v.get("message")?;
    let model = msg.get("model")?.as_str()?.to_string();
    if model.is_empty() {
        return None;
    }
    let usage = msg.get("usage")?;
    let input = usage
        .get("input_tokens")
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);
    let cache_read = usage
        .get("cache_read_input_tokens")
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);
    let cache_write = usage
        .get("cache_creation_input_tokens")
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);
    let output = usage
        .get("output_tokens")
        .and_then(|t| t.as_f64())
        .unwrap_or(0.0);
    let total = input + cache_read + cache_write + output;
    let curves = load_curves();
    let context_window = curves
        .models
        .iter()
        .find(|m| m.id == model)
        .map(|m| m.context_window as f64)
        .unwrap_or(200_000.0);
    // Timestamp from the line itself — more reliable than file mtime.
    let ts_ms = v
        .get("timestamp")
        .and_then(|t| t.as_str())
        .and_then(continue_parse_timestamp_ms)
        .unwrap_or(0);
    Some((
        model,
        (total / context_window * 100.0).clamp(0.0, 100.0),
        total as u64,
        ts_ms,
    ))
}

/// Highest-fill active Claude Code session among the `max_files` most recently
/// modified files, limited to files touched within the last `activity_secs` seconds.
/// Returns (model, fill_pct, session, tokens, last_active_ms).
pub fn read_claude_jsonl_usage(
    activity_secs: u64,
    max_files: usize,
) -> Option<(String, f64, String, u64, i64)> {
    let projects_dir = home_dir()?.join(".claude").join("projects");
    let cutoff = recent_cutoff(activity_secs);
    collect_jsonl(&projects_dir)
        .iter()
        .take(max_files)
        .filter(|(mtime, _)| *mtime >= cutoff)
        .filter_map(|(_, path)| {
            let content = fs::read_to_string(path).ok()?;
            let (model, fill_pct, tokens, last_active_ms) = content
                .lines()
                .filter_map(parse_claude_usage_line)
                .next_back()?;
            let session = claude_session_title(&content).unwrap_or_else(|| "-".to_string());
            Some((model, fill_pct, session, tokens, last_active_ms))
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate40_short() {
        let s = "hello";
        assert_eq!(truncate40(s), "hello");
    }

    #[test]
    fn truncate40_long() {
        let s = "a".repeat(50);
        let result = truncate40(&s);
        assert!(result.ends_with("..."));
        assert!(result.len() <= 43);
    }
}
