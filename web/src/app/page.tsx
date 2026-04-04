"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import MetricCard from "@/components/MetricCard";
import PodCard from "@/components/PodCard";
import TelemetryBar from "@/components/TelemetryBar";
import LiveLapFeed from "@/components/LiveLapFeed";
import { EmptyState } from "@/components/Skeleton";
import { useWebSocket } from "@/hooks/useWebSocket";
import { api, racingWsPodsOnly } from "@/lib/api";

function MonitorIcon() {
  return (
    <svg className="w-10 h-10" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25A2.25 2.25 0 015.25 3h13.5A2.25 2.25 0 0121 5.25z" />
    </svg>
  );
}

export default function LiveOverview() {
  const { connected, pods, latestTelemetry, recentLaps, billingTimers, pendingAuthTokens, sendCommand } = useWebSocket();
  const [revenueToday, setRevenueToday] = useState<string | null>(null);

  // Fetch daily revenue on mount
  useEffect(() => {
    api.dailyBillingReport()
      .then((report) => {
        const rupees = Math.round(report.total_revenue_paise / 100);
        setRevenueToday(`\u20B9${rupees.toLocaleString("en-IN")}`);
      })
      .catch(() => {
        setRevenueToday("\u2014");
      });
  }, []);

  const racingPods = pods ? racingWsPodsOnly(pods) : [];
  const sortedPods = [...racingPods].sort((a, b) => a.number - b.number);
  const podsOnline = racingPods.filter((p) => p.status !== "offline").length;

  return (
    <DashboardLayout>
      {/* Page header */}
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

      {/* MetricCard KPI row */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
        <MetricCard
          title="Active Sessions"
          value={billingTimers.size}
        />
        <MetricCard
          title="Pods Online"
          value={`${podsOnline}/${pods.length}`}
          alert={podsOnline === 0 && pods.length > 0}
        />
        <MetricCard
          title="Revenue Today"
          value={revenueToday ?? undefined}
          loading={revenueToday === null}
        />
        <MetricCard
          title="Queue"
          value={pendingAuthTokens.size}
          alert={pendingAuthTokens.size > 3}
        />
      </div>

      {/* Telemetry Bar */}
      <div className="mb-6">
        <TelemetryBar data={latestTelemetry} />
      </div>

      {/* F1 Timing Tower Pod Strip */}
      <div className="mb-6">
        <h2 className="text-sm font-medium text-neutral-400 mb-3">
          Pods ({pods.length})
        </h2>
        {pods.length === 0 ? (
          <EmptyState
            icon={<MonitorIcon />}
            headline="No pods connected"
            hint="Start rc-agent on sim PCs to see them here."
          />
        ) : (
          <div className="space-y-1">
            {sortedPods.map((pod) => (
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
