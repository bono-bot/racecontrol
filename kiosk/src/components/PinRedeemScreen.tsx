"use client";

import { useState, useEffect, useRef } from "react";
import { api } from "@/lib/api";

// PIN charset: no ambiguous I, L, O, 0, 1
const PIN_CHARS = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const AUTO_CLOSE_SUCCESS_MS = 15_000;
const AUTO_RETURN_ERROR_MS = 10_000;

// Defaults matching backend constants (PIN_REDEEM_LENGTH, etc. in routes.rs).
// Can be overridden via pinConfig prop when parent fetches /kiosk/settings.
const DEFAULT_PIN_LENGTH = 4;

type RedeemStep = "entry" | "validating" | "success" | "error" | "lockout";

/** PIN configuration from server /kiosk/settings — all optional with safe defaults. */
export interface PinConfig {
  pinLength?: number;
}

interface PinRedeemScreenProps {
  onClose: () => void;
  pinConfig?: PinConfig;
}

export default function PinRedeemScreen({ onClose, pinConfig }: PinRedeemScreenProps) {
  const PIN_LENGTH = pinConfig?.pinLength ?? DEFAULT_PIN_LENGTH;
  const [step, setStep] = useState<RedeemStep>("entry");
  const [pin, setPin] = useState("");
  const [errorMsg, setErrorMsg] = useState("");
  const [remainingAttempts, setRemainingAttempts] = useState<number | null>(null);
  const [lockoutSeconds, setLockoutSeconds] = useState(0);

  // Success data
  const [resultPodNumber, setResultPodNumber] = useState(0);
  const [resultDriverName, setResultDriverName] = useState("");
  const [resultExperience, setResultExperience] = useState("");
  const [resultTier, setResultTier] = useState("");
  const [resultSeconds, setResultSeconds] = useState(0);

  const lockoutRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // ---- Character input ----
  function handleChar(ch: string) {
    if (pin.length < PIN_LENGTH) {
      setPin((prev) => prev + ch);
    }
  }

  function handleBackspace() {
    setPin((prev) => prev.slice(0, -1));
  }

  function handleClear() {
    setPin("");
  }

  // ---- Submit ----
  async function handleSubmit() {
    if (pin.length !== PIN_LENGTH) return;
    setStep("validating");
    try {
      const res = await api.redeemPin(pin);

      // F1+F4 fix: use status field for reliable state detection instead of
      // checking lockout_remaining_seconds (which can be 0/falsy) or string matching
      if (res.status === "lockout") {
        setLockoutSeconds(res.lockout_remaining_seconds ?? 300);
        setStep("lockout");
      } else if (res.status === "pending_debit") {
        setErrorMsg(res.error ?? "Your booking is being processed.");
        setRemainingAttempts(null);
        setStep("error");
      } else if (res.error) {
        setErrorMsg(res.error);
        setRemainingAttempts(res.remaining_attempts ?? null);
        setStep("error");
      } else {
        setResultPodNumber(res.pod_number ?? 0);
        setResultDriverName(res.driver_name ?? "");
        setResultExperience(res.experience_name ?? "");
        setResultTier(res.tier_name ?? "");
        setResultSeconds(res.allocated_seconds ?? 0);
        setStep("success");
      }
    } catch (err: unknown) {
      // F2 fix: log the actual error for debugging, not just generic message
      const msg = err instanceof Error ? err.message : String(err);
      console.error("[PinRedeemScreen] submission failed:", msg);
      setErrorMsg("Network error - please try again");
      setStep("error");
    }
  }

  // ---- Auto-close success after 15s ----
  useEffect(() => {
    if (step !== "success") return;
    const timer = setTimeout(onClose, AUTO_CLOSE_SUCCESS_MS);
    return () => clearTimeout(timer);
  }, [step, onClose]);

  // ---- Auto-return to entry after error 10s ----
  useEffect(() => {
    if (step !== "error") return;
    const timer = setTimeout(() => {
      setStep("entry");
      setPin("");
    }, AUTO_RETURN_ERROR_MS);
    return () => clearTimeout(timer);
  }, [step]);

  // ---- Lockout countdown ----
  useEffect(() => {
    if (step !== "lockout") return;
    lockoutRef.current = setInterval(() => {
      setLockoutSeconds((prev) => {
        if (prev <= 1) {
          if (lockoutRef.current) clearInterval(lockoutRef.current);
          setStep("entry");
          setPin("");
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => {
      if (lockoutRef.current) clearInterval(lockoutRef.current);
    };
  }, [step]);

  const lockoutMin = Math.floor(lockoutSeconds / 60);
  const lockoutSec = lockoutSeconds % 60;
  const allocatedMinutes = Math.floor(resultSeconds / 60);
  // F4 fix: pending detection now uses error message as fallback only —
  // primary detection is via status field in handleSubmit (sets step directly)
  const isPending = errorMsg.toLowerCase().includes("being processed");

  // ---- ENTRY ----
  if (step === "entry") {
    return (
      <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#1A1A1A]">
        {/* Close button */}
        <button
          onClick={onClose}
          className="absolute top-6 right-6 w-12 h-12 rounded-full border border-[#333333] bg-[#222222] flex items-center justify-center text-[#5A5A5A] hover:text-white hover:border-[#E10600] transition-colors"
        >
          <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </button>

        <h1 className="text-3xl font-bold text-white mb-8">Enter Your Booking PIN</h1>

        {/* 6 PIN boxes */}
        <div className="flex gap-3 mb-8">
          {Array.from({ length: PIN_LENGTH }).map((_, i) => (
            <div
              key={i}
              className={`w-[60px] h-[72px] rounded-lg border-2 flex items-center justify-center transition-all ${
                i < pin.length
                  ? "border-[#E10600] bg-[#E10600]/10"
                  : i === pin.length
                  ? "border-[#E10600]/50 bg-[#222222]"
                  : "border-[#333333] bg-[#222222]"
              }`}
            >
              <span className="font-mono text-2xl font-bold text-white">
                {pin[i] || ""}
              </span>
            </div>
          ))}
        </div>

        {/* Character grid: 7 columns */}
        <div className="grid grid-cols-7 gap-2 mb-4">
          {PIN_CHARS.split("").map((ch) => (
            <button
              key={ch}
              onClick={() => handleChar(ch)}
              className="w-14 h-14 rounded-lg bg-[#222222] hover:bg-[#333333] text-white font-semibold border border-[#333333] transition-colors text-lg"
            >
              {ch}
            </button>
          ))}
          {/* Fill remaining cells in the last row: 31 chars = 4 rows of 7 + 3 chars, need backspace filling row 5 */}
          {/* Last row has 3 chars (Y, Z are in row 4... let's calculate: 31 / 7 = 4 rows + 3 extra) */}
          {/* The grid will naturally flow. After 31 chars, add Backspace (1 col) and Clear (2 cols) and Submit (3 cols) on the bottom row */}
        </div>

        {/* Action row */}
        <div className="grid grid-cols-7 gap-2 w-fit">
          <button
            onClick={handleBackspace}
            className="col-span-1 h-14 rounded-lg bg-[#222222] hover:bg-[#333333] text-[#5A5A5A] hover:text-white font-semibold border border-[#333333] transition-colors flex items-center justify-center"
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
          <button
            onClick={handleClear}
            className="col-span-2 h-14 rounded-lg bg-[#222222] hover:bg-[#333333] text-[#5A5A5A] hover:text-white font-semibold border border-[#333333] transition-colors text-sm"
          >
            Clear
          </button>
          <button
            onClick={handleSubmit}
            disabled={pin.length !== PIN_LENGTH}
            className="col-span-4 h-14 rounded-lg bg-[#E10600] hover:bg-[#FF1A1A] disabled:bg-[#333333] disabled:text-[#5A5A5A] text-white font-bold border border-transparent transition-colors text-lg"
          >
            Submit
          </button>
        </div>
      </div>
    );
  }

  // ---- VALIDATING ----
  if (step === "validating") {
    return (
      <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#1A1A1A]">
        {/* Frozen PIN boxes */}
        <div className="flex gap-3 mb-8">
          {Array.from({ length: PIN_LENGTH }).map((_, i) => (
            <div
              key={i}
              className="w-[60px] h-[72px] rounded-lg border-2 border-[#E10600] bg-[#E10600]/10 flex items-center justify-center"
            >
              <span className="font-mono text-2xl font-bold text-white">
                {pin[i] || ""}
              </span>
            </div>
          ))}
        </div>
        <div className="w-12 h-12 border-4 border-[#E10600] border-t-transparent rounded-full animate-spin mb-4" />
        <p className="text-lg text-white font-semibold">Validating PIN...</p>
      </div>
    );
  }

  // ---- SUCCESS ----
  if (step === "success") {
    return (
      <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#1A1A1A]">
        <div className="w-20 h-20 rounded-full bg-green-500/20 flex items-center justify-center mb-6">
          <svg className="w-10 h-10 text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M5 13l4 4L19 7" />
          </svg>
        </div>

        {resultDriverName && (
          <p className="text-[#5A5A5A] text-sm mb-2">Welcome, {resultDriverName}</p>
        )}
        <h2 className="text-2xl font-bold text-white mb-2">Head to Pod</h2>
        <p className="text-8xl font-bold text-[#E10600] mb-6">{resultPodNumber}</p>

        {resultExperience && (
          <p className="text-white text-lg font-semibold mb-1">{resultExperience}</p>
        )}
        {resultTier && (
          <p className="text-[#5A5A5A] text-sm mb-1">{resultTier}</p>
        )}
        {allocatedMinutes > 0 && (
          <p className="text-white font-semibold">{allocatedMinutes} minutes</p>
        )}

        <p className="text-[#5A5A5A] text-sm mt-6 animate-pulse">Your game is loading...</p>

        <button
          onClick={onClose}
          className="mt-6 px-6 py-2 border border-[#333333] rounded-lg text-[#5A5A5A] hover:text-white hover:border-[#E10600] transition-colors text-sm"
        >
          Done
        </button>
      </div>
    );
  }

  // ---- ERROR ----
  if (step === "error") {
    return (
      <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#1A1A1A]">
        <div className="w-20 h-20 rounded-full bg-red-900/30 flex items-center justify-center mb-6">
          {isPending ? (
            <svg className="w-10 h-10 text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          ) : (
            <svg className="w-10 h-10 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M6 18L18 6M6 6l12 12" />
            </svg>
          )}
        </div>

        <h2 className="text-2xl font-bold text-white mb-2">
          {isPending ? "Booking in Progress" : "Invalid PIN"}
        </h2>
        <p className="text-[#5A5A5A] text-sm text-center max-w-xs mb-2">
          {isPending
            ? "Your booking is being processed. Please try again in a minute."
            : errorMsg || "Please check your PIN and try again"}
        </p>

        {remainingAttempts !== null && !isPending && (
          <p className="text-amber-400 text-sm font-semibold mb-4">
            {remainingAttempts} attempt{remainingAttempts !== 1 ? "s" : ""} remaining
          </p>
        )}

        <button
          onClick={() => {
            setStep("entry");
            setPin("");
            setRemainingAttempts(null);
          }}
          className="px-8 py-3 bg-[#E10600] hover:bg-[#FF1A1A] text-white font-bold rounded-lg transition-colors"
        >
          Try Again
        </button>

        <button
          onClick={onClose}
          className="mt-4 text-[#5A5A5A] text-sm hover:text-white transition-colors"
        >
          Back
        </button>
      </div>
    );
  }

  // ---- LOCKOUT ----
  if (step === "lockout") {
    return (
      <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-[#1A1A1A]">
        <div className="w-20 h-20 rounded-full bg-red-900/30 flex items-center justify-center mb-6">
          <svg className="w-10 h-10 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
          </svg>
        </div>

        <h2 className="text-2xl font-bold text-white mb-4">Too Many Attempts</h2>

        <div className="text-5xl font-bold text-[#E10600] font-mono mb-4">
          {lockoutMin}:{String(lockoutSec).padStart(2, "0")}
        </div>

        <p className="text-[#5A5A5A] text-sm">Please wait before trying again</p>
      </div>
    );
  }

  return null;
}
