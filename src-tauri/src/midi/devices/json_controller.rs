/// Fully data-driven `MidiController` implementation.
///
/// Unlike `LaunchpadMiniMk3` (which hardcodes the layout and MIDI mapping in
/// Rust), every button here — its position, its kind, and which raw MIDI
/// message triggers it — comes from a `DeviceProfileConfig` JSON file (see
/// `device_profile.rs`). This is how a brand-new controller can be supported
/// without writing a dedicated driver.
///
/// LED feedback is intentionally not supported here: SysEx colour formats
/// are too hardware-specific to express generically, and no part of the UI
/// currently uses it (see `MidiController::build_color_message`'s default-ish
/// empty-Vec behaviour on `GenericController`).
use crate::midi::device_profile::{MidiInputSpec, MidiMessageType, PadDefinition};
use crate::midi::traits::{DeviceAction, DeviceLayout, LayoutPad, MidiController, PadKind, Rgb};

pub struct JsonController {
    name: String,
    rows: u8,
    cols: u8,
    pads: Vec<PadDefinition>,
}

impl JsonController {
    pub fn new(name: String, rows: u8, cols: u8, pads: Vec<PadDefinition>) -> Self {
        Self {
            name,
            rows,
            cols,
            pads,
        }
    }

    /// Finds the pad definition whose `input` matches the given raw message
    /// shape, if any.
    fn find_pad(&self, input: &MidiInputSpec) -> Option<&PadDefinition> {
        self.pads.iter().find(|p| p.input == *input)
    }
}

impl MidiController for JsonController {
    fn name(&self) -> &str {
        &self.name
    }

    fn translate_message(&self, message: &[u8]) -> Option<DeviceAction> {
        if message.len() < 2 {
            return None;
        }

        let status = message[0];
        let event_nibble = status >> 4;
        let channel = status & 0x0F;
        let number = message[1];
        let value = message.get(2).copied().unwrap_or(0);

        // NoteOff (0x8) is always a release; NoteOn (0x9) with value 0 is
        // also a release per the MIDI spec. Control Change (0xB) buttons
        // report 0 on release and a non-zero value on press.
        let (message_type, is_release) = match event_nibble {
            0x9 => (MidiMessageType::Note, value == 0),
            0x8 => (MidiMessageType::Note, true),
            0xB => (MidiMessageType::ControlChange, value == 0),
            _ => return None,
        };

        let input = MidiInputSpec {
            message_type,
            channel,
            number,
        };
        let pad = self.find_pad(&input)?;

        match pad.kind {
            PadKind::GridPad => Some(if is_release {
                DeviceAction::StopSound {
                    pad_id: pad.id.clone(),
                    note: number,
                }
            } else {
                DeviceAction::PlaySound {
                    pad_id: pad.id.clone(),
                    note: number,
                    velocity: value,
                }
            }),
            PadKind::SceneButton | PadKind::ControlButton => {
                if is_release {
                    return None;
                }
                Some(DeviceAction::ChangePage {
                    page_index: pad.page_index.unwrap_or(0),
                })
            }
        }
    }

    /// Not supported generically — see the module doc comment.
    fn build_color_message(&self, _note: u8, _color: Rgb) -> Vec<u8> {
        vec![]
    }

    fn pad_notes(&self) -> Vec<u8> {
        self.pads
            .iter()
            .filter(|p| p.kind == PadKind::GridPad)
            .map(|p| p.input.number)
            .collect()
    }

    fn layout(&self) -> DeviceLayout {
        DeviceLayout {
            rows: self.rows,
            cols: self.cols,
            pads: self
                .pads
                .iter()
                .map(|p| LayoutPad {
                    id: p.id.clone(),
                    note: p.input.number,
                    row: p.row,
                    col: p.col,
                    kind: p.kind,
                })
                .collect(),
        }
    }
}
