use serde::{Deserialize, Serialize};

/// 8-bit RGB color (0–255 per channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[allow(dead_code)]
impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// High-level, device-agnostic action produced from a raw MIDI message.
/// The frontend listens for `device_action` Tauri events carrying this type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum DeviceAction {
    /// Trigger the sound bound to `pad_id` at the given `velocity`.
    /// `note` is the raw MIDI/CC number, kept only for the MIDI monitor and
    /// LED feedback — sound assignment must always key off `pad_id`, since
    /// raw numbers can collide across message types (e.g. a Note and a
    /// Control Change sharing the same number) once devices are described
    /// generically via JSON.
    PlaySound { pad_id: String, note: u8, velocity: u8 },
    /// Release the sound bound to `pad_id`.
    StopSound { pad_id: String, note: u8 },
    /// Switch to `page_index` in the active profile.
    ChangePage { page_index: u8 },
}

/// The physical role of a button on the controller, used by the frontend to
/// render each cell appropriately (sound pad vs. non-assignable control button).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PadKind {
    /// A sound-triggering pad (assignable to a sample).
    GridPad,
    /// A page-switch control button on the side of the grid (not assignable
    /// to a sound).
    SceneButton,
    /// A page-switch control button above the grid (not assignable to a
    /// sound). Distinguished from `SceneButton` because some devices (e.g.
    /// the Launchpad Mini MK3) send it as Control Change rather than Note.
    ControlButton,
}

/// Position and role of a single button within the device's physical layout.
/// `row`/`col` are 0-indexed, top-left origin, matching CSS grid conventions
/// (the frontend adds 1 when writing `grid-row` / `grid-column`).
///
/// `id` is the button's stable, unique identity used for sound assignment
/// (`PadConfig.id`) — it must never collide with another pad's `id` on the
/// same device, even if their raw `note` values happen to coincide (e.g. a
/// Note-based pad and a Control-Change-based pad sharing the same number).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutPad {
    pub id: String,
    pub note: u8,
    pub row: u8,
    pub col: u8,
    pub kind: PadKind,
}

/// The full physical button layout of a MIDI controller, as computed by its
/// `MidiController` driver. This is the single source of truth the frontend
/// uses to render the pad grid — no row/column assumption is hardcoded in
/// the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceLayout {
    pub rows: u8,
    pub cols: u8,
    pub pads: Vec<LayoutPad>,
}

/// Hardware abstraction for a MIDI controller.
///
/// Implement this trait to support any MIDI device layout without touching
/// the rest of the application. The trait is object-safe and `Send`-bound so
/// it can be boxed and stored across threads.
pub trait MidiController: Send {
    /// Human-readable device name shown in the UI.
    fn name(&self) -> &str;

    /// Translate a raw MIDI message (as received from `midir`) into a
    /// `DeviceAction`, returning `None` for messages the device does not map
    /// to any action (e.g. aftertouch, pitch-bend, internal clock).
    fn translate_message(&self, message: &[u8]) -> Option<DeviceAction>;

    /// Build the raw MIDI/SysEx bytes that set pad `note` to `color` on the
    /// physical device. Returns an empty `Vec` when the device has no LED
    /// feedback support.
    fn build_color_message(&self, note: u8, color: Rgb) -> Vec<u8>;

    /// Optional initialization sequence to send to the device right after
    /// connection (e.g., switching a Launchpad to Programmer Mode).
    /// Each inner `Vec<u8>` is one independent MIDI or SysEx message.
    fn init_sequence(&self) -> Vec<Vec<u8>> {
        vec![]
    }

    /// Optional teardown sequence sent when the app quits (e.g., switching a
    /// Launchpad back out of Programmer Mode so it returns to normal
    /// standalone operation instead of being left stuck showing whatever the
    /// last LED state was). Each inner `Vec<u8>` is one independent MIDI or
    /// SysEx message. Default: nothing to undo.
    fn exit_sequence(&self) -> Vec<Vec<u8>> {
        vec![]
    }

    /// Every MIDI note that this device treats as a sound-triggering pad.
    /// Used to build the pad grid in the UI.
    fn pad_notes(&self) -> Vec<u8>;

    /// Computes the full physical button layout (dimensions + per-note
    /// row/col position) used by the frontend to render the grid.
    ///
    /// The default implementation arranges `pad_notes()` into the smallest
    /// square-ish grid that fits them all, in note order. Devices with a
    /// non-trivial physical layout (e.g. an 8×8 grid plus side buttons)
    /// should override this method to return their real layout instead.
    fn layout(&self) -> DeviceLayout {
        let notes = self.pad_notes();
        let cols = (notes.len() as f64).sqrt().ceil().max(1.0) as u8;
        let rows = if cols == 0 {
            0
        } else {
            ((notes.len() as f64) / (cols as f64)).ceil() as u8
        };

        let pads = notes
            .iter()
            .enumerate()
            .map(|(i, &note)| LayoutPad {
                id: format!("note-{note}"),
                note,
                row: (i as u8) / cols,
                col: (i as u8) % cols,
                kind: PadKind::GridPad,
            })
            .collect();

        DeviceLayout { rows, cols, pads }
    }
}
