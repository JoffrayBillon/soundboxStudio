export interface MidiDevice {
  id: number;
  name: string;
  is_connected: boolean;
}

export interface PadConfig {
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
