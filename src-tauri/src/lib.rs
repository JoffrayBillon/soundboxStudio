mod audio;
mod commands;
mod error;
mod midi;
mod profile;
mod settings;
mod state;

use std::sync::atomic::Ordering;

use state::AppState;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            app.manage(AppState::new());

            // Re-applies the persisted master volume / normalize-volumes /
            // launch-at-startup / minimize-to-tray preferences (see
            // `settings.rs`) right away, so behaviour is correct from the
            // very first moment even if the frontend never re-sends them.
            let state = app.state::<AppState>();
            let saved_settings = settings::load();
            state.audio.lock().set_master_volume(saved_settings.master_volume);
            state
                .normalize_volumes
                .store(saved_settings.normalize_volumes, Ordering::Relaxed);
            state
                .launch_at_startup
                .store(saved_settings.launch_at_startup, Ordering::Relaxed);
            state.minimize_to_tray_on_close.store(
                saved_settings.minimize_to_tray_on_close || saved_settings.launch_at_startup,
                Ordering::Relaxed,
            );

            // Reconciles the actual OS-level autostart registration with
            // our saved preference on every launch — the user could have
            // removed it directly from the OS's own startup-apps settings
            // without going through our UI, or reinstalled the app, so
            // this can't just be assumed to still match. Both calls are
            // idempotent no-ops if already in the desired state.
            let autolaunch = app.autolaunch();
            let _ = if saved_settings.launch_at_startup {
                autolaunch.enable()
            } else {
                autolaunch.disable()
            };

            // Watches for pads whose sound finishes playing on its own
            // (as opposed to a manual re-click) and reverts their LED /
            // "currently playing" bookkeeping accordingly.
            let finished_rx = state.audio.lock().take_finished_receiver();
            let app_handle = app.handle().clone();
            if let Some(finished_rx) = finished_rx {
                std::thread::spawn(move || {
                    for pad_id in finished_rx {
                        commands::stop_pad_playback(&app_handle, &pad_id);
                    }
                });
            }

            // System tray icon: lets the app keep running (MIDI/audio
            // engine, Programmer Mode, etc. all stay live) after the main
            // window is closed when "minimize to tray" is enabled, with a
            // way to bring the window back or actually quit.
            let show_item = MenuItem::with_id(app, "show", "Afficher Soundbox Studio", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
            let tray_menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap())
                .menu(&tray_menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Hides (instead of closing) the main window when "minimize to
            // tray" is enabled, so the app keeps running in the background
            // — the only way to actually quit is then the tray's "Quitter"
            // item (or the OS killing the process). When the setting is
            // off, this is a no-op and the window closes normally, which
            // triggers the `RunEvent::ExitRequested` handler below.
            if let Some(window) = app.get_webview_window("main") {
                let close_app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let state = close_app_handle.state::<AppState>();
                        if state.minimize_to_tray_on_close.load(Ordering::Relaxed) {
                            api.prevent_close();
                            if let Some(window) = close_app_handle.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_midi_devices,
            commands::connect_midi_device,
            commands::disconnect_midi_device,
            commands::list_audio_output_devices,
            commands::set_audio_output_device,
            commands::play_sound_file,
            commands::stop_all_sounds,
            commands::set_master_volume,
            commands::set_normalize_volumes,
            commands::compute_sound_gain,
            commands::trigger_pad,
            commands::sync_pad_led,
            commands::list_profiles,
            commands::save_profile,
            commands::load_profile,
            commands::delete_profile,
            commands::create_profile,
            commands::get_active_profile,
            // Device controller
            commands::load_device_profile,
            commands::use_launchpad_mini_mk3,
            commands::use_generic_controller,
            commands::get_device_controller_info,
            commands::get_device_layout,
            commands::set_pad_color,
            commands::translate_midi_message,
            commands::resync_device,
            commands::load_app_settings,
            commands::save_app_settings,
            commands::set_launch_at_startup,
            commands::set_minimize_to_tray,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Leave Programmer Mode (return the Launchpad to its normal
            // Session layout) when the app quits, instead of stranding the
            // device stuck in Programmer Mode after Soundbox Studio closes.
            // Runs synchronously so the SysEx is actually sent before the
            // process exits, rather than racing a background thread.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                commands::send_controller_exit(&state);
            }
        });
}
