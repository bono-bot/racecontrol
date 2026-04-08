"use client";

import { useState, useEffect, useMemo } from "react";
import Image from "next/image";
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
  const lapCount = telemetry?.lap_number ?? 0;
  const simType = gameInfo?.sim_type || "";

  // Game logo from gameDisplayInfo
  const gameEntry = simType ? GAME_DISPLAY[simType] : undefined;
  const logoSrc = gameEntry?.logo;

  // Best/last lap
  const validLaps = podLaps.filter((l) => l.valid && l.lap_time_ms > 0);
  const bestLap =
    validLaps.length > 0
      ? Math.min(...validLaps.map((l) => l.lap_time_ms))
      : 0;
  const lastLap =
    validLaps.length > 0 ? validLaps[0]?.lap_time_ms ?? 0 : 0;

  const timerCritical = remaining < 300;

  return (
    <div className="rounded-lg bg-[#141414] border-l-[3px] border-l-rp-red border border-[#2A2A2A] flex flex-col overflow-hidden motion-safe:glow-active">
      {/* Top bar: Pod number + Game logo */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-[#2A2A2A]">
        <div className="flex items-center gap-2">
          <span className="text-xl font-bold text-white font-display leading-none">
            {padPod(pod.number)}
          </span>
          <span className="text-sm text-[#888] truncate max-w-[120px]">
            {billing.driver_name}
          </span>
        </div>
        {logoSrc && (
          <Image
            src={logoSrc}
            alt={gameEntry?.name || simType}
            width={32}
            height={32}
            className="opacity-80"
          />
        )}
      </div>

      {/* Telemetry content */}
      <div className="flex-1 flex flex-col justify-between px-3 py-2">
        {/* Speed - hero metric */}
        <div className="flex items-baseline gap-1">
          <span className="text-4xl font-bold text-white font-display leading-none tabular-nums">
            {Math.round(speed)}
          </span>
          <span className="text-xs text-[#666] font-mono">km/h</span>
        </div>

        {/* Lap + Timer row */}
        <div className="flex items-center justify-between mt-1">
          <span className="text-sm text-[#888] font-mono">
            LAP {lapCount}
          </span>
          <span
            className={`text-sm font-mono tabular-nums font-semibold ${
              timerCritical
                ? "text-rp-red motion-safe:animate-pulse"
                : "text-[#888]"
            }`}
          >
            {formatTimer(remaining)}
          </span>
        </div>

        {/* Lap times */}
        <div className="flex items-center justify-between mt-1 gap-2">
          <div className="flex flex-col">
            <span
              className="text-[#666] uppercase tracking-wider font-mono"
              style={{ fontSize: "0.55rem" }}
            >
              Best
            </span>
            <span className="text-sm font-semibold text-purple-400 font-mono tabular-nums">
              {formatLapTime(bestLap)}
            </span>
          </div>
          <div className="flex flex-col items-end">
            <span
              className="text-[#666] uppercase tracking-wider font-mono"
              style={{ fontSize: "0.55rem" }}
            >
              Last
            </span>
            <span className="text-sm font-semibold text-emerald-400 font-mono tabular-nums">
              {formatLapTime(lastLap)}
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
