import * as React from "react";
import { cn } from "@/lib/utils";

// StatusDot — Layer 1 8px state dot per Captain bundle 2026-05-02
// components.jsx StatusDot API. Colors green/amber/red/grey + optional pulse
// animation. Used in: pod grid, fleet health, session cards, alert lists.

export type StatusDotState = "green" | "amber" | "red" | "grey";

type StatusDotProps = React.ComponentProps<"span"> & {
  state?: StatusDotState;
  pulse?: boolean;
  size?: number;
};

const stateBg: Record<StatusDotState, string> = {
  green: "bg-rp-green",
  amber: "bg-rp-amber",
  red: "bg-rp-red",
  grey: "bg-rp-grey",
};

export function StatusDot({
  state = "grey",
  pulse,
  size = 8,
  className,
  ...props
}: StatusDotProps) {
  return (
    <span
      role="status"
      aria-label={`status: ${state}`}
      data-rp-state={state}
      data-rp-pulse={pulse ? "true" : undefined}
      className={cn(
        "inline-block rounded-full shrink-0",
        stateBg[state],
        pulse && "ring-2 ring-current/20 animate-pulse",
        className,
      )}
      style={{ width: size, height: size }}
      {...props}
    />
  );
}
