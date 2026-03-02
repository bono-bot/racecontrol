"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import type { Lap } from "@/lib/api";
import { api } from "@/lib/api";

function formatLapTime(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  if (minutes > 0) {
    return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
  }
  return `${seconds}.${String(millis).padStart(3, "0")}`;
}

export default function LeaderboardsPage() {
  const [laps, setLaps] = useState<Lap[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listLaps().then((res) => {
      setLaps(res.laps || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  // Group by track+car, pick best per driver
  const grouped = new Map<string, Lap[]>();
  for (const lap of laps.filter((l) => l.valid)) {
    const key = `${lap.track} | ${lap.car}`;
    if (!grouped.has(key)) grouped.set(key, []);
    grouped.get(key)!.push(lap);
  }

  // Sort each group by lap time
  for (const [, group] of grouped) {
    group.sort((a, b) => a.lap_time_ms - b.lap_time_ms);
  }

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">Leaderboards</h1>
        <p className="text-sm text-rp-grey">Track records and personal bests</p>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading leaderboards...</div>
      ) : grouped.size === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No lap records yet</p>
          <p className="text-rp-grey text-sm">
            Leaderboards populate as drivers complete valid laps.
          </p>
        </div>
      ) : (
        <div className="space-y-6">
          {Array.from(grouped.entries()).map(([combo, comboLaps]) => (
            <div key={combo}>
              <h3 className="text-sm font-medium text-rp-red mb-2">{combo}</h3>
              <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
                {comboLaps.slice(0, 10).map((lap, i) => (
                  <div
                    key={lap.id}
                    className={`flex items-center justify-between px-4 py-2 text-sm ${
                      i < comboLaps.length - 1 ? "border-b border-rp-border/50" : ""
                    }`}
                  >
                    <div className="flex items-center gap-3">
                      <span
                        className={`w-6 text-center font-mono font-bold ${
                          i === 0
                            ? "text-yellow-400"
                            : i === 1
                            ? "text-neutral-400"
                            : i === 2
                            ? "text-amber-600"
                            : "text-rp-grey"
                        }`}
                      >
                        {i + 1}
                      </span>
                      <span className="text-neutral-300">
                        {lap.driver_id?.slice(0, 8) || "Unknown"}
                      </span>
                    </div>
                    <span className="font-mono font-bold text-emerald-400">
                      {formatLapTime(lap.lap_time_ms)}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
