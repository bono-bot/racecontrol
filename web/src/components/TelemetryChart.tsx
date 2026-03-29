"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
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

interface TelemetryChartProps {
  lapId: string;
  onClose?: () => void;
}

interface ChartDataPoint {
  time: number;
  speed: number | null;
  throttle: number | null;
  brake: number | null;
  steering: number | null;
  gear: number | null;
  rpm: number | null;
}

const darkTooltipStyle = {
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

const gridProps = {
  strokeDasharray: "3 3",
  stroke: "#333333",
  vertical: false,
};

export default function TelemetryChart({ lapId, onClose }: TelemetryChartProps) {
  const [data, setData] = useState<LapTelemetryData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);
    api
      .lapTelemetry(lapId, "100ms")
      .then((res: LapTelemetryData) => {
        if (res.error) {
          setError(res.error);
        } else {
          setData(res);
        }
      })
      .catch((err: Error) => {
        setError(err?.message || "Failed to load telemetry");
      })
      .finally(() => setLoading(false));
  }, [lapId]);

  const chartData: ChartDataPoint[] = (data?.samples || []).map((s) => ({
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

  if (loading) {
    return (
      <div className="bg-rp-card border border-rp-border rounded-lg p-6 flex items-center justify-center gap-3">
        <div className="w-5 h-5 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
        <span className="text-rp-grey text-sm">Loading telemetry...</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-rp-card border border-rp-border rounded-lg p-4">
        <div className="flex items-center justify-between">
          <p className="text-rp-red text-sm">{error}</p>
          {onClose && (
            <button onClick={onClose} className="text-rp-grey hover:text-white text-xs">
              Close
            </button>
          )}
        </div>
      </div>
    );
  }

  if (!data || chartData.length === 0) {
    return (
      <div className="bg-rp-card border border-rp-border rounded-lg p-4">
        <div className="flex items-center justify-between">
          <p className="text-rp-grey text-sm">No telemetry data available for this lap.</p>
          {onClose && (
            <button onClick={onClose} className="text-rp-grey hover:text-white text-xs">
              Close
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-rp-border">
        <div className="flex items-center gap-3 text-xs text-rp-grey min-w-0">
          <span className="text-white font-medium truncate">{data.track}</span>
          <span className="text-rp-border">|</span>
          <span className="truncate">{data.car}</span>
          <span className="text-rp-border">|</span>
          <span className="uppercase">{data.sim_type}</span>
        </div>
        {onClose && (
          <button
            onClick={onClose}
            className="w-6 h-6 flex items-center justify-center rounded-full text-rp-grey hover:text-white hover:bg-rp-border transition-colors flex-shrink-0 ml-2"
            aria-label="Close telemetry"
          >
            <svg width="10" height="10" viewBox="0 0 14 14" fill="none">
              <path d="M1 1L13 13M13 1L1 13" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          </button>
        )}
      </div>

      {/* Charts grid: side-by-side on desktop, stacked on mobile */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-0 lg:gap-0">
        {/* Speed Chart */}
        <div className="p-3 border-b lg:border-b-0 lg:border-r border-rp-border/50">
          <p className="text-[10px] text-rp-grey uppercase tracking-wider mb-2 font-medium">
            Speed (km/h)
          </p>
          <ResponsiveContainer width="100%" height={100}>
            <AreaChart data={chartData} syncId="telemetry-web">
              <defs>
                <linearGradient id={`speedFill-${lapId}`} x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#FFFFFF" stopOpacity={0.15} />
                  <stop offset="100%" stopColor="#FFFFFF" stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid {...gridProps} />
              <XAxis {...xAxisProps} hide />
              <YAxis
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={35}
              />
              <Tooltip
                {...darkTooltipStyle}
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                labelFormatter={(v: any) => `${Number(v).toFixed(2)}s`}
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                formatter={(v: any) => [`${Math.round(Number(v))} km/h`, "Speed"]}
              />
              <Area
                type="monotone"
                dataKey="speed"
                stroke="#FFFFFF"
                strokeWidth={1.5}
                fill={`url(#speedFill-${lapId})`}
                dot={false}
                isAnimationActive={true}
                animationDuration={600}
              />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Throttle & Brake Chart */}
        <div className="p-3 border-b lg:border-b-0 lg:border-r border-rp-border/50">
          <div className="flex items-center justify-between mb-2">
            <p className="text-[10px] text-rp-grey uppercase tracking-wider font-medium">
              Throttle / Brake (%)
            </p>
            <div className="flex items-center gap-2 text-[10px] text-rp-grey">
              <span className="flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-[#22C55E] inline-block" />
                Thr
              </span>
              <span className="flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-[#EF4444] inline-block" />
                Brk
              </span>
            </div>
          </div>
          <ResponsiveContainer width="100%" height={100}>
            <LineChart data={chartData} syncId="telemetry-web">
              <CartesianGrid {...gridProps} />
              <XAxis {...xAxisProps} hide />
              <YAxis
                domain={[0, 100]}
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={35}
                ticks={[0, 50, 100]}
              />
              <Tooltip
                {...darkTooltipStyle}
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                labelFormatter={(v: any) => `${Number(v).toFixed(2)}s`}
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                formatter={(v: any, name: any) => [
                  `${Math.round(Number(v))}%`,
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
                animationDuration={600}
              />
              <Line
                type="monotone"
                dataKey="brake"
                stroke="#EF4444"
                strokeWidth={1.5}
                dot={false}
                isAnimationActive={true}
                animationDuration={600}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Gear Chart */}
        <div className="p-3">
          <p className="text-[10px] text-rp-grey uppercase tracking-wider mb-2 font-medium">
            Gear
          </p>
          <ResponsiveContainer width="100%" height={100}>
            <LineChart data={chartData} syncId="telemetry-web">
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
                domain={[0, 8]}
                tick={{ fill: "#5A5A5A", fontSize: 10 }}
                tickLine={false}
                axisLine={false}
                width={35}
                ticks={[1, 2, 3, 4, 5, 6, 7, 8]}
              />
              <Tooltip
                {...darkTooltipStyle}
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                labelFormatter={(v: any) => `${Number(v).toFixed(2)}s`}
                // eslint-disable-next-line @typescript-eslint/no-explicit-any
                formatter={(v: any) => [`${Math.round(Number(v))}`, "Gear"]}
              />
              <Line
                type="stepAfter"
                dataKey="gear"
                stroke="#3B82F6"
                strokeWidth={1.5}
                dot={false}
                isAnimationActive={true}
                animationDuration={600}
              />
            </LineChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  );
}
