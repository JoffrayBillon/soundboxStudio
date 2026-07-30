use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::midi::devices::{
    json_controller::JsonController, launchpad_mini_mk3::LaunchpadMiniMk3, GenericController,
};
use crate::midi::traits::{MidiController, PadKind};

/// Identifies the concrete driver to instantiate for a device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    LaunchpadMiniMk3,
    Generic,
    /// Fully data-driven device: layout, button kind, and MIDI mapping are
    /// all described by `pads` instead of a dedicated Rust driver. Use this
    /// to support a new controller purely through a JSON file.
    Json,
}

/// The raw MIDI message shape a button reacts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiMessageType {
    Note,
    ControlChange,
}

/// Describes how to recognize a button's press/release from a raw MIDI
/// message: `channel` is 0-indexed (MIDI channel 1 = `0`), `number` is the
/// note number or CC number depending on `message_type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MidiInputSpec {
    pub message_type: MidiMessageType,
    pub channel: u8,
    pub number: u8,
}

/// One physical button of a JSON-described device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadDefinition {
    /// Stable, unique identity used for sound assignment (`PadConfig.id`).
    /// Must be unique across the whole device, even across different
    /// `input` message types (e.g. a Note-based pad and a
    /// Control-Change-based pad must never share an `id`, even if their
    /// raw `number` happens to coincide).
    pub id: String,
    pub row: u8,
    pub col: u8,
    pub kind: PadKind,
    pub input: MidiInputSpec,
    /// Page index this button selects when pressed. Only meaningful for
    /// `scene_button` / `control_button` kinds; ignored for `grid_pad`.
    #[serde(default)]
    pub page_index: Option<u8>,
}

/// Serialisable configuration for a MIDI controller device.
/// Stored as a local JSON file, one file per physical device / layout variant.
///
/// # Example JSON (built-in driver, parameterised)
/// ```json
/// {
///   "device_type": "launchpad_mini_mk3",
///   "name": "Launchpad Mini MK3",
///   "programmer_mode": true,
///   "page_change_notes": [89, 79, 69, 59, 49, 39, 29, 19]
/// }
/// ```
///
/// # Example JSON (fully data-driven, no dedicated driver needed)
/// ```json
/// {
///   "device_type": "json",
///   "name": "My Custom Controller",
///   "rows": 2,
///   "cols": 2,
///   "pads": [
///     { "id": "pad-1-1", "row": 0, "col": 0, "kind": "grid_pad",
///       "input": { "message_type": "note", "channel": 0, "number": 36 } },
///     { "id": "ctrl-1", "row": 1, "col": 0, "kind": "control_button",
///       "input": { "message_type": "control_change", "channel": 0, "number": 20 },
///       "page_index": 0 }
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfileConfig {
    pub device_type: DeviceType,
    /// Display name shown in the UI.
    pub name: String,
    /// Notes that trigger a `ChangePage` action instead of `PlaySound`.
    /// The page index equals the note's position in this list.
    /// Only used by `DeviceType::LaunchpadMiniMk3` / `DeviceType::Generic`.
    #[serde(default)]
    pub page_change_notes: Vec<u8>,
    /// When `true`, the driver sends the device's "enter Programmer Mode"
    /// SysEx on connection. Defaults to `true`. Only used by
    /// `DeviceType::LaunchpadMiniMk3`.
    #[serde(default = "default_true")]
    pub programmer_mode: bool,
    /// Grid dimensions, only used by `DeviceType::Json`.
    #[serde(default)]
    pub rows: u8,
    #[serde(default)]
    pub cols: u8,
    /// Full button list, only used by `DeviceType::Json`.
    #[serde(default)]
    pub pads: Vec<PadDefinition>,
}


fn default_true() -> bool {
    true
}

impl DeviceProfileConfig {
    // ── Built-in presets ──────────────────────────────────────────────────────

    /// Returns the default configuration for a Launchpad Mini MK3.
    /// Right-column scene buttons (top→bottom: 89, 79, …, 19) are mapped to
    /// pages 0–7.
    pub fn default_launchpad_mini_mk3() -> Self {
        Self {
            device_type: DeviceType::LaunchpadMiniMk3,
            name: "Launchpad Mini MK3".into(),
            programmer_mode: true,
            page_change_notes: vec![89, 79, 69, 59, 49, 39, 29, 19],
            rows: 0,
            cols: 0,
            pads: vec![],
        }
    }

    // ── Persistence ───────────────────────────────────────────────────────────

    /// Loads a `DeviceProfileConfig` from a local JSON file.
    pub fn load(path: &Path) -> AppResult<Self> {
        let json =
            std::fs::read_to_string(path).map_err(|e| AppError::Io(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| AppError::Profile(e.to_string()))
    }

    /// Serialises this config to a pretty-printed JSON file.
    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> AppResult<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json).map_err(|e| AppError::Io(e.to_string()))
    }

    // ── Factory ───────────────────────────────────────────────────────────────

    /// Instantiates the concrete `MidiController` driver described by this
    /// config. The box is `Send` because `MidiController: Send`.
    pub fn build_controller(&self) -> Box<dyn MidiController> {
        match self.device_type {
            DeviceType::LaunchpadMiniMk3 => Box::new(LaunchpadMiniMk3::new(
                self.page_change_notes.clone(),
                self.programmer_mode,
            )),
            DeviceType::Generic => Box::new(GenericController::new(
                self.name.clone(),
                self.page_change_notes.clone(),
            )),
            DeviceType::Json => Box::new(JsonController::new(
                self.name.clone(),
                self.rows,
                self.cols,
                self.pads.clone(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Loads `device-profiles/launchpad-mini-mk3.json` (repo root) and
    /// checks that it is behaviourally equivalent to the hardcoded
    /// `LaunchpadMiniMk3` driver: same layout (ids, positions, kinds) and
    /// same translation of raw MIDI messages into `DeviceAction`s. This is
    /// the exercise that validates the JSON schema actually captures
    /// everything a device needs (layout, button kind, MIDI mapping).
    fn load_lpmk3_json_profile() -> DeviceProfileConfig {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../device-profiles/launchpad-mini-mk3.json");
        DeviceProfileConfig::load(&path)
            .unwrap_or_else(|e| panic!("failed to load {}: {e}", path.display()))
    }

    #[test]
    fn json_profile_layout_matches_hardcoded_driver() {
        let json_controller = load_lpmk3_json_profile().build_controller();
        let hardcoded = DeviceProfileConfig::default_launchpad_mini_mk3().build_controller();

        let mut json_layout = json_controller.layout();
        let mut hardcoded_layout = hardcoded.layout();
        assert_eq!(json_layout.rows, hardcoded_layout.rows);
        assert_eq!(json_layout.cols, hardcoded_layout.cols);

        json_layout.pads.sort_by(|a, b| a.id.cmp(&b.id));
        hardcoded_layout.pads.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(json_layout.pads, hardcoded_layout.pads);

        // Every id must be unique — this is the whole point of the refactor.
        let ids: HashSet<&str> = json_layout.pads.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), json_layout.pads.len(), "duplicate pad id found");
    }

    #[test]
    fn json_profile_translates_grid_note_on_like_hardcoded_driver() {
        let json_controller = load_lpmk3_json_profile().build_controller();
        let hardcoded = DeviceProfileConfig::default_launchpad_mini_mk3().build_controller();

        // NoteOn channel 1, note 34 (row 3, col 4), velocity 100.
        let message = [0x90, 34, 100];
        assert_eq!(
            json_controller.translate_message(&message),
            hardcoded.translate_message(&message),
        );
    }

    #[test]
    fn json_profile_translates_top_row_control_change_like_hardcoded_driver() {
        let json_controller = load_lpmk3_json_profile().build_controller();
        let hardcoded = DeviceProfileConfig::default_launchpad_mini_mk3().build_controller();

        // Control Change channel 1, CC 93 (3rd top-row button), pressed.
        let message = [0xB0, 93, 127];
        assert_eq!(
            json_controller.translate_message(&message),
            hardcoded.translate_message(&message),
        );
    }

    #[test]
    fn json_profile_translates_scene_button_like_hardcoded_driver() {
        let json_controller = load_lpmk3_json_profile().build_controller();
        let hardcoded = DeviceProfileConfig::default_launchpad_mini_mk3().build_controller();

        // NoteOn channel 1, note 59 (scene button, row 5), pressed.
        let message = [0x90, 59, 127];
        assert_eq!(
            json_controller.translate_message(&message),
            hardcoded.translate_message(&message),
        );
    }
}

