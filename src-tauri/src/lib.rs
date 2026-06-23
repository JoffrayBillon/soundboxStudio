mod audio;
mod commands;
mod error;
mod midi;
mod profile;
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
            commands::list_profiles,
            commands::save_profile,
            commands::load_profile,
            commands::delete_profile,
            commands::create_profile,
            commands::get_active_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
