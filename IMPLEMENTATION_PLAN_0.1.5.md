# HalluMeter 0.1.5 — Implementation Plan

**Source:** [AUDIT_REPORT_0.1.4.md](AUDIT_REPORT_0.1.4.md) (2026-07-19)
**Branch:** `audit-0.1.5`
**Scope:** the four new priority findings (N1–N4) + doc drift (N10) + version bump.
Carryovers (M2/M3/M5, L-items) stay open — deliberately out of scope to keep the diff reviewable.

> Field note from the user: unknown-model fallback already *displays* new model names
> correctly (names come straight from the JSONL). N1 is therefore purely about the
> risk-curve and context-window math behind the ring, not the label.

---

## 1. N1 — curves.json refresh + model-family prefix matching

**Data** ([src-tauri/assets/curves.json](src-tauri/assets/curves.json)):
- Add entries: `claude-sonnet-5` (clone Sonnet 4.6 curve), `claude-opus-4-8` and
  `claude-fable-5` (clone Opus 4.6 curve — Fable shares Mythos-class weights, best
  available anchor is the strongest Claude curve), `claude-haiku-4-5` (between Sonnet
  and GPT-mini — smaller model, earlier degradation).
- All `context_window: 200000` (Claude Code default; 1M-beta not visible in JSONL).
- Each `curve_source` notes "cloned pending benchmark data" so future easing passes
  know these are unmeasured.

**Code** ([src-tauri/src/core.rs](src-tauri/src/core.rs)):
- New `find_model_curve(model) -> Option<&'static ModelCurve>`: exact id match first,
  then longest-prefix match (`model.starts_with(curve.id)` on a `-` boundary) so
  date-suffixed ids like `claude-haiku-4-5-20251001` hit the family curve.
- `interpolate_curve` uses it; on total miss, keep current first-curve fallback but
  `eprintln!` once per unknown id (static `Mutex<HashSet<String>>`).
- New `context_window_for(model) -> Option<u64>` used by
  [claude.rs](src-tauri/src/sources/claude.rs) (replaces inline lookup + 200 000
  fallback stays) and [copilot.rs](src-tauri/src/sources/copilot.rs) (128 000 fallback stays).
- Test: `claude-haiku-4-5-20251001` resolves to the haiku curve, not the generic first curve.

## 2. N2 — real WiX upgradeCode

- Generate one GUID with `[guid]::NewGuid()`, replace the `A1B2C3D4-…` placeholder in
  [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json).
- **Consequence (accepted):** MSI installs of ≤0.1.4 won't auto-upgrade — uninstall
  once, then install 0.1.5. Note this in release notes/CHANGES.md.

## 3. N3 — stable session id for the panic one-shot

Panic reset currently keys on the display title, which mutates (first-message →
ai-title → custom-title), so the Easter egg can re-fire mid-session.

- Reader tuples gain a **6th element** `session_id: String` (appended last, so
  positional test asserts `usage.0`–`usage.4` stay valid):
  - claude → session file path; codex → `session_meta` id, else file path;
    copilot → session dir path; forge → `"forge"`; continue → `sessionId`.
- `ContextPayload` gains `session_id`; poll loop's `prev_session` and App.svelte's
  reset check compare `session_id` instead of the title. Title remains display-only.

## 4. N4 — per-file parse cache in claude.rs

- `static SESSION_CACHE: Mutex<HashMap<PathBuf, CachedSession>>` keyed on
  `(mtime, len)`; on hit, skip the file read and both parse passes entirely.
- Bounded: prune entries whose path wasn't seen this poll (map stays ≤ recent-file count).

## 5. N10 — doc drift + version

- [App.svelte:59](src/App.svelte#L59): comment 99 % → 95 %.
- [sources/mod.rs:1](src-tauri/src/sources/mod.rs#L1): header lists all five sources correctly.
- Bump `0.1.4` → `0.1.5` in `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
- CHANGES.md entry.

## 6. Verification

1. `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (src-tauri), `npx vitest run`.
2. Kill any running HalluMeter.exe (Windows file-lock lesson), then `npm run tauri build`.
3. Manual smoke: launch the built exe, confirm live session shows a curve-matched
   model (no unknown-model log line), ring updates, panic one-shot untouched.
