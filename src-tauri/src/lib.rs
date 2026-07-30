mod audio;
mod commands;
mod error;
mod midi;
mod profile;
mod settings;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            app.manage(AppState::new());

            // Re-applies the persisted master volume / normalize-volumes
            // preference (see `settings.rs`) to the live audio engine right
            // away, so playback is correct from the very first sound even
            // if the frontend never re-sends them itself.
            let state = app.state::<AppState>();
            let saved_settings = settings::load();
            state.audio.lock().set_master_volume(saved_settings.master_volume);
            state
                .normalize_volumes
                .store(saved_settings.normalize_volumes, std::sync::atomic::Ordering::Relaxed);

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
