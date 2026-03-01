export default function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    offline: "bg-zinc-700 text-zinc-400",
    idle: "bg-emerald-900/50 text-emerald-400",
    in_session: "bg-orange-900/50 text-orange-400",
    error: "bg-red-900/50 text-red-400",
    active: "bg-orange-900/50 text-orange-400",
    pending: "bg-zinc-700 text-zinc-400",
    finished: "bg-blue-900/50 text-blue-400",
    running: "bg-emerald-900/50 text-emerald-400",
    connected: "bg-emerald-900/50 text-emerald-400",
    disconnected: "bg-red-900/50 text-red-400",
  };

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-medium ${
        colors[status] || "bg-zinc-700 text-zinc-400"
      }`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${
          status === "in_session" || status === "active" || status === "running"
            ? "bg-orange-400 animate-pulse"
            : status === "idle" || status === "connected"
            ? "bg-emerald-400"
            : status === "error" || status === "disconnected"
            ? "bg-red-400"
            : "bg-zinc-500"
        }`}
      />
      {status.replace(/_/g, " ")}
    </span>
  );
}
