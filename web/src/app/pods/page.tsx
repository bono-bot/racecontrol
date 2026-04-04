"use client";

import { useEffect, useState, useCallback } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import PodCard from "@/components/PodCard";
import StatusBadge from "@/components/StatusBadge";
import CountdownTimer from "@/components/CountdownTimer";
import BillingStartModal from "@/components/BillingStartModal";
import { Skeleton, EmptyState } from "@/components/Skeleton";
import { useWebSocket } from "@/hooks/useWebSocket";
import { api, racingWsPodsOnly } from "@/lib/api";
import type { Pod, PodFleetStatus } from "@/lib/api";

function MonitorIcon() {
  return (
    <svg className="w-10 h-10" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25A2.25 2.25 0 015.25 3h13.5A2.25 2.25 0 0121 5.25z" />
    </svg>
  );
}

function CloseIcon() {
  return (
    <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
      <path strokeLinecap="round" strokeLinejoin="round" d="M6 18L18 6M6 6l12 12" />
    </svg>
  );
}

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${h}h ${m}m`;
}

export default function PodsPage() {
  const { connected, pods, billingTimers, pendingAuthTokens, sendCommand } = useWebSocket();

  const [selectedPod, setSelectedPod] = useState<Pod | null>(null);
  const [drawerHealth, setDrawerHealth] = useState<PodFleetStatus | null>(null);
  const [drawerHealthLoading, setDrawerHealthLoading] = useState(false);
  const [modalPod, setModalPod] = useState<Pod | null>(null);

  const racingPods = racingWsPodsOnly(pods);
  const sortedPods = [...racingPods].sort((a, b) => a.number - b.number);

  // Status counts for KPI pills
  const onlineCount = racingPods.filter((p) => p.status !== "offline").length;
  const racingCount = racingPods.filter((p) => p.status === "in_session").length;
  const offlineCount = pods.filter((p) => p.status === "offline").length;

  // Fetch fleet health when drawer opens
  useEffect(() => {
    if (!selectedPod) {
      setDrawerHealth(null);
      return;
    }
    setDrawerHealthLoading(true);
    api.fleetHealth()
      .then((statuses) => {
        const match = statuses.find((s) => s.pod_number === selectedPod.number);
        setDrawerHealth(match ?? null);
      })
      .catch(() => setDrawerHealth(null))
      .finally(() => setDrawerHealthLoading(false));
  }, [selectedPod]);

  // Keep selectedPod in sync with WS updates
  useEffect(() => {
    if (!selectedPod) return;
    const updated = pods.find((p) => p.id === selectedPod.id);
    if (updated) {
      setSelectedPod(updated);
    }
  }, [pods, selectedPod?.id]);

  const handleStartSession = useCallback(
    (data: { pod_id: string; driver_id: string; pricing_tier_id: string; payment_method?: string; custom_price_paise?: number; custom_duration_minutes?: number; staff_discount_paise?: number; discount_reason?: string }) => {
      api.startBilling(data).catch(console.error);
      setModalPod(null);
    },
    []
  );

  const isLoading = pods.length === 0 && !connected;

  return (
    <DashboardLayout>
      {/* Page header */}
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-2xl font-bold text-white">Pods</h1>
        <span className="text-xs bg-rp-card border border-rp-border rounded-full px-3 py-1 text-neutral-300">
          {pods.length} pods
        </span>
      </div>

      {/* KPI sub-header strip */}
      <div className="flex gap-4 mb-4 text-sm">
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-rp-green/20 text-rp-green">
          <span className="w-1.5 h-1.5 rounded-full bg-rp-green" />
          {onlineCount} Online
        </span>
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-rp-red/20 text-rp-red">
          <span className="w-1.5 h-1.5 rounded-full bg-rp-red" />
          {racingCount} Racing
        </span>
        <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-rp-card text-neutral-400">
          <span className="w-1.5 h-1.5 rounded-full bg-rp-grey" />
          {offlineCount} Offline
        </span>
      </div>

      {/* Loading skeleton */}
      {isLoading ? (
        <div className="space-y-1">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-14 rounded-lg" />
          ))}
        </div>
      ) : pods.length === 0 ? (
        <EmptyState
          icon={<MonitorIcon />}
          headline="No pods registered"
          hint="Pods appear automatically when rc-agent connects."
        />
      ) : (
        /* F1 Timing Tower Strip */
        <div className="space-y-1">
          {sortedPods.map((pod) => (
            <div
              key={pod.id}
              className="cursor-pointer"
              onClick={() => setSelectedPod(pod)}
            >
              <PodCard
                pod={pod}
                billingSession={billingTimers.get(pod.id)}
                pendingToken={pendingAuthTokens.get(pod.id)}
                onCancelToken={(tokenId) =>
                  sendCommand("cancel_assignment", { token_id: tokenId })
                }
              />
            </div>
          ))}
        </div>
      )}

      {/* Detail Drawer */}
      {selectedPod && (
        <>
          {/* Backdrop */}
          <div
            className="fixed inset-0 bg-black/40 z-40"
            onClick={() => setSelectedPod(null)}
          />

          <aside className="fixed inset-y-0 right-0 w-80 bg-rp-card border-l border-rp-border p-6 z-50 flex flex-col overflow-y-auto">
            {/* Drawer Header */}
            <div className="flex items-center justify-between mb-6">
              <div>
                <h2 className="text-lg font-bold text-white font-mono">
                  Pod {String(selectedPod.number).padStart(2, "0")}
                </h2>
                <p className="text-sm text-neutral-400">{selectedPod.name}</p>
              </div>
              <button
                onClick={() => setSelectedPod(null)}
                className="text-neutral-400 hover:text-white transition-colors"
                aria-label="Close drawer"
              >
                <CloseIcon />
              </button>
            </div>

            {/* Status Section */}
            <div className="mb-6">
              <h3 className="text-xs font-medium text-rp-grey uppercase tracking-wider mb-3">Status</h3>
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-sm text-neutral-400">Status</span>
                  <StatusBadge status={selectedPod.status} />
                </div>
                {drawerHealthLoading ? (
                  <Skeleton className="h-4 w-full" />
                ) : drawerHealth ? (
                  <>
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-neutral-400">Uptime</span>
                      <span className="text-sm text-neutral-300">{formatUptime(drawerHealth.uptime_secs)}</span>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-neutral-400">WS Connected</span>
                      <span className={`text-sm ${drawerHealth.ws_connected ? "text-rp-green" : "text-rp-red"}`}>
                        {drawerHealth.ws_connected ? "Yes" : "No"}
                      </span>
                    </div>
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-neutral-400">Build</span>
                      <span className="text-sm font-mono text-neutral-300">{drawerHealth.build_id || "\u2014"}</span>
                    </div>
                  </>
                ) : null}
              </div>
            </div>

            {/* Billing Session Section */}
            {billingTimers.get(selectedPod.id) && (() => {
              const session = billingTimers.get(selectedPod.id)!;
              return (
                <div className="mb-6">
                  <h3 className="text-xs font-medium text-rp-grey uppercase tracking-wider mb-3">Active Session</h3>
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-neutral-400">Driver</span>
                      <span className="text-sm text-white">{session.driver_name}</span>
                    </div>
                    {session.pricing_tier_name && (
                      <div className="flex items-center justify-between">
                        <span className="text-sm text-neutral-400">Tier</span>
                        <span className="text-sm text-neutral-300">{session.pricing_tier_name}</span>
                      </div>
                    )}
                    <div className="flex justify-center mt-2">
                      <CountdownTimer
                        remaining={session.remaining_seconds}
                        allocated={session.allocated_seconds}
                        drivingState={session.driving_state}
                      />
                    </div>
                  </div>
                </div>
              );
            })()}

            {/* Pending Token Section */}
            {pendingAuthTokens.get(selectedPod.id) && (() => {
              const token = pendingAuthTokens.get(selectedPod.id)!;
              return (
                <div className="mb-6">
                  <h3 className="text-xs font-medium text-rp-grey uppercase tracking-wider mb-3">Pending Assignment</h3>
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-sm text-neutral-400">Driver</span>
                      <span className="text-sm text-yellow-400">{token.driver_name}</span>
                    </div>
                    {token.auth_type === "pin" && (
                      <div className="flex items-center justify-between">
                        <span className="text-sm text-neutral-400">PIN</span>
                        <span className="text-sm font-mono font-bold tracking-widest text-yellow-300">{token.token}</span>
                      </div>
                    )}
                    {token.auth_type === "qr" && (
                      <div className="flex items-center justify-between">
                        <span className="text-sm text-neutral-400">Auth</span>
                        <span className="text-sm text-neutral-300">QR on rig</span>
                      </div>
                    )}
                    <button
                      onClick={() => sendCommand("cancel_assignment", { token_id: token.id })}
                      className="w-full mt-2 text-sm text-red-400 hover:text-red-300 bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2 transition-colors"
                    >
                      Cancel Assignment
                    </button>
                  </div>
                </div>
              );
            })()}

            {/* Spacer */}
            <div className="flex-1" />

            {/* Start Session Button */}
            <button
              onClick={() => {
                setModalPod(selectedPod);
              }}
              disabled={selectedPod.status === "offline"}
              className="w-full py-2.5 rounded-lg text-sm font-medium transition-colors bg-rp-red text-white hover:bg-rp-red/80 disabled:opacity-40 disabled:cursor-not-allowed"
            >
              Start Session
            </button>
          </aside>
        </>
      )}

      {/* Billing Start Modal */}
      {modalPod && (
        <BillingStartModal
          podId={modalPod.id}
          podName={`Pod ${String(modalPod.number).padStart(2, "0")} - ${modalPod.name}`}
          onClose={() => setModalPod(null)}
          onStart={handleStartSession}

        />
      )}
    </DashboardLayout>
  );
}
