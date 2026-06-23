import { useEffect, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { MidiDevice, MidiEvent } from "../types";

export function useMidi() {
  const [devices, setDevices] = useState<MidiDevice[]>([]);
  const [activeDeviceId, setActiveDeviceId] = useState<number | null>(null);
  const [lastEvent, setLastEvent] = useState<MidiEvent | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refreshDevices = useCallback(async () => {
    try {
      const result = await invoke<MidiDevice[]>("list_midi_devices");
      setDevices(result);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const connectDevice = useCallback(async (deviceId: number) => {
    try {
      await invoke("connect_midi_device", { deviceId });
      setActiveDeviceId(deviceId);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const disconnectDevice = useCallback(async () => {
    try {
      await invoke("disconnect_midi_device");
      setActiveDeviceId(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refreshDevices();
    const unlisten = listen<MidiEvent>("midi_event", (event) => {
      setLastEvent(event.payload);
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, [refreshDevices]);

  return {
    devices,
    activeDeviceId,
    lastEvent,
    error,
    refreshDevices,
    connectDevice,
    disconnectDevice,
  };
}
