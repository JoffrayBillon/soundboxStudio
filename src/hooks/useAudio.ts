import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AudioDevice } from "../types";

/// Payload of the `pad_playback_changed` Tauri event, emitted by the backend
/// whenever a pad starts or stops playing its sound — from a click, a
/// physical press, a manual re-click, or the sound finishing on its own.
interface PadPlaybackChanged {
  pad_id: string;
  playing: boolean;
}

export function useAudio() {
  const [outputDevices, setOutputDevices] = useState<AudioDevice[]>([]);
  const [activeOutputId, setActiveOutputId] = useState<string | null>(null);
  const [playingPads, setPlayingPads] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  const refreshOutputDevices = useCallback(async () => {
    try {
      const result = await invoke<AudioDevice[]>("list_audio_output_devices");
      setOutputDevices(result);
      const def = result.find((d) => d.is_default);
      if (def) setActiveOutputId(def.id);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const setOutputDevice = useCallback(async (deviceId: string) => {
    try {
      await invoke("set_audio_output_device", { deviceId });
      setActiveOutputId(deviceId);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const playSoundFile = useCallback(
    async (filePath: string, volume: number = 1.0) => {
      try {
        await invoke("play_sound_file", { filePath, volume });
      } catch (e) {
        setError(String(e));
      }
    },
    []
  );

  const stopAllSounds = useCallback(async () => {
    try {
      await invoke("stop_all_sounds");
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /// Toggles playback for pad `padId`: starts its assigned sound (backend
  /// looks up the sound path/volume from the active profile) and lights its
  /// LED green/blinking, or — if it's already playing — stops it. Works the
  /// same whether triggered by an on-screen click or a physical pad press.
  const triggerPad = useCallback(async (padId: string) => {
    try {
      await invoke("trigger_pad", { padId });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refreshOutputDevices();
  }, [refreshOutputDevices]);

  useEffect(() => {
    const unlisten = listen<PadPlaybackChanged>("pad_playback_changed", (event) => {
      const { pad_id, playing } = event.payload;
      setPlayingPads((prev) => {
        const next = new Set(prev);
        if (playing) {
          next.add(pad_id);
        } else {
          next.delete(pad_id);
        }
        return next;
      });
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  return {
    outputDevices,
    activeOutputId,
    playingPads,
    error,
    setOutputDevice,
    playSoundFile,
    stopAllSounds,
    triggerPad,
    refreshOutputDevices,
  };
}

