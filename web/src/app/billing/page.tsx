"use client";

// ─── BILLING MODULE SCOPE ──────────────────────────────────────────────────
// Billing is billing + customer registration ONLY.
// DO NOT add links/buttons to flows that include game-launch UI.
//
// ALLOWED: pod occupancy, active-session cards (driver/tier/countdown/elapsed),
//          pause/resume/end/extend/cancel controls, pending auth token +
//          Cancel Assignment, wallet top-up, customer registration modal
//          (walk-in form only, no game picker).
// FORBIDDEN: game launch / game picker / "Launch Game" buttons → kiosk :3300/staff
//            telemetry / lap times → /telemetry
//            pod diagnostics → /fleet
//            driver CRUD → /drivers read-only; edits → admin :3201
//
// Before adding any link, trace the destination FLOW end-to-end. A URL that
// *sounds* in-scope can route staff INTO an out-of-scope flow (e.g. Setup
// Wizard includes game picker). The scope is the flow the user experiences,
// not the string of the URL.
// ───────────────────────────────────────────────────────────────────────────

import { useEffect, useState } from "react";
import { AlertTriangle, Server } from "lucide-react";
import DashboardLayout from "@/components/DashboardLayout";
import CountdownTimer from "@/components/CountdownTimer";
import WalletTopupModal from "@/components/WalletTopupModal";
import StatusBadge from "@/components/StatusBadge";
import { Skeleton, EmptyState } from "@/components/Skeleton";
import { useToast } from "@/components/Toast";
import { useWebSocket } from "@/hooks/useWebSocket";
import { racingWsPodsOnly } from "@/lib/api";
import type { Pod } from "@/lib/api";

export default function BillingPage() {
  const { pods, billingTimers, billingWarnings, pendingAuthTokens, sendCommand, pendingCommands } = useWebSocket();
  const [showTopup, setShowTopup] = useState(false);
  const [initialising, setInitialising] = useState(true);
  // Tracks per-session/per-token "in flight" so individual buttons can disable
  // without blocking the whole page. Keys: session_id or token_id.
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
  const { toast } = useToast();

  // Flip initialising to false after 200ms (P2-C: reduced from 800ms)
  useEffect(() => {
    const timer = setTimeout(() => setInitialising(false), 200);
    return () => clearTimeout(timer);
  }, []);

  // POS is monitoring-only — billing starts from Staff Kiosk (:3300/staff)

  function markBusy(id: string, busy: boolean) {
    setBusyIds((prev) => {
      const next = new Set(prev);
      if (busy) next.add(id);
      else next.delete(id);
      return next;
    });
  }

  // B1 STRUCTURAL FIX: handlers are async and toast based on the REAL server
  // outcome (resolves via billing_session_changed / auth_token_cleared ack)
  // instead of the previous optimistic fire-and-forget + unconditional success
  // toast. Any server rejection now surfaces as an error toast with the real
  // reason. Timeout (5s) also produces an explicit error, not false success.

  async function handleCancelToken(tokenId: string) {
    if (busyIds.has(tokenId)) return;
    markBusy(tokenId, true);
    try {
      await sendCommand("cancel_assignment", { token_id: tokenId });
      toast({ message: "Assignment cancelled", type: "success" });
    } catch (e) {
      toast({ message: `Cancel failed: ${(e as Error).message}`, type: "error" });
    } finally {
      markBusy(tokenId, false);
    }
  }

  async function handlePauseResume(sessionId: string, isPaused: boolean) {
    if (busyIds.has(sessionId)) return;
    markBusy(sessionId, true);
    try {
      const cmd = isPaused ? "resume_billing" : "pause_billing";
      // Field name MUST be billing_session_id per DashboardCommand schema
      // (rc-common/src/protocol.rs:1649-1664). Sending "session_id" caused
      // silent parse failure at the WS handler for months — fake success
      // toasts while server never received the command.
      await sendCommand(cmd, { billing_session_id: sessionId });
      toast({ message: isPaused ? "Session resumed" : "Session paused", type: "success" });
    } catch (e) {
      const verb = isPaused ? "Resume" : "Pause";
      toast({ message: `${verb} failed: ${(e as Error).message}`, type: "error" });
    } finally {
      markBusy(sessionId, false);
    }
  }

  async function handleEnd(sessionId: string) {
    if (busyIds.has(sessionId)) return;
    markBusy(sessionId, true);
    try {
      await sendCommand("end_billing", { billing_session_id: sessionId });
      toast({ message: "Session ended", type: "success" });
    } catch (e) {
      toast({ message: `End failed: ${(e as Error).message}`, type: "error" });
    } finally {
      markBusy(sessionId, false);
    }
  }

  async function handleExtend(sessionId: string) {
    if (busyIds.has(sessionId)) return;
    markBusy(sessionId, true);
    try {
      await sendCommand("extend_billing", {
        billing_session_id: sessionId,
        additional_seconds: 600,
      });
      toast({ message: "+10 minutes added", type: "success" });
    } catch (e) {
      toast({ message: `Extend failed: ${(e as Error).message}`, type: "error" });
    } finally {
      markBusy(sessionId, false);
    }
  }

  const sortedPods = racingWsPodsOnly([...pods]).sort((a, b) => a.number - b.number);

  // Count paused sessions
  const pausedCount = Array.from(billingTimers.values()).filter(
    (b) =>
      b.status === "paused_manual" ||
      b.status === "paused_disconnect" ||
      b.status === "paused_game_pause"
  ).length;

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Billing</h1>
          <p className="text-sm text-rp-grey">Session management</p>
        </div>
        <div className="flex gap-3 items-center">
          <button
            onClick={() => setShowTopup(true)}
            className="px-4 py-1.5 bg-green-600 hover:bg-green-700 text-white text-xs font-semibold rounded-lg transition-colors"
          >
            Top Up Wallet
          </button>
          <span className="bg-rp-card border border-rp-border rounded-lg px-3 py-1.5 text-xs text-neutral-300">
            {billingTimers.size} active
          </span>
          <span className="bg-rp-card border border-rp-border rounded-lg px-3 py-1.5 text-xs text-neutral-300">
            {pausedCount} paused
          </span>
        </div>
      </div>

      {/* Billing Warnings */}
      {billingWarnings.length > 0 && (
        <div className="mb-4 space-y-2">
          {billingWarnings.map((w, i) => {
            const pod = pods.find((p) => p.id === w.podId);
            const podLabel = pod
              ? `Pod ${String(pod.number).padStart(2, "0")}`
              : w.podId;
            const mins = Math.floor(w.remaining / 60);
            const secs = w.remaining % 60;
            return (
              <div
                key={`${w.sessionId}-${i}`}
                className="bg-amber-500/10 border border-amber-500/30 rounded-lg px-4 py-3 flex items-center gap-3"
              >
                <AlertTriangle className="w-4 h-4 text-amber-400 shrink-0" />
                <span className="text-sm text-amber-200">
                  <span className="font-semibold">{podLabel}</span> &mdash;{" "}
                  {mins > 0
                    ? `${mins}m ${secs}s remaining`
                    : `${secs}s remaining`}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {/* Loading skeleton */}
      {initialising && pods.length === 0 ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <Skeleton key={i} className="h-40 rounded-lg" />
          ))}
        </div>
      ) : sortedPods.length === 0 ? (
        /* Empty state */
        <div className="border border-rp-border rounded-lg">
          <EmptyState
            icon={<Server className="w-10 h-10" />}
            headline="No pods connected"
            hint="Pods appear automatically when rc-agent connects from a sim PC."
          />
        </div>
      ) : (
        /* Pod Grid */
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {sortedPods.map((pod) => {
            const billing = billingTimers.get(pod.id);
            const pendingToken = pendingAuthTokens.get(pod.id);
            const isPaused =
              billing?.status === "paused_manual" ||
              billing?.status === "paused_disconnect" ||
              billing?.status === "paused_game_pause";
            const isWaitingForGame = billing?.status === "waiting_for_game";

            return (
              <div
                key={pod.id}
                className={`rounded-lg border p-4 transition-all ${
                  billing
                    ? "border-rp-red/50 bg-rp-red/5"
                    : pendingToken
                    ? "border-yellow-500/50 bg-yellow-500/5"
                    : pod.status === "idle"
                    ? "border-rp-border bg-rp-card"
                    : pod.status === "offline"
                    ? "border-rp-border bg-rp-card/50"
                    : "border-rp-border bg-rp-card"
                }`}
              >
                {/* Pod header */}
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <span className="text-xl font-bold text-neutral-300">
                      {String(pod.number).padStart(2, "0")}
                    </span>
                    <span className="text-sm text-rp-grey">{pod.name}</span>
                  </div>
                  {pendingToken ? (
                    <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-yellow-500/20 text-yellow-400 animate-pulse">
                      WAITING
                    </span>
                  ) : (
                    <StatusBadge status={pod.status} />
                  )}
                </div>

                {billing ? (
                  <div className="space-y-3">
                    {/* Countdown */}
                    <CountdownTimer
                      remaining={billing.remaining_seconds}
                      allocated={billing.allocated_seconds}
                      drivingState={billing.driving_state}
                    />

                    {/* Driver & tier info */}
                    <div className="space-y-1 text-xs">
                      <div className="flex justify-between">
                        <span className="text-rp-grey">Driver</span>
                        <span className="text-rp-red">
                          {billing.driver_name}
                        </span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-rp-grey">Tier</span>
                        <span className="text-neutral-300">
                          {billing.pricing_tier_name}
                        </span>
                      </div>
                    </div>

                    {/* Session status badge for waiting_for_game */}
                    {isWaitingForGame && (
                      <div className="flex items-center justify-center py-1">
                        <StatusBadge status="waiting_for_game" />
                      </div>
                    )}

                    {/* Controls -- hidden during waiting_for_game */}
                    {!isWaitingForGame && (
                      <div className="flex gap-2">
                        <button
                          onClick={() =>
                            handlePauseResume(billing.id, isPaused)
                          }
                          className={`flex-1 rounded px-2 py-1.5 text-xs font-medium transition-colors ${
                            isPaused
                              ? "bg-emerald-500/20 text-emerald-400 hover:bg-emerald-500/30"
                              : "bg-rp-card text-neutral-400 hover:bg-rp-card"
                          }`}
                        >
                          {isPaused ? "Resume" : "Pause"}
                        </button>
                        <button
                          onClick={() => handleEnd(billing.id)}
                          className="flex-1 rounded px-2 py-1.5 text-xs font-medium bg-red-500/20 text-red-400 hover:bg-red-500/30 transition-colors"
                        >
                          End
                        </button>
                        <button
                          onClick={() => handleExtend(billing.id)}
                          className="rounded px-2 py-1.5 text-xs font-medium bg-rp-card text-neutral-400 hover:bg-rp-card transition-colors"
                          title="Extend by 10 minutes"
                        >
                          +10m
                        </button>
                      </div>
                    )}
                  </div>
                ) : pendingToken ? (
                  <div className="space-y-2">
                    <div className="text-xs space-y-1.5">
                      <div className="flex justify-between">
                        <span className="text-rp-grey">Driver</span>
                        <span className="text-yellow-400">{pendingToken.driver_name}</span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-rp-grey">Tier</span>
                        <span className="text-neutral-300">{pendingToken.pricing_tier_name}</span>
                      </div>
                      {pendingToken.auth_type === "pin" && (
                        <div className="flex justify-between items-center">
                          <span className="text-rp-grey">PIN</span>
                          <span className="text-2xl font-bold font-mono tracking-widest text-yellow-300">
                            {pendingToken.token}
                          </span>
                        </div>
                      )}
                      {pendingToken.auth_type === "qr" && (
                        <div className="flex justify-between">
                          <span className="text-rp-grey">Method</span>
                          <span className="text-neutral-300">QR scan</span>
                        </div>
                      )}
                    </div>
                    <div className="flex items-center justify-between text-xs">
                      <span className="text-yellow-400 animate-pulse font-medium">Waiting for customer...</span>
                    </div>
                    <button
                      onClick={() => handleCancelToken(pendingToken.id)}
                      className="w-full rounded-lg py-2 text-xs font-medium bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/20 transition-colors"
                    >
                      Cancel Assignment
                    </button>
                  </div>
                ) : (
                  <div className="pt-2">
                    <div className={`w-full rounded-lg py-2.5 text-sm font-medium text-center ${
                      pod.status === "offline"
                        ? "bg-rp-card text-rp-grey"
                        : "bg-emerald-500/10 text-emerald-400 border border-emerald-500/20"
                    }`}>
                      {pod.status === "offline" ? "Offline" : "Idle — Start from Staff Kiosk"}
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Wallet Top-Up Modal */}
      {showTopup && (
        <WalletTopupModal
          onClose={() => setShowTopup(false)}
          onSuccess={() => toast({ message: "Wallet topped up", type: "success" })}
        />
      )}
    </DashboardLayout>
  );
}
