mod audio;
mod core;
mod settings;
mod sources;

#[cfg(test)]
mod tests;

use audio::AudioPlayer;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalSize, Manager, Size,
};

/// Tauri-managed wrapper around the shared mute flag.
struct MuteState(Arc<AtomicBool>);

/// Tracks the current risk state so the tray/taskbar icon can be re-applied
/// when the window is shown after being hidden.
struct AppColorState(Arc<Mutex<String>>);

#[tauri::command]
fn set_mute(state: tauri::State<MuteState>, muted: bool) {
    state.0.store(muted, Ordering::Relaxed);
}

/// Plays panic.mp3 once — called by the frontend Easter egg trigger.
/// Emits `panic-audio-done` when playback finishes so the frontend
/// knows exactly when to cut to black.
#[tauri::command]
fn play_panic_audio(app: tauri::AppHandle, mute: tauri::State<MuteState>) {
    if mute.0.load(Ordering::Relaxed) {
        let _ = app.emit("panic-audio-done", ());
        return;
    }
    let Ok(resource_dir) = app.path().resource_dir() else {
        let _ = app.emit("panic-audio-done", ());
        return;
    };
    let muted = mute.0.clone();
    std::thread::spawn(move || {
        let path = resource_dir.join("assets").join("audio").join("panic.mp3");
        if let Err(e) = audio::play_mp3_blocking(&path, muted) {
            eprintln!("[audio] panic playback error: {e}");
        }
        let _ = app.emit("panic-audio-done", ());
    });
}

/// Returns true on first launch (flag file absent), false on subsequent runs.
#[tauri::command]
fn check_first_run(app: tauri::AppHandle) -> bool {
    let Ok(data_dir) = app.path().app_data_dir() else {
        return false;
    };
    let flag = data_dir.join("seen.flag");
    if flag.exists() {
        return false;
    }
    let _ = std::fs::create_dir_all(&data_dir);
    let _ = std::fs::write(&flag, "");
    true
}

fn state_to_rgb(state: &str) -> [u8; 3] {
    match state {
        "amber" => [245, 158, 11],
        "red" => [239, 68, 68],
        "stale" | "unavailable" => [107, 114, 128],
        _ => [34, 197, 94], // green
    }
}

fn set_tray_color(app: &tauri::AppHandle, state: &str) {
    let [r, g, b] = state_to_rgb(state);
    const SZ: u32 = 32;
    let cx = (SZ as f32 - 1.0) / 2.0;
    let outer_r = cx - 1.0;
    let inner_r = cx - 5.0;
    let mut rgba = Vec::with_capacity((SZ * SZ * 4) as usize);
    for y in 0..SZ {
        for x in 0..SZ {
            let dx = x as f32 - cx;
            let dy = y as f32 - cx;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist >= inner_r && dist <= outer_r {
                rgba.extend_from_slice(&[r, g, b, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    let icon = Image::new_owned(rgba, SZ, SZ);
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_icon(Some(icon.clone()));
    }
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_icon(icon);
    }
}

fn persist_window_size(app: &tauri::AppHandle, width: u32, height: u32) {
    let Ok(data_dir) = app.path().app_data_dir() else {
        return;
    };

    let mut settings = crate::settings::load_settings(&data_dir);
    if settings.window_width == Some(width) && settings.window_height == Some(height) {
        return;
    }

    settings.window_width = Some(width);
    settings.window_height = Some(height);

    if let Err(err) = crate::settings::save_settings(&data_dir, &settings) {
        eprintln!("[settings] failed to persist window size: {err}");
    }
}

fn persist_always_on_top(app: &tauri::AppHandle, always_on_top: bool) {
    let Ok(data_dir) = app.path().app_data_dir() else {
        return;
    };

    let mut settings = crate::settings::load_settings(&data_dir);
    if settings.always_on_top == always_on_top {
        return;
    }

    settings.always_on_top = always_on_top;

    if let Err(err) = crate::settings::save_settings(&data_dir, &settings) {
        eprintln!("[settings] failed to persist always-on-top: {err}");
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(debug_assertions)]
            eprintln!("[hallumeter] reading usage from ~/.claude/projects/");

            let cfg = app
                .path()
                .app_data_dir()
                .map(|d| crate::settings::load_settings(&d))
                .unwrap_or_default();

            // Shared state: current risk color, used to re-apply icon on window show.
            let color_state: Arc<Mutex<String>> = Arc::new(Mutex::new("green".to_string()));
            app.manage(AppColorState(color_state.clone()));

            // Tray menu
            let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let always_on_top_item = CheckMenuItem::with_id(
                app,
                "always_on_top",
                "Always on top",
                true,
                cfg.always_on_top,
                None::<&str>,
            )?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &always_on_top_item, &quit_item])?;

            let always_on_top_item_h = always_on_top_item.clone();
            let mut tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .tooltip("HalluMeter")
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                            // Re-apply dynamic icon — the taskbar may show the static
                            // bundle icon after the window was hidden.
                            let st = app.state::<AppColorState>().0.lock().unwrap().clone();
                            set_tray_color(app, &st);
                        }
                    }
                })
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                            let st = app.state::<AppColorState>().0.lock().unwrap().clone();
                            set_tray_color(app, &st);
                        }
                    }
                    "always_on_top" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let new_value = !w.is_always_on_top().unwrap_or(false);
                            let _ = w.set_always_on_top(new_value);
                            let _ = always_on_top_item_h.set_checked(new_value);
                            persist_always_on_top(app, new_value);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });

            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // Re-apply dynamic icon whenever the window gains focus (covers the
            // case where Windows reverts to the static bundle icon on un-hide).
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.set_always_on_top(cfg.always_on_top);
                if let (Some(width), Some(height)) = (cfg.window_width, cfg.window_height) {
                    let _ =
                        win.set_size(Size::Logical(LogicalSize::new(width as f64, height as f64)));
                }

                let app_h = app.app_handle().clone();
                win.on_window_event(move |event| match event {
                    tauri::WindowEvent::Focused(true) => {
                        let st = app_h.state::<AppColorState>().0.lock().unwrap().clone();
                        set_tray_color(&app_h, &st);
                    }
                    tauri::WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                        persist_window_size(&app_h, size.width, size.height);
                    }
                    _ => {}
                });
            }

            // Audio player — muted flag shared with set_mute command.
            let mut player = AudioPlayer::new();
            app.manage(MuteState(player.muted.clone()));

            let resource_dir = app.path().resource_dir()?;
            let app_handle = app.app_handle().clone();
            let color_state_thread = color_state;

            std::thread::spawn(move || {
                use crate::core::{interpolate_curve, risk_to_state, ContextPayload};
                use crate::sources::{
                    read_claude_jsonl_usage, read_codex_jsonl_usage, read_continue_usage,
                };

                let activity_secs = cfg.activity_window_mins * 60;
                let correlation_ms = cfg.continue_correlation_secs as i64 * 1000;
                let stale_timeout = Duration::from_secs(cfg.stale_timeout_secs);

                let mut last_data = Instant::now();
                let mut prev_state = String::new();
                let mut prev_fill_pct = -1.0_f64;
                let mut request_counter: u32 = 0;
                let mut play_threshold: u32 = player.rand_threshold() as u32;
                let mut prev_session = String::new();
                let mut panic_played = false;
                let mut last_variant_by_state: HashMap<String, u8> = HashMap::new();

                loop {
                    // Collect all active sources, annotate with risk_score.
                    let mut candidates: Vec<(String, f64, String, u64, i64, f64)> = [
                        read_claude_jsonl_usage(activity_secs, cfg.claude_max_files),
                        read_codex_jsonl_usage(activity_secs, cfg.codex_max_files),
                        read_continue_usage(activity_secs, correlation_ms),
                    ]
                    .into_iter()
                    .flatten()
                    .map(|(model, fill_pct, session, tokens, last_active_ms)| {
                        let adjusted = (fill_pct + cfg.context_overhead_pct).clamp(0.0, 100.0);
                        let risk_score = interpolate_curve(&model, adjusted);
                        (model, adjusted, session, tokens, last_active_ms, risk_score)
                    })
                    .collect();

                    // Select most recently active source.
                    // If two sources are within 60s of each other, prefer higher risk_score.
                    candidates.sort_by(|a, b| {
                        let time_diff = (a.4 - b.4).abs();
                        if time_diff <= 60_000 {
                            b.5.partial_cmp(&a.5).unwrap_or(std::cmp::Ordering::Equal)
                        } else {
                            b.4.cmp(&a.4)
                        }
                    });
                    let best = candidates.into_iter().next().map(
                        |(model, fill_pct, session, tokens, _, risk_score)| {
                            (model, fill_pct, session, tokens, risk_score)
                        },
                    );

                    if let Some((model, fill_pct, session, tokens, risk_score)) = best {
                        let state =
                            risk_to_state(risk_score, cfg.amber_threshold, cfg.red_threshold)
                                .to_string();
                        last_data = Instant::now();

                        // Reset panic one-shot when a new session is detected.
                        if session != prev_session && !prev_session.is_empty() {
                            panic_played = false;
                        }
                        prev_session = session.clone();

                        // Panic Easter egg — plays once when context hits 95%.
                        let panic_firing = fill_pct >= 95.0 && !panic_played;
                        if panic_firing {
                            panic_played = true;
                            player.play_panic(&resource_dir);
                        }

                        // Keep shared state in sync for window-focus re-application.
                        *color_state_thread.lock().unwrap() = state.clone();

                        let mut emit_variant: u8 = 0;

                        // Skip state-change audio when panic fires to avoid overlap.
                        if panic_firing {
                            prev_state = state.clone();
                            request_counter = 0;
                        } else if state != prev_state {
                            let v = player
                                .rand_1_5_avoiding(last_variant_by_state.get(&state).copied());
                            player.play(&state, v, &resource_dir);
                            last_variant_by_state.insert(state.clone(), v);
                            emit_variant = v;
                            prev_state = state.clone();
                            request_counter = 0;
                            play_threshold = player.rand_threshold() as u32;
                        } else if fill_pct > prev_fill_pct && prev_fill_pct >= 0.0 {
                            request_counter += 1;
                            if request_counter >= play_threshold {
                                let v = player
                                    .rand_1_5_avoiding(last_variant_by_state.get(&state).copied());
                                player.play(&state, v, &resource_dir);
                                last_variant_by_state.insert(state.clone(), v);
                                emit_variant = v;
                                request_counter = 0;
                                play_threshold = player.rand_threshold() as u32;
                            }
                        }

                        prev_fill_pct = fill_pct;

                        let _ = app_handle.emit(
                            "context-update",
                            ContextPayload {
                                fill_pct,
                                risk_score,
                                state: state.clone(),
                                model,
                                session,
                                variant: emit_variant,
                                tokens,
                            },
                        );
                        set_tray_color(&app_handle, &state);
                    } else if last_data.elapsed() >= stale_timeout {
                        if prev_state != "stale" {
                            prev_state = "stale".to_string();
                            prev_fill_pct = -1.0;
                            request_counter = 0;
                            play_threshold = player.rand_threshold() as u32;
                        }
                        *color_state_thread.lock().unwrap() = "stale".to_string();
                        let _ = app_handle.emit(
                            "context-update",
                            ContextPayload {
                                fill_pct: 0.0,
                                risk_score: 0.0,
                                state: "stale".to_string(),
                                model: "—".to_string(),
                                session: "—".to_string(),
                                variant: 0,
                                tokens: 0,
                            },
                        );
                        set_tray_color(&app_handle, "stale");
                    }
                    std::thread::sleep(Duration::from_secs(5));
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_mute,
            check_first_run,
            play_panic_audio
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
