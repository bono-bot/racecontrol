"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { api } from "@/lib/api";
import PinRedeemScreen from "@/components/PinRedeemScreen";
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

// ─── Timeouts ────────────────────────────────────────────────────────────

const INACTIVITY_MS = 60_000;
const SUCCESS_RETURN_MS = 15_000;
const ERROR_RETURN_MS = 10_000;

// ─── PIN Entry Step ──────────────────────────────────────────────────────

type PinStep = "numpad" | "validating" | "success" | "error";

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

  // PIN redeem overlay (remote booking flow)
  const [showPinRedeem, setShowPinRedeem] = useState(false);

  // PIN modal state
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);
  const [pinStep, setPinStep] = useState<PinStep>("numpad");
  const [pin, setPin] = useState("");
  const [errorMsg, setErrorMsg] = useState("");
  const [lastActivity, setLastActivity] = useState(Date.now());

  // Success data
  const [resultPodNumber, setResultPodNumber] = useState(0);
  const [resultDriverName, setResultDriverName] = useState("");
  const [resultTierName, setResultTierName] = useState("");
  const [resultAllocatedSeconds, setResultAllocatedSeconds] = useState(0);

  const touch = useCallback(() => setLastActivity(Date.now()), []);

  const closeModal = useCallback(() => {
    setSelectedPodId(null);
    setPinStep("numpad");
    setPin("");
    setErrorMsg("");
  }, []);

  // Inactivity → close modal (numpad + validating — prevents stuck modal on API hang)
  useEffect(() => {
    if (!selectedPodId || (pinStep !== "numpad" && pinStep !== "validating")) return;
    const interval = setInterval(() => {
      if (Date.now() - lastActivity > INACTIVITY_MS) {
        closeModal();
      }
    }, 5000);
    return () => clearInterval(interval);
  }, [selectedPodId, pinStep, lastActivity, closeModal]);

  // Success → auto-close modal
  useEffect(() => {
    if (pinStep !== "success") return;
    const timer = setTimeout(closeModal, SUCCESS_RETURN_MS);
    return () => clearTimeout(timer);
  }, [pinStep, closeModal]);

  // Error → back to numpad
  useEffect(() => {
    if (pinStep !== "error") return;
    const timer = setTimeout(() => {
      setPinStep("numpad");
      setPin("");
    }, ERROR_RETURN_MS);
    return () => clearTimeout(timer);
  }, [pinStep]);

  // ─── PIN handlers ──────────────────────────────────────────────────────

  function handleDigit(digit: string) {
    touch();
    if (pin.length < 4) {
      setPin((prev) => prev + digit);
    }
  }

  function handleBackspace() {
    touch();
    setPin((prev) => prev.slice(0, -1));
  }

  function handleClear() {
    touch();
    setPin("");
  }

  const handleSubmit = useCallback(async () => {
    if (pin.length !== 4 || !selectedPodId) return;
    touch();
    setPinStep("validating");
    setErrorMsg("");

    try {
      const res = await api.validateKioskPin(pin, selectedPodId);

      if (res.error) {
        setErrorMsg(res.error);
        setPinStep("error");
        return;
      }

      setResultPodNumber(res.pod_number || 0);
      setResultDriverName(res.driver_name || "Racer");
      setResultTierName(res.pricing_tier_name || "");
      setResultAllocatedSeconds(res.allocated_seconds || 0);
      setPinStep("success");
    } catch {
      setErrorMsg("Network error — please try again");
      setPinStep("error");
    }
  }, [pin, selectedPodId, touch]);

  // Auto-submit when 4 digits entered
  useEffect(() => {
    if (pin.length === 4 && pinStep === "numpad") {
      handleSubmit();
    }
  }, [pin, pinStep, handleSubmit]);

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
          <Link
            href="/staff"
            className="px-3 py-1.5 text-xs font-medium border border-rp-border rounded-lg text-rp-grey hover:text-white hover:border-rp-red transition-colors min-h-[44px] min-w-[44px] flex items-center justify-center"
          >
            Staff Login
          </Link>
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

            // ── Idle pod card (tappable) ──
            return (
              <button
                key={pod.id}
                data-testid={`pod-card-${pod.number}`}
                onClick={() => {
                  setSelectedPodId(pod.id);
                  setPinStep("numpad");
                  setPin("");
                  setErrorMsg("");
                  touch();
                }}
                className="rounded-xl border-2 border-rp-border bg-rp-card active:scale-[0.97] transition-all flex flex-col items-center justify-center gap-3 cursor-pointer focus-visible:border-rp-red"
              >
                <span className="text-5xl font-bold text-white font-[family-name:var(--font-display)]">
                  {pod.number}
                </span>
                <span className="px-3 py-1 rounded-full bg-green-500/15 text-green-400 text-xs font-semibold uppercase tracking-wider">
                  Available
                </span>
                <span className="text-rp-grey text-xs">
                  Tap to Enter PIN
                </span>
              </button>
            );
          })}
        </div>
      </main>

      {/* Bottom bar */}
      <footer className="flex items-center justify-center gap-6 py-3 border-t border-rp-border bg-rp-card">
        <Link
          href="/book"
          data-testid="book-session-btn"
          className="px-8 py-3 bg-rp-red hover:bg-rp-red-hover text-white font-semibold rounded-lg text-sm transition-colors min-h-[60px] flex items-center"
        >
          Book a Session
        </Link>
        <button
          onClick={() => setShowPinRedeem(true)}
          className="px-8 py-3 bg-[#222222] hover:bg-[#333333] text-white font-semibold rounded-lg text-sm transition-colors border border-[#333333] min-h-[60px]"
        >
          Have a PIN?
        </button>
      </footer>

      {/* ─── PIN Modal Overlay ─────────────────────────────────────────── */}
      {selectedPodId && (
        <PinModal
          podId={selectedPodId}
          podNumber={pods.get(selectedPodId)?.number || 0}
          step={pinStep}
          pin={pin}
          errorMsg={errorMsg}
          resultPodNumber={resultPodNumber}
          resultDriverName={resultDriverName}
          resultTierName={resultTierName}
          resultAllocatedSeconds={resultAllocatedSeconds}
          onDigit={handleDigit}
          onBackspace={handleBackspace}
          onClear={handleClear}
          onClose={closeModal}
          onRetry={() => {
            setPinStep("numpad");
            setPin("");
          }}
        />
      )}

      {/* ─── PIN Redeem Overlay (remote booking) ────────────────────────── */}
      {showPinRedeem && (
        <PinRedeemScreen onClose={() => setShowPinRedeem(false)} />
      )}
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

// ─── PIN Modal ──────────────────────────────────────────────────────────

function PinModal({
  podNumber,
  step,
  pin,
  errorMsg,
  resultPodNumber,
  resultDriverName,
  resultTierName,
  resultAllocatedSeconds,
  onDigit,
  onBackspace,
  onClear,
  onClose,
  onRetry,
}: {
  podId: string;
  podNumber: number;
  step: PinStep;
  pin: string;
  errorMsg: string;
  resultPodNumber: number;
  resultDriverName: string;
  resultTierName: string;
  resultAllocatedSeconds: number;
  onDigit: (d: string) => void;
  onBackspace: () => void;
  onClear: () => void;
  onClose: () => void;
  onRetry: () => void;
}) {
  const minutes = Math.floor(resultAllocatedSeconds / 60);

  return (
    <div
      data-testid="pin-modal"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm"
      onClick={(e) => {
        if (e.target === e.currentTarget && step === "numpad") onClose();
      }}
    >
      <div className="bg-rp-card border border-rp-border rounded-2xl p-8 w-full max-w-sm shadow-2xl">
        {/* ── Numpad ── */}
        {step === "numpad" && (
          <>
            <div className="text-center mb-6">
              <p className="text-rp-grey text-sm uppercase tracking-wider mb-1">
                Rig {podNumber}
              </p>
              <h2 className="text-2xl font-bold text-white">Enter Your PIN</h2>
            </div>

            {/* PIN dots */}
            <div className="flex justify-center gap-3 mb-8">
              {[0, 1, 2, 3].map((i) => (
                <div
                  key={i}
                  className={`w-14 h-16 rounded-lg border-2 flex items-center justify-center transition-all ${
                    i < pin.length
                      ? "border-rp-red bg-rp-red/10"
                      : i === pin.length
                      ? "border-rp-red/50 bg-rp-surface"
                      : "border-rp-border bg-rp-surface"
                  }`}
                >
                  <span className="text-3xl font-bold text-white font-[family-name:var(--font-mono-jb)]">
                    {pin[i] || ""}
                  </span>
                </div>
              ))}
            </div>

            {/* Numpad */}
            <div className="grid grid-cols-3 gap-2">
              {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((digit) => (
                <button
                  key={digit}
                  onClick={() => onDigit(digit)}
                  className="h-16 rounded-lg bg-rp-surface border border-rp-border text-2xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
                >
                  {digit}
                </button>
              ))}
              <button
                onClick={onClear}
                className="h-16 rounded-lg bg-rp-surface border border-rp-border text-xs font-semibold text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
              >
                Clear
              </button>
              <button
                onClick={() => onDigit("0")}
                className="h-16 rounded-lg bg-rp-surface border border-rp-border text-2xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
              >
                0
              </button>
              <button
                onClick={onBackspace}
                className="h-16 rounded-lg bg-rp-surface border border-rp-border flex items-center justify-center text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
              >
                <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2M3 12l6.414-6.414A2 2 0 0110.828 5H21a2 2 0 012 2v10a2 2 0 01-2 2H10.828a2 2 0 01-1.414-.586L3 12z"
                  />
                </svg>
              </button>
            </div>

            <button
              onClick={onClose}
              className="mt-6 w-full text-center text-rp-grey text-sm hover:text-white transition-colors"
            >
              Cancel
            </button>
          </>
        )}

        {/* ── Validating ── */}
        {step === "validating" && (
          <div className="flex flex-col items-center py-12">
            <div className="w-12 h-12 border-4 border-rp-red border-t-transparent rounded-full animate-spin mb-4" />
            <p className="text-lg text-white font-semibold">Validating PIN...</p>
            <p className="text-rp-grey text-sm mt-1">Setting up your rig</p>
          </div>
        )}

        {/* ── Success ── */}
        {step === "success" && (
          <div className="flex flex-col items-center py-6 gap-4">
            <div className="w-16 h-16 rounded-full bg-green-500/20 flex items-center justify-center">
              <svg className="w-8 h-8 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
              </svg>
            </div>
            <h2 className="text-xl font-bold text-white">
              Welcome, {resultDriverName}!
            </h2>
            <div className="bg-rp-surface border-2 border-rp-red rounded-xl p-4 text-center glow-active">
              <p className="text-xs text-rp-grey uppercase tracking-wider mb-1">Head to Rig</p>
              <p className="text-5xl font-bold text-white font-[family-name:var(--font-display)]">
                {resultPodNumber}
              </p>
            </div>
            {resultTierName && (
              <p className="text-rp-grey text-sm">{resultTierName}</p>
            )}
            {minutes > 0 && (
              <p className="text-white font-semibold">{minutes} minutes</p>
            )}
            <button
              onClick={onClose}
              className="mt-2 px-6 py-2 border border-rp-border rounded-lg text-rp-grey hover:text-white hover:border-rp-red transition-colors text-sm"
            >
              Done
            </button>
          </div>
        )}

        {/* ── Error ── */}
        {step === "error" && (
          <div className="flex flex-col items-center py-6 gap-4">
            <div className="w-16 h-16 rounded-full bg-red-900/30 flex items-center justify-center">
              <svg className="w-8 h-8 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </div>
            <h2 className="text-xl font-bold text-white">Invalid PIN</h2>
            <p className="text-rp-grey text-sm text-center">
              {errorMsg || "Please check your PIN and try again"}
            </p>
            <button
              onClick={onRetry}
              className="px-6 py-3 bg-rp-red hover:bg-rp-red-hover text-white font-bold rounded-lg transition-colors"
            >
              Try Again
            </button>
            <button
              onClick={onClose}
              className="text-rp-grey text-sm hover:text-white transition-colors"
            >
              Back
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
