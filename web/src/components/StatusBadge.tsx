export default function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    offline: "bg-rp-card text-neutral-400",
    idle: "bg-emerald-900/50 text-emerald-400",
    in_session: "bg-rp-red/20 text-rp-red",
    error: "bg-red-900/50 text-red-400",
    active: "bg-rp-red/20 text-rp-red",
    pending: "bg-rp-card text-neutral-400",
    finished: "bg-blue-900/50 text-blue-400",
    running: "bg-emerald-900/50 text-emerald-400",
    launching: "bg-amber-900/50 text-amber-400",
    stopping: "bg-amber-900/50 text-amber-400",
    connected: "bg-emerald-900/50 text-emerald-400",
    disconnected: "bg-red-900/50 text-red-400",
  };

  return (
    <span
      className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-medium ${
        colors[status] || "bg-rp-card text-neutral-400"
      }`}
    >
      <span
        className={`w-1.5 h-1.5 rounded-full ${
          status === "in_session" || status === "active" || status === "running" || status === "launching" || status === "stopping"
            ? "bg-rp-red animate-pulse"
            : status === "idle" || status === "connected"
            ? "bg-emerald-400"
            : status === "error" || status === "disconnected"
            ? "bg-red-400"
            : "bg-rp-grey"
        }`}
      />
      {status.replace(/_/g, " ")}
    </span>
  );
}
