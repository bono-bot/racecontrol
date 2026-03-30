"use client";

import { useState, useEffect, useCallback } from "react";
import { useRouter } from "next/navigation";
import { setToken, isAuthenticated } from "@/lib/auth";
import PinPad from "@/components/PinPad";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

export default function LoginPage() {
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [lockoutSeconds, setLockoutSeconds] = useState(0);
  const router = useRouter();

  // Redirect if already authenticated (SSR safe — in useEffect only)
  useEffect(() => {
    if (isAuthenticated()) {
      router.push("/");
    }
  }, [router]);

  // Lockout countdown timer
  useEffect(() => {
    if (lockoutSeconds <= 0) return;
    const timer = setInterval(() => {
      setLockoutSeconds((s) => {
        if (s <= 1) {
          clearInterval(timer);
          return 0;
        }
        return s - 1;
      });
    }, 1000);
    return () => clearInterval(timer);
  }, [lockoutSeconds]);

  const handleComplete = useCallback(
    async (pin: string) => {
      setError(null);
      setLoading(true);
      try {
        const res = await fetch(`${API_BASE}/api/v1/staff/validate-pin`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ pin }),
        });

        if (res.status === 429) {
          const data = await res.json().catch(() => ({}));
          const secs = data.lockout_seconds ?? 30;
          setLockoutSeconds(secs);
          setError(`Too many attempts. Try again in ${secs}s`);
          return;
        }

        if (res.status === 200) {
          const data = await res.json();
          if (data.status === "ok" && data.token) {
            setToken(data.token);
            router.push("/");
            return;
          }
          setError(data.error || "Invalid staff PIN");
          return;
        }

        setError("Invalid staff PIN");
      } catch {
        setError("Cannot reach server. Check your connection.");
      } finally {
        setLoading(false);
      }
    },
    [router]
  );

  return (
    <div
      className="min-h-screen flex items-center justify-center bg-rp-black px-4"
      style={{
        backgroundImage: `repeating-linear-gradient(
          -45deg,
          transparent,
          transparent 40px,
          rgba(225, 6, 0, 0.03) 40px,
          rgba(225, 6, 0, 0.03) 41px
        )`,
      }}
    >
      <div className="w-full max-w-xs bg-rp-card border border-rp-border rounded-2xl p-8 shadow-2xl">
        {/* Racing Red accent bar at card top */}
        <div className="h-1 bg-rp-red rounded-t-2xl -mt-8 -mx-8 mb-8" />

        {/* Wordmark */}
        <div className="text-center mb-8">
          <div className="flex items-center justify-center gap-2 mb-2">
            <svg
              className="w-6 h-6 text-rp-red"
              fill="currentColor"
              viewBox="0 0 24 24"
            >
              <path d="M3 3h9l-1.5 3H12l1.5-3H21v9l-1.5-3H18l1.5 3H3V3z" />
            </svg>
            <h1 className="text-xl font-bold text-white tracking-tight">
              RaceControl
            </h1>
          </div>
          <p className="text-xs text-rp-grey">Racing Point Bandlaguda</p>
          <p className="text-xs text-rp-grey mt-0.5">
            Enter your 6-digit staff PIN
          </p>
        </div>

        {/* PinPad component — handles keyboard, PIN display, numpad */}
        <PinPad
          onComplete={handleComplete}
          disabled={loading || lockoutSeconds > 0}
          error={
            lockoutSeconds > 0
              ? `Locked out \u2014 ${lockoutSeconds}s remaining`
              : error
          }
          loading={loading}
        />
      </div>
    </div>
  );
}
