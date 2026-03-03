"use client";

import { useKioskSocket } from "@/hooks/useKioskSocket";
import { useEffect, useState } from "react";

function formatLapTime(ms: number): string {
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = (totalSecs % 60).toFixed(3);
  return `${mins}:${parseFloat(secs) < 10 ? "0" : ""}${secs}`;
}

export default function SpectatorMode() {
  const { connected, pods, latestTelemetry, recentLaps, billingTimers } = useKioskSocket();
  const [clock, setClock] = useState("");

  useEffect(() => {
    const tick = () => {
      setClock(
        new Date().toLocaleTimeString("en-IN", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        })
      );
    };
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, []);

  const sortedPods = Array.from(pods.values()).sort((a, b) => a.number - b.number);

  return (
    <div className="h-screen flex flex-col bg-rp-black text-white overflow-hidden">
      {/* Header Bar */}
      <header className="flex items-center justify-between px-8 py-4 border-b border-rp-border">
        <h1 className="text-2xl font-bold tracking-wider uppercase">Racing Point</h1>
        <p className="text-rp-grey text-sm tracking-widest uppercase">May the Fastest Win</p>
        <span className="text-3xl font-semibold tabular-nums">{clock}</span>
      </header>

      <div className="flex-1 flex overflow-hidden">
        {/* Left: Mini Pod Grid */}
        <div className="flex-1 p-6">
          <div className="grid grid-cols-4 grid-rows-2 gap-4 h-full">
            {sortedPods.map((pod) => {
              const tel = latestTelemetry.get(pod.id);
              const billing = billingTimers.get(pod.id);
              const isActive = pod.status === "in_session";

              return (
                <div
                  key={pod.id}
                  className={`flex flex-col items-center justify-center rounded-lg border ${
                    isActive
                      ? "bg-rp-card border-rp-red/30"
                      : "bg-zinc-900 border-zinc-800"
                  }`}
                >
                  <p className="text-xs text-rp-grey mb-1">Pod {pod.number}</p>
                  {isActive && tel ? (
                    <>
                      <p className="text-4xl font-bold tabular-nums">{Math.round(tel.speed_kmh)}</p>
                      <p className="text-xs text-rp-grey">km/h</p>
                      {billing && (
                        <p className="text-xs text-rp-grey mt-1 truncate max-w-full px-2">
                          {billing.driver_name}
                        </p>
                      )}
                    </>
                  ) : isActive ? (
                    <p className="text-sm text-rp-red">Active</p>
                  ) : (
                    <p className="text-sm text-zinc-600">
                      {pod.status === "offline" ? "Offline" : "Idle"}
                    </p>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        {/* Right: Leaderboard + Lap Feed */}
        <div className="w-[400px] border-l border-rp-border flex flex-col">
          {/* Leaderboard */}
          <div className="p-4 border-b border-rp-border">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-rp-grey mb-3">
              Live Leaderboard
            </h2>
            {recentLaps.length === 0 ? (
              <p className="text-xs text-zinc-600">No laps yet</p>
            ) : (
              <div className="space-y-2">
                {/* Group best laps by driver */}
                {getBestLaps(recentLaps)
                  .slice(0, 8)
                  .map((entry, i) => (
                    <div key={entry.driver_id} className="flex items-center gap-3">
                      <span className="w-6 text-right text-sm font-bold text-rp-grey">
                        {i + 1}.
                      </span>
                      <span className="flex-1 text-sm text-white truncate">
                        {entry.driver_id.slice(0, 8)}
                      </span>
                      <span className="text-sm font-mono text-rp-red tabular-nums">
                        {formatLapTime(entry.best_lap_ms)}
                      </span>
                      <span className="text-xs text-rp-grey">{entry.track}</span>
                    </div>
                  ))}
              </div>
            )}
          </div>

          {/* Recent Laps Feed */}
          <div className="flex-1 p-4 overflow-hidden">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-rp-grey mb-3">
              Latest Laps
            </h2>
            <div className="space-y-2">
              {recentLaps.slice(0, 15).map((lap) => (
                <div key={lap.id} className="flex items-center gap-2 text-xs">
                  <span className="text-rp-grey">&#127937;</span>
                  <span className="text-white truncate flex-1">
                    {lap.driver_id.slice(0, 8)}
                  </span>
                  <span className="text-rp-grey">Lap {lap.lap_number ?? "?"}</span>
                  <span className="font-mono text-white tabular-nums">
                    {formatLapTime(lap.lap_time_ms)}
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Connection status bar */}
      {!connected && (
        <div className="absolute bottom-0 left-0 right-0 bg-rp-red/90 text-white text-center py-1 text-xs">
          Reconnecting to RaceControl...
        </div>
      )}
    </div>
  );
}

interface BestLapEntry {
  driver_id: string;
  track: string;
  best_lap_ms: number;
}

function getBestLaps(laps: { driver_id: string; track: string; lap_time_ms: number; valid: boolean }[]): BestLapEntry[] {
  const map = new Map<string, BestLapEntry>();
  for (const lap of laps) {
    if (!lap.valid) continue;
    const existing = map.get(lap.driver_id);
    if (!existing || lap.lap_time_ms < existing.best_lap_ms) {
      map.set(lap.driver_id, {
        driver_id: lap.driver_id,
        track: lap.track,
        best_lap_ms: lap.lap_time_ms,
      });
    }
  }
  return Array.from(map.values()).sort((a, b) => a.best_lap_ms - b.best_lap_ms);
}
