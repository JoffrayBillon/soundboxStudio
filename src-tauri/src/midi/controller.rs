use std::sync::Arc;

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::error::{AppError, AppResult};
use crate::midi::traits::MidiController;

// ── Public data types ─────────────────────────────────────────────────────────

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

/// Raw MIDI event emitted on the `midi_event` Tauri channel.
/// The `device_action` channel carries the translated `DeviceAction`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiEvent {
    pub timestamp: u64,
    pub note: u8,
    pub velocity: u8,
    pub channel: u8,
    pub event_type: MidiEventType,
}

// ── MidiPort ──────────────────────────────────────────────────────────────────

/// Manages the raw MIDI input/output connections for the active physical device.
///
/// Renamed from `MidiController` to avoid collision with the `MidiController`
/// trait that carries the device-abstraction logic.
pub struct MidiPort {
    input_connection: Option<MidiInputConnection<()>>,
    output_connection: Option<MidiOutputConnection>,
    connected_device_id: Option<usize>,
    connected_device_name: Option<String>,
}

impl MidiPort {
    pub fn new() -> Self {
        Self {
            input_connection: None,
            output_connection: None,
            connected_device_id: None,
            connected_device_name: None,
        }
    }

    // ── Device enumeration ────────────────────────────────────────────────────

    pub fn list_devices() -> AppResult<Vec<MidiDevice>> {
        let midi_in = MidiInput::new("soundbox-studio-query")
            .map_err(|e| AppError::Midi(e.to_string()))?;

        let devices = midi_in
            .ports()
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

    // ── Connection lifecycle ──────────────────────────────────────────────────

    /// Opens the MIDI input for `device_id`, optionally translates messages
    /// through `device_controller`, and emits both `midi_event` (raw) and
    /// `device_action` (translated) Tauri events.
    /// Also tries to open a matching MIDI output for LED / SysEx feedback.
    pub fn connect(
        &mut self,
        device_id: usize,
        app_handle: tauri::AppHandle,
        device_controller: Arc<Mutex<Option<Box<dyn MidiController>>>>,
    ) -> AppResult<()> {
        self.disconnect();

        let midi_in = MidiInput::new("soundbox-studio")
            .map_err(|e| AppError::Midi(e.to_string()))?;

        let ports = midi_in.ports();
        let port = ports
            .get(device_id)
            .ok_or_else(|| AppError::DeviceNotFound(format!("MIDI device ID {device_id}")))?;

        let device_name = midi_in
            .port_name(port)
            .map_err(|e| AppError::Midi(e.to_string()))?;

        // Try to open a matching MIDI output (best-effort, not fatal if absent).
        let output_connection = Self::try_open_output(&device_name);

        let input_connection = midi_in
            .connect(
                port,
                "soundbox-studio-input",
                move |timestamp, message, _| {
                    // 1. Always emit the raw event for the MIDI monitor.
                    if let Some(raw) = parse_midi_message(timestamp, message) {
                        let _ = app_handle.emit("midi_event", raw);
                    }
                    // 2. Emit a translated DeviceAction if a controller is loaded.
                    if let Some(ctrl) = device_controller.lock().as_ref() {
                        if let Some(action) = ctrl.translate_message(message) {
                            let _ = app_handle.emit("device_action", action);
                        }
                    }
                },
                (),
            )
            .map_err(|e| AppError::Midi(e.to_string()))?;

        self.input_connection = Some(input_connection);
        self.output_connection = output_connection;
        self.connected_device_id = Some(device_id);
        self.connected_device_name = Some(device_name);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(conn) = self.input_connection.take() {
            conn.close();
        }
        // Dropping MidiOutputConnection closes it automatically.
        self.output_connection.take();
        self.connected_device_id = None;
        self.connected_device_name = None;
    }

    // ── MIDI output ───────────────────────────────────────────────────────────

    /// Sends raw bytes (SysEx or standard MIDI) to the connected output port.
    /// Returns `Err(AppError::NotConnected)` when no output is open.
    pub fn send_bytes(&mut self, bytes: &[u8]) -> AppResult<()> {
        match self.output_connection.as_mut() {
            Some(conn) => conn
                .send(bytes)
                .map_err(|e| AppError::Midi(e.to_string())),
            None => Err(AppError::NotConnected),
        }
    }

    // ── Accessors ─────────────────────────────────────────────────────────────

    #[allow(dead_code)]
    pub fn connected_device_id(&self) -> Option<usize> {
        self.connected_device_id
    }

    #[allow(dead_code)]
    pub fn connected_device_name(&self) -> Option<&str> {
        self.connected_device_name.as_deref()
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Searches MIDI output ports for one whose name matches `device_name`
    /// and returns an open connection, or `None` if not found / unavailable.
    fn try_open_output(device_name: &str) -> Option<MidiOutputConnection> {
        let midi_out = MidiOutput::new("soundbox-studio-output").ok()?;
        let port = midi_out
            .ports()
            .into_iter()
            .find(|p| midi_out.port_name(p).ok().as_deref() == Some(device_name))?;
        midi_out.connect(&port, "soundbox-studio-out").ok()
    }
}

// ── MIDI message parsing ──────────────────────────────────────────────────────

fn parse_midi_message(timestamp: u64, message: &[u8]) -> Option<MidiEvent> {
    if message.len() < 2 {
        return None;
    }

    let status = message[0];
    let channel = status & 0x0F;
    let event_nibble = status >> 4;

    match event_nibble {
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
        0x8 => Some(MidiEvent {
            timestamp,
            note: message[1],
            velocity: message.get(2).copied().unwrap_or(0),
            channel,
            event_type: MidiEventType::NoteOff,
        }),
        0xB => Some(MidiEvent {
            timestamp,
            note: message[1],
            velocity: message.get(2).copied().unwrap_or(0),
            channel,
            event_type: MidiEventType::ControlChange,
        }),
        _ => None,
    }
}
