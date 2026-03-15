"use client";

import { useRef, useState, useEffect } from "react";
import type {
  Pod,
  TelemetryFrame,
  BillingSession,
  BillingWarning,
  GameLaunchInfo,
} from "@/lib/types";
import { LiveTelemetry } from "./LiveTelemetry";
import { api } from "@/lib/api";

interface LiveSessionPanelProps {
  pod: Pod;
  telemetry?: TelemetryFrame;
  billing: BillingSession;
  warning?: BillingWarning;
  gameInfo?: GameLaunchInfo;
  walletBalance?: number;
  onEndSession: (billingSessionId: string) => void;
  onPauseSession: (billingSessionId: string) => void;
  onResumeSession: (billingSessionId: string) => void;
  onExtendSession: (billingSessionId: string) => void;
  onLaunchGame?: (podId: string) => void;
  onRelaunchGame?: (podId: string) => void;
  onTopUp?: (driverId: string) => void;
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function LiveSessionPanel({
  pod,
  telemetry,
  billing,
  warning,
  gameInfo,
  walletBalance,
  onEndSession,
  onPauseSession,
  onResumeSession,
  onExtendSession,
  onLaunchGame,
  onRelaunchGame,
  onTopUp,
}: LiveSessionPanelProps) {
  const hasWarning = !!warning;

  // Local countdown: interpolate between WebSocket ticks for smooth 1s updates
  const [localRemaining, setLocalRemaining] = useState(billing.remaining_seconds);
  useEffect(() => {
    setLocalRemaining(billing.remaining_seconds);
  }, [billing.remaining_seconds]);
  useEffect(() => {
    if (billing.status === "paused_manual") return;
    const iv = setInterval(() => {
      setLocalRemaining((prev) => Math.max(0, prev - 1));
    }, 1000);
    return () => clearInterval(iv);
  }, [billing.id, billing.status]);

  // Stabilize car/track display
  const stableCarRef = useRef<{ sessionId: string; car: string; track: string } | null>(null);
  if (billing && telemetry?.car) {
    if (!stableCarRef.current || stableCarRef.current.sessionId !== billing.id) {
      stableCarRef.current = { sessionId: billing.id, car: telemetry.car, track: telemetry.track };
    }
  }
  const stableCar = stableCarRef.current;

  return (
    <div className="flex flex-col h-full p-5 gap-5">
      {/* Driver + Session Info */}
      <div className="bg-rp-surface border border-rp-border rounded-xl p-4">
        <div className="flex items-center justify-between mb-3">
          <div>
            <p className="text-xl font-bold text-white">{billing.driver_name}</p>
            <p className="text-xs text-rp-grey">Pod {pod.number} &middot; {billing.pricing_tier_name}</p>
          </div>
          <div className="text-right">
            <div className="flex items-center gap-1.5">
              <span
                className={`w-2 h-2 rounded-full ${
                  billing.driving_state === "active"
                    ? "bg-green-500 pulse-dot"
                    : billing.driving_state === "idle"
                    ? "bg-amber-500"
                    : "bg-zinc-600"
                }`}
              />
              <span className="text-xs text-rp-grey capitalize">
                {billing.driving_state === "active" ? "Driving" : billing.driving_state === "idle" ? "Idle" : "No Device"}
              </span>
            </div>
            {walletBalance !== undefined && (
              <p className="text-xs text-rp-grey mt-1">
                Wallet: <span className="text-white font-medium">{(walletBalance / 100).toFixed(0)} cr</span>
              </p>
            )}
          </div>
        </div>

        {/* Car + Track */}
        {stableCar && (
          <div className="flex gap-2 text-xs text-rp-grey">
            <span className="bg-zinc-800 px-2 py-0.5 rounded">{stableCar.track}</span>
            <span className="bg-zinc-800 px-2 py-0.5 rounded">{stableCar.car}</span>
          </div>
        )}
      </div>

      {/* Session Timer — expanded */}
      <div className="bg-rp-surface border border-rp-border rounded-xl p-4">
        <div className="flex justify-between items-baseline mb-2">
          <span className="text-xs text-rp-grey uppercase tracking-wider">
            {billing.status === "paused_manual" ? "Paused" : "Session Time"}
          </span>
          <span className={`text-2xl font-bold font-mono tabular-nums ${hasWarning ? "text-amber-400" : "text-white"}`}>
            {formatTime(localRemaining)}
          </span>
        </div>
        <div className="w-full h-2 bg-zinc-800 rounded-full overflow-hidden">
          <div
            className={`h-full rounded-full transition-all duration-1000 ${
              hasWarning ? "bg-amber-500" : localRemaining < 120 ? "bg-rp-red" : "bg-rp-red"
            }`}
            style={{
              width: `${Math.max(0, (localRemaining / billing.allocated_seconds) * 100)}%`,
            }}
          />
        </div>
        <div className="flex justify-between text-xs text-rp-grey mt-1.5">
          <span>Drove {formatTime(billing.driving_seconds)}</span>
          <span>of {formatTime(billing.allocated_seconds)}</span>
        </div>
      </div>

      {/* Launch Game button — when billing active but no game running */}
      {onLaunchGame && (!gameInfo || gameInfo.game_state === "idle") && (
        <button
          onClick={() => onLaunchGame(pod.id)}
          className="py-3 bg-blue-600 hover:bg-blue-500 text-white font-semibold rounded-xl transition-colors"
        >
          Launch Game
        </button>
      )}

      {/* Game Crashed banner + Relaunch button */}
      {gameInfo?.game_state === "error" && (
        <div className="bg-red-900/30 border border-red-600/50 rounded-xl px-4 py-3 text-center">
          <span className="text-red-400 font-bold text-sm uppercase tracking-wider">Game Crashed</span>
          {gameInfo.error_message && (
            <p className="text-red-400/70 text-xs mt-1">{gameInfo.error_message}</p>
          )}
          {onRelaunchGame && (
            <button
              onClick={() => onRelaunchGame(pod.id)}
              className="mt-2 w-full py-3 bg-red-600 hover:bg-red-500 text-white font-semibold rounded-xl transition-colors"
            >
              Relaunch Game
            </button>
          )}
        </div>
      )}

      {/* Live Telemetry — expanded */}
      {telemetry && (
        <div className="bg-rp-surface border border-rp-border rounded-xl p-4">
          <h4 className="text-xs text-rp-grey uppercase tracking-wider mb-3">Live Telemetry</h4>
          <LiveTelemetry telemetry={telemetry} />
        </div>
      )}

      {/* Controls */}
      <div className="mt-auto space-y-3">
        {/* Quick controls row */}
        <div className="grid grid-cols-2 gap-2">
          <TransmissionTogglePanel podId={pod.id} />
          <FfbTogglePanel podId={pod.id} />
        </div>

        <div className="grid grid-cols-3 gap-2">
          {billing.status === "active" && (
            <button
              onClick={() => onPauseSession(billing.id)}
              className="py-2.5 border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded-lg text-sm transition-colors"
            >
              Pause
            </button>
          )}
          {billing.status === "paused_manual" && (
            <button
              onClick={() => onResumeSession(billing.id)}
              className="py-2.5 bg-green-600 hover:bg-green-500 text-white rounded-lg text-sm transition-colors"
            >
              Resume
            </button>
          )}
          <button
            onClick={() => onExtendSession(billing.id)}
            className="py-2.5 border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded-lg text-sm transition-colors"
          >
            +10 min
          </button>
          {onTopUp && (
            <button
              onClick={() => onTopUp(billing.driver_id)}
              className="py-2.5 border border-emerald-600/50 text-emerald-400 hover:bg-emerald-600/10 rounded-lg text-sm transition-colors"
            >
              Top Up
            </button>
          )}
        </div>

        <button
          onClick={() => onEndSession(billing.id)}
          className="w-full py-3 border-2 border-rp-red text-rp-red hover:bg-rp-red hover:text-white font-semibold rounded-xl text-sm transition-colors"
        >
          End Session
        </button>
      </div>
    </div>
  );
}

// Inline transmission toggle for panel (larger than card version)
function TransmissionTogglePanel({ podId }: { podId: string }) {
  const [mode, setMode] = useState<"auto" | "manual">("auto");
  const [busy, setBusy] = useState(false);

  const toggle = async () => {
    const next = mode === "auto" ? "manual" : "auto";
    setBusy(true);
    try {
      await api.setTransmission(podId, next);
      setMode(next);
    } catch { /* ignore */ }
    setBusy(false);
  };

  return (
    <button
      onClick={toggle}
      disabled={busy}
      className={`py-2.5 rounded-lg text-sm font-medium transition-colors ${
        mode === "manual"
          ? "bg-blue-600/20 border border-blue-500/50 text-blue-400"
          : "border border-rp-border text-rp-grey hover:text-white"
      }`}
    >
      {mode === "auto" ? "Auto Trans" : "Manual Trans"}
    </button>
  );
}

function FfbTogglePanel({ podId }: { podId: string }) {
  const [preset, setPreset] = useState<string>("medium");
  const [busy, setBusy] = useState(false);
  const FFB_CYCLE = ["light", "medium", "strong"] as const;
  const FFB_LABELS: Record<string, string> = { light: "FFB Light", medium: "FFB Medium", strong: "FFB Strong" };

  const cycle = async () => {
    const idx = FFB_CYCLE.indexOf(preset as typeof FFB_CYCLE[number]);
    const next = FFB_CYCLE[(idx + 1) % FFB_CYCLE.length];
    setBusy(true);
    try {
      await api.setFfb(podId, next);
      setPreset(next);
    } catch { /* ignore */ }
    setBusy(false);
  };

  return (
    <button
      onClick={cycle}
      disabled={busy}
      className={`py-2.5 rounded-lg text-sm font-medium transition-colors ${
        preset === "strong"
          ? "bg-rp-red/20 border border-rp-red/50 text-rp-red"
          : preset === "light"
          ? "bg-green-600/20 border border-green-500/50 text-green-400"
          : "border border-rp-border text-rp-grey hover:text-white"
      }`}
    >
      {FFB_LABELS[preset] || "FFB Medium"}
    </button>
  );
}

