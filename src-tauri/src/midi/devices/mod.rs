pub mod json_controller;
pub mod launchpad_mini_mk3;

use crate::midi::traits::{DeviceAction, MidiController, Rgb};

/// A generic MIDI controller that maps NoteOn/NoteOff to `PlaySound`/`StopSound`
/// on all channels and all notes, with optional page-change note overrides.
/// Use this as a fallback when no device-specific driver is available.
pub struct GenericController {
    name: String,
    /// Notes that trigger a page change instead of a sound.
    page_change_notes: Vec<u8>,
}

impl GenericController {
    pub fn new(name: impl Into<String>, page_change_notes: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            page_change_notes,
        }
    }
}

impl MidiController for GenericController {
    fn name(&self) -> &str {
        &self.name
    }

    fn translate_message(&self, message: &[u8]) -> Option<DeviceAction> {
        if message.len() < 2 {
            return None;
        }

        let event_nibble = message[0] >> 4;
        let note = message[1];
        let velocity = message.get(2).copied().unwrap_or(0);

        match event_nibble {
            0x9 => {
                if let Some(idx) = self.page_change_notes.iter().position(|&n| n == note) {
                    return Some(DeviceAction::ChangePage {
                        page_index: idx as u8,
                    });
                }
                Some(if velocity == 0 {
                    DeviceAction::StopSound {
                        pad_id: format!("note-{note}"),
                        note,
                    }
                } else {
                    DeviceAction::PlaySound {
                        pad_id: format!("note-{note}"),
                        note,
                        velocity,
                    }
                })
            }
            0x8 => Some(DeviceAction::StopSound {
                pad_id: format!("note-{note}"),
                note,
            }),
            _ => None,
        }
    }

    /// Generic controllers have no LED feedback capability.
    fn build_color_message(&self, _note: u8, _color: Rgb) -> Vec<u8> {
        vec![]
    }

    fn pad_notes(&self) -> Vec<u8> {
        // All 128 MIDI notes are valid pads on a generic device.
        (0u8..=127).collect()
    }
}
