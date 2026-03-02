"use client";

import DashboardLayout from "@/components/DashboardLayout";
import PodCard from "@/components/PodCard";
import TelemetryBar from "@/components/TelemetryBar";
import LiveLapFeed from "@/components/LiveLapFeed";
import { useWebSocket } from "@/hooks/useWebSocket";

export default function LiveOverview() {
  const { connected, pods, latestTelemetry, recentLaps, billingTimers, pendingAuthTokens, sendCommand } = useWebSocket();

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Live Overview</h1>
          <p className="text-sm text-rp-grey">Real-time pod and telemetry monitoring</p>
        </div>
        <div className="flex items-center gap-2">
          <span
            className={`w-2 h-2 rounded-full ${
              connected ? "bg-emerald-400 animate-pulse" : "bg-red-400"
            }`}
          />
          <span className="text-xs text-rp-grey">
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
        <h2 className="text-sm font-medium text-neutral-400 mb-3">
          Pods ({pods.length})
        </h2>
        {pods.length === 0 ? (
          <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center text-rp-grey text-sm">
            No pods connected. Start rc-agent on sim PCs to see them here.
          </div>
        ) : (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
            {pods.map((pod) => (
              <PodCard
                key={pod.id}
                pod={pod}
                billingSession={billingTimers.get(pod.id)}
                pendingToken={pendingAuthTokens.get(pod.id)}
                onCancelToken={(tokenId) =>
                  sendCommand("cancel_assignment", { token_id: tokenId })
                }
              />
            ))}
          </div>
        )}
      </div>

      {/* Live Lap Feed */}
      <div>
        <h2 className="text-sm font-medium text-neutral-400 mb-3">Recent Laps</h2>
        <LiveLapFeed laps={recentLaps} />
      </div>
    </DashboardLayout>
  );
}
