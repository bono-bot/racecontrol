"use client";

import { useEffect, useState, useCallback, useRef } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import type { PodReservation, BillingSession } from "@/lib/api";

export default function ActiveSessionPage() {
  const router = useRouter();
  const [reservation, setReservation] = useState<PodReservation | null>(null);
  const [billing, setBilling] = useState<BillingSession | null>(null);
  const [podNumber, setPodNumber] = useState<number>(0);
  const [loading, setLoading] = useState(true);
  const [ending, setEnding] = useState(false);
  const groupChecked = useRef(false);

  const loadState = useCallback(async () => {
    try {
      const res = await api.activeReservation();
      if (!res.reservation) {
        router.push("/book");
        return;
      }
      setReservation(res.reservation);
      setPodNumber(res.pod_number || 0);
      setBilling(res.active_billing || null);

      // Check for group session (only once)
      if (!groupChecked.current) {
        groupChecked.current = true;
        const gRes = await api.groupSession();
        if (gRes.group_session && ["forming", "ready", "active", "all_validated"].includes(gRes.group_session.status)) {
          router.push("/book/group");
          return;
        }
      }
    } catch {
      // network error — keep trying
    } finally {
      setLoading(false);
    }
  }, [router]);

  useEffect(() => {
    loadState();
    const interval = setInterval(loadState, 3000);
    return () => clearInterval(interval);
  }, [loadState]);

  async function handleEnd() {
    setEnding(true);
    try {
      await api.endReservation();
      router.push("/dashboard");
    } catch {
      setEnding(false);
    }
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!reservation) return null;

  const isSessionActive = billing && billing.status === "active";
  const remainingSeconds = billing
    ? billing.allocated_seconds - billing.driving_seconds
    : 0;

  return (
    <div className="px-4 pt-12 pb-24 max-w-lg mx-auto text-center">
      {/* Pod assignment */}
      <div className="mb-8">
        <p className="text-rp-grey text-sm mb-2">Your Pod</p>
        <div className="text-7xl font-bold text-white mb-2">{podNumber}</div>
        {!isSessionActive && (
          <p className="text-rp-grey">
            Walk to Pod {podNumber} and scan the QR code on the screen
          </p>
        )}
      </div>

      {/* Timer (when session is active) */}
      {isSessionActive && billing && (
        <div className="mb-8">
          <div className="bg-rp-card border border-rp-border rounded-2xl p-8 inline-block">
            <p className="text-rp-grey text-xs mb-2">Time Remaining</p>
            <p className="text-5xl font-mono font-bold text-white">
              {formatTime(remainingSeconds)}
            </p>
            <div className="mt-3 w-full bg-neutral-800 rounded-full h-2">
              <div
                className="bg-rp-red h-2 rounded-full transition-all"
                style={{
                  width: `${(remainingSeconds / billing.allocated_seconds) * 100}%`,
                }}
              />
            </div>
          </div>
        </div>
      )}

      {/* Status indicator */}
      <div className="mb-8">
        {isSessionActive ? (
          <div className="flex items-center justify-center gap-2">
            <div className="w-3 h-3 bg-green-500 rounded-full animate-pulse" />
            <span className="text-green-400 font-semibold">Session Active</span>
          </div>
        ) : (
          <div className="flex items-center justify-center gap-2">
            <div className="w-3 h-3 bg-amber-500 rounded-full animate-pulse" />
            <span className="text-amber-400 font-semibold">
              Waiting for QR scan
            </span>
          </div>
        )}
      </div>

      {/* Session ended — continue flow */}
      {billing && ["completed", "ended_early"].includes(billing.status) && (
        <div className="mb-8">
          <div className="bg-emerald-900/30 border border-emerald-500/30 rounded-xl p-4 mb-4">
            <p className="text-emerald-400 font-semibold">Session Complete!</p>
            <p className="text-neutral-400 text-sm mt-1">
              Want to keep racing? Pick your next session.
            </p>
          </div>
          <a
            href="/book"
            className="inline-block bg-rp-red text-white font-semibold px-8 py-3 rounded-xl"
          >
            Pick Next Race
          </a>
        </div>
      )}

      {/* End session button */}
      <button
        onClick={handleEnd}
        disabled={ending}
        className="text-rp-grey text-sm underline disabled:opacity-50"
      >
        {ending ? "Ending..." : "End Session & Leave Pod"}
      </button>
    </div>
  );
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
