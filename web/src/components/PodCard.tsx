"use client";

import { useState, useEffect } from "react";
import type { Pod, BillingSession, AuthTokenInfo } from "@/lib/api";
import StatusBadge from "./StatusBadge";
import CountdownTimer from "./CountdownTimer";

const simLabels: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  assetto_corsa_evo: "AC EVO",
  assetto_corsa_rally: "AC Rally",
  f1_25: "F1 25",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  forza: "Forza Motorsport",
  forza_horizon_5: "Forza Horizon 5",
};

interface PodCardProps {
  pod: Pod;
  billingSession?: BillingSession;
  pendingToken?: AuthTokenInfo;
  onCancelToken?: (tokenId: string) => void;
}

/** Map pod/session status to left-edge bar color */
function getBarColor(status: string): string {
  switch (status) {
    case "idle":
    case "connected":
      return "bg-rp-green";
    case "in_session":
    case "active":
      return "bg-rp-red";
    case "error":
    case "disconnected":
    case "cancelled":
    case "cancelled_no_playable":
      return "bg-red-500";
    case "pending":
    case "stopping":
      return "bg-rp-yellow";
    case "offline":
    case "completed":
      return "bg-rp-grey";
    case "launching":
    case "loading":
    case "maintenance":
      return "bg-blue-400";
    default:
      return "bg-rp-grey";
  }
}

/** Map status to card border highlight */
function getCardBorder(status: string, hasBilling: boolean, isPending: boolean): string {
  if (hasBilling) return "border-rp-red/30";
  if (isPending) return "border-yellow-500/30";
  if (status === "in_session") return "border-rp-red/30";
  if (status === "error") return "border-red-500/30";
  return "border-rp-border";
}

function ExpiryCountdown({ expiresAt }: { expiresAt: string }) {
  const [remaining, setRemaining] = useState(0);

  useEffect(() => {
    const update = () => {
      const diff = Math.max(
        0,
        Math.floor((new Date(expiresAt).getTime() - Date.now()) / 1000)
      );
      setRemaining(diff);
    };
    update();
    const interval = setInterval(update, 1000);
    return () => clearInterval(interval);
  }, [expiresAt]);

  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;

  return (
    <span className="text-yellow-400 font-mono text-xs">
      {mins}:{String(secs).padStart(2, "0")}
    </span>
  );
}

export default function PodCard({
  pod,
  billingSession,
  pendingToken,
  onCancelToken,
}: PodCardProps) {
  const isPending = !!pendingToken;
  const barColor = getBarColor(isPending ? "pending" : pod.status);
  const cardBorder = getCardBorder(pod.status, !!billingSession, isPending);
  const isOffline = pod.status === "offline";

  return (
    <div
      className={`flex items-stretch rounded-lg border overflow-hidden transition-all ${cardBorder} ${isOffline ? "opacity-60" : ""}`}
    >
      {/* Left status bar — F1 timing tower style */}
      <div className={`w-1 flex-shrink-0 ${barColor}`} />

      {/* Main content row */}
      <div className="flex items-center gap-4 px-4 py-3 flex-1 bg-rp-card min-w-0">
        {/* Pod number */}
        <span className="text-2xl font-mono font-bold text-neutral-300 w-8 text-center flex-shrink-0">
          {String(pod.number).padStart(2, "0")}
        </span>

        {/* Status badge */}
        {isPending ? (
          <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-yellow-500/20 text-yellow-400 animate-pulse flex-shrink-0">
            WAITING
          </span>
        ) : (
          <StatusBadge status={pod.status} />
        )}

        {/* Info section */}
        <div className="min-w-0 flex-1">
          {billingSession ? (
            <div className="flex flex-col gap-0.5">
              <span className="text-rp-red font-medium text-sm truncate">
                {billingSession.driver_name}
              </span>
              <span className="text-neutral-400 text-xs truncate">
                {simLabels[pod.sim_type] || pod.sim_type}
                {billingSession.pricing_tier_name && (
                  <span className="text-rp-grey ml-2">{billingSession.pricing_tier_name}</span>
                )}
              </span>
            </div>
          ) : isPending && pendingToken ? (
            <div className="flex flex-col gap-0.5">
              <span className="text-yellow-400 font-medium text-sm truncate">
                {pendingToken.driver_name}
              </span>
              <div className="flex items-center gap-2 text-xs">
                <span className="text-neutral-400">{pendingToken.pricing_tier_name}</span>
                {pendingToken.auth_type === "pin" && (
                  <span className="font-mono font-bold tracking-widest text-yellow-300">
                    PIN: {pendingToken.token}
                  </span>
                )}
                {pendingToken.auth_type === "qr" && (
                  <span className="text-neutral-300">QR on rig</span>
                )}
              </div>
            </div>
          ) : (
            <div className="flex flex-col gap-0.5">
              <span className="text-neutral-300 text-xs truncate">
                {simLabels[pod.sim_type] || pod.sim_type}
              </span>
              {pod.current_driver && (
                <span className="text-rp-red text-xs truncate">{pod.current_driver}</span>
              )}
              {!pod.current_driver && (
                <span className="text-rp-grey text-xs">{pod.ip_address || "No IP"}</span>
              )}
            </div>
          )}
        </div>

        {/* Right section — timer / expiry / cancel */}
        <div className="ml-auto flex items-center gap-4 flex-shrink-0">
          {billingSession && (
            <CountdownTimer
              remaining={billingSession.remaining_seconds}
              allocated={billingSession.allocated_seconds}
              drivingState={billingSession.driving_state}
              compact
            />
          )}

          {isPending && pendingToken && (
            <div className="flex items-center gap-3">
              <div className="flex flex-col items-end gap-0.5">
                <span className="text-rp-grey text-[10px]">Expires</span>
                <ExpiryCountdown expiresAt={pendingToken.expires_at} />
              </div>
              {onCancelToken && (
                <button
                  onClick={() => onCancelToken(pendingToken.id)}
                  className="text-xs text-red-400 hover:text-red-300 bg-red-500/10 border border-red-500/20 rounded px-2 py-1 transition-colors"
                >
                  Cancel
                </button>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
