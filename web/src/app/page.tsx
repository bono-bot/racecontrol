"use client";

import DashboardLayout from "@/components/DashboardLayout";
import PodCard from "@/components/PodCard";
import TelemetryBar from "@/components/TelemetryBar";
import LiveLapFeed from "@/components/LiveLapFeed";
import { useWebSocket } from "@/hooks/useWebSocket";

export default function LiveOverview() {
  const { connected, pods, latestTelemetry, recentLaps, billingTimers } = useWebSocket();

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">Live Overview</h1>
          <p className="text-sm text-zinc-500">Real-time pod and telemetry monitoring</p>
        </div>
        <div className="flex items-center gap-2">
          <span
            className={`w-2 h-2 rounded-full ${
              connected ? "bg-emerald-400 animate-pulse" : "bg-red-400"
            }`}
          />
          <span className="text-xs text-zinc-500">
            {connected ? "Connected" : "Disconnected"}
          </span>
        </div>
      </div>

      {/* Telemetry Bar */}
      <div className="mb-6">
        <TelemetryBar data={latestTelemetry} />
      </div>

      {/* Pod Grid */}
      <div className="mb-6">
        <h2 className="text-sm font-medium text-zinc-400 mb-3">
          Pods ({pods.length})
        </h2>
        {pods.length === 0 ? (
          <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-8 text-center text-zinc-500 text-sm">
            No pods connected. Start rc-agent on sim PCs to see them here.
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
            {pods.map((pod) => (
              <PodCard key={pod.id} pod={pod} billingSession={billingTimers.get(pod.id)} />
            ))}
          </div>
        )}
      </div>

      {/* Live Lap Feed */}
      <div>
        <h2 className="text-sm font-medium text-zinc-400 mb-3">Recent Laps</h2>
        <LiveLapFeed laps={recentLaps} />
      </div>
    </DashboardLayout>
  );
}
