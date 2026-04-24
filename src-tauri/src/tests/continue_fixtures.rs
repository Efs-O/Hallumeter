// HalluMeter

use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) struct ContinueFixture<'a> {
    pub(super) session_id: &'a str,
    pub(super) session_title: &'a str,
    pub(super) chat_timestamp: &'a str,
    pub(super) chat_model_name: &'a str,
    pub(super) chat_model_title: Option<&'a str>,
    pub(super) token_timestamp: &'a str,
    pub(super) token_model: &'a str,
    pub(super) prompt_tokens: u64,
    pub(super) config_name: &'a str,
    pub(super) config_model: Option<&'a str>,
    pub(super) context_length: Option<u64>,
}

pub(super) fn make_continue_fixture(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("hallumeter-{label}-{unique}"));
    remove_dir_if_exists(&root);
    fs::create_dir_all(root.join("sessions")).expect("sessions dir");
    fs::create_dir_all(root.join("dev_data").join("0.2.0")).expect("dev_data dir");
    root
}

pub(super) fn write_continue_fixture(root: &Path, fixture: ContinueFixture<'_>) {
    let sessions_json = json!([{
        "sessionId": fixture.session_id,
        "title": fixture.session_title
    }])
    .to_string();
    let mut chat_obj = json!({
        "sessionId": fixture.session_id,
        "timestamp": fixture.chat_timestamp,
        "modelName": fixture.chat_model_name,
    });
    if let Some(model_title) = fixture.chat_model_title {
        chat_obj["modelTitle"] = json!(model_title);
    }
    let chat_jsonl = chat_obj.to_string();
    let tokens_jsonl = json!({
        "timestamp": fixture.token_timestamp,
        "model": fixture.token_model,
        "promptTokens": fixture.prompt_tokens
    })
    .to_string();
    let config_yaml = match (fixture.config_model, fixture.context_length) {
        (Some(config_model), Some(context_length)) => format!(
            "models:\n  - name: \"{}\"\n    model: \"{}\"\n    contextLength: {}\n",
            fixture.config_name, config_model, context_length
        ),
        (None, Some(context_length)) => format!(
            "models:\n  - name: \"{}\"\n    contextLength: {}\n",
            fixture.config_name, context_length
        ),
        (Some(config_model), None) => format!(
            "models:\n  - name: \"{}\"\n    model: \"{}\"\n",
            fixture.config_name, config_model
        ),
        (None, None) => format!("models:\n  - name: \"{}\"\n", fixture.config_name),
    };

    fs::write(root.join("sessions").join("sessions.json"), sessions_json).expect("sessions.json");
    fs::write(
        root.join("dev_data")
            .join("0.2.0")
            .join("chatInteraction.jsonl"),
        format!("{chat_jsonl}\n"),
    )
    .expect("chatInteraction.jsonl");
    fs::write(
        root.join("dev_data")
            .join("0.2.0")
            .join("tokensGenerated.jsonl"),
        format!("{tokens_jsonl}\n"),
    )
    .expect("tokensGenerated.jsonl");
    fs::write(root.join("config.yaml"), config_yaml).expect("config.yaml");
}

pub(super) fn remove_dir_if_exists(path: &Path) {
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
}
