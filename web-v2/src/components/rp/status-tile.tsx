import * as React from "react";
import { Card } from "@/components/ui/card";
import { cn } from "@/lib/utils";

// StatusTile — Layer 1 KPI tile per Captain bundle 2026-05-02 components.jsx
// StatusTile API. Caption (uppercase eyebrow) + display value (mono-big or
// display-L) + optional sub. Used in: cockpit KPIs, session totals, fleet
// summaries.

type StatusTileProps = React.ComponentProps<typeof Card> & {
  caption: React.ReactNode;
  value: React.ReactNode;
  sub?: React.ReactNode;
  /** Render value in display font (Chakra Petch) instead of mono */
  display?: boolean;
};

export function StatusTile({
  caption,
  value,
  sub,
  display,
  className,
  ...props
}: StatusTileProps) {
  return (
    <Card
      className={cn(
        "rounded-md bg-rp-card border border-rp-border ring-0 px-4 py-3 gap-1",
        className,
      )}
      {...props}
    >
      <span className="font-body text-[11px] font-semibold uppercase tracking-[0.06em] text-rp-ink-dim">
        {caption}
      </span>
      <span
        className={cn(
          "block leading-none text-rp-ink",
          display
            ? "font-display text-4xl font-bold tracking-tight"
            : "font-mono text-2xl font-semibold tabular-nums",
        )}
      >
        {value}
      </span>
      {sub && (
        <span className="font-body text-xs text-rp-ink-dim">{sub}</span>
      )}
    </Card>
  );
}
