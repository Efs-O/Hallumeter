// Continue IDE — shared types, timestamp parser, and model-id normalizer.

use serde::Deserialize;
use std::path::PathBuf;

use super::home_dir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinueChatEvent {
    pub(crate) session_id: String,
    pub(crate) timestamp_ms: i64,
    pub(crate) model_id: String,
    pub(crate) model_title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinueTokenEvent {
    pub(crate) timestamp_ms: i64,
    pub(crate) model_id: String,
    pub(crate) prompt_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinueSessionMeta {
    pub(crate) title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinueModelConfig {
    pub(crate) canonical_model_id: String,
    pub(crate) context_window: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct ContinueSessionsFileEntry {
    #[serde(rename = "sessionId")]
    pub(super) session_id: String,
    pub(super) title: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ContinueConfigFile {
    #[serde(default)]
    pub(super) models: Vec<ContinueConfigModelEntry>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ContinueConfigModelEntry {
    pub(super) name: Option<String>,
    pub(super) model: Option<String>,
    #[serde(rename = "contextLength")]
    pub(super) context_length: Option<u64>,
}

pub(super) fn continue_root() -> Option<PathBuf> {
    home_dir().map(|dir| dir.join(".continue"))
}

/// Parses an ISO 8601 timestamp string to milliseconds since the Unix epoch.
/// Accepts `YYYY-MM-DDTHH:MM:SS[.fraction]Z` or `±HH:MM` UTC offset.
/// Date-to-epoch-day conversion uses Howard Hinnant's civil_from_days algorithm
/// (mathematically proven correct for all valid proleptic Gregorian dates).
/// Returns None for any malformed, truncated, or trailing-garbage input.
pub(crate) fn continue_parse_timestamp_ms(ts: &str) -> Option<i64> {
    fn parse_digits(input: &str, start: usize, len: usize) -> Option<i32> {
        input.get(start..start + len)?.parse::<i32>().ok()
    }

    fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
        let year = year - if month <= 2 { 1 } else { 0 };
        let era = if year >= 0 { year } else { year - 399 } / 400;
        let yoe = year - era * 400;
        let month = month as i32;
        let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day as i32 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        (era as i64) * 146097 + (doe as i64) - 719468
    }

    let ts = ts.trim();
    if ts.len() < 20 {
        return None;
    }

    let year = parse_digits(ts, 0, 4)?;
    if ts.get(4..5)? != "-" {
        return None;
    }
    let month = parse_digits(ts, 5, 2)? as u32;
    if ts.get(7..8)? != "-" {
        return None;
    }
    let day = parse_digits(ts, 8, 2)? as u32;
    if ts.get(10..11)? != "T" {
        return None;
    }
    let hour = parse_digits(ts, 11, 2)? as i64;
    if ts.get(13..14)? != ":" {
        return None;
    }
    let minute = parse_digits(ts, 14, 2)? as i64;
    if ts.get(16..17)? != ":" {
        return None;
    }
    let second = parse_digits(ts, 17, 2)? as i64;

    let mut index = 19;
    let mut millis: i64 = 0;
    if ts.get(index..index + 1) == Some(".") {
        index += 1;
        let frac_start = index;
        while ts
            .as_bytes()
            .get(index)
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            index += 1;
        }
        let frac = ts.get(frac_start..index)?;
        if frac.is_empty() {
            return None;
        }
        let digits = frac.len().min(3);
        millis = frac.get(..digits)?.parse::<i64>().ok()?;
        for _ in digits..3 {
            millis *= 10;
        }
    }

    let offset_ms = match ts.get(index..index + 1)? {
        "Z" => {
            index += 1;
            0
        }
        "+" | "-" => {
            let sign = if ts.get(index..index + 1)? == "+" {
                1
            } else {
                -1
            };
            let offset_hour = parse_digits(ts, index + 1, 2)? as i64;
            if ts.get(index + 3..index + 4)? != ":" {
                return None;
            }
            let offset_minute = parse_digits(ts, index + 4, 2)? as i64;
            index += 6;
            sign * ((offset_hour * 3600) + (offset_minute * 60)) * 1000
        }
        _ => return None,
    };

    if index != ts.len() {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let day_ms = ((hour * 3600) + (minute * 60) + second) * 1000 + millis;
    Some(days * 86_400_000 + day_ms - offset_ms)
}

pub(crate) fn continue_normalize_model_id(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut last_was_dash = false;
    for ch in raw.trim().chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_whitespace() {
            if !last_was_dash && !normalized.is_empty() {
                normalized.push('-');
                last_was_dash = true;
            }
            continue;
        }
        normalized.push(ch);
        last_was_dash = ch == '-';
    }
    normalized.trim_matches('-').to_string()
}
