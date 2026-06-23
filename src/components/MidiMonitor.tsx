import React, { useEffect, useRef, useState } from "react";
import type { MidiEvent } from "../types";

interface MidiMonitorProps {
  lastEvent: MidiEvent | null;
}

export const MidiMonitor: React.FC<MidiMonitorProps> = ({ lastEvent }) => {
  const eventsRef = useRef<MidiEvent[]>([]);
  const [displayEvents, setDisplayEvents] = useState<MidiEvent[]>([]);

  useEffect(() => {
    if (!lastEvent) return;
    eventsRef.current = [lastEvent, ...eventsRef.current].slice(0, 20);
    setDisplayEvents([...eventsRef.current]);
  }, [lastEvent]);

  return (
    <div className="bg-gray-900 rounded-xl border border-gray-800 p-3 h-44 flex flex-col">
      <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wider mb-2 flex-shrink-0">
        MIDI Monitor
      </h3>
      <div className="overflow-y-auto flex-1 font-mono text-[11px] space-y-0.5">
        {displayEvents.length === 0 ? (
          <p className="text-gray-600 italic">Waiting for MIDI input…</p>
        ) : (
          displayEvents.map((evt, i) => (
            <div
              key={`${evt.timestamp}-${i}`}
              className={[
                "flex gap-3 px-2 py-0.5 rounded",
                i === 0
                  ? "bg-violet-900/30 text-violet-200"
                  : "text-gray-600",
              ].join(" ")}
            >
              <span className="w-24 flex-shrink-0">{evt.event_type}</span>
              <span>N:{String(evt.note).padStart(3, "0")}</span>
              <span>V:{String(evt.velocity).padStart(3, "0")}</span>
              <span>Ch:{evt.channel}</span>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
