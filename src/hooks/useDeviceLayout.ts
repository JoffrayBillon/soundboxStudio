import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { DeviceLayout } from "../types";

/**
 * Known substrings (normalized: lowercased, non-alphanumeric stripped) used
 * to recognize a Launchpad Mini MK3 regardless of platform/driver naming.
 * Real-world MIDI port names vary a lot:
 *   - Windows (WinMM):  "MIDIIN2 (LPMiniMK3 MIDI)"
 *   - macOS (CoreMIDI):  "LPMiniMK3 MIDI" / "Launchpad Mini MK3 MIDI 1"
 *   - Linux (ALSA):      "Launchpad Mini MK3 MIDI 1"
 */
const LAUNCHPAD_MINI_MK3_ALIASES = ["launchpadminimk3", "lpminimk3"];

function isLaunchpadMiniMk3(deviceName: string): boolean {
  const normalized = deviceName.toLowerCase().replace(/[^a-z0-9]/g, "");
  return LAUNCHPAD_MINI_MK3_ALIASES.some((alias) => normalized.includes(alias));
}

/**
 * Activates the appropriate `MidiController` driver for the connected device
 * (a dedicated one when recognized, otherwise a generic fallback) and fetches
 * its physical button layout from the Rust backend.
 *
 * The returned `DeviceLayout` is the sole source of truth for rendering the
 * pad grid — no rows/cols/positions are hardcoded in the frontend.
 */
export function useDeviceLayout(deviceName: string | null, connected: boolean) {
  const [layout, setLayout] = useState<DeviceLayout | null>(null);
  const [controllerName, setControllerName] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!connected || !deviceName) {
      setLayout(null);
      setControllerName(null);
      return;
    }

    let cancelled = false;

    (async () => {
      try {
        if (isLaunchpadMiniMk3(deviceName)) {
          await invoke("use_launchpad_mini_mk3");
        } else {
          await invoke("use_generic_controller", { deviceName });
        }

        const [result, info] = await Promise.all([
          invoke<DeviceLayout>("get_device_layout"),
          invoke<{ name: string } | null>("get_device_controller_info"),
        ]);
        if (!cancelled) {
          setLayout(result);
          setControllerName(info?.name ?? null);
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [deviceName, connected]);

  // Loads a user-provided `DeviceProfileConfig` JSON file (see
  // `device_profile.rs`) and activates it as the current controller. This is
  // how new devices can be supported purely through configuration, without
  // writing a dedicated Rust driver — e.g. a Launchpad X or a differently
  // laid-out Launchpad Mini MK3 variant.
  const loadCustomProfile = useCallback(async (path: string) => {
    try {
      await invoke("load_device_profile", { path });
      const [result, info] = await Promise.all([
        invoke<DeviceLayout>("get_device_layout"),
        invoke<{ name: string } | null>("get_device_controller_info"),
      ]);
      setLayout(result);
      setControllerName(info?.name ?? null);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  return { layout, controllerName, error, loadCustomProfile };
}
