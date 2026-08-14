# HalluMeter audit — review report

**Date:** 2026-08-12  
**Version:** 0.1.5  
**Scope:** application source, runtime configuration, repository hygiene, error handling, fallbacks, background work, and automated checks.  
**Method:** five bounded read-only passes; no product code, configuration, data, or files were deleted/changed. This report is the only file added.

## Executive summary

The checked quality gates pass: Rust formatting, Clippy with warnings denied, 45 Rust tests, and 16 Vitest tests. The main risks are hidden operational failures: several settings and Tauri operations deliberately discard errors, the no-source case appears healthy briefly, and the `unavailable` UI state is never emitted. The five-second poll loop is intentionally bounded but still does synchronous filesystem scanning/parsing on a dedicated thread.

## Findings

### High

#### H1 — First-run marker reports success even when persistence fails

`check_first_run` ignores failures from both `create_dir_all` and `write`, then returns `true` ([src-tauri/src/lib.rs](src-tauri/src/lib.rs#L62)). A read-only or full app-data location can therefore show the onboarding on every launch with no diagnostic.

Suggested fix: make the command return `Result<bool, String>`; only return `true` after the flag is written, and surface/log a contextual error when persistence fails.

#### H2 — Settings corruption is silently replaced by defaults

`load_settings` returns defaults for a missing, unreadable, empty, or malformed `settings.json` ([src-tauri/src/settings.rs](src-tauri/src/settings.rs#L90)). A typo can silently reset thresholds, source limits, overhead, and window preferences. The backend also silently defaults when `app_data_dir()` fails ([src-tauri/src/lib.rs](src-tauri/src/lib.rs#L169)).

Suggested fix: distinguish absent settings (acceptable defaults) from read/parse/path errors. Preserve the invalid file, log its path and parse error, and expose a visible non-blocking configuration warning.

#### H3 — No data source is initially represented as healthy

The poll thread initializes `last_data` to now and emits no state until the stale timeout expires ([src-tauri/src/lib.rs](src-tauri/src/lib.rs#L283), [src-tauri/src/lib.rs](src-tauri/src/lib.rs#L412)). The frontend itself starts green, so a fresh launch with no supported tool/session looks "Functioning normally" for the default 30 seconds.

Suggested fix: emit an explicit initial state immediately. Use `unavailable` when no configured/supported source is discoverable, and `stale` only after previously valid data stops arriving.

### Medium

#### M1 — `unavailable` is dead state, and its message is Claude-specific

The frontend type, CSS, audio guard, and color mapper include `unavailable`, but the backend emits only green/amber/red/stale ([src/lib/risk.ts](src/lib/risk.ts#L10), [src-tauri/src/lib.rs](src-tauri/src/lib.rs#L412)). Its text says “Claude not found,” although the app supports five sources.

Suggested fix: either implement the state as described in H3 with source-neutral wording (for example, “No supported session detected”), or remove the unused state and all related branches.

#### M2 — Multiple runtime errors are silently discarded

The following failures do not reach the user and, in several cases, are not logged:

- `app.emit` failures for panic completion and context updates ([src-tauri/src/lib.rs](src-tauri/src/lib.rs#L43), [src-tauri/src/lib.rs#L398)).
- Tray/window icon, show, focus, window-size, and always-on-top failures ([src-tauri/src/lib.rs](src-tauri/src/lib.rs#L105), [src-tauri/src/lib.rs#L154), [src-tauri/src/lib.rs#L243)).
- The tray always-on-top toggle treats an `is_always_on_top` failure as `false`, potentially inverting the setting ([src-tauri/src/lib.rs](src-tauri/src/lib.rs#L225)).
- Frontend `check_first_run`, event subscription, external-link open, hide, and quit promises have no rejection handling ([src/App.svelte](src/App.svelte#L105), [src/App.svelte#L204)).

Suggested fix: create a small, consistent error-reporting policy. Log unexpected operating-system/Tauri failures with action and context; return `Result` for commands where the UI needs to recover; do not invent a boolean fallback after a failed window query.

#### M3 — The poll loop has bounded but synchronous I/O every five seconds

One detached OS thread scans up to 6 Claude files, 10 Codex files, 10 Copilot directories, the Forge bridge, and Continue telemetry every five seconds ([src-tauri/src/lib.rs](src-tauri/src/lib.rs#L270), [src-tauri/src/lib.rs#L292), [src-tauri/src/lib.rs#L435)). Claude cache keys avoid re-parsing unchanged files, but the source roots are still enumerated each cycle; Codex, Copilot, and Continue readers parse selected data synchronously. Large telemetry histories or slow/network-mounted profiles may delay a cycle and increase CPU/disk use.

Suggested fix: add timing/diagnostic counters first (per-source scan duration, candidates scanned, parse failures). Then cache directory metadata/offsets, tail append-only JSONL rather than re-reading it, and consider filesystem watching with a low-frequency safety poll. Keep all reads off the UI thread as they are now.

#### M4 — Frontend timers can outlive the state they describe

Every voice update schedules a nine-second clear without cancelling the previous timer ([src/App.svelte](src/App.svelte#L129)). A prior timer can clear a newer line early. Panic listener cleanup is deferred with an untracked two-minute timeout, and `listen()` rejection leaves the panic view stuck unless audio invocation itself fails ([src/App.svelte](src/App.svelte#L70)).

Suggested fix: retain and cancel timer handles on replacement/unmount; wrap listener registration and command invocation in one async flow; unsubscribe immediately after the one completion event rather than using a two-minute delayed cleanup.

#### M5 — Capability grants are broader than the current use

The default capability grants `shell:default` and `process:default` ([src-tauri/capabilities/default.json](src-tauri/capabilities/default.json#L6)). The frontend uses shell only to open one fixed HTTPS link and process only to quit ([src/App.svelte](src/App.svelte#L204), [src/App.svelte#L253)). Broad defaults enlarge the impact of a future frontend injection bug.

Suggested fix: replace defaults with the narrowest `shell:allow-open` scope that allows only HTTPS (ideally the one fixed host) and the one required process permission. Validate against Tauri v2 capability syntax before changing permissions.

#### M6 — User documentation drifts from the executable defaults and field name

`SETTINGS.md` examples/documentation call the Copilot limit `forge_max_files` and describe defaults of `activity_window_mins: 30`, amber `0.20`, red `0.38`; the program uses `copilot_max_files`, 15, 0.15, and 0.30 ([SETTINGS.md](SETTINGS.md#L23), [src-tauri/src/settings.rs](src-tauri/src/settings.rs#L71)). `TODO.md` also says CSP is null, but CSP is already configured ([TODO.md](TODO.md#L19), [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json#L30)).

Suggested fix: update docs and TODO as a single reviewable documentation change; add a test or generated settings-reference snippet sourced from `UserSettings::default()` to stop future drift.

### Low

#### L1 — Hardcoded conventional home subpath bypasses the Tauri path API

Continue bridge discovery derives `Desktop/llamabridge/config/bridge.yaml` from `USERPROFILE`/`HOME` ([src-tauri/src/settings.rs](src-tauri/src/settings.rs#L17)). This conflicts with the project rule to use Tauri’s path API and can be wrong for relocated desktops, localized paths, or unusual profiles.

Suggested fix: resolve Desktop through `app.path().desktop_dir()` at setup, then pass the resolved optional path to settings/source logic; retain the user-configured path as the explicit option.

#### L2 — Stale repository candidates require owner review, but do not delete them

The root has legacy reports/plans and ignored media. In particular, the 446.5 KiB `ElevenLabs_…mp3` is not tracked and is only mentioned by a historical audit; `ScreenShot018.jpg`, `hallumeter.jpg`, `HALLUSCRIBE_PLAN.md`, `HALLUCINATION_METRICS_REPORT.md`, and `CONTINUE_READER_*.md` are ignored. The untracked `hardware/` directory is also present and outside the desktop app’s active source tree.

Suggested fix: confirm ownership before any cleanup. If intentional local working material, relocate it to a clearly documented local-notes/assets location or keep the ignores; if obsolete, delete only after explicit approval. No deletion was performed.

#### L3 — Legacy audit/plan files make current status hard to determine

`AUDIT_REPORT.md`, `AUDIT_REPORT_0.1.4.md`, and `IMPLEMENTATION_PLAN_0.1.5.md` remain alongside the current state. Some now-fixed entries and explicitly deferred work are mixed with live information.

Suggested fix: keep this report as the current review artifact and, after implementation decisions, add a short status index or archive superseded reports. Do not remove history without approval.

## Intentional fallbacks and their risks

Some fallbacks are documented/intentional, but deserve explicit telemetry or UI disclosure:

- Unknown models use the first generic curve/context-window fallback; a warning is printed once per model ([src-tauri/src/core.rs](src-tauri/src/core.rs#L82)). This is visible only in logs and may materially distort risk scores.
- Claude/Codex/Copilot/Forge/Continue source readers convert many file, metadata, and parse failures to `None`, which is later indistinguishable from an inactive session. This avoids breaking the meter on partial writes, but hides schema changes and permission failures.
- Missing optional session titles use generic labels such as `Codex`, `Continue`, or `-`; harmless for display, but they can mask a parser mismatch.
- The Continue bridge discovery silently falls back from an invalid configured path to the conventional Desktop path.

Suggested direction: retain resilience for transient partial writes, but record rate-limited per-source diagnostic state (last success, last error kind, and last parse error) and make it inspectable in a small diagnostics view or debug log.

## Source files over the 350-LOC rule — list only

| Lines | File |
|---:|---|
| 448 | `src-tauri/src/lib.rs` |

No LOC remediation is proposed here, per request.

## Verification

| Check | Result |
|---|---|
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | Passed |
| `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` | Passed |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Passed — 45 tests |
| `npx vitest run --reporter=dot` | Passed — 16 tests |

The normal combined quality-gate command was initially interrupted by the tool timeout while a cold Rust build ran; each gate was then rerun independently and passed.

## Recommended review order

1. Decide desired no-source behavior (`unavailable` versus immediate `stale`) and the level of diagnostic visibility (H2/H3/M1).
2. Approve the error-handling contract for settings, Tauri commands, and transient source parsing (H1/H2/M2).
3. Decide whether to tighten capabilities (M5), then verify the exact Tauri permission syntax before implementation.
4. Correct docs/TODO and decide ownership of stale-candidate files (M6/L2/L3).
5. Profile real telemetry directories before optimizing the polling architecture (M3).
