"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import {
  getRecentTimelines,
  getTimeline,
  type TimelineSummary,
  type TimelineDetail,
  type TimelineEvent,
} from "@/lib/api/metrics";

const simLabels: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  assetto_corsa_evo: "AC EVO",
  f1_25: "F1 25",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  forza_motorsport: "Forza Motorsport",
};

function outcomeClass(outcome: string): string {
  const lower = outcome.toLowerCase();
  if (lower.includes("success")) return "text-emerald-400";
  if (lower.includes("timeout")) return "text-amber-400";
  return "text-red-400";
}

function fmtMs(ms: number): string {
  if (ms >= 60000) return `${(ms / 60000).toFixed(1)}m`;
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`;
  return `${ms}ms`;
}

interface TimelineDetailViewProps {
  detail: TimelineDetail;
}

function TimelineDetailView({ detail }: TimelineDetailViewProps) {
  return (
    <div className="space-y-3">
      {/* Summary row */}
      <div className="flex items-center gap-4 text-xs">
        <span className={`font-semibold ${outcomeClass(detail.outcome)}`}>
          {detail.outcome}
        </span>
        <span className="text-rp-grey">
          Total: <span className="text-neutral-300 font-mono">{fmtMs(detail.total_duration_ms)}</span>
        </span>
        {detail.preset_id && (
          <span className="text-rp-grey">
            Preset: <span className="text-neutral-300 font-mono">{detail.preset_id}</span>
          </span>
        )}
        {detail.billing_session_id && (
          <span className="text-rp-grey">
            Session: <span className="text-neutral-300 font-mono">{detail.billing_session_id.slice(0, 8)}…</span>
          </span>
        )}
      </div>

      {/* Checkpoint events */}
      {detail.events.length === 0 ? (
        <p className="text-rp-grey text-xs italic">No checkpoint events recorded for this launch.</p>
      ) : (
        <div className="space-y-2">
          {detail.events.map((event: TimelineEvent, idx: number) => (
            <div key={idx} className="bg-[#1a1a1a] border border-rp-border rounded px-3 py-2">
              <dl className="grid grid-cols-2 sm:grid-cols-3 gap-x-4 gap-y-1 text-xs">
                {(Object.entries(event) as [string, unknown][]).map(([k, v]: [string, unknown]) => (
                  <div key={k} className="flex gap-1.5">
                    <dt className="text-rp-grey shrink-0">{k}:</dt>
                    <dd className="text-neutral-300 font-mono truncate">
                      {typeof v === "object" ? JSON.stringify(v) : String(v)}
                    </dd>
                  </div>
                ))}
              </dl>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default function TimelinePage() {
  const [summaries, setSummaries] = useState<TimelineSummary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<TimelineDetail | null>(null);
  const [loading, setLoading] = useState<boolean>(true);
  const [detailLoading, setDetailLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  async function loadList() {
    try {
      const data = await getRecentTimelines(50);
      setSummaries(data);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load timelines");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    loadList();
    const interval = setInterval(loadList, 30_000);
    return () => clearInterval(interval);
  }, []);

  async function handleSelect(launchId: string) {
    if (selected === launchId) {
      // Collapse
      setSelected(null);
      setDetail(null);
      return;
    }
    setSelected(launchId);
    setDetail(null);
    setDetailLoading(true);
    try {
      const d = await getTimeline(launchId);
      setDetail(d);
    } catch (e) {
      console.error("Failed to load timeline detail:", e);
    } finally {
      setDetailLoading(false);
    }
  }

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Launch Timeline Viewer</h1>
          <p className="text-sm text-rp-grey">
            Click a launch to expand checkpoint events. Auto-refreshes every 30 seconds.
          </p>
        </div>
      </div>

      {error && (
        <div className="mb-4 px-4 py-3 rounded-lg bg-red-900/20 border border-red-500/30 text-red-400 text-sm">
          {error}
        </div>
      )}

      {loading ? (
        <div className="flex items-center justify-center py-16">
          <p className="text-rp-grey text-sm animate-pulse">Loading timelines…</p>
        </div>
      ) : (
        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          {summaries.length === 0 ? (
            <div className="p-8 text-center text-rp-grey text-sm">
              No launch timeline data yet. Data appears after pods attempt game launches.
            </div>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border text-rp-grey text-xs uppercase tracking-wide">
                  <th className="px-4 py-3 text-left">Pod</th>
                  <th className="px-4 py-3 text-left">Game</th>
                  <th className="px-4 py-3 text-left">Outcome</th>
                  <th className="px-4 py-3 text-left">Duration</th>
                  <th className="px-4 py-3 text-left">Started At</th>
                  <th className="px-4 py-3 text-center w-8"></th>
                </tr>
              </thead>
              <tbody>
                {summaries.map((s) => (
                  <>
                    <tr
                      key={s.launch_id}
                      onClick={() => handleSelect(s.launch_id)}
                      className={`cursor-pointer border-b border-rp-border transition-colors hover:bg-white/5 ${
                        selected === s.launch_id ? "bg-white/5" : ""
                      }`}
                    >
                      <td className="px-4 py-3 font-mono text-neutral-300 text-xs">
                        {s.pod_id.length > 8 ? s.pod_id.slice(-8) : s.pod_id}
                      </td>
                      <td className="px-4 py-3 text-neutral-300">
                        {simLabels[s.sim_type] ?? s.sim_type}
                      </td>
                      <td className="px-4 py-3">
                        <span className={`font-medium ${outcomeClass(s.outcome)}`}>
                          {s.outcome}
                        </span>
                      </td>
                      <td className="px-4 py-3 font-mono text-neutral-400 text-xs">
                        {fmtMs(s.total_duration_ms)}
                      </td>
                      <td className="px-4 py-3 text-rp-grey text-xs">
                        {new Date(s.started_at + "Z").toLocaleString()}
                      </td>
                      <td className="px-4 py-3 text-center text-rp-grey text-xs">
                        {selected === s.launch_id ? "▲" : "▼"}
                      </td>
                    </tr>

                    {selected === s.launch_id && (
                      <tr key={`${s.launch_id}-detail`} className="bg-[#1a1a1a]">
                        <td colSpan={6} className="px-4 py-4 border-b border-rp-border">
                          {detailLoading ? (
                            <p className="text-rp-grey animate-pulse text-xs">Loading checkpoint events…</p>
                          ) : detail ? (
                            <TimelineDetailView detail={detail} />
                          ) : (
                            <p className="text-rp-grey text-xs">No detail available.</p>
                          )}
                        </td>
                      </tr>
                    )}
                  </>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </DashboardLayout>
  );
}
