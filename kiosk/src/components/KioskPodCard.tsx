"use client";

import { useRef } from "react";
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

interface KioskPodCardProps {
  pod: Pod;
  telemetry?: TelemetryFrame;
  billing?: BillingSession;
  warning?: BillingWarning;
  gameInfo?: GameLaunchInfo;
  authToken?: AuthTokenInfo;
  walletBalance?: number; // paise
  onStartSession: (podId: string) => void;
  onEndSession: (billingSessionId: string) => void;
  onPauseSession: (billingSessionId: string) => void;
  onResumeSession: (billingSessionId: string) => void;
  onExtendSession: (billingSessionId: string) => void;
  onCancelAssignment: (tokenId: string) => void;
  onLaunchGame?: (podId: string) => void;
  onStartNow?: (authToken: AuthTokenInfo) => void;
  onTopUp?: (driverId: string) => void;
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

export function KioskPodCard({
  pod,
  telemetry,
  billing,
  warning,
  gameInfo,
  authToken,
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
}: KioskPodCardProps) {
  const state = derivePodState(pod, billing, authToken, gameInfo);
  const isOffline = pod.status === "offline";
  const hasWarning = !!warning;

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

  return (
    <div
      className={`
        relative flex flex-col rounded-lg border overflow-hidden transition-all duration-300
        ${isOffline ? "bg-zinc-900 border-zinc-800 opacity-60" : ""}
        ${state === "idle" && !isOffline ? "bg-rp-card border-rp-border hover:border-rp-grey" : ""}
        ${state === "on_track" ? "bg-rp-card border-rp-red/40 glow-active" : ""}
        ${state === "waiting" ? "bg-rp-card border-amber-500/40" : ""}
        ${state === "selecting" ? "bg-rp-card border-blue-500/40" : ""}
        ${state === "ending" ? "bg-rp-card border-green-500/40" : ""}
        ${hasWarning ? "border-amber-500 animate-pulse" : ""}
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
        <StateLabel state={state} isOffline={isOffline} />
      </div>

      {/* Card Body */}
      <div className="flex-1 px-4 py-3 flex flex-col gap-2 min-h-[160px]">
        {/* IDLE */}
        {state === "idle" && !isOffline && (
          <div className="flex-1 flex flex-col items-center justify-center gap-3">
            <p className="text-rp-grey text-sm">Ready for session</p>
            <button
              onClick={() => onStartSession(pod.id)}
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
                  onClick={() => onStartNow(authToken)}
                  className="px-4 py-1.5 text-xs bg-rp-red hover:bg-rp-red-hover text-white font-semibold rounded transition-colors"
                >
                  Start Now
                </button>
              )}
              <button
                onClick={() => onCancelAssignment(authToken.id)}
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
                onClick={() => onLaunchGame(pod.id)}
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
                  {formatTime(billing.remaining_seconds)}
                </span>
              </div>
              <div className="w-full h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all duration-1000 ${
                    hasWarning
                      ? "bg-amber-500"
                      : billing.remaining_seconds < 120
                      ? "bg-rp-red"
                      : "bg-rp-red"
                  }`}
                  style={{
                    width: `${Math.max(0, (billing.remaining_seconds / billing.allocated_seconds) * 100)}%`,
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
                  onClick={() => onPauseSession(billing.id)}
                  className="flex-1 px-2 py-1 text-xs border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded transition-colors"
                >
                  Pause
                </button>
              )}
              {billing.status === "paused_manual" && (
                <button
                  onClick={() => onResumeSession(billing.id)}
                  className="flex-1 px-2 py-1 text-xs bg-green-600 hover:bg-green-500 text-white rounded transition-colors"
                >
                  Resume
                </button>
              )}
              <button
                onClick={() => onExtendSession(billing.id)}
                className="flex-1 px-2 py-1 text-xs border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded transition-colors"
              >
                +10m
              </button>
              <button
                onClick={() => onEndSession(billing.id)}
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
              onClick={() => onStartSession(pod.id)}
              className="px-5 py-2 bg-rp-red hover:bg-rp-red-hover text-white font-semibold rounded-md transition-colors text-sm"
            >
              New Session
            </button>
          </div>
        )}
      </div>
    </div>
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
