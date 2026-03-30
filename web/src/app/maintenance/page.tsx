"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { fetchApi } from "@/lib/api";

interface MaintenanceEvent {
  id: string;
  pod_id: number | null;
  event_type: string;
  severity: string;
  component: string;
  description: string;
  detected_at: string;
  resolved_at: string | null;
  source: string;
}

interface MaintenanceSummary {
  total_events: number;
  by_severity: Record<string, number>;
  by_type: Record<string, number>;
  mttr_minutes: number;
  self_heal_rate: number;
  open_tasks: number;
}

const SEVERITY_COLORS: Record<string, string> = {
  Critical: "bg-red-500/20 text-red-400 border-red-500/30",
  High: "bg-orange-500/20 text-orange-400 border-orange-500/30",
  Medium: "bg-yellow-500/20 text-yellow-400 border-yellow-500/30",
  Low: "bg-blue-500/20 text-blue-400 border-blue-500/30",
};

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("en-IN", { timeZone: "Asia/Kolkata", hour12: false });
  } catch {
    return iso;
  }
}

export default function MaintenancePage() {
  const [summary, setSummary] = useState<MaintenanceSummary | null>(null);
  const [events, setEvents] = useState<MaintenanceEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      fetchApi<MaintenanceSummary>("/maintenance/summary").catch(() => null),
      fetchApi<{ events: MaintenanceEvent[] }>("/maintenance/events?hours=24").catch(() => ({ events: [] })),
    ])
      .then(([sum, evts]) => {
        setSummary(sum);
        setEvents(evts?.events ?? []);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, []);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Maintenance</h1>
          <p className="text-sm text-rp-grey">Pod health events and self-healing activity</p>
        </div>
        <div className="flex gap-2">
          <a
            href="/maintenance/tasks"
            className="px-3 py-1.5 text-xs font-medium bg-rp-card border border-rp-border rounded-lg text-neutral-300 hover:text-white hover:border-neutral-500 transition-colors"
          >
            Tasks
          </a>
          <a
            href="/maintenance/feedback"
            className="px-3 py-1.5 text-xs font-medium bg-rp-card border border-rp-border rounded-lg text-neutral-300 hover:text-white hover:border-neutral-500 transition-colors"
          >
            Feedback
          </a>
        </div>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading maintenance data...</div>
      ) : error ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-red-400 mb-2">Failed to load maintenance data</p>
          <p className="text-rp-grey text-sm">{error}</p>
        </div>
      ) : (
        <>
          {/* KPI Row */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
            <div className="bg-rp-card border border-rp-border rounded-lg p-4">
              <p className="text-xs text-rp-grey uppercase tracking-wider mb-1">MTTR</p>
              <p className="text-2xl font-bold text-white">
                {summary?.mttr_minutes != null ? `${summary.mttr_minutes}m` : "--"}
              </p>
              <p className="text-xs text-rp-grey mt-1">Mean time to resolve</p>
            </div>
            <div className="bg-rp-card border border-rp-border rounded-lg p-4">
              <p className="text-xs text-rp-grey uppercase tracking-wider mb-1">Self-Heal Rate</p>
              <p className="text-2xl font-bold text-green-400">
                {summary?.self_heal_rate != null ? `${Math.round(summary.self_heal_rate)}%` : "--"}
              </p>
              <p className="text-xs text-rp-grey mt-1">Auto-resolved events</p>
            </div>
            <div className="bg-rp-card border border-rp-border rounded-lg p-4">
              <p className="text-xs text-rp-grey uppercase tracking-wider mb-1">Open Tasks</p>
              <p className="text-2xl font-bold text-yellow-400">
                {summary?.open_tasks ?? "--"}
              </p>
              <p className="text-xs text-rp-grey mt-1">Pending maintenance</p>
            </div>
            <div className="bg-rp-card border border-rp-border rounded-lg p-4">
              <p className="text-xs text-rp-grey uppercase tracking-wider mb-1">Events (24h)</p>
              <p className="text-2xl font-bold text-white">
                {events.length}
              </p>
              <p className="text-xs text-rp-grey mt-1">Last 24 hours</p>
            </div>
          </div>

          {/* Event Timeline */}
          <div className="bg-rp-card border border-rp-border rounded-lg">
            <div className="px-4 py-3 border-b border-rp-border">
              <h2 className="text-sm font-semibold text-white">Recent Events</h2>
            </div>
            {events.length === 0 ? (
              <div className="p-8 text-center text-rp-grey text-sm">
                No maintenance events in the last 24 hours.
              </div>
            ) : (
              <div className="divide-y divide-rp-border">
                {events.map((evt) => (
                  <div key={evt.id} className="px-4 py-3 flex items-start gap-3">
                    <span
                      className={`mt-0.5 inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border ${SEVERITY_COLORS[evt.severity] ?? "bg-neutral-500/20 text-neutral-400 border-neutral-500/30"}`}
                    >
                      {evt.severity}
                    </span>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 mb-0.5">
                        {evt.pod_id != null && (
                          <span className="text-xs font-mono text-rp-grey">Pod {evt.pod_id}</span>
                        )}
                        <span className="text-xs text-rp-grey">{evt.component}</span>
                        <span className="text-xs text-neutral-500">{evt.event_type}</span>
                      </div>
                      <p className="text-sm text-neutral-300 truncate">{evt.description}</p>
                    </div>
                    <div className="text-right shrink-0">
                      <p className="text-xs text-rp-grey">{formatTimestamp(evt.detected_at)}</p>
                      {evt.resolved_at ? (
                        <p className="text-xs text-green-500">Resolved</p>
                      ) : (
                        <p className="text-xs text-yellow-500">Open</p>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </>
      )}
    </DashboardLayout>
  );
}
