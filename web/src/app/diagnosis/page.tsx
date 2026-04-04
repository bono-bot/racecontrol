"use client";

import { useEffect, useState, useCallback } from "react";
import {
  diagnosisApi,
  type PodHealth,
  type DebugIncident,
  type DebugDiagnosis,
  type PodDiagnosticEvent,
  type MeshSolution,
  type MeshStats,
} from "@/lib/api";

// ─── Constants ──────────────────────────────────────────────────────────────

const HEALTH_COLORS: Record<string, string> = {
  green: "bg-green-500",
  yellow: "bg-yellow-400",
  orange: "bg-orange-500",
  red: "bg-red-500",
  grey: "bg-zinc-600",
};

const TIER_LABELS: Record<number, string> = {
  1: "Deterministic",
  2: "Knowledge Base",
  3: "Single Model",
  4: "Multi-Model",
  5: "Human",
};

const STATUS_BADGE: Record<string, string> = {
  Candidate: "bg-yellow-500/20 text-yellow-400 border-yellow-500/30",
  FleetVerified: "bg-blue-500/20 text-blue-400 border-blue-500/30",
  Hardened: "bg-green-500/20 text-green-400 border-green-500/30",
  Demoted: "bg-red-500/20 text-red-400 border-red-500/30",
  Retired: "bg-zinc-500/20 text-zinc-400 border-zinc-500/30",
};

function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleTimeString("en-IN", {
      timeZone: "Asia/Kolkata",
      hour12: false,
    });
  } catch {
    return "--:--:--";
  }
}

// ─── Tabs ───────────────────────────────────────────────────────────────────

type Tab = "overview" | "incidents" | "solutions" | "events";

// ─── Page Component ─────────────────────────────────────────────────────────

export default function DiagnosisPage() {
  const [tab, setTab] = useState<Tab>("overview");
  const [pods, setPods] = useState<PodHealth[]>([]);
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);
  const [incidents, setIncidents] = useState<DebugIncident[]>([]);
  const [podEvents, setPodEvents] = useState<PodDiagnosticEvent[]>([]);
  const [solutions, setSolutions] = useState<MeshSolution[]>([]);
  const [meshStats, setMeshStats] = useState<MeshStats | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  // Incident workflow
  const [issueText, setIssueText] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [currentIncident, setCurrentIncident] = useState<DebugIncident | null>(null);
  const [diagnosis, setDiagnosis] = useState<DebugDiagnosis | null>(null);
  const [diagnosing, setDiagnosing] = useState(false);

  // ─── Data Loading ──────────────────────────────────────────────────────────

  const loadOverview = useCallback(async () => {
    try {
      const data = await diagnosisApi.debugActivity(2);
      setPods(data.pod_health.filter((p) => p.pod_number >= 1 && p.pod_number <= 8));
      setLoadError(null);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : "Failed to load");
    }
  }, []);

  const loadIncidents = useCallback(async () => {
    try {
      const res = await diagnosisApi.listDebugIncidents("open");
      setIncidents(res.incidents);
    } catch {
      /* ignore polling errors */
    }
  }, []);

  const loadPodEvents = useCallback(async () => {
    if (!selectedPodId) return;
    try {
      const res = await diagnosisApi.podDiagnosticEvents(selectedPodId, 20);
      setPodEvents(res.events);
    } catch {
      /* ignore */
    }
  }, [selectedPodId]);

  const loadSolutions = useCallback(async () => {
    try {
      const [solRes, statsRes] = await Promise.all([
        diagnosisApi.meshSolutions(),
        diagnosisApi.meshStats(),
      ]);
      setSolutions(solRes.solutions);
      setMeshStats(statsRes);
    } catch {
      /* ignore */
    }
  }, []);

  // Initial + periodic refresh
  useEffect(() => {
    loadOverview();
    loadIncidents();
    const interval = setInterval(() => {
      loadOverview();
      loadIncidents();
    }, 30_000);
    return () => clearInterval(interval);
  }, [loadOverview, loadIncidents]);

  // Pod events (when pod selected)
  useEffect(() => {
    if (!selectedPodId) return;
    loadPodEvents();
    const interval = setInterval(loadPodEvents, 15_000);
    return () => clearInterval(interval);
  }, [selectedPodId, loadPodEvents]);

  // Solutions (when tab active)
  useEffect(() => {
    if (tab === "solutions") loadSolutions();
  }, [tab, loadSolutions]);

  // ─── Incident Handlers ────────────────────────────────────────────────────

  async function handleSubmitIncident() {
    if (!issueText.trim()) return;
    setSubmitting(true);
    try {
      const res = await diagnosisApi.createDebugIncident(
        issueText.trim(),
        selectedPodId || undefined
      );
      setCurrentIncident(res.incident);
      setIssueText("");
      setDiagnosis(null);
      await loadIncidents();
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : "Failed to create");
    } finally {
      setSubmitting(false);
    }
  }

  async function handleDiagnose() {
    if (!currentIncident) return;
    setDiagnosing(true);
    try {
      const res = await diagnosisApi.diagnoseIncident(currentIncident.id);
      setDiagnosis(res);
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : "Diagnosis failed");
    } finally {
      setDiagnosing(false);
    }
  }

  async function handleApplyFix(action: string) {
    if (!currentIncident) return;
    try {
      await diagnosisApi.applyDebugFix(
        currentIncident.id,
        action,
        selectedPodId || undefined
      );
      await loadIncidents();
      await loadPodEvents();
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : "Fix failed");
    }
  }

  async function handleResolve(incidentId: string) {
    try {
      await diagnosisApi.resolveDebugIncident(incidentId, "resolved");
      setCurrentIncident(null);
      setDiagnosis(null);
      await loadIncidents();
    } catch {
      /* ignore */
    }
  }

  async function handlePromoteSolution(id: string) {
    try {
      await diagnosisApi.promoteSolution(id);
      await loadSolutions();
    } catch (e) {
      setLoadError(e instanceof Error ? e.message : "Promote failed");
    }
  }

  // ─── Render ───────────────────────────────────────────────────────────────

  return (
    <div className="min-h-screen bg-rp-black text-white p-4 md:p-6 space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-bold">Meshed Intelligence</h1>
          <p className="text-xs text-zinc-500">
            Pod Diagnosis &amp; Solution Management
          </p>
        </div>
        {loadError && (
          <p className="text-xs text-red-400 max-w-xs truncate">{loadError}</p>
        )}
      </div>

      {/* Tabs */}
      <div className="flex gap-1 border-b border-zinc-800 pb-1">
        {(["overview", "incidents", "solutions", "events"] as Tab[]).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-3 py-1.5 text-sm rounded-t transition-colors ${
              tab === t
                ? "bg-zinc-800 text-white border-b-2 border-rp-red"
                : "text-zinc-500 hover:text-zinc-300"
            }`}
          >
            {t === "overview"
              ? "Pod Health"
              : t === "incidents"
              ? "Incidents"
              : t === "solutions"
              ? "Mesh Solutions"
              : "Diagnostic Events"}
          </button>
        ))}
      </div>

      {/* ─── Tab: Overview ──────────────────────────────────────────────────── */}
      {tab === "overview" && (
        <div className="space-y-4">
          {/* Pod Grid */}
          <div className="grid grid-cols-4 md:grid-cols-8 gap-2">
            {pods
              .sort((a, b) => a.pod_number - b.pod_number)
              .map((pod) => (
                <button
                  key={pod.pod_id}
                  onClick={() =>
                    setSelectedPodId(
                      selectedPodId === pod.pod_id ? null : pod.pod_id
                    )
                  }
                  className={`p-3 rounded-lg border transition-colors text-center ${
                    selectedPodId === pod.pod_id
                      ? "border-rp-red bg-zinc-800"
                      : "border-zinc-700 bg-zinc-900 hover:border-zinc-600"
                  }`}
                >
                  <div
                    className={`w-3 h-3 rounded-full mx-auto mb-1 ${
                      HEALTH_COLORS[pod.health] || "bg-zinc-600"
                    }`}
                  />
                  <p className="text-sm font-medium">Pod {pod.pod_number}</p>
                  <p className="text-[10px] text-zinc-500">{pod.status}</p>
                </button>
              ))}
          </div>

          {/* Quick Incident Report */}
          <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4 space-y-3">
            <h3 className="text-sm font-semibold text-zinc-300">
              Report Issue
              {selectedPodId && (
                <span className="text-rp-red ml-1">
                  — Pod{" "}
                  {pods.find((p) => p.pod_id === selectedPodId)?.pod_number}
                </span>
              )}
            </h3>
            <div className="flex gap-2">
              <input
                type="text"
                value={issueText}
                onChange={(e) => setIssueText(e.target.value)}
                onKeyDown={(e) =>
                  e.key === "Enter" && handleSubmitIncident()
                }
                placeholder="Describe the issue..."
                className="flex-1 bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm focus:outline-none focus:border-rp-red"
              />
              <button
                onClick={handleSubmitIncident}
                disabled={submitting || !issueText.trim()}
                className="px-4 py-2 bg-rp-red text-white text-sm rounded hover:bg-red-700 disabled:opacity-50"
              >
                {submitting ? "..." : "Report"}
              </button>
            </div>
          </div>

          {/* Active Incident + Diagnosis */}
          {currentIncident && (
            <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4 space-y-3">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-semibold text-amber-400">
                  Active Incident: {currentIncident.category}
                </h3>
                <button
                  onClick={() => handleResolve(currentIncident.id)}
                  className="text-xs text-green-400 hover:underline"
                >
                  Mark Resolved
                </button>
              </div>
              <p className="text-xs text-zinc-400">
                {currentIncident.description}
              </p>

              {!diagnosis && (
                <button
                  onClick={handleDiagnose}
                  disabled={diagnosing}
                  className="px-3 py-1.5 bg-amber-600 text-white text-xs rounded hover:bg-amber-700 disabled:opacity-50"
                >
                  {diagnosing ? "Diagnosing..." : "Run AI Diagnosis"}
                </button>
              )}

              {diagnosis && (
                <div className="bg-zinc-800 rounded p-3 space-y-2">
                  <div className="flex justify-between items-center">
                    <span className="text-xs font-semibold text-blue-400">
                      AI Diagnosis
                    </span>
                    <span className="text-[10px] text-zinc-600 font-mono">
                      {diagnosis.model}
                    </span>
                  </div>
                  <p className="text-xs text-zinc-300">{diagnosis.diagnosis}</p>
                  {diagnosis.past_resolutions?.length > 0 && (
                    <div className="space-y-1">
                      <p className="text-[10px] text-zinc-500 font-semibold">
                        Past Resolutions:
                      </p>
                      {diagnosis.past_resolutions.map((r, i) => (
                        <div key={i} className="text-[10px] text-zinc-400">
                          {r.resolution_text} (★{r.effectiveness}/5)
                        </div>
                      ))}
                    </div>
                  )}
                  {diagnosis.playbook && (
                    <div className="space-y-1 mt-2">
                      <p className="text-[10px] text-zinc-500 font-semibold">
                        Suggested Fixes:
                      </p>
                      {diagnosis.playbook.steps.map((s) => (
                        <button
                          key={s.step_number}
                          onClick={() => handleApplyFix(s.action)}
                          className="block w-full text-left text-xs bg-zinc-700 hover:bg-zinc-600 rounded px-2 py-1"
                        >
                          {s.step_number}. {s.action}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          {/* Open Incidents List */}
          {incidents.length > 0 && (
            <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
              <h3 className="text-sm font-semibold text-zinc-300 mb-2">
                Open Incidents ({incidents.length})
              </h3>
              <div className="space-y-1">
                {incidents.slice(0, 10).map((inc) => (
                  <button
                    key={inc.id}
                    onClick={() => {
                      setCurrentIncident(inc);
                      setDiagnosis(null);
                    }}
                    className={`w-full text-left text-xs px-2 py-1.5 rounded transition-colors ${
                      currentIncident?.id === inc.id
                        ? "bg-amber-900/30 border border-amber-600/40"
                        : "bg-zinc-800 hover:bg-zinc-700"
                    }`}
                  >
                    <span className="text-zinc-500">
                      {formatTime(inc.created_at)}
                    </span>{" "}
                    <span className="text-zinc-300">{inc.description}</span>
                    {inc.pod_id && (
                      <span className="text-zinc-600 ml-1">
                        (Pod{" "}
                        {pods.find((p) => p.pod_id === inc.pod_id)?.pod_number ||
                          inc.pod_id}
                        )
                      </span>
                    )}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* ─── Tab: Incidents ─────────────────────────────────────────────────── */}
      {tab === "incidents" && (
        <div className="space-y-2">
          {incidents.length === 0 ? (
            <p className="text-sm text-zinc-500 text-center py-8">
              No open incidents
            </p>
          ) : (
            incidents.map((inc) => (
              <div
                key={inc.id}
                className="bg-zinc-900 border border-zinc-800 rounded-lg p-3 space-y-1"
              >
                <div className="flex justify-between items-center">
                  <span className="text-xs font-semibold text-amber-400">
                    {inc.category}
                  </span>
                  <span className="text-[10px] text-zinc-600">
                    {formatTime(inc.created_at)}
                  </span>
                </div>
                <p className="text-xs text-zinc-300">{inc.description}</p>
                <div className="flex gap-2 pt-1">
                  <button
                    onClick={() => {
                      setCurrentIncident(inc);
                      setDiagnosis(null);
                      setTab("overview");
                    }}
                    className="text-[10px] text-blue-400 hover:underline"
                  >
                    Diagnose
                  </button>
                  <button
                    onClick={() => handleResolve(inc.id)}
                    className="text-[10px] text-green-400 hover:underline"
                  >
                    Resolve
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      )}

      {/* ─── Tab: Mesh Solutions ────────────────────────────────────────────── */}
      {tab === "solutions" && (
        <div className="space-y-4">
          {/* Stats */}
          {meshStats && (
            <div className="grid grid-cols-2 md:grid-cols-4 gap-2">
              {[
                { label: "Total Solutions", value: meshStats.total_solutions },
                { label: "Fleet Verified", value: meshStats.fleet_verified },
                { label: "Hardened", value: meshStats.hardened },
                {
                  label: "Total Cost",
                  value: `$${meshStats.total_cost.toFixed(2)}`,
                },
              ].map((s) => (
                <div
                  key={s.label}
                  className="bg-zinc-900 border border-zinc-800 rounded-lg p-3 text-center"
                >
                  <p className="text-lg font-bold">{s.value}</p>
                  <p className="text-[10px] text-zinc-500">{s.label}</p>
                </div>
              ))}
            </div>
          )}

          {/* Solution Cards */}
          {solutions.length === 0 ? (
            <p className="text-sm text-zinc-500 text-center py-8">
              No mesh solutions yet
            </p>
          ) : (
            <div className="space-y-2">
              {solutions.map((sol) => (
                <div
                  key={sol.id}
                  className="bg-zinc-900 border border-zinc-800 rounded-lg p-3 space-y-2"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span
                        className={`text-[10px] px-2 py-0.5 rounded border ${
                          STATUS_BADGE[sol.status] || STATUS_BADGE.Candidate
                        }`}
                      >
                        {sol.status}
                      </span>
                      <span className="text-xs font-semibold text-zinc-300">
                        {sol.problem_key}
                      </span>
                    </div>
                    <span className="text-[10px] text-zinc-600">
                      Tier {sol.diagnosis_tier}
                    </span>
                  </div>
                  <p className="text-xs text-zinc-400">{sol.root_cause}</p>
                  <div className="flex items-center gap-4 text-[10px] text-zinc-500">
                    <span>
                      {sol.success_count}✓ / {sol.fail_count}✗
                    </span>
                    <span>
                      Confidence: {(sol.confidence * 100).toFixed(0)}%
                    </span>
                    <span>Cost: ${sol.cost_to_diagnose.toFixed(2)}</span>
                    <span>Fix: {sol.fix_type}</span>
                    <span>Node: {sol.source_node}</span>
                  </div>
                  {sol.tags && sol.tags.length > 0 && (
                    <div className="flex gap-1 flex-wrap">
                      {sol.tags.map((tag) => (
                        <span
                          key={tag}
                          className="text-[9px] px-1.5 py-0.5 bg-zinc-800 rounded text-zinc-500"
                        >
                          {tag}
                        </span>
                      ))}
                    </div>
                  )}
                  {sol.status === "Candidate" && (
                    <button
                      onClick={() => handlePromoteSolution(sol.id)}
                      className="text-[10px] text-blue-400 hover:underline"
                    >
                      Promote to Fleet Verified
                    </button>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* ─── Tab: Diagnostic Events ─────────────────────────────────────────── */}
      {tab === "events" && (
        <div className="space-y-4">
          {/* Pod Selector */}
          <div className="flex gap-1 flex-wrap">
            {pods
              .sort((a, b) => a.pod_number - b.pod_number)
              .map((pod) => (
                <button
                  key={pod.pod_id}
                  onClick={() => setSelectedPodId(pod.pod_id)}
                  className={`px-2 py-1 text-xs rounded ${
                    selectedPodId === pod.pod_id
                      ? "bg-rp-red text-white"
                      : "bg-zinc-800 text-zinc-400 hover:bg-zinc-700"
                  }`}
                >
                  Pod {pod.pod_number}
                </button>
              ))}
          </div>

          {!selectedPodId ? (
            <p className="text-sm text-zinc-500 text-center py-8">
              Select a pod to view diagnostic events
            </p>
          ) : podEvents.length === 0 ? (
            <p className="text-sm text-zinc-500 text-center py-8">
              No diagnostic events for this pod
            </p>
          ) : (
            <div className="space-y-1">
              {podEvents.map((evt, i) => (
                <div
                  key={`${evt.problem_hash}-${i}`}
                  className={`bg-zinc-900 border rounded-lg p-3 space-y-1 ${
                    evt.outcome === "fixed"
                      ? "border-green-800/40"
                      : evt.outcome === "failed_to_fix"
                      ? "border-red-800/40"
                      : "border-zinc-800"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span
                        className={`w-2 h-2 rounded-full ${
                          evt.outcome === "fixed"
                            ? "bg-green-500"
                            : evt.outcome === "failed_to_fix"
                            ? "bg-red-500"
                            : "bg-amber-500"
                        }`}
                      />
                      <span className="text-xs font-medium text-zinc-300">
                        {evt.trigger}
                      </span>
                    </div>
                    <span className="text-[10px] text-zinc-600">
                      {formatTime(evt.timestamp)}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 text-[10px] text-zinc-500">
                    <span>
                      Tier {evt.tier} ({TIER_LABELS[evt.tier] || "Unknown"})
                    </span>
                    <span>
                      Confidence: {(evt.confidence * 100).toFixed(0)}%
                    </span>
                    <span>{evt.fix_type}</span>
                    <span
                      className={
                        evt.source === "staff"
                          ? "text-amber-400"
                          : "text-zinc-500"
                      }
                    >
                      {evt.source}
                    </span>
                  </div>
                  <p className="text-[10px] text-zinc-400">
                    <span className="text-zinc-600">Root cause:</span>{" "}
                    {evt.root_cause}
                  </p>
                  <p className="text-[10px] text-zinc-400">
                    <span className="text-zinc-600">Action:</span> {evt.action}
                  </p>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
