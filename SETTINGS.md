# HalluMeter — User Settings

HalluMeter can be customised by editing a plain JSON file in a text editor. The app reads it once at startup — restart HalluMeter after saving changes.

---

## Settings file location

| Platform | Path |
|---|---|
| Windows | `%APPDATA%\dev.hallumeter.app\settings.json` |
| macOS | `~/Library/Application Support/dev.hallumeter.app/settings.json` |
| Linux | `~/.local/share/dev.hallumeter.app/settings.json` |

If the file does not exist, HalluMeter uses the defaults shown below. You can create the file at any time — missing fields fall back to defaults, so partial files are always valid.

---

## All settings with defaults

```json
{
  "activity_window_mins": 30,
  "stale_timeout_secs": 30,
  "claude_max_files": 6,
  "codex_max_files": 10,
  "continue_correlation_secs": 120,
  "amber_threshold": 0.20,
  "red_threshold": 0.38
}
```

---

## Field reference

### `activity_window_mins` — default `15`

How long a session file is considered "live" after it was last written to (in minutes).

If a session file has not been updated within this window, HalluMeter stops reading it. This prevents a finished or closed session from keeping the ring coloured indefinitely.

- Raise this if HalluMeter goes grey too quickly during long pauses between prompts.
- Lower this if you switch tools often and want stale sessions to clear faster.

---

### `stale_timeout_secs` — default `30`

How many consecutive seconds with no valid session data before the ring turns grey (in seconds).

Once all session files fall outside the `activity_window_mins`, HalluMeter waits this long before switching to the grey/stale state. This gives the poll cycle a few ticks to confirm the session is truly gone before changing the display.

- Most users will not need to change this.

---

### `claude_max_files` — default `6`

Maximum number of Claude Code session files considered per poll cycle.

HalluMeter picks the most recently modified files up to this limit, then reads the one with the highest context fill. Raising this is useful if you run many parallel Claude Code sessions simultaneously.

---

### `codex_max_files` — default `10`

Maximum number of Codex session files considered per poll cycle. Same logic as `claude_max_files`.

---

### `continue_correlation_secs` — default `120`

Maximum time gap (in seconds) between a Continue chat event and its matching token count event for them to be considered the same turn.

Continue stores chat interactions and token counts in separate files. HalluMeter correlates them by timestamp and model ID. If the two events are further apart than this window, the turn is skipped.

- Raise this if your local LLM is slow to respond and HalluMeter frequently misses readings.
- Lower this if you run multiple models and are seeing cross-session false matches.

---

### `amber_threshold` — default `0.20`

Risk score (0.0–1.0) at which the ring turns **amber**.

The risk score is calculated from HalluMeter's degradation curves — it is not the same as fill percentage. At the default of `0.20`, amber triggers at roughly 40–45% context fill on Claude Sonnet 4.6.

- Lower this (e.g. `0.15`) for an earlier warning.
- Raise this (e.g. `0.28`) if amber fires too often for your workflow.
- Must be less than `red_threshold`.

---

### `red_threshold` — default `0.38`

Risk score (0.0–1.0) at which the ring turns **red**.

At the default of `0.38`, red triggers at roughly 75–80% context fill on Claude Sonnet 4.6.

- Lower this (e.g. `0.30`) for an earlier red alert.
- Raise this (e.g. `0.50`) to reserve red for truly critical fill levels.
- Must be greater than `amber_threshold`.

---

## Example: earlier warnings, longer activity window

```json
{
  "activity_window_mins": 60,
  "amber_threshold": 0.15,
  "red_threshold": 0.30
}
```

Fields not listed here stay at their defaults.

---

## Example: slow local LLM via Continue

```json
{
  "continue_correlation_secs": 300
}
```

---

## Notes

- JSON does not support comments — remove any before saving.
- The file is only read at startup. Restart HalluMeter after editing.
- If the file contains invalid JSON, HalluMeter silently falls back to all defaults.
- `amber_threshold` and `red_threshold` are risk scores, not fill percentages. The relationship between fill % and risk depends on the model curve — see [RESEARCH.md](RESEARCH.md).
