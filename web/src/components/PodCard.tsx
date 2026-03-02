import { useState, useEffect } from "react";
import type { Pod, BillingSession, AuthTokenInfo } from "@/lib/api";
import StatusBadge from "./StatusBadge";
import CountdownTimer from "./CountdownTimer";

const simLabels: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  f1_25: "F1 25",
  forza: "Forza Motorsport",
};

interface PodCardProps {
  pod: Pod;
  billingSession?: BillingSession;
  pendingToken?: AuthTokenInfo;
  onCancelToken?: (tokenId: string) => void;
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

  return (
    <div
      className={`rounded-lg border p-4 transition-all ${
        billingSession
          ? "border-orange-500/50 bg-orange-500/5"
          : isPending
          ? "border-yellow-500/50 bg-yellow-500/5"
          : pod.status === "in_session"
          ? "border-orange-500/50 bg-orange-500/5"
          : pod.status === "idle"
          ? "border-emerald-500/30 bg-zinc-900"
          : pod.status === "error"
          ? "border-red-500/50 bg-red-500/5"
          : "border-zinc-800 bg-zinc-900"
      }`}
    >
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className="text-xl font-bold text-zinc-300">
            {String(pod.number).padStart(2, "0")}
          </span>
          <span className="text-sm text-zinc-500">{pod.name}</span>
        </div>
        {isPending ? (
          <span className="text-[10px] font-bold px-2 py-0.5 rounded-full bg-yellow-500/20 text-yellow-400 animate-pulse">
            WAITING
          </span>
        ) : (
          <StatusBadge status={pod.status} />
        )}
      </div>

      {billingSession ? (
        <div className="space-y-2">
          <CountdownTimer
            remaining={billingSession.remaining_seconds}
            allocated={billingSession.allocated_seconds}
            drivingState={billingSession.driving_state}
          />
          <div className="text-xs space-y-1 mt-2">
            <div className="flex justify-between">
              <span className="text-zinc-500">Driver</span>
              <span className="text-orange-400">{billingSession.driver_name}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-zinc-500">Tier</span>
              <span className="text-zinc-300">{billingSession.pricing_tier_name}</span>
            </div>
          </div>
        </div>
      ) : isPending ? (
        <div className="space-y-2">
          {/* Pending auth token — show PIN or QR indicator */}
          <div className="text-xs space-y-1.5">
            <div className="flex justify-between">
              <span className="text-zinc-500">Driver</span>
              <span className="text-yellow-400">{pendingToken.driver_name}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-zinc-500">Tier</span>
              <span className="text-zinc-300">{pendingToken.pricing_tier_name}</span>
            </div>
            {pendingToken.auth_type === "pin" && (
              <div className="flex justify-between items-center">
                <span className="text-zinc-500">PIN</span>
                <span className="text-2xl font-bold font-mono tracking-widest text-yellow-300">
                  {pendingToken.token}
                </span>
              </div>
            )}
            {pendingToken.auth_type === "qr" && (
              <div className="flex justify-between">
                <span className="text-zinc-500">Method</span>
                <span className="text-zinc-300">QR on rig screen</span>
              </div>
            )}
            <div className="flex justify-between">
              <span className="text-zinc-500">Expires in</span>
              <ExpiryCountdown expiresAt={pendingToken.expires_at} />
            </div>
          </div>
          {onCancelToken && (
            <button
              onClick={() => onCancelToken(pendingToken.id)}
              className="w-full mt-2 text-xs text-red-400 hover:text-red-300 bg-red-500/10 border border-red-500/20 rounded py-1.5 transition-colors"
            >
              Cancel Assignment
            </button>
          )}
        </div>
      ) : (
        <div className="space-y-1.5 text-xs">
          <div className="flex justify-between">
            <span className="text-zinc-500">Sim</span>
            <span className="text-zinc-300">{simLabels[pod.sim_type] || pod.sim_type}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-zinc-500">IP</span>
            <span className="text-zinc-400 font-mono">{pod.ip_address || "—"}</span>
          </div>
          {pod.current_driver && (
            <div className="flex justify-between">
              <span className="text-zinc-500">Driver</span>
              <span className="text-orange-400">{pod.current_driver}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
