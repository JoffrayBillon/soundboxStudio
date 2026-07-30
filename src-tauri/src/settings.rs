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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// Matched by name (not the transient numeric port index returned by
    /// `list_midi_devices`, which can shift between runs depending on OS
    /// enumeration order) against the freshly-fetched device list on
    /// startup.
    pub last_midi_device_name: Option<String>,
    /// cpal device name, also used as `AudioDevice.id`.
    pub last_audio_device_id: Option<String>,
    pub last_profile_id: Option<String>,
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
