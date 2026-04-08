"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import type { Pod } from "@/lib/types";

interface KioskHeaderProps {
  connected: boolean;
  pods: Map<string, Pod>;
  venueName?: string;
  staffName?: string;
  onSignOut?: () => void;
}

export function KioskHeader({ connected, pods, venueName = "Racing Point", staffName, onSignOut }: KioskHeaderProps) {
  const [clock, setClock] = useState("");
  const pathname = usePathname();

  useEffect(() => {
    const tick = () => {
      const now = new Date();
      setClock(
        now.toLocaleTimeString("en-IN", {
          timeZone: "Asia/Kolkata",
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
    <header className="flex items-center justify-between px-5 py-2.5 bg-[#0A0A0A] border-b-2 border-rp-red shrink-0">
      {/* Left: Brand */}
      <div className="flex items-center gap-4">
        <h1 className="text-lg font-bold tracking-wider uppercase text-white font-[family-name:var(--font-display)]">
          RACING<span className="text-rp-red">POINT</span>
        </h1>
        {staffName && (
          <div className="flex items-center gap-2 border-l border-[#2A2A2A] pl-4">
            {[
              { href: "/staff", label: "Dashboard" },
              { href: "/debug", label: "Debug" },
            ].map((nav) => (
              <Link
                key={nav.href}
                href={nav.href}
                className={`px-3 py-1.5 text-xs font-medium rounded transition-colors duration-200 cursor-pointer ${
                  pathname === nav.href
                    ? "bg-rp-red/15 text-white border border-rp-red/40"
                    : "text-zinc-500 hover:text-white hover:bg-[#1E1E1E]"
                }`}
              >
                {nav.label}
              </Link>
            ))}
            <Link
              href="/shutdown"
              className="px-3 py-1.5 text-xs text-red-500 hover:text-red-400 hover:bg-red-500/10 rounded cursor-pointer transition-colors duration-200"
            >
              Shutdown
            </Link>
          </div>
        )}
      </div>

      {/* Center: Pod status count pills */}
      <div className="flex items-center gap-2">
        <StatusPill count={activePods} color="red" label="Racing" />
        <StatusPill count={idlePods} color="green" label="Available" />
        <StatusPill count={offlinePods} color="grey" label="Offline" />
      </div>

      {/* Right: Staff + WS dot + Sign Out */}
      <div className="flex items-center gap-4">
        <span className="text-sm font-mono tabular-nums text-zinc-400">
          {clock}
        </span>

        {staffName && (
          <>
            <span className="text-xs text-zinc-500 font-sans">
              {staffName}
            </span>
          </>
        )}

        {/* WS connection dot */}
        <div className="flex items-center gap-1.5" title={connected ? "WebSocket connected" : "WebSocket disconnected"}>
          <span
            className={`w-2 h-2 rounded-full ${
              connected ? "bg-green-500 pulse-dot" : "bg-red-500"
            }`}
          />
          <span className="text-[10px] text-zinc-600 uppercase tracking-wider">
            {connected ? "Live" : "Down"}
          </span>
        </div>

        {staffName && onSignOut && (
          <button
            onClick={onSignOut}
            className="text-xs text-rp-red hover:text-white cursor-pointer transition-colors duration-200"
          >
            Sign Out
          </button>
        )}
      </div>
    </header>
  );
}

function StatusPill({ count, color, label }: { count: number; color: "red" | "green" | "grey"; label: string }) {
  const colorMap = {
    red: { dot: "bg-rp-red", bg: "bg-rp-red/10", text: "text-rp-red", border: "border-rp-red/20" },
    green: { dot: "bg-emerald-500", bg: "bg-emerald-500/10", text: "text-emerald-400", border: "border-emerald-500/20" },
    grey: { dot: "bg-zinc-600", bg: "bg-zinc-800", text: "text-zinc-500", border: "border-zinc-700" },
  };
  const c = colorMap[color];

  return (
    <div className={`flex items-center gap-1.5 px-3 py-1 rounded-full border ${c.bg} ${c.border}`}>
      <span className={`w-1.5 h-1.5 rounded-full ${c.dot}`} />
      <span className={`text-xs font-mono font-bold tabular-nums ${c.text}`}>{count}</span>
      <span className="text-[10px] text-zinc-500 uppercase tracking-wider">{label}</span>
    </div>
  );
}
