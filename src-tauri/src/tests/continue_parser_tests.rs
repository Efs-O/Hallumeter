// HalluMeter

use crate::sources::{
    continue_best_token_match, continue_compute_fill_pct, continue_normalize_model_id,
    continue_parse_chat_event, continue_parse_timestamp_ms, continue_parse_token_event,
    continue_session_title, ContinueChatEvent, ContinueModelConfig, ContinueSessionMeta,
    ContinueTokenEvent,
};
use std::collections::HashMap;

#[test]
fn continue_parses_iso_timestamp_to_ms() {
    assert_eq!(
        continue_parse_timestamp_ms("1970-01-01T00:00:01.234Z"),
        Some(1234)
    );
    assert_eq!(
        continue_parse_timestamp_ms("1970-01-01T02:00:01.234+02:00"),
        Some(1234)
    );
    assert_eq!(
        continue_parse_timestamp_ms("1970-01-01T00:00:01Z"),
        Some(1000)
    );
    assert_eq!(
        continue_parse_timestamp_ms("1970-01-01T00:00:01.1Z"),
        Some(1100)
    );
    assert_eq!(
        continue_parse_timestamp_ms("1970-01-01T00:00:01.000-05:00"),
        Some(18_001_000)
    );
    assert!(continue_parse_timestamp_ms("1972-02-29T00:00:00Z").is_some());
    assert!(continue_parse_timestamp_ms("1970-01-01T00:00:01.000ZEXTRA").is_none());
    assert!(continue_parse_timestamp_ms("1970-01-01").is_none());
}

#[test]
fn continue_parses_chat_interaction_event() {
    let line = r#"{"sessionId":"sess-1","timestamp":"2099-04-07T12:00:01.000Z","modelName":"qwen35-9b-long","modelTitle":"Qwen3.5 9B Long Context"}"#;
    let event = continue_parse_chat_event(line).expect("chat event should parse");

    assert_eq!(event.session_id, "sess-1");
    assert_eq!(event.timestamp_ms, 4_079_246_401_000);
    assert_eq!(event.model_id, "qwen35-9b-long");
    assert_eq!(
        event.model_title.as_deref(),
        Some("Qwen3.5 9B Long Context")
    );
}

#[test]
fn continue_parses_tokens_generated_event() {
    let line =
        r#"{"timestamp":"2099-04-07T12:00:11.000Z","model":"qwen35-9b-long","promptTokens":2048}"#;
    let event = continue_parse_token_event(line).expect("token event should parse");

    assert_eq!(event.timestamp_ms, 4_079_246_411_000);
    assert_eq!(event.model_id, "qwen35-9b-long");
    assert_eq!(event.prompt_tokens, 2048);
}

#[test]
fn continue_normalizes_model_ids_consistently() {
    assert_eq!(continue_normalize_model_id("Llama 3.1 8B"), "llama-3.1-8b");
    assert_eq!(continue_normalize_model_id("llama3.1:8b"), "llama3.1:8b");
    assert_eq!(
        continue_normalize_model_id("Qwen3.5 9B Long Context"),
        "qwen3.5-9b-long-context"
    );
    assert_eq!(
        continue_normalize_model_id("qwen35-9b-long"),
        "qwen35-9b-long"
    );
}

#[test]
fn continue_chooses_nearest_token_event_same_model() {
    let chat = ContinueChatEvent {
        session_id: "sess-1".to_string(),
        timestamp_ms: 100_000,
        model_id: "qwen35-9b-long".to_string(),
        model_title: None,
    };
    let tokens = vec![
        ContinueTokenEvent {
            timestamp_ms: 70_000,
            model_id: "qwen35-9b-long".to_string(),
            prompt_tokens: 1000,
        },
        ContinueTokenEvent {
            timestamp_ms: 110_000,
            model_id: "qwen35-9b-long".to_string(),
            prompt_tokens: 2000,
        },
    ];

    let matched = continue_best_token_match(&chat, &tokens, 120_000).expect("best token match");
    assert_eq!(matched.timestamp_ms, 110_000);
    assert_eq!(matched.prompt_tokens, 2000);
}

#[test]
fn continue_rejects_far_token_event() {
    let chat = ContinueChatEvent {
        session_id: "sess-1".to_string(),
        timestamp_ms: 100_000,
        model_id: "qwen35-9b-long".to_string(),
        model_title: None,
    };
    let tokens = vec![ContinueTokenEvent {
        timestamp_ms: 221_000,
        model_id: "qwen35-9b-long".to_string(),
        prompt_tokens: 2000,
    }];

    assert!(continue_best_token_match(&chat, &tokens, 120_000).is_none());
}

#[test]
fn continue_prefers_same_model_over_closer_wrong_model() {
    let chat = ContinueChatEvent {
        session_id: "sess-1".to_string(),
        timestamp_ms: 100_000,
        model_id: "qwen35-9b-long".to_string(),
        model_title: None,
    };
    let tokens = vec![
        ContinueTokenEvent {
            timestamp_ms: 101_000,
            model_id: "gemma4:26b-it-q4_k_m".to_string(),
            prompt_tokens: 3000,
        },
        ContinueTokenEvent {
            timestamp_ms: 118_000,
            model_id: "qwen35-9b-long".to_string(),
            prompt_tokens: 1500,
        },
    ];

    let matched =
        continue_best_token_match(&chat, &tokens, 120_000).expect("same-model token match");
    assert_eq!(matched.model_id, "qwen35-9b-long");
    assert_eq!(matched.timestamp_ms, 118_000);
}

#[test]
fn continue_ignores_zero_prompt_tokens() {
    let line =
        r#"{"timestamp":"2099-04-07T12:00:11.000Z","model":"qwen35-9b-long","promptTokens":0}"#;
    assert!(continue_parse_token_event(line).is_none());
}

#[test]
fn continue_resolves_session_title_from_sessions_index() {
    let mut index = HashMap::new();
    index.insert(
        "sess-1".to_string(),
        ContinueSessionMeta {
            title: "Continue Session".to_string(),
        },
    );

    assert_eq!(continue_session_title("sess-1", &index), "Continue Session");
}

#[test]
fn continue_computes_fill_pct_from_config_context_length() {
    let mut map = HashMap::new();
    map.insert(
        "qwen35-9b-long".to_string(),
        ContinueModelConfig {
            canonical_model_id: "qwen35-9b-long".to_string(),
            context_window: 32_768,
        },
    );

    let fill_pct =
        continue_compute_fill_pct("qwen35-9b-long", 8192, &map).expect("fill pct should resolve");
    assert!((fill_pct - 25.0).abs() < f64::EPSILON);
}
