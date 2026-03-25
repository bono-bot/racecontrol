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
import { GAME_LABELS, CLASS_COLORS } from "@/lib/constants";

function formatLapTimeShort(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = (totalSecs % 60).toFixed(3);
  return `${mins}:${parseFloat(secs) < 10 ? "0" : ""}${secs}`;
}

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
  onRelaunchGame?: () => void;
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
  onRelaunchGame,
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
          installedGames={pod.installed_games}
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
          onRelaunchGame={onRelaunchGame}
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
  installedGames,
  isStandalone,
  onSelectExperience,
}: {
  experiences: KioskExperience[];
  installedGames?: string[];
  isStandalone: boolean;
  onSelectExperience?: (id: string) => void;
}) {
  const [gameFilter, setGameFilter] = useState<string>("all");

  // Empty or missing = show all (backward compat with old agents)
  const hasGameData = installedGames && installedGames.length > 0;

  const isAvailable = (exp: KioskExperience) =>
    !hasGameData || installedGames.includes(exp.game);

  // Unique games for filter tabs
  const gameTabs = ["all", ...new Set(experiences.map((e) => e.game))];

  // Filter by selected game, then sort available-first
  const filtered = gameFilter === "all"
    ? experiences
    : experiences.filter((e) => e.game === gameFilter);

  const sorted = [...filtered].sort((a, b) => {
    const aOk = isAvailable(a) ? 0 : 1;
    const bOk = isAvailable(b) ? 0 : 1;
    return aOk - bOk || a.sort_order - b.sort_order;
  });

  return (
    <div className={`flex-1 flex flex-col ${isStandalone ? "p-8" : "p-2"} overflow-hidden`}>
      <h2
        className={`font-bold text-white uppercase tracking-wide ${
          isStandalone ? "text-2xl mb-6" : "text-xs mb-2"
        }`}
      >
        Select Experience
      </h2>

      {/* Game filter tabs */}
      {gameTabs.length > 2 && (
        <div className={`flex gap-1 flex-wrap ${isStandalone ? "mb-4" : "mb-1.5"}`}>
          {gameTabs.map((game) => (
            <button
              key={game}
              onClick={() => setGameFilter(game)}
              className={`rounded-full border transition-colors ${
                isStandalone ? "px-4 py-1.5 text-xs" : "px-2 py-0.5 text-[9px]"
              } ${
                gameFilter === game
                  ? "border-rp-red text-rp-red bg-rp-red/10"
                  : "border-rp-border text-rp-grey hover:text-white"
              }`}
            >
              {game === "all" ? "All" : GAME_LABELS[game] || game}
            </button>
          ))}
        </div>
      )}

      <div className={`flex-1 overflow-y-auto ${isStandalone ? "space-y-3" : "space-y-1"}`}>
        {sorted.length === 0 ? (
          <p className={`text-rp-grey ${isStandalone ? "text-lg" : "text-[10px]"}`}>
            No experiences available
          </p>
        ) : (
          sorted.map((exp) => {
            const available = isAvailable(exp);
            return (
              <button
                key={exp.id}
                onClick={() => available && onSelectExperience?.(exp.id)}
                disabled={!available}
                className={`w-full flex items-center gap-2 border rounded transition-colors text-left ${
                  isStandalone ? "px-5 py-4 gap-4" : "px-2 py-1.5"
                } ${
                  available
                    ? "border-rp-border hover:border-rp-red/50 bg-rp-surface cursor-pointer"
                    : "border-rp-border/30 bg-rp-surface/30 cursor-not-allowed opacity-40"
                }`}
              >
                {exp.car_class && (
                  <span
                    className={`flex items-center justify-center rounded font-bold ${
                      available
                        ? CLASS_COLORS[exp.car_class] || "bg-zinc-600 text-white"
                        : "bg-zinc-800 text-zinc-500"
                    } ${isStandalone ? "w-10 h-10 text-sm" : "w-5 h-5 text-[9px]"}`}
                  >
                    {exp.car_class}
                  </span>
                )}
                <div className="flex-1 min-w-0">
                  <p
                    className={`font-semibold truncate ${
                      available ? "text-white" : "text-zinc-600"
                    } ${isStandalone ? "text-lg" : "text-[11px]"}`}
                  >
                    {exp.name}
                  </p>
                  <p
                    className={`truncate ${
                      available ? "text-rp-grey" : "text-zinc-700"
                    } ${isStandalone ? "text-sm" : "text-[9px]"}`}
                  >
                    {exp.track} &middot; {exp.car}
                  </p>
                </div>
                <div className="text-right shrink-0">
                  {available ? (
                    <>
                      <p className={`text-rp-grey ${isStandalone ? "text-sm" : "text-[9px]"}`}>
                        {exp.duration_minutes}min
                      </p>
                      <p className={`text-rp-grey capitalize ${isStandalone ? "text-xs" : "text-[8px]"}`}>
                        {exp.start_type}
                      </p>
                    </>
                  ) : (
                    <p className={`text-zinc-600 italic ${isStandalone ? "text-xs" : "text-[8px]"}`}>
                      Not installed
                    </p>
                  )}
                </div>
              </button>
            );
          })
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
  onRelaunchGame,
  warning,
}: {
  billing: BillingSession;
  telemetry?: TelemetryFrame;
  gameInfo?: GameLaunchInfo;
  isStandalone: boolean;
  onEndSession?: () => void;
  onRelaunchGame?: () => void;
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

      {/* Game Crashed Banner */}
      {gameInfo?.game_state === "error" && (
        <div className={`bg-red-900/30 border border-red-600/50 rounded-xl text-center ${
          isStandalone ? "px-6 py-5 mt-4" : "px-3 py-2 mt-2"
        }`}>
          <span className={`text-red-400 font-bold uppercase tracking-wider ${
            isStandalone ? "text-lg" : "text-xs"
          }`}>
            Game Crashed
          </span>
          {gameInfo.error_message && (
            <p className={`text-red-400/70 mt-1 ${isStandalone ? "text-sm" : "text-[9px]"}`}>
              {gameInfo.error_message}
            </p>
          )}
          {onRelaunchGame && (
            <button
              onClick={onRelaunchGame}
              className={`mt-3 w-full bg-red-600 hover:bg-red-500 text-white font-semibold rounded-xl transition-colors ${
                isStandalone ? "py-3 text-base" : "py-1.5 text-xs"
              }`}
            >
              Relaunch Game
            </button>
          )}
        </div>
      )}

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
