"use client";

interface CountdownTimerProps {
  remaining: number;
  allocated: number;
  drivingState: string;
  compact?: boolean;
}

function formatCountdown(seconds: number): string {
  if (seconds <= 0) return "00:00";
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

const CIRCUMFERENCE = 2 * Math.PI * 40; // ~251.2

export default function CountdownTimer({
  remaining,
  allocated,
  drivingState,
  compact = false,
}: CountdownTimerProps) {
  const progress = allocated > 0 ? Math.max(0, Math.min(1, remaining / allocated)) : 0;

  const isLow = remaining < 300; // < 5 min
  const isCritical = remaining < 60; // < 1 min

  // Stroke color based on threshold
  const strokeColor = isCritical
    ? "#ef4444"
    : isLow
    ? "#f59e0b"
    : "var(--color-rp-red)";

  // Text color classes
  const textColor = isCritical
    ? "text-red-500 animate-pulse"
    : isLow
    ? "text-amber-400"
    : "text-rp-red";

  const ringSize = compact ? "w-20 h-20" : "w-28 h-28";
  const textSize = compact ? "text-base" : "text-2xl";

  return (
    <div className="space-y-2" role="timer" aria-label={`${formatCountdown(remaining)} remaining`}>
      {/* SVG Radial Ring */}
      <div className={`${ringSize} mx-auto relative`}>
        <svg viewBox="0 0 100 100" className="w-full h-full" aria-hidden="true">
          {/* Background circle */}
          <circle
            cx="50"
            cy="50"
            r="40"
            fill="none"
            stroke="#333"
            strokeWidth={8}
          />
          {/* Progress circle */}
          <circle
            cx="50"
            cy="50"
            r="40"
            fill="none"
            stroke={strokeColor}
            strokeWidth={8}
            strokeLinecap="round"
            strokeDasharray={CIRCUMFERENCE}
            strokeDashoffset={CIRCUMFERENCE * (1 - progress)}
            transform="rotate(-90 50 50)"
            className={`transition-[stroke-dashoffset] duration-1000 ease-linear ${isCritical ? "animate-pulse" : ""}`}
          />
        </svg>
        {/* Time text centered in ring */}
        <div className="absolute inset-0 flex items-center justify-center">
          <span className={`${textSize} font-mono font-bold ${textColor}`}>
            {formatCountdown(remaining)}
          </span>
        </div>
      </div>

      {/* Screen reader: announce only at key intervals (30s, 60s, 5min, 10min) */}
      {(remaining === 600 || remaining === 300 || remaining === 60 || remaining === 30 || remaining === 10) && (
        <span className="sr-only" role="status" aria-live="assertive">
          {formatCountdown(remaining)} remaining
        </span>
      )}

      {/* Driving state indicator — only in full mode */}
      {!compact && (
        <div className="flex items-center justify-center gap-2 text-xs">
          {drivingState === "active" ? (
            <>
              <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              <span className="text-emerald-400">Driving</span>
            </>
          ) : drivingState === "idle" ? (
            <>
              <span className="w-2 h-2 rounded-full bg-rp-grey" />
              <span className="text-neutral-400">Paused (idle)</span>
            </>
          ) : (
            <>
              <span className="w-2 h-2 rounded-full bg-rp-card" />
              <span className="text-rp-grey">No telemetry</span>
            </>
          )}
        </div>
      )}
    </div>
  );
}

export { formatCountdown };
