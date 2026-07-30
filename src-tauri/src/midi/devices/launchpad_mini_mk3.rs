/// Launchpad Mini MK3 — Programmer Mode implementation.
///
/// In Programmer Mode the device:
/// - Sends / receives NoteOn / NoteOff on MIDI channel 1 (0-indexed: 0).
/// - Uses the layout `note = row * 10 + col` (row / col both 1-based).
///   • Grid pads: rows 1–8, cols 1–8 → notes 11–88.
///   • Right-column scene buttons: col 9 → notes 19, 29, …, 89 (Note messages).
///   • Top-row buttons: 8 buttons above the grid → Control Change 91–98
///     (channel 1), *not* Note messages.
/// - Accepts SysEx for RGB LED colour (values 0–63, i.e. 6-bit).
use crate::midi::traits::{DeviceAction, DeviceLayout, LayoutPad, MidiController, PadKind, Rgb};

/// Fixed Control Change numbers for the 8 top-row buttons, left to right.
/// This mapping is part of the Launchpad Mini MK3 Programmer Mode protocol
/// and is not user-configurable (unlike the scene column, which is derived
/// from `page_change_notes`).
const TOP_ROW_CCS: [u8; 8] = [91, 92, 93, 94, 95, 96, 97, 98];

pub struct LaunchpadMiniMk3 {
    /// Notes (from the device layout) that trigger a page change.
    /// The page index equals the position in this list.
    page_change_notes: Vec<u8>,
    /// Whether to emit the "enter Programmer Mode" SysEx on `init_sequence()`.
    programmer_mode: bool,
}

impl LaunchpadMiniMk3 {
    pub fn new(page_change_notes: Vec<u8>, programmer_mode: bool) -> Self {
        Self {
            page_change_notes,
            programmer_mode,
        }
    }

    /// Returns `(row, col)` (1-indexed) for a programmer-mode note, or `None`
    /// if the note does not belong to the 8×8 grid.
    fn note_to_grid(note: u8) -> Option<(u8, u8)> {
        let row = note / 10;
        let col = note % 10;
        if (1..=8).contains(&row) && (1..=8).contains(&col) {
            Some((row, col))
        } else {
            None
        }
    }

    /// Stable identity for a grid pad, keyed by its physical (1-indexed) row
    /// and column — matches the `id` scheme used by the equivalent JSON
    /// device profile (see `device-profiles/launchpad-mini-mk3.json`).
    fn grid_pad_id(note: u8) -> String {
        let (row, col) = Self::note_to_grid(note).expect("grid_pad_id called with non-grid note");
        format!("pad-{row}-{col}")
    }
}

impl MidiController for LaunchpadMiniMk3 {
    fn name(&self) -> &str {
        "Launchpad Mini MK3"
    }

    fn translate_message(&self, message: &[u8]) -> Option<DeviceAction> {
        if message.len() < 2 {
            return None;
        }

        let status = message[0];
        let event_nibble = status >> 4; // 0x8 = NoteOff, 0x9 = NoteOn
        let channel = status & 0x0F; // 0 = MIDI channel 1

        // In Programmer Mode all grid / scene / top-row pads use channel 1 (idx 0).
        if channel != 0 {
            return None;
        }

        let note = message[1];
        let velocity = message.get(2).copied().unwrap_or(0);

        match event_nibble {
            // NoteOn (velocity 0 is treated as NoteOff per MIDI spec)
            0x9 => {
                // Page-change buttons take priority over grid pads.
                if let Some(idx) = self.page_change_notes.iter().position(|&n| n == note) {
                    return Some(DeviceAction::ChangePage {
                        page_index: idx as u8,
                    });
                }

                if Self::note_to_grid(note).is_some() {
                    return Some(if velocity == 0 {
                        DeviceAction::StopSound {
                            pad_id: Self::grid_pad_id(note),
                            note,
                        }
                    } else {
                        DeviceAction::PlaySound {
                            pad_id: Self::grid_pad_id(note),
                            note,
                            velocity,
                        }
                    });
                }

                None
            }
            // NoteOff
            0x8 => {
                if Self::note_to_grid(note).is_some() {
                    Some(DeviceAction::StopSound {
                        pad_id: Self::grid_pad_id(note),
                        note,
                    })
                } else {
                    None
                }
            }
            // Control Change — only the top-row buttons use this on this
            // device. They report value 127 on press, 0 on release; only the
            // press is translated into an action. Page indices 8–15 are used
            // (offset past the scene column's 0–7) so the two rows never
            // collide.
            0xB => {
                let cc = message[1];
                let value = message.get(2).copied().unwrap_or(0);
                if value == 0 {
                    return None;
                }
                TOP_ROW_CCS
                    .iter()
                    .position(|&c| c == cc)
                    .map(|idx| DeviceAction::ChangePage {
                        page_index: 8 + idx as u8,
                    })
            }
            _ => None,
        }
    }

    /// Builds a SysEx RGB LED lighting message for the Launchpad Mini MK3.
    ///
    /// Format: `F0 00 20 29 02 0D 03  03 <note> <r/4> <g/4> <b/4>  F7`
    /// RGB components are scaled 8-bit → 6-bit by right-shifting two bits.
    fn build_color_message(&self, note: u8, color: Rgb) -> Vec<u8> {
        vec![
            0xF0,
            0x00, 0x20, 0x29, // Novation manufacturer ID
            0x02, 0x0D,       // Launchpad Mini MK3 product ID
            0x03,             // LED lighting command
            0x03,             // Sub-type: RGB
            note,
            color.r >> 2,     // 0-255 → 0-63
            color.g >> 2,
            color.b >> 2,
            0xF7,
        ]
    }

    /// Returns the SysEx to switch the device into Programmer Mode (mode 1).
    fn init_sequence(&self) -> Vec<Vec<u8>> {
        if !self.programmer_mode {
            return vec![];
        }
        // Standalone layout select — select layout 0x7F = Programmer Mode.
        // Format: F0 00 20 29 02 0D 00 7F F7
        vec![vec![0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x00, 0x7F, 0xF7]]
    }

    fn pad_notes(&self) -> Vec<u8> {
        // 8×8 grid: notes 11–88 with the layout formula row*10 + col.
        let mut notes = Vec::with_capacity(64);
        for row in 1u8..=8 {
            for col in 1u8..=8 {
                notes.push(row * 10 + col);
            }
        }
        notes
    }

    /// Builds the real physical layout of the Launchpad Mini MK3: 8 top-row
    /// control buttons (row 0), an 8×8 grid (rows 1–8, columns 0–7), and the
    /// right-hand column of 8 scene-launch buttons (column 8) — matching the
    /// physical arrangement of the hardware.
    ///
    /// Device row 8 is the physical top grid row, so it is placed directly
    /// under the control-button row — UI row = `1 + (8 - device_row)`.
    fn layout(&self) -> DeviceLayout {
        const GRID_COLS: u8 = 8;
        let mut pads = Vec::with_capacity(1 + 64 + 8);

        // Top-row control buttons (CC 91–98), aligned above the grid columns.
        for (col, _cc) in TOP_ROW_CCS.iter().enumerate() {
            pads.push(LayoutPad {
                id: format!("ctrl-{}", col + 1),
                note: TOP_ROW_CCS[col],
                row: 0,
                col: col as u8,
                kind: PadKind::ControlButton,
            });
        }

        // 8×8 grid pads (columns 0–7, rows 1–8).
        for device_row in 1u8..=8 {
            for device_col in 1u8..=8 {
                pads.push(LayoutPad {
                    id: format!("pad-{device_row}-{device_col}"),
                    note: device_row * 10 + device_col,
                    row: 1 + (8 - device_row),
                    col: device_col - 1,
                    kind: PadKind::GridPad,
                });
            }
        }

        // Right-hand scene-launch buttons (column 8), one per grid row.
        for &note in &self.page_change_notes {
            let device_row = note / 10;
            if (1..=8).contains(&device_row) {
                pads.push(LayoutPad {
                    id: format!("scene-{device_row}"),
                    note,
                    row: 1 + (8 - device_row),
                    col: GRID_COLS,
                    kind: PadKind::SceneButton,
                });
            }
        }

        DeviceLayout {
            rows: 1 + 8,
            cols: GRID_COLS + 1,
            pads,
        }
    }
}
