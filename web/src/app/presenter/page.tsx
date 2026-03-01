"use client";

import { useWebSocket } from "@/hooks/useWebSocket";
import LiveLapFeed from "@/components/LiveLapFeed";

export default function PresenterPage() {
  const { connected, pods, recentLaps } = useWebSocket();

  const activePods = pods.filter((p) => p.status === "in_session");
  const idlePods = pods.filter((p) => p.status === "idle");

  return (
    <div className="min-h-screen bg-zinc-950 p-8">
      {/* Header */}
      <div className="flex items-center justify-between mb-8">
        <div>
          <h1 className="text-4xl font-bold text-orange-500">RaceControl</h1>
          <p className="text-zinc-500">RacingPoint Bandlaguda — Live Leaderboard</p>
        </div>
        <div className="flex items-center gap-3">
          <span
            className={`w-3 h-3 rounded-full ${
              connected ? "bg-emerald-400 animate-pulse" : "bg-red-400"
            }`}
          />
          <span className="text-sm text-zinc-400">
            {activePods.length} active / {idlePods.length} idle / {pods.length} total
          </span>
        </div>
      </div>

      {/* Live Laps - Full Width */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-6">
        <h2 className="text-xl font-bold text-zinc-200 mb-4">Latest Laps</h2>
        <LiveLapFeed laps={recentLaps} />
      </div>
    </div>
  );
}
