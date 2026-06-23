use midir::{MidiInput, MidiInputConnection};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiDevice {
    pub id: usize,
    pub name: String,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MidiEventType {
    NoteOn,
    NoteOff,
    ControlChange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiEvent {
    pub timestamp: u64,
    pub note: u8,
    pub velocity: u8,
    pub channel: u8,
    pub event_type: MidiEventType,
}

pub struct MidiController {
    connection: Option<MidiInputConnection<()>>,
    connected_device_id: Option<usize>,
}

impl MidiController {
    pub fn new() -> Self {
        Self {
            connection: None,
            connected_device_id: None,
        }
    }

    pub fn list_devices() -> AppResult<Vec<MidiDevice>> {
        let midi_in = MidiInput::new("soundbox-studio-query")
            .map_err(|e| AppError::Midi(e.to_string()))?;

        let ports = midi_in.ports();
        let devices = ports
            .iter()
            .enumerate()
            .filter_map(|(id, port)| {
                midi_in.port_name(port).ok().map(|name| MidiDevice {
                    id,
                    name,
                    is_connected: false,
                })
            })
            .collect();

        Ok(devices)
    }

    pub fn connect(&mut self, device_id: usize, app_handle: tauri::AppHandle) -> AppResult<()> {
        self.disconnect();

        let midi_in = MidiInput::new("soundbox-studio")
            .map_err(|e| AppError::Midi(e.to_string()))?;

        let ports = midi_in.ports();
        let port = ports
            .get(device_id)
            .ok_or_else(|| AppError::DeviceNotFound(format!("MIDI device ID {device_id}")))?;

        let connection = midi_in
            .connect(
                port,
                "soundbox-studio-input",
                move |timestamp, message, _| {
                    if let Some(event) = parse_midi_message(timestamp, message) {
                        let _ = app_handle.emit("midi_event", event);
                    }
                },
                (),
            )
            .map_err(|e| AppError::Midi(e.to_string()))?;

        self.connection = Some(connection);
        self.connected_device_id = Some(device_id);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(conn) = self.connection.take() {
            conn.close();
        }
        self.connected_device_id = None;
    }

    pub fn connected_device_id(&self) -> Option<usize> {
        self.connected_device_id
    }
}

fn parse_midi_message(timestamp: u64, message: &[u8]) -> Option<MidiEvent> {
    if message.len() < 2 {
        return None;
    }

    let status = message[0];
    let channel = status & 0x0F;
    let event_type_nibble = status >> 4;

    match event_type_nibble {
        // Note On
        0x9 => {
            let note = message[1];
            let velocity = message.get(2).copied().unwrap_or(0);
            let event_type = if velocity == 0 {
                MidiEventType::NoteOff
            } else {
                MidiEventType::NoteOn
            };
            Some(MidiEvent { timestamp, note, velocity, channel, event_type })
        }
        // Note Off
        0x8 => {
            let note = message[1];
            let velocity = message.get(2).copied().unwrap_or(0);
            Some(MidiEvent {
                timestamp,
                note,
                velocity,
                channel,
                event_type: MidiEventType::NoteOff,
            })
        }
        // Control Change
        0xB => {
            let note = message[1];
            let velocity = message.get(2).copied().unwrap_or(0);
            Some(MidiEvent {
                timestamp,
                note,
                velocity,
                channel,
                event_type: MidiEventType::ControlChange,
            })
        }
        _ => None,
    }
}
