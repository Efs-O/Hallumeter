// User-editable runtime settings. Loaded from app_data_dir/settings.json at startup.
// Missing fields fall back to defaults — partial files are always valid.

use crate::core::{AMBER_THRESHOLD, RED_THRESHOLD};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// If `UserSettings::continue_bridge_yaml` points to an existing file, that path
/// is used. Otherwise, when `Desktop/llamabridge/config/bridge.yaml` exists under
/// `USERPROFILE` / `HOME`, that file is used (for local llamabridge setups).
pub fn resolve_continue_bridge_yaml_path(settings: &UserSettings) -> Option<PathBuf> {
    if let Some(ref s) = settings.continue_bridge_yaml {
        let p = PathBuf::from(s.trim());
        if p.is_file() {
            return Some(p);
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let p = PathBuf::from(home)
        .join("Desktop")
        .join("llamabridge")
        .join("config")
        .join("bridge.yaml");
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserSettings {
    /// How long a session file stays "live" before HalluMeter stops reading it (minutes).
    pub activity_window_mins: u64,
    /// How long after the last valid reading before the ring turns grey (seconds).
    pub stale_timeout_secs: u64,
    /// Max number of Claude Code session files considered per poll cycle.
    pub claude_max_files: usize,
    /// Max number of Codex session files considered per poll cycle.
    pub codex_max_files: usize,
    /// Max number of GitHub Copilot CLI session dirs considered per poll cycle.
    pub copilot_max_files: usize,
    /// Max time gap between a Continue chat event and its matching token event (seconds).
    pub continue_correlation_secs: u64,
    /// Optional path to a llamabridge `bridge.yaml` (or compatible `models: … num_ctx:`).
    /// When the file exists, model context sizes are read from it instead of
    /// `~/.continue/config.yaml`. If unset, HalluMeter also checks
    /// `Desktop/llamabridge/config/bridge.yaml` under the user profile when that file exists.
    pub continue_bridge_yaml: Option<String>,
    /// Risk score at which the ring turns amber (0.0–1.0).
    pub amber_threshold: f64,
    /// Risk score at which the ring turns red (0.0–1.0).
    pub red_threshold: f64,
    /// Extra fill % added to every reading to account for system prompt, tools,
    /// memory, and skills overhead not reflected in JSONL usage fields (0–50).
    pub context_overhead_pct: f64,
    /// Whether the main window should stay above other windows.
    pub always_on_top: bool,
    /// Last user-selected window width in logical pixels.
    pub window_width: Option<u32>,
    /// Last user-selected window height in logical pixels.
    pub window_height: Option<u32>,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            activity_window_mins: 15,
            stale_timeout_secs: 30,
            claude_max_files: 6,
            codex_max_files: 10,
            copilot_max_files: 10,
            continue_correlation_secs: 120,
            continue_bridge_yaml: None,
            amber_threshold: AMBER_THRESHOLD,
            red_threshold: RED_THRESHOLD,
            context_overhead_pct: 5.0,
            always_on_top: true,
            window_width: None,
            window_height: None,
        }
    }
}

/// Load settings from `<app_data_dir>/settings.json`.
/// Returns defaults if the file is absent, empty, or malformed.
pub fn load_settings(app_data_dir: &Path) -> UserSettings {
    let path = app_data_dir.join("settings.json");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return UserSettings::default();
    };
    serde_json::from_str(&content).unwrap_or_default()
}

/// Persist settings to `<app_data_dir>/settings.json`.
pub fn save_settings(app_data_dir: &Path, settings: &UserSettings) -> std::io::Result<()> {
    std::fs::create_dir_all(app_data_dir)?;
    let path = app_data_dir.join("settings.json");
    let content = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(path, content)
}
