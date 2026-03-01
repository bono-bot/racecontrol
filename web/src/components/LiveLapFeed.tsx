import type { Lap } from "@/lib/api";

function formatLapTime(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  if (minutes > 0) {
    return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
  }
  return `${seconds}.${String(millis).padStart(3, "0")}`;
}

export default function LiveLapFeed({ laps }: { laps: Lap[] }) {
  if (laps.length === 0) {
    return (
      <div className="text-center py-8 text-zinc-500 text-sm">
        No laps recorded yet. Start a session to see live data.
      </div>
    );
  }

  return (
    <div className="space-y-1">
      {laps.map((lap, i) => (
        <div
          key={lap.id || i}
          className={`flex items-center justify-between px-3 py-2 rounded text-sm ${
            i === 0 ? "bg-orange-500/10 border border-orange-500/30" : "bg-zinc-900"
          }`}
        >
          <div className="flex items-center gap-3">
            <span className="text-zinc-500 text-xs w-6">L{lap.lap_number || "?"}</span>
            <span className="text-zinc-300">{lap.driver_id?.slice(0, 8) || "Unknown"}</span>
            <span className="text-zinc-500 text-xs">{lap.track}</span>
          </div>
          <div className="flex items-center gap-3">
            <span className="text-zinc-500 text-xs">{lap.car}</span>
            <span
              className={`font-mono font-bold ${
                lap.valid ? "text-emerald-400" : "text-red-400 line-through"
              }`}
            >
              {formatLapTime(lap.lap_time_ms)}
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}
