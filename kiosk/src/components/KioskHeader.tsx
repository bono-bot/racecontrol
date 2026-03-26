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

      {/* Right: Nav + Staff + Clock + Connection */}
      <div className="flex items-center gap-4">
        {staffName && (
          <div className="flex items-center gap-6 border-r border-rp-border pr-6">
            {[
              { href: "/staff", label: "Dashboard" },
              { href: "/debug", label: "Debug" },
            ].map((nav) => (
              <Link
                key={nav.href}
                href={nav.href}
                className={`px-4 py-2 text-sm font-medium border rounded-lg transition-colors ${
                  pathname === nav.href
                    ? "border-rp-red bg-rp-red/10 text-white"
                    : "border-rp-border text-rp-grey hover:text-white hover:border-rp-red hover:bg-rp-red/10"
                }`}
              >
                {nav.label}
              </Link>
            ))}
            <Link
              href="/shutdown"
              className="px-3 py-1.5 border border-red-600/40 text-red-400 hover:bg-red-600/10 rounded text-xs transition-colors"
            >
              Shutdown Venue
            </Link>
            <span className="text-sm text-rp-grey ml-2">
              Staff: <span className="text-white font-medium">{staffName}</span>
            </span>
            {onSignOut && (
              <button
                onClick={onSignOut}
                className="px-4 py-2 text-sm font-medium border border-rp-border rounded-lg text-rp-grey hover:text-white hover:border-rp-red hover:bg-rp-red/10 transition-colors"
              >
                Logout
              </button>
            )}
          </div>
        )}
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
