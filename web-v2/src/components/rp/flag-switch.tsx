"use client";

import * as React from "react";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

// FlagSwitch — Layer 1 wrapper around shadcn Switch per Captain bundle
// 2026-05-02 components.jsx FlagSwitch API. Adds `pending` state with amber
// border per HANDOFF.md §3 ("FlagSwitch ... Add `pending` border state (amber)").

type FlagSwitchProps = React.ComponentProps<typeof Switch> & {
  pending?: boolean;
};

export function FlagSwitch({ pending, className, ...props }: FlagSwitchProps) {
  return (
    <Switch
      data-rp-pending={pending ? "true" : undefined}
      className={cn(
        "data-checked:bg-rp-red data-unchecked:bg-rp-border-hi",
        pending &&
          "ring-2 ring-rp-amber ring-offset-2 ring-offset-rp-card animate-[none] [&]:transition-none",
        className,
      )}
      {...props}
    />
  );
}
