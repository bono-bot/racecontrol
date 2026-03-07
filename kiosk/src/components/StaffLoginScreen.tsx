"use client";

import { useState, useEffect } from "react";
import { api } from "@/lib/api";

interface StaffLoginScreenProps {
  onAuthenticated: (staffId: string, staffName: string) => void;
}

type LoginStep = "idle" | "pin_entry" | "validating" | "error";

export function StaffLoginScreen({ onAuthenticated }: StaffLoginScreenProps) {
  const [step, setStep] = useState<LoginStep>("idle");
  const [pin, setPin] = useState("");
  const [errorMsg, setErrorMsg] = useState("");

  function handleDigit(digit: string) {
    if (pin.length < 4) {
      setPin((prev) => prev + digit);
    }
  }

  function handleBackspace() {
    setPin((prev) => prev.slice(0, -1));
  }

  function handleClear() {
    setPin("");
  }

  async function handleSubmit() {
    if (pin.length !== 4) return;
    setStep("validating");
    setErrorMsg("");

    try {
      const res = await api.validateStaffPin(pin);
      if (res.error) {
        setErrorMsg(res.error);
        setStep("error");
        return;
      }
      onAuthenticated(res.staff_id || "", res.staff_name || "Staff");
    } catch {
      setErrorMsg("Network error - please try again");
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

  // ─── IDLE ─────────────────────────────────────────────────────────────
  if (step === "idle") {
    return (
      <button
        onClick={() => setStep("pin_entry")}
        className="h-screen w-screen flex flex-col items-center justify-center gap-8 bg-rp-black cursor-pointer"
      >
        <div className="text-center">
          <h1 className="text-6xl font-bold tracking-tight font-[family-name:var(--font-display)]">
            RACING<span className="text-rp-red">POINT</span>
          </h1>
          <p className="text-rp-grey text-lg mt-2 tracking-widest uppercase">
            Staff Terminal
          </p>
        </div>

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
                  d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
                />
              </svg>
            </div>
          </div>
          <p className="text-2xl text-white font-medium">Tap to Sign In</p>
          <p className="text-rp-grey text-sm">Staff PIN required</p>
        </div>
      </button>
    );
  }

  // ─── PIN ENTRY ────────────────────────────────────────────────────────
  if (step === "pin_entry") {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center bg-rp-black px-8">
        <div className="text-center mb-8">
          <h1 className="text-3xl font-bold text-white mb-2">Staff Login</h1>
          <p className="text-rp-grey">Enter your 4-digit staff PIN</p>
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
                {pin[i] ? "\u2022" : ""}
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

        <button
          onClick={() => { setStep("idle"); setPin(""); }}
          className="mt-8 text-rp-grey text-sm hover:text-white transition-colors"
        >
          Back
        </button>
      </div>
    );
  }

  // ─── VALIDATING ───────────────────────────────────────────────────────
  if (step === "validating") {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center bg-rp-black">
        <div className="w-16 h-16 border-4 border-rp-red border-t-transparent rounded-full animate-spin mb-6" />
        <p className="text-xl text-white font-semibold">Signing in...</p>
      </div>
    );
  }

  // ─── ERROR ────────────────────────────────────────────────────────────
  if (step === "error") {
    return (
      <div className="h-screen w-screen flex flex-col items-center justify-center gap-6 bg-rp-black">
        <div className="w-20 h-20 rounded-full bg-red-900/30 flex items-center justify-center">
          <svg className="w-10 h-10 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={3} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </div>
        <div className="text-center">
          <h1 className="text-3xl font-bold text-white mb-2">Access Denied</h1>
          <p className="text-rp-grey">{errorMsg || "Invalid staff PIN"}</p>
        </div>
        <button
          onClick={() => { setStep("pin_entry"); setPin(""); }}
          className="px-8 py-4 bg-rp-red hover:bg-rp-red-hover text-white font-bold text-lg rounded-lg transition-colors"
        >
          Try Again
        </button>
        <button
          onClick={() => { setStep("idle"); setPin(""); }}
          className="text-rp-grey text-sm hover:text-white transition-colors"
        >
          Back
        </button>
      </div>
    );
  }

  return null;
}
