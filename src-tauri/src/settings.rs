// User-editable runtime settings. Loaded from app_data_dir/settings.json at startup.
// Missing fields fall back to defaults — partial files are always valid.

use crate::core::{AMBER_THRESHOLD, RED_THRESHOLD};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Resolves an explicit bridge path, or the conventional path beneath the platform desktop.
/// An invalid explicit path is an error instead of silently selecting a different source.
pub fn resolve_continue_bridge_yaml_path(
    settings: &UserSettings,
    desktop_dir: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    if let Some(ref s) = settings.continue_bridge_yaml {
        let p = PathBuf::from(s.trim());
        if p.is_file() {
            return Ok(Some(p));
        }
        return Err(format!(
            "Configured Continue bridge file was not found: {}",
            p.display()
        ));
    }
    let Some(desktop_dir) = desktop_dir else {
        return Ok(None);
    };
    let p = desktop_dir
        .join("llamabridge")
        .join("config")
        .join("bridge.yaml");
    if p.is_file() {
        Ok(Some(p))
    } else {
        Ok(None)
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

impl UserSettings {
    fn validate(&self) -> Result<(), String> {
        if self.activity_window_mins == 0 {
            return Err("activity_window_mins must be at least 1".to_string());
        }
        if self.stale_timeout_secs == 0 {
            return Err("stale_timeout_secs must be at least 1".to_string());
        }
        if self.claude_max_files == 0 || self.codex_max_files == 0 || self.copilot_max_files == 0 {
            return Err("all source file limits must be at least 1".to_string());
        }
        if self.continue_correlation_secs == 0 {
            return Err("continue_correlation_secs must be at least 1".to_string());
        }
        if !self.amber_threshold.is_finite()
            || !self.red_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.amber_threshold)
            || !(0.0..=1.0).contains(&self.red_threshold)
            || self.amber_threshold >= self.red_threshold
        {
            return Err("thresholds must satisfy 0.0 ≤ amber < red ≤ 1.0".to_string());
        }
        if !self.context_overhead_pct.is_finite()
            || !(0.0..=50.0).contains(&self.context_overhead_pct)
        {
            return Err("context_overhead_pct must be between 0 and 50".to_string());
        }
        Ok(())
    }
}

/// Load settings from `<app_data_dir>/settings.json`.
/// A missing file intentionally uses defaults; unreadable or malformed files are errors.
pub fn load_settings(app_data_dir: &Path) -> Result<UserSettings, String> {
    let path = app_data_dir.join("settings.json");
    if !path.exists() {
        return Ok(UserSettings::default());
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    if content.trim().is_empty() {
        return Err(format!("Settings file is empty: {}", path.display()));
    };
    let settings: UserSettings = serde_json::from_str(&content)
        .map_err(|error| format!("Invalid settings in {}: {error}", path.display()))?;
    settings
        .validate()
        .map_err(|error| format!("Invalid settings in {}: {error}", path.display()))?;
    Ok(settings)
}

/// Persist settings to `<app_data_dir>/settings.json`.
pub fn save_settings(app_data_dir: &Path, settings: &UserSettings) -> std::io::Result<()> {
    settings.validate().map_err(std::io::Error::other)?;
    std::fs::create_dir_all(app_data_dir)?;
    let path = app_data_dir.join("settings.json");
    let content = serde_json::to_string_pretty(settings).map_err(std::io::Error::other)?;
    std::fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::UserSettings;

    #[test]
    fn rejects_inverted_thresholds() {
        let settings = UserSettings {
            amber_threshold: 0.4,
            red_threshold: 0.2,
            ..UserSettings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn accepts_defaults() {
        assert!(UserSettings::default().validate().is_ok());
    }
}
