"use client";
import { useState, useEffect, useCallback, FormEvent } from "react";
import { useRouter } from "next/navigation";
import { setToken, isAuthenticated } from "@/lib/auth";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

export default function LoginPage() {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const router = useRouter();

  useEffect(() => {
    if (isAuthenticated()) {
      router.push("/");
    }
  }, [router]);

  const handleSubmit = useCallback(async (pinValue?: string) => {
    const submitPin = pinValue ?? pin;
    if (submitPin.length !== 4) return;
    setError(null);
    setLoading(true);

    try {
      const res = await fetch(`${API_BASE}/api/v1/staff/validate-pin`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ pin: submitPin }),
      });

      if (res.status === 200) {
        const data = await res.json();
        if (data.status === "ok" && data.token) {
          setToken(data.token);
          router.push("/");
        } else {
          setError(data.error || "Invalid staff PIN");
          setPin("");
        }
      } else {
        setError("Invalid staff PIN");
        setPin("");
      }
    } catch {
      setError("Cannot reach server. Check your connection.");
    } finally {
      setLoading(false);
    }
  }, [pin, router]);

  function handleDigit(digit: string) {
    if (loading) return;
    setPin((prev) => {
      if (prev.length >= 4) return prev;
      const newPin = prev + digit;
      setError(null);
      if (newPin.length === 4) {
        handleSubmit(newPin);
      }
      return newPin;
    });
  }

  function handleBackspace() {
    setPin((prev) => prev.slice(0, -1));
    setError(null);
  }

  function handleClear() {
    setPin("");
    setError(null);
  }

  // Keyboard support
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key >= "0" && e.key <= "9") {
        handleDigit(e.key);
      } else if (e.key === "Backspace") {
        handleBackspace();
      } else if (e.key === "Escape") {
        handleClear();
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  return (
    <div className="min-h-screen flex items-center justify-center bg-[#1A1A1A] px-4">
      <div className="w-full max-w-sm text-center">
        {/* Header */}
        <div className="mb-8">
          <h1 className="text-2xl font-bold text-white tracking-tight">
            RaceControl
          </h1>
          <p className="text-sm text-neutral-400 mt-1">Enter your 4-digit staff PIN</p>
        </div>

        {/* PIN display boxes */}
        <div className="flex justify-center gap-4 mb-8">
          {[0, 1, 2, 3].map((i) => (
            <div
              key={i}
              className={`w-14 h-14 rounded-xl border-2 flex items-center justify-center text-2xl font-bold transition-all ${
                i < pin.length
                  ? "border-[#E10600] bg-[#E10600]/10 text-white"
                  : "border-[#333333] bg-[#222222] text-transparent"
              }`}
            >
              {i < pin.length ? "\u2022" : "0"}
            </div>
          ))}
        </div>

        {/* Error */}
        {error && (
          <p className="text-sm text-[#E10600] text-center font-medium mb-4">
            {error}
          </p>
        )}

        {/* Loading */}
        {loading && (
          <p className="text-neutral-400 text-sm animate-pulse mb-4">
            Verifying...
          </p>
        )}

        {/* Numpad */}
        <div className="grid grid-cols-3 gap-3 max-w-[280px] mx-auto">
          {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((digit) => (
            <button
              key={digit}
              onClick={() => handleDigit(digit)}
              disabled={loading || pin.length >= 4}
              className="h-16 rounded-xl bg-[#222222] border border-[#333333] text-white text-xl font-semibold
                         hover:bg-[#333333] hover:border-[#555555] active:bg-[#E10600]/20
                         disabled:opacity-30 disabled:cursor-not-allowed transition-all"
            >
              {digit}
            </button>
          ))}
          <button
            onClick={handleClear}
            disabled={loading}
            className="h-16 rounded-xl bg-[#222222] border border-[#333333] text-neutral-400 text-sm font-medium
                       hover:bg-[#333333] transition-all disabled:opacity-30"
          >
            Clear
          </button>
          <button
            onClick={() => handleDigit("0")}
            disabled={loading || pin.length >= 4}
            className="h-16 rounded-xl bg-[#222222] border border-[#333333] text-white text-xl font-semibold
                       hover:bg-[#333333] hover:border-[#555555] active:bg-[#E10600]/20
                       disabled:opacity-30 disabled:cursor-not-allowed transition-all"
          >
            0
          </button>
          <button
            onClick={handleBackspace}
            disabled={loading || pin.length === 0}
            className="h-16 rounded-xl bg-[#222222] border border-[#333333] text-neutral-400 text-lg font-medium
                       hover:bg-[#333333] transition-all disabled:opacity-30"
          >
            &#9003;
          </button>
        </div>
      </div>
    </div>
  );
}
