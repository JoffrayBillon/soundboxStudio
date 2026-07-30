import { useEffect, useState } from "react";

interface SettingsPanelProps {
  masterVolume: number;
  normalizeVolumes: boolean;
  /** Fired continuously while dragging the slider — applies live to the
   * audio engine but is NOT persisted (persisting on every pixel of drag
   * would mean dozens of disk writes per second). */
  onMasterVolumeChange: (volume: number) => void;
  /** Fired once the drag ends (mouse/touch release) — persists the final
   * value to disk so it survives an app restart. */
  onMasterVolumeCommit: (volume: number) => void;
  onNormalizeVolumesChange: (enabled: boolean) => void;
  onClose: () => void;
}

/// Settings overlay for the two sound-related global preferences: a master
/// output volume slider, and a checkbox that makes every sound play back at
/// a more consistent perceived loudness (see `compute_sound_gain` /
/// `PadConfig.normalized_gain` on the backend). Rendered as a small modal
/// (overlay + centered panel), matching the app's existing overlay pattern
/// used for the pad context menu.
export function SettingsPanel({
  masterVolume,
  normalizeVolumes,
  onMasterVolumeChange,
  onMasterVolumeCommit,
  onNormalizeVolumesChange,
  onClose,
}: SettingsPanelProps) {
  // Local live value so the slider tracks the pointer smoothly without
  // waiting on the (debounced) persisted `masterVolume` prop to round-trip.
  const [liveVolume, setLiveVolume] = useState(masterVolume);

  useEffect(() => {
    setLiveVolume(masterVolume);
  }, [masterVolume]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <>
      <div className="fixed inset-0 z-40 bg-black/50" onClick={onClose} />
      <div className="fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-96 rounded-lg border border-gray-700 bg-gray-900 shadow-2xl p-5">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-semibold text-gray-100">Sound settings</h2>
          <button
            className="text-gray-500 hover:text-gray-200 text-lg leading-none"
            onClick={onClose}
            aria-label="Close"
          >
            ×
          </button>
        </div>

        <div className="mb-5">
          <div className="flex items-center justify-between mb-1.5">
            <label className="text-xs font-medium text-gray-300">
              Master volume
            </label>
            <span className="text-xs text-gray-500 tabular-nums">
              {Math.round(liveVolume * 100)}%
            </span>
          </div>
          <input
            type="range"
            min={0}
            max={150}
            step={1}
            value={Math.round(liveVolume * 100)}
            onChange={(e) => {
              const volume = Number(e.target.value) / 100;
              setLiveVolume(volume);
              onMasterVolumeChange(volume);
            }}
            onMouseUp={() => onMasterVolumeCommit(liveVolume)}
            onTouchEnd={() => onMasterVolumeCommit(liveVolume)}
            className="w-full accent-violet-600"
          />
          <p className="text-[11px] text-gray-600 mt-1">
            Overall output level, applied on top of each pad's own sound.
          </p>
        </div>

        <label className="flex items-start gap-2 cursor-pointer">
          <input
            type="checkbox"
            checked={normalizeVolumes}
            onChange={(e) => onNormalizeVolumesChange(e.target.checked)}
            className="mt-0.5 accent-violet-600"
          />
          <span>
            <span className="text-xs font-medium text-gray-300 block">
              Normalize all sounds
            </span>
            <span className="text-[11px] text-gray-600 block mt-0.5">
              Automatically levels each sound's loudest point so louder
              recordings don't overpower quieter ones. Computed once per
              sound when it's assigned to a pad.
            </span>
          </span>
        </label>
      </div>
    </>
  );
}
