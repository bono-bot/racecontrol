"use client";

import { useState, useEffect } from "react";
import { LiveTelemetry } from "./LiveTelemetry";
import { F1Speedometer } from "./F1Speedometer";
import { SessionTimer } from "./SessionTimer";
import type {
  Pod,
  BillingSession,
  TelemetryFrame,
  GameLaunchInfo,
  AuthTokenInfo,
  KioskExperience,
  BillingWarning,
} from "@/lib/types";

function formatLapTimeShort(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = (totalSecs % 60).toFixed(3);
  return `${mins}:${parseFloat(secs) < 10 ? "0" : ""}${secs}`;
}

// ─── Re-export for consistent styling across files ──────────────────────────

export const GAME_LABELS: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  f1_25: "F1 25",
  assetto_corsa_rally: "AC Rally",
  le_mans_ultimate: "LeMans Ultimate",
  iracing: "iRacing",
};

export const CLASS_COLORS: Record<string, string> = {
  A: "bg-rp-red text-white",
  B: "bg-orange-500 text-white",
  C: "bg-amber-500 text-black",
  D: "bg-green-500 text-white",
};

// ─── State Derivation ───────────────────────────────────────────────────────

type KioskViewState = "disabled" | "idle" | "waiting" | "launching" | "in_session" | "complete";

function deriveKioskState(
  pod: Pod,
  billing?: BillingSession,
  gameInfo?: GameLaunchInfo,
  authToken?: AuthTokenInfo
): KioskViewState {
  if (pod.status === "disabled") return "disabled";
  if (authToken && authToken.status === "pending") return "waiting";
  if (billing) {
    if (billing.status === "completed" || billing.status === "ended_early") return "complete";
    if (gameInfo?.game_state === "launching") return "launching";
    return "in_session";
  }
  return "idle";
}

// ─── Props ──────────────────────────────────────────────────────────────────

interface PodKioskViewProps {
  pod: Pod;
  billing?: BillingSession;
  telemetry?: TelemetryFrame;
  gameInfo?: GameLaunchInfo;
  authToken?: AuthTokenInfo;
  experiences: KioskExperience[];
  mode: "standalone" | "control";
  onSelectExperience?: (experienceId: string) => void;
  onEndSession?: () => void;
  warning?: BillingWarning;
}

// ─── Component ──────────────────────────────────────────────────────────────

export function PodKioskView({
  pod,
  billing,
  telemetry,
  gameInfo,
  authToken,
  experiences,
  mode,
  onSelectExperience,
  onEndSession,
  warning,
}: PodKioskViewProps) {
  const state = deriveKioskState(pod, billing, gameInfo, authToken);
  const isStandalone = mode === "standalone";

  return (
    <div
      className={`flex flex-col ${
        isStandalone
          ? "h-screen w-screen bg-rp-black"
          : "h-full w-full bg-rp-card rounded-lg border border-rp-border"
      }`}
    >
      {state === "disabled" && <DisabledView isStandalone={isStandalone} />}
      {state === "idle" && (
        <IdleView
          experiences={experiences}
          isStandalone={isStandalone}
          onSelectExperience={onSelectExperience}
        />
      )}
      {state === "waiting" && (
        <WaitingView authToken={authToken!} isStandalone={isStandalone} />
      )}
      {state === "launching" && (
        <LaunchingView gameInfo={gameInfo} isStandalone={isStandalone} />
      )}
      {state === "in_session" && (
        <InSessionView
          billing={billing!}
          telemetry={telemetry}
          gameInfo={gameInfo}
          isStandalone={isStandalone}
          onEndSession={onEndSession}
          warning={warning}
        />
      )}
      {state === "complete" && (
        <CompleteView billing={billing!} isStandalone={isStandalone} />
      )}
    </div>
  );
}

// ─── Disabled ───────────────────────────────────────────────────────────────

function DisabledView({ isStandalone }: { isStandalone: boolean }) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center opacity-50">
      <svg
        className={`${isStandalone ? "w-24 h-24" : "w-10 h-10"} text-rp-grey mb-3`}
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={1.5}
      >
        <circle cx="12" cy="12" r="10" />
        <line x1="4" y1="4" x2="20" y2="20" />
      </svg>
      <p className={`font-semibold text-rp-grey ${isStandalone ? "text-2xl" : "text-sm"}`}>
        Kiosk Disabled
      </p>
      <p className={`text-rp-grey mt-1 ${isStandalone ? "text-base" : "text-[10px]"}`}>
        Please seek operator assistance
      </p>
    </div>
  );
}

// ─── Idle (Experience Selector) ─────────────────────────────────────────────

function IdleView({
  experiences,
  isStandalone,
  onSelectExperience,
}: {
  experiences: KioskExperience[];
  isStandalone: boolean;
  onSelectExperience?: (id: string) => void;
}) {
  return (
    <div className={`flex-1 flex flex-col ${isStandalone ? "p-8" : "p-2"} overflow-hidden`}>
      <h2
        className={`font-bold text-white uppercase tracking-wide ${
          isStandalone ? "text-2xl mb-6" : "text-xs mb-2"
        }`}
      >
        Select Experience
      </h2>

      <div className={`flex-1 overflow-y-auto ${isStandalone ? "space-y-3" : "space-y-1"}`}>
        {experiences.length === 0 ? (
          <p className={`text-rp-grey ${isStandalone ? "text-lg" : "text-[10px]"}`}>
            No experiences available
          </p>
        ) : (
          experiences.map((exp) => (
            <button
              key={exp.id}
              onClick={() => onSelectExperience?.(exp.id)}
              className={`w-full flex items-center gap-2 border border-rp-border rounded transition-colors text-left hover:border-rp-red/50 bg-rp-surface ${
                isStandalone ? "px-5 py-4 gap-4" : "px-2 py-1.5"
              }`}
            >
              {exp.car_class && (
                <span
                  className={`flex items-center justify-center rounded font-bold ${
                    CLASS_COLORS[exp.car_class] || "bg-zinc-600 text-white"
                  } ${isStandalone ? "w-10 h-10 text-sm" : "w-5 h-5 text-[9px]"}`}
                >
                  {exp.car_class}
                </span>
              )}
              <div className="flex-1 min-w-0">
                <p
                  className={`font-semibold text-white truncate ${
                    isStandalone ? "text-lg" : "text-[11px]"
                  }`}
                >
                  {exp.name}
                </p>
                <p
                  className={`text-rp-grey truncate ${
                    isStandalone ? "text-sm" : "text-[9px]"
                  }`}
                >
                  {exp.track} &middot; {exp.car}
                </p>
              </div>
              <div className="text-right shrink-0">
                <p className={`text-rp-grey ${isStandalone ? "text-sm" : "text-[9px]"}`}>
                  {exp.duration_minutes}min
                </p>
              </div>
            </button>
          ))
        )}
      </div>
    </div>
  );
}

// ─── Waiting ────────────────────────────────────────────────────────────────

function WaitingView({
  authToken,
  isStandalone,
}: {
  authToken: AuthTokenInfo;
  isStandalone: boolean;
}) {
  return (
    <div className="flex-1 flex flex-col items-center justify-center">
      <p className={`text-rp-grey uppercase tracking-wide ${isStandalone ? "text-lg mb-4" : "text-[10px] mb-2"}`}>
        Awaiting Customer
      </p>
      {authToken.auth_type === "pin" && (
        <p
          className={`font-bold text-white tabular-nums tracking-[0.3em] ${
            isStandalone ? "text-6xl" : "text-2xl"
          }`}
        >
          {authToken.token}
        </p>
      )}
      <div
        className={`rounded-full border-2 border-rp-red border-t-transparent animate-spin mt-4 ${
          isStandalone ? "w-8 h-8" : "w-4 h-4"
        }`}
      />
      <p className={`text-rp-grey mt-3 ${isStandalone ? "text-sm" : "text-[9px]"}`}>
        {authToken.driver_name} &middot; {authToken.pricing_tier_name}
      </p>
    </div>
  );
}

// ─── Launching ──────────────────────────────────────────────────────────────

function LaunchingView({
  gameInfo,
  isStandalone,
}: {
  gameInfo?: GameLaunchInfo;
  isStandalone: boolean;
}) {
  const label = gameInfo?.sim_type
    ? GAME_LABELS[gameInfo.sim_type] || gameInfo.sim_type
    : "Game";

  return (
    <div className="flex-1 flex flex-col items-center justify-center">
      <div
        className={`rounded-full border-2 border-rp-red border-t-transparent animate-spin ${
          isStandalone ? "w-12 h-12 mb-6" : "w-6 h-6 mb-3"
        }`}
      />
      <p className={`font-semibold text-white ${isStandalone ? "text-2xl" : "text-sm"}`}>
        Launching {label}...
      </p>
    </div>
  );
}

// ─── In Session (Racing HUD) ────────────────────────────────────────────────

function InSessionView({
  billing,
  telemetry,
  gameInfo,
  isStandalone,
  onEndSession,
  warning,
}: {
  billing: BillingSession;
  telemetry?: TelemetryFrame;
  gameInfo?: GameLaunchInfo;
  isStandalone: boolean;
  onEndSession?: () => void;
  warning?: BillingWarning;
}) {
  const trackName = telemetry?.track || "";
  const carName = telemetry?.car || "";
  const hasWarning = !!warning;

  return (
    <div className={`flex-1 flex flex-col ${isStandalone ? "p-6" : "p-2"} overflow-hidden`}>
      {/* Header: track/car + driver */}
      <div className={`flex items-center justify-between ${isStandalone ? "mb-4" : "mb-1"}`}>
        <div className="min-w-0 flex-1">
          {(trackName || carName) && (
            <p
              className={`text-white font-semibold truncate ${
                isStandalone ? "text-lg" : "text-[11px]"
              }`}
            >
              {trackName}{trackName && carName ? " — " : ""}{carName}
            </p>
          )}
          <p className={`text-rp-grey truncate ${isStandalone ? "text-sm" : "text-[9px]"}`}>
            {billing.driver_name} &middot; {billing.pricing_tier_name}
          </p>
        </div>
      </div>

      {/* Timer */}
      <SessionTimer billing={billing} hasWarning={hasWarning} />

      {/* Telemetry */}
      {telemetry && (
        <div className={isStandalone ? "mt-4 flex-1 flex flex-col items-center justify-center" : "mt-2"}>
          {isStandalone ? (
            <div className="flex flex-col items-center gap-3">
              <F1Speedometer telemetry={telemetry} size={320} />
              {/* Lap info below the gauge */}
              <div className="flex items-center gap-6 text-center">
                <div>
                  <p className="text-2xl font-bold text-white tabular-nums">
                    {Math.round(telemetry.speed_kmh)}
                    <span className="text-sm text-rp-grey ml-1">km/h</span>
                  </p>
                </div>
                <div className="h-6 w-px bg-rp-border" />
                <div>
                  <p className="text-sm text-rp-grey">Lap {telemetry.lap_number}</p>
                  <p className="text-lg font-mono text-white tabular-nums">
                    {formatLapTimeShort(telemetry.lap_time_ms)}
                  </p>
                </div>
              </div>
            </div>
          ) : (
            /* Compact: speed + gear only */
            <div className="flex items-end gap-3">
              <div>
                <p className="text-lg font-bold text-white tabular-nums leading-none">
                  {Math.round(telemetry.speed_kmh)}
                </p>
                <p className="text-[8px] text-rp-grey uppercase">km/h</p>
              </div>
              <div className="text-center">
                <p className="text-base font-bold text-white leading-none">
                  {telemetry.gear === 0
                    ? "N"
                    : telemetry.gear === -1
                    ? "R"
                    : telemetry.gear}
                </p>
                <p className="text-[8px] text-rp-grey uppercase">Gear</p>
              </div>
              <div className="ml-auto text-right">
                <p className="text-[10px] text-white">Lap {telemetry.lap_number}</p>
              </div>
            </div>
          )}
        </div>
      )}

      {/* End Session button (standalone only) */}
      {isStandalone && onEndSession && (
        <button
          onClick={onEndSession}
          className="mt-auto px-6 py-3 bg-rp-red/20 border border-rp-red text-rp-red rounded-lg text-sm font-semibold hover:bg-rp-red hover:text-white transition-colors"
        >
          End Session
        </button>
      )}
    </div>
  );
}

// ─── Complete ───────────────────────────────────────────────────────────────

function CompleteView({
  billing,
  isStandalone,
}: {
  billing: BillingSession;
  isStandalone: boolean;
}) {
  const [countdown, setCountdown] = useState(15);

  useEffect(() => {
    const interval = setInterval(() => {
      setCountdown((prev) => (prev > 0 ? prev - 1 : 0));
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  const droveMins = Math.floor(billing.driving_seconds / 60);
  const droveSecs = billing.driving_seconds % 60;

  return (
    <div className="flex-1 flex flex-col items-center justify-center">
      <svg
        className={`text-green-500 ${isStandalone ? "w-16 h-16 mb-4" : "w-8 h-8 mb-2"}`}
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={2}
      >
        <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
      </svg>
      <p className={`font-bold text-white ${isStandalone ? "text-3xl" : "text-sm"}`}>
        Session Complete!
      </p>
      <p className={`text-rp-grey mt-2 ${isStandalone ? "text-lg" : "text-[10px]"}`}>
        Driving time: {droveMins}m {droveSecs}s
      </p>
      <p className={`text-rp-grey mt-1 ${isStandalone ? "text-sm" : "text-[9px]"}`}>
        Resetting in {countdown}s...
      </p>
    </div>
  );
}
