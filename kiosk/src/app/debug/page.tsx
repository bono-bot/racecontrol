"use client";

import { useEffect, useState, useCallback } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { KioskHeader } from "@/components/KioskHeader";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import type {
  PodHealth,
  DebugPlaybook,
  DebugIncident,
  DebugDiagnosis,
  DebugActivityData,
} from "@/lib/types";

// ─── Helpers ─────────────────────────────────────────────────────────────────

const HEALTH_COLORS: Record<string, string> = {
  green: "bg-green-500",
  yellow: "bg-yellow-400",
  orange: "bg-orange-500",
  red: "bg-red-500",
  grey: "bg-zinc-600",
};

const EVENT_COLORS: Record<string, string> = {
  started: "text-green-400",
  stopped: "text-red-400",
  completed: "text-blue-400",
  paused: "text-yellow-400",
  resumed: "text-green-300",
  launch: "text-green-400",
  crash: "text-red-400",
  stop: "text-orange-400",
  tick: "text-zinc-500",
};

function timeAgo(dateStr: string): string {
  const now = new Date();
  const then = new Date(dateStr);
  const secs = Math.floor((now.getTime() - then.getTime()) / 1000);
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
}

// ─── Component ───────────────────────────────────────────────────────────────

export default function DebugPage() {
  const router = useRouter();
  const { connected, pods } = useKioskSocket();

  const [staffName, setStaffName] = useState<string | null>(null);
  const [activity, setActivity] = useState<DebugActivityData | null>(null);
  const [playbooks, setPlaybooks] = useState<DebugPlaybook[]>([]);
  const [incidents, setIncidents] = useState<DebugIncident[]>([]);
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);

  // Diagnostics panel state
  const [issueText, setIssueText] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [currentIncident, setCurrentIncident] = useState<DebugIncident | null>(null);
  const [currentPlaybook, setCurrentPlaybook] = useState<DebugPlaybook | null>(null);
  const [diagnosis, setDiagnosis] = useState<DebugDiagnosis | null>(null);
  const [diagnosing, setDiagnosing] = useState(false);

  // Resolve flow
  const [resolveText, setResolveText] = useState("");
  const [resolveEffectiveness, setResolveEffectiveness] = useState(3);
  const [resolving, setResolving] = useState(false);

  // Auth check
  useEffect(() => {
    if (typeof window !== "undefined") {
      const name = sessionStorage.getItem("kiosk_staff_name");
      if (!name) {
        router.push("/");
        return;
      }
      setStaffName(name);
    }
  }, [router]);

  // Load data
  const loadData = useCallback(async () => {
    try {
      const [actRes, pbRes, incRes] = await Promise.all([
        api.debugActivity(2),
        api.debugPlaybooks(),
        api.listDebugIncidents("open"),
      ]);
      setActivity(actRes);
      setPlaybooks(pbRes.playbooks || []);
      setIncidents(incRes.incidents || []);
    } catch (e) {
      console.error("Failed to load debug data:", e);
    }
  }, []);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 10000);
    return () => clearInterval(interval);
  }, [loadData]);

  // Submit incident
  async function handleSubmitIncident() {
    if (!issueText.trim()) return;
    setSubmitting(true);
    try {
      const res = await api.createDebugIncident(issueText.trim(), selectedPodId || undefined);
      setCurrentIncident(res.incident);
      setCurrentPlaybook(res.playbook || null);
      setDiagnosis(null);
      setIssueText("");
      loadData();
    } catch (e) {
      console.error("Failed to create incident:", e);
    } finally {
      setSubmitting(false);
    }
  }

  // AI diagnose
  async function handleDiagnose() {
    if (!currentIncident) return;
    setDiagnosing(true);
    try {
      const res = await api.diagnoseIncident(currentIncident.id);
      setDiagnosis(res);
      if (res.playbook) setCurrentPlaybook(res.playbook);
    } catch (e) {
      console.error("Failed to diagnose:", e);
    } finally {
      setDiagnosing(false);
    }
  }

  // Resolve
  async function handleResolve() {
    if (!currentIncident) return;
    setResolving(true);
    try {
      await api.resolveDebugIncident(
        currentIncident.id,
        "resolved",
        resolveText || undefined,
        resolveEffectiveness,
      );
      setCurrentIncident(null);
      setCurrentPlaybook(null);
      setDiagnosis(null);
      setResolveText("");
      setResolveEffectiveness(3);
      loadData();
    } catch (e) {
      console.error("Failed to resolve:", e);
    } finally {
      setResolving(false);
    }
  }

  // Dismiss
  async function handleDismiss() {
    if (!currentIncident) return;
    await api.resolveDebugIncident(currentIncident.id, "dismissed");
    setCurrentIncident(null);
    setCurrentPlaybook(null);
    setDiagnosis(null);
    loadData();
  }

  function handleSignOut() {
    sessionStorage.removeItem("kiosk_staff_name");
    sessionStorage.removeItem("kiosk_staff_id");
    router.push("/");
  }

  if (!staffName) return null;

  // Sort pod health by pod_number
  const podHealth = (activity?.pod_health || []).sort((a, b) => a.pod_number - b.pod_number);

  // Merge and sort timeline events
  const timelineEvents = [
    ...(activity?.billing_events || [])
      .filter((e) => e.event_type !== "tick")
      .map((e) => ({
        id: e.id,
        type: "billing" as const,
        event: e.event_type,
        pod_id: e.pod_id || "",
        time: e.created_at,
        detail: e.session_id,
      })),
    ...(activity?.game_events || []).map((e) => ({
      id: e.id,
      type: "game" as const,
      event: e.event_type,
      pod_id: e.pod_id,
      time: e.created_at,
      detail: e.error_message || "",
    })),
  ].sort((a, b) => b.time.localeCompare(a.time));

  return (
    <div className="min-h-screen bg-rp-black flex flex-col">
      <KioskHeader
        connected={connected}
        pods={pods}
        staffName={staffName}
        onSignOut={handleSignOut}
      />

      <div className="flex-1 grid grid-cols-12 gap-4 p-4 overflow-hidden">
        {/* ─── LEFT: Pod Health Grid ─────────────────────────────── */}
        <div className="col-span-3 flex flex-col gap-4">
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <h2 className="text-sm font-semibold text-rp-grey uppercase tracking-wider mb-4">
              Pod Health
            </h2>
            <div className="grid grid-cols-2 gap-3">
              {podHealth.map((pod) => (
                <button
                  key={pod.pod_id}
                  onClick={() =>
                    setSelectedPodId(selectedPodId === pod.pod_id ? null : pod.pod_id)
                  }
                  className={`flex flex-col items-center gap-2 p-3 rounded-lg border transition-all ${
                    selectedPodId === pod.pod_id
                      ? "border-rp-red bg-rp-red/10"
                      : "border-rp-border hover:border-zinc-500"
                  }`}
                >
                  <div className="relative">
                    <span
                      className={`block w-5 h-5 rounded-full ${
                        HEALTH_COLORS[pod.health] || "bg-zinc-600"
                      } ${pod.health === "green" ? "animate-pulse" : ""}`}
                    />
                  </div>
                  <span className="text-white font-bold text-lg">
                    {pod.pod_number}
                  </span>
                  <span className="text-xs text-rp-grey">
                    {pod.seconds_since_heartbeat > 9000
                      ? "Never"
                      : `${pod.seconds_since_heartbeat}s`}
                  </span>
                </button>
              ))}
            </div>
          </div>

          {/* Open incidents */}
          <div className="bg-rp-card border border-rp-border rounded-xl p-4 flex-1 overflow-y-auto">
            <h2 className="text-sm font-semibold text-rp-grey uppercase tracking-wider mb-3">
              Open Incidents ({incidents.length})
            </h2>
            {incidents.length === 0 ? (
              <p className="text-zinc-500 text-sm">No open incidents</p>
            ) : (
              <div className="flex flex-col gap-2">
                {incidents.map((inc) => (
                  <button
                    key={inc.id}
                    onClick={() => {
                      setCurrentIncident(inc);
                      const pb = playbooks.find((p) => p.category === inc.category);
                      setCurrentPlaybook(pb || null);
                      setDiagnosis(null);
                    }}
                    className={`text-left p-2 rounded-lg border transition-colors ${
                      currentIncident?.id === inc.id
                        ? "border-rp-red bg-rp-red/10"
                        : "border-rp-border hover:border-zinc-500"
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-mono text-rp-red">
                        {inc.category}
                      </span>
                      <span className="text-xs text-zinc-500">
                        {timeAgo(inc.created_at)}
                      </span>
                    </div>
                    <p className="text-sm text-white mt-1 truncate">
                      {inc.description}
                    </p>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>

        {/* ─── CENTER: Activity Timeline ─────────────────────────── */}
        <div className="col-span-5 bg-rp-card border border-rp-border rounded-xl p-4 overflow-y-auto">
          <h2 className="text-sm font-semibold text-rp-grey uppercase tracking-wider mb-4">
            Activity Timeline
            <span className="text-zinc-500 ml-2 font-normal">Last 2 hours</span>
          </h2>
          {timelineEvents.length === 0 ? (
            <p className="text-zinc-500 text-sm">No recent events</p>
          ) : (
            <div className="flex flex-col gap-1">
              {timelineEvents.slice(0, 150).map((ev) => (
                <div
                  key={ev.id}
                  className="flex items-center gap-3 py-1.5 border-b border-rp-border/50 last:border-0"
                >
                  <span
                    className={`w-2 h-2 rounded-full flex-shrink-0 ${
                      ev.type === "billing" ? "bg-blue-500" : "bg-purple-500"
                    }`}
                  />
                  <span className="text-xs text-zinc-500 w-14 flex-shrink-0 font-mono">
                    {timeAgo(ev.time)}
                  </span>
                  <span
                    className={`text-sm font-medium w-20 flex-shrink-0 ${
                      EVENT_COLORS[ev.event] || "text-zinc-400"
                    }`}
                  >
                    {ev.event}
                  </span>
                  <span className="text-xs text-zinc-400 flex-shrink-0">
                    {ev.pod_id ? `Pod ${ev.pod_id.slice(-1)}` : ""}
                  </span>
                  {ev.detail && ev.type === "game" && (
                    <span className="text-xs text-red-400 truncate">
                      {ev.detail}
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>

        {/* ─── RIGHT: Diagnostics Panel ──────────────────────────── */}
        <div className="col-span-4 flex flex-col gap-4 overflow-y-auto">
          {/* Issue Input */}
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <h2 className="text-sm font-semibold text-rp-grey uppercase tracking-wider mb-3">
              Report Issue
            </h2>
            <textarea
              value={issueText}
              onChange={(e) => setIssueText(e.target.value)}
              placeholder="Describe the problem... e.g. 'Pod 3 screen is blank'"
              className="w-full bg-rp-black border border-rp-border rounded-lg p-3 text-white text-sm resize-none h-20 focus:border-rp-red focus:outline-none"
            />
            <div className="flex items-center gap-3 mt-3">
              <select
                value={selectedPodId || ""}
                onChange={(e) => setSelectedPodId(e.target.value || null)}
                className="bg-rp-black border border-rp-border rounded-lg px-3 py-2 text-sm text-white focus:border-rp-red focus:outline-none"
              >
                <option value="">All Pods</option>
                {podHealth.map((p) => (
                  <option key={p.pod_id} value={p.pod_id}>
                    Pod {p.pod_number}
                  </option>
                ))}
              </select>
              <button
                onClick={handleSubmitIncident}
                disabled={!issueText.trim() || submitting}
                className="flex-1 bg-rp-red hover:bg-rp-red/80 disabled:bg-zinc-700 disabled:text-zinc-500 text-white font-semibold py-2 rounded-lg transition-colors text-sm"
              >
                {submitting ? "Submitting..." : "Submit"}
              </button>
            </div>
          </div>

          {/* Active Incident / Playbook / Diagnosis */}
          {currentIncident && (
            <div className="bg-rp-card border border-rp-border rounded-xl p-4 flex-1 overflow-y-auto">
              {/* Incident header */}
              <div className="flex items-center justify-between mb-3">
                <div>
                  <span className="text-xs font-mono text-rp-red uppercase">
                    {currentIncident.category.replace(/_/g, " ")}
                  </span>
                  <p className="text-white text-sm mt-1">
                    {currentIncident.description}
                  </p>
                </div>
                <button
                  onClick={handleDismiss}
                  className="text-xs text-zinc-500 hover:text-white border border-rp-border rounded px-2 py-1"
                >
                  Dismiss
                </button>
              </div>

              {/* Playbook steps */}
              {currentPlaybook && (
                <div className="mb-4">
                  <h3 className="text-xs font-semibold text-rp-grey uppercase tracking-wider mb-2">
                    Playbook: {currentPlaybook.title}
                  </h3>
                  <div className="flex flex-col gap-1.5">
                    {currentPlaybook.steps.map((step) => (
                      <div
                        key={step.step_number}
                        className="flex gap-2 text-sm"
                      >
                        <span className="text-rp-red font-bold w-5 flex-shrink-0">
                          {step.step_number}.
                        </span>
                        <div>
                          <span className="text-white">{step.action}</span>
                          <span className="text-zinc-500 ml-1 text-xs">
                            ({step.expected_result})
                          </span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* AI Diagnose button */}
              {!diagnosis && (
                <button
                  onClick={handleDiagnose}
                  disabled={diagnosing}
                  className="w-full bg-zinc-800 hover:bg-zinc-700 border border-rp-border text-white font-medium py-2 rounded-lg transition-colors text-sm mb-4"
                >
                  {diagnosing ? (
                    <span className="flex items-center justify-center gap-2">
                      <span className="w-4 h-4 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
                      Analyzing with AI...
                    </span>
                  ) : (
                    "AI Diagnosis"
                  )}
                </button>
              )}

              {/* AI Diagnosis result */}
              {diagnosis && (
                <div className="mb-4">
                  <div className="flex items-center justify-between mb-2">
                    <h3 className="text-xs font-semibold text-rp-grey uppercase tracking-wider">
                      AI Diagnosis
                    </h3>
                    <span className="text-xs text-zinc-600 font-mono">
                      {diagnosis.model}
                    </span>
                  </div>
                  <div className="bg-rp-black border border-rp-border rounded-lg p-3 text-sm text-zinc-300 whitespace-pre-wrap max-h-48 overflow-y-auto">
                    {diagnosis.diagnosis}
                  </div>

                  {/* Past resolutions */}
                  {diagnosis.past_resolutions.length > 0 && (
                    <div className="mt-3">
                      <h4 className="text-xs text-zinc-500 mb-1">
                        Past fixes for this category:
                      </h4>
                      {diagnosis.past_resolutions.map((r, i) => (
                        <div
                          key={i}
                          className="text-xs text-zinc-400 py-1 border-b border-rp-border/30 last:border-0"
                        >
                          <span className="text-yellow-500">
                            {"★".repeat(r.effectiveness)}
                            {"☆".repeat(5 - r.effectiveness)}
                          </span>{" "}
                          {r.resolution_text}
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {/* Resolve flow */}
              <div className="border-t border-rp-border pt-3">
                <h3 className="text-xs font-semibold text-rp-grey uppercase tracking-wider mb-2">
                  Resolve
                </h3>
                <textarea
                  value={resolveText}
                  onChange={(e) => setResolveText(e.target.value)}
                  placeholder="What fixed it? (helps future diagnosis)"
                  className="w-full bg-rp-black border border-rp-border rounded-lg p-2 text-white text-sm resize-none h-16 focus:border-rp-red focus:outline-none"
                />
                <div className="flex items-center gap-3 mt-2">
                  <div className="flex items-center gap-1">
                    {[1, 2, 3, 4, 5].map((n) => (
                      <button
                        key={n}
                        onClick={() => setResolveEffectiveness(n)}
                        className={`text-lg ${
                          n <= resolveEffectiveness
                            ? "text-yellow-500"
                            : "text-zinc-600"
                        }`}
                      >
                        ★
                      </button>
                    ))}
                  </div>
                  <button
                    onClick={handleResolve}
                    disabled={resolving}
                    className="flex-1 bg-green-600 hover:bg-green-700 disabled:bg-zinc-700 text-white font-semibold py-2 rounded-lg transition-colors text-sm"
                  >
                    {resolving ? "Saving..." : "Resolve"}
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
