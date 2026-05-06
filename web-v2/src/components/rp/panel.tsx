"use client";

import * as React from "react";
import {
  Card,
  CardContent,
  CardHeader,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";

// Panel — Layer 1 wrapper around shadcn Card per Captain bundle 2026-05-02
// components.jsx Panel API. CardHeader rendered as uppercase Chakra Petch
// eyebrow per §3.2.2 h3 type token (size 14 / weight 600 / tracking 0.08em).
//
// Usage:
//   <Panel title="Active sessions">
//     <Panel.Body>...</Panel.Body>
//   </Panel>

type PanelProps = React.ComponentProps<typeof Card> & {
  title?: React.ReactNode;
  action?: React.ReactNode;
};

export function Panel({ title, action, className, children, ...props }: PanelProps) {
  return (
    <Card
      className={cn(
        "rounded-md bg-rp-card border border-rp-border ring-0 gap-0 py-0",
        className,
      )}
      {...props}
    >
      {(title || action) && (
        <CardHeader
          className={cn(
            "flex items-center justify-between px-4 py-3 border-b border-rp-border",
          )}
        >
          {title && (
            <span className="font-display text-sm font-semibold uppercase tracking-[0.08em] text-rp-ink-dim">
              {title}
            </span>
          )}
          {action && <div>{action}</div>}
        </CardHeader>
      )}
      <CardContent className="px-4 py-4">{children}</CardContent>
    </Card>
  );
}
