"use client";

import { useEffect, useState, useCallback } from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import DashboardLayout from "@/components/DashboardLayout";
import {
  fetchMetricNames,
  fetchMetricSnapshot,
  fetchMetricQuery,
  type SnapshotEntry,
  type TimePoint,
} from "@/lib/api/tsdb";

// ─── Time range config ────────────────────────────────────────────────────────

const TIME_RANGES: { label: string; seconds: number }[] = [
  { label: "1h", seconds: 3600 },
  { label: "6h", seconds: 21600 },
  { label: "24h", seconds: 86400 },
  { label: "7d", seconds: 604800 },
  { label: "30d", seconds: 2592000 },
];

function formatTs(ts: number): string {
  const d = new Date(ts * 1000);
  return d.toLocaleTimeString("en-IN", { hour: "2-digit", minute: "2-digit", hour12: false });
}

function formatValue(v: number): string {
  if (Math.abs(v) >= 1_000_000) return `${(v / 1_000_000).toFixed(2)}M`;
  if (Math.abs(v) >= 1_000) return `${(v / 1_000).toFixed(2)}k`;
  return v.toFixed(2);
}

// ─── Page component ───────────────────────────────────────────────────────────

export default function MetricsDashboardPage() {
  const [names, setNames] = useState<string[]>([]);
  const [snapshot, setSnapshot] = useState<SnapshotEntry[]>([]);
  const [selectedPod, setSelectedPod] = useState<number | null>(null);
  const [timeRange, setTimeRange] = useState("1h");
  const [chartData, setChartData] = useState<Map<string, TimePoint[]>>(new Map());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);

  // Derive unique pod list from snapshot
  const podList: number[] = Array.from(
    new Set(snapshot.map((e) => e.pod).filter((p): p is number => p !== null))
  ).sort((a, b) => a - b);

  const rangeSeconds =
    TIME_RANGES.find((r) => r.label === timeRange)?.seconds ?? 3600;

  const loadData = useCallback(async () => {
    try {
      const nowEpoch = Math.floor(Date.now() / 1000);
      const fromEpoch = nowEpoch - rangeSeconds;

      const [fetchedNames, fetchedSnapshot] = await Promise.all([
        fetchMetricNames(),
        fetchMetricSnapshot(selectedPod ?? undefined),
      ]);

      setNames(fetchedNames);
      setSnapshot(fetchedSnapshot);

      // Fetch time-series for each name in parallel
      const queries = await Promise.allSettled(
        fetchedNames.map((name) =>
          fetchMetricQuery(name, fromEpoch, nowEpoch, selectedPod ?? undefined)
        )
      );

      const newChartData = new Map<string, TimePoint[]>();
      fetchedNames.forEach((name, i) => {
        const result = queries[i];
        if (result.status === "fulfilled") {
          newChartData.set(name, result.value.points);
        } else {
          newChartData.set(name, []);
        }
      });

      setChartData(newChartData);
      setLastUpdated(new Date());
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load metrics");
    } finally {
      setLoading(false);
    }
  }, [selectedPod, rangeSeconds]);

  useEffect(() => {
    setLoading(true);
    loadData();
    const interval = setInterval(loadData, 30_000);
    return () => clearInterval(interval);
  }, [loadData]);

  // Latest snapshot value for a given metric (optionally filtered by pod)
  const latestValue = (name: string): number | null => {
    const matches = snapshot.filter((e) => e.name === name);
    if (matches.length === 0) return null;
    // Sort by updated_at descending, take first
    const sorted = [...matches].sort((a, b) => b.updated_at - a.updated_at);
    return sorted[0].value;
  };

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Metrics Dashboard</h1>
          <p className="text-sm text-zinc-400">
            Real-time TSDB telemetry from all pods
          </p>
        </div>
        {lastUpdated && (
          <span className="text-xs text-zinc-400 font-mono">
            Updated:{" "}
            {lastUpdated.toLocaleTimeString("en-IN", {
              hour: "2-digit",
              minute: "2-digit",
              second: "2-digit",
              hour12: false,
            })}
          </span>
        )}
      </div>

      {/* Controls */}
      <div className="flex flex-wrap items-center gap-4 mb-6">
        {/* Time range selector */}
        <div className="flex items-center gap-1 bg-zinc-800 rounded-lg p-1">
          {TIME_RANGES.map((r) => (
            <button
              key={r.label}
              onClick={() => setTimeRange(r.label)}
              className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
                timeRange === r.label
                  ? "bg-rp-red text-white"
                  : "text-zinc-400 hover:text-white"
              }`}
            >
              {r.label}
            </button>
          ))}
        </div>

        {/* Pod selector */}
        <div className="flex items-center gap-1 bg-zinc-800 rounded-lg p-1">
          <button
            onClick={() => setSelectedPod(null)}
            className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
              selectedPod === null
                ? "bg-rp-red text-white"
                : "text-zinc-400 hover:text-white"
            }`}
          >
            All
          </button>
          {podList.map((p) => (
            <button
              key={p}
              onClick={() => setSelectedPod(p)}
              className={`px-3 py-1 rounded text-sm font-medium transition-colors ${
                selectedPod === p
                  ? "bg-rp-red text-white"
                  : "text-zinc-400 hover:text-white"
              }`}
            >
              Pod {p}
            </button>
          ))}
        </div>
      </div>

      {/* Error state */}
      {error && (
        <div className="mb-6 p-4 bg-red-900/20 border border-red-700 rounded-lg text-red-400 text-sm">
          {error}
        </div>
      )}

      {/* Loading skeleton */}
      {loading && (
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 mb-6">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="bg-zinc-800 rounded-lg p-4 h-24 animate-pulse" />
          ))}
        </div>
      )}

      {/* Empty state */}
      {!loading && names.length === 0 && (
        <div className="text-center py-16 text-zinc-500">
          <p className="text-lg">No metrics available</p>
          <p className="text-sm mt-1">
            Metrics are emitted by pods and stored by the server TSDB pipeline.
          </p>
        </div>
      )}

      {/* Headline cards */}
      {!loading && names.length > 0 && (
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4 mb-8">
          {names.map((name) => {
            const val = latestValue(name);
            return (
              <div key={name} className="bg-zinc-800 rounded-lg p-4 border border-zinc-700">
                <p className="text-xs text-zinc-400 truncate font-mono mb-1">{name}</p>
                <p className="text-2xl font-bold text-white">
                  {val !== null ? formatValue(val) : "—"}
                </p>
                <p className="text-xs text-zinc-500 mt-1">latest</p>
              </div>
            );
          })}
        </div>
      )}

      {/* Sparkline charts */}
      {!loading && names.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {names.map((name) => {
            const points = chartData.get(name) ?? [];
            return (
              <div key={name} className="bg-zinc-800 rounded-lg p-4 border border-zinc-700">
                <h3 className="text-sm font-semibold text-white font-mono mb-4">{name}</h3>
                {points.length === 0 ? (
                  <div className="h-32 flex items-center justify-center text-zinc-500 text-sm">
                    No data for selected range
                  </div>
                ) : (
                  <ResponsiveContainer width="100%" height={128}>
                    <AreaChart
                      data={points}
                      margin={{ top: 4, right: 4, left: 0, bottom: 0 }}
                    >
                      <defs>
                        <linearGradient id={`grad-${name}`} x1="0" y1="0" x2="0" y2="1">
                          <stop offset="5%" stopColor="#E10600" stopOpacity={0.3} />
                          <stop offset="95%" stopColor="#E10600" stopOpacity={0} />
                        </linearGradient>
                      </defs>
                      <CartesianGrid strokeDasharray="3 3" stroke="#333" />
                      <XAxis
                        dataKey="ts"
                        tickFormatter={formatTs}
                        tick={{ fill: "#71717a", fontSize: 10 }}
                        axisLine={false}
                        tickLine={false}
                      />
                      <YAxis
                        tickFormatter={formatValue}
                        tick={{ fill: "#71717a", fontSize: 10 }}
                        axisLine={false}
                        tickLine={false}
                        width={48}
                      />
                      <Tooltip
                        contentStyle={{
                          background: "#1a1a1a",
                          border: "1px solid #333",
                          borderRadius: 6,
                          fontSize: 12,
                        }}
                        labelFormatter={(label) => formatTs(label as number)}
                        formatter={(value) => [formatValue(value as number), name]}
                      />
                      <Area
                        type="monotone"
                        dataKey="value"
                        stroke="#E10600"
                        strokeWidth={1.5}
                        fill={`url(#grad-${name})`}
                        dot={false}
                        isAnimationActive={false}
                      />
                    </AreaChart>
                  </ResponsiveContainer>
                )}
              </div>
            );
          })}
        </div>
      )}
    </DashboardLayout>
  );
}
