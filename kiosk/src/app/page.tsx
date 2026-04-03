"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import type { Pod, TelemetryFrame, BillingSession, GameLaunchInfo, Lap } from "@/lib/types";

// ─── Helpers ─────────────────────────────────────────────────────────────

function formatLapTime(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const totalSec = ms / 1000;
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toFixed(3).padStart(6, "0")}`;
}

function gameLabel(simType: string): string {
  const map: Record<string, string> = {
    assetto_corsa: "AC",
    ac: "AC",
    f1_25: "F1",
    f1: "F1",
    iracing: "iR",
    le_mans_ultimate: "LMU",
    lmu: "LMU",
    forza: "FRZ",
  };
  return map[simType] || simType.toUpperCase().slice(0, 3);
}

function formatTimer(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
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

  // No PIN modal state needed — billing is started from POS by staff


  // ─── Pod sorting ──────────────────────────────────────────────────────

  const sortedPods = Array.from(pods.values()).sort((a, b) => a.number - b.number);
  // Ensure 8 slots
  const podSlots: (Pod | null)[] = [];
  for (let i = 1; i <= 8; i++) {
    podSlots.push(sortedPods.find((p) => p.number === i) || null);
  }

  const idleCount = sortedPods.filter((p) => p.status === "idle").length;
  const activeCount = sortedPods.filter((p) => p.status === "in_session").length;
  const offlineCount = sortedPods.filter((p) => p.status === "offline" || p.status === "disabled").length;

  // ─── Render ───────────────────────────────────────────────────────────

  return (
    <div className="h-screen flex flex-col bg-rp-black overflow-hidden">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-3 bg-rp-card border-b border-rp-border">
        <div className="flex items-center gap-3">
          <h1 className="text-xl font-bold tracking-wide uppercase text-white font-[family-name:var(--font-display)]">
            RACING<span className="text-rp-red">POINT</span>
          </h1>
          <span className="text-xs text-rp-grey font-medium tracking-widest uppercase">
            Choose Your Rig
          </span>
        </div>

        <div className="flex items-center gap-4 text-sm">
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-green-500" />
            <span className="text-white font-semibold">{idleCount}</span>
            <span className="text-rp-grey">Available</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-rp-red" />
            <span className="text-white font-semibold">{activeCount}</span>
            <span className="text-rp-grey">Racing</span>
          </div>
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-zinc-500" />
            <span className="text-white font-semibold">{offlineCount}</span>
            <span className="text-rp-grey">Offline</span>
          </div>
        </div>

        <div className="flex items-center gap-4">
          <div data-testid="ws-status" className="flex items-center gap-2">
            <span
              className={`w-2.5 h-2.5 rounded-full ${
                connected ? "bg-green-500 pulse-dot" : "bg-red-500"
              }`}
            />
            <span className="text-xs text-rp-grey">
              {connected ? "Live" : "Connecting..."}
            </span>
          </div>
        </div>
      </header>

      {/* Pod Grid — 4x2 */}
      <main data-testid="pod-grid" className="flex-1 p-4 overflow-hidden">
        <div className="grid grid-cols-4 grid-rows-2 gap-3 h-full">
          {podSlots.map((pod, idx) => {
            const podNum = idx + 1;

            if (!pod) {
              // Empty slot — show shimmer while WS connecting, "Offline" once connected
              return (
                <div
                  key={`empty-${podNum}`}
                  className={`rounded-xl border border-rp-border bg-rp-card/30 flex flex-col items-center justify-center ${
                    connected ? "opacity-40" : "animate-pulse opacity-30"
                  }`}
                >
                  <span className="text-4xl font-bold text-rp-grey font-[family-name:var(--font-display)]">
                    {podNum}
                  </span>
                  <span className="text-xs text-rp-grey mt-1">
                    {connected ? "Offline" : ""}
                  </span>
                </div>
              );
            }

            const billing = billingTimers.get(pod.id);
            const telemetry = latestTelemetry.get(pod.id);
            const gameInfo = gameStates.get(pod.id);
            const isActive = pod.status === "in_session" && billing;
            const isIdle = pod.status === "idle";
            const isOffline = pod.status === "offline" || pod.status === "disabled";

            // ── Active pod card ──
            if (isActive && billing) {
              const podLaps = recentLaps.filter((l) => l.driver_id === billing.driver_id);
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

            // ── Offline/disabled pod ──
            if (isOffline) {
              return (
                <div
                  key={pod.id}
                  className="rounded-xl border border-rp-border bg-rp-card/30 flex flex-col items-center justify-center opacity-40"
                >
                  <span className="text-4xl font-bold text-rp-grey font-[family-name:var(--font-display)]">
                    {pod.number}
                  </span>
                  <span className="text-xs text-rp-grey mt-1">
                    {pod.status === "disabled" ? "Maintenance" : "Offline"}
                  </span>
                </div>
              );
            }

            // ── Idle pod card (display only — staff starts sessions from POS) ──
            return (
              <div
                key={pod.id}
                data-testid={`pod-card-${pod.number}`}
                className="rounded-xl border-2 border-green-500/30 bg-rp-card flex flex-col items-center justify-center gap-3"
              >
                <span className="text-5xl font-bold text-white font-[family-name:var(--font-display)]">
                  {pod.number}
                </span>
                <span className="px-3 py-1 rounded-full bg-green-500/15 text-green-400 text-xs font-semibold uppercase tracking-wider">
                  Available
                </span>
              </div>
            );
          })}
        </div>
      </main>

      {/* Footer — staff login only */}
      <footer className="flex items-center justify-center py-3 border-t border-rp-border bg-rp-card">
        <Link
          href="/staff"
          className="px-6 py-2 text-xs font-medium border border-rp-border rounded-lg text-rp-grey hover:text-white hover:border-rp-red transition-colors"
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
  const brake = telemetry?.brake ?? 0;
  const lapCount = telemetry?.lap_number ?? 0;
  const simType = gameInfo?.sim_type || "";

  // Derive best/last lap from recent laps for this driver
  const validLaps = podLaps.filter((l) => l.valid && l.lap_time_ms > 0);
  const bestLap = validLaps.length > 0
    ? Math.min(...validLaps.map((l) => l.lap_time_ms))
    : 0;
  const lastLap = validLaps.length > 0 ? validLaps[0]?.lap_time_ms ?? 0 : 0;

  return (
    <div className="rounded-xl border border-rp-red/40 bg-rp-card flex flex-col overflow-hidden glow-active">
      {/* Top bar: pod number + driver + game */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-rp-border">
        <div className="flex items-center gap-2">
          <span className="text-lg font-bold text-white font-[family-name:var(--font-display)]">
            {pod.number}
          </span>
          <span className="text-sm text-rp-grey truncate max-w-[100px]">
            {billing.driver_name}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {simType && (
            <span className="px-2 py-0.5 rounded bg-rp-red/20 text-rp-red text-xs font-bold">
              {gameLabel(simType)}
            </span>
          )}
          <div className="flex flex-col items-end">
            <span className="text-[0.55rem] text-rp-grey uppercase tracking-wider leading-none">Remaining</span>
            <span className={`text-xs font-[family-name:var(--font-mono-jb)] ${remaining < 300 ? "text-rp-red animate-pulse" : "text-rp-grey"}`}>
              {formatTimer(remaining)}
            </span>
          </div>
        </div>
      </div>

      {/* Telemetry grid */}
      <div className="flex-1 grid grid-cols-2 gap-x-3 gap-y-1 px-3 py-2 text-xs">
        {/* Speed */}
        <div className="flex flex-col">
          <span className="text-rp-grey uppercase tracking-wider" style={{ fontSize: "0.6rem" }}>
            Speed
          </span>
          <span className="text-xl font-bold text-white font-[family-name:var(--font-mono-jb)] leading-tight">
            {Math.round(speed)}
            <span className="text-rp-grey text-xs ml-0.5">km/h</span>
          </span>
        </div>

        {/* RPM */}
        <div className="flex flex-col">
          <span className="text-rp-grey uppercase tracking-wider" style={{ fontSize: "0.6rem" }}>
            RPM
          </span>
          <span className="text-xl font-bold text-white font-[family-name:var(--font-mono-jb)] leading-tight">
            {rpm > 1000 ? `${(rpm / 1000).toFixed(1)}k` : rpm}
          </span>
        </div>

        {/* Brake */}
        <div className="flex flex-col">
          <span className="text-rp-grey uppercase tracking-wider" style={{ fontSize: "0.6rem" }}>
            Brake
          </span>
          <div className="flex items-center gap-1.5">
            <div className="flex-1 h-2 rounded-full bg-rp-surface overflow-hidden">
              <div
                className="h-full rounded-full bg-rp-red transition-all"
                style={{ width: `${Math.round(brake * 100)}%` }}
              />
            </div>
            <span className="text-white font-[family-name:var(--font-mono-jb)] w-8 text-right">
              {Math.round(brake * 100)}%
            </span>
          </div>
        </div>

        {/* Lap count */}
        <div className="flex flex-col">
          <span className="text-rp-grey uppercase tracking-wider" style={{ fontSize: "0.6rem" }}>
            Laps
          </span>
          <span className="text-lg font-bold text-white font-[family-name:var(--font-mono-jb)]">
            {lapCount}
          </span>
        </div>

        {/* Best lap */}
        <div className="flex flex-col">
          <span className="text-rp-grey uppercase tracking-wider" style={{ fontSize: "0.6rem" }}>
            Best
          </span>
          <span className="text-sm font-semibold text-purple-400 font-[family-name:var(--font-mono-jb)]">
            {formatLapTime(bestLap)}
          </span>
        </div>

        {/* Last lap */}
        <div className="flex flex-col">
          <span className="text-rp-grey uppercase tracking-wider" style={{ fontSize: "0.6rem" }}>
            Last
          </span>
          <span className="text-sm font-semibold text-green-400 font-[family-name:var(--font-mono-jb)]">
            {formatLapTime(lastLap)}
          </span>
        </div>
      </div>
    </div>
  );
}

