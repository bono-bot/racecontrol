"use client";

import * as React from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

// ActionButton — Layer 1 wrapper around shadcn Button per Captain bundle
// 2026-05-02 components.jsx ActionButton API. Per §IV ratify block:
//   variant: primary (rp-red) / secondary (rp-card-hi) / ghost / danger
//   size:    sm (32px) / md (40px) / lg (48px) / xl (64px)

type ActionButtonVariant = "primary" | "secondary" | "ghost" | "danger";
type ActionButtonSize = "sm" | "md" | "lg" | "xl";

type ActionButtonProps = Omit<
  React.ComponentProps<typeof Button>,
  "variant" | "size"
> & {
  variant?: ActionButtonVariant;
  size?: ActionButtonSize;
};

const variantClass: Record<ActionButtonVariant, string> = {
  primary: "bg-rp-red text-rp-ink hover:bg-rp-red-glow active:bg-rp-red-deep",
  secondary:
    "bg-rp-card-hi text-rp-ink border border-rp-border-hi hover:bg-rp-border hover:border-rp-border-hi",
  ghost: "bg-transparent text-rp-ink hover:bg-rp-card-hi",
  danger: "bg-rp-red text-rp-ink hover:bg-rp-red-glow active:bg-rp-red-deep",
};

const sizeClass: Record<ActionButtonSize, string> = {
  sm: "h-8 px-3 text-xs",
  md: "h-10 px-4 text-sm",
  lg: "h-12 px-6 text-base",
  xl: "h-16 px-8 text-lg font-semibold",
};

export function ActionButton({
  className,
  variant = "primary",
  size = "md",
  ...props
}: ActionButtonProps) {
  return (
    <Button
      data-rp-variant={variant}
      data-rp-size={size}
      className={cn(
        "font-body font-semibold rounded-md uppercase tracking-wide",
        "transition-colors duration-(--motion-fast) ease-(--ease-std)",
        "border-0",
        variantClass[variant],
        sizeClass[size],
        className,
      )}
      {...props}
    />
  );
}
