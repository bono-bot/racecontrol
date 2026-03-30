"use client";

import type { ReactNode } from "react";

// --- Base Skeleton ---

export function Skeleton({
  className,
  style,
}: {
  className?: string;
  style?: React.CSSProperties;
}) {
  return (
    <div
      className={`animate-pulse bg-rp-border rounded ${className ?? ""}`}
      style={style}
      aria-hidden="true"
    />
  );
}

// --- SkeletonCard (matches PodCard approximate layout) ---

export function SkeletonCard() {
  return (
    <div className="bg-rp-card border border-rp-border rounded-lg p-4">
      {/* Title row */}
      <div className="flex items-center gap-2 mb-3">
        <Skeleton className="h-3 w-8" />
        <Skeleton className="h-3 w-20" />
      </div>
      {/* Content area */}
      <Skeleton className="h-6 w-full mb-3" />
      {/* Footer row */}
      <div className="flex items-center justify-between">
        <Skeleton className="h-3 w-16" />
        <Skeleton className="h-3 w-12" />
      </div>
    </div>
  );
}

// --- SkeletonRow (matches table row layout, 5 cells) ---

export function SkeletonRow() {
  return (
    <div className="flex gap-4 px-4 py-3" aria-hidden="true">
      <Skeleton className="h-4" style={{ width: "16%" }} />
      <Skeleton className="h-4" style={{ width: "28%" }} />
      <Skeleton className="h-4" style={{ width: "20%" }} />
      <Skeleton className="h-4" style={{ width: "16%" }} />
      <Skeleton className="h-4" style={{ width: "12%" }} />
    </div>
  );
}

// --- EmptyState ---

export function EmptyState({
  icon,
  headline,
  hint,
}: {
  icon: ReactNode;
  headline: string;
  hint?: string;
}) {
  return (
    <div className="flex flex-col items-center justify-center py-12 px-4">
      <div className="text-rp-grey/40 mb-3">{icon}</div>
      <p className="text-neutral-300 font-medium text-sm">{headline}</p>
      {hint && <p className="text-rp-grey text-xs mt-1">{hint}</p>}
    </div>
  );
}
