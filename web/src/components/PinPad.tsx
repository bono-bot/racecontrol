"use client";

import { useState, useEffect, useCallback } from "react";

interface PinPadProps {
  onComplete: (pin: string) => void;
  onReset?: () => void;
  disabled?: boolean;
  error?: string | null;
  loading?: boolean;
  digits?: number;
}

export default function PinPad({
  onComplete,
  onReset,
  disabled = false,
  error = null,
  loading = false,
  digits = 6,
}: PinPadProps) {
  const [pin, setPin] = useState("");

  const addDigit = useCallback(
    (d: string) => {
      if (disabled || loading) return;
      setPin((prev) => {
        if (prev.length >= digits) return prev;
        return prev + d;
      });
    },
    [disabled, loading, digits]
  );

  const removeDigit = useCallback(() => {
    if (disabled || loading) return;
    setPin((prev) => prev.slice(0, -1));
  }, [disabled, loading]);

  const clearAll = useCallback(() => {
    if (disabled || loading) return;
    setPin("");
    onReset?.();
  }, [disabled, loading, onReset]);

  // Auto-submit when all digits entered
  useEffect(() => {
    if (pin.length === digits) {
      onComplete(pin);
      // Reset internal state after submit
      const timer = setTimeout(() => setPin(""), 100);
      return () => clearTimeout(timer);
    }
  }, [pin, digits, onComplete]);

  // Keyboard handler
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (disabled || loading) return;
      if (e.key >= "0" && e.key <= "9") {
        addDigit(e.key);
      } else if (e.key === "Backspace") {
        removeDigit();
      } else if (e.key === "Escape") {
        clearAll();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [disabled, loading, addDigit, removeDigit, clearAll]);

  const buttons = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "CLR", "0", "\u232B"];

  return (
    <div className="flex flex-col items-center" role="group" aria-label="PIN entry">
      {/* PIN display */}
      <div className="flex justify-center gap-3 mb-6" aria-label={`${pin.length} of ${digits} digits entered`}>
        {Array.from({ length: digits }).map((_, i) => (
          <div
            key={i}
            className={`w-12 h-12 rounded-xl border-2 flex items-center justify-center text-2xl font-bold transition-all ${
              i < pin.length
                ? "border-rp-red bg-rp-red/10 text-white"
                : "border-rp-border bg-rp-card text-transparent"
            }`}
          >
            {i < pin.length ? "\u2022" : "\u2022"}
          </div>
        ))}
      </div>

      {/* Error / Loading feedback */}
      <div className="min-h-[20px] mb-4" id="pin-status" role="status" aria-live="polite">
        {loading && (
          <span className="text-neutral-400 text-sm animate-pulse text-center block">
            Verifying...
          </span>
        )}
        {error && !loading && (
          <span className="text-sm text-rp-red text-center font-medium block" role="alert">
            {error}
          </span>
        )}
      </div>

      {/* Numpad grid */}
      <div className="grid grid-cols-3 gap-3 max-w-[240px] mx-auto">
        {buttons.map((label) => (
          <button
            key={label}
            type="button"
            disabled={disabled || loading}
            onClick={() => {
              if (label === "CLR") clearAll();
              else if (label === "\u232B") removeDigit();
              else addDigit(label);
            }}
            className="h-14 rounded-xl bg-rp-card border border-rp-border text-white text-xl font-semibold hover:bg-rp-surface hover:border-rp-grey active:bg-rp-red/20 transition-all disabled:opacity-30"
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
