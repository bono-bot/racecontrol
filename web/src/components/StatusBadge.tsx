// Label overrides for statuses whose raw value is not user-friendly
const STATUS_LABELS: Record<string, string> = {
  waiting_for_game: "Loading...",
  paused_disconnect: "Disconnected",
  paused_game_pause: "Game Crashed",
  cancelled_no_playable: "Never Started",
  paused_manual: "Paused",
  in_session: "In Session",
};

// Dot animation: which statuses get the pulsing indicator
const PULSING = new Set([
  "in_session",
  "active",
  "running",
  "launching",
  "loading",
  "stopping",
  "waiting_for_game",
]);

const COLORS: Record<string, string> = {
  // Pod statuses
  offline: "bg-rp-card text-neutral-400",
  idle: "bg-emerald-900/50 text-emerald-400",
  in_session: "bg-rp-red/20 text-rp-red",
  connected: "bg-emerald-900/50 text-emerald-400",
  disconnected: "bg-red-900/50 text-red-400",
  finished: "bg-blue-900/50 text-blue-400",

  // Game states (6 variants)
  launching: "bg-blue-900/50 text-blue-400",
  loading: "bg-blue-900/50 text-blue-400",
  running: "bg-emerald-900/50 text-emerald-400",
  stopping: "bg-amber-900/50 text-amber-400",
  error: "bg-red-900/50 text-red-400",

  // Billing session statuses (10 variants)
  pending: "bg-gray-900/50 text-gray-400",
  waiting_for_game: "bg-purple-900/50 text-purple-400",
  active: "bg-emerald-900/50 text-emerald-400",
  paused_manual: "bg-blue-900/50 text-blue-400",
  paused_disconnect: "bg-orange-900/50 text-orange-400",
  paused_game_pause: "bg-yellow-900/50 text-yellow-400",
  completed: "bg-gray-900/50 text-gray-400",
  ended_early: "bg-amber-900/50 text-amber-400",
  cancelled: "bg-red-900/50 text-red-400",
  cancelled_no_playable: "bg-red-900/50 text-red-400",
};

function dotColor(status: string): string {
  if (status === "active" || status === "running" || status === "in_session") {
    return "bg-emerald-400";
  }
  if (status === "launching" || status === "loading" || status === "waiting_for_game") {
    return "bg-purple-400 animate-pulse";
  }
  if (status === "stopping") {
    return "bg-amber-400";
  }
  if (status === "error" || status === "disconnected" || status === "cancelled" || status === "cancelled_no_playable") {
    return "bg-red-400";
  }
  if (status === "paused_disconnect") {
    return "bg-orange-400";
  }
  if (status === "paused_game_pause") {
    return "bg-yellow-400";
  }
  if (status === "paused_manual") {
    return "bg-blue-400";
  }
  if (status === "idle" || status === "connected") {
    return "bg-emerald-400";
  }
  return "bg-rp-grey";
}

export default function StatusBadge({ status }: { status: string }) {
  const colorClass = COLORS[status] || "bg-rp-card text-neutral-400";
  const label = STATUS_LABELS[status] || status.replace(/_/g, " ");
  const isPulsing = PULSING.has(status);

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-medium ${colorClass}`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${dotColor(status)} ${isPulsing ? "animate-pulse" : ""}`}
      />
      {label}
    </span>
  );
}
