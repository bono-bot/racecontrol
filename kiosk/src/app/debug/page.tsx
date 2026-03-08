"use client";

import { useEffect, useState, useCallback, useMemo } from "react";
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
  PodActivityEntry,
} from "@/lib/types";

// ─── Constants ──────────────────────────────────────────────────────────────

const HEALTH_COLORS: Record<string, string> = {
  green: "bg-green-500",
  yellow: "bg-yellow-400",
  orange: "bg-orange-500",
  red: "bg-red-500",
  grey: "bg-zinc-600",
};

const CATEGORY_DOT: Record<string, string> = {
  system: "bg-zinc-400",
  game: "bg-purple-500",
  billing: "bg-blue-500",
  auth: "bg-green-500",
  race_engineer: "bg-amber-500",
};

function formatTime(isoStr: string): string {
  try {
    const d = new Date(isoStr);
    return d.toLocaleTimeString("en-IN", { hour12: false });
  } catch {
    return "--:--:--";
  }
}

function getPodLabel(podNumber: number, podId: string): string {
  if (podNumber > 0) return `Pod ${podNumber}`;
  const last = podId.replace("pod_", "");
  return `Pod ${last}`;
}

// ─── Component ──────────────────────────────────────────────────────────────

export default function DebugPage() {
  const router = useRouter();
  const { connected, pods, activityLog } = useKioskSocket();

  const [staffName, setStaffName] = useState<string | null>(null);
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);
  const [activity, setActivity] = useState<DebugActivityData | null>(null);
  const [playbooks, setPlaybooks] = useState<DebugPlaybook[]>([]);
  const [incidents, setIncidents] = useState<DebugIncident[]>([]);

  // Diagnostics panel
  const [issueText, setIssueText] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [currentIncident, setCurrentIncident] = useState<DebugIncident | null>(null);
  const [currentPlaybook, setCurrentPlaybook] = useState<DebugPlaybook | null>(null);
  const [diagnosis, setDiagnosis] = useState<DebugDiagnosis | null>(null);
  const [diagnosing, setDiagnosing] = useState(false);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);

  // Resolve flow
  const [resolveText, setResolveText] = useState("");
  const [resolveEffectiveness, setResolveEffectiveness] = useState(3);
  const [resolving, setResolving] = useState(false);

  // Auth
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

  // Load debug data (incidents, playbooks, pod health)
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
    const interval = setInterval(loadData, 30000); // refresh incidents less often
    return () => clearInterval(interval);
  }, [loadData]);

  // Filtered activity
  const filteredActivity = useMemo(() => {
    if (!selectedPodId) return activityLog;
    return activityLog.filter((e) => e.pod_id === selectedPodId);
  }, [activityLog, selectedPodId]);

  // Pod health from old debug data
  const podHealth = useMemo(
    () => (activity?.pod_health || []).sort((a, b) => a.pod_number - b.pod_number),
    [activity]
  );

  // Build pod list from WS pods for grid
  const podList = useMemo(() => {
    return Array.from(pods.values()).sort((a, b) => a.number - b.number);
  }, [pods]);

  // ─── Handlers ─────────────────────────────────────────────────────────────

  async function handleSubmitIncident() {
    if (!issueText.trim()) return;
    setSubmitting(true);
    try {
      const res = await api.createDebugIncident(issueText.trim(), selectedPodId || undefined);
      setCurrentIncident(res.incident);
      setCurrentPlaybook(res.playbook || null);
      setDiagnosis(null);
      setIssueText("");
      setDiagnosticsOpen(true);
      loadData();
    } catch (e) {
      console.error("Failed to create incident:", e);
    } finally {
      setSubmitting(false);
    }
  }

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

  // ─── Render ───────────────────────────────────────────────────────────────

  return (
    <div className="min-h-screen bg-rp-black flex flex-col">
      <KioskHeader
        connected={connected}
        pods={pods}
        staffName={staffName}
        onSignOut={handleSignOut}
      />

      <div className="flex-1 flex gap-4 p-4 overflow-hidden">
        {/* ─── LEFT SIDEBAR: Pod Grid ──────────────────────────────── */}
        <div className="w-48 flex-shrink-0 flex flex-col gap-3">
          <div className="bg-rp-card border border-rp-border rounded-xl p-3">
            <h2 className="text-xs font-semibold text-rp-grey uppercase tracking-wider mb-3">
              Pods
            </h2>
            <div className="grid grid-cols-2 gap-2">
              {(podHealth.length > 0 ? podHealth : podList.map(p => ({
                pod_id: p.id,
                pod_number: p.number,
                seconds_since_heartbeat: 0,
                health: p.status === "idle" || p.status === "in_session" ? "green" : p.status === "offline" ? "grey" : "red",
                status: p.status,
              } as PodHealth))).map((pod) => (
                <button
                  key={pod.pod_id}
                  onClick={() =>
                    setSelectedPodId(selectedPodId === pod.pod_id ? null : pod.pod_id)
                  }
                  className={`flex flex-col items-center gap-1 p-2 rounded-lg border transition-all ${
                    selectedPodId === pod.pod_id
                      ? "border-[#E10600] bg-[#E10600]/10"
                      : "border-rp-border hover:border-zinc-500"
                  }`}
                >
                  <span
                    className={`block w-3 h-3 rounded-full ${
                      HEALTH_COLORS[pod.health] || "bg-zinc-600"
                    } ${pod.health === "green" ? "animate-pulse" : ""}`}
                  />
                  <span className="text-white font-bold text-sm">
                    {pod.pod_number}
                  </span>
                </button>
              ))}
            </div>

            {/* All Pods button */}
            <button
              onClick={() => setSelectedPodId(null)}
              className={`w-full mt-2 text-xs py-1.5 rounded-lg border transition-colors ${
                selectedPodId === null
                  ? "border-[#E10600] bg-[#E10600]/10 text-white"
                  : "border-rp-border text-zinc-400 hover:text-white"
              }`}
            >
              All Pods
            </button>
          </div>

          {/* Open incidents count */}
          {incidents.length > 0 && (
            <button
              onClick={() => setDiagnosticsOpen(true)}
              className="bg-rp-card border border-rp-border rounded-xl p-3 text-left hover:border-zinc-500 transition-colors"
            >
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-rp-grey uppercase">
                  Incidents
                </span>
                <span className="bg-[#E10600] text-white text-xs font-bold px-2 py-0.5 rounded-full">
                  {incidents.length}
                </span>
              </div>
            </button>
          )}
        </div>

        {/* ─── MAIN: Live Activity Feed ────────────────────────────── */}
        <div className="flex-1 flex flex-col gap-4 min-w-0">
          {/* Header */}
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold text-rp-grey uppercase tracking-wider">
              Live Activity
              {selectedPodId && (
                <span className="text-white ml-2 normal-case">
                  — {getPodLabel(
                    podHealth.find((p) => p.pod_id === selectedPodId)?.pod_number || 0,
                    selectedPodId
                  )}
                </span>
              )}
            </h2>
            <div className="flex items-center gap-2">
              <span
                className={`w-2 h-2 rounded-full ${
                  connected ? "bg-green-500 animate-pulse" : "bg-red-500"
                }`}
              />
              <span className="text-xs text-zinc-500">
                {connected ? "Live" : "Disconnected"}
              </span>
              <span className="text-xs text-zinc-600 ml-2">
                {filteredActivity.length} events
              </span>
            </div>
          </div>

          {/* Activity Feed */}
          <div className="flex-1 bg-rp-card border border-rp-border rounded-xl overflow-y-auto">
            {filteredActivity.length === 0 ? (
              <div className="flex items-center justify-center h-full text-zinc-500 text-sm">
                No activity yet — events will appear in real-time
              </div>
            ) : (
              <div className="flex flex-col">
                {filteredActivity.map((entry) => {
                  const isRaceEngineer = entry.source === "race_engineer" || entry.category === "race_engineer";
                  return (
                    <div
                      key={entry.id}
                      className={`flex items-start gap-3 px-4 py-2 border-b border-rp-border/30 last:border-0 border-l-2 ${
                        isRaceEngineer
                          ? "border-l-amber-500 bg-amber-500/5"
                          : "border-l-transparent"
                      }`}
                    >
                      {/* Time */}
                      <span className="text-xs text-zinc-500 font-mono w-16 flex-shrink-0 pt-0.5">
                        {formatTime(entry.timestamp)}
                      </span>

                      {/* Category dot */}
                      <span
                        className={`w-2 h-2 rounded-full flex-shrink-0 mt-1.5 ${
                          CATEGORY_DOT[entry.category] || "bg-zinc-500"
                        }`}
                      />

                      {/* Pod label */}
                      {!selectedPodId && entry.pod_number > 0 && (
                        <button
                          onClick={() => setSelectedPodId(entry.pod_id)}
                          className="text-xs text-zinc-400 hover:text-white w-12 flex-shrink-0 pt-0.5 text-left"
                        >
                          Pod {entry.pod_number}
                        </button>
                      )}

                      {/* Action + Details */}
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          {isRaceEngineer && (
                            <span className="text-amber-500 text-xs font-semibold flex items-center gap-1">
                              <svg className="w-3 h-3" fill="currentColor" viewBox="0 0 20 20">
                                <path fillRule="evenodd" d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z" clipRule="evenodd" />
                              </svg>
                              Race Engineer
                            </span>
                          )}
                          <span
                            className={`text-sm font-medium ${
                              isRaceEngineer ? "text-amber-400" : "text-white"
                            }`}
                          >
                            {entry.action}
                          </span>
                        </div>
                        {entry.details && (
                          <p className="text-xs text-zinc-500 mt-0.5 truncate">
                            {entry.details}
                          </p>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* ─── Diagnostics Section (collapsible) ───────────────── */}
          <div className="bg-rp-card border border-rp-border rounded-xl">
            <button
              onClick={() => setDiagnosticsOpen(!diagnosticsOpen)}
              className="w-full flex items-center justify-between p-3 text-left"
            >
              <h2 className="text-xs font-semibold text-rp-grey uppercase tracking-wider">
                Report Issue / Diagnostics
              </h2>
              <svg
                className={`w-4 h-4 text-zinc-500 transition-transform ${
                  diagnosticsOpen ? "rotate-180" : ""
                }`}
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
              </svg>
            </button>

            {diagnosticsOpen && (
              <div className="px-3 pb-3 space-y-3">
                {/* Issue Input */}
                <div className="flex gap-2">
                  <textarea
                    value={issueText}
                    onChange={(e) => setIssueText(e.target.value)}
                    placeholder="Describe the problem... e.g. 'Pod 3 screen is blank'"
                    className="flex-1 bg-rp-black border border-rp-border rounded-lg p-2 text-white text-sm resize-none h-16 focus:border-[#E10600] focus:outline-none"
                  />
                  <div className="flex flex-col gap-2">
                    <select
                      value={selectedPodId || ""}
                      onChange={(e) => setSelectedPodId(e.target.value || null)}
                      className="bg-rp-black border border-rp-border rounded-lg px-2 py-1 text-xs text-white focus:border-[#E10600] focus:outline-none"
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
                      className="bg-[#E10600] hover:bg-[#E10600]/80 disabled:bg-zinc-700 disabled:text-zinc-500 text-white font-semibold py-1.5 px-4 rounded-lg transition-colors text-xs"
                    >
                      {submitting ? "..." : "Submit"}
                    </button>
                  </div>
                </div>

                {/* Active Incident / Playbook / Diagnosis */}
                {currentIncident && (
                  <div className="border border-rp-border rounded-lg p-3 space-y-3">
                    {/* Incident header */}
                    <div className="flex items-center justify-between">
                      <div>
                        <span className="text-xs font-mono text-[#E10600] uppercase">
                          {currentIncident.category.replace(/_/g, " ")}
                        </span>
                        <p className="text-sm text-white mt-0.5">
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

                    {/* Playbook */}
                    {currentPlaybook && (
                      <div>
                        <h3 className="text-xs font-semibold text-rp-grey uppercase tracking-wider mb-1">
                          Playbook: {currentPlaybook.title}
                        </h3>
                        <div className="flex flex-col gap-1">
                          {currentPlaybook.steps.map((step) => (
                            <div key={step.step_number} className="flex gap-2 text-xs">
                              <span className="text-[#E10600] font-bold w-4 flex-shrink-0">
                                {step.step_number}.
                              </span>
                              <span className="text-white">{step.action}</span>
                              <span className="text-zinc-500">({step.expected_result})</span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}

                    {/* Ask Race Engineer button */}
                    {!diagnosis && (
                      <button
                        onClick={handleDiagnose}
                        disabled={diagnosing}
                        className="w-full bg-amber-600/20 hover:bg-amber-600/30 border border-amber-600/50 text-amber-400 font-medium py-2 rounded-lg transition-colors text-sm flex items-center justify-center gap-2"
                      >
                        {diagnosing ? (
                          <>
                            <span className="w-4 h-4 border-2 border-amber-500 border-t-transparent rounded-full animate-spin" />
                            Analyzing...
                          </>
                        ) : (
                          <>
                            <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                              <path fillRule="evenodd" d="M11.49 3.17c-.38-1.56-2.6-1.56-2.98 0a1.532 1.532 0 01-2.286.948c-1.372-.836-2.942.734-2.106 2.106.54.886.061 2.042-.947 2.287-1.561.379-1.561 2.6 0 2.978a1.532 1.532 0 01.947 2.287c-.836 1.372.734 2.942 2.106 2.106a1.532 1.532 0 012.287.947c.379 1.561 2.6 1.561 2.978 0a1.533 1.533 0 012.287-.947c1.372.836 2.942-.734 2.106-2.106a1.533 1.533 0 01.947-2.287c1.561-.379 1.561-2.6 0-2.978a1.532 1.532 0 01-.947-2.287c.836-1.372-.734-2.942-2.106-2.106a1.532 1.532 0 01-2.287-.947zM10 13a3 3 0 100-6 3 3 0 000 6z" clipRule="evenodd" />
                            </svg>
                            Ask Race Engineer
                          </>
                        )}
                      </button>
                    )}

                    {/* Diagnosis result */}
                    {diagnosis && (
                      <div>
                        <div className="flex items-center justify-between mb-1">
                          <h3 className="text-xs font-semibold text-amber-500 uppercase tracking-wider">
                            Race Engineer Diagnosis
                          </h3>
                          <span className="text-xs text-zinc-600 font-mono">{diagnosis.model}</span>
                        </div>
                        <div className="bg-rp-black border border-amber-600/30 rounded-lg p-2 text-xs text-zinc-300 whitespace-pre-wrap max-h-36 overflow-y-auto">
                          {diagnosis.diagnosis}
                        </div>

                        {diagnosis.past_resolutions.length > 0 && (
                          <div className="mt-2">
                            <h4 className="text-xs text-zinc-500 mb-1">Past fixes:</h4>
                            {diagnosis.past_resolutions.map((r, i) => (
                              <div key={i} className="text-xs text-zinc-400 py-0.5">
                                <span className="text-yellow-500">
                                  {"★".repeat(r.effectiveness)}{"☆".repeat(5 - r.effectiveness)}
                                </span>{" "}
                                {r.resolution_text}
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    )}

                    {/* Resolve flow */}
                    <div className="border-t border-rp-border pt-2">
                      <textarea
                        value={resolveText}
                        onChange={(e) => setResolveText(e.target.value)}
                        placeholder="What fixed it?"
                        className="w-full bg-rp-black border border-rp-border rounded-lg p-2 text-white text-xs resize-none h-12 focus:border-[#E10600] focus:outline-none"
                      />
                      <div className="flex items-center gap-2 mt-1">
                        <div className="flex items-center gap-0.5">
                          {[1, 2, 3, 4, 5].map((n) => (
                            <button
                              key={n}
                              onClick={() => setResolveEffectiveness(n)}
                              className={`text-sm ${
                                n <= resolveEffectiveness ? "text-yellow-500" : "text-zinc-600"
                              }`}
                            >
                              ★
                            </button>
                          ))}
                        </div>
                        <button
                          onClick={handleResolve}
                          disabled={resolving}
                          className="flex-1 bg-green-600 hover:bg-green-700 disabled:bg-zinc-700 text-white font-semibold py-1.5 rounded-lg transition-colors text-xs"
                        >
                          {resolving ? "Saving..." : "Resolve"}
                        </button>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
