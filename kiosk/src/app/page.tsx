"use client";

/* eslint-disable @next/next/no-img-element */
import { useState, useEffect, useMemo } from "react";
import Link from "next/link";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { GAME_DISPLAY } from "@/lib/gameDisplayInfo";
import type { Pod, TelemetryFrame, BillingSession, GameLaunchInfo, Lap } from "@/lib/types";

// ─── Helpers ─────────────────────────────────────────────────────────────

function formatLapTime(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const totalSec = ms / 1000;
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toFixed(3).padStart(6, "0")}`;
}

function formatTimer(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function padPod(n: number): string {
  return String(n).padStart(2, "0");
}

function formatTrackName(track: string): string {
  // Convert snake_case/kebab-case track IDs to readable names
  return track
    .replace(/[-_]/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

// ─── RPM Arc Gauge ──────────────────────────────────────────────────────
function RpmGauge({ rpm, maxRpm, gear, speed }: { rpm: number; maxRpm: number; gear: number; speed: number }) {
  const size = 170;
  const cx = size / 2;
  const cy = size / 2 + 6;
  const r = 68;
  const startAngle = -225;
  const endAngle = 45;
  const totalAngle = endAngle - startAngle;

  const pct = Math.min(maxRpm > 0 ? rpm / maxRpm : 0, 1);
  const activeAngle = pct * totalAngle;

  const toRad = (deg: number) => (deg * Math.PI) / 180;
  const arcStart = startAngle;
  const arcEnd = startAngle + activeAngle;

  const x1 = cx + r * Math.cos(toRad(arcStart));
  const y1 = cy + r * Math.sin(toRad(arcStart));
  const x2 = cx + r * Math.cos(toRad(arcEnd));
  const y2 = cy + r * Math.sin(toRad(arcEnd));

  const bgX2 = cx + r * Math.cos(toRad(endAngle));
  const bgY2 = cy + r * Math.sin(toRad(endAngle));

  const largeArc = activeAngle > 180 ? 1 : 0;
  const bgLargeArc = totalAngle > 180 ? 1 : 0;

  const rpmColor = pct > 0.9 ? "#E10600" : pct > 0.7 ? "#F59E0B" : "#22C55E";
  const gearLabel = gear === -1 ? "R" : gear === 0 ? "N" : String(gear);
  const rpmDisplay = Math.round(rpm).toLocaleString();

  return (
    <svg width={size} height={size - 16} viewBox={`0 0 ${size} ${size - 16}`}>
      <path d={`M ${x1} ${y1} A ${r} ${r} 0 ${bgLargeArc} 1 ${bgX2} ${bgY2}`}
        fill="none" stroke="#2A2A2A" strokeWidth={8} strokeLinecap="round" />
      {pct > 0.01 && (
        <path d={`M ${x1} ${y1} A ${r} ${r} 0 ${largeArc} 1 ${x2} ${y2}`}
          fill="none" stroke={rpmColor} strokeWidth={8} strokeLinecap="round"
          style={{ filter: `drop-shadow(0 0 8px ${rpmColor}90)` }} />
      )}
      <text x={cx} y={cy - 12} textAnchor="middle" dominantBaseline="central"
        className="font-display" fill="white" fontSize="42" fontWeight="bold">{gearLabel}</text>
      <text x={cx} y={cy + 20} textAnchor="middle" dominantBaseline="central"
        className="font-mono" fill="white" fontSize="20" fontWeight="700">{Math.round(speed)}</text>
      <text x={cx} y={cy + 35} textAnchor="middle" dominantBaseline="central"
        className="font-mono" fill="#555" fontSize="9">km/h</text>
      <text x={cx} y={cy + 52} textAnchor="middle" dominantBaseline="central"
        className="font-mono" fill={rpmColor} fontSize="10" fontWeight="600">{rpmDisplay} RPM</text>
    </svg>
  );
}

// ─── Lap History ────────────────────────────────────────────────────────
function LapHistory({ laps, bestLapMs }: { laps: Lap[]; bestLapMs: number }) {
  const recent = laps
    .filter((l) => l.lap_time_ms > 0)
    .slice(0, 4);

  if (recent.length === 0) return null;

  return (
    <div className="flex flex-col gap-0.5 max-h-[60px] overflow-hidden">
      {recent.map((lap, i) => {
        const isBest = lap.valid && lap.lap_time_ms === bestLapMs;
        return (
          <div key={lap.id || i} className="flex items-center justify-between text-xs font-mono">
            <span className="text-[#555] w-6">L{recent.length - i}</span>
            <span className={
              !lap.valid
                ? "text-red-500/70 line-through"
                : isBest
                  ? "text-purple-400 font-semibold"
                  : "text-emerald-400"
            }>
              {formatLapTime(lap.lap_time_ms)}
            </span>
            {!lap.valid && <span className="text-red-500/50 text-[9px] ml-1">INV</span>}
            {isBest && <span className="text-purple-400 text-[9px] ml-1">PB</span>}
            {lap.valid && !isBest && <span className="text-transparent text-[9px] ml-1">--</span>}
          </div>
        );
      })}
    </div>
  );
}

// Game color map for badge dots
const GAME_COLORS: Record<string, string> = {
  assetto_corsa: "#E10600",
  assetto_corsa_evo: "#FF3300",
  f1_25: "#E10600",
  iracing: "#0056B3",
  le_mans_ultimate: "#003366",
  forza: "#107C10",
  forza_horizon_5: "#E9A200",
  ea_wrc: "#0066CC",
};

// ─── Customer Landing Page ───────────────────────────────────────────────

export default function CustomerLanding() {
  const {
    connected,
    pods,
    latestTelemetry,
    recentLaps,
    billingTimers,
    gameStates,
  } = useKioskSocket();

  // ─── Clock ────────────────────────────────────────────────────────────
  const [clock, setClock] = useState("");

  useEffect(() => {
    const tick = () => {
      const now = new Date();
      setClock(
        now.toLocaleTimeString("en-IN", {
          timeZone: "Asia/Kolkata",
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
          hour12: false,
        })
      );
    };
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, []);

  // ─── Pod sorting ──────────────────────────────────────────────────────

  const sortedPods = Array.from(pods.values()).sort((a, b) => a.number - b.number);
  const podSlots: (Pod | null)[] = [];
  for (let i = 1; i <= 8; i++) {
    podSlots.push(sortedPods.find((p) => p.number === i) || null);
  }

  const idleCount = sortedPods.filter((p) => p.status === "idle").length;
  const activeCount = sortedPods.filter((p) => p.status === "in_session").length;
  const offlineCount = sortedPods.filter(
    (p) => p.status === "offline" || p.status === "disabled"
  ).length;

  // ─── Ticker data: recent laps with driver names ──────────────────────

  const tickerItems = useMemo(() => {
    // Build a driver_id -> driver_name map from billing sessions
    const driverNames = new Map<string, string>();
    for (const [, billing] of billingTimers) {
      driverNames.set(billing.driver_id, billing.driver_name);
    }
    // Also pull from telemetry frames (has driver_name)
    for (const [, telem] of latestTelemetry) {
      if (telem.driver_name) {
        // telemetry doesn't have driver_id directly, but pod_id maps
        // We'll use billing as primary source
      }
    }

    // Build pod_id -> pod_number map
    const podNumbers = new Map<string, number>();
    for (const [id, pod] of pods) {
      podNumbers.set(id, pod.number);
    }

    // Build driver_id -> pod_id from billing
    const driverPods = new Map<string, string>();
    for (const [podId, billing] of billingTimers) {
      driverPods.set(billing.driver_id, podId);
    }

    return recentLaps
      .filter((l) => l.valid && l.lap_time_ms > 0)
      .slice(0, 20)
      .map((lap) => {
        const podId = driverPods.get(lap.driver_id) || "";
        const podNum = podNumbers.get(podId) || 0;
        const driverName = driverNames.get(lap.driver_id) || "Driver";
        const track = lap.track ? formatTrackName(lap.track) : "Unknown";
        return {
          podNum,
          track,
          time: formatLapTime(lap.lap_time_ms),
          driver: driverName,
        };
      });
  }, [recentLaps, billingTimers, pods, latestTelemetry]);

  // ─── Render ───────────────────────────────────────────────────────────

  return (
    <div className="h-screen flex flex-col bg-[#0A0A0A] overflow-hidden">
      {/* ── Header ── */}
      <header className="flex items-center justify-between px-6 py-3 bg-[#141414] border-b border-rp-red">
        {/* Left: Brand */}
        <h1 className="text-xl tracking-widest uppercase text-white font-display">
          RACING <span className="text-rp-red">POINT</span>
        </h1>

        {/* Center: Status pills */}
        <div className="flex items-center gap-3">
          {idleCount > 0 && (
            <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-emerald-500/15 text-emerald-500 text-xs font-semibold font-mono tracking-wide">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 motion-safe:animate-pulse" />
              {idleCount} AVAILABLE
            </span>
          )}
          {activeCount > 0 && (
            <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-rp-red/15 text-rp-red text-xs font-semibold font-mono tracking-wide">
              <span className="w-1.5 h-1.5 rounded-full bg-rp-red" />
              {activeCount} RACING
            </span>
          )}
          {offlineCount > 0 && (
            <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-zinc-700/40 text-zinc-500 text-xs font-semibold font-mono tracking-wide">
              <span className="w-1.5 h-1.5 rounded-full bg-zinc-500" />
              {offlineCount} OFFLINE
            </span>
          )}
        </div>

        {/* Right: Clock + WS status */}
        <div className="flex items-center gap-4">
          <div data-testid="ws-status" className="flex items-center gap-2">
            <span
              className={`w-2 h-2 rounded-full ${
                connected ? "bg-emerald-500 pulse-dot" : "bg-red-500"
              }`}
            />
            <span className="text-xs text-[#666]">
              {connected ? "LIVE" : "CONNECTING"}
            </span>
          </div>
          <span className="text-lg text-white font-mono tabular-nums tracking-wider">
            {clock}
          </span>
        </div>
      </header>

      {/* ── Pod Grid 4x2 ── */}
      <main data-testid="pod-grid" className="flex-1 p-4 overflow-hidden">
        <div className="grid grid-cols-4 grid-rows-2 gap-3 h-full">
          {podSlots.map((pod, idx) => {
            const podNum = idx + 1;

            if (!pod) {
              return (
                <div
                  key={`empty-${podNum}`}
                  className={`rounded-lg bg-[#0F0F0F] border border-[#2A2A2A] flex flex-col items-center justify-center ${
                    connected ? "opacity-30" : "animate-pulse opacity-20"
                  }`}
                >
                  <span className="text-3xl font-bold text-[#333] font-display">
                    {padPod(podNum)}
                  </span>
                  <span className="text-xs text-[#333] mt-1 font-mono uppercase tracking-wider">
                    {connected ? "Offline" : ""}
                  </span>
                </div>
              );
            }

            const billing = billingTimers.get(pod.id);
            const telemetry = latestTelemetry.get(pod.id);
            const gameInfo = gameStates.get(pod.id);
            const isActive = pod.status === "in_session" && billing;
            const isOffline =
              pod.status === "offline" || pod.status === "disabled";

            // ── Active pod ──
            if (isActive && billing) {
              const podLaps = recentLaps.filter(
                (l) => l.driver_id === billing.driver_id
              );
              return (
                <ActivePodCard
                  key={pod.id}
                  pod={pod}
                  billing={billing}
                  telemetry={telemetry}
                  gameInfo={gameInfo}
                  podLaps={podLaps}
                />
              );
            }

            // ── Offline/disabled ──
            if (isOffline) {
              return (
                <div
                  key={pod.id}
                  className="rounded-lg bg-[#0F0F0F] border border-[#2A2A2A] flex flex-col items-center justify-center opacity-30"
                >
                  <span className="text-3xl font-bold text-[#333] font-display">
                    {padPod(pod.number)}
                  </span>
                  <span className="text-xs text-[#333] mt-1 font-mono uppercase tracking-wider">
                    {pod.status === "disabled" ? "Maintenance" : "Offline"}
                  </span>
                </div>
              );
            }

            // ── Idle / Available ──
            return (
              <div
                key={pod.id}
                data-testid={`pod-card-${pod.number}`}
                className="rounded-lg bg-[#141414] border-l-[3px] border-l-emerald-500 border border-[#2A2A2A] flex flex-col items-center justify-center gap-2 motion-safe:glow-available"
              >
                <span className="text-4xl font-bold text-white font-display">
                  {padPod(pod.number)}
                </span>
                <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-emerald-500/15 text-emerald-500 text-xs font-semibold font-mono uppercase tracking-wider">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                  Available
                </span>
                <span className="text-2xl font-bold text-white/60 font-display motion-safe:breathe">
                  READY
                </span>
              </div>
            );
          })}
        </div>
      </main>

      {/* ── Bottom Ticker ── */}
      <div className="h-10 bg-[#111] border-t border-[#2A2A2A] flex items-center overflow-hidden relative">
        {tickerItems.length > 0 ? (
          <div className="ticker-scroll flex items-center gap-0 whitespace-nowrap">
            {/* Duplicate the content for seamless scroll loop */}
            {[0, 1].map((copy) => (
              <div key={copy} className="flex items-center gap-0">
                {tickerItems.map((item, i) => (
                  <span
                    key={`${copy}-${i}`}
                    className="flex items-center gap-3 px-6 text-xs font-mono"
                  >
                    <span className="text-white font-semibold">
                      POD {padPod(item.podNum)}
                    </span>
                    <span className="text-rp-red">|</span>
                    <span className="text-[#888]">{item.track}</span>
                    <span className="text-rp-red">|</span>
                    <span className="text-white font-semibold tabular-nums">
                      {item.time}
                    </span>
                    <span className="text-rp-red">|</span>
                    <span className="text-[#888]">{item.driver}</span>
                  </span>
                ))}
              </div>
            ))}
          </div>
        ) : (
          <div className="flex items-center justify-center w-full">
            <span className="text-xs text-[#444] font-mono uppercase tracking-widest">
              Waiting for lap data
            </span>
          </div>
        )}
      </div>

      {/* ── Staff Login (subtle) ── */}
      <footer className="flex items-center justify-center py-2 bg-[#0A0A0A]">
        <Link
          href="/staff"
          className="px-6 py-1.5 text-xs font-medium border border-[#2A2A2A] rounded-lg text-[#444] hover:text-white hover:border-rp-red transition-colors cursor-pointer"
        >
          Staff Login
        </Link>
      </footer>
    </div>
  );
}

// ─── Active Pod Card ────────────────────────────────────────────────────

function ActivePodCard({
  pod,
  billing,
  telemetry,
  gameInfo,
  podLaps,
}: {
  pod: Pod;
  billing: BillingSession;
  telemetry?: TelemetryFrame;
  gameInfo?: GameLaunchInfo;
  podLaps: Lap[];
}) {
  const remaining = billing.remaining_seconds ?? 0;
  const speed = telemetry?.speed_kmh ?? 0;
  const rpm = telemetry?.rpm ?? 0;
  const maxRpm = 9000; // AC default; real max_rpm not in TelemetryFrame yet
  const gear = telemetry?.gear ?? 0;
  const lapCount = telemetry?.lap_number ?? 0;
  const simType = gameInfo?.sim_type || "";
  const track = telemetry?.track || "";
  const car = telemetry?.car || "";

  const gameEntry = simType ? GAME_DISPLAY[simType] : undefined;
  const gameColor = GAME_COLORS[simType] || "#666";

  const validLaps = podLaps.filter((l) => l.valid && l.lap_time_ms > 0);
  const bestLap = validLaps.length > 0 ? Math.min(...validLaps.map((l) => l.lap_time_ms)) : 0;

  const timerCritical = remaining < 300;

  return (
    <div className="rounded-lg bg-[#141414] border-l-[3px] border-l-rp-red border border-[#2A2A2A] flex flex-col overflow-hidden motion-safe:glow-active">
      {/* Header */}
      <div className="px-3 pt-2.5 pb-2 border-b border-[#2A2A2A]">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-sm font-bold text-rp-red font-display tracking-wider">POD {padPod(pod.number)}</span>
          {gameEntry && (
            <span className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-[#1E1E1E] border border-[#333]">
              <span className="w-2.5 h-2.5 rounded-sm flex-shrink-0" style={{ backgroundColor: gameColor }} />
              <span className="text-[11px] text-[#aaa] font-semibold tracking-wide">{gameEntry.abbr}</span>
            </span>
          )}
        </div>
        <p className="text-base font-semibold text-white truncate leading-tight">{billing.driver_name}</p>
        {(car || track) && (
          <p className="text-[10px] text-[#555] font-mono truncate mt-0.5">
            {car && formatTrackName(car)}{car && track ? "  ·  " : ""}{track && formatTrackName(track)}
          </p>
        )}
      </div>

      {/* Telemetry: gauge centered + data below */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <div className="flex items-center justify-between px-3 py-1">
          <span className={`text-base font-mono tabular-nums font-bold ${timerCritical ? "text-rp-red motion-safe:animate-pulse" : "text-white"}`}>
            {formatTimer(remaining)}
          </span>
          <span className="text-sm font-mono text-[#888]">LAP {lapCount}</span>
        </div>

        <div className="flex-1 flex items-center justify-center -mt-1 -mb-2">
          <RpmGauge rpm={rpm} maxRpm={maxRpm} gear={gear} speed={speed} />
        </div>

        <div className="px-3 pb-2">
          <LapHistory laps={podLaps} bestLapMs={bestLap} />
        </div>
      </div>
    </div>
  );
}
