// Continue IDE — session index, model config, event parsing, and public reader.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::continue_types::{
    continue_normalize_model_id, continue_parse_timestamp_ms, continue_root, ContinueChatEvent,
    ContinueConfigFile, ContinueModelConfig, ContinueSessionMeta, ContinueSessionsFileEntry,
    ContinueTokenEvent,
};
use super::recent_cutoff_ms;

fn continue_sessions_index_from_path(path: &Path) -> HashMap<String, ContinueSessionMeta> {
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string(path) else {
        return map;
    };
    if let Ok(entries) = serde_json::from_str::<Vec<ContinueSessionsFileEntry>>(&content) {
        for ContinueSessionsFileEntry { session_id, title } in entries {
            if !session_id.trim().is_empty() && !title.trim().is_empty() {
                map.insert(session_id, ContinueSessionMeta { title });
            }
        }
        return map;
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return map;
    };
    let Some(entries) = value
        .get("sessions")
        .and_then(|sessions| sessions.as_array())
    else {
        return map;
    };
    for entry in entries {
        let Some(session_id) = entry.get("sessionId").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(title) = entry.get("title").and_then(|value| value.as_str()) else {
            continue;
        };
        if !session_id.trim().is_empty() && !title.trim().is_empty() {
            map.insert(
                session_id.to_string(),
                ContinueSessionMeta {
                    title: title.to_string(),
                },
            );
        }
    }
    map
}

fn continue_sessions_index(root: &Path) -> HashMap<String, ContinueSessionMeta> {
    continue_sessions_index_from_path(&root.join("sessions").join("sessions.json"))
}

fn continue_model_config_map_from_path(path: &Path) -> HashMap<String, ContinueModelConfig> {
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string(path) else {
        return map;
    };
    let Ok(config) = serde_yaml::from_str::<ContinueConfigFile>(&content) else {
        return map;
    };

    for entry in config.models {
        let Some(context_window) = entry.context_length else {
            continue;
        };
        if context_window == 0 {
            continue;
        }

        let normalized_model = entry
            .model
            .as_deref()
            .map(continue_normalize_model_id)
            .filter(|value| !value.is_empty());
        let normalized_name = entry
            .name
            .as_deref()
            .map(continue_normalize_model_id)
            .filter(|value| !value.is_empty());
        let canonical_model_id = normalized_model
            .clone()
            .or_else(|| normalized_name.clone())
            .unwrap_or_default();
        if canonical_model_id.is_empty() {
            continue;
        }

        let config = ContinueModelConfig {
            canonical_model_id: canonical_model_id.clone(),
            context_window,
        };

        if let Some(key) = normalized_model {
            map.insert(key, config.clone());
        }
        if let Some(key) = normalized_name {
            map.entry(key).or_insert_with(|| config.clone());
        }
    }

    map
}

fn continue_model_config_map(root: &Path) -> HashMap<String, ContinueModelConfig> {
    continue_model_config_map_from_path(&root.join("config.yaml"))
}

pub(crate) fn continue_parse_chat_event(line: &str) -> Option<ContinueChatEvent> {
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    let session_id = value
        .get("sessionId")
        .and_then(|value| value.as_str())
        .map(str::trim)?
        .to_string();
    let model_name = value
        .get("modelName")
        .and_then(|value| value.as_str())
        .map(str::trim)?;
    if session_id.is_empty() || model_name.is_empty() {
        return None;
    }
    let timestamp_ms = continue_parse_timestamp_ms(value.get("timestamp")?.as_str()?)?;
    let model_id = continue_normalize_model_id(model_name);
    if model_id.is_empty() {
        return None;
    }
    let model_title = value
        .get("modelTitle")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    Some(ContinueChatEvent {
        session_id,
        timestamp_ms,
        model_id,
        model_title,
    })
}

pub(crate) fn continue_parse_token_event(line: &str) -> Option<ContinueTokenEvent> {
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    let timestamp_ms = continue_parse_timestamp_ms(value.get("timestamp")?.as_str()?)?;
    let model_id = continue_normalize_model_id(value.get("model")?.as_str()?);
    let prompt_tokens = value.get("promptTokens")?.as_u64()?;
    if model_id.is_empty() || prompt_tokens == 0 {
        return None;
    }

    Some(ContinueTokenEvent {
        timestamp_ms,
        model_id,
        prompt_tokens,
    })
}

fn continue_recent_chat_events(path: &Path, cutoff_ms: i64) -> Vec<ContinueChatEvent> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(continue_parse_chat_event)
        .filter(|event| event.timestamp_ms >= cutoff_ms)
        .collect()
}

fn continue_recent_token_events(path: &Path, cutoff_ms: i64) -> Vec<ContinueTokenEvent> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(continue_parse_token_event)
        .filter(|event| event.timestamp_ms >= cutoff_ms)
        .collect()
}

pub(crate) fn continue_best_token_match(
    chat: &ContinueChatEvent,
    tokens: &[ContinueTokenEvent],
    correlation_ms: i64,
) -> Option<ContinueTokenEvent> {
    // Prefer the most recent token event that:
    //   (a) matches the model, and
    //   (b) is within correlation_ms of the chat event (before or after).
    // "Most recent" wins over "closest" — ensures subsequent prompts update the fill %.
    tokens
        .iter()
        .filter(|token| {
            token.model_id == chat.model_id
                && token.prompt_tokens > 0
                && (token.timestamp_ms - chat.timestamp_ms).abs() <= correlation_ms
        })
        .max_by_key(|token| token.timestamp_ms)
        .cloned()
}

pub(crate) fn continue_compute_fill_pct(
    model_id: &str,
    tokens: u64,
    model_map: &HashMap<String, ContinueModelConfig>,
) -> Option<f64> {
    let context_window = model_map.get(model_id)?.context_window as f64;
    Some((tokens as f64 / context_window * 100.0).clamp(0.0, 100.0))
}

fn continue_canonical_model_id(
    model_id: &str,
    model_title: Option<&str>,
    model_map: &HashMap<String, ContinueModelConfig>,
) -> String {
    model_map
        .get(model_id)
        .or_else(|| {
            model_title.and_then(|title| model_map.get(&continue_normalize_model_id(title)))
        })
        .map(|config| config.canonical_model_id.clone())
        .unwrap_or_else(|| model_id.to_string())
}

pub(crate) fn continue_session_title(
    session_id: &str,
    index: &HashMap<String, ContinueSessionMeta>,
) -> String {
    index
        .get(session_id)
        .map(|meta| meta.title.clone())
        .unwrap_or_else(|| "Continue".to_string())
}

pub(crate) fn read_continue_usage_from_root(
    root: &Path,
    activity_secs: u64,
    correlation_ms: i64,
) -> Option<(String, f64, String, u64, i64)> {
    let chat_path = root
        .join("dev_data")
        .join("0.2.0")
        .join("chatInteraction.jsonl");
    let tokens_path = root
        .join("dev_data")
        .join("0.2.0")
        .join("tokensGenerated.jsonl");
    let cutoff_ms = recent_cutoff_ms(activity_secs as i64);

    let session_index = continue_sessions_index(root);
    let model_map = continue_model_config_map(root);
    if model_map.is_empty() {
        return None;
    }

    let chat = continue_recent_chat_events(&chat_path, cutoff_ms)
        .into_iter()
        .max_by_key(|event| event.timestamp_ms)?;
    let canonical_chat_model =
        continue_canonical_model_id(&chat.model_id, chat.model_title.as_deref(), &model_map);
    let canonical_chat = ContinueChatEvent {
        model_id: canonical_chat_model.clone(),
        ..chat.clone()
    };
    let tokens = continue_recent_token_events(&tokens_path, cutoff_ms)
        .into_iter()
        .map(|token| ContinueTokenEvent {
            model_id: continue_canonical_model_id(&token.model_id, None, &model_map),
            ..token
        })
        .collect::<Vec<_>>();
    let matched = continue_best_token_match(&canonical_chat, &tokens, correlation_ms)?;
    let fill_pct =
        continue_compute_fill_pct(&canonical_chat.model_id, matched.prompt_tokens, &model_map)?;
    let model = canonical_chat_model;
    let session = continue_session_title(&chat.session_id, &session_index);

    Some((
        model,
        fill_pct,
        session,
        matched.prompt_tokens,
        chat.timestamp_ms,
    ))
}

pub fn read_continue_usage(
    activity_secs: u64,
    correlation_ms: i64,
) -> Option<(String, f64, String, u64, i64)> {
    let root = continue_root()?;
    read_continue_usage_from_root(&root, activity_secs, correlation_ms)
}
