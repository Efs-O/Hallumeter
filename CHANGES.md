# HalluMeter — Recent Changes

## 0.1.8 — Context menu stays inside the window

- **Right-click menu no longer opens outside the window.** `.ctx-menu` was positioned at the
  raw click coordinates with no clamping, so right-clicking the hide dash (`right: 7px`) laid
  the ~110 px menu out past the right edge of the borderless window, where `overflow: hidden`
  clips it away. The menu position is now clamped into the viewport with a 4 px margin, using
  the menu's measured size (`bind:clientWidth/clientHeight`, seeded with its design size so
  the first frame is already correct). Also fixes the same overflow at the bottom edge near
  the bottom bar and the left edge near the diagnostic `!` button. (`App.svelte`)

---

## 0.1.5 — Curve refresh + audit round 2

Fixes from the follow-up audit (see `AUDIT_REPORT_0.1.4.md` / `IMPLEMENTATION_PLAN_0.1.5.md`):

- **curves.json knows the Claude 5 generation.** Added `claude-sonnet-5`, `claude-opus-4-8`,
  `claude-fable-5`, and `claude-haiku-4-5` (curves cloned from the nearest measured family
  pending benchmark data). New family-prefix matching means date-suffixed ids like
  `claude-haiku-4-5-20251001` resolve to their family curve instead of the generic fallback,
  and unknown model ids are now logged once instead of failing silently. (`core.rs`, `curves.json`)
- **Real WiX `upgradeCode` GUID** replaces the tutorial placeholder. ⚠️ MSI installs of
  ≤0.1.4 will not auto-upgrade — uninstall once, then install 0.1.5.
- **Panic Easter egg can no longer re-fire mid-session.** The one-shot now keys on a stable
  `session_id` (file path / session uuid) carried through every reader and the payload; the
  display title still upgrades (first-message → ai-title → custom-title) without side effects.
- **Claude session files are cached per `(mtime, len)`** — unchanged files skip the full
  read + double JSON parse every 5 s poll. (`claude.rs`)
- Doc drift: panic-threshold comment (99 % → 95 %), `sources/mod.rs` header now lists all
  five sources correctly.

---

## 0.1.4 — Audit fixes

Fixes from a full-repo audit (see `AUDIT_REPORT.md`):

- **Panic Easter egg no longer plays `panic.mp3` twice.** The backend poll loop and the
  frontend both fired panic audio at ≥95 % fill, causing overlapping playback. Audio + the
  cut-to-black visual are now owned solely by the frontend (`play_panic_audio`); the backend
  only tracks the one-shot to suppress the normal state cue. (`lib.rs`, `audio.rs`)
- **`curves.json` is parsed once and cached** (`OnceLock`) instead of re-deserializing on every
  poll cycle and every Claude JSONL line — a pure CPU/allocation win for an always-on app. (`core.rs`)
- **`.gitignore` hardened** against accidentally committing the stray `New folder/`,
  `hallumeter sounds/`, and `.coordination/` working directories.

---

## Source Architecture Refactor: Forge → real Forge + Copilot CLI separated

### Background

The old `src-tauri/src/sources.rs` (single file) was already split into a `sources/` directory.
Within that directory, `forge.rs` was incorrectly reading from `~/.copilot/session-state/`
(GitHub Copilot CLI), not from the actual Forge VS Code extension. The two have been separated
into properly named, properly targeted readers.

---

### What changed

#### `src-tauri/src/sources/` directory

| File | Status | What it does |
|------|--------|-------------|
| `forge.rs` | **Replaced** | Now reads `~/.forge/hallumeter-bridge.json` — written by Forge VS Code extension |
| `copilot.rs` | **New** | The old forge.rs content, renamed — reads `~/.copilot/session-state/` (Copilot CLI) |
| `mod.rs` | **Updated** | Added `copilot` module; exports `read_copilot_usage` + `read_forge_usage` |
| `claude.rs` | Unchanged | |
| `codex.rs` | Unchanged | |
| `continue_*.rs` | Unchanged | |

#### `src-tauri/src/sources/forge.rs` — new implementation

Reads a single JSON file `~/.forge/hallumeter-bridge.json` written by the Forge VS Code extension.

Format:
```json
{"model":"gemma4-e4b-it-ud-q4kxl","used_tokens":12500,"max_tokens":98304,"timestamp_ms":1747000000000}
```

- Returns `None` if file is absent, stale (older than `activity_window_mins`), or malformed
- Does NOT take `max_files` parameter (single file, not a directory scan)
- Session label: `"Forge · <model-name>"`

#### `src-tauri/src/sources/copilot.rs` — new file (was forge.rs)

Identical logic to the old `forge.rs`, but:
- All internal names use `copilot_` prefix
- Session label changed from `"Forge · …"` to `"Copilot · …"`
- Controlled by `copilot_max_files` setting (see below)

#### `src-tauri/src/lib.rs` — updated

Poll loop candidates array now calls:
```rust
read_forge_usage(activity_secs),           // Forge VS Code bridge
read_copilot_usage(activity_secs, cfg.copilot_max_files),  // Copilot CLI
```

#### `src-tauri/src/settings.rs` — updated

`forge_max_files` renamed to `copilot_max_files`.

**Migration note:** if a `settings.json` exists in the app data dir with `forge_max_files`,
it will be silently ignored (falls back to default of 10). Rename the key to `copilot_max_files`
if you need a custom value.

---

### What the Forge VS Code extension must do

The Forge extension (`Desktop/Forge`) was updated to write `~/.forge/hallumeter-bridge.json`
on every `postTokenBudget()` call (after each conversation turn).

The Forge extension was also updated to write full session JSONL files to `~/.forge/sessions/`
for HalluScribe. See `Desktop/Forge/CHANGES.md` for details.

---

### Dependency map

```
Forge VS Code extension
  └── writes ~/.forge/hallumeter-bridge.json   ← HalluMeter reads this
  └── writes ~/.forge/sessions/*.jsonl          ← HalluScribe reads this

GitHub Copilot CLI (future)
  └── writes ~/.copilot/session-state/          ← HalluMeter reads this (copilot.rs)
```

---

### Tests

HalluMeter Rust tests still pass (`cargo test` in `src-tauri/`).
New unit test added in `forge.rs`: `parses_bridge_json`.
Existing Copilot CLI tests preserved in `copilot.rs`.
