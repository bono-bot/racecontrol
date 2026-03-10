"use client";

import { useEffect, useState } from "react";
import { publicApi } from "@/lib/api";
import type { LapTelemetryData } from "@/lib/api";
import {
  LineChart,
  Line,
  AreaChart,
  Area,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";

function formatLapTime(ms: number): string {
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis
    .toString()
    .padStart(3, "0")}`;
}

interface TelemetryChartProps {
  lapId: string;
  onClose: () => void;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const darkTooltipStyle: any = {
  contentStyle: {
    backgroundColor: "#222222",
    border: "1px solid #333333",
    borderRadius: "8px",
    fontSize: "12px",
    color: "#ffffff",
  },
  labelStyle: { color: "#999999" },
  itemStyle: { padding: "2px 0" },
};

export default function TelemetryChart({ lapId, onClose }: TelemetryChartProps) {
  const [data, setData] = useState<LapTelemetryData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);
    publicApi
      .lapTelemetry(lapId)
      .then((res) => {
        if (res.error) {
          setError(res.error);
        } else {
          setData(res);
        }
      })
      .catch((err) => {
        setError(err?.message || "Failed to load telemetry");
      })
      .finally(() => setLoading(false));
  }, [lapId]);

  // Transform samples for recharts: offset_ms -> time_s
  const chartData = (data?.samples || []).map((s) => ({
    time: Number((s.offset_ms / 1000).toFixed(2)),
    speed: s.speed,
    throttle: s.throttle,
    brake: s.brake,
    steering: s.steering,
    gear: s.gear,
    rpm: s.rpm,
  }));

  const xAxisProps = {
    dataKey: "time" as const,
    type: "number" as const,
    domain: ["dataMin", "dataMax"] as [string, string],
    tick: { fill: "#5A5A5A", fontSize: 10 },
    tickLine: false,
    axisLine: { stroke: "#333333" },
  };

  const gridProps = {
    strokeDasharray: "3 3",
    stroke: "#333333",
    vertical: false,
  };

  if (loading) {
    return (
      <div className="fixed inset-0 z-50 bg-rp-dark/95 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3">
          <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
          <p className="text-rp-grey text-sm">Loading telemetry...</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="fixed inset-0 z-50 bg-rp-dark/95 flex items-center justify-center">
        <div className="bg-rp-card border border-rp-border rounded-xl p-6 max-w-sm mx-4 text-center">
          <p className="text-rp-red text-sm mb-4">{error}</p>
          <button
            onClick={onClose}
            className="px-4 py-2 bg-rp-card border border-rp-border rounded-lg text-sm text-white hover:bg-rp-border transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    );
  }

  if (!data || chartData.length === 0) {
    return (
      <div className="fixed inset-0 z-50 bg-rp-dark/95 flex items-center justify-center">
        <div className="bg-rp-card border border-rp-border rounded-xl p-6 max-w-sm mx-4 text-center">
          <p className="text-rp-grey text-sm mb-4">No telemetry data available for this lap.</p>
          <button
            onClick={onClose}
            className="px-4 py-2 bg-rp-card border border-rp-border rounded-lg text-sm text-white hover:bg-rp-border transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-50 bg-rp-dark/95 overflow-y-auto animate-in fade-in duration-200">
      {/* Header */}
      <div className="sticky top-0 z-10 bg-rp-dark/90 backdrop-blur-sm border-b border-rp-border px-4 py-3">
        <div className="max-w-3xl mx-auto flex items-center justify-between">
          <div className="min-w-0">
            <h2 className="text-base font-bold text-white truncate">
              {data.track}
            </h2>
            <div className="flex items-center gap-2 text-xs text-rp-grey">
              <span className="truncate">{data.car}</span>
              <span className="text-rp-border">|</span>
              <span className="font-mono text-white">
                {formatLapTime(data.lap_time_ms)}
              </span>
              <span className="text-rp-border">|</span>
              <span className="uppercase">{data.sim_type}</span>
            </div>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 flex items-center justify-center rounded-full bg-rp-card border border-rp-border text-rp-grey hover:text-white hover:border-rp-red transition-colors flex-shrink-0 ml-3"
            aria-label="Close"
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path
                d="M1 1L13 13M13 1L1 13"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
              />
            </svg>
          </button>
        </div>
      </div>

      {/* Sector times (if available) */}
      {(data.sector1_ms || data.sector2_ms || data.sector3_ms) && (
        <div className="max-w-3xl mx-auto px-4 pt-3">
          <div className="flex gap-2">
            {[data.sector1_ms, data.sector2_ms, data.sector3_ms].map((s, i) =>
              s ? (
                <div
                  key={i}
                  className="flex-1 bg-rp-card border border-rp-border rounded-lg px-3 py-1.5 text-center"
                >
                  <p className="text-[10px] text-rp-grey uppercase">S{i + 1}</p>
                  <p className="text-xs font-mono text-white">
                    {(s / 1000).toFixed(3)}
                  </p>
                </div>
              ) : null
            )}
          </div>
        </div>
      )}

      {/* Charts */}
      <div className="max-w-3xl mx-auto px-4 py-4 space-y-3 pb-8">
        {/* Speed Chart */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-3">
          <p className="text-[10px] text-rp-grey uppercase tracking-wider mb-2 font-medium">
            Speed (km/h)
          </p>
          <ResponsiveContainer width="100%" height={120}>
            <AreaChart data={chartData} syncId="telemetry">
              <defs>
                <linearGradient id="speedFill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#3B82F6" stopOpacity={0.3} />
                  <stop offset="100%" stopColor="#3B82F6" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid {...gridProps} />
              <XAxis {...xAxisProps} hide />
              <YAxis
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={40}
              />
              <Tooltip
                {...darkTooltipStyle}
                labelFormatter={(v: number) => `${v.toFixed(2)}s`}
                formatter={(v: number) => [`${Math.round(v)} km/h`, "Speed"]}
              />
              <Area
                type="monotone"
                dataKey="speed"
                stroke="#3B82F6"
                strokeWidth={1.5}
                fill="url(#speedFill)"
                dot={false}
                isAnimationActive={true}
                animationDuration={800}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Throttle & Brake Chart */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-3">
          <p className="text-[10px] text-rp-grey uppercase tracking-wider mb-2 font-medium">
            Throttle / Brake (%)
          </p>
          <ResponsiveContainer width="100%" height={120}>
            <LineChart data={chartData} syncId="telemetry">
              <CartesianGrid {...gridProps} />
              <XAxis {...xAxisProps} hide />
              <YAxis
                domain={[0, 100]}
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={40}
                ticks={[0, 50, 100]}
              />
              <Tooltip
                {...darkTooltipStyle}
                labelFormatter={(v: number) => `${v.toFixed(2)}s`}
                formatter={(v: number, name: string) => [
                  `${Math.round(v)}%`,
                  name === "throttle" ? "Throttle" : "Brake",
                ]}
              />
              <Line
                type="monotone"
                dataKey="throttle"
                stroke="#22C55E"
                strokeWidth={1.5}
                dot={false}
                isAnimationActive={true}
                animationDuration={800}
              />
              <Line
                type="monotone"
                dataKey="brake"
                stroke="#EF4444"
                strokeWidth={1.5}
                dot={false}
                isAnimationActive={true}
                animationDuration={800}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Steering Chart */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-3">
          <p className="text-[10px] text-rp-grey uppercase tracking-wider mb-2 font-medium">
            Steering
          </p>
          <ResponsiveContainer width="100%" height={120}>
            <LineChart data={chartData} syncId="telemetry">
              <CartesianGrid {...gridProps} />
              <XAxis {...xAxisProps} hide />
              <YAxis
                domain={[-100, 100]}
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={40}
                ticks={[-100, 0, 100]}
              />
              <Tooltip
                {...darkTooltipStyle}
                labelFormatter={(v: number) => `${v.toFixed(2)}s`}
                formatter={(v: number) => [
                  `${v > 0 ? "+" : ""}${Math.round(v)}`,
                  "Steering",
                ]}
              />
              <Line
                type="monotone"
                dataKey="steering"
                stroke="#F97316"
                strokeWidth={1.5}
                dot={false}
                isAnimationActive={true}
                animationDuration={800}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Gear & RPM Chart */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-3">
          <div className="flex items-center justify-between mb-2">
            <p className="text-[10px] text-rp-grey uppercase tracking-wider font-medium">
              Gear / RPM
            </p>
            <div className="flex items-center gap-3 text-[10px] text-rp-grey">
              <span className="flex items-center gap-1">
                <span className="w-2 h-2 rounded-full bg-white inline-block" />
                Gear
              </span>
              <span className="flex items-center gap-1">
                <span className="w-2 h-2 rounded-full bg-neutral-600 inline-block" />
                RPM
              </span>
            </div>
          </div>
          <ResponsiveContainer width="100%" height={120}>
            <AreaChart data={chartData} syncId="telemetry">
              <defs>
                <linearGradient id="rpmFill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#6B7280" stopOpacity={0.2} />
                  <stop offset="100%" stopColor="#6B7280" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid {...gridProps} />
              <XAxis
                {...xAxisProps}
                label={{
                  value: "Time (s)",
                  position: "insideBottomRight",
                  offset: -5,
                  style: { fill: "#5A5A5A", fontSize: 10 },
                }}
              />
              <YAxis
                yAxisId="gear"
                domain={[0, 8]}
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={30}
                ticks={[1, 2, 3, 4, 5, 6, 7, 8]}
              />
              <YAxis
                yAxisId="rpm"
                orientation="right"
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={50}
                tickFormatter={(v: number) =>
                  v >= 1000 ? `${(v / 1000).toFixed(0)}k` : `${v}`
                }
              />
              <Tooltip
                {...darkTooltipStyle}
                labelFormatter={(v: number) => `${v.toFixed(2)}s`}
                formatter={(v: number, name: string) => {
                  if (name === "gear") return [`${Math.round(v)}`, "Gear"];
                  return [`${Math.round(v).toLocaleString()}`, "RPM"];
                }}
              />
              <Area
                yAxisId="rpm"
                type="monotone"
                dataKey="rpm"
                stroke="#6B7280"
                strokeWidth={1}
                fill="url(#rpmFill)"
                dot={false}
                isAnimationActive={true}
                animationDuration={800}
              />
              <Line
                yAxisId="gear"
                type="stepAfter"
                dataKey="gear"
                stroke="#FFFFFF"
                strokeWidth={1.5}
                dot={false}
                isAnimationActive={true}
                animationDuration={800}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  );
}
