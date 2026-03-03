"use client";

import type { BillingSession } from "@/lib/types";

interface SessionTimerProps {
  billing: BillingSession;
  hasWarning?: boolean;
}

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function SessionTimer({ billing, hasWarning }: SessionTimerProps) {
  const progress = Math.max(0, (billing.remaining_seconds / billing.allocated_seconds) * 100);
  const isLow = billing.remaining_seconds < 120;

  return (
    <div>
      <div className="flex justify-between text-xs text-rp-grey mb-1">
        <span>{billing.status === "paused_manual" ? "Paused" : "Remaining"}</span>
        <span className={`font-mono ${hasWarning ? "text-amber-400 font-bold animate-pulse" : isLow ? "text-rp-red font-semibold" : ""}`}>
          {formatTime(billing.remaining_seconds)}
        </span>
      </div>
      <div className="w-full h-2 bg-zinc-800 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-1000 ${
            hasWarning ? "bg-amber-500" : isLow ? "bg-rp-red" : "bg-rp-red"
          }`}
          style={{ width: `${progress}%` }}
        />
      </div>
      {/* Driving state dot */}
      <div className="flex items-center gap-1 mt-1">
        <span
          className={`w-1.5 h-1.5 rounded-full ${
            billing.driving_state === "active"
              ? "bg-green-500 pulse-dot"
              : billing.driving_state === "idle"
              ? "bg-amber-500"
              : "bg-zinc-600"
          }`}
        />
        <span className="text-[10px] text-rp-grey capitalize">
          {billing.driving_state === "active" ? "Driving" : billing.driving_state === "idle" ? "Paused" : "No Device"}
        </span>
        <span className="text-[10px] text-rp-grey ml-auto">
          Drove {formatTime(billing.driving_seconds)}
        </span>
      </div>
    </div>
  );
}
