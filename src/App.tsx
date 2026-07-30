import React, { useState, useCallback, useEffect, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { DevicePanel } from "./components/DevicePanel";
import { MidiMonitor } from "./components/MidiMonitor";
import { PadGrid } from "./components/PadGrid";
import { ProfileManager } from "./components/ProfileManager";
import { SettingsPanel } from "./components/SettingsPanel";
import { StatusBar } from "./components/StatusBar";
import { WindowControls } from "./components/WindowControls";
import { useMidi } from "./hooks/useMidi";
import { useAudio } from "./hooks/useAudio";
import { useDeviceLayout } from "./hooks/useDeviceLayout";
import { useAppSettings } from "./hooks/useAppSettings";
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
    playingPads,
    triggerPad,
    setOutputDevice,
  } = useAudio();

  // Remembers the last-used MIDI device / audio output / profile across app
  // restarts (see `settings.rs`) so the user doesn't have to redo this setup
  // every launch.
  const { settings, loaded: settingsLoaded, updateSettings } = useAppSettings();
  // Each restore should only ever be attempted once at startup — otherwise
  // e.g. a later manual disconnect would immediately get auto-reconnected
  // again the next time `midiDevices` refreshes.
  const hasTriedMidiRestore = useRef(false);
  const hasTriedAudioRestore = useRef(false);
  const hasTriedProfileRestore = useRef(false);

  const [activeProfile, setActiveProfile] = useState<Profile | null>(null);
  const [padContextMenu, setPadContextMenu] = useState<{
    padId: string;
    x: number;
    y: number;
    hasSound: boolean;
  } | null>(null);
  // Id of the pad currently under the cursor during a file drag, so
  // `PadGrid` can highlight it as a drop target (null when not dragging or
  // hovering empty space).
  const [dragOverPadId, setDragOverPadId] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);

  const activeDevice = midiDevices.find((d) => d.id === activeDeviceId);
  const {
    layout: deviceLayout,
    controllerName,
    loadCustomProfile,
  } = useDeviceLayout(activeDevice?.name ?? null, activeDeviceId !== null);

  // Auto-reconnects the last-used MIDI device by name once it shows up in a
  // `list_midi_devices` refresh (numeric ids/ports aren't stable across
  // restarts, so name is the only thing worth persisting/matching on).
  useEffect(() => {
    if (
      hasTriedMidiRestore.current ||
      !settingsLoaded ||
      activeDeviceId !== null ||
      midiDevices.length === 0
    ) {
      return;
    }
    hasTriedMidiRestore.current = true;
    const match = settings.last_midi_device_name
      ? midiDevices.find((d) => d.name === settings.last_midi_device_name)
      : undefined;
    if (match) connectMidi(match.id);
  }, [settingsLoaded, settings.last_midi_device_name, midiDevices, activeDeviceId, connectMidi]);

  // Auto-restores the last-used audio output device once the list is
  // fetched. Note: `set_audio_output_device` is currently a backend no-op
  // (see commands.rs) so this only affects which option looks selected in
  // the UI for now, not actual routing.
  useEffect(() => {
    if (hasTriedAudioRestore.current || !settingsLoaded || audioDevices.length === 0) {
      return;
    }
    hasTriedAudioRestore.current = true;
    const match = settings.last_audio_device_id
      ? audioDevices.find((d) => d.id === settings.last_audio_device_id)
      : undefined;
    if (match && match.id !== activeOutputId) setOutputDevice(match.id);
  }, [settingsLoaded, settings.last_audio_device_id, audioDevices, activeOutputId, setOutputDevice]);

  // Auto-loads the last-used profile on startup.
  useEffect(() => {
    if (hasTriedProfileRestore.current || !settingsLoaded || !settings.last_profile_id) {
      return;
    }
    hasTriedProfileRestore.current = true;
    invoke<Profile>("load_profile", { id: settings.last_profile_id })
      .then(setActiveProfile)
      .catch(() => {
        // Profile may have been deleted since — nothing to restore.
      });
  }, [settingsLoaded, settings.last_profile_id]);

  // Persist the current selection whenever it changes, so the *next*
  // restart restores whatever was last actually in use.
  useEffect(() => {
    if (activeDevice?.name) updateSettings({ last_midi_device_name: activeDevice.name });
  }, [activeDevice?.name, updateSettings]);

  useEffect(() => {
    if (activeOutputId) updateSettings({ last_audio_device_id: activeOutputId });
  }, [activeOutputId, updateSettings]);

  useEffect(() => {
    if (activeProfile?.id) updateSettings({ last_profile_id: activeProfile.id });
  }, [activeProfile?.id, updateSettings]);

  // Re-sends the active controller's init sequence (re-asserts Programmer
  // Mode on a Launchpad) on demand — there's no way to query the device's
  // current mode over MIDI, so this is a manual safety net for when it was
  // reset by a physical button combo/power cycle the app didn't detect.
  const handleResyncDevice = useCallback(async () => {
    try {
      await invoke("resync_device");
    } catch (err) {
      console.error("Failed to resync device:", err);
    }
  }, []);

  // Master volume: applied live to the audio engine on every slider tick,
  // but only persisted to disk once the drag ends (see `SettingsPanel`) —
  // otherwise every pixel of movement would trigger a settings.json write.
  const handleMasterVolumeChange = useCallback((volume: number) => {
    invoke("set_master_volume", { volume }).catch((err) => {
      console.error("Failed to set master volume:", err);
    });
  }, []);

  const handleMasterVolumeCommit = useCallback(
    (volume: number) => {
      updateSettings({ master_volume: volume });
    },
    [updateSettings]
  );

  // Normalize-volumes toggle: applied live and persisted immediately since
  // it's a single discrete click, not a continuous drag.
  const handleNormalizeVolumesChange = useCallback(
    (enabled: boolean) => {
      invoke("set_normalize_volumes", { enabled }).catch((err) => {
        console.error("Failed to set normalize volumes:", err);
      });
      updateSettings({ normalize_volumes: enabled });
    },
    [updateSettings]
  );

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

  // Toggles playback: starts the pad's sound (and blinks its LED green) if
  // it isn't already playing, or stops it if it is — so clicking the same
  // pad repeatedly no longer layers overlapping copies of the same sound.
  const handlePadClick = useCallback(
    async (id: string) => {
      await triggerPad(id);
    },
    [triggerPad]
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

  // Persists `pads` as the new active profile (locally + on the backend),
  // then syncs `padId`'s idle LED so the physical (and future) LED state
  // immediately reflects the sound assignment/removal instead of only
  // updating on the pad's next play/stop cycle.
  const persistPads = useCallback(
    async (pads: PadConfig[], padId: string) => {
      if (!activeProfile) return;
      const updatedProfile: Profile = { ...activeProfile, pads };
      setActiveProfile(updatedProfile);
      try {
        await invoke("save_profile", { prof: updatedProfile });
        await invoke("sync_pad_led", { padId });
      } catch (err) {
        console.error("Failed to save profile:", err);
      }
    },
    [activeProfile]
  );

  // Shared by both the file-picker flow (`handleAssignSound`) and dropping a
  // file onto a pad (drag-and-drop effect below) — assigns `filePath` as the
  // pad's sound, creating a new `PadConfig` entry if the pad had none yet.
  // Also (re)computes the file's normalization gain so "normalize volumes"
  // (see `SettingsPanel`) has an up-to-date value cached the moment a sound
  // is assigned, instead of needing a separate step.
  const assignSoundPath = useCallback(
    async (id: string, filePath: string) => {
      if (!activeProfile) return;

      let normalizedGain = 1.0;
      try {
        normalizedGain = await invoke<number>("compute_sound_gain", { filePath });
      } catch (err) {
        console.error("Failed to analyze sound for normalization:", err);
      }

      const existing = activeProfile.pads.find((p) => p.id === id);
      const layoutPad = deviceLayout?.pads.find((p) => p.id === id);
      const updatedPads: PadConfig[] = existing
        ? activeProfile.pads.map((p) =>
            p.id === id
              ? { ...p, sound_path: filePath, normalized_gain: normalizedGain }
              : p
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
              normalized_gain: normalizedGain,
            },
          ];

      await persistPads(updatedPads, id);
    },
    [activeProfile, deviceLayout, persistPads]
  );

  const handleAssignSound = useCallback(
    async (id: string) => {
      if (!activeProfile) return;

      const selected = await open({
        multiple: false,
        filters: [
          { name: "Audio", extensions: ["wav", "mp3", "ogg", "flac"] },
        ],
      });
      if (!selected) return;
      const filePath = Array.isArray(selected) ? selected[0] : selected;
      await assignSoundPath(id, filePath);
    },
    [activeProfile, assignSoundPath]
  );

  const handleRemoveSound = useCallback(
    async (id: string) => {
      if (!activeProfile) return;
      const updatedPads = activeProfile.pads.map((p) =>
        p.id === id ? { ...p, sound_path: null } : p
      );
      await persistPads(updatedPads, id);
    },
    [activeProfile, persistPads]
  );

  // Right-clicking a pad now opens a small menu (Assign / Remove) instead of
  // immediately opening the file picker, so removing a sound is possible.
  const handlePadContextMenu = useCallback(
    (id: string, e: React.MouseEvent) => {
      e.preventDefault();
      if (!activeProfile) return;
      const hasSound = !!activeProfile.pads.find((p) => p.id === id)?.sound_path;
      setPadContextMenu({ padId: id, x: e.clientX, y: e.clientY, hasSound });
    },
    [activeProfile]
  );

  // Closes the pad context menu on Escape. Click-outside / right-click-
  // elsewhere is instead handled by a full-screen overlay rendered behind
  // the menu (see the JSX below) rather than raw `window` listeners here —
  // attaching those directly raced with *opening* a new menu: a stale
  // listener from a previously-open menu would fire during the same bubble
  // phase as the new menu's own state update and immediately null it back
  // out, so the menu appeared to never open at all after the first time.
  useEffect(() => {
    if (!padContextMenu) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPadContextMenu(null);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [padContextMenu]);

  // Quick sound import: dropping an audio file onto a pad assigns it,
  // without needing the right-click menu. Tauri's webview intercepts native
  // OS file drag-and-drop itself (it's not a normal HTML5 `drop` DOM event),
  // so this has to go through `onDragDropEvent` rather than `onDrop` props.
  // The event's position is in physical pixels, so it's converted to CSS
  // pixels before hit-testing which pad element is under the cursor via
  // `data-pad-id` (set in PadGrid.tsx). While the drag is over a pad,
  // `dragOverPadId` is kept in sync so `PadGrid` can show a "drop here"
  // highlight — cleared on drop, cancel, or leaving the window.
  useEffect(() => {
    const hitTestPadId = (x: number, y: number): string | null => {
      const ratio = window.devicePixelRatio || 1;
      const target = document
        .elementFromPoint(x / ratio, y / ratio)
        ?.closest<HTMLElement>("[data-pad-id]");
      return target?.dataset.padId ?? null;
    };

    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await getCurrentWebview().onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          setDragOverPadId(hitTestPadId(event.payload.position.x, event.payload.position.y));
          return;
        }

        setDragOverPadId(null);
        if (event.payload.type !== "drop") return;

        const filePath = event.payload.paths[0];
        if (!filePath || !/\.(wav|mp3|ogg|flac)$/i.test(filePath)) return;
        const padId = hitTestPadId(event.payload.position.x, event.payload.position.y);
        if (padId) assignSoundPath(padId, filePath);
      });
    })();
    return () => unlisten?.();
  }, [assignSoundPath]);

  return (
    <div className="flex flex-col h-screen bg-gray-950">
      {/* Header doubles as the custom titlebar now that native OS
          decorations are disabled (`decorations: false` in
          tauri.conf.json) — `data-tauri-drag-region` lets the window still
          be dragged/double-click-maximized by this area, and
          `WindowControls` provides the minimize/maximize/close buttons the
          OS chrome used to. Buttons inside don't carry the drag-region
          attribute themselves, so they stay normally clickable. */}
      <header
        data-tauri-drag-region
        onDoubleClick={() => getCurrentWindow().toggleMaximize()}
        className="flex items-center justify-between pl-6 h-11 bg-gray-900 border-b border-gray-800 flex-shrink-0"
      >
        <div data-tauri-drag-region className="flex items-center gap-3 h-full flex-1">
          <div className="w-7 h-7 bg-violet-600 rounded-lg flex items-center justify-center text-sm font-bold text-white pointer-events-none">
            S
          </div>
          <h1 className="text-sm font-semibold text-gray-100 tracking-wide pointer-events-none">
            Soundbox Studio
          </h1>
        </div>
        <div className="flex items-center h-full">
          <button
            className="text-gray-400 hover:text-gray-100 text-lg leading-none px-3 h-full flex items-center hover:bg-gray-800"
            onClick={() => setShowSettings(true)}
            aria-label="Sound settings"
            title="Sound settings"
          >
            ⚙
          </button>
          <WindowControls />
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
            onResyncDevice={handleResyncDevice}
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
                playingPadIds={playingPads}
                dragOverPadId={dragOverPadId}
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

      {showSettings && (
        <SettingsPanel
          masterVolume={settings.master_volume}
          normalizeVolumes={settings.normalize_volumes}
          onMasterVolumeChange={handleMasterVolumeChange}
          onMasterVolumeCommit={handleMasterVolumeCommit}
          onNormalizeVolumesChange={handleNormalizeVolumesChange}
          onClose={() => setShowSettings(false)}
        />
      )}

      {padContextMenu && (
        <>
          {/* Full-screen overlay behind the menu: closes it on any click or
              right-click elsewhere. A sibling (not an ancestor) of the menu
              below, so clicks on the menu itself never reach it. */}
          <div
            className="fixed inset-0 z-40"
            onClick={() => setPadContextMenu(null)}
            onContextMenu={(e) => {
              e.preventDefault();
              setPadContextMenu(null);
            }}
          />
          <div
            className="fixed z-50 min-w-[170px] rounded-md border border-gray-700 bg-gray-900 shadow-xl py-1 text-sm"
            style={{ left: padContextMenu.x, top: padContextMenu.y }}
          >
            <button
              className="w-full text-left px-3 py-1.5 text-gray-200 hover:bg-violet-600 hover:text-white"
              onClick={() => {
                const { padId } = padContextMenu;
                setPadContextMenu(null);
                handleAssignSound(padId);
              }}
            >
              Assign sound…
            </button>
            <button
              className="w-full text-left px-3 py-1.5 text-gray-200 hover:bg-red-600 hover:text-white disabled:text-gray-600 disabled:hover:bg-transparent disabled:cursor-not-allowed"
              disabled={!padContextMenu.hasSound}
              onClick={() => {
                const { padId } = padContextMenu;
                setPadContextMenu(null);
                handleRemoveSound(padId);
              }}
            >
              Remove sound
            </button>
          </div>
        </>
      )}
    </div>
  );
}
