import React, { useState, useCallback, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { DevicePanel } from "./components/DevicePanel";
import { MidiMonitor } from "./components/MidiMonitor";
import { PadGrid } from "./components/PadGrid";
import { ProfileManager } from "./components/ProfileManager";
import { StatusBar } from "./components/StatusBar";
import { useMidi } from "./hooks/useMidi";
import { useAudio } from "./hooks/useAudio";
import { useDeviceLayout } from "./hooks/useDeviceLayout";
import type { PadConfig, Profile } from "./types";

export default function App() {
  const {
    devices: midiDevices,
    activeDeviceId,
    lastEvent,
    lastAction,
    refreshDevices: refreshMidi,
    connectDevice: connectMidi,
    disconnectDevice: disconnectMidi,
  } = useMidi();

  const {
    outputDevices: audioDevices,
    activeOutputId,
    playSoundFile,
    setOutputDevice,
  } = useAudio();

  const [activeProfile, setActiveProfile] = useState<Profile | null>(null);

  const activeDevice = midiDevices.find((d) => d.id === activeDeviceId);
  const {
    layout: deviceLayout,
    controllerName,
    loadCustomProfile,
  } = useDeviceLayout(activeDevice?.name ?? null, activeDeviceId !== null);

  // Lets the user pick a `DeviceProfileConfig` JSON file (see
  // `device_profile.rs`) to support a device that has no dedicated Rust
  // driver, without writing any code.
  const handleLoadCustomProfile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Device profile", extensions: ["json"] }],
    });
    if (!selected) return;
    const filePath = Array.isArray(selected) ? selected[0] : selected;
    await loadCustomProfile(filePath);
  }, [loadCustomProfile]);

  const handlePadClick = useCallback(
    async (id: string) => {
      const pad = activeProfile?.pads.find((p) => p.id === id);
      if (pad?.sound_path) {
        await playSoundFile(pad.sound_path, pad.volume);
      }
    },
    [activeProfile, playSoundFile]
  );

  // Physical pad presses go through the backend's `translate_message` and
  // are re-emitted as `device_action` events (PlaySound/StopSound/ChangePage)
  // in addition to the raw `midi_event` (used only for the pad glow
  // animation). Without this, pressing a hardware pad lit it up but never
  // actually played the assigned sound.
  useEffect(() => {
    if (lastAction?.type === "PlaySound") {
      handlePadClick(lastAction.payload.pad_id);
    }
  }, [lastAction, handlePadClick]);

  // Scene/control buttons don't have an assignable action yet — this is a
  // placeholder until we build the UI to create and assign them.
  const handleFunctionClick = useCallback(() => {
    // No-op for now.
  }, []);

  const handlePadContextMenu = useCallback(
    async (id: string, e: React.MouseEvent) => {
      e.preventDefault();
      if (!activeProfile) return;

      const selected = await open({
        multiple: false,
        filters: [
          { name: "Audio", extensions: ["wav", "mp3", "ogg", "flac"] },
        ],
      });

      if (!selected) return;
      const filePath = Array.isArray(selected) ? selected[0] : selected;

      const existing = activeProfile.pads.find((p) => p.id === id);
      const layoutPad = deviceLayout?.pads.find((p) => p.id === id);
      const updatedPads: PadConfig[] = existing
        ? activeProfile.pads.map((p) =>
            p.id === id ? { ...p, sound_path: filePath } : p
          )
        : [
            ...activeProfile.pads,
            {
              id,
              note: layoutPad?.note ?? 0,
              label: "",
              color: "#1F2937",
              sound_path: filePath,
              volume: 1.0,
            },
          ];

      const updatedProfile: Profile = { ...activeProfile, pads: updatedPads };
      setActiveProfile(updatedProfile);

      try {
        await invoke("save_profile", { prof: updatedProfile });
      } catch (err) {
        console.error("Failed to save profile:", err);
      }
    },
    [activeProfile, deviceLayout]
  );

  return (
    <div className="flex flex-col h-screen bg-gray-950">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-3 bg-gray-900 border-b border-gray-800 flex-shrink-0">
        <div className="flex items-center gap-3">
          <div className="w-7 h-7 bg-violet-600 rounded-lg flex items-center justify-center text-sm font-bold text-white">
            S
          </div>
          <h1 className="text-sm font-semibold text-gray-100 tracking-wide">
            Soundbox Studio
          </h1>
        </div>
      </header>

      {/* Main layout */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left sidebar */}
        <aside className="w-60 flex-shrink-0 flex flex-col gap-3 p-3 bg-gray-950 border-r border-gray-800 overflow-y-auto">
          <DevicePanel
            midiDevices={midiDevices}
            activeDeviceId={activeDeviceId}
            audioDevices={audioDevices}
            activeAudioId={activeOutputId}
            controllerName={controllerName}
            onConnectMidi={connectMidi}
            onDisconnectMidi={disconnectMidi}
            onRefreshMidi={refreshMidi}
            onSelectAudio={setOutputDevice}
            onLoadCustomProfile={handleLoadCustomProfile}
          />
          <ProfileManager
            activeProfile={activeProfile}
            deviceLayout={deviceLayout}
            onProfileLoad={setActiveProfile}
          />
        </aside>

        {/* Central pad grid */}
        <main className="flex-1 flex flex-col overflow-hidden">
          {activeProfile && deviceLayout ? (
            <div className="flex-1 overflow-auto flex items-center justify-center p-6">
              <PadGrid
                layout={deviceLayout}
                pads={activeProfile.pads}
                lastEvent={lastEvent}
                onPadClick={handlePadClick}
                onPadContextMenu={handlePadContextMenu}
                onFunctionClick={handleFunctionClick}
              />
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center text-gray-600">
                <div className="text-5xl mb-4 select-none">🎹</div>
                <p className="text-base font-medium text-gray-500">
                  {!activeDevice
                    ? "No MIDI device connected"
                    : !deviceLayout
                    ? "Loading device layout…"
                    : "No profile loaded"}
                </p>
                <p className="text-xs mt-2 text-gray-600">
                  {!activeDevice ? (
                    "Connect a MIDI controller in the sidebar to load its grid layout."
                  ) : (
                    <>
                      Create or select a profile in the sidebar to get started.
                      <br />
                      Right-click any pad to assign a sound file.
                    </>
                  )}
                </p>
              </div>
            </div>
          )}


          <div className="flex-shrink-0 p-3 border-t border-gray-800">
            <MidiMonitor lastEvent={lastEvent} />
          </div>
        </main>
      </div>

      <StatusBar
        isConnected={activeDeviceId !== null}
        deviceName={activeDevice?.name ?? null}
        profileName={activeProfile?.name ?? null}
      />
    </div>
  );
}
