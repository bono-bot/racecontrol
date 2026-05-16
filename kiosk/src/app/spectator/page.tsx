"use client";

import { useKioskSocket } from "@/hooks/useKioskSocket";
import { BrandLogo } from "@/components/BrandLogo";
import { useEffect, useState, useMemo, useCallback, useRef } from "react";
import type { Lap, TelemetryFrame, BillingSession, Pod, GameLaunchInfo } from "@/lib/types";
import { KioskLeaderboard } from "@/components/KioskLeaderboard";

// ─── Constants ────────────────────────────────────────────────────────────────

const TRACE_HISTORY_MS = 10_000;
const TRACE_SLICES = 20;
const SIDEBAR_WIDTH = 450;

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formatLapTime(ms: number): string {
  if (!ms || ms <= 0) return "--:--.---";
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = totalSecs % 60;
  return `${mins}:${secs < 10 ? "0" : ""}${secs.toFixed(3)}`;
}

function formatGap(gapMs: number): string {
  if (gapMs <= 0) return "";
  return `+${(gapMs / 1000).toFixed(3)}`;
}

function formatSessionTimer(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function prettyName(raw: string): string {
  if (!raw) return "Unknown";
  const segment = raw.split(/[/\\]/).pop() || raw;
  return segment
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .trim();
}

// TODO: V2 substrate ambiguity — game-brand badges (F1 red, ACR orange, ACE teal,
//   FH5 yellow, AC green, iR purple, LMU/ACC blue, FRZ amber, default zinc) use
//   Tailwind defaults because the V2 substrate defines only rp-red/green/yellow/purple
//   plus neutrals. Collapsing 9 distinct game identifiers into the 4 substrate hues
//   would lose UX information density (drivers visually identify their sim by badge
//   color at a glance). Brand-color tokens are out of scope for the kiosk substrate
//   and arguably belong to a separate "game brands" registry. Kept as Tailwind
//   defaults pending substrate decision.
function gameLabel(simType: string): { label: string; color: string } {
  const s = (simType || "").toLowerCase();
  if (s.includes("f1") || s === "f1_25" || s === "f1_24") return { label: "F1", color: "bg-red-600" };
  if (s === "assetto_corsa_rally" || s.includes("acr")) return { label: "ACR", color: "bg-orange-600" };
  if (s === "assetto_corsa_evo" || s.includes("ace")) return { label: "ACE", color: "bg-teal-600" };
  if (s === "forza_horizon_5" || s.includes("fh5")) return { label: "FH5", color: "bg-yellow-600" };
  if (s.includes("assetto") || s === "assetto_corsa") return { label: "AC", color: "bg-green-600" };
  if (s.includes("iracing") || s.includes("i_racing") || s === "iracing") return { label: "iR", color: "bg-purple-600" };
  if (s.includes("le_mans") || s === "le_mans_ultimate" || s === "lmu") return { label: "LMU", color: "bg-blue-600" };
  if (s.includes("forza")) return { label: "FRZ", color: "bg-amber-600" };
  if (s.includes("acc")) return { label: "ACC", color: "bg-blue-600" };
  return { label: simType?.toUpperCase()?.slice(0, 3) || "---", color: "bg-zinc-600" };
}

// ─── Data structures ──────────────────────────────────────────────────────────

interface TracePoint {
  t: number;
  throttle: number;
  brake: number;
}

interface RigRow {
  podId: string;
  podNumber: number;
  driverName: string;
  simType: string;
  lapNumber: number;
  lastLapMs: number;
  bestLapMs: number;
  gapMs: number;
  status: "live" | "pit" | "idle";
  isOverallBest: boolean;
  isPersonalBest: boolean;
}

interface ActivityItem {
  id: string;
  type: "fastest" | "joined" | "pit" | "disconnect" | "lap";
  text: string;
  time: string;
  lapTimeMs?: number;
}

// ─── Build driver name map ────────────────────────────────────────────────────

function buildDriverNameMap(
  billingTimers: Map<string, BillingSession>,
  latestTelemetry: Map<string, TelemetryFrame>
): Map<string, string> {
  const map = new Map<string, string>();
  billingTimers.forEach((session) => {
    if (session.driver_name && session.driver_id) {
      map.set(session.driver_id, session.driver_name);
    }
    // Also map pod_id -> driver_name for convenience
    if (session.driver_name && session.pod_id) {
      map.set(`pod:${session.pod_id}`, session.driver_name);
    }
  });
  latestTelemetry.forEach((tel) => {
    if (tel.driver_name && tel.pod_id) {
      if (!map.has(`pod:${tel.pod_id}`)) {
        map.set(`pod:${tel.pod_id}`, tel.driver_name);
      }
    }
  });
  return map;
}

// ─── Build rig rows for the live timing table ────────────────────────────────

function buildRigRows(
  pods: Map<string, Pod>,
  latestTelemetry: Map<string, TelemetryFrame>,
  billingTimers: Map<string, BillingSession>,
  gameStates: Map<string, GameLaunchInfo>,
  recentLaps: Lap[],
  driverNames: Map<string, string>
): RigRow[] {
  // Compute best laps per driver and overall best
  const bestPerDriver = new Map<string, number>();
  let overallBestMs = Infinity;

  for (const lap of recentLaps) {
    if (!lap.valid || lap.lap_time_ms <= 0) continue;
    const current = bestPerDriver.get(lap.driver_id);
    if (!current || lap.lap_time_ms < current) {
      bestPerDriver.set(lap.driver_id, lap.lap_time_ms);
    }
    if (lap.lap_time_ms < overallBestMs) {
      overallBestMs = lap.lap_time_ms;
    }
  }
  if (overallBestMs === Infinity) overallBestMs = 0;

  // Compute last lap per driver
  const lastLapPerDriver = new Map<string, number>();
  for (const lap of recentLaps) {
    if (!lap.valid || lap.lap_time_ms <= 0) continue;
    if (!lastLapPerDriver.has(lap.driver_id)) {
      lastLapPerDriver.set(lap.driver_id, lap.lap_time_ms);
    }
  }

  // Lap count per driver
  const lapCountPerDriver = new Map<string, number>();
  for (const lap of recentLaps) {
    if (!lap.valid || lap.lap_time_ms <= 0) continue;
    lapCountPerDriver.set(lap.driver_id, (lapCountPerDriver.get(lap.driver_id) || 0) + 1);
  }

  const rows: RigRow[] = [];

  for (const [podId, pod] of Array.from(pods.entries())) {
    const tel = latestTelemetry.get(podId);
    const billing = billingTimers.get(podId);
    const game = gameStates.get(podId);

    const driverName = billing?.driver_name || driverNames.get(`pod:${podId}`) || tel?.driver_name || "";
    const driverId = billing?.driver_id || "";
    const simType = game?.sim_type || pod.current_game || (tel?.car ? "assetto_corsa" : "");

    let status: "live" | "pit" | "idle" = "idle";
    if (pod.status === "in_session" && billing) {
      if (billing.driving_state === "active" && tel && tel.speed_kmh > 1) {
        status = "live";
      } else if (billing.driving_state === "active") {
        status = "pit";
      } else if (billing.driving_state === "idle") {
        status = "pit";
      } else {
        status = "live";
      }
    }

    const bestMs = bestPerDriver.get(driverId) || 0;
    const lastMs = lastLapPerDriver.get(driverId) || 0;
    const lapNum = tel?.lap_number || 0;
    const isOverallBest = bestMs > 0 && bestMs === overallBestMs;
    const isPersonalBest = bestMs > 0 && !isOverallBest;

    rows.push({
      podId,
      podNumber: pod.number,
      driverName,
      simType,
      lapNumber: lapNum || (lapCountPerDriver.get(driverId) || 0),
      lastLapMs: lastMs,
      bestLapMs: bestMs,
      gapMs: overallBestMs > 0 && bestMs > 0 ? bestMs - overallBestMs : 0,
      status: driverName ? status : "idle",
      isOverallBest,
      isPersonalBest,
    });
  }

  // Sort: live first, then pit, then idle; within same status, by best lap
  const statusOrder = { live: 0, pit: 1, idle: 2 };
  rows.sort((a, b) => {
    const sa = statusOrder[a.status];
    const sb = statusOrder[b.status];
    if (sa !== sb) return sa - sb;
    if (a.bestLapMs && b.bestLapMs) return a.bestLapMs - b.bestLapMs;
    if (a.bestLapMs) return -1;
    if (b.bestLapMs) return 1;
    return a.podNumber - b.podNumber;
  });

  return rows;
}

// ─── Build activity feed ──────────────────────────────────────────────────────

function buildActivityFeed(
  recentLaps: Lap[],
  driverNames: Map<string, string>,
  overallBestMs: number
): ActivityItem[] {
  const personalBests = new Map<string, number>();

  // First pass: find personal bests
  for (const lap of [...recentLaps].reverse()) {
    if (!lap.valid || lap.lap_time_ms <= 0) continue;
    const key = `${lap.driver_id}:${lap.track}`;
    const current = personalBests.get(key);
    if (!current || lap.lap_time_ms < current) {
      personalBests.set(key, lap.lap_time_ms);
    }
  }

  return recentLaps.slice(0, 15).map((lap) => {
    const name = driverNames.get(lap.driver_id) || lap.driver_id.slice(0, 12);
    const isBest = lap.valid && lap.lap_time_ms === overallBestMs && overallBestMs > 0;
    const pbKey = `${lap.driver_id}:${lap.track}`;
    const isPB = lap.valid && lap.lap_time_ms === personalBests.get(pbKey) && !isBest;

    let type: ActivityItem["type"] = "lap";
    if (isBest) type = "fastest";
    else if (isPB) type = "joined"; // green dot for PB

    return {
      id: lap.id,
      type,
      text: `${name} — ${formatLapTime(lap.lap_time_ms)}`,
      time: `L${lap.lap_number ?? "?"}`,
      lapTimeMs: lap.lap_time_ms,
    };
  });
}

// ─── Speedometer SVG ──────────────────────────────────────────────────────────

function Speedometer({ speed, maxSpeed = 350 }: { speed: number; maxSpeed?: number }) {
  const radius = 80;
  const strokeWidth = 8;
  const cx = 100;
  const cy = 100;
  // Arc from 135deg to 405deg (270 degree sweep)
  const startAngle = 135;
  const endAngle = 405;
  const sweepAngle = endAngle - startAngle;
  const fraction = Math.min(speed / maxSpeed, 1);
  const activeAngle = startAngle + sweepAngle * fraction;

  function polarToCartesian(angle: number) {
    const rad = (angle * Math.PI) / 180;
    return {
      x: cx + radius * Math.cos(rad),
      y: cy + radius * Math.sin(rad),
    };
  }

  function describeArc(start: number, end: number) {
    const s = polarToCartesian(start);
    const e = polarToCartesian(end);
    const largeArc = end - start > 180 ? 1 : 0;
    return `M ${s.x} ${s.y} A ${radius} ${radius} 0 ${largeArc} 1 ${e.x} ${e.y}`;
  }

  return (
    <svg viewBox="0 0 200 200" className="w-[180px] h-[180px]">
      {/* Background arc */}
      <path
        d={describeArc(startAngle, endAngle)}
        fill="none"
        stroke="var(--rp-border)"
        strokeWidth={strokeWidth}
        strokeLinecap="round"
      />
      {/* Active arc */}
      {fraction > 0.005 && (
        <path
          d={describeArc(startAngle, activeAngle)}
          fill="none"
          stroke="var(--rp-red)"
          strokeWidth={strokeWidth}
          strokeLinecap="round"
        />
      )}
      {/* Speed text */}
      <text
        x={cx}
        y={cy - 5}
        textAnchor="middle"
        dominantBaseline="central"
        className="fill-white font-[family-name:var(--font-mono-jb)]"
        fontSize="36"
        fontWeight="bold"
      >
        {Math.round(speed)}
      </text>
      <text
        x={cx}
        y={cy + 22}
        textAnchor="middle"
        dominantBaseline="central"
        className="fill-rp-grey"
        fontSize="11"
      >
        km/h
      </text>
    </svg>
  );
}

// ─── RPM Bar ──────────────────────────────────────────────────────────────────

function RPMBar({ rpm, maxRpm = 9000 }: { rpm: number; maxRpm?: number }) {
  const segments = 20;
  const fraction = Math.min(rpm / maxRpm, 1);
  const activeSegments = Math.round(fraction * segments);

  return (
    <div className="w-full">
      <div className="flex items-center justify-between mb-1">
        <span className="text-[10px] uppercase tracking-wider text-rp-grey">RPM</span>
        <span className="font-[family-name:var(--font-mono-jb)] text-sm text-white">{Math.round(rpm)}</span>
      </div>
      <div className="flex gap-[2px]">
        {Array.from({ length: segments }, (_, i) => {
          const segFraction = i / segments;
          let color = "var(--rp-border)"; // inactive
          if (i < activeSegments) {
            if (segFraction < 0.6) color = "var(--rp-green)";
            else if (segFraction < 0.8) color = "var(--rp-yellow)";
            else color = "var(--rp-red)";
          }
          return (
            <div
              key={i}
              className="flex-1 h-3 rounded-sm"
              style={{ backgroundColor: color }}
            />
          );
        })}
      </div>
    </div>
  );
}

// ─── Gear Indicator (boxed) ───────────────────────────────────────────────────

function GearBox({ gear }: { gear: number }) {
  const label = gear === 0 ? "N" : gear === -1 ? "R" : `${gear}`;
  return (
    <div className="flex flex-col items-center gap-1">
      <span className="text-[10px] uppercase tracking-wider text-rp-grey">Gear</span>
      <div className="w-14 h-14 rounded-lg border-2 border-rp-red flex items-center justify-center bg-rp-black">
        <span className="text-3xl font-bold font-[family-name:var(--font-mono-jb)] text-white">
          {label}
        </span>
      </div>
    </div>
  );
}

// ─── Throttle/Brake Trace ─────────────────────────────────────────────────────

function InputTrace({ history }: { history: TracePoint[] }) {
  const now = Date.now();
  const sliceWidth = TRACE_HISTORY_MS / TRACE_SLICES;
  const slices: { throttle: number; brake: number }[] = [];

  for (let i = 0; i < TRACE_SLICES; i++) {
    const sliceStart = now - TRACE_HISTORY_MS + i * sliceWidth;
    const sliceEnd = sliceStart + sliceWidth;
    // Find the closest point in this slice
    let bestPoint: TracePoint | null = null;
    let bestDist = Infinity;
    for (const p of history) {
      if (p.t >= sliceStart && p.t < sliceEnd) {
        const dist = Math.abs(p.t - (sliceStart + sliceWidth / 2));
        if (dist < bestDist) {
          bestDist = dist;
          bestPoint = p;
        }
      }
    }
    slices.push({
      throttle: bestPoint ? bestPoint.throttle : 0,
      brake: bestPoint ? bestPoint.brake : 0,
    });
  }

  // Current values
  const currentThrottle = history.length > 0 ? history[history.length - 1].throttle : 0;
  const currentBrake = history.length > 0 ? history[history.length - 1].brake : 0;

  return (
    <div className="w-full">
      <div className="flex items-center justify-between mb-2">
        <span className="text-[10px] uppercase tracking-wider text-rp-grey">Inputs (10s Trace)</span>
        <div className="flex gap-3 text-[10px]">
          <span className="text-rp-green">THR</span>
          <span className="text-rp-red">BRK</span>
        </div>
      </div>
      {/* Trace bars */}
      <div className="relative h-20 bg-rp-black rounded-lg overflow-hidden flex">
        {slices.map((s, i) => (
          <div key={i} className="flex-1 relative">
            {/* Throttle bar from bottom */}
            <div
              className="absolute bottom-0 left-0 right-[1px] bg-rp-green/60 transition-all duration-75"
              style={{ height: `${s.throttle * 100}%` }}
            />
            {/* Brake bar from bottom, overlapping */}
            <div
              className="absolute bottom-0 left-0 right-[1px] bg-rp-red/60 transition-all duration-75"
              style={{ height: `${s.brake * 100}%` }}
            />
          </div>
        ))}
      </div>
      {/* Current percentage bars */}
      <div className="mt-2 space-y-1">
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-rp-green w-6">THR</span>
          <div className="flex-1 h-2 bg-rp-black rounded-full overflow-hidden">
            <div
              className="h-full bg-rp-green rounded-full transition-all duration-100"
              style={{ width: `${currentThrottle * 100}%` }}
            />
          </div>
          <span className="text-[10px] font-[family-name:var(--font-mono-jb)] text-rp-green w-8 text-right">
            {Math.round(currentThrottle * 100)}%
          </span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-rp-red w-6">BRK</span>
          <div className="flex-1 h-2 bg-rp-black rounded-full overflow-hidden">
            <div
              className="h-full bg-rp-red rounded-full transition-all duration-100"
              style={{ width: `${currentBrake * 100}%` }}
            />
          </div>
          <span className="text-[10px] font-[family-name:var(--font-mono-jb)] text-rp-red w-8 text-right">
            {Math.round(currentBrake * 100)}%
          </span>
        </div>
      </div>
    </div>
  );
}

// ─── Status Badge ─────────────────────────────────────────────────────────────

function StatusBadge({ status }: { status: "live" | "pit" | "idle" }) {
  if (status === "live") {
    return (
      <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-rp-green/20 text-rp-green text-xs font-semibold">
        <span className="w-1.5 h-1.5 rounded-full bg-rp-green pulse-dot" />
        LIVE
      </span>
    );
  }
  if (status === "pit") {
    return (
      <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-rp-yellow/20 text-rp-yellow text-xs font-semibold">
        PIT
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-rp-card text-rp-grey text-xs font-semibold">
      IDLE
    </span>
  );
}

// ─── Game Badge ───────────────────────────────────────────────────────────────

function GameBadge({ simType }: { simType: string }) {
  const { label, color } = gameLabel(simType);
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded text-[10px] font-bold text-white ${color}`}>
      {label}
    </span>
  );
}

// ─── Activity Dot ─────────────────────────────────────────────────────────────

function ActivityDot({ type }: { type: ActivityItem["type"] }) {
  const colors: Record<string, string> = {
    fastest: "bg-rp-purple",
    joined: "bg-rp-green",
    pit: "bg-rp-yellow",
    disconnect: "bg-rp-red",
    lap: "bg-rp-grey",
  };
  return <span className={`w-2 h-2 rounded-full flex-shrink-0 ${colors[type] || "bg-rp-grey"}`} />;
}

// ─── Telemetry Sidebar ────────────────────────────────────────────────────────

function TelemetrySidebar({
  open,
  onClose,
  rig,
  telemetry,
  billing,
  gameState,
  traceHistory,
  overallBestMs,
}: {
  open: boolean;
  onClose: () => void;
  rig: RigRow | null;
  telemetry?: TelemetryFrame;
  billing?: BillingSession;
  gameState?: GameLaunchInfo;
  traceHistory: TracePoint[];
  overallBestMs: number;
}) {
  if (!rig) return null;

  const driverName = rig.driverName || "Unknown Driver";
  const car = telemetry?.car ? prettyName(telemetry.car) : "---";
  const gameName = gameState?.sim_type ? prettyName(gameState.sim_type) : rig.simType ? prettyName(rig.simType) : "---";
  const speed = telemetry?.speed_kmh ?? 0;
  const rpm = telemetry?.rpm ?? 0;
  const gear = telemetry?.gear ?? 0;

  const currentLapMs = telemetry?.lap_time_ms ?? 0;
  const lastLapMs = rig.lastLapMs;
  const bestLapMs = rig.bestLapMs;
  const bestIsOverall = rig.isOverallBest;

  return (
    <div
      className="fixed top-0 right-0 h-full z-50 transition-transform duration-300 ease-in-out"
      style={{
        width: `${SIDEBAR_WIDTH}px`,
        transform: open ? "translateX(0)" : `translateX(${SIDEBAR_WIDTH}px)`,
      }}
    >
      {/* Backdrop */}
      {open && (
        <div
          className="fixed inset-0 bg-black/40 -z-10"
          onClick={onClose}
        />
      )}

      <div className="h-full bg-rp-black border-l border-rp-border flex flex-col overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-rp-border">
          <div className="flex items-center gap-2">
            <span className="w-2.5 h-2.5 rounded-full bg-rp-red pulse-dot" />
            <span className="text-sm font-bold uppercase tracking-wider font-[family-name:var(--font-display)]">
              Rig {rig.podNumber} — Telemetry
            </span>
          </div>
          <button
            onClick={onClose}
            className="w-8 h-8 rounded-lg flex items-center justify-center text-rp-grey hover:text-white hover:bg-rp-surface transition-colors"
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <path d="M4 4L12 12M12 4L4 12" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            </svg>
          </button>
        </div>

        {/* Driver Info */}
        <div className="px-5 py-4 border-b border-rp-border">
          <div className="flex items-center gap-3">
            <div className="w-12 h-12 rounded-full border-2 border-rp-red bg-rp-surface flex items-center justify-center text-lg font-bold text-white">
              {driverName.charAt(0).toUpperCase()}
            </div>
            <div>
              <p className="text-base font-bold text-white font-[family-name:var(--font-display)]">
                {driverName}
              </p>
              <p className="text-xs text-rp-grey">
                {gameName} &middot; {car}
              </p>
            </div>
          </div>
        </div>

        {/* Speedometer */}
        <div className="px-5 py-3 flex justify-center border-b border-rp-border">
          <Speedometer speed={speed} />
        </div>

        {/* RPM + Gear */}
        <div className="px-5 py-3 border-b border-rp-border">
          <div className="flex items-end gap-4">
            <div className="flex-1">
              <RPMBar rpm={rpm} />
            </div>
            <GearBox gear={gear} />
          </div>
        </div>

        {/* Inputs Trace */}
        <div className="px-5 py-3 border-b border-rp-border">
          <InputTrace history={traceHistory} />
        </div>

        {/* Session Timing */}
        <div className="px-5 py-4">
          <span className="text-[10px] uppercase tracking-wider text-rp-grey block mb-3">Session Timing</span>
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs text-rp-grey">Current Lap</span>
              <span className="font-[family-name:var(--font-mono-jb)] text-sm text-white">
                {currentLapMs > 0 ? formatLapTime(currentLapMs) : "--:--.---"}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-rp-grey">Last Lap</span>
              <span className="font-[family-name:var(--font-mono-jb)] text-sm text-white">
                {lastLapMs > 0 ? formatLapTime(lastLapMs) : "--:--.---"}
              </span>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-rp-grey">Best Lap</span>
              <span className={`font-[family-name:var(--font-mono-jb)] text-sm font-bold ${
                bestIsOverall ? "text-rp-purple" : bestLapMs > 0 ? "text-rp-green" : "text-white"
              }`}>
                {bestLapMs > 0 ? formatLapTime(bestLapMs) : "--:--.---"}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ─── Main Spectator Component ─────────────────────────────────────────────────

export default function SpectatorMode() {
  const {
    connected,
    pods,
    latestTelemetry,
    recentLaps,
    billingTimers,
    gameStates,
    cameraFocus,
    sendCommand,
  } = useKioskSocket();

  // State
  const [sessionSeconds, setSessionSeconds] = useState(0);
  const [activeTab, setActiveTab] = useState<"live_timing" | "track_map" | "rigs" | "settings" | "leaderboard">("live_timing");
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [cameraEnabled, setCameraEnabled] = useState(false);
  const [cameraMode, setCameraMode] = useState("closest_cycle");

  // Trace history ref
  const traceHistoryRef = useRef<Map<string, TracePoint[]>>(new Map());

  // Session timer
  useEffect(() => {
    const interval = setInterval(() => {
      setSessionSeconds((s) => s + 1);
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  // Update trace history on telemetry changes
  useEffect(() => {
    const now = Date.now();
    const cutoff = now - TRACE_HISTORY_MS;

    latestTelemetry.forEach((tel, podId) => {
      const existing = traceHistoryRef.current.get(podId) || [];
      existing.push({ t: now, throttle: tel.throttle, brake: tel.brake });
      // Trim old entries
      const trimmed = existing.filter((p) => p.t >= cutoff);
      traceHistoryRef.current.set(podId, trimmed);
    });
  }, [latestTelemetry]);

  // Camera controls
  const toggleCamera = useCallback(() => {
    const newEnabled = !cameraEnabled;
    setCameraEnabled(newEnabled);
    sendCommand("set_camera_mode", { mode: cameraMode, enabled: newEnabled });
  }, [cameraEnabled, cameraMode, sendCommand]);

  const changeCameraMode = useCallback(
    (mode: string) => {
      setCameraMode(mode);
      if (cameraEnabled) {
        sendCommand("set_camera_mode", { mode, enabled: true });
      }
    },
    [cameraEnabled, sendCommand]
  );

  // Open sidebar for a rig
  const openRigTelemetry = useCallback((podId: string) => {
    setSelectedPodId(podId);
    setSidebarOpen(true);
  }, []);

  const closeSidebar = useCallback(() => {
    setSidebarOpen(false);
  }, []);

  // Derived data
  const driverNames = useMemo(
    () => buildDriverNameMap(billingTimers, latestTelemetry),
    [billingTimers, latestTelemetry]
  );

  const rigRows = useMemo(
    () => buildRigRows(pods, latestTelemetry, billingTimers, gameStates, recentLaps, driverNames),
    [pods, latestTelemetry, billingTimers, gameStates, recentLaps, driverNames]
  );

  const overallBestMs = useMemo(() => {
    let best = Infinity;
    for (const lap of recentLaps) {
      if (lap.valid && lap.lap_time_ms > 0 && lap.lap_time_ms < best) {
        best = lap.lap_time_ms;
      }
    }
    return best === Infinity ? 0 : best;
  }, [recentLaps]);

  const fastestLapInfo = useMemo(() => {
    let fastest: Lap | null = null;
    for (const lap of recentLaps) {
      if (!lap.valid || lap.lap_time_ms <= 0) continue;
      if (!fastest || lap.lap_time_ms < fastest.lap_time_ms) {
        fastest = lap;
      }
    }
    if (!fastest) return null;
    return {
      driverName: driverNames.get(fastest.driver_id) || fastest.driver_id.slice(0, 12),
      lapTimeMs: fastest.lap_time_ms,
      track: fastest.track,
      car: fastest.car,
      simType: "",
    };
  }, [recentLaps, driverNames]);

  const activePodCount = useMemo(
    () => Array.from(pods.values()).filter((p) => p.status === "in_session").length,
    [pods]
  );

  const totalLaps = useMemo(
    () => recentLaps.filter((l) => l.valid).length,
    [recentLaps]
  );

  const activityFeed = useMemo(
    () => buildActivityFeed(recentLaps, driverNames, overallBestMs),
    [recentLaps, driverNames, overallBestMs]
  );

  // Get trace history for selected pod
  const selectedTraceHistory = selectedPodId ? (traceHistoryRef.current.get(selectedPodId) || []) : [];

  // Selected rig row
  const selectedRig = rigRows.find((r) => r.podId === selectedPodId) || null;

  const tabs = [
    { key: "live_timing" as const, label: "Live Timing" },
    { key: "leaderboard" as const, label: "Leaderboard" },
    { key: "track_map" as const, label: "Track Map" },
    { key: "rigs" as const, label: "Rigs" },
    { key: "settings" as const, label: "Settings" },
  ];

  const cameraModes = [
    { key: "closest_cycle", label: "Closest Cycle" },
    { key: "leader", label: "Leader" },
    { key: "closest", label: "Closest" },
    { key: "cycle", label: "Cycle" },
  ];

  return (
    <div className="h-screen flex flex-col bg-rp-black text-white overflow-hidden font-[family-name:var(--font-display)]">
      {/* ── Header ────────────────────────────────────────────────────── */}
      <header className="flex items-center justify-between px-6 py-3 border-b border-rp-border bg-rp-black flex-shrink-0">
        {/* Left: Title + Timer */}
        <div className="flex items-center gap-5">
          <h1 className="flex items-center gap-3 text-lg font-bold tracking-[0.15em] uppercase">
            <BrandLogo size="sm" alt="Racing Point" />
            <span>Race Control</span>
          </h1>
          <span className="font-[family-name:var(--font-mono-jb)] text-lg font-bold text-rp-red tabular-nums">
            {formatSessionTimer(sessionSeconds)}
          </span>
        </div>

        {/* Center: Nav Tabs */}
        <nav className="flex items-center gap-1">
          {tabs.map((tab) => (
            <button
              key={tab.key}
              onClick={() => setActiveTab(tab.key)}
              className={`px-4 py-1.5 rounded-lg text-xs font-semibold uppercase tracking-wider transition-colors ${
                activeTab === tab.key
                  ? "bg-rp-red text-white"
                  : "text-rp-grey hover:text-white hover:bg-rp-surface/50"
              }`}
            >
              {tab.label}
            </button>
          ))}
        </nav>

        {/* Right: Camera + End Session */}
        <div className="flex items-center gap-3">
          {/* Camera toggle */}
          <button
            onClick={toggleCamera}
            className={`px-3 py-1.5 text-xs font-bold uppercase tracking-wider rounded-lg transition-colors ${
              cameraEnabled
                ? "bg-rp-red text-white"
                : "bg-rp-surface text-rp-grey hover:text-white"
            }`}
          >
            CAM {cameraEnabled ? "ON" : "OFF"}
          </button>
          {/* Camera mode selector */}
          {cameraEnabled && (
            <select
              value={cameraMode}
              onChange={(e) => changeCameraMode(e.target.value)}
              className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider bg-rp-surface text-rp-grey hover:text-white rounded-lg border border-rp-border outline-none cursor-pointer"
            >
              {cameraModes.map((m) => (
                <option key={m.key} value={m.key}>
                  {m.label}
                </option>
              ))}
            </select>
          )}

          {/* End Session */}
          <button className="px-4 py-1.5 text-xs font-bold uppercase tracking-wider bg-rp-surface text-rp-red border border-rp-red/30 hover:bg-rp-red hover:text-white rounded-lg transition-colors">
            End Session
          </button>
        </div>
      </header>

      {/* ── Main Content ──────────────────────────────────────────────── */}
      {activeTab === "leaderboard" ? (
        <div className="flex-1 overflow-hidden">
          <KioskLeaderboard />
        </div>
      ) : (
      <div className="flex-1 flex overflow-hidden">
        {/* Left Section: Live Timing (65%) */}
        <div className="flex-[65] flex flex-col overflow-hidden">
          <div className="flex-1 p-5 overflow-auto">
            {/* Live Timing Table */}
            <div className="rounded-xl border border-rp-border overflow-hidden">
              {/* Table Header */}
              <div className="grid grid-cols-[50px_1fr_80px_60px_120px_120px_100px_90px] gap-2 px-4 py-3 bg-rp-red/10 border-b border-rp-border">
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey text-center">Pos</span>
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey">Driver / Rig</span>
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey text-center">Game</span>
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey text-center">Lap</span>
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey text-right">Last Lap</span>
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey text-right">Best Lap</span>
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey text-right">Gap</span>
                <span className="text-[10px] font-bold uppercase tracking-wider text-rp-grey text-center">Status</span>
              </div>

              {/* Table Rows */}
              {rigRows.length === 0 ? (
                <div className="px-4 py-8 text-center text-rp-grey text-sm">
                  Waiting for pods to connect...
                </div>
              ) : (
                rigRows.map((rig, i) => {
                  const isIdle = rig.status === "idle";
                  return (
                    <button
                      key={rig.podId}
                      onClick={() => openRigTelemetry(rig.podId)}
                      className={`w-full grid grid-cols-[50px_1fr_80px_60px_120px_120px_100px_90px] gap-2 px-4 py-2.5 border-b border-rp-border/50 text-left transition-colors hover:bg-white/5 cursor-pointer ${
                        isIdle ? "opacity-40" : ""
                      } ${selectedPodId === rig.podId && sidebarOpen ? "bg-white/5" : ""}`}
                    >
                      {/* Position */}
                      <span className={`text-center font-[family-name:var(--font-mono-jb)] text-sm font-bold ${
                        !isIdle && i === 0 ? "text-rp-red" : "text-rp-grey"
                      }`}>
                        {isIdle ? "-" : i + 1}
                      </span>

                      {/* Driver / Rig */}
                      <div className="min-w-0">
                        <p className="text-sm font-semibold text-white truncate">
                          {rig.driverName || `Rig ${rig.podNumber}`}
                        </p>
                        <p className="text-[10px] text-rp-grey">
                          Rig {rig.podNumber}
                        </p>
                      </div>

                      {/* Game */}
                      <div className="flex items-center justify-center">
                        {rig.simType ? <GameBadge simType={rig.simType} /> : <span className="text-rp-grey">---</span>}
                      </div>

                      {/* Lap */}
                      <span className="text-center font-[family-name:var(--font-mono-jb)] text-sm text-white">
                        {rig.lapNumber > 0 ? rig.lapNumber : "-"}
                      </span>

                      {/* Last Lap */}
                      <span className="text-right font-[family-name:var(--font-mono-jb)] text-sm text-white">
                        {rig.lastLapMs > 0 ? formatLapTime(rig.lastLapMs) : "--:--.---"}
                      </span>

                      {/* Best Lap */}
                      <span className={`text-right font-[family-name:var(--font-mono-jb)] text-sm font-bold ${
                        rig.isOverallBest
                          ? "text-rp-purple"
                          : rig.isPersonalBest
                          ? "text-rp-green"
                          : "text-white"
                      }`}>
                        {rig.bestLapMs > 0 ? formatLapTime(rig.bestLapMs) : "--:--.---"}
                      </span>

                      {/* Gap */}
                      <span className="text-right font-[family-name:var(--font-mono-jb)] text-xs text-rp-grey">
                        {rig.gapMs > 0 ? formatGap(rig.gapMs) : (rig.bestLapMs > 0 && i === 0 ? "LEADER" : "")}
                      </span>

                      {/* Status */}
                      <div className="flex items-center justify-center">
                        <StatusBadge status={rig.status} />
                      </div>
                    </button>
                  );
                })
              )}
            </div>
          </div>
        </div>

        {/* Right Section: Stats Sidebar (35%) */}
        <div className="flex-[35] border-l border-rp-border flex flex-col overflow-hidden bg-rp-black/50">
          <div className="flex-1 p-5 overflow-auto space-y-4">
            {/* Active Rigs Card */}
            <div className="rounded-xl border border-rp-border bg-rp-surface p-4">
              <p className="text-[10px] font-bold uppercase tracking-wider text-rp-grey mb-1">Active Rigs</p>
              <div className="flex items-baseline gap-1">
                <span className="text-4xl font-bold font-[family-name:var(--font-mono-jb)] text-rp-red">
                  {activePodCount}
                </span>
                <span className="text-lg text-rp-grey font-[family-name:var(--font-mono-jb)]">
                  / {pods.size}
                </span>
              </div>
            </div>

            {/* Total Laps Card */}
            <div className="rounded-xl border border-rp-border bg-rp-surface p-4">
              <p className="text-[10px] font-bold uppercase tracking-wider text-rp-grey mb-1">Total Laps</p>
              <span className="text-4xl font-bold font-[family-name:var(--font-mono-jb)] text-white">
                {totalLaps}
              </span>
            </div>

            {/* Fastest Lap Overall Card */}
            <div className="rounded-xl border border-rp-purple/30 bg-rp-purple/10 p-4">
              <p className="text-[10px] font-bold uppercase tracking-wider text-rp-purple mb-2">Fastest Lap Overall</p>
              {fastestLapInfo ? (
                <>
                  <span className="text-3xl font-bold font-[family-name:var(--font-mono-jb)] text-white block mb-1">
                    {formatLapTime(fastestLapInfo.lapTimeMs)}
                  </span>
                  <p className="text-xs text-rp-grey">
                    {fastestLapInfo.driverName} &middot; {prettyName(fastestLapInfo.car)}
                  </p>
                </>
              ) : (
                <span className="text-lg text-rp-grey font-[family-name:var(--font-mono-jb)]">--:--.---</span>
              )}
            </div>

            {/* Recent Activity Feed */}
            <div className="rounded-xl border border-rp-border bg-rp-surface p-4">
              <p className="text-[10px] font-bold uppercase tracking-wider text-rp-grey mb-3">Recent Activity</p>
              {activityFeed.length === 0 ? (
                <p className="text-xs text-rp-grey">No activity yet...</p>
              ) : (
                <div className="space-y-2">
                  {activityFeed.slice(0, 10).map((item) => (
                    <div key={item.id} className="flex items-start gap-2.5">
                      {/* Timeline dot */}
                      <div className="flex flex-col items-center mt-1.5">
                        <ActivityDot type={item.type} />
                        <div className="w-px h-3 bg-rp-border mt-1" />
                      </div>
                      {/* Content */}
                      <div className="flex-1 min-w-0">
                        <p className="text-xs text-white truncate">{item.text}</p>
                        <p className="text-[10px] text-rp-grey">{item.time}</p>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        </div>
      </div>
      )}

      {/* ── Telemetry Sidebar ─────────────────────────────────────────── */}
      <TelemetrySidebar
        open={sidebarOpen}
        onClose={closeSidebar}
        rig={selectedRig}
        telemetry={selectedPodId ? latestTelemetry.get(selectedPodId) : undefined}
        billing={selectedPodId ? billingTimers.get(selectedPodId) : undefined}
        gameState={selectedPodId ? gameStates.get(selectedPodId) : undefined}
        traceHistory={selectedTraceHistory}
        overallBestMs={overallBestMs}
      />

      {/* ── Camera Focus Bar ──────────────────────────────────────────── */}
      {cameraEnabled && cameraFocus && cameraFocus.pod_id && (
        <div className="flex items-center justify-center gap-3 px-8 py-1.5 bg-rp-red/10 border-t border-rp-red/30 flex-shrink-0">
          <div className="w-2 h-2 bg-rp-red rounded-full pulse-dot" />
          <span className="text-xs font-semibold uppercase tracking-wider text-rp-red">
            Camera: {cameraFocus.driver_name}
          </span>
          <span className="text-[10px] text-rp-grey uppercase">
            {cameraFocus.reason.replace("_", " ")}
          </span>
        </div>
      )}

      {/* ── Connection Lost Overlay ────────────────────────────────────── */}
      {!connected && (
        <div className="absolute bottom-0 left-0 right-0 bg-rp-red/90 text-white text-center py-2 text-sm font-semibold tracking-wider uppercase">
          Reconnecting to RaceControl...
        </div>
      )}
    </div>
  );
}
