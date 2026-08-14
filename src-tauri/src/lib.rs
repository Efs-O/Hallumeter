mod audio;
mod core;
mod monitor;
mod settings;
mod sources;

#[cfg(test)]
mod tests;

use audio::AudioPlayer;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
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
        emit_panic_audio_done(&app);
        return;
    }
    let Ok(resource_dir) = app.path().resource_dir() else {
        eprintln!("[audio] panic playback skipped: could not resolve resource directory");
        emit_panic_audio_done(&app);
        return;
    };
    let muted = mute.0.clone();
    std::thread::spawn(move || {
        let path = resource_dir.join("assets").join("audio").join("panic.mp3");
        if let Err(e) = audio::play_mp3_blocking(&path, muted) {
            eprintln!("[audio] panic playback error: {e}");
        }
        emit_panic_audio_done(&app);
    });
}

fn emit_panic_audio_done(app: &tauri::AppHandle) {
    if let Err(error) = app.emit("panic-audio-done", ()) {
        eprintln!("[audio] failed to emit panic completion: {error}");
    }
}

/// Returns true on first launch (flag file absent), false on subsequent runs.
#[tauri::command]
fn check_first_run(app: tauri::AppHandle) -> Result<bool, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not resolve application data directory: {error}"))?;
    let flag = data_dir.join("seen.flag");
    if flag.exists() {
        return Ok(false);
    }
    std::fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create {}: {error}", data_dir.display()))?;
    std::fs::write(&flag, "")
        .map_err(|error| format!("Could not create {}: {error}", flag.display()))?;
    Ok(true)
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
        if let Err(error) = tray.set_icon(Some(icon.clone())) {
            eprintln!("[window] failed to update tray icon: {error}");
        }
    }
    if let Some(win) = app.get_webview_window("main") {
        if let Err(error) = win.set_icon(icon) {
            eprintln!("[window] failed to update window icon: {error}");
        }
    }
}

fn refresh_window_icon(app: &tauri::AppHandle) {
    match app.state::<AppColorState>().0.lock() {
        Ok(state) => set_tray_color(app, &state),
        Err(error) => eprintln!("[window] failed to read current icon state: {error}"),
    }
}

fn show_and_focus(window: &tauri::WebviewWindow) {
    if let Err(error) = window.show() {
        eprintln!("[window] failed to show main window: {error}");
        return;
    }
    if let Err(error) = window.set_focus() {
        eprintln!("[window] failed to focus main window: {error}");
    }
}

fn persist_window_size(app: &tauri::AppHandle, width: u32, height: u32) {
    let Ok(data_dir) = app.path().app_data_dir() else {
        eprintln!("[settings] could not resolve application data directory for window size");
        return;
    };

    let mut settings = match crate::settings::load_settings(&data_dir) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!(
                "[settings] refused to overwrite settings while persisting window size: {error}"
            );
            return;
        }
    };
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
        eprintln!("[settings] could not resolve application data directory for always-on-top");
        return;
    };

    let mut settings = match crate::settings::load_settings(&data_dir) {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!(
                "[settings] refused to overwrite settings while persisting always-on-top: {error}"
            );
            return;
        }
    };
    if settings.always_on_top == always_on_top {
        return;
    }

    settings.always_on_top = always_on_top;

    if let Err(err) = crate::settings::save_settings(&data_dir, &settings) {
        eprintln!("[settings] failed to persist always-on-top: {err}");
    }
}

pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                show_and_focus(&w);
                refresh_window_icon(app);
            }
        }));
    }

    builder
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(debug_assertions)]
            eprintln!("[hallumeter] reading usage from ~/.claude/projects/");

            let (cfg, mut setup_diagnostic) = match app.path().app_data_dir() {
                Ok(data_dir) => match crate::settings::load_settings(&data_dir) {
                    Ok(settings) => (settings, None),
                    Err(error) => {
                        eprintln!("[settings] {error}");
                        (crate::settings::UserSettings::default(), Some(error))
                    }
                },
                Err(error) => {
                    let diagnostic =
                        format!("Could not resolve application data directory: {error}");
                    eprintln!("[settings] {diagnostic}");
                    (crate::settings::UserSettings::default(), Some(diagnostic))
                }
            };

            let desktop_dir = match app.path().desktop_dir() {
                Ok(path) => Some(path),
                Err(error) => {
                    eprintln!("[settings] could not resolve desktop directory: {error}");
                    None
                }
            };
            let continue_bridge_yaml =
                match crate::settings::resolve_continue_bridge_yaml_path(&cfg, desktop_dir) {
                    Ok(path) => path,
                    Err(error) => {
                        eprintln!("[settings] {error}");
                        if setup_diagnostic.is_none() {
                            setup_diagnostic = Some(error);
                        }
                        None
                    }
                };

            // Shared state: current risk color, used to re-apply icon on window show.
            let color_state: Arc<Mutex<String>> = Arc::new(Mutex::new("unavailable".to_string()));
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
                            show_and_focus(&w);
                            // Re-apply dynamic icon — the taskbar may show the static
                            // bundle icon after the window was hidden.
                            refresh_window_icon(app);
                        }
                    }
                })
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            show_and_focus(&w);
                            refresh_window_icon(app);
                        }
                    }
                    "always_on_top" => {
                        if let Some(w) = app.get_webview_window("main") {
                            match w.is_always_on_top() {
                                Ok(current_value) => {
                                    let new_value = !current_value;
                                    if let Err(error) = w.set_always_on_top(new_value) {
                                        eprintln!("[window] failed to update always-on-top: {error}");
                                    } else {
                                        if let Err(error) = always_on_top_item_h.set_checked(new_value) {
                                            eprintln!("[window] failed to update always-on-top menu: {error}");
                                        }
                                        persist_always_on_top(app, new_value);
                                    }
                                }
                                Err(error) => eprintln!("[window] failed to read always-on-top: {error}"),
                            }
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
                if let Err(error) = win.set_always_on_top(cfg.always_on_top) {
                    eprintln!("[window] failed to apply always-on-top setting: {error}");
                }
                if let (Some(width), Some(height)) = (cfg.window_width, cfg.window_height) {
                    if let Err(error) =
                        win.set_size(Size::Logical(LogicalSize::new(width as f64, height as f64)))
                    {
                        eprintln!("[window] failed to restore saved size: {error}");
                    }
                }

                let app_h = app.app_handle().clone();
                win.on_window_event(move |event| match event {
                    tauri::WindowEvent::Focused(true) => {
                        refresh_window_icon(&app_h);
                    }
                    tauri::WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                        persist_window_size(&app_h, size.width, size.height);
                    }
                    _ => {}
                });
            }

            // Audio player — muted flag shared with set_mute command.
            let player = AudioPlayer::new();
            app.manage(MuteState(player.muted.clone()));

            let resource_dir = app.path().resource_dir().map_err(|error| {
                eprintln!("[app] could not resolve resource directory: {error}");
                error
            })?;
            let app_handle = app.app_handle().clone();
            let color_state_thread = color_state;

            std::thread::spawn(move || {
                crate::monitor::Monitor::new(
                    app_handle,
                    color_state_thread,
                    resource_dir,
                    cfg,
                    continue_bridge_yaml,
                    setup_diagnostic,
                    player,
                )
                .run();
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
