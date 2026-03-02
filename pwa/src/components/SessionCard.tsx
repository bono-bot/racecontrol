"use client";

import type { BillingSession } from "@/lib/api";
import Link from "next/link";

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}m ${s}s`;
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleDateString("en-IN", {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function statusColor(status: string): string {
  switch (status) {
    case "active":
      return "text-emerald-400";
    case "completed":
      return "text-neutral-400";
    case "ended_early":
      return "text-rp-red";
    case "cancelled":
      return "text-red-400";
    default:
      return "text-rp-grey";
  }
}

function statusLabel(status: string): string {
  switch (status) {
    case "active":
      return "Active";
    case "completed":
      return "Completed";
    case "ended_early":
      return "Ended Early";
    case "paused_manual":
      return "Paused";
    case "cancelled":
      return "Cancelled";
    case "pending":
      return "Pending";
    default:
      return status;
  }
}

export default function SessionCard({ session }: { session: BillingSession }) {
  return (
    <Link href={`/sessions/${session.id}`}>
      <div className="bg-rp-card border border-rp-border rounded-xl p-4 active:bg-rp-card transition-colors">
        <div className="flex items-center justify-between mb-2">
          <span className="text-sm font-medium text-neutral-300">
            Pod {session.pod_id.replace("pod_", "#")}
          </span>
          <span className={`text-xs font-medium ${statusColor(session.status)}`}>
            {statusLabel(session.status)}
          </span>
        </div>

        <div className="flex items-end justify-between">
          <div>
            <p className="text-lg font-bold text-white">
              {formatDuration(session.driving_seconds)}
            </p>
            <p className="text-xs text-rp-grey">
              of {formatDuration(session.allocated_seconds)}
            </p>
          </div>
          <p className="text-xs text-rp-grey">{formatDate(session.started_at)}</p>
        </div>

        {/* Progress bar */}
        <div className="mt-3 h-1.5 bg-rp-card rounded-full overflow-hidden">
          <div
            className="h-full bg-rp-red rounded-full transition-all"
            style={{
              width: `${Math.min(
                100,
                (session.driving_seconds / session.allocated_seconds) * 100
              )}%`,
            }}
          />
        </div>
      </div>
    </Link>
  );
}
