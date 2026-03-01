import type { Pod, BillingSession } from "@/lib/api";
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
}

export default function PodCard({ pod, billingSession }: PodCardProps) {
  return (
    <div
      className={`rounded-lg border p-4 transition-all ${
        billingSession
          ? "border-orange-500/50 bg-orange-500/5"
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
        <StatusBadge status={pod.status} />
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
