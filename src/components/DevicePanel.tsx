import React from "react";
import type { MidiDevice, AudioDevice } from "../types";

interface DevicePanelProps {
  midiDevices: MidiDevice[];
  activeDeviceId: number | null;
  audioDevices: AudioDevice[];
  activeAudioId: string | null;
  onConnectMidi: (id: number) => void;
  onDisconnectMidi: () => void;
  onRefreshMidi: () => void;
  onSelectAudio: (id: string) => void;
}

export const DevicePanel: React.FC<DevicePanelProps> = ({
  midiDevices,
  activeDeviceId,
  audioDevices,
  activeAudioId,
  onConnectMidi,
  onDisconnectMidi,
  onRefreshMidi,
  onSelectAudio,
}) => {
  return (
    <div className="flex flex-col gap-4 p-4 bg-gray-900 rounded-xl border border-gray-800">
      {/* MIDI Section */}
      <div>
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">
            MIDI Input
          </h3>
          <button
            onClick={onRefreshMidi}
            className="text-xs text-gray-600 hover:text-violet-400 transition-colors"
          >
            Refresh
          </button>
        </div>
        <div className="flex flex-col gap-1">
          {midiDevices.length === 0 ? (
            <p className="text-xs text-gray-600 italic py-1">
              No MIDI devices found.
            </p>
          ) : (
            midiDevices.map((device) => {
              const isActive = device.id === activeDeviceId;
              return (
                <button
                  key={device.id}
                  onClick={() =>
                    isActive ? onDisconnectMidi() : onConnectMidi(device.id)
                  }
                  className={[
                    "flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors text-left",
                    isActive
                      ? "bg-violet-600/20 text-violet-300 border border-violet-500/40"
                      : "bg-gray-800 text-gray-400 hover:bg-gray-700 border border-transparent",
                  ].join(" ")}
                >
                  <span
                    className={[
                      "w-2 h-2 rounded-full flex-shrink-0",
                      isActive ? "bg-violet-400 animate-pulse" : "bg-gray-600",
                    ].join(" ")}
                  />
                  <span className="truncate text-xs">{device.name}</span>
                </button>
              );
            })
          )}
        </div>
      </div>

      {/* Audio Output Section */}
      <div>
        <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2">
          Audio Output
        </h3>
        <select
          value={activeAudioId ?? ""}
          onChange={(e) => onSelectAudio(e.target.value)}
          className="w-full bg-gray-800 text-gray-300 text-xs rounded-lg px-3 py-2 border border-gray-700 focus:outline-none focus:ring-1 focus:ring-violet-500"
        >
          {audioDevices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
              {d.is_default ? " (default)" : ""}
            </option>
          ))}
        </select>
      </div>
    </div>
  );
};
