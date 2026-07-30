use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::audio::engine::{AudioDevice, AudioEngine};
use crate::error::{AppError, AppResult};
use crate::midi::controller::{MidiDevice, MidiPort};
use crate::midi::device_profile::DeviceProfileConfig;
use crate::midi::devices::GenericController;
use crate::midi::traits::{DeviceAction, DeviceLayout, Rgb};
use crate::profile::{self, Profile};
use crate::state::AppState;

// ─── MIDI commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_midi_devices(_state: State<'_, AppState>) -> AppResult<Vec<MidiDevice>> {
    MidiPort::list_devices()
}

#[tauri::command]
pub fn connect_midi_device(
    device_id: usize,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> AppResult<()> {
    let controller_arc = Arc::clone(&state.device_controller);
    let mut port = state.midi.lock();
    port.connect(device_id, app_handle, controller_arc)?;

    // Send the device's init sequence (e.g., enter Programmer Mode). Best-effort.
    let init_msgs: Vec<Vec<u8>> = state
        .device_controller
        .lock()
        .as_ref()
        .map(|ctrl| ctrl.init_sequence())
        .unwrap_or_default();
    for msg in &init_msgs {
        let _ = port.send_bytes(msg);
    }
    Ok(())
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
    grid_pads: Vec<profile::GridPadSpec>,
    state: State<'_, AppState>,
) -> AppResult<Profile> {
    let prof = Profile::new(name, device_name, rows, cols, grid_pads);
    profile::save_profile(&prof)?;
    *state.active_profile.lock() = Some(prof.clone());
    Ok(prof)
}

#[tauri::command]
pub fn get_active_profile(state: State<'_, AppState>) -> Option<Profile> {
    state.active_profile.lock().clone()
}

// ─── Device controller commands ───────────────────────────────────────────────

/// Info about the currently loaded device driver, returned to the frontend.
#[derive(Serialize)]
pub struct DeviceControllerInfo {
    pub name: String,
    pub pad_notes: Vec<u8>,
}

/// Loads a `DeviceProfileConfig` from a local JSON file and activates the
/// corresponding controller driver.
#[tauri::command]
pub fn load_device_profile(path: String, state: State<'_, AppState>) -> AppResult<()> {
    let config = DeviceProfileConfig::load(std::path::Path::new(&path))?;
    *state.device_controller.lock() = Some(config.build_controller());
    Ok(())
}

/// Convenience command: activates the built-in Launchpad Mini MK3 driver
/// without needing an external JSON file.
#[tauri::command]
pub fn use_launchpad_mini_mk3(state: State<'_, AppState>) -> AppResult<()> {
    let ctrl = DeviceProfileConfig::default_launchpad_mini_mk3().build_controller();
    *state.device_controller.lock() = Some(ctrl);
    Ok(())
}

/// Activates a fallback `GenericController` for any device that has no
/// dedicated driver. Every MIDI note is treated as a sound-triggering pad.
#[tauri::command]
pub fn use_generic_controller(device_name: String, state: State<'_, AppState>) -> AppResult<()> {
    let ctrl = GenericController::new(device_name, vec![]);
    *state.device_controller.lock() = Some(Box::new(ctrl));
    Ok(())
}

/// Returns the name and pad-note list of the active device controller.
#[tauri::command]
pub fn get_device_controller_info(
    state: State<'_, AppState>,
) -> Option<DeviceControllerInfo> {
    state.device_controller.lock().as_ref().map(|ctrl| DeviceControllerInfo {
        name: ctrl.name().to_string(),
        pad_notes: ctrl.pad_notes(),
    })
}

/// Returns the full physical button layout (dimensions + per-note row/col
/// position) of the active device controller. This is the single source of
/// truth the frontend uses to render the pad grid dynamically.
#[tauri::command]
pub fn get_device_layout(state: State<'_, AppState>) -> Option<DeviceLayout> {
    state.device_controller.lock().as_ref().map(|ctrl| ctrl.layout())
}

/// Sets the LED colour of pad `note` on the physical device by sending the
/// appropriate MIDI/SysEx bytes through the open output port.
#[tauri::command]
pub fn set_pad_color(
    note: u8,
    r: u8,
    g: u8,
    b: u8,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let message = {
        let ctrl = state.device_controller.lock();
        match ctrl.as_ref() {
            Some(c) => c.build_color_message(note, Rgb { r, g, b }),
            None => return Err(AppError::NotConnected),
        }
    };
    if !message.is_empty() {
        state.midi.lock().send_bytes(&message)?;
    }
    Ok(())
}

/// Translates a raw MIDI message through the active device controller.
/// Useful for debugging and for headless testing.
#[tauri::command]
pub fn translate_midi_message(
    message: Vec<u8>,
    state: State<'_, AppState>,
) -> Option<DeviceAction> {
    state
        .device_controller
        .lock()
        .as_ref()
        .and_then(|ctrl| ctrl.translate_message(&message))
}
