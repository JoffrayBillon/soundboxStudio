import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AudioDevice } from "../types";

export function useAudio() {
  const [outputDevices, setOutputDevices] = useState<AudioDevice[]>([]);
  const [activeOutputId, setActiveOutputId] = useState<string | null>(null);
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

  useEffect(() => {
    refreshOutputDevices();
  }, [refreshOutputDevices]);

  return {
    outputDevices,
    activeOutputId,
    error,
    setOutputDevice,
    playSoundFile,
    stopAllSounds,
    refreshOutputDevices,
  };
}
