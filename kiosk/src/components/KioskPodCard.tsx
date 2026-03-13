"use client";

import React, { useRef, useState, useEffect } from "react";
import type {
  Pod,
  TelemetryFrame,
  BillingSession,
  BillingWarning,
  GameLaunchInfo,
  AuthTokenInfo,
  KioskPodState,
} from "@/lib/types";
import { LiveTelemetry } from "./LiveTelemetry";
import { api } from "@/lib/api";

interface KioskPodCardProps {
  pod: Pod;
  telemetry?: TelemetryFrame;
  billing?: BillingSession;
  warning?: BillingWarning;
  gameInfo?: GameLaunchInfo;
  authToken?: AuthTokenInfo;
  walletBalance?: number; // paise
  compact?: boolean;
  isSelected?: boolean;
  onSelect?: (podId: string) => void;
  onStartSession: (podId: string) => void;
  onEndSession: (billingSessionId: string) => void;
  onPauseSession: (billingSessionId: string) => void;
  onResumeSession: (billingSessionId: string) => void;
  onExtendSession: (billingSessionId: string) => void;
  onCancelAssignment: (tokenId: string) => void;
  onLaunchGame?: (podId: string) => void;
  onStartNow?: (authToken: AuthTokenInfo) => void;
  onTopUp?: (driverId: string) => void;
  onWakePod?: (podId: string) => void;
  onRestartPod?: (podId: string) => void;
  onShutdownPod?: (podId: string) => void;
}

function derivePodState(
  pod: Pod,
  billing?: BillingSession,
  authToken?: AuthTokenInfo,
  gameInfo?: GameLaunchInfo
): KioskPodState {
  if (pod.status === "offline") return "idle";

  // Has pending auth token → waiting for customer
  if (authToken && authToken.status === "pending") return "waiting";

  // Active billing session
  if (billing) {
    if (billing.status === "completed" || billing.status === "ended_early") return "ending";
    if (gameInfo?.game_state === "running") return "on_track";
    if (gameInfo?.game_state === "launching") return "selecting";
    return "on_track";
  }

  return "idle";
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function formatLapTime(ms: number): string {
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = (totalSecs % 60).toFixed(3);
  return `${mins}:${parseFloat(secs) < 10 ? "0" : ""}${secs}`;
}

export const KioskPodCard = React.memo(function KioskPodCard({
  pod,
  telemetry,
  billing,
  warning,
  gameInfo,
  authToken,
  compact = false,
  isSelected = false,
  onSelect,
  onStartSession,
  onEndSession,
  onPauseSession,
  onResumeSession,
  onExtendSession,
  onCancelAssignment,
  onLaunchGame,
  onStartNow,
  walletBalance,
  onTopUp,
  onWakePod,
  onRestartPod,
  onShutdownPod,
}: KioskPodCardProps) {
  const state = derivePodState(pod, billing, authToken, gameInfo);
  const isOffline = pod.status === "offline";
  const hasWarning = !!warning;

  // Local countdown: interpolate between WebSocket billing ticks for smooth 1s updates
  const [localRemaining, setLocalRemaining] = useState(billing?.remaining_seconds ?? 0);
  useEffect(() => {
    if (billing) setLocalRemaining(billing.remaining_seconds);
  }, [billing?.remaining_seconds]);
  useEffect(() => {
    if (!billing || billing.status === "paused_manual") return;
    const iv = setInterval(() => {
      setLocalRemaining((prev) => Math.max(0, prev - 1));
    }, 1000);
    return () => clearInterval(iv);
  }, [billing?.id, billing?.status]);
  const displayRemaining = billing ? localRemaining : 0;

  // Stabilize car/track display — AC shared memory can flicker between cars
  const stableCarRef = useRef<{ sessionId: string; car: string; track: string } | null>(null);
  if (billing && telemetry?.car) {
    if (!stableCarRef.current || stableCarRef.current.sessionId !== billing.id) {
      // New session or first telemetry — lock in the car
      stableCarRef.current = { sessionId: billing.id, car: telemetry.car, track: telemetry.track };
    }
  }
  if (!billing) {
    stableCarRef.current = null;
  }
  const stableCar = stableCarRef.current;

  // Click handler for compact mode
  const handleCardClick = () => {
    if (onSelect) {
      onSelect(pod.id);
    }
  };

  // ─── COMPACT MODE ──────────────────────────────────────────────────────
  if (compact) {
    return (
      <div
        onClick={handleCardClick}
        className={`
          relative flex flex-col rounded-lg border overflow-hidden transition-all duration-300 cursor-pointer
          ${isSelected ? "ring-2 ring-rp-red border-rp-red" : ""}
          ${isOffline ? "bg-zinc-900 border-zinc-800 opacity-60" : ""}
          ${state === "idle" && !isOffline && !isSelected ? "bg-rp-card border-rp-border hover:border-rp-grey" : ""}
          ${state === "on_track" && !isSelected ? "bg-rp-card border-rp-red/40" : ""}
          ${state === "waiting" && !isSelected ? "bg-rp-card border-amber-500/40" : ""}
          ${state === "selecting" && !isSelected ? "bg-rp-card border-blue-500/40" : ""}
          ${state === "ending" && !isSelected ? "bg-rp-card border-green-500/40" : ""}
          ${hasWarning && !isSelected ? "border-amber-500 animate-pulse" : ""}
        `}
      >
        {/* Compact Header */}
        <div className="flex items-center justify-between px-3 py-1.5 border-b border-rp-border/50">
          <div className="flex items-center gap-1.5">
            <span
              className={`w-1.5 h-1.5 rounded-full ${
                isOffline ? "bg-zinc-600" : pod.status === "in_session" ? "bg-rp-red" : "bg-green-500"
              }`}
            />
            <span className="font-semibold text-xs">Pod {pod.number}</span>
          </div>
          <StateLabel state={state} isOffline={isOffline} />
        </div>

        {/* Compact Body */}
        <div className="px-3 py-2 flex flex-col gap-1">
          {/* Driver name */}
          {billing && (
            <p className="text-white font-semibold text-xs truncate">{billing.driver_name}</p>
          )}
          {state === "waiting" && authToken && (
            <p className="text-amber-400 text-xs truncate">{authToken.driver_name}</p>
          )}
          {state === "idle" && !isOffline && (
            <p className="text-rp-grey text-xs">Ready</p>
          )}
          {isOffline && (
            <p className="text-zinc-600 text-xs">Offline</p>
          )}

          {/* Timer bar (compact) */}
          {billing && state === "on_track" && (
            <div>
              <div className="flex justify-between text-[10px] text-rp-grey mb-0.5">
                <span>{billing.status === "paused_manual" ? "Paused" : ""}</span>
                <span className={`font-mono ${hasWarning ? "text-amber-400 font-bold" : ""}`}>
                  {formatTime(displayRemaining)}
                </span>
              </div>
              <div className="w-full h-1 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all duration-1000 ${
                    hasWarning ? "bg-amber-500" : "bg-rp-red"
                  }`}
                  style={{
                    width: `${Math.max(0, (displayRemaining / billing.allocated_seconds) * 100)}%`,
                  }}
                />
              </div>
            </div>
          )}

          {/* Waiting PIN (compact) */}
          {state === "waiting" && authToken?.auth_type === "pin" && (
            <p className="text-lg font-bold tracking-[0.2em] text-white font-mono text-center">
              {authToken.token}
            </p>
          )}
        </div>
      </div>
    );
  }

  // ─── FULL MODE (original) ──────────────────────────────────────────────
  return (
    <div
      onClick={onSelect ? handleCardClick : undefined}
      className={`
        relative flex flex-col rounded-lg border overflow-hidden transition-all duration-300
        ${onSelect ? "cursor-pointer" : ""}
        ${isSelected ? "ring-2 ring-rp-red border-rp-red" : ""}
        ${isOffline ? "bg-zinc-900 border-zinc-800 opacity-60" : ""}
        ${state === "idle" && !isOffline && !isSelected ? "bg-rp-card border-rp-border hover:border-rp-grey" : ""}
        ${state === "on_track" && !isSelected ? "bg-rp-card border-rp-red/40 glow-active" : ""}
        ${state === "waiting" && !isSelected ? "bg-rp-card border-amber-500/40" : ""}
        ${state === "selecting" && !isSelected ? "bg-rp-card border-blue-500/40" : ""}
        ${state === "ending" && !isSelected ? "bg-rp-card border-green-500/40" : ""}
        ${hasWarning && !isSelected ? "border-amber-500 animate-pulse" : ""}
      `}
    >
      {/* Pod Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-rp-border/50">
        <div className="flex items-center gap-2">
          <span
            className={`w-2 h-2 rounded-full ${
              isOffline ? "bg-zinc-600" : pod.status === "in_session" ? "bg-rp-red" : "bg-green-500"
            }`}
          />
          <span className="font-semibold text-sm">Pod {pod.number}</span>
        </div>
        <div className="flex items-center gap-2">
          {/* Power buttons */}
          {isOffline && onWakePod && (
            <button
              onClick={(e) => { e.stopPropagation(); onWakePod(pod.id); }}
              className="w-8 h-8 flex items-center justify-center rounded-md bg-green-900/40 text-green-400 hover:bg-green-800/50 transition-colors"
              title="Power on (WOL)"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
            </button>
          )}
          {!isOffline && onRestartPod && (
            <button
              onClick={(e) => { e.stopPropagation(); onRestartPod(pod.id); }}
              className="w-8 h-8 flex items-center justify-center rounded-md bg-yellow-900/40 text-yellow-400 hover:bg-yellow-800/50 transition-colors"
              title="Restart pod"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            </button>
          )}
          {!isOffline && onShutdownPod && (
            <button
              onClick={(e) => { e.stopPropagation(); onShutdownPod(pod.id); }}
              className="w-8 h-8 flex items-center justify-center rounded-md bg-red-900/40 text-red-400 hover:bg-red-800/50 transition-colors"
              title="Shutdown pod"
            >
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                <path strokeLinecap="round" strokeLinejoin="round" d="M5.636 5.636a9 9 0 1012.728 0M12 3v9" />
              </svg>
            </button>
          )}
          {/* Blank screen toggle — available when online */}
          {!isOffline && <BlankScreenButton podId={pod.id} />}
          <StateLabel state={state} isOffline={isOffline} />
        </div>
      </div>

      {/* Card Body */}
      <div className="flex-1 px-4 py-3 flex flex-col gap-2 min-h-[160px]">
        {/* IDLE */}
        {state === "idle" && !isOffline && (
          <div className="flex-1 flex flex-col items-center justify-center gap-3">
            <p className="text-rp-grey text-sm">Ready for session</p>
            <button
              onClick={(e) => { e.stopPropagation(); onStartSession(pod.id); }}
              className="px-6 py-2.5 bg-rp-red hover:bg-rp-red-hover text-white font-semibold rounded-md transition-colors text-sm"
            >
              Start Session
            </button>
          </div>
        )}

        {/* OFFLINE */}
        {isOffline && (
          <div className="flex-1 flex items-center justify-center">
            <p className="text-zinc-600 text-sm">Offline</p>
          </div>
        )}

        {/* WAITING FOR CUSTOMER (PIN/QR assigned) */}
        {state === "waiting" && authToken && (
          <div className="flex-1 flex flex-col items-center justify-center gap-2">
            <p className="text-amber-400 text-xs font-medium uppercase tracking-wider">
              Waiting for Customer
            </p>
            <p className="text-sm text-rp-grey">{authToken.driver_name}</p>
            {authToken.auth_type === "pin" && (
              <p className="text-3xl font-bold tracking-[0.3em] text-white font-mono">
                {authToken.token}
              </p>
            )}
            {authToken.auth_type === "qr" && (
              <p className="text-sm text-rp-grey">Scan QR at pod</p>
            )}
            <p className="text-xs text-rp-grey">{authToken.pricing_tier_name}</p>
            <div className="flex gap-2 mt-1">
              {onStartNow && (
                <button
                  onClick={(e) => { e.stopPropagation(); onStartNow(authToken); }}
                  className="px-4 py-1.5 text-xs bg-rp-red hover:bg-rp-red-hover text-white font-semibold rounded transition-colors"
                >
                  Start Now
                </button>
              )}
              <button
                onClick={(e) => { e.stopPropagation(); onCancelAssignment(authToken.id); }}
                className="px-3 py-1.5 text-xs border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded transition-colors"
              >
                Cancel
              </button>
            </div>
          </div>
        )}

        {/* SELECTING / LAUNCHING */}
        {state === "selecting" && (
          <div className="flex-1 flex flex-col items-center justify-center gap-2">
            <p className="text-blue-400 text-xs font-medium uppercase tracking-wider">
              Launching Game
            </p>
            {gameInfo && (
              <p className="text-sm text-white">{gameInfo.sim_type}</p>
            )}
            <div className="w-6 h-6 border-2 border-blue-400 border-t-transparent rounded-full animate-spin" />
          </div>
        )}

        {/* ON TRACK */}
        {state === "on_track" && billing && (
          <div className="flex-1 flex flex-col gap-2">
            {/* Driver + Experience */}
            <div>
              <p className="text-white font-semibold text-sm truncate">
                {billing.driver_name}
              </p>
            </div>

            {/* Launch Game button — shown when billing active but no game running */}
            {onLaunchGame && (!gameInfo || gameInfo.game_state === "idle") && (
              <button
                onClick={(e) => { e.stopPropagation(); onLaunchGame(pod.id); }}
                className="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white font-semibold rounded-md transition-colors text-sm"
              >
                Launch Game
              </button>
            )}

            {/* Live Telemetry */}
            {telemetry && <LiveTelemetry telemetry={telemetry} />}

            {/* Session Timer */}
            <div className="mt-auto">
              <div className="flex justify-between text-xs text-rp-grey mb-1">
                <span>{billing.status === "paused_manual" ? "Paused" : "Remaining"}</span>
                <span className={`font-mono ${hasWarning ? "text-amber-400 font-bold" : ""}`}>
                  {formatTime(displayRemaining)}
                </span>
              </div>
              <div className="w-full h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all duration-1000 ${
                    hasWarning
                      ? "bg-amber-500"
                      : displayRemaining < 120
                      ? "bg-rp-red"
                      : "bg-rp-red"
                  }`}
                  style={{
                    width: `${Math.max(0, (displayRemaining / billing.allocated_seconds) * 100)}%`,
                  }}
                />
              </div>
              {/* Driving state indicator */}
              <div className="flex items-center gap-1 mt-1">
                <span
                  className={`w-1.5 h-1.5 rounded-full ${
                    billing.driving_state === "active"
                      ? "bg-green-500 pulse-dot"
                      : billing.driving_state === "idle"
                      ? "bg-amber-500"
                      : "bg-zinc-600"
                  }`}
                />
                <span className="text-[10px] text-rp-grey capitalize">
                  {billing.driving_state === "active" ? "Driving" : billing.driving_state === "idle" ? "Idle" : "No Device"}
                </span>
              </div>
            </div>

            {/* Quick Actions */}
            <div className="flex gap-1.5 mt-1">
              {billing.status === "active" && (
                <button
                  onClick={(e) => { e.stopPropagation(); onPauseSession(billing.id); }}
                  className="flex-1 px-2 py-1 text-xs border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded transition-colors"
                >
                  Pause
                </button>
              )}
              {billing.status === "paused_manual" && (
                <button
                  onClick={(e) => { e.stopPropagation(); onResumeSession(billing.id); }}
                  className="flex-1 px-2 py-1 text-xs bg-green-600 hover:bg-green-500 text-white rounded transition-colors"
                >
                  Resume
                </button>
              )}
              <TransmissionToggle podId={pod.id} />
              <FfbToggle podId={pod.id} />
              <button
                onClick={(e) => { e.stopPropagation(); onExtendSession(billing.id); }}
                className="flex-1 px-2 py-1 text-xs border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded transition-colors"
              >
                +10m
              </button>
              <button
                onClick={(e) => { e.stopPropagation(); onEndSession(billing.id); }}
                className="flex-1 px-2 py-1 text-xs border border-rp-red/50 text-rp-red hover:bg-rp-red/10 rounded transition-colors"
              >
                End
              </button>
            </div>
          </div>
        )}

        {/* ENDING */}
        {state === "ending" && (
          <div className="flex-1 flex flex-col items-center justify-center gap-3">
            <p className="text-green-400 text-sm font-semibold">Session Complete!</p>
            {billing && (
              <div className="text-center text-xs text-rp-grey space-y-0.5">
                <p>{billing.driver_name}</p>
                <p>Drove {formatTime(billing.driving_seconds)}</p>
              </div>
            )}
            <button
              onClick={(e) => { e.stopPropagation(); onStartSession(pod.id); }}
              className="px-5 py-2 bg-rp-red hover:bg-rp-red-hover text-white font-semibold rounded-md transition-colors text-sm"
            >
              New Session
            </button>
          </div>
        )}
      </div>
    </div>
  );
});

function TransmissionToggle({ podId }: { podId: string }) {
  const [mode, setMode] = useState<"auto" | "manual">("auto");
  const [busy, setBusy] = useState(false);

  const toggle = async () => {
    const next = mode === "auto" ? "manual" : "auto";
    setBusy(true);
    try {
      await api.setTransmission(podId, next);
      setMode(next);
    } catch {
      // ignore
    }
    setBusy(false);
  };

  return (
    <button
      onClick={toggle}
      disabled={busy}
      className={`flex-1 px-2 py-1 text-xs rounded transition-colors ${
        mode === "manual"
          ? "bg-blue-600/20 border border-blue-500/50 text-blue-400 hover:bg-blue-600/30"
          : "border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey"
      }`}
    >
      {mode === "auto" ? "Auto" : "Manual"}
    </button>
  );
}

const FFB_CYCLE = ["light", "medium", "strong"] as const;
const FFB_LABELS: Record<string, string> = { light: "FFB Lo", medium: "FFB Mid", strong: "FFB Hi" };

function FfbToggle({ podId }: { podId: string }) {
  const [preset, setPreset] = useState<string>("medium");
  const [busy, setBusy] = useState(false);

  const cycle = async () => {
    const idx = FFB_CYCLE.indexOf(preset as typeof FFB_CYCLE[number]);
    const next = FFB_CYCLE[(idx + 1) % FFB_CYCLE.length];
    setBusy(true);
    try {
      await api.setFfb(podId, next);
      setPreset(next);
    } catch {
      // ignore
    }
    setBusy(false);
  };

  return (
    <button
      onClick={cycle}
      disabled={busy}
      className={`flex-1 px-2 py-1 text-xs rounded transition-colors ${
        preset === "strong"
          ? "bg-rp-red/20 border border-rp-red/50 text-rp-red hover:bg-rp-red/30"
          : preset === "light"
          ? "bg-green-600/20 border border-green-500/50 text-green-400 hover:bg-green-600/30"
          : "border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey"
      }`}
    >
      {FFB_LABELS[preset] || "FFB Mid"}
    </button>
  );
}

function BlankScreenButton({ podId }: { podId: string }) {
  const [blanked, setBlanked] = useState(false);
  const [busy, setBusy] = useState(false);

  const toggle = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const next = !blanked;
    setBusy(true);
    try {
      const res = await api.setBlankScreen(podId, next);
      if (res.ok) {
        setBlanked(next);
      }
    } catch (err) {
      alert(`Screen ${next ? "blank" : "unblank"} failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
    setBusy(false);
  };

  return (
    <button
      onClick={toggle}
      disabled={busy}
      className={`w-8 h-8 flex items-center justify-center rounded-md transition-colors ${
        blanked
          ? "bg-zinc-600 text-zinc-300 hover:bg-zinc-500"
          : "bg-zinc-800 text-rp-grey hover:bg-zinc-700 hover:text-white"
      }`}
      title={blanked ? "Unblank screen" : "Blank screen"}
    >
      <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        {blanked ? (
          <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
        ) : (
          <path strokeLinecap="round" strokeLinejoin="round" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M3 3l18 18" />
        )}
      </svg>
    </button>
  );
}

function StateLabel({ state, isOffline }: { state: KioskPodState; isOffline: boolean }) {
  if (isOffline)
    return <span className="text-[10px] font-medium text-zinc-600 uppercase tracking-wider">Offline</span>;

  const styles: Record<KioskPodState, string> = {
    idle: "text-green-500 bg-green-500/10",
    registering: "text-blue-400 bg-blue-400/10",
    waiting: "text-amber-400 bg-amber-400/10",
    selecting: "text-blue-400 bg-blue-400/10",
    on_track: "text-rp-red bg-rp-red/10",
    ending: "text-green-400 bg-green-400/10",
  };

  const labels: Record<KioskPodState, string> = {
    idle: "Idle",
    registering: "Registering",
    waiting: "Waiting",
    selecting: "Launching",
    on_track: "On Track",
    ending: "Complete",
  };

  return (
    <span className={`text-[10px] font-semibold uppercase tracking-wider px-2 py-0.5 rounded ${styles[state]}`}>
      {labels[state]}
    </span>
  );
}
