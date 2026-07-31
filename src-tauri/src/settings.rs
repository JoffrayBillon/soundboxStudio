//! Persisted user preferences so the last-used MIDI device, audio output,
//! and profile survive an app restart instead of having to be reselected
//! every launch. Stored as a single `settings.json` next to the profiles
//! directory (`dirs::data_local_dir()/soundbox-studio/`).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

fn settings_path() -> AppResult<PathBuf> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| AppError::Io("Cannot determine local data directory".into()))?
        .join("soundbox-studio");

    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| AppError::Io(e.to_string()))?;
    }

    Ok(dir.join("settings.json"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Matched by name (not the transient numeric port index returned by
    /// `list_midi_devices`, which can shift between runs depending on OS
    /// enumeration order) against the freshly-fetched device list on
    /// startup.
    pub last_midi_device_name: Option<String>,
    /// cpal device name, also used as `AudioDevice.id`.
    pub last_audio_device_id: Option<String>,
    pub last_profile_id: Option<String>,
    /// Global output gain multiplier (1.0 = unity), applied on top of each
    /// pad's own volume/normalization gain. `#[serde(default = ...)]` so
    /// settings.json files saved before this field existed still parse,
    /// defaulting to unity instead of `f32::default()` (0.0, which would
    /// silently mute everything).
    #[serde(default = "default_master_volume")]
    pub master_volume: f32,
    /// When true, playback multiplies each pad's volume by its cached
    /// `PadConfig.normalized_gain` so louder/quieter source recordings play
    /// back at a more consistent perceived loudness.
    #[serde(default)]
    pub normalize_volumes: bool,
    /// Whether the app registers itself to launch automatically when the
    /// OS starts (see `commands::set_launch_at_startup`).
    #[serde(default)]
    pub launch_at_startup: bool,
    /// Whether closing the main window hides it to the system tray instead
    /// of quitting. Forced to `true` whenever `launch_at_startup` is
    /// enabled (see `commands::set_launch_at_startup` /
    /// `set_minimize_to_tray`) — otherwise an app that auto-launches at
    /// boot would have no way to be reached again after its first close.
    #[serde(default)]
    pub minimize_to_tray_on_close: bool,
    /// Whether the MIDI monitor debug panel is shown at the bottom of the
    /// pad grid. A dev/debug aid, off by default so it doesn't clutter the
    /// UI during normal use.
    #[serde(default)]
    pub show_midi_monitor: bool,
}

fn default_master_volume() -> f32 {
    1.0
}

// Manual `Default` impl (rather than `#[derive(Default)]`) so
// `master_volume` defaults to unity (1.0) instead of `f32::default()`
// (0.0, which would silently mute all playback) — this is the fallback
// used by `load()` when settings.json doesn't exist yet or fails to parse.
impl Default for AppSettings {
    fn default() -> Self {
        Self {
            last_midi_device_name: None,
            last_audio_device_id: None,
            last_profile_id: None,
            master_volume: default_master_volume(),
            normalize_volumes: false,
            launch_at_startup: false,
            minimize_to_tray_on_close: false,
            show_midi_monitor: false,
        }
    }
}

/// Loads settings from disk, falling back to defaults (all `None`) if the
/// file doesn't exist yet or fails to parse — this is a best-effort
/// convenience feature, never a hard requirement for the app to start.
pub fn load() -> AppSettings {
    settings_path()
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn save(settings: &AppSettings) -> AppResult<()> {
    let path = settings_path()?;
    let json = serde_json::to_string_pretty(settings)?;
    fs::write(path, json).map_err(|e| AppError::Io(e.to_string()))
}
