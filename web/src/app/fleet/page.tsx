"use client";

import { useEffect, useState } from "react";
import { AlertTriangle } from "lucide-react";
import DashboardLayout from "@/components/DashboardLayout";
import MetricCard from "@/components/MetricCard";
import StatusBadge from "@/components/StatusBadge";
import { Skeleton, EmptyState } from "@/components/Skeleton";
import { api } from "@/lib/api";
import type { PodFleetStatus } from "@/lib/api";

function formatUptime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

export default function FleetHealthPage() {
  const [pods, setPods] = useState<PodFleetStatus[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);

  useEffect(() => {
    const load = () => {
      api
        .fleetHealth()
        .then((data) => {
          setPods(data || []);
          setLastUpdated(new Date());
          setLoading(false);
          setError(false);
        })
        .catch(() => {
          setLoading(false);
          setError(true);
        });
    };
    load();
    const interval = setInterval(load, 30_000);
    return () => clearInterval(interval);
  }, []);

  // Summary metrics
  const onlineCount = pods.filter(
    (p) => p.ws_connected && p.http_reachable
  ).length;
  const offlineCount = pods.filter(
    (p) => !p.ws_connected || !p.http_reachable
  ).length;
  const buildIds = new Set(pods.map((p) => p.build_id));
  const buildConsistency =
    buildIds.size <= 1 ? "Uniform" : `${buildIds.size} variants`;

  const sortedPods = [...pods].sort((a, b) => a.pod_number - b.pod_number);

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Fleet Health</h1>
          <p className="text-sm text-rp-grey">
            Real-time status of all racing pods
          </p>
        </div>
        {lastUpdated && (
          <span className="text-xs text-rp-grey font-mono">
            Last updated:{" "}
            {lastUpdated.toLocaleTimeString("en-IN", {
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
              hour12: false,
            })}
          </span>
        )}
      </div>

      {/* Summary MetricCards */}
      <div className="grid grid-cols-3 gap-4 mb-6">
        <MetricCard
          title="Online"
          value={loading ? undefined : onlineCount}
          loading={loading}
          alert={!loading && onlineCount < pods.length && pods.length > 0}
        />
        <MetricCard
          title="Offline"
          value={loading ? undefined : offlineCount}
          loading={loading}
          alert={!loading && offlineCount > 0}
        />
        <MetricCard
          title="Build Consistency"
          value={loading ? undefined : buildConsistency}
          loading={loading}
          alert={!loading && buildIds.size > 1}
        />
      </div>

      {/* Error state */}
      {error && !loading && pods.length === 0 && (
        <EmptyState
          icon={<AlertTriangle className="w-12 h-12" />}
          headline="Fleet health unavailable"
          hint="Check server connectivity at 192.168.31.23:8080"
        />
      )}

      {/* Loading skeleton grid */}
      {loading && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-36 rounded-lg" />
          ))}
        </div>
      )}

      {/* Pod health grid */}
      {!loading && pods.length > 0 && (
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
          {sortedPods.map((pod) => {
            const isOnline = pod.ws_connected && pod.http_reachable;
            return (
              <div
                key={pod.pod_number}
                className={`rounded-lg border p-4 ${
                  isOnline
                    ? "border-emerald-500/30 bg-emerald-500/5"
                    : "border-red-500/30 bg-red-500/5"
                }`}
              >
                <div className="flex items-center justify-between mb-3">
                  <span className="text-2xl font-bold font-mono text-white">
                    {String(pod.pod_number).padStart(2, "0")}
                  </span>
                  <StatusBadge status={isOnline ? "idle" : "offline"} />
                </div>
                <div className="space-y-1 text-xs">
                  <div className="flex justify-between">
                    <span className="text-rp-grey">Build</span>
                    <span className="font-mono text-neutral-300">
                      {pod.build_id.slice(0, 8)}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-rp-grey">Uptime</span>
                    <span className="text-neutral-300">
                      {formatUptime(pod.uptime_secs)}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-rp-grey">WS</span>
                    <span
                      className={
                        pod.ws_connected
                          ? "text-emerald-400"
                          : "text-red-400"
                      }
                    >
                      {pod.ws_connected ? "Connected" : "Disconnected"}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-rp-grey">HTTP</span>
                    <span
                      className={
                        pod.http_reachable
                          ? "text-emerald-400"
                          : "text-red-400"
                      }
                    >
                      {pod.http_reachable ? "Reachable" : "Unreachable"}
                    </span>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </DashboardLayout>
  );
}
