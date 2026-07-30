use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use crate::audio::engine::{AudioDevice, AudioEngine};
use crate::error::{AppError, AppResult};
use crate::midi::controller::{MidiDevice, MidiPort};
use crate::midi::device_profile::DeviceProfileConfig;
use crate::midi::devices::GenericController;
use crate::midi::traits::{DeviceAction, DeviceLayout, Rgb};
use crate::profile::schema::PadConfig;
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
    {
        let mut port = state.midi.lock();
        port.connect(device_id, app_handle, controller_arc)?;
    }

    // A controller may already be active from a previous connection (e.g.
    // reconnecting) — (re)send its init sequence now that the output port
    // is open. If no controller is active yet, this is a no-op; the
    // frontend activates one right after connecting (see
    // `use_launchpad_mini_mk3` / `load_device_profile`), which re-sends it.
    send_controller_init(&state);
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

/// Sets the global output gain multiplier (1.0 = unity) applied on top of
/// every pad's own volume/normalization gain. Live-only — the caller (see
/// `useAppSettings`) is responsible for persisting it via
/// `save_app_settings` once the user finishes dragging the slider.
#[tauri::command]
pub fn set_master_volume(volume: f32, state: State<'_, AppState>) -> AppResult<()> {
    state.audio.lock().set_master_volume(volume);
    Ok(())
}

/// Toggles whether playback applies each pad's cached
/// `PadConfig::normalized_gain`. Live-only, same persistence caveat as
/// `set_master_volume`.
#[tauri::command]
pub fn set_normalize_volumes(enabled: bool, state: State<'_, AppState>) -> AppResult<()> {
    state.normalize_volumes.store(enabled, Ordering::Relaxed);
    Ok(())
}

/// Decodes `file_path` and returns the gain that would bring its peak
/// amplitude to a consistent target level (see
/// `audio::engine::analyze_normalization_gain`). Called once when a sound
/// is assigned to a pad; the result is cached as that pad's
/// `normalized_gain` rather than recomputed on every play.
#[tauri::command]
pub fn compute_sound_gain(file_path: String) -> AppResult<f32> {
    crate::audio::engine::analyze_normalization_gain(&file_path)
}

// ─── Profile commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_profiles() -> AppResult<Vec<Profile>> {
    profile::list_profiles()
}

#[tauri::command]
pub fn save_profile(prof: Profile, state: State<'_, AppState>) -> AppResult<()> {
    profile::save_profile(&prof)?;
    *state.active_profile.lock() = Some(prof);
    sync_all_pad_leds(&state);
    Ok(())
}

#[tauri::command]
pub fn load_profile(id: String, state: State<'_, AppState>) -> AppResult<Profile> {
    let prof = profile::load_profile(&id)?;
    *state.active_profile.lock() = Some(prof.clone());
    sync_all_pad_leds(&state);
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
    sync_all_pad_leds(&state);
    Ok(prof)
}

#[tauri::command]
pub fn get_active_profile(state: State<'_, AppState>) -> Option<Profile> {
    state.active_profile.lock().clone()
}

// ─── App settings (persisted device/profile selection) ───────────────────────

/// Returns the last-used MIDI device name, audio output id, and profile id
/// so the frontend can restore them on startup instead of the user having
/// to reconnect/reselect everything on every launch.
#[tauri::command]
pub fn load_app_settings() -> crate::settings::AppSettings {
    crate::settings::load()
}

#[tauri::command]
pub fn save_app_settings(settings: crate::settings::AppSettings) -> AppResult<()> {
    crate::settings::save(&settings)
}

// ─── Device controller commands ───────────────────────────────────────────────

/// Info about the currently loaded device driver, returned to the frontend.
#[derive(Serialize)]
pub struct DeviceControllerInfo {
    pub name: String,
    pub pad_notes: Vec<u8>,
}

/// Sends the active controller's `init_sequence()` (e.g. the Launchpad Mini
/// MK3's "enter Programmer Mode" SysEx) through the open MIDI output, then
/// pushes every pad's idle LED colour from the active profile (if any) —
/// entering Programmer Mode alone doesn't repaint the grid, so without this
/// the physical LEDs stay whatever the device's own standalone layout last
/// showed, which looked like "Programmer Mode isn't really being enabled".
/// Best-effort/no-op when there's no controller or no output connected yet —
/// callers don't need to special-case either situation.
fn send_controller_init(state: &State<'_, AppState>) {
    let init_msgs: Vec<Vec<u8>> = state
        .device_controller
        .lock()
        .as_ref()
        .map(|ctrl| ctrl.init_sequence())
        .unwrap_or_default();
    {
        let mut port = state.midi.lock();
        for msg in &init_msgs {
            let _ = port.send_bytes(msg);
        }
    }
    sync_all_pad_leds(state);
}

/// Sends the active controller's `exit_sequence()` (e.g. the Launchpad Mini
/// MK3's "leave Programmer Mode, return to Session layout" SysEx) through
/// the open MIDI output. Called once when the app quits (see `lib.rs`) so
/// the device isn't left stuck in Programmer Mode after Soundbox Studio
/// closes. Best-effort/no-op when there's no controller or output.
pub(crate) fn send_controller_exit(state: &State<'_, AppState>) {
    let exit_msgs: Vec<Vec<u8>> = state
        .device_controller
        .lock()
        .as_ref()
        .map(|ctrl| ctrl.exit_sequence())
        .unwrap_or_default();
    let mut port = state.midi.lock();
    for msg in &exit_msgs {
        let _ = port.send_bytes(msg);
    }
}

/// Pushes every pad's idle LED colour (off if unassigned, otherwise
/// `idle_pad_color`) from the active profile to the physical device, keyed
/// by matching each of the controller's `pad_notes()` against a profile pad
/// with the same `note`. Pads currently playing are skipped so this doesn't
/// fight their pulse thread. A no-op if there's no active controller.
fn sync_all_pad_leds(state: &State<'_, AppState>) {
    let Some(pad_notes) = state
        .device_controller
        .lock()
        .as_ref()
        .map(|ctrl| ctrl.pad_notes())
    else {
        return;
    };

    let pads_by_note: HashMap<u8, PadConfig> = state
        .active_profile
        .lock()
        .as_ref()
        .map(|p| p.pads.iter().map(|pad| (pad.note, pad.clone())).collect())
        .unwrap_or_default();
    let playing_ids: HashSet<String> = state.playing_pads.lock().keys().cloned().collect();

    for note in pad_notes {
        let color = match pads_by_note.get(&note) {
            Some(pad) if playing_ids.contains(&pad.id) => continue,
            Some(pad) => idle_pad_color(pad),
            None => Rgb::BLACK,
        };
        send_pad_color(state, note, color);
    }
}

/// Loads a `DeviceProfileConfig` from a local JSON file and activates the
/// corresponding controller driver.
#[tauri::command]
pub fn load_device_profile(path: String, state: State<'_, AppState>) -> AppResult<()> {
    let config = DeviceProfileConfig::load(std::path::Path::new(&path))?;
    *state.device_controller.lock() = Some(config.build_controller());
    send_controller_init(&state);
    Ok(())
}

/// Convenience command: activates the built-in Launchpad Mini MK3 driver
/// without needing an external JSON file.
#[tauri::command]
pub fn use_launchpad_mini_mk3(state: State<'_, AppState>) -> AppResult<()> {
    let ctrl = DeviceProfileConfig::default_launchpad_mini_mk3().build_controller();
    *state.device_controller.lock() = Some(ctrl);
    send_controller_init(&state);
    Ok(())
}

/// Activates a fallback `GenericController` for any device that has no
/// dedicated driver. Every MIDI note is treated as a sound-triggering pad.
#[tauri::command]
pub fn use_generic_controller(device_name: String, state: State<'_, AppState>) -> AppResult<()> {
    let ctrl = GenericController::new(device_name, vec![]);
    *state.device_controller.lock() = Some(Box::new(ctrl));
    send_controller_init(&state);
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

/// Re-sends the active controller's `init_sequence()` (e.g. re-asserts
/// Programmer Mode on a Launchpad). There is no documented way to *query* a
/// Launchpad's current mode over MIDI — switching modes is write-only — so
/// this is a manual "just in case" resync for when the device was reset by
/// a physical button combo, power cycle, or USB replug that the app didn't
/// otherwise detect as a real disconnect.
#[tauri::command]
pub fn resync_device(state: State<'_, AppState>) -> AppResult<()> {
    if state.device_controller.lock().is_none() {
        return Err(AppError::NotConnected);
    }
    send_controller_init(&state);
    Ok(())
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
    if state.device_controller.lock().is_none() {
        return Err(AppError::NotConnected);
    }
    send_pad_color(&state, note, Rgb { r, g, b });
    Ok(())
}

// ─── Pad playback (trigger / stop / LED feedback) ─────────────────────────────

const PLAYBACK_PULSE_COLOR: Rgb = Rgb { r: 0, g: 255, b: 0 };
/// Full breathe-in/breathe-out cycle duration, matching the frontend's
/// `animate-pulse` cadence (Tailwind's default pulse animation is a 2s
/// cycle) so the physical LED and the on-screen highlight pulse in sync.
const PLAYBACK_PULSE_PERIOD: Duration = Duration::from_millis(2000);
/// How often the LED brightness is updated while pulsing. Small enough to
/// look smooth and to react quickly once playback stops.
const PLAYBACK_PULSE_STEP: Duration = Duration::from_millis(40);

/// LED colour for a pad that has a sound assigned but isn't currently
/// playing. A single shared default for now — a future feature will let
/// this be customized per pad.
const DEFAULT_ASSIGNED_COLOR: Rgb = Rgb { r: 200, g: 200, b: 200 };

/// The LED colour a pad should sit at when it's idle (not playing): off if
/// no sound is assigned, otherwise the default "assigned" indicator colour.
fn idle_pad_color(pad: &PadConfig) -> Rgb {
    if pad.sound_path.is_some() {
        DEFAULT_ASSIGNED_COLOR
    } else {
        Rgb::BLACK
    }
}

/// Emitted on the `pad_playback_changed` Tauri event whenever a pad starts or
/// stops playing its sound (from a click, a physical press, a manual
/// re-click, or the sound finishing on its own) — the frontend uses this as
/// the single source of truth for which pads to highlight as "playing".
#[derive(Clone, Serialize)]
struct PadPlaybackChanged {
    pad_id: String,
    playing: bool,
}

/// Sends the SysEx/MIDI bytes (if any) to set pad `note`'s LED to `color`.
/// Silently does nothing when the active driver has no LED support or no
/// MIDI output is open — LED feedback is always best-effort.
fn send_pad_color(state: &State<'_, AppState>, note: u8, color: Rgb) {
    let message = match state.device_controller.lock().as_ref() {
        Some(ctrl) => ctrl.build_color_message(note, color),
        None => return,
    };
    if !message.is_empty() {
        let _ = state.midi.lock().send_bytes(&message);
    }
}

/// Sets pad `pad_id`'s LED to its idle colour (off if it has no sound
/// assigned, otherwise the default "assigned" indicator colour) based on
/// its current entry in the active profile. Called by the frontend right
/// after assigning or removing a pad's sound, so the LED reflects the new
/// state immediately instead of only updating on the next play/stop cycle.
/// A no-op (not an error) if the pad isn't playing and simply doesn't exist
/// yet, or if there's no device/output connected.
#[tauri::command]
pub fn sync_pad_led(pad_id: String, state: State<'_, AppState>) -> AppResult<()> {
    // Don't fight a pad's own blink thread if it happens to be playing.
    if state.playing_pads.lock().contains_key(&pad_id) {
        return Ok(());
    }
    let Some(pad) = state
        .active_profile
        .lock()
        .as_ref()
        .and_then(|p| p.pads.iter().find(|p| p.id == pad_id).cloned())
    else {
        return Ok(());
    };
    send_pad_color(&state, pad.note, idle_pad_color(&pad));
    Ok(())
}

/// Parses a `"#RRGGBB"` (or `"RRGGBB"`) hex colour string into an `Rgb`,
/// falling back to black if malformed. Not used yet — reserved for the
/// upcoming feature letting each pad customize its own idle/assigned LED
/// colour (see `idle_pad_color`).
#[allow(dead_code)]
fn hex_to_rgb(hex: &str) -> Rgb {
    let hex = hex.trim_start_matches('#');
    let channel = |range: std::ops::Range<usize>| {
        hex.get(range).and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0)
    };
    Rgb {
        r: channel(0..2),
        g: channel(2..4),
        b: channel(4..6),
    }
}

/// Clears the "currently playing" bookkeeping for `pad_id` (signalling its
/// blink thread to stop and revert the LED to the pad's static colour) and
/// notifies the frontend. Shared by both the manual re-click stop path
/// (`trigger_pad`) and the natural-completion watcher (see `lib.rs`).
pub(crate) fn stop_pad_playback(app_handle: &tauri::AppHandle, pad_id: &str) {
    let state = app_handle.state::<AppState>();
    if let Some(flag) = state.playing_pads.lock().remove(pad_id) {
        flag.store(false, Ordering::Relaxed);
    }
    let _ = app_handle.emit(
        "pad_playback_changed",
        PadPlaybackChanged {
            pad_id: pad_id.to_string(),
            playing: false,
        },
    );
}

/// Toggles playback for `pad_id`: if it isn't currently playing, starts its
/// assigned sound and pulses its LED green (smooth breathing brightness,
/// not a hard blink) until the sound ends; if it is already playing, this
/// click stops it instead of layering another copy of
/// the sound on top. Used for both on-screen clicks and physical pad presses.
#[tauri::command]
pub fn trigger_pad(
    pad_id: String,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> AppResult<bool> {
    // Already playing → this click means "stop".
    if state.playing_pads.lock().contains_key(&pad_id) {
        state.audio.lock().stop_pad(pad_id.clone())?;
        stop_pad_playback(&app_handle, &pad_id);
        return Ok(false);
    }

    let Some(pad) = state
        .active_profile
        .lock()
        .as_ref()
        .and_then(|p| p.pads.iter().find(|p| p.id == pad_id).cloned())
    else {
        return Ok(false);
    };
    let Some(sound_path) = pad.sound_path.clone() else {
        return Ok(false);
    };

    // Only actually apply the cached normalization gain when the user has
    // enabled "normalize volumes" — otherwise a pad's own `volume` (default
    // unity) is used unmodified, same as before this feature existed.
    let effective_volume = if state.normalize_volumes.load(Ordering::Relaxed) {
        pad.volume * pad.normalized_gain
    } else {
        pad.volume
    };

    state
        .audio
        .lock()
        .play_for_pad(pad_id.clone(), &sound_path, effective_volume)?;

    let flag = Arc::new(AtomicBool::new(true));
    state
        .playing_pads
        .lock()
        .insert(pad_id.clone(), Arc::clone(&flag));
    let _ = app_handle.emit(
        "pad_playback_changed",
        PadPlaybackChanged {
            pad_id: pad_id.clone(),
            playing: true,
        },
    );

    let note = pad.note;
    let idle_color = idle_pad_color(&pad);
    let blink_app_handle = app_handle.clone();
    thread::spawn(move || {
        let state = blink_app_handle.state::<AppState>();
        let start = std::time::Instant::now();
        let period_ms = PLAYBACK_PULSE_PERIOD.as_millis() as f64;
        while flag.load(Ordering::Relaxed) {
            // Smooth breathing brightness: a raised cosine eases from 0 to 1
            // and back to 0 once per `PLAYBACK_PULSE_PERIOD`, instead of a
            // hard on/off toggle.
            let elapsed_ms = start.elapsed().as_millis() as f64 % period_ms;
            let phase = elapsed_ms / period_ms; // 0.0..1.0
            let brightness = 0.5 - 0.5 * (phase * std::f64::consts::TAU).cos();
            let pulsed = Rgb {
                r: (PLAYBACK_PULSE_COLOR.r as f64 * brightness).round() as u8,
                g: (PLAYBACK_PULSE_COLOR.g as f64 * brightness).round() as u8,
                b: (PLAYBACK_PULSE_COLOR.b as f64 * brightness).round() as u8,
            };
            send_pad_color(&state, note, pulsed);
            thread::sleep(PLAYBACK_PULSE_STEP);
        }
        send_pad_color(&state, note, idle_color);
    });

    Ok(true)
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
