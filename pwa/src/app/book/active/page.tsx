"use client";

import { useEffect, useState, useCallback, useRef } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { PodReservation, BillingSession } from "@/lib/api";

export default function ActiveSessionPage() {
  const router = useRouter();
  const [reservation, setReservation] = useState<PodReservation | null>(null);
  const [billing, setBilling] = useState<BillingSession | null>(null);
  const [podNumber, setPodNumber] = useState<number>(0);
  const [loading, setLoading] = useState(true);
  const [ending, setEnding] = useState(false);
  const groupChecked = useRef(false);

  // Mid-session controls state
  const [sheetOpen, setSheetOpen] = useState(false);
  const [absOn, setAbsOn] = useState(true);
  const [tcOn, setTcOn] = useState(true);
  const [autoTrans, setAutoTrans] = useState(true);
  const [ffbPercent, setFfbPercent] = useState(70);
  const [confirmMsg, setConfirmMsg] = useState<string | null>(null);
  const ffbTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    loadState();
    const interval = setInterval(loadState, 3000);
    return () => clearInterval(interval);
  }, [loadState, router]);

  // Cleanup FFB debounce timer on unmount
  useEffect(() => {
    return () => {
      if (ffbTimerRef.current) clearTimeout(ffbTimerRef.current);
    };
  }, []);

  // Mid-session controls handlers
  async function openSheet() {
    setSheetOpen(true);
    if (!reservation) return;
    try {
      const state = await api.getAssistState(reservation.pod_id);
      if (state.abs !== undefined) setAbsOn(state.abs > 0);
      if (state.tc !== undefined) setTcOn(state.tc > 0);
      if (state.auto_shifter !== undefined) setAutoTrans(state.auto_shifter);
      if (state.ffb_percent !== undefined) setFfbPercent(state.ffb_percent);
    } catch {
      // Use cached values if query fails
    }
  }

  async function toggleAssist(type: "abs" | "tc" | "transmission") {
    if (!reservation) return;
    const newValue = type === "abs" ? !absOn : type === "tc" ? !tcOn : !autoTrans;

    // Update local state immediately for responsive feel
    if (type === "abs") setAbsOn(newValue);
    else if (type === "tc") setTcOn(newValue);
    else setAutoTrans(newValue);

    try {
      await api.setAssist(reservation.pod_id, type, newValue);
      showConfirm(
        type === "abs"
          ? `ABS: ${newValue ? "ON" : "OFF"}`
          : type === "tc"
            ? `TC: ${newValue ? "ON" : "OFF"}`
            : `Transmission: ${newValue ? "AUTO" : "MANUAL"}`
      );
    } catch {
      // Revert on failure
      if (type === "abs") setAbsOn(!newValue);
      else if (type === "tc") setTcOn(!newValue);
      else setAutoTrans(!newValue);
    }
  }

  function handleFfbChange(value: number) {
    setFfbPercent(value); // Update slider position immediately (visual)

    // Debounce the actual API call by 500ms
    if (ffbTimerRef.current) clearTimeout(ffbTimerRef.current);
    ffbTimerRef.current = setTimeout(async () => {
      if (!reservation) return;
      try {
        await api.setFfbGain(reservation.pod_id, value);
        showConfirm(`FFB: ${value}%`);
      } catch {
        // Slider already shows the value; no revert needed
      }
    }, 500);
  }

  function showConfirm(msg: string) {
    setConfirmMsg(msg);
    setTimeout(() => setConfirmMsg(null), 3000);
  }

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
            Enter your PIN at the kiosk terminal
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
              Waiting for PIN entry
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

      {/* Gear icon — mid-session controls trigger */}
      {isSessionActive && (
        <button
          onClick={openSheet}
          className="fixed bottom-24 right-4 w-12 h-12 bg-rp-card border border-rp-border rounded-full flex items-center justify-center shadow-lg z-40"
          aria-label="Session Controls"
        >
          <svg className="w-6 h-6 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
        </button>
      )}

      {/* Bottom sheet backdrop */}
      {sheetOpen && (
        <div className="fixed inset-0 bg-black/50 z-40" onClick={() => setSheetOpen(false)} />
      )}

      {/* Bottom sheet — mid-session controls */}
      <div
        className={`fixed inset-x-0 bottom-0 z-50 transform transition-transform duration-300 ${
          sheetOpen ? "translate-y-0" : "translate-y-full"
        }`}
      >
        <div className="bg-[#222222] border-t border-[#333333] rounded-t-2xl px-6 pt-3 pb-8 max-w-lg mx-auto">
          {/* Drag handle */}
          <div className="w-12 h-1 bg-gray-500/50 rounded-full mx-auto mb-6" />

          <h3 className="text-white font-semibold text-lg mb-4">Session Controls</h3>

          {/* Inline confirmation */}
          {confirmMsg && (
            <div className="mb-4 px-3 py-2 bg-[#E10600]/20 border border-[#E10600]/40 rounded-lg text-center text-sm text-white">
              {confirmMsg}
            </div>
          )}

          {/* ABS Toggle */}
          <div className="flex items-center justify-between py-3 border-b border-[#333333]">
            <span className="text-white">ABS</span>
            <button
              onClick={() => toggleAssist("abs")}
              className={`w-12 h-7 rounded-full transition-colors ${absOn ? "bg-[#E10600]" : "bg-gray-600"}`}
            >
              <div
                className={`w-5 h-5 bg-white rounded-full transform transition-transform mx-1 ${
                  absOn ? "translate-x-5" : "translate-x-0"
                }`}
              />
            </button>
          </div>

          {/* TC Toggle */}
          <div className="flex items-center justify-between py-3 border-b border-[#333333]">
            <span className="text-white">Traction Control</span>
            <button
              onClick={() => toggleAssist("tc")}
              className={`w-12 h-7 rounded-full transition-colors ${tcOn ? "bg-[#E10600]" : "bg-gray-600"}`}
            >
              <div
                className={`w-5 h-5 bg-white rounded-full transform transition-transform mx-1 ${
                  tcOn ? "translate-x-5" : "translate-x-0"
                }`}
              />
            </button>
          </div>

          {/* Transmission Toggle */}
          <div className="flex items-center justify-between py-3 border-b border-[#333333]">
            <span className="text-white">Auto Transmission</span>
            <button
              onClick={() => toggleAssist("transmission")}
              className={`w-12 h-7 rounded-full transition-colors ${autoTrans ? "bg-[#E10600]" : "bg-gray-600"}`}
            >
              <div
                className={`w-5 h-5 bg-white rounded-full transform transition-transform mx-1 ${
                  autoTrans ? "translate-x-5" : "translate-x-0"
                }`}
              />
            </button>
          </div>

          {/* FFB Slider */}
          <div className="pt-4">
            <div className="flex items-center justify-between mb-2">
              <span className="text-white">Force Feedback</span>
              <span className="text-white font-mono text-sm">{ffbPercent}%</span>
            </div>
            <input
              type="range"
              min={10}
              max={100}
              step={1}
              value={ffbPercent}
              onChange={(e) => handleFfbChange(Number(e.target.value))}
              className="w-full h-2 bg-gray-700 rounded-lg appearance-none cursor-pointer accent-[#E10600]"
            />
            <div className="flex justify-between text-xs text-gray-500 mt-1">
              <span>10%</span>
              <span>100%</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}
