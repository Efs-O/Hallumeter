use crate::audio::AudioPlayer;
use crate::core::{estimate_risk, risk_to_state, ContextPayload};
use crate::settings::UserSettings;
use crate::sources::{
    read_claude_jsonl_usage, read_codex_jsonl_usage, read_continue_usage, read_copilot_usage,
    read_forge_usage, Usage, UsageResult,
};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Emitter;

type Candidate = (String, f64, String, u64, i64, String, f64, bool);

pub struct Monitor {
    app: tauri::AppHandle,
    color_state: Arc<Mutex<String>>,
    resource_dir: PathBuf,
    cfg: UserSettings,
    continue_bridge_yaml: Option<PathBuf>,
    setup_diagnostic: Option<String>,
    player: AudioPlayer,
    last_data: Option<Instant>,
    prev_state: String,
    prev_diagnostic: Option<String>,
    prev_fill_pct: f64,
    prev_session: String,
    request_counter: u32,
    play_threshold: u32,
    panic_played: bool,
    last_variant_by_state: HashMap<String, u8>,
}

impl Monitor {
    pub fn new(
        app: tauri::AppHandle,
        color_state: Arc<Mutex<String>>,
        resource_dir: PathBuf,
        cfg: UserSettings,
        continue_bridge_yaml: Option<PathBuf>,
        setup_diagnostic: Option<String>,
        player: AudioPlayer,
    ) -> Self {
        let mut player = player;
        let play_threshold = player.rand_threshold() as u32;
        Self {
            app,
            color_state,
            resource_dir,
            cfg,
            continue_bridge_yaml,
            setup_diagnostic,
            player,
            last_data: None,
            prev_state: String::new(),
            prev_diagnostic: None,
            prev_fill_pct: -1.0,
            prev_session: String::new(),
            request_counter: 0,
            play_threshold,
            panic_played: false,
            last_variant_by_state: HashMap::new(),
        }
    }

    pub fn run(mut self) {
        loop {
            let started = Instant::now();
            self.poll_once();
            let elapsed = started.elapsed();
            if elapsed >= Duration::from_secs(1) {
                eprintln!(
                    "[monitor] slow poll completed in {} ms",
                    elapsed.as_millis()
                );
            }
            std::thread::sleep(Duration::from_secs(5));
        }
    }

    fn poll_once(&mut self) {
        if let Some(diagnostic) = self.setup_diagnostic.clone() {
            self.emit_unavailable("Configuration error", "—", diagnostic);
            return;
        }

        let activity_secs = self.cfg.activity_window_mins * 60;
        let correlation_ms = self.cfg.continue_correlation_secs as i64 * 1000;
        let sources = [
            (
                "Claude",
                read_claude_jsonl_usage(activity_secs, self.cfg.claude_max_files),
            ),
            (
                "Codex",
                read_codex_jsonl_usage(activity_secs, self.cfg.codex_max_files),
            ),
            ("Forge", read_forge_usage(activity_secs)),
            (
                "Copilot",
                read_copilot_usage(activity_secs, self.cfg.copilot_max_files),
            ),
            (
                "Continue",
                read_continue_usage(
                    activity_secs,
                    correlation_ms,
                    self.continue_bridge_yaml.clone(),
                ),
            ),
        ];
        let (usages, source_errors) = collect_usages(sources);

        let mut unsupported = Vec::new();
        let mut candidates: Vec<Candidate> = usages
            .into_iter()
            .filter_map(
                |(model, fill_pct, session, tokens, active_ms, session_id)| {
                    let adjusted = (fill_pct + self.cfg.context_overhead_pct).clamp(0.0, 100.0);
                    let Some(risk) = estimate_risk(&model, adjusted) else {
                        unsupported.push((model, session, active_ms));
                        return None;
                    };
                    Some((
                        model,
                        adjusted,
                        session,
                        tokens,
                        active_ms,
                        session_id,
                        risk.risk_score,
                        risk.approximate,
                    ))
                },
            )
            .collect();

        candidates.sort_by(candidate_order);
        if let Some(candidate) = candidates.into_iter().next() {
            self.emit_active(candidate, source_errors.into_iter().next());
            return;
        }

        if let Some((model, session, _)) = unsupported.into_iter().max_by_key(|item| item.2) {
            let diagnostic = format!("Unsupported model curve: {model}");
            self.emit_unavailable(&session, &model, diagnostic);
            return;
        }

        if let Some(diagnostic) = source_errors.into_iter().next() {
            self.emit_unavailable("Source error", "—", diagnostic);
            return;
        }

        if self
            .last_data
            .is_some_and(|last| last.elapsed() >= Duration::from_secs(self.cfg.stale_timeout_secs))
        {
            self.emit_empty("stale", None);
        } else if self.last_data.is_none() {
            self.emit_empty(
                "unavailable",
                Some("No supported active session detected".to_string()),
            );
        }
    }

    fn emit_active(&mut self, candidate: Candidate, diagnostic: Option<String>) {
        let (model, fill_pct, session, tokens, _, session_id, risk_score, approximate) = candidate;
        // A source error is worth more to the user than the approximate-curve notice,
        // so only fall back to the latter when there is nothing else to report.
        let diagnostic = diagnostic.or_else(|| {
            approximate.then(|| format!("Approximate curve — no benchmark data for {model}"))
        });
        let state =
            risk_to_state(risk_score, self.cfg.amber_threshold, self.cfg.red_threshold).to_string();
        self.last_data = Some(Instant::now());

        if session_id != self.prev_session && !self.prev_session.is_empty() {
            self.panic_played = false;
        }
        self.prev_session = session_id.clone();
        let panic_firing = fill_pct >= 95.0 && !self.panic_played;
        self.panic_played |= panic_firing;
        let variant = self.next_audio_variant(&state, fill_pct, panic_firing);
        self.prev_fill_pct = fill_pct;

        self.emit(
            ContextPayload {
                fill_pct,
                risk_score,
                state,
                model,
                session,
                session_id,
                variant,
                tokens,
                diagnostic,
                approximate,
            },
            true,
        );
    }

    fn next_audio_variant(&mut self, state: &str, fill_pct: f64, panic_firing: bool) -> u8 {
        if panic_firing {
            self.prev_state = state.to_string();
            self.request_counter = 0;
            return 0;
        }
        let changed = state != self.prev_state;
        let threshold_hit = fill_pct > self.prev_fill_pct && self.prev_fill_pct >= 0.0 && {
            self.request_counter += 1;
            self.request_counter >= self.play_threshold
        };
        if !changed && !threshold_hit {
            return 0;
        }
        let variant = self
            .player
            .rand_1_5_avoiding(self.last_variant_by_state.get(state).copied());
        self.player.play(state, variant, &self.resource_dir);
        self.last_variant_by_state
            .insert(state.to_string(), variant);
        self.prev_state = state.to_string();
        self.request_counter = 0;
        self.play_threshold = self.player.rand_threshold() as u32;
        variant
    }

    fn emit_unavailable(&mut self, session: &str, model: &str, diagnostic: String) {
        if self.prev_diagnostic.as_deref() != Some(diagnostic.as_str()) {
            eprintln!("[monitor] {diagnostic}");
        }
        self.emit(
            ContextPayload {
                fill_pct: 0.0,
                risk_score: 0.0,
                state: "unavailable".to_string(),
                model: model.to_string(),
                session: session.to_string(),
                session_id: "—".to_string(),
                variant: 0,
                tokens: 0,
                diagnostic: Some(diagnostic),
                approximate: false,
            },
            false,
        );
    }

    fn emit_empty(&mut self, state: &str, diagnostic: Option<String>) {
        self.emit(
            ContextPayload {
                fill_pct: 0.0,
                risk_score: 0.0,
                state: state.to_string(),
                model: "—".to_string(),
                session: "—".to_string(),
                session_id: "—".to_string(),
                variant: 0,
                tokens: 0,
                diagnostic,
                approximate: false,
            },
            false,
        );
    }

    fn emit(&mut self, payload: ContextPayload, active: bool) {
        self.prev_diagnostic = payload.diagnostic.clone();
        if self.prev_state != payload.state {
            self.prev_state = payload.state.clone();
            if !active {
                self.prev_fill_pct = -1.0;
                self.request_counter = 0;
            }
        }
        match self.color_state.lock() {
            Ok(mut color) => *color = payload.state.clone(),
            Err(error) => eprintln!("[monitor] color-state lock failed: {error}"),
        }
        if let Err(error) = self.app.emit("context-update", payload.clone()) {
            eprintln!("[monitor] failed to emit context update: {error}");
        }
        crate::set_tray_color(&self.app, &payload.state);
    }
}

fn collect_usages(sources: [(&str, UsageResult); 5]) -> (Vec<Usage>, Vec<String>) {
    let mut usages = Vec::new();
    let mut errors = Vec::new();
    for (name, result) in sources {
        match result {
            Ok(Some(usage)) => usages.push(usage),
            Ok(None) => {}
            Err(error) => errors.push(format!("{name}: {error}")),
        }
    }
    (usages, errors)
}

fn candidate_order(a: &Candidate, b: &Candidate) -> Ordering {
    if (a.4 - b.4).abs() <= 60_000 {
        b.6.partial_cmp(&a.6).unwrap_or(Ordering::Equal)
    } else {
        b.4.cmp(&a.4)
    }
}
