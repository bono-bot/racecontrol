"use client";

interface CountdownTimerProps {
  remaining: number;
  allocated: number;
  drivingState: string;
}

function formatCountdown(seconds: number): string {
  if (seconds <= 0) return "00:00";
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

export default function CountdownTimer({
  remaining,
  allocated,
  drivingState,
}: CountdownTimerProps) {
  const used = allocated - remaining;
  const percent = allocated > 0 ? Math.min((used / allocated) * 100, 100) : 0;

  const isLow = remaining < 300; // < 5 min
  const isCritical = remaining < 60; // < 1 min

  const timeColor = isCritical
    ? "text-red-500 animate-pulse"
    : isLow
    ? "text-amber-400"
    : "text-orange-500";

  const barColor = isCritical
    ? "bg-red-500"
    : isLow
    ? "bg-amber-400"
    : "bg-orange-500";

  return (
    <div className="space-y-2">
      {/* Timer display */}
      <div className={`text-3xl font-mono font-bold text-center ${timeColor}`}>
        {formatCountdown(remaining)}
      </div>

      {/* Progress bar */}
      <div className="w-full h-2 bg-zinc-800 rounded-full overflow-hidden">
        <div
          className={`h-full rounded-full transition-all duration-1000 ${barColor}`}
          style={{ width: `${percent}%` }}
        />
      </div>

      {/* Driving state indicator */}
      <div className="flex items-center justify-center gap-2 text-xs">
        {drivingState === "active" ? (
          <>
            <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
            <span className="text-emerald-400">Driving</span>
          </>
        ) : drivingState === "idle" ? (
          <>
            <span className="w-2 h-2 rounded-full bg-zinc-500" />
            <span className="text-zinc-400">Paused (idle)</span>
          </>
        ) : (
          <>
            <span className="w-2 h-2 rounded-full bg-zinc-700" />
            <span className="text-zinc-600">No telemetry</span>
          </>
        )}
      </div>
    </div>
  );
}

export { formatCountdown };
