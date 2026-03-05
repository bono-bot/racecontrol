"use client";

import DashboardLayout from "@/components/DashboardLayout";
import { useWebSocket } from "@/hooks/useWebSocket";

const ERS_MODES = ["None", "Medium", "Hotlap", "Overtake"] as const;
const ERS_COLORS = [
  "text-rp-grey",
  "text-blue-400",
  "text-purple-400",
  "text-rp-red",
] as const;

function formatLapTime(ms: number | undefined): string {
  if (!ms || ms === 0) return "--:--.---";
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}

function formatSector(ms: number | undefined): string {
  if (!ms || ms === 0) return "--.---";
  const seconds = Math.floor(ms / 1000);
  const millis = ms % 1000;
  return `${seconds}.${String(millis).padStart(3, "0")}`;
}

export default function TelemetryPage() {
  const { connected, latestTelemetry: t } = useWebSocket();

  const isF1 = t?.drs_active !== undefined;

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Live Telemetry</h1>
          {t ? (
            <p className="text-sm text-rp-grey">
              <span className="text-rp-red font-semibold">{t.driver_name || "Unknown"}</span>
              {" "}&mdash; {t.car} @ {t.track}
            </p>
          ) : (
            <p className="text-sm text-rp-grey">Waiting for telemetry data...</p>
          )}
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

      {!t ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-12 text-center text-rp-grey">
          No live telemetry &mdash; waiting for pod connection
        </div>
      ) : (
        <div className="space-y-4">
          {/* Row 1: Speed / Gear / RPM */}
          <div className="grid grid-cols-3 gap-4">
            {/* Speed */}
            <div className="bg-rp-card border border-rp-border rounded-lg p-6 text-center">
              <div className="text-5xl font-mono font-bold text-white tracking-tight">
                {Math.round(t.speed_kmh)}
              </div>
              <div className="text-xs text-rp-grey mt-1 uppercase tracking-wider">km/h</div>
            </div>

            {/* Gear */}
            <div className="bg-rp-card border border-rp-border rounded-lg p-6 text-center">
              <div className="text-5xl font-mono font-bold text-white">
                {t.gear === 0 ? "N" : t.gear === -1 ? "R" : t.gear}
              </div>
              <div className="text-xs text-rp-grey mt-1 uppercase tracking-wider">Gear</div>
            </div>

            {/* RPM */}
            <div className="bg-rp-card border border-rp-border rounded-lg p-6">
              <div className="flex items-baseline justify-between mb-2">
                <span className="text-3xl font-mono font-bold text-white">
                  {(t.rpm / 1000).toFixed(1)}
                </span>
                <span className="text-xs text-rp-grey uppercase">x1000 RPM</span>
              </div>
              <RpmBar rpm={t.rpm} maxRpm={isF1 ? 15000 : 9000} />
            </div>
          </div>

          {/* Row 2: Throttle / Brake */}
          <div className="grid grid-cols-2 gap-4">
            <PedalBar label="Throttle" value={t.throttle} color="emerald" />
            <PedalBar label="Brake" value={t.brake} color="red" />
          </div>

          {/* Row 3: DRS / ERS (F1 only) */}
          {isF1 && (
            <div className="grid grid-cols-2 gap-4">
              <DrsIndicator active={t.drs_active!} available={t.drs_available!} />
              <ErsDisplay mode={t.ers_deploy_mode!} percent={t.ers_store_percent!} />
            </div>
          )}

          {/* Row 4: Sector Times */}
          <SectorDisplay
            currentSector={t.sector}
            sector1Ms={t.sector1_ms}
            sector2Ms={t.sector2_ms}
            sector3Ms={t.sector3_ms}
          />

          {/* Row 5: Lap Timer / Best Lap */}
          <div className="grid grid-cols-2 gap-4">
            {/* Current Lap */}
            <div className="bg-rp-card border border-rp-border rounded-lg p-5">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs text-rp-grey uppercase tracking-wider">
                  Current Lap
                </span>
                <span className="text-xs text-rp-grey">Lap {t.lap_number}</span>
              </div>
              <div
                className={`text-3xl font-mono font-bold ${
                  t.current_lap_invalid
                    ? "text-red-400 line-through"
                    : "text-white"
                }`}
              >
                {formatLapTime(t.lap_time_ms)}
              </div>
              {t.current_lap_invalid && (
                <span className="text-xs text-red-400 mt-1 inline-block">INVALID</span>
              )}
            </div>

            {/* Best Lap */}
            <div className="bg-rp-card border border-rp-border rounded-lg p-5">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs text-rp-grey uppercase tracking-wider">
                  Best Lap
                </span>
                {t.best_lap_ms && (
                  <span className="text-[10px] font-bold bg-purple-500/20 text-purple-400 px-2 py-0.5 rounded">
                    PB
                  </span>
                )}
              </div>
              <div className="text-3xl font-mono font-bold text-purple-400">
                {formatLapTime(t.best_lap_ms)}
              </div>
            </div>
          </div>
        </div>
      )}
    </DashboardLayout>
  );
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function RpmBar({ rpm, maxRpm }: { rpm: number; maxRpm: number }) {
  const percent = Math.min((rpm / maxRpm) * 100, 100);
  const redZone = 85; // Red zone starts at 85% of max

  return (
    <div className="h-3 bg-neutral-800 rounded-full overflow-hidden relative">
      {/* Red zone marker */}
      <div
        className="absolute top-0 bottom-0 bg-red-900/30 rounded-r-full"
        style={{ left: `${redZone}%`, right: 0 }}
      />
      {/* Fill */}
      <div
        className={`h-full rounded-full transition-all duration-75 ${
          percent >= redZone ? "bg-red-500" : "bg-blue-500"
        }`}
        style={{ width: `${percent}%` }}
      />
    </div>
  );
}

function PedalBar({ label, value, color }: { label: string; color: "emerald" | "red"; value: number }) {
  const percent = Math.round(value * 100);
  const bgClass = color === "emerald" ? "bg-emerald-500" : "bg-red-500";
  const textClass = color === "emerald" ? "text-emerald-400" : "text-red-400";

  return (
    <div className="bg-rp-card border border-rp-border rounded-lg p-4">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-rp-grey uppercase tracking-wider">{label}</span>
        <span className={`text-sm font-mono font-bold ${textClass}`}>{percent}%</span>
      </div>
      <div className="h-4 bg-neutral-800 rounded-full overflow-hidden">
        <div
          className={`h-full ${bgClass} rounded-full transition-all duration-75`}
          style={{ width: `${percent}%` }}
        />
      </div>
    </div>
  );
}

function DrsIndicator({ active, available }: { active: boolean; available: boolean }) {
  let bg: string;
  let text: string;
  let label: string;

  if (active) {
    bg = "bg-emerald-500/20 border-emerald-500/50";
    text = "text-emerald-400";
    label = "DRS ACTIVE";
  } else if (available) {
    bg = "bg-yellow-500/15 border-yellow-500/40";
    text = "text-yellow-400";
    label = "DRS AVAILABLE";
  } else {
    bg = "bg-rp-card border-rp-border";
    text = "text-rp-grey";
    label = "DRS OFF";
  }

  return (
    <div className={`${bg} border rounded-lg p-5 text-center`}>
      <div className={`text-2xl font-bold ${text}`}>DRS</div>
      <div className={`text-xs mt-1 ${text} uppercase tracking-wider`}>{label}</div>
    </div>
  );
}

function ErsDisplay({ mode, percent }: { mode: number; percent: number }) {
  const modeIdx = Math.min(mode, 3);
  const modeLabel = ERS_MODES[modeIdx];
  const modeColor = ERS_COLORS[modeIdx];

  // Bar color based on mode
  const barColor =
    mode === 3
      ? "bg-rp-red"
      : mode === 2
        ? "bg-purple-500"
        : mode === 1
          ? "bg-blue-500"
          : "bg-neutral-600";

  return (
    <div className="bg-rp-card border border-rp-border rounded-lg p-5">
      <div className="flex items-center justify-between mb-2">
        <span className="text-xs text-rp-grey uppercase tracking-wider">ERS Deploy</span>
        <span className={`text-sm font-bold ${modeColor}`}>{modeLabel}</span>
      </div>
      <div className="flex items-center gap-3">
        <div className="flex-1 h-4 bg-neutral-800 rounded-full overflow-hidden">
          <div
            className={`h-full ${barColor} rounded-full transition-all duration-150`}
            style={{ width: `${Math.round(percent)}%` }}
          />
        </div>
        <span className="text-xs font-mono text-neutral-400 w-10 text-right">
          {Math.round(percent)}%
        </span>
      </div>
    </div>
  );
}

function SectorDisplay({
  currentSector,
  sector1Ms,
  sector2Ms,
  sector3Ms,
}: {
  currentSector: number;
  sector1Ms?: number;
  sector2Ms?: number;
  sector3Ms?: number;
}) {
  const sectors = [
    { label: "S1", ms: sector1Ms, idx: 0 },
    { label: "S2", ms: sector2Ms, idx: 1 },
    { label: "S3", ms: sector3Ms, idx: 2 },
  ];

  return (
    <div className="grid grid-cols-3 gap-4">
      {sectors.map((s) => {
        const isActive = currentSector === s.idx;
        const hasTime = s.ms && s.ms > 0;

        return (
          <div
            key={s.label}
            className={`rounded-lg p-4 text-center border ${
              isActive
                ? "bg-rp-red/10 border-rp-red/50"
                : "bg-rp-card border-rp-border"
            }`}
          >
            <div
              className={`text-xs uppercase tracking-wider mb-1 ${
                isActive ? "text-rp-red" : "text-rp-grey"
              }`}
            >
              {s.label}
            </div>
            <div
              className={`text-lg font-mono font-bold ${
                isActive ? "text-rp-red" : hasTime ? "text-white" : "text-rp-grey"
              }`}
            >
              {hasTime ? formatSector(s.ms) : "--.---"}
            </div>
          </div>
        );
      })}
    </div>
  );
}
