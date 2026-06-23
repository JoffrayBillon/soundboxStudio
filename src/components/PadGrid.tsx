import React, { useEffect, useRef, useState } from "react";
import type { PadConfig, MidiEvent } from "../types";

interface PadGridProps {
  pads: PadConfig[];
  rows: number;
  cols: number;
  lastEvent: MidiEvent | null;
  onPadClick: (pad: PadConfig) => void;
  onPadContextMenu: (pad: PadConfig, e: React.MouseEvent) => void;
}

export const PadGrid: React.FC<PadGridProps> = ({
  pads,
  rows,
  cols,
  lastEvent,
  onPadClick,
  onPadContextMenu,
}) => {
  const activeNotes = useRef<Set<number>>(new Set());
  const [activePads, setActivePads] = useState<Set<number>>(new Set());

  useEffect(() => {
    if (!lastEvent) return;

    if (lastEvent.event_type === "NoteOn") {
      setActivePads((prev) => new Set(prev).add(lastEvent.note));
      activeNotes.current.add(lastEvent.note);
    } else if (lastEvent.event_type === "NoteOff") {
      setActivePads((prev) => {
        const next = new Set(prev);
        next.delete(lastEvent.note);
        return next;
      });
      activeNotes.current.delete(lastEvent.note);
    }
  }, [lastEvent]);

  return (
    <div
      className="grid gap-2 p-4"
      style={{
        gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
        gridTemplateRows: `repeat(${rows}, minmax(0, 1fr))`,
        width: `min(${cols * 80}px, 100%)`,
      }}
    >
      {pads.slice(0, rows * cols).map((pad) => {
        const isActive = activePads.has(pad.note);
        return (
          <button
            key={pad.note}
            onClick={() => onPadClick(pad)}
            onContextMenu={(e) => onPadContextMenu(pad, e)}
            className={[
              "aspect-square rounded-lg flex flex-col items-center justify-center",
              "text-xs font-semibold transition-all duration-75 cursor-pointer",
              "border border-gray-700/50 focus:outline-none focus:ring-2 focus:ring-violet-500",
              isActive
                ? "scale-95 brightness-150 shadow-lg shadow-violet-500/40"
                : "hover:brightness-125 hover:border-gray-500",
            ].join(" ")}
            style={{
              backgroundColor: isActive ? pad.color : `${pad.color}99`,
              minWidth: "60px",
              minHeight: "60px",
            }}
            title={
              pad.sound_path
                ? pad.sound_path.split(/[\\/]/).pop()
                : "Right-click to assign a sound"
            }
          >
            <span className="truncate px-1 text-center leading-tight text-white/90">
              {pad.label || `N${pad.note}`}
            </span>
            {pad.sound_path && (
              <span className="text-[10px] text-white/50 mt-0.5">♪</span>
            )}
          </button>
        );
      })}
    </div>
  );
};
