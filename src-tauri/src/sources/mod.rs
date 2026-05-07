// JSONL session readers for Claude Code, Codex, Forge (Copilot CLI), and Continue.
// Each returns (model_id, fill_pct, session_title, tokens, last_active_ms) or None.

use std::cmp::Reverse;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod claude;
mod codex;
mod continue_bridge_yaml;
mod continue_reader;
mod continue_types;
mod forge;

// Public API used by lib.rs
pub use claude::read_claude_jsonl_usage;
pub use codex::read_codex_jsonl_usage;
pub use continue_reader::read_continue_usage;
pub use forge::read_forge_usage;

// Items used by tests.rs (cfg(test) gate suppresses unused-import warnings in non-test builds)
#[cfg(test)]
pub(crate) use continue_reader::{
    continue_best_token_match, continue_compute_fill_pct, continue_parse_chat_event,
    continue_parse_token_event, continue_session_title, read_continue_usage_from_root,
};
#[cfg(test)]
pub(crate) use continue_types::{
    continue_normalize_model_id, continue_parse_timestamp_ms, ContinueChatEvent,
    ContinueModelConfig, ContinueSessionMeta, ContinueTokenEvent,
};

// pub(super) helpers shared by submodules
pub(super) fn home_dir() -> Option<PathBuf> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()
        .map(PathBuf::from)
}

pub(super) fn recent_cutoff(duration_secs: u64) -> SystemTime {
    SystemTime::now()
        .checked_sub(Duration::from_secs(duration_secs))
        .unwrap_or(UNIX_EPOCH)
}

pub(crate) fn recent_cutoff_ms(duration_secs: i64) -> i64 {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    now_ms.saturating_sub(duration_secs.saturating_mul(1000))
}

pub(super) fn collect_jsonl(root: &PathBuf) -> Vec<(SystemTime, PathBuf)> {
    let mut files: Vec<(SystemTime, PathBuf)> = Vec::new();
    collect_jsonl_recursive(root, &mut files, 4);
    files.sort_by_key(|entry| Reverse(entry.0));
    files
}

fn collect_jsonl_recursive(dir: &PathBuf, out: &mut Vec<(SystemTime, PathBuf)>, depth: u8) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_recursive(&path, out, depth - 1);
        } else if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    out.push((modified, path));
                }
            }
        }
    }
}

pub(super) fn truncate40(s: &str) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(40).collect();
    if chars.next().is_some() {
        format!("{truncated}...") // ASCII ellipsis — avoids rendering issues on some platforms
    } else {
        truncated
    }
}
