// HalluMeter

use crate::sources::read_continue_usage_from_root;

use super::continue_fixtures::{
    make_continue_fixture, remove_dir_if_exists, write_continue_fixture, ContinueFixture,
};

#[test]
fn continue_returns_none_without_context_length() {
    let root = make_continue_fixture("continue-none-context");
    write_continue_fixture(
        &root,
        ContinueFixture {
            session_id: "sess-1",
            session_title: "Missing Context Window",
            chat_timestamp: "2099-04-07T12:00:00.000Z",
            chat_model_name: "qwen35-9b-long",
            chat_model_title: Some("Qwen3.5 9B Long Context"),
            token_timestamp: "2099-04-07T12:00:10.000Z",
            token_model: "qwen35-9b-long",
            prompt_tokens: 4096,
            config_name: "Qwen3.5 9B Long Context",
            config_model: Some("qwen35-9b-long"),
            context_length: None,
        },
    );

    assert!(read_continue_usage_from_root(&root, 1800, 120_000, None).is_none());
    remove_dir_if_exists(&root);
}

#[test]
fn continue_end_to_end_reader_returns_expected_tuple() {
    let root = make_continue_fixture("continue-e2e-expected");
    write_continue_fixture(
        &root,
        ContinueFixture {
            session_id: "sess-qwen",
            session_title: "Continue Qwen Session",
            chat_timestamp: "2099-04-07T12:00:00.000Z",
            chat_model_name: "qwen35-9b-long",
            chat_model_title: Some("Qwen3.5 9B Long Context"),
            token_timestamp: "2099-04-07T12:00:12.000Z",
            token_model: "qwen35-9b-long",
            prompt_tokens: 4096,
            config_name: "Qwen3.5 9B Long Context",
            config_model: Some("qwen35-9b-long"),
            context_length: Some(32_768),
        },
    );

    let usage = read_continue_usage_from_root(&root, 1800, 120_000, None)
        .expect("continue usage should resolve");
    assert_eq!(usage.0, "qwen35-9b-long");
    assert!((usage.1 - 12.5).abs() < f64::EPSILON);
    assert_eq!(usage.2, "Continue Qwen Session");
    assert_eq!(usage.3, 4096);
    remove_dir_if_exists(&root);
}

#[test]
fn continue_end_to_end_reader_returns_none_on_untrusted_match() {
    let root = make_continue_fixture("continue-e2e-untrusted");
    write_continue_fixture(
        &root,
        ContinueFixture {
            session_id: "sess-qwen",
            session_title: "Untrusted Qwen Session",
            chat_timestamp: "2099-04-07T12:00:00.000Z",
            chat_model_name: "qwen35-9b-long",
            chat_model_title: Some("Qwen3.5 9B Long Context"),
            token_timestamp: "2099-04-07T12:03:01.000Z",
            token_model: "qwen35-9b-long",
            prompt_tokens: 4096,
            config_name: "Qwen3.5 9B Long Context",
            config_model: Some("qwen35-9b-long"),
            context_length: Some(32_768),
        },
    );

    assert!(read_continue_usage_from_root(&root, 1800, 120_000, None).is_none());
    remove_dir_if_exists(&root);
}

#[test]
fn continue_end_to_end_qwen_long_context_fixture_returns_expected_tuple() {
    let root = make_continue_fixture("continue-e2e-qwen");
    write_continue_fixture(
        &root,
        ContinueFixture {
            session_id: "sess-qwen",
            session_title: "Qwen Long Context Session",
            chat_timestamp: "2099-04-07T12:10:00.000Z",
            chat_model_name: "qwen35-9b-long",
            chat_model_title: Some("Qwen3.5 9B Long Context"),
            token_timestamp: "2099-04-07T12:10:20.000Z",
            token_model: "qwen35-9b-long",
            prompt_tokens: 8192,
            config_name: "Qwen3.5 9B Long Context",
            config_model: Some("qwen35-9b-long"),
            context_length: Some(65_536),
        },
    );

    let usage = read_continue_usage_from_root(&root, 1800, 120_000, None)
        .expect("qwen fixture should resolve");
    assert_eq!(usage.0, "qwen35-9b-long");
    assert!((usage.1 - 12.5).abs() < f64::EPSILON);
    assert_eq!(usage.2, "Qwen Long Context Session");
    assert_eq!(usage.3, 8192);
    remove_dir_if_exists(&root);
}

#[test]
fn continue_end_to_end_gemma4_fixture_returns_expected_tuple() {
    let root = make_continue_fixture("continue-e2e-gemma");
    write_continue_fixture(
        &root,
        ContinueFixture {
            session_id: "sess-gemma",
            session_title: "Gemma GGUF Session",
            chat_timestamp: "2099-04-07T13:00:00.000Z",
            chat_model_name: "gemma4:26b-it-q4_k_m",
            chat_model_title: Some("Gemma 4 26B A4B IT (llama-server, direct GGUF)"),
            token_timestamp: "2099-04-07T13:00:18.000Z",
            token_model: "gemma4:26b-it-q4_k_m",
            prompt_tokens: 12288,
            config_name: "Gemma 4 26B A4B IT (llama-server, direct GGUF)",
            config_model: Some("gemma4:26b-it-q4_k_m"),
            context_length: Some(49_152),
        },
    );

    let usage = read_continue_usage_from_root(&root, 1800, 120_000, None)
        .expect("gemma fixture should resolve");
    assert_eq!(usage.0, "gemma4:26b-it-q4_k_m");
    assert!((usage.1 - 25.0).abs() < f64::EPSILON);
    assert_eq!(usage.2, "Gemma GGUF Session");
    assert_eq!(usage.3, 12288);
    remove_dir_if_exists(&root);
}

#[test]
fn continue_end_to_end_gemma4_display_name_chat_matches_slug_token() {
    let root = make_continue_fixture("continue-e2e-gemma-display-name");
    write_continue_fixture(
        &root,
        ContinueFixture {
            session_id: "sess-gemma",
            session_title: "Gemma GGUF Session",
            chat_timestamp: "2099-04-07T13:00:00.000Z",
            chat_model_name: "Gemma 4 26B A4B IT (llama-server, direct GGUF)",
            chat_model_title: Some("Gemma 4 26B A4B IT (llama-server, direct GGUF)"),
            token_timestamp: "2099-04-07T13:00:18.000Z",
            token_model: "gemma4-26b-a4b-it-q3km",
            prompt_tokens: 23902,
            config_name: "Gemma 4 26B A4B IT (llama-server, direct GGUF)",
            config_model: Some("gemma4-26b-a4b-it-q3km"),
            context_length: Some(64_000),
        },
    );

    let usage = read_continue_usage_from_root(&root, 1800, 120_000, None)
        .expect("gemma display-name fixture should resolve");
    assert_eq!(usage.0, "gemma4-26b-a4b-it-q3km");
    assert!((usage.1 - 37.346875).abs() < 1e-9);
    assert_eq!(usage.2, "Gemma GGUF Session");
    assert_eq!(usage.3, 23902);
    remove_dir_if_exists(&root);
}
