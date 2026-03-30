"use client";

interface MetricCardProps {
  title: string;
  value: number | string | undefined;
  unit?: string;
  delta?: number;
  deltaLabel?: string;
  alert?: boolean;
  loading?: boolean;
}

function DeltaIndicator({ delta, label }: { delta: number; label?: string }) {
  if (delta === 0) {
    return (
      <span className="text-xs text-rp-grey">
        0 {label && <span className="ml-1">{label}</span>}
      </span>
    );
  }

  const isPositive = delta > 0;
  const colorClass = isPositive ? "text-rp-green" : "text-rp-red";
  const arrow = isPositive ? "\u25B2" : "\u25BC";
  const sign = isPositive ? "+" : "";

  return (
    <span className={`text-xs ${colorClass}`}>
      {arrow} {sign}
      {delta}
      {label && <span className="ml-1 text-rp-grey">{label}</span>}
    </span>
  );
}

export default function MetricCard({
  title,
  value,
  unit,
  delta,
  deltaLabel,
  alert = false,
  loading = false,
}: MetricCardProps) {
  if (loading) {
    return (
      <div className="bg-rp-card border border-rp-border rounded-lg p-4 animate-pulse">
        <div className="h-3 w-24 bg-rp-border rounded mb-3" />
        <div className="h-8 w-16 bg-rp-border rounded mb-2" />
        <div className="h-3 w-20 bg-rp-border rounded" />
      </div>
    );
  }

  const displayValue = value !== undefined && value !== null ? String(value) : "\u2014";

  return (
    <div
      className={`bg-rp-card border rounded-lg p-4 transition-all ${
        alert ? "border-rp-red" : "border-rp-border"
      }`}
    >
      <p className="text-xs font-medium text-rp-grey uppercase tracking-wider">
        {title}
      </p>
      <p className="text-3xl font-mono font-bold text-white mt-1">
        {unit && <span className="text-lg text-rp-grey mr-0.5">{unit}</span>}
        {displayValue}
      </p>
      {delta !== undefined && (
        <div className="mt-1">
          <DeltaIndicator delta={delta} label={deltaLabel} />
        </div>
      )}
    </div>
  );
}
