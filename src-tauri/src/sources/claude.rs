// Claude Code JSONL session reader.

use crate::core::context_window_for;
use crate::sources::continue_types::continue_parse_timestamp_ms;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use super::{collect_jsonl, home_dir, recent_cutoff, truncate40, UsageResult};

/// Parsed result for one session file, reused while `(mtime, len)` is unchanged.
/// Session files are re-scanned every 5 s poll; long sessions grow to tens of MB,
/// so skipping the read+parse for unchanged files is a real win in an always-on app.
struct CachedSession {
    mtime: SystemTime,
    len: u64,
    parsed: Option<ParsedSession>,
}

type ParsedSession = (String, f64, String, u64, i64);

static SESSION_CACHE: Mutex<Option<HashMap<PathBuf, CachedSession>>> = Mutex::new(None);

fn parse_session_file(path: &Path) -> Result<Option<ParsedSession>, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let Some((model, fill_pct, tokens, last_active_ms)) = content
        .lines()
        .filter_map(parse_claude_usage_line)
        .next_back()
    else {
        return Ok(None);
    };
    let session = claude_session_title(&content).unwrap_or_else(|| "-".to_string());
    Ok(Some((model, fill_pct, session, tokens, last_active_ms)))
}

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
    let context_window = context_window_for(&model)? as f64;
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
/// Returns (model, fill_pct, session, tokens, last_active_ms, session_id).
/// `session_id` is the session file path — stable for the file's lifetime, unlike
/// the display title which upgrades (first-message → ai-title → custom-title).
pub fn read_claude_jsonl_usage(activity_secs: u64, max_files: usize) -> UsageResult {
    let Some(home) = home_dir() else {
        return Ok(None);
    };
    let projects_dir = home.join(".claude").join("projects");
    if !projects_dir.exists() {
        return Ok(None);
    }
    let cutoff = recent_cutoff(activity_secs);
    let recent: Vec<(SystemTime, PathBuf)> = collect_jsonl(&projects_dir)?
        .into_iter()
        .take(max_files)
        .filter(|(mtime, _)| *mtime >= cutoff)
        .collect();

    let mut guard = SESSION_CACHE
        .lock()
        .map_err(|error| format!("Claude session cache lock failed: {error}"))?;
    let cache = guard.get_or_insert_with(HashMap::new);
    // Drop entries for files that left the recent window so the map stays bounded.
    cache.retain(|path, _| recent.iter().any(|(_, p)| p == path));

    let mut usages = Vec::new();
    for (mtime, path) in &recent {
        let len = fs::metadata(path)
            .map_err(|error| format!("Could not inspect {}: {error}", path.display()))?
            .len();
        let hit = cache
            .get(path)
            .is_some_and(|c| c.mtime == *mtime && c.len == len);
        if !hit {
            cache.insert(
                path.clone(),
                CachedSession {
                    mtime: *mtime,
                    len,
                    parsed: parse_session_file(path)?,
                },
            );
        }
        let Some((model, fill_pct, session, tokens, last_active_ms)) =
            cache.get(path).and_then(|cached| cached.parsed.clone())
        else {
            continue;
        };
        let session_id = path.to_string_lossy().into_owned();
        usages.push((model, fill_pct, session, tokens, last_active_ms, session_id));
    }
    // Recent files with no parseable usage record is a normal state — a session that
    // has only just started, or one on a model with no curve entry. Reporting it as a
    // source error masks the real diagnostic ("Unsupported model curve: …") downstream.
    Ok(usages
        .into_iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)))
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
