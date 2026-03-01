"use client";

import { useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import CountdownTimer from "@/components/CountdownTimer";
import BillingStartModal from "@/components/BillingStartModal";
import StatusBadge from "@/components/StatusBadge";
import { useWebSocket } from "@/hooks/useWebSocket";
import type { Pod } from "@/lib/api";

export default function BillingPage() {
  const { pods, billingTimers, billingWarnings, sendCommand } = useWebSocket();
  const [modalPod, setModalPod] = useState<Pod | null>(null);

  function handleStart(data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    custom_price_paise?: number;
    custom_duration_minutes?: number;
  }) {
    sendCommand("start_billing", data);
    setModalPod(null);
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
          <h1 className="text-2xl font-bold text-zinc-100">Billing</h1>
          <p className="text-sm text-zinc-500">Session management</p>
        </div>
        <span className="text-xs text-zinc-500">
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
        <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-8 text-center">
          <p className="text-zinc-400 mb-2">No pods connected</p>
          <p className="text-zinc-500 text-sm">
            Pods appear automatically when rc-agent connects from a sim PC.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {sortedPods.map((pod) => {
            const billing = billingTimers.get(pod.id);
            const isPaused = billing?.status === "paused_manual";

            return (
              <div
                key={pod.id}
                className={`rounded-lg border p-4 transition-all ${
                  billing
                    ? "border-orange-500/50 bg-orange-500/5"
                    : pod.status === "idle"
                    ? "border-zinc-700 bg-zinc-900"
                    : pod.status === "offline"
                    ? "border-zinc-800 bg-zinc-900/50"
                    : "border-zinc-800 bg-zinc-900"
                }`}
              >
                {/* Pod header */}
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <span className="text-xl font-bold text-zinc-300">
                      {String(pod.number).padStart(2, "0")}
                    </span>
                    <span className="text-sm text-zinc-500">{pod.name}</span>
                  </div>
                  <StatusBadge status={pod.status} />
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
                        <span className="text-zinc-500">Driver</span>
                        <span className="text-orange-400">
                          {billing.driver_name}
                        </span>
                      </div>
                      <div className="flex justify-between">
                        <span className="text-zinc-500">Tier</span>
                        <span className="text-zinc-300">
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
                            : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
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
                        className="rounded px-2 py-1.5 text-xs font-medium bg-zinc-800 text-zinc-400 hover:bg-zinc-700 transition-colors"
                        title="Extend by 10 minutes"
                      >
                        +10m
                      </button>
                    </div>
                  </div>
                ) : (
                  <div className="pt-2">
                    <button
                      onClick={() => setModalPod(pod)}
                      disabled={pod.status === "offline"}
                      className={`w-full rounded-lg py-2.5 text-sm font-semibold transition-all ${
                        pod.status === "offline"
                          ? "bg-zinc-800 text-zinc-600 cursor-not-allowed"
                          : "bg-orange-500 text-white hover:bg-orange-600 active:bg-orange-700"
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
        />
      )}
    </DashboardLayout>
  );
}
