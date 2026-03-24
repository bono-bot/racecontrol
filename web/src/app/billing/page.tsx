"use client";

import { useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import CountdownTimer from "@/components/CountdownTimer";
import BillingStartModal from "@/components/BillingStartModal";
import StatusBadge from "@/components/StatusBadge";
import { useWebSocket } from "@/hooks/useWebSocket";
import type { Pod } from "@/lib/api";

export default function BillingPage() {
  const { pods, billingTimers, billingWarnings, pendingAuthTokens, sendCommand } = useWebSocket();
  const [modalPod, setModalPod] = useState<Pod | null>(null);

  function handleStart(data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    custom_price_paise?: number;
    custom_duration_minutes?: number;
    payment_method: string;
    staff_discount_paise?: number;
    discount_reason?: string;
  }) {
    sendCommand("start_billing", data);
    setModalPod(null);
  }

  function handleAssign(data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    auth_type: string;
    custom_price_paise?: number;
    custom_duration_minutes?: number;
    payment_method: string;
    staff_discount_paise?: number;
    discount_reason?: string;
  }) {
    sendCommand("assign_customer", data);
    setModalPod(null);
  }

  function handleCancelToken(tokenId: string) {
    sendCommand("cancel_assignment", { token_id: tokenId });
  }

  function handlePauseResume(sessionId: string, isPaused: boolean) {
    if (isPaused) {
      sendCommand("resume_billing", { session_id: sessionId });
    } else {
      sendCommand("pause_billing", { session_id: sessionId });
    }
  }

  function handleEnd(sessionId: string) {
    sendCommand("end_billing", { session_id: sessionId });
  }

  function handleExtend(sessionId: string) {
    sendCommand("extend_billing", {
      session_id: sessionId,
      additional_seconds: 600,
    });
  }

  const sortedPods = [...pods].sort((a, b) => a.number - b.number);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Billing</h1>
          <p className="text-sm text-rp-grey">Session management</p>
        </div>
        <span className="text-xs text-rp-grey">
          {billingTimers.size} active session{billingTimers.size !== 1 ? "s" : ""}
        </span>
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
                <span className="text-amber-400 text-lg">&#9888;</span>
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

      {/* Pod Grid */}
      {sortedPods.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No pods connected</p>
          <p className="text-rp-grey text-sm">
            Pods appear automatically when rc-agent connects from a sim PC.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {sortedPods.map((pod) => {
            const billing = billingTimers.get(pod.id);
            const pendingToken = pendingAuthTokens.get(pod.id);
            const isPaused = billing?.status === "paused_manual";

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

                    {/* Controls */}
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
                    <button
                      onClick={() => setModalPod(pod)}
                      disabled={pod.status === "offline"}
                      className={`w-full rounded-lg py-2.5 text-sm font-semibold transition-all ${
                        pod.status === "offline"
                          ? "bg-rp-card text-rp-grey cursor-not-allowed"
                          : "bg-rp-red text-white hover:bg-rp-red active:bg-rp-red"
                      }`}
                    >
                      Start Session
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Start Modal */}
      {modalPod && (
        <BillingStartModal
          podId={modalPod.id}
          podName={`Pod ${String(modalPod.number).padStart(2, "0")} - ${modalPod.name}`}
          onClose={() => setModalPod(null)}
          onStart={handleStart}
          onAssign={handleAssign}
        />
      )}
    </DashboardLayout>
  );
}
