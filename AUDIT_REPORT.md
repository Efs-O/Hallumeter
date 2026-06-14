# HalluMeter — Full Repository Audit

**Date:** 2026-06-15
**Scope:** Rust backend, Svelte/TS frontend, CI/packaging, docs, security, dependency health, repo hygiene
**Version audited:** 0.1.3
**Method:** Static read of every source file + config; no code changes made (report-only).

> The app is in good shape: it compiles clean under `clippy -D warnings`, has tests on both
> sides (16 Rust + 16 Vitest), a sensible CSP, and a husky gate mirrored in CI. The findings
> below are mostly *hidden* bugs, edge cases, and efficiency wins — not things you'd notice
> in normal use.

---

## Severity legend
- 🔴 **High** — wrong behavior users can hit, or a real correctness/perf bug
- 🟠 **Medium** — edge-case bug, footgun, or notable inefficiency
- 🟡 **Low** — cosmetic, doc drift, dead code, hygiene

---

## 🔴 High

### H1 — Panic Easter egg plays `panic.mp3` twice (overlapping)
Both the backend poll loop **and** the frontend trigger panic audio at ≥95 % fill:

- Backend: [lib.rs:343-347](src-tauri/src/lib.rs#L343-L347) — `player.play_panic(&resource_dir)` spawns a thread that decodes/plays `panic.mp3`.
- Frontend: [App.svelte:121-123](src/App.svelte#L121-L123) → `triggerPanic()` → `invoke("play_panic_audio")` → [lib.rs:41-58](src-tauri/src/lib.rs#L41-L58) spawns *another* thread that plays the same `panic.mp3`.

Both one-shots (`panic_played` in Rust, `panicFired` in JS) are independent, so at the moment
fill crosses 95 % you get **two simultaneous playbacks** of the same clip (slightly phased,
sounds like an echo/flam). `rodio` opens a second `OutputStream`, so they genuinely overlap.

**Fix:** pick one owner of the audio. The frontend path (`play_panic_audio`) is the one that
emits `panic-audio-done` for the visual cut-to-black sync, so it should own playback. Remove
the `player.play_panic(&resource_dir)` call at [lib.rs:346](src-tauri/src/lib.rs#L346) (and
the now-unused `AudioPlayer::play_panic`), keeping the backend responsible only for the
one-shot bookkeeping / state-audio suppression.

### H2 — `curves.json` is re-parsed from scratch on a hot path (per JSONL line)
`load_curves()` deserializes the embedded JSON string on **every** call ([core.rs:50-52](src-tauri/src/core.rs#L50-L52)), and it's called:

- once per candidate per poll in `interpolate_curve` ([core.rs:59](src-tauri/src/core.rs#L59)), and
- **once per JSONL line** in `parse_claude_usage_line` ([claude.rs:91](src-tauri/src/sources/claude.rs#L91)), which runs for every `message.usage` line in every Claude file each cycle, and
- once per Copilot shutdown parse ([copilot.rs:134](src-tauri/src/sources/copilot.rs#L134)).

A busy Claude session file has hundreds–thousands of usage lines; with `claude_max_files = 6`
that's potentially thousands of full JSON parses **every 5 seconds**, forever. It's invisible
on a fast desktop but it's pure wasted CPU/allocation in an always-on background app.

**Fix:** parse once and cache. `static CURVES: OnceLock<CurvesConfig>` (std, no new dep):
```rust
use std::sync::OnceLock;
static CURVES: OnceLock<CurvesConfig> = OnceLock::new();
pub fn load_curves() -> &'static CurvesConfig {
    CURVES.get_or_init(|| serde_json::from_str(CURVES_JSON).expect("curves.json is malformed"))
}
```
Then change the call sites to borrow. The data is `include_str!`-embedded and immutable, so a
process-lifetime cache is exactly right.

---

## 🟠 Medium

### M1 — Untracked junk directories are not git-ignored (publish risk)
`git status` shows these untracked and **not** covered by `.gitignore`:
- `New folder/` (loose `*.mp3` working copies)
- `hallumeter sounds/` (more loose mp3s, incl. `yellow *.mp3`)
- `.coordination/` (`autonomy.json`, `claims.json`, `events.ndjson`, `mcpstdio.log` — tooling state/logs)

`.gitignore` only ignores `/*.mp3` (root level), `new voices/`, `_drafts/`, `.claude/`. A
casual `git add .` would commit all of the above — including the `.coordination/` logs.

**Fix:** add to `.gitignore`:
```
/New folder/
/hallumeter sounds/
/.coordination/
```
Or delete the stray sound folders if they're superseded by `src-tauri/assets/audio/`.

### M2 — Claude source can report a *stale-but-high-fill* session over the *active* one
`read_claude_jsonl_usage` selects the file with the **maximum fill %** among recent files
([claude.rs:134](src-tauri/src/sources/claude.rs#L134)), then returns *that* file's timestamp
as `last_active_ms`. If you have two Claude sessions inside the activity window — one you're
actively typing in (low fill) and one idle-but-nearly-full — the reader surfaces the idle
high-fill one, and the global selector in [lib.rs:316-323](src-tauri/src/lib.rs#L316-L323)
then treats *its* (older) timestamp as the activity time. Net effect: the ring/session label
can show the wrong (idle) session.

This differs from Codex/Copilot, which also `max_by` fill but it's a smaller surface. Consider
selecting the **most recently active** file first (mtime/line-timestamp), then reading its
fill — matching the "most recent wins" intent of the top-level selector.

### M3 — Source-selection comparator is not a total order
The sort in [lib.rs:316-323](src-tauri/src/lib.rs#L316-L323) compares by "within 60 s → higher
risk, else → newer". This relation isn't transitive (A~B by time→risk, B~C by time→risk, but
A vs C may flip to time), so it's not a strict weak ordering. With only ≤5 candidates Rust's
`sort_by` won't panic and the result is "good enough", but it can pick a non-obvious winner in
3-way near-ties. If you ever add more sources, prefer an explicit two-key approach: bucket by
"is this within 60 s of the newest?", then within the newest bucket pick max risk; otherwise
pick max timestamp. Cleaner and deterministic.

### M4 — `panic_played` reset misses the very first session
[lib.rs:337](src-tauri/src/lib.rs#L337): `if session != prev_session && !prev_session.is_empty()`.
The `!prev_session.is_empty()` guard means the reset is skipped on the first transition out of
the empty initial state — fine — but combined with the fact that `prev_session` is only set
when a `best` candidate exists, a session that *starts already ≥95 %* on the first poll fires
panic once (good), and a later genuinely-new session resets it (good). No action required, but
note the frontend uses a different guard (`session !== "—"`, [App.svelte:108](src/App.svelte#L108));
the two one-shots can drift out of phase across rapid session switches. Worth unifying the
reset condition on both sides once H1 makes the frontend the single audio owner.

### M5 — Continue reader hard-codes the `dev_data/0.2.0/` schema path
[continue_reader.rs:264-271](src-tauri/src/sources/continue_reader.rs#L264-L271) reads
`dev_data/0.2.0/chatInteraction.jsonl` and `tokensGenerated.jsonl`. When Continue bumps its
dev-data schema version, this silently returns `None` (Continue just stops showing up — no
error). Consider globbing `dev_data/*/` and taking the highest version dir, or at least
documenting the coupling so it's a known maintenance point.

---

## 🟡 Low / cosmetic / hygiene

### L1 — `"unavailable"` state is dead code
The backend only ever emits `green | amber | red | stale` (poll loop + stale branch). The
`"unavailable"` arm exists in `state_to_rgb` ([lib.rs:79](src-tauri/src/lib.rs#L79)),
`AudioPlayer::play` ([audio.rs:80](src-tauri/src/audio.rs#L80)), `stateToColor`/`stateToMessage`
([risk.ts:18,28](src/lib/risk.ts#L18)), and is unreachable. The "Claude not found" message can
never render. Either wire it up (e.g. emit `unavailable` when *no* source dirs exist at all, vs
`stale` when a source went quiet) or drop it to reduce surface.

### L2 — Doc/code drift on the panic threshold
[App.svelte:59](src/App.svelte#L59) comment says "triggers once when fillPct hits **99%**", but
the actual trigger is `>= 95` ([App.svelte:121](src/App.svelte#L121)) and the inline comment at
[App.svelte:120](src/App.svelte#L120) says 95 %. Pick one number in the comments.

### L3 — Audio cadence doc drift
`AudioPlayer::rand_threshold` returns **8..=14** (`% 7 + 8`, [audio.rs:51-57](src-tauri/src/audio.rs#L51-L57))
and the doc comment correctly says "8..=14 … roughly 40–70 s". But the project memory / older
notes say "rand 5–10 requests". Make sure `SETTINGS.md` / README reflect 8–14 if they mention it.

### L4 — Per-playback `rodio::OutputStream`
Every cue opens a fresh `OutputStream` + `Sink` on a detached thread ([audio.rs:96-114](src-tauri/src/audio.rs#L96-L114)).
Functionally fine and keeps muting responsive, but it re-acquires the audio device each cue and
means overlapping cues each hold their own device handle. Low priority; only worth revisiting if
you ever hear device-contention glitches. A long-lived shared stream + one `Sink` would be the
"correct" structure but adds lifetime complexity in the detached-thread model.

### L5 — Frontend `listen("panic-audio-done")` registers a fresh listener per trigger
[App.svelte:68-71](src/App.svelte#L68-L71) sets up a new event listener inside `triggerPanic`,
auto-unsubscribed after 120 s. Bounded and small, but if H1 is fixed and panic can re-fire on a
new session within 120 s you can stack listeners. Prefer a single `onMount` listener that reads
`panicPhase`, or unsub immediately inside the handler.

### L6 — Repo carries duplicate/large media at root
`app-icon.png`, `ScreenShot018.jpg` (now git-ignored), and the 457 KB `ElevenLabs_…mp3` live at
the repo root; `docs/` already has `hallumeter.jpg`/`screenshot.jpg`. The root copies are mostly
ignored now, but they still sit in the working tree (and may be in history → clone bloat).
Consider `git rm --cached` for anything already committed and consolidate media under `docs/`.

### L7 — `shell:default` / `process:default` scope
[capabilities/default.json](src-tauri/capabilities/default.json) grants `shell:default` (used by
`open("https://x.com/...")`) and `process:default` (used by `exit(0)`). These work, but the
shell `open` capability is broad. If you want to tighten: scope `shell:allow-open` to an
`https?://` validator so a future XSS-ish bug couldn't `open` arbitrary URIs/executables. Low
risk given the locked-down CSP, but it's a cheap hardening.

### L8 — CI `build.yml` only builds/uploads on Windows
The matrix runs fmt/clippy/test on all three OSes (good), but `npm run tauri build` + artifact
upload are gated to Windows only ([build.yml](.github/workflows/build.yml)). Linux/macOS
*compile* via `cargo`/clippy but the **bundling** step (webkit/appindicator/dmg) is never
exercised until a tag triggers `release.yml`. A bundling break on mac/Linux would only surface
at release time. Consider running `tauri build` on all three in CI (or at least on PRs touching
packaging) to catch it earlier.

### L9 — `risk.ts` exports an unused helper
`fillPctToDashOffset` ([risk.ts:34-36](src/lib/risk.ts#L34-L36)) isn't used by `App.svelte` (the
ring is a full stroked circle, not a dash-offset arc). Either it's leftover from an earlier
arc-based design or intended for future use. If dead, remove it (and its test if any) to avoid
implying the ring is a progress arc.

---

## What's already good (don't regress)
- **Robust JSONL parsing:** every reader uses `let Ok(...) else { continue }` per line, so one
  malformed line never aborts a file — the exact lesson from the project history is applied
  consistently across all five sources.
- **Hand-rolled ISO-8601 parser** ([continue_types.rs:64-158](src-tauri/src/sources/continue_types.rs#L64-L158))
  is careful: civil_from_days, fractional-second truncation to ms, strict trailing-garbage
  rejection, offset handling. No `chrono` dependency needed.
- **Settings are forward/backward compatible:** `#[serde(default)]` + `unwrap_or_default()` means
  partial/corrupt `settings.json` never bricks the app ([settings.rs:90-96](src-tauri/src/settings.rs#L90-L96)).
- **Tight CSP** with `object-src 'none'`, `frame-ancestors 'none'`, no remote origins in prod.
- **Quality gate parity:** husky pre-commit mirrors CI (fmt + clippy -D warnings + cargo test +
  vitest), so local and CI fail the same way.

---

## Suggested priority order
1. **H1** (double panic audio) — small, isolated, user-audible.
2. **H2** (cache `load_curves`) — small, pure win for an always-on app.
3. **M1** (gitignore the stray dirs) — 3 lines, prevents an embarrassing accidental commit.
4. **M2 / M5** — correctness/maintenance for multi-session and Continue-version drift.
5. The 🟡 items as cleanup when touching those files.

*No code was changed. Want me to implement any of these? H1 + H2 + M1 are low-risk and I can do
them without touching audio timing or curve values.*
