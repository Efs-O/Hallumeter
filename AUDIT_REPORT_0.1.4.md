# HalluMeter — Follow-up Audit (v0.1.4)

**Date:** 2026-07-19
**Scope:** Rust backend, Svelte/TS frontend, packaging, CI, dependency health
**Method:** Static read of all source files + configs. **No code was changed** — report only.
**Previous audit:** [AUDIT_REPORT.md](AUDIT_REPORT.md) (2026-06-15, v0.1.3)

> Verdict: the app is in solid shape. The 0.1.4 "audit fixes" release genuinely resolved
> the previous report's top findings — panic audio is now frontend-owned with backend
> one-shot bookkeeping only, `load_curves()` is `OnceLock`-cached, and the stray
> directories are git-ignored (working tree is clean). What remains is mostly data
> freshness, packaging hygiene, and edge cases.

---

## Status of previous findings

| Prior ID | Finding | Status in 0.1.4 |
|---|---|---|
| H1 | Panic audio played twice | ✅ **Fixed** — backend only tracks the one-shot ([lib.rs:342-350](src-tauri/src/lib.rs#L342-L350)) |
| H2 | `curves.json` re-parsed per JSONL line | ✅ **Fixed** — `OnceLock` cache ([core.rs:54-58](src-tauri/src/core.rs#L54-L58)) |
| M1 | Junk dirs not git-ignored | ✅ **Fixed** — `.gitignore` covers them; `git status` clean |
| M2 | Stale-but-high-fill session wins over active one | ⬜ Open (see C1) |
| M3 | Non-transitive source-selection comparator | ⬜ Open (see C2) |
| M5 | Continue `dev_data/0.2.0/` hard-coded | ⬜ Open (see C3) |
| L1 | `"unavailable"` state is dead code | ⬜ Open |
| L2 | Panic-threshold comment says 99 %, code says 95 % | ⬜ Open — [App.svelte:59](src/App.svelte#L59) still says 99 % |
| L4 | Per-playback `rodio::OutputStream` | ⬜ Open (accepted trade-off) |
| L5 | Listener stacking in `triggerPanic` | ⬜ Open |
| L7 | Broad `shell:default` capability | ⬜ Open |
| L8 | Tauri bundling only exercised on Windows in CI | ⬜ Open |
| L9 | Unused `fillPctToDashOffset` export | ⬜ Open |

---

## 🔴 New — High

### N1 — `curves.json` model list is a generation behind
[curves.json](src-tauri/assets/curves.json) knows: `claude-sonnet-4-6`, `claude-opus-4-6`,
`gpt-5-4`, `gpt-5-4-mini`, `gemini-3-1-pro`. Current Claude Code sessions report models
like `claude-sonnet-5`, `claude-opus-4-8`, `claude-fable-5`, `claude-haiku-4-5-20251001`
— none of which are in the file. Two silent fallbacks then kick in:

- [claude.rs:92-97](src-tauri/src/sources/claude.rs#L92-L97): unknown model → context
  window defaults to **200 000**. Correct for most Claude models today, but wrong for any
  larger-context model, making **fill % itself wrong**, not just the risk mapping.
- [core.rs:64-72](src-tauri/src/core.rs#L64-L72): unknown model → first curve in the file
  (`claude-sonnet-4-6`'s degradation curve) is used for risk.

For an app whose whole purpose is watching Claude Code sessions, this is the
highest-impact finding: **on a current model the meter runs entirely on fallbacks.**
It still shows plausible numbers, which is exactly why it's easy to miss.

**Suggestions (pure data change, no code):**
1. Add entries for the current model ids (Sonnet 5, Opus 4.8, Fable 5, Haiku 4.5, and
   current Codex/Gemini ids if you use those sources). Cloning the nearest existing
   curve per tier is fine as a first pass; refine knots later per RESEARCH.md.
2. Optional code hardening: log once per unknown model id (`eprintln!` behind a seen-set)
   so future model bumps are visible instead of silent. Same applies to the `128_000`
   fallback in [copilot.rs:141](src-tauri/src/sources/copilot.rs#L141).
3. Consider a prefix/family match (`claude-sonnet-*` → sonnet curve) so date-suffixed ids
   like `claude-haiku-4-5-20251001` don't need exact entries.

### N2 — WiX `upgradeCode` is a placeholder GUID
[tauri.conf.json](src-tauri/tauri.conf.json) ships
`"upgradeCode": "A1B2C3D4-E5F6-7890-ABCD-EF1234567890"` — a sequential
tutorial-placeholder GUID. Risks:

- Any other app that copy-pasted the same placeholder collides in Windows Installer's
  upgrade table — one install can silently remove/upgrade the other.
- The upgrade code is supposed to be **permanent for the product's lifetime**; every
  released MSI so far carries this value, so changing it later breaks the auto-upgrade
  path from those versions (users would need to uninstall first).

**Suggestion:** generate one real GUID (`[guid]::NewGuid()` in PowerShell), put it in the
config, and never touch it again. Do this **before** wider distribution — the earlier the
switch, the fewer installs are stranded on the placeholder lineage. Worth a release-notes
line telling existing users to uninstall/reinstall once.

---

## 🟠 New — Medium

### N3 — Panic one-shot resets when the session *title* changes, not the session
Both sides key the reset on the session **string**:
[lib.rs:337](src-tauri/src/lib.rs#L337) (`session != prev_session`) and
[App.svelte:108](src/App.svelte#L108) (`e.payload.session !== session`). But the Claude
title is *derived and mutable*: it starts as the first user message, then upgrades to
`ai-title`, then possibly `custom-title` ([claude.rs:11-64](src-tauri/src/sources/claude.rs#L11-L64)).
Each upgrade looks like "new session" → `panic_played`/`panicFired` reset → at ≥95 % fill
the panic Easter egg (audio + strobe) can **re-fire inside the same session**. Codex has
the same shape (index `thread_name` appearing later replaces the fallback title).

**Suggestion:** carry a stable id (Claude file path / Codex `session_id`) through the
payload for one-shot bookkeeping, and keep the pretty title for display only. Small
struct change in `ContextPayload`, no behavior change otherwise.

### N4 — Whole-file re-read and double parse of every Claude session, every 5 s
Per poll cycle, each recent Claude file (≤ 6) is fully read
([claude.rs:126](src-tauri/src/sources/claude.rs#L126)) and then parsed **twice**: once
scanning from the end for usage ([claude.rs:127-130](src-tauri/src/sources/claude.rs#L127-L130) —
`next_back()` is smart, it stops at the last valid line) and once **front-to-back over
every line** for the title ([claude.rs:131](src-tauri/src/sources/claude.rs#L131)).
Long-running sessions grow to tens of MB, so the title scan alone becomes
megabytes of JSON parsing every 5 seconds for a value that almost never changes.

**Suggestion:** cache per path keyed on `(mtime, len)` — skip both the read and the
title parse when unchanged. A `HashMap<PathBuf, CachedSession>` owned by the poll thread
is enough; no locking needed. This is the successor to the fixed H2 — same "always-on
background app" argument.

### N5 — `serde_yaml 0.9` is unmaintained; `rodio 0.17` is several majors behind
[Cargo.toml](src-tauri/Cargo.toml):
- `serde_yaml` was archived by its maintainer (2024) — no fixes, including security
  fixes, will ever land. It works today, but `cargo audit`/dependabot will flag it
  (RUSTSEC-2024-0320). Drop-in successor: `serde_yml`, or the smaller `yaml-rust2` if
  only `bridge.yaml`/`config.yaml` parsing is needed.
- `rodio 0.17` is ~2 years old (0.20/0.21 current, with a reworked `OutputStream` API).
  Not urgent, but upgrading later gets harder the longer it waits, and the newer API
  makes the long-lived-stream fix for prior-L4 more natural.

Low urgency, but both are the kind of debt that's cheapest to pay while the audio/YAML
surface is this small.

---

## 🟡 New — Low / hygiene

### N6 — Version string lives in three places
`package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` all say `0.1.4`
(consistent today, and history shows bump commits do touch all three). One missed file
would ship a mismatched MSI product version. **Suggestion:** a tiny `npm version`-hook or
bump script that rewrites all three, or point `tauri.conf.json` at
`"version": "../package.json"` (Tauri 2 supports reading it from there).

### N7 — Window *size* persists, position doesn't
[lib.rs:255-257](src-tauri/src/lib.rs#L255-L257) persists resize; `center: true` in
[tauri.conf.json](src-tauri/tauri.conf.json) recenters every launch. For a
pin-it-in-a-corner always-on-top widget, restoring the last position (with an
on-screen-bounds sanity check for unplugged monitors) is the natural next step —
`window_x`/`window_y` alongside the existing `window_width`/`window_height` in
[settings.rs](src-tauri/src/settings.rs).

### N8 — First-run flag is written before the user sees the overlay
[lib.rs:62-73](src-tauri/src/lib.rs#L62-L73): `check_first_run` creates `seen.flag` at
query time. If the app crashes or is closed before the overlay is dismissed, the intro
never shows again. Cosmetic; fix is a separate `mark_first_run_seen` command invoked on
dismiss.

### N9 — Poll thread holds the color mutex across icon redraws (non-issue, but note)
The `AppColorState` mutex is locked briefly and cloned everywhere — done correctly. Just
noting it survived review; no action.

### N10 — Doc drift accumulating
- [App.svelte:59](src/App.svelte#L59): "hits 99%" vs. actual 95 % trigger (prior L2, still there).
- [sources/mod.rs:1](src-tauri/src/sources/mod.rs#L1): header says "Claude Code, Codex,
  Forge (Copilot CLI), and Continue" — Forge and Copilot CLI are *separate* sources now
  (five, not four), and Forge is the VS Code bridge, not Copilot CLI.
- `AUDIT_REPORT.md` (v0.1.3) is now historical — consider renaming to
  `docs/audits/2026-06-15.md` so the repo root doesn't imply it's current.

---

## What's still good (don't regress)

- Per-line `let Ok(...) else { continue }` JSONL tolerance everywhere — one bad line
  never kills a reader.
- `#[serde(default)]` settings — partial/corrupt `settings.json` can't brick startup.
- Tight CSP (`object-src 'none'`, `frame-ancestors 'none'`, no remote origins in prod).
- Husky pre-commit mirrors CI (fmt + clippy `-D warnings` + cargo test + vitest).
- The `next_back()` usage-line scan in claude.rs — parses from the file's end, so the
  hot path already avoids full-file JSON parsing (the *title* scan is the exception, N4).

---

## Suggested priority order

1. **N1** — refresh `curves.json` model ids. Pure data edit, biggest accuracy win.
2. **N2** — real WiX GUID, one line, and the cost of delaying grows with every install.
3. **N3** — stable session id for the panic one-shot (small, prevents a user-visible
   double Easter egg).
4. **N4** — per-file cache (perf, same spirit as the already-fixed H2).
5. **N5 + carryovers (M2/M3/M5)** — when next touching those files.
6. 🟡 items as drive-by cleanup.

*No code was changed for this audit. N1 and N2 are safe to apply without touching any
logic; say the word and I'll do them.*
