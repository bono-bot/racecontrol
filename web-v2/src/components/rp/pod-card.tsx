"use client";

import * as React from "react";
import { Card } from "@/components/ui/card";
import { StatusDot, type StatusDotState } from "@/components/rp/status-dot";
import { cn } from "@/lib/utils";

// PodCard — Layer 1 wrapper around shadcn Card per Captain bundle 2026-05-02
// components.jsx PodCard API. State-keyed accent border on left edge +
// StatusDot in header per HANDOFF.md §3 ("PodCard ... grid item w/ accent
// border keyed to state").

const accentBorder: Record<StatusDotState, string> = {
  green: "border-l-rp-green",
  amber: "border-l-rp-amber",
  red: "border-l-rp-red",
  grey: "border-l-rp-grey",
};

type PodCardProps = React.ComponentProps<typeof Card> & {
  podNumber: number;
  state?: StatusDotState;
  pulse?: boolean;
  caption?: React.ReactNode;
};

export function PodCard({
  podNumber,
  state = "grey",
  pulse,
  caption,
  className,
  children,
  ...props
}: PodCardProps) {
  return (
    <Card
      data-rp-pod-state={state}
      className={cn(
        "rounded-md bg-rp-card border border-rp-border border-l-4 ring-0 px-4 py-3 gap-2",
        accentBorder[state],
        className,
      )}
      {...props}
    >
      <div className="flex items-center justify-between">
        <span className="font-display text-2xl font-bold tracking-tight text-rp-ink">
          POD {String(podNumber).padStart(2, "0")}
        </span>
        <StatusDot state={state} pulse={pulse} />
      </div>
      {caption && (
        <span className="font-body text-xs uppercase tracking-[0.08em] text-rp-ink-dim">
          {caption}
        </span>
      )}
      {children && <div className="text-sm text-rp-ink-dim">{children}</div>}
    </Card>
  );
}
