use tauri::State;

use crate::audio::engine::{AudioDevice, AudioEngine};
use crate::error::AppResult;
use crate::midi::controller::{MidiController, MidiDevice};
use crate::profile::{self, Profile};
use crate::state::AppState;

// ─── MIDI commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_midi_devices(_state: State<'_, AppState>) -> AppResult<Vec<MidiDevice>> {
    MidiController::list_devices()
}

#[tauri::command]
pub fn connect_midi_device(
    device_id: usize,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> AppResult<()> {
    state.midi.lock().connect(device_id, app_handle)
}

#[tauri::command]
pub fn disconnect_midi_device(state: State<'_, AppState>) -> AppResult<()> {
    state.midi.lock().disconnect();
    Ok(())
}

// ─── Audio commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_audio_output_devices(_state: State<'_, AppState>) -> AppResult<Vec<AudioDevice>> {
    AudioEngine::list_output_devices()
}

#[tauri::command]
pub fn set_audio_output_device(_device_id: String, _state: State<'_, AppState>) -> AppResult<()> {
    // Selecting a non-default output device requires rebuilding the audio
    // thread with the chosen cpal device. Planned for a future release.
    Ok(())
}

#[tauri::command]
pub fn play_sound_file(
    file_path: String,
    volume: f32,
    state: State<'_, AppState>,
) -> AppResult<()> {
    state.audio.lock().play_sound_file(&file_path, volume)
}

#[tauri::command]
pub fn stop_all_sounds(state: State<'_, AppState>) -> AppResult<()> {
    state.audio.lock().stop_all_sounds()
}

// ─── Profile commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_profiles() -> AppResult<Vec<Profile>> {
    profile::list_profiles()
}

#[tauri::command]
pub fn save_profile(prof: Profile) -> AppResult<()> {
    profile::save_profile(&prof)
}

#[tauri::command]
pub fn load_profile(id: String, state: State<'_, AppState>) -> AppResult<Profile> {
    let prof = profile::load_profile(&id)?;
    *state.active_profile.lock() = Some(prof.clone());
    Ok(prof)
}

#[tauri::command]
pub fn delete_profile(id: String) -> AppResult<()> {
    profile::delete_profile(&id)
}

#[tauri::command]
pub fn create_profile(
    name: String,
    device_name: String,
    rows: u8,
    cols: u8,
    state: State<'_, AppState>,
) -> AppResult<Profile> {
    let prof = Profile::new(name, device_name, rows, cols);
    profile::save_profile(&prof)?;
    *state.active_profile.lock() = Some(prof.clone());
    Ok(prof)
}

#[tauri::command]
pub fn get_active_profile(state: State<'_, AppState>) -> Option<Profile> {
    state.active_profile.lock().clone()
}
