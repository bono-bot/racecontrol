"use client";

import type { TelemetryFrame } from "@/lib/types";

interface LiveTelemetryProps {
  telemetry: TelemetryFrame;
}

function formatLapTime(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = (totalSecs % 60).toFixed(3);
  return `${mins}:${parseFloat(secs) < 10 ? "0" : ""}${secs}`;
}

export function LiveTelemetry({ telemetry }: LiveTelemetryProps) {
  const rpmPercent = Math.min(100, (telemetry.rpm / 18000) * 100);

  return (
    <div className="space-y-2">
      {/* Speed + Gear + Lap row */}
      <div className="flex items-end gap-4">
        {/* Speed */}
        <div>
          <p className="text-3xl font-bold text-white tabular-nums leading-none">
            {Math.round(telemetry.speed_kmh)}
          </p>
          <p className="text-[10px] text-rp-grey uppercase mt-0.5">km/h</p>
        </div>

        {/* Gear */}
        <div className="text-center">
          <p className="text-2xl font-bold text-white leading-none">
            {telemetry.gear === 0 ? "N" : telemetry.gear === -1 ? "R" : telemetry.gear}
          </p>
          <p className="text-[10px] text-rp-grey uppercase mt-0.5">Gear</p>
        </div>

        {/* Lap */}
        <div className="ml-auto text-right">
          <p className="text-sm font-semibold text-white">Lap {telemetry.lap_number}</p>
          <p className="text-xs text-rp-grey font-mono tabular-nums">
            {formatLapTime(telemetry.lap_time_ms)}
          </p>
        </div>
      </div>

      {/* RPM bar */}
      <div>
        <div className="w-full h-1 bg-zinc-800 rounded-full overflow-hidden">
          <div
            className={`h-full rounded-full transition-all duration-100 ${
              rpmPercent > 90 ? "bg-rp-red" : rpmPercent > 70 ? "bg-amber-500" : "bg-green-500"
            }`}
            style={{ width: `${rpmPercent}%` }}
          />
        </div>
        <p className="text-[9px] text-rp-grey mt-0.5 text-right tabular-nums">
          {telemetry.rpm.toLocaleString()} RPM
        </p>
      </div>

      {/* Track + Car */}
      <p className="text-xs text-rp-grey truncate">
        {telemetry.track} — {telemetry.car}
      </p>
    </div>
  );
}
