"use client";

import { useState, useEffect } from "react";
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
  // Local countdown: interpolate between WebSocket ticks for smooth 1s updates
  const [localRemaining, setLocalRemaining] = useState(billing.remaining_seconds);
  useEffect(() => {
    setLocalRemaining(billing.remaining_seconds);
  }, [billing.remaining_seconds]);
  // Only decrement when actually active — all pause states stop the countdown
  useEffect(() => {
    if (billing.status !== "active") return;
    const iv = setInterval(() => {
      setLocalRemaining((prev) => Math.max(0, prev - 1));
    }, 1000);
    return () => clearInterval(iv);
  }, [billing.id, billing.status]);

  const progress = Math.max(0, (localRemaining / billing.allocated_seconds) * 100);
  const isLow = localRemaining < 120;

  return (
    <div>
      <div className="flex justify-between text-xs text-rp-grey mb-1">
        <span>
          {billing.status === "paused_manual" ? "Paused" :
           billing.status === "waiting_for_game" ? "Game Loading" :
           billing.status === "paused_game_pause" ? "Relaunching" :
           billing.status === "paused_disconnect" ? "Disconnected" :
           "Remaining"}
        </span>
        <span className={`font-mono ${hasWarning ? "text-amber-400 font-bold animate-pulse" : isLow ? "text-rp-red font-semibold" : ""}`}>
          {formatTime(localRemaining)}
        </span>
      </div>
      <div className="w-full h-2 bg-zinc-800 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-1000 ${
            hasWarning ? "bg-amber-500" : isLow ? "bg-rp-red" : "bg-emerald-500"
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
