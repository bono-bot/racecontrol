"use client";

import type { Lap } from "@/lib/types";

interface LiveLapTickerProps {
  laps: Lap[];
  maxItems?: number;
}

function formatLapTime(ms: number): string {
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = (totalSecs % 60).toFixed(3);
  return `${mins}:${parseFloat(secs) < 10 ? "0" : ""}${secs}`;
}

export function LiveLapTicker({ laps, maxItems = 15 }: LiveLapTickerProps) {
  return (
    <div className="space-y-1.5">
      {laps.slice(0, maxItems).map((lap, i) => (
        <div
          key={lap.id}
          className={`flex items-center gap-2 text-xs py-1 ${
            i === 0 ? "text-white" : "text-rp-grey"
          }`}
        >
          <span className="text-rp-grey">&#127937;</span>
          <span className="truncate flex-1">
            {lap.driver_id.slice(0, 8)}
          </span>
          <span>Lap {lap.lap_number ?? "?"}</span>
          <span className={`font-mono tabular-nums ${i === 0 ? "text-rp-red font-semibold" : ""}`}>
            {formatLapTime(lap.lap_time_ms)}
          </span>
          {!lap.valid && <span className="text-amber-500 text-[10px]">INV</span>}
        </div>
      ))}
      {laps.length === 0 && (
        <p className="text-xs text-zinc-600 text-center py-4">No laps recorded yet</p>
      )}
    </div>
  );
}
