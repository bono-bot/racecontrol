"use client";

import { useState, useEffect, useCallback } from "react";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { api } from "@/lib/api";

// ─── Step Definitions ─────────────────────────────────────────────────────

type Step = "idle" | "pin_entry" | "validating" | "success" | "error";

// ─── Timeouts ─────────────────────────────────────────────────────────────

const INACTIVITY_MS = 60_000;
const SUCCESS_RETURN_MS = 15_000;
const ERROR_RETURN_MS = 10_000;

// ─── Main Walk-In Page (PIN Entry Terminal) ───────────────────────────────

export default function WalkInPage() {
  const { pods, connected } = useKioskSocket();

  const [step, setStep] = useState<Step>("idle");
  const [pin, setPin] = useState("");
  const [errorMsg, setErrorMsg] = useState("");
  const [lastActivity, setLastActivity] = useState(Date.now());

  // Success data
  const [resultPodNumber, setResultPodNumber] = useState(0);
  const [resultDriverName, setResultDriverName] = useState("");
  const [resultTierName, setResultTierName] = useState("");
  const [resultAllocatedSeconds, setResultAllocatedSeconds] = useState(0);

  const touch = useCallback(() => setLastActivity(Date.now()), []);

  const reset = useCallback(() => {
    setStep("idle");
    setPin("");
    setErrorMsg("");
    setResultPodNumber(0);
    setResultDriverName("");
    setResultTierName("");
    setResultAllocatedSeconds(0);
  }, []);

  // Inactivity → back to idle (only during pin_entry)
  useEffect(() => {
    if (step !== "pin_entry") return;
    const interval = setInterval(() => {
      if (Date.now() - lastActivity > INACTIVITY_MS) {
        reset();
      }
    }, 5000);
    return () => clearInterval(interval);
  }, [step, lastActivity, reset]);

  // Success screen → auto-return to idle
  useEffect(() => {
    if (step !== "success") return;
    const timer = setTimeout(reset, SUCCESS_RETURN_MS);
    return () => clearTimeout(timer);
  }, [step, reset]);

  // Error screen → auto-return to pin_entry
  useEffect(() => {
    if (step !== "error") return;
    const timer = setTimeout(() => {
      setStep("pin_entry");
      setPin("");
    }, ERROR_RETURN_MS);
    return () => clearTimeout(timer);
  }, [step]);

  // ─── PIN digit handlers ─────────────────────────────────────────────────

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

  // ─── Submit PIN ─────────────────────────────────────────────────────────

  async function handleSubmit() {
    if (pin.length !== 4) return;
    touch();
    setStep("validating");
    setErrorMsg("");

    try {
      const res = await api.validateKioskPin(pin);

      if (res.error) {
        setErrorMsg(res.error);
        setStep("error");
        return;
      }

      setResultPodNumber(res.pod_number || 0);
      setResultDriverName(res.driver_name || "Racer");
      setResultTierName(res.pricing_tier_name || "");
      setResultAllocatedSeconds(res.allocated_seconds || 0);
      setStep("success");
    } catch {
      setErrorMsg("Network error — please try again");
      setStep("error");
    }
  }

  // Auto-submit when 4 digits entered
  useEffect(() => {
    if (pin.length === 4 && step === "pin_entry") {
      handleSubmit();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pin]);

  // ─── Idle pod count ─────────────────────────────────────────────────────

  const idlePods = Array.from(pods.values())
    .filter((p) => p.status === "idle")
    .sort((a, b) => a.number - b.number);

  // ─── Touch wrapper ──────────────────────────────────────────────────────

  const wrap = (content: React.ReactNode) => (
    <div className="h-screen w-screen overflow-hidden" onClick={touch}>
      {content}
    </div>
  );

  // ═══════════════════════════════════════════════════════════════════════
  // STEP: IDLE — "Tap to Enter PIN"
  // ═══════════════════════════════════════════════════════════════════════
  if (step === "idle") {
    return wrap(
      <button
        onClick={() => {
          touch();
          setStep("pin_entry");
        }}
        className="h-full w-full flex flex-col items-center justify-center gap-8 bg-rp-black cursor-pointer"
      >
        {/* Logo / Brand */}
        <div className="text-center">
          <h1 className="text-6xl font-bold tracking-tight font-[family-name:var(--font-display)]">
            RACING<span className="text-rp-red">POINT</span>
          </h1>
          <p className="text-rp-grey text-lg mt-2 tracking-widest uppercase">
            May the Fastest Win
          </p>
        </div>

        {/* Animated tap prompt */}
        <div className="flex flex-col items-center gap-4 mt-8">
          <div className="w-20 h-20 rounded-full border-2 border-rp-red/50 flex items-center justify-center animate-pulse">
            <div className="w-12 h-12 rounded-full bg-rp-red/20 flex items-center justify-center">
              <svg
                className="w-6 h-6 text-rp-red"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"
                />
              </svg>
            </div>
          </div>
          <p className="text-2xl text-white font-medium">Tap to Enter PIN</p>
          <p className="text-rp-grey text-sm">
            Book on the app, then enter your PIN here
          </p>
        </div>

        {/* Connection dot */}
        <div className="absolute bottom-6 flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full ${connected ? "bg-rp-green pulse-dot" : "bg-rp-grey"}`}
          />
          <span className="text-xs text-rp-grey">
            {connected ? "Connected" : "Connecting..."}
          </span>
        </div>
      </button>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // STEP: PIN ENTRY — Numpad
  // ═══════════════════════════════════════════════════════════════════════
  if (step === "pin_entry") {
    return wrap(
      <div className="h-full flex flex-col items-center justify-center bg-rp-black px-8">
        {/* Header */}
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-white mb-2">Enter Your PIN</h1>
          <p className="text-rp-grey">4-digit PIN from the Racing Point app</p>
        </div>

        {/* PIN display boxes */}
        <div className="flex gap-4 mb-10">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className={`w-20 h-24 rounded-xl border-2 flex items-center justify-center transition-all ${
                i < pin.length
                  ? "border-rp-red bg-rp-red/10"
                  : i === pin.length
                  ? "border-rp-red/50 bg-rp-surface"
                  : "border-rp-border bg-rp-surface"
              }`}
            >
              <span className="text-5xl font-bold text-white font-[family-name:var(--font-mono-jb)]">
                {pin[i] || ""}
              </span>
            </div>
          ))}
        </div>

        {/* Numpad */}
        <div className="grid grid-cols-3 gap-3 w-full max-w-sm">
          {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((digit) => (
            <button
              key={digit}
              onClick={() => handleDigit(digit)}
              className="h-20 rounded-xl bg-rp-surface border border-rp-border text-3xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
            >
              {digit}
            </button>
          ))}
          {/* Bottom row: Clear, 0, Backspace */}
          <button
            onClick={handleClear}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border text-sm font-semibold text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
          >
            Clear
          </button>
          <button
            onClick={() => handleDigit("0")}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border text-3xl font-bold text-white hover:bg-rp-red/10 hover:border-rp-red/50 active:bg-rp-red/20 transition-colors"
          >
            0
          </button>
          <button
            onClick={handleBackspace}
            className="h-20 rounded-xl bg-rp-surface border border-rp-border flex items-center justify-center text-rp-grey hover:text-white hover:border-rp-red/50 transition-colors"
          >
            <svg className="w-7 h-7" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                strokeWidth={2}
                d="M12 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2M3 12l6.414-6.414A2 2 0 0110.828 5H21a2 2 0 012 2v10a2 2 0 01-2 2H10.828a2 2 0 01-1.414-.586L3 12z"
              />
            </svg>
          </button>
        </div>

        {/* Back button */}
        <button
          onClick={reset}
          className="mt-8 text-rp-grey text-sm hover:text-white transition-colors"
        >
          Back
        </button>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // STEP: VALIDATING — Spinner
  // ═══════════════════════════════════════════════════════════════════════
  if (step === "validating") {
    return wrap(
      <div className="h-full flex flex-col items-center justify-center bg-rp-black">
        <div className="w-16 h-16 border-4 border-rp-red border-t-transparent rounded-full animate-spin mb-6" />
        <p className="text-xl text-white font-semibold">Validating PIN...</p>
        <p className="text-rp-grey text-sm mt-2">Setting up your rig</p>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // STEP: SUCCESS — "Go to Rig #X"
  // ═══════════════════════════════════════════════════════════════════════
  if (step === "success") {
    const minutes = Math.floor(resultAllocatedSeconds / 60);

    return wrap(
      <div className="h-full flex flex-col items-center justify-center gap-8 bg-rp-black">
        {/* Checkmark */}
        <div className="w-20 h-20 rounded-full bg-rp-green/20 flex items-center justify-center">
          <svg
            className="w-10 h-10 text-rp-green"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={3}
              d="M5 13l4 4L19 7"
            />
          </svg>
        </div>

        <div className="text-center">
          <h1 className="text-3xl font-bold text-white">
            You&apos;re all set, {resultDriverName}!
          </h1>
          <p className="text-rp-grey text-lg mt-2">Head to your assigned rig</p>
        </div>

        {/* Pod number — big */}
        <div className="bg-rp-surface border-2 border-rp-red rounded-2xl p-8 text-center glow-active">
          <p className="text-sm text-rp-grey uppercase tracking-wider mb-2">
            Go to Rig
          </p>
          <p className="text-8xl font-bold text-white font-[family-name:var(--font-display)]">
            {resultPodNumber}
          </p>
        </div>

        {/* Session info */}
        <div className="text-center space-y-1">
          {resultTierName && (
            <p className="text-rp-grey text-sm">{resultTierName}</p>
          )}
          {minutes > 0 && (
            <p className="text-white font-semibold">{minutes} minutes</p>
          )}
        </div>

        {/* Auto-return notice */}
        <p className="text-rp-grey text-sm mt-4">
          This screen will reset automatically
        </p>

        <button
          onClick={reset}
          className="px-8 py-3 border border-rp-border rounded-lg text-rp-grey hover:text-white hover:border-rp-red transition-colors"
        >
          Done
        </button>
      </div>
    );
  }

  // ═══════════════════════════════════════════════════════════════════════
  // STEP: ERROR — "Invalid PIN"
  // ═══════════════════════════════════════════════════════════════════════
  if (step === "error") {
    return wrap(
      <div className="h-full flex flex-col items-center justify-center gap-6 bg-rp-black">
        {/* Error icon */}
        <div className="w-20 h-20 rounded-full bg-red-900/30 flex items-center justify-center">
          <svg
            className="w-10 h-10 text-red-400"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={3}
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </div>

        <div className="text-center">
          <h1 className="text-3xl font-bold text-white mb-2">Invalid PIN</h1>
          <p className="text-rp-grey">{errorMsg || "Please check your PIN and try again"}</p>
        </div>

        <button
          onClick={() => {
            setStep("pin_entry");
            setPin("");
          }}
          className="px-8 py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-lg transition-colors"
        >
          Try Again
        </button>

        <button
          onClick={reset}
          className="text-rp-grey text-sm hover:text-white transition-colors"
        >
          Back to Home
        </button>
      </div>
    );
  }

  return null;
}
