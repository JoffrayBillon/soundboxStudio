import React, { useEffect, useState } from "react";
import type { DeviceLayout, PadConfig, MidiEvent } from "../types";

interface PadGridProps {
  layout: DeviceLayout;
  /// Sound assignments, looked up by unique pad id. Cells with no matching
  /// entry render as empty/unassigned pads.
  pads: PadConfig[];
  lastEvent: MidiEvent | null;
  onPadClick: (id: string) => void;
  onPadContextMenu: (id: string, e: React.MouseEvent) => void;
  /// Called when a scene (right column) or control (top row) button is
  /// clicked. These have no assignable action yet — reserved for a future
  /// feature to create and bind custom actions to them.
  onFunctionClick: (id: string) => void;
}

export const PadGrid: React.FC<PadGridProps> = ({
  layout,
  pads,
  lastEvent,
  onPadClick,
  onPadContextMenu,
  onFunctionClick,
}) => {
  const [activeNotes, setActiveNotes] = useState<Set<number>>(new Set());

  useEffect(() => {
    if (!lastEvent) return;

    if (lastEvent.event_type === "NoteOn") {
      setActiveNotes((prev) => new Set(prev).add(lastEvent.note));
    } else if (lastEvent.event_type === "NoteOff") {
      setActiveNotes((prev) => {
        const next = new Set(prev);
        next.delete(lastEvent.note);
        return next;
      });
    } else if (lastEvent.event_type === "ControlChange") {
      // Top-row control buttons report their CC value on press (>0) and
      // release (0); mirror the same on/off glow used for note-based pads.
      setActiveNotes((prev) => {
        const next = new Set(prev);
        if (lastEvent.velocity > 0) {
          next.add(lastEvent.note);
        } else {
          next.delete(lastEvent.note);
        }
        return next;
      });
    }
  }, [lastEvent]);

  const padsById = new Map(pads.map((p) => [p.id, p]));

  return (
    <div
      className="grid gap-2 p-4"
      style={{
        gridTemplateColumns: `repeat(${layout.cols}, minmax(0, 1fr))`,
        gridTemplateRows: `repeat(${layout.rows}, minmax(0, 1fr))`,
        width: `min(${layout.cols * 80}px, 100%)`,
      }}
    >
      {layout.pads.map((layoutPad) => {
        const isActive = activeNotes.has(layoutPad.note);
        const isFunctionButton =
          layoutPad.kind === "scene_button" ||
          layoutPad.kind === "control_button";
        const pad = padsById.get(layoutPad.id);

        return (
          <button
            key={layoutPad.id}
            onClick={() =>
              isFunctionButton
                ? onFunctionClick(layoutPad.id)
                : onPadClick(layoutPad.id)
            }
            onContextMenu={(e) => {
              e.preventDefault();
              if (!isFunctionButton) onPadContextMenu(layoutPad.id, e);
            }}
            className={[
              "aspect-square rounded-lg flex flex-col items-center justify-center",
              "text-xs font-semibold transition-all duration-75",
              "border border-gray-700/50 focus:outline-none focus:ring-2 focus:ring-violet-500",
              "cursor-pointer hover:brightness-125 hover:border-gray-500",
              isActive
                ? "scale-95 brightness-150 shadow-lg shadow-violet-500/40"
                : "",
            ].join(" ")}
            style={{
              gridRow: layoutPad.row + 1,
              gridColumn: layoutPad.col + 1,
              backgroundColor: isFunctionButton
                ? isActive
                  ? "#4b5563"
                  : "#1f2937"
                : isActive
                ? (pad?.color ?? "#1F2937")
                : `${pad?.color ?? "#1F2937"}99`,
              minWidth: "60px",
              minHeight: "60px",
            }}
            title={
              isFunctionButton
                ? `${
                    layoutPad.kind === "scene_button" ? "Scene" : "Control"
                  } button (no action assigned yet)`
                : pad?.sound_path
                ? pad.sound_path.split(/[\\/]/).pop()
                : "Right-click to assign a sound"
            }
          >
            <span className="truncate px-1 text-center leading-tight text-white/90">
              {isFunctionButton ? "▸" : pad?.label || `N${layoutPad.note}`}
            </span>
            {!isFunctionButton && pad?.sound_path && (
              <span className="text-[10px] text-white/50 mt-0.5">♪</span>
            )}
          </button>
        );
      })}
    </div>
  );
};
