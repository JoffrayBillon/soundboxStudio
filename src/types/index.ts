export interface MidiDevice {
  id: number;
  name: string;
  is_connected: boolean;
}

export interface PadConfig {
  /** Stable identity matching `LayoutPad.id` — sound assignment is always
   * keyed by `id`, never by the raw `note`/CC number. */
  id: string;
  /** Raw MIDI note/CC number, kept only for display/debugging. */
  note: number;
  label: string;
  color: string;
  sound_path: string | null;
  volume: number;
}

export interface Profile {
  id: string;
  name: string;
  device_name: string;
  rows: number;
  cols: number;
  pads: PadConfig[];
}

export interface AudioDevice {
  id: string;
  name: string;
  is_default: boolean;
}

export interface MidiEvent {
  timestamp: number;
  note: number;
  velocity: number;
  channel: number;
  event_type: "NoteOn" | "NoteOff" | "ControlChange";
}

/// The physical role of a button on the controller.
export type PadKind = "grid_pad" | "scene_button" | "control_button";

/// High-level, device-agnostic action emitted as a `device_action` Tauri
/// event, mirroring the Rust `DeviceAction` enum.
export type DeviceAction =
  | { type: "PlaySound"; payload: { pad_id: string; note: number; velocity: number } }
  | { type: "StopSound"; payload: { pad_id: string; note: number } }
  | { type: "ChangePage"; payload: { page_index: number } };

/// Position and role of a single button, as computed by the Rust
/// `MidiController` driver. `row`/`col` are 0-indexed, top-left origin.
export interface LayoutPad {
  /** Stable, unique identity across the whole device — always use this key
   * for sound assignment/lookup, never `note`. */
  id: string;
  note: number;
  row: number;
  col: number;
  kind: PadKind;
}

/// The full physical button layout of the active MIDI controller, fetched
/// from the backend via `get_device_layout`. This is the single source of
/// truth for rendering the pad grid — no dimensions are hardcoded in the UI.
export interface DeviceLayout {
  rows: number;
  cols: number;
  pads: LayoutPad[];
}
