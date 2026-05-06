"use client";

import * as React from "react";
import Link from "next/link";
import { Bell, Coffee } from "lucide-react";
import { StatusDot } from "@/components/rp";
import { cn } from "@/lib/utils";

// Chef notifications widget — POS header. Shows pending + ready cafe orders.
// Per V2 Q3 lock (kiosk/PWA→chef→staff-delivery→pay-separately) + Q-S6
// closure (Opt C hybrid: WhatsApp primary + chef display kitchen audio +
// staff-courier delivery). POS staff need visibility into "what's ready
// for me to deliver" without leaving their current screen.
//
// MOCK data this pass; Phase 1 wires to real chef-display state stream.

type ChefStat = {
  pending: number;
  ready: number;
};

const MOCK: ChefStat = { pending: 2, ready: 1 };

export function ChefNotifications() {
  const stat = MOCK; // TODO Phase 1: useChefDisplayState() hook from /api/v1/cafe/orders

  const hasReady = stat.ready > 0;

  return (
    <Link
      href="/pos/cafe/orders"
      className={cn(
        "flex items-center gap-3 px-4 py-2 rounded-md border transition-colors duration-(--motion-fast)",
        hasReady
          ? "bg-rp-card-hi border-rp-amber/40 hover:border-rp-amber"
          : "bg-rp-card border-rp-border hover:border-rp-border-hi",
      )}
      aria-label={`${stat.pending} pending, ${stat.ready} ready for delivery`}
    >
      <Coffee className="size-4 text-rp-ink-dim shrink-0" />
      <div className="flex items-center gap-4">
        <Stat label="pending" value={stat.pending} dot="amber" />
        <span aria-hidden className="text-rp-border-hi">|</span>
        <Stat
          label="ready"
          value={stat.ready}
          dot={hasReady ? "green" : "grey"}
          pulse={hasReady}
        />
      </div>
      {hasReady && (
        <Bell className="size-4 text-rp-amber animate-pulse shrink-0" aria-hidden />
      )}
    </Link>
  );
}

function Stat({
  label,
  value,
  dot,
  pulse,
}: {
  label: string;
  value: number;
  dot: "amber" | "green" | "grey";
  pulse?: boolean;
}) {
  return (
    <div className="flex items-center gap-2">
      <StatusDot state={dot} pulse={pulse} size={6} />
      <span className="font-mono text-base font-semibold tabular-nums text-rp-ink">{value}</span>
      <span className="font-body uppercase text-[10px] tracking-[0.08em] text-rp-grey">{label}</span>
    </div>
  );
}
