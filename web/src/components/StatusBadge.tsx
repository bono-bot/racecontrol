"use client";

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
  "waiting_for_game",
]);

// Racing flag color system (SC-01)
// green  = idle, connected, active, running
// red    = in_session, error, cancelled, cancelled_no_playable, disconnected
// amber  = pending, stopping, ended_early
// grey   = offline, completed
// blue   = launching, loading, waiting_for_game, maintenance, paused_manual, finished
// orange = paused_disconnect
// yellow = paused_game_pause

interface FlagStyle {
  bg: string;
  text: string;
  dot: string;
}

const FLAG_STYLES: Record<string, FlagStyle> = {
  green: {
    bg: "bg-rp-green/20",
    text: "text-rp-green",
    dot: "bg-rp-green",
  },
  red: {
    bg: "bg-rp-red/20",
    text: "text-rp-red",
    dot: "bg-rp-red",
  },
  amber: {
    bg: "bg-rp-yellow/20",
    text: "text-rp-yellow",
    dot: "bg-rp-yellow",
  },
  grey: {
    bg: "bg-rp-card",
    text: "text-neutral-400",
    dot: "bg-rp-grey",
  },
  blue: {
    bg: "bg-blue-900/50",
    text: "text-blue-400",
    dot: "bg-blue-400",
  },
  orange: {
    bg: "bg-orange-900/50",
    text: "text-orange-400",
    dot: "bg-orange-400",
  },
  yellow: {
    bg: "bg-rp-yellow/20",
    text: "text-rp-yellow",
    dot: "bg-rp-yellow",
  },
  purple: {
    bg: "bg-purple-900/50",
    text: "text-purple-400",
    dot: "bg-purple-400",
  },
};

const STATUS_TO_FLAG: Record<string, string> = {
  // Green: ready/active states
  idle: "green",
  connected: "green",
  active: "green",
  running: "green",

  // Red: fault/error states
  in_session: "red",
  error: "red",
  cancelled: "red",
  cancelled_no_playable: "red",
  disconnected: "red",

  // Amber: transitional states
  pending: "amber",
  stopping: "amber",
  ended_early: "amber",

  // Grey: inactive states
  offline: "grey",
  completed: "grey",

  // Blue: loading/maintenance states
  launching: "blue",
  loading: "blue",
  waiting_for_game: "blue",
  maintenance: "blue",
  paused_manual: "blue",
  finished: "blue",

  // Orange: disconnect pause
  paused_disconnect: "orange",

  // Yellow: game pause
  paused_game_pause: "yellow",
};

function getFlagStyle(status: string): FlagStyle {
  const flag = STATUS_TO_FLAG[status] ?? "grey";
  return FLAG_STYLES[flag] ?? FLAG_STYLES.grey;
}

export default function StatusBadge({ status }: { status: string }) {
  const style = getFlagStyle(status);
  const label = STATUS_LABELS[status] || status.replace(/_/g, " ");
  const isPulsing = PULSING.has(status);

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-medium ${style.bg} ${style.text}`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${style.dot} ${isPulsing ? "animate-pulse" : ""}`}
      />
      {label}
    </span>
  );
}
