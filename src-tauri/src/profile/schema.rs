use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadConfig {
    /// Stable identity matching `LayoutPad.id` — sound assignment is always
    /// keyed by `id`, never by the raw `note`/CC number, so two pads can
    /// never accidentally share a sound just because their underlying MIDI
    /// numbers happen to coincide.
    pub id: String,
    /// Raw MIDI note/CC number, kept only for display/debugging.
    pub note: u8,
    pub label: String,
    pub color: String,
    pub sound_path: Option<String>,
    pub volume: f32,
    /// Per-sound normalization gain, computed once from the source file's
    /// peak amplitude when a sound is assigned (see `commands::compute_sound_gain`)
    /// and cached here so it doesn't need to be recomputed on every play.
    /// Only actually applied during playback when the user has enabled
    /// "normalize volumes" (`AppSettings.normalize_volumes` /
    /// `AppState.normalize_volumes`); otherwise ignored. Defaults to unity
    /// (1.0, i.e. no adjustment) for profiles saved before this field
    /// existed.
    #[serde(default = "default_normalized_gain")]
    pub normalized_gain: f32,
}

fn default_normalized_gain() -> f32 {
    1.0
}

impl Default for PadConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            note: 0,
            label: String::new(),
            color: "#1F2937".to_string(),
            sound_path: None,
            volume: 1.0,
            normalized_gain: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub device_name: String,
    pub rows: u8,
    pub cols: u8,
    pub pads: Vec<PadConfig>,
}

/// Identity of one grid pad, as reported by the device's `DeviceLayout`.
/// Used only to seed a new `Profile`'s pad list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridPadSpec {
    pub id: String,
    pub note: u8,
}

impl Profile {
    /// Creates a new profile whose pads match the real physical buttons
    /// reported by the device's `MidiController::layout()` (via
    /// `GridPadSpec`), instead of an arbitrary sequential range. `rows`/`cols`
    /// are stored purely as display metadata; the actual grid geometry comes
    /// from the device layout.
    pub fn new(
        name: String,
        device_name: String,
        rows: u8,
        cols: u8,
        grid_pads: Vec<GridPadSpec>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            device_name,
            rows,
            cols,
            pads: grid_pads
                .into_iter()
                .map(|spec| PadConfig {
                    id: spec.id,
                    note: spec.note,
                    ..Default::default()
                })
                .collect(),
        }
    }
}

