import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export interface AppSettings {
  last_midi_device_name: string | null;
  last_audio_device_id: string | null;
  last_profile_id: string | null;
  /** Global output gain multiplier (1.0 = unity). */
  master_volume: number;
  /** Whether playback applies each pad's cached `normalized_gain`. */
  normalize_volumes: boolean;
}

const DEFAULT_SETTINGS: AppSettings = {
  last_midi_device_name: null,
  last_audio_device_id: null,
  last_profile_id: null,
  master_volume: 1.0,
  normalize_volumes: false,
};

/// Persists the last-used MIDI device / audio output / profile (see
/// `settings.rs`) so the app can restore them automatically on the next
/// launch instead of the user having to reconnect/reselect everything every
/// time. Loaded once on mount (`loaded` flips true once that finishes, so
/// callers can wait before attempting to restore anything); `updateSettings`
/// merges a partial patch into the current settings and persists the result.
export function useAppSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    (async () => {
      try {
        const result = await invoke<AppSettings>("load_app_settings");
        setSettings(result);
      } catch {
        // First run or a corrupt/missing file — defaults are fine.
      } finally {
        setLoaded(true);
      }
    })();
  }, []);

  const updateSettings = useCallback((patch: Partial<AppSettings>) => {
    setSettings((prev) => {
      const next = { ...prev, ...patch };
      invoke("save_app_settings", { settings: next }).catch(() => {});
      return next;
    });
  }, []);

  return { settings, loaded, updateSettings };
}
