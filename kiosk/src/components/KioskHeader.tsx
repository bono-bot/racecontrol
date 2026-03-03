"use client";

import { useEffect, useState } from "react";
import type { Pod } from "@/lib/types";

interface KioskHeaderProps {
  connected: boolean;
  pods: Map<string, Pod>;
  venueName?: string;
}

export function KioskHeader({ connected, pods, venueName = "Racing Point" }: KioskHeaderProps) {
  const [clock, setClock] = useState("");

  useEffect(() => {
    const tick = () => {
      const now = new Date();
      setClock(
        now.toLocaleTimeString("en-IN", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        })
      );
    };
    tick();
    const interval = setInterval(tick, 1000);
    return () => clearInterval(interval);
  }, []);

  const podArray = Array.from(pods.values());
  const activePods = podArray.filter((p) => p.status === "in_session").length;
  const idlePods = podArray.filter((p) => p.status === "idle").length;
  const offlinePods = podArray.filter((p) => p.status === "offline").length;

  return (
    <header className="flex items-center justify-between px-6 py-3 bg-rp-card border-b border-rp-border">
      {/* Left: Brand */}
      <div className="flex items-center gap-3">
        <h1 className="text-xl font-bold tracking-wide uppercase text-white">
          {venueName}
        </h1>
        <span className="text-xs text-rp-grey font-medium tracking-widest uppercase">
          Kiosk Terminal
        </span>
      </div>

      {/* Center: Pod counts */}
      <div className="flex items-center gap-6 text-sm">
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-rp-red" />
          <span className="text-white font-semibold">{activePods}</span>
          <span className="text-rp-grey">Active</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-green-500" />
          <span className="text-white font-semibold">{idlePods}</span>
          <span className="text-rp-grey">Idle</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-zinc-600" />
          <span className="text-white font-semibold">{offlinePods}</span>
          <span className="text-rp-grey">Offline</span>
        </div>
      </div>

      {/* Right: Clock + Connection */}
      <div className="flex items-center gap-4">
        <span className="text-2xl font-semibold tabular-nums text-white">
          {clock}
        </span>
        <div className="flex items-center gap-2">
          <span
            className={`w-2.5 h-2.5 rounded-full ${
              connected ? "bg-green-500 pulse-dot" : "bg-red-500"
            }`}
          />
          <span className="text-xs text-rp-grey">
            {connected ? "Connected" : "Disconnected"}
          </span>
        </div>
      </div>
    </header>
  );
}
