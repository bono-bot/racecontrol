"use client";

import { useEffect, useState, useCallback } from "react";
import {
  staffDiagnosisApi,
  staffAuth,
  type PodHealth,
  type DebugIncident,
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

// ─── PIN Login Screen ───────────────────────────────────────────────────────

function StaffPinLogin({ onAuth }: { onAuth: (name: string) => void }) {
  const [pin, setPin] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function handleSubmit() {
    if (pin.length < 4) return;
    setLoading(true);
    setError(null);
    try {
      const res = await staffAuth.validatePin(pin);
      if (res.error) {
        setError(res.error);
      } else {
        // Store staff token for subsequent API calls (sessionStorage)
        // AND set cookie for server-side middleware (MMA VERIFY P1 fix)
        if (res.token && typeof window !== "undefined") {
          sessionStorage.setItem("pwa_staff_token", res.token);
          sessionStorage.setItem("pwa_staff_name", res.staff_name || "Staff");
          // Set httpOnly-like cookie for middleware validation
          // Short expiry: 4 hours (per MMA consensus for PWA staff tokens)
          document.cookie = `pwa_staff_jwt=${res.token}; path=/; max-age=14400; SameSite=Strict`;
        }
        onAuth(res.staff_name || "Staff");
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Authentication failed");
    } finally {
      setLoading(false);
      setPin("");
    }
  }

  return (
    <div className="min-h-screen bg-zinc-950 flex items-center justify-center p-4">
      <div className="bg-zinc-900 border border-zinc-800 rounded-xl p-6 w-full max-w-sm space-y-4">
        <div className="text-center">
          <h1 className="text-xl font-bold text-white">Staff Diagnosis</h1>
          <p className="text-xs text-zinc-500 mt-1">
            Enter daily staff PIN to access pod diagnostics
          </p>
        </div>
        <div className="flex gap-2 justify-center">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <div
              key={i}
              className={`w-3 h-3 rounded-full border ${
                pin.length > i
                  ? "bg-red-500 border-red-500"
                  : "border-zinc-600"
              }`}
            />
          ))}
        </div>
        <input
          type="password"
          inputMode="numeric"
          maxLength={6}
          value={pin}
          onChange={(e) => setPin(e.target.value.replace(/\D/g, ""))}
          onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
          autoFocus
          className="w-full bg-zinc-800 border border-zinc-700 rounded-lg px-4 py-3 text-center text-lg tracking-widest text-white focus:outline-none focus:border-red-500"
          placeholder="------"
        />
        {error && (
          <p className="text-xs text-red-400 text-center">{error}</p>
        )}
        <button
          onClick={handleSubmit}
          disabled={loading || pin.length < 4}
          className="w-full py-3 bg-red-600 text-white rounded-lg font-medium hover:bg-red-700 disabled:opacity-50 transition-colors"
        >
          {loading ? "Verifying..." : "Unlock Diagnostics"}
        </button>
      </div>
    </div>
  );
}

// ─── Main Diagnosis Dashboard (Read-Only) ───────────────────────────────────

export default function StaffDiagnosisPage() {
  const [staffName, setStaffName] = useState<string | null>(null);
  const [hydrated, setHydrated] = useState(false);

  // Check session on mount
  useEffect(() => {
    setHydrated(true);
    if (typeof window !== "undefined") {
      const name = sessionStorage.getItem("pwa_staff_name");
      if (name) setStaffName(name);
    }
  }, []);

  if (!hydrated) return null;

  if (!staffName) {
    return <StaffPinLogin onAuth={setStaffName} />;
  }

  return <DiagnosisDashboard staffName={staffName} onLogout={() => {
    sessionStorage.removeItem("pwa_staff_token");
    sessionStorage.removeItem("pwa_staff_name");
    // Clear server-side middleware cookie
    document.cookie = "pwa_staff_jwt=; path=/; max-age=0";
    setStaffName(null);
  }} />;
}

// ─── Dashboard Component ────────────────────────────────────────────────────

function DiagnosisDashboard({
  staffName,
  onLogout,
}: {
  staffName: string;
  onLogout: () => void;
}) {
  const [pods, setPods] = useState<PodHealth[]>([]);
  const [incidents, setIncidents] = useState<DebugIncident[]>([]);
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);
  const [podEvents, setPodEvents] = useState<PodDiagnosticEvent[]>([]);
  const [solutions, setSolutions] = useState<MeshSolution[]>([]);
  const [meshStats, setMeshStats] = useState<MeshStats | null>(null);
  const [view, setView] = useState<"pods" | "solutions">("pods");

  const loadData = useCallback(async () => {
    try {
      const [activity, incRes, solRes, statsRes] = await Promise.all([
        staffDiagnosisApi.debugActivity(2),
        staffDiagnosisApi.listIncidents("open"),
        staffDiagnosisApi.meshSolutions(),
        staffDiagnosisApi.meshStats(),
      ]);
      setPods(activity.pod_health);
      setIncidents(incRes.incidents);
      setSolutions(solRes.solutions);
      setMeshStats(statsRes);
    } catch {
      /* API error — keep showing stale data */
    }
  }, []);

  const loadPodEvents = useCallback(async () => {
    if (!selectedPodId) return;
    try {
      const res = await staffDiagnosisApi.podDiagnosticEvents(selectedPodId, 15);
      setPodEvents(res.events);
    } catch {
      /* ignore */
    }
  }, [selectedPodId]);

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 60_000); // PWA: 60s refresh per MMA consensus
    return () => clearInterval(interval);
  }, [loadData]);

  useEffect(() => {
    if (!selectedPodId) return;
    loadPodEvents();
    const interval = setInterval(loadPodEvents, 30_000);
    return () => clearInterval(interval);
  }, [selectedPodId, loadPodEvents]);

  return (
    <div className="min-h-screen bg-zinc-950 text-white p-4 space-y-4 max-w-lg mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-bold">Diagnostics</h1>
          <p className="text-[10px] text-zinc-500">
            Logged in as {staffName} — read-only
          </p>
        </div>
        <button
          onClick={onLogout}
          className="text-xs text-zinc-500 hover:text-red-400"
        >
          Sign Out
        </button>
      </div>

      {/* View Toggle */}
      <div className="flex gap-1">
        <button
          onClick={() => setView("pods")}
          className={`px-3 py-1 text-xs rounded ${
            view === "pods" ? "bg-red-600 text-white" : "bg-zinc-800 text-zinc-400"
          }`}
        >
          Pods & Incidents
        </button>
        <button
          onClick={() => setView("solutions")}
          className={`px-3 py-1 text-xs rounded ${
            view === "solutions" ? "bg-red-600 text-white" : "bg-zinc-800 text-zinc-400"
          }`}
        >
          Mesh Solutions
        </button>
      </div>

      {view === "pods" && (
        <>
          {/* Pod Grid (mobile: 4 cols) */}
          <div className="grid grid-cols-4 gap-2">
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
                  className={`p-2 rounded-lg border text-center ${
                    selectedPodId === pod.pod_id
                      ? "border-red-500 bg-zinc-800"
                      : "border-zinc-800 bg-zinc-900"
                  }`}
                >
                  <div
                    className={`w-2.5 h-2.5 rounded-full mx-auto mb-0.5 ${
                      HEALTH_COLORS[pod.health] || "bg-zinc-600"
                    }`}
                  />
                  <p className="text-xs font-medium">P{pod.pod_number}</p>
                </button>
              ))}
          </div>

          {/* Pod Events (when selected) */}
          {selectedPodId && podEvents.length > 0 && (
            <div className="space-y-1">
              <h3 className="text-xs font-semibold text-zinc-400">
                Pod {pods.find((p) => p.pod_id === selectedPodId)?.pod_number}{" "}
                — Diagnostic Events
              </h3>
              {podEvents.slice(0, 10).map((evt, i) => (
                <div
                  key={i}
                  className={`p-2 rounded border text-[10px] ${
                    evt.outcome === "fixed"
                      ? "border-green-900/40 bg-green-900/10"
                      : evt.outcome === "failed_to_fix"
                      ? "border-red-900/40 bg-red-900/10"
                      : "border-zinc-800 bg-zinc-900"
                  }`}
                >
                  <div className="flex justify-between">
                    <span className="text-zinc-300">{evt.trigger}</span>
                    <span className="text-zinc-600">
                      T{evt.tier} • {(evt.confidence * 100).toFixed(0)}%
                    </span>
                  </div>
                  <p className="text-zinc-500 mt-0.5">{evt.root_cause}</p>
                </div>
              ))}
            </div>
          )}

          {/* Open Incidents */}
          <div className="space-y-1">
            <h3 className="text-xs font-semibold text-zinc-400">
              Open Incidents ({incidents.length})
            </h3>
            {incidents.length === 0 ? (
              <p className="text-[10px] text-zinc-600 text-center py-4">
                No open incidents
              </p>
            ) : (
              incidents.slice(0, 15).map((inc) => (
                <div
                  key={inc.id}
                  className="bg-zinc-900 border border-zinc-800 rounded p-2"
                >
                  <div className="flex justify-between text-[10px]">
                    <span className="text-amber-400 font-medium">
                      {inc.category}
                    </span>
                    <span className="text-zinc-600">
                      {formatTime(inc.created_at)}
                    </span>
                  </div>
                  <p className="text-[10px] text-zinc-400 mt-0.5">
                    {inc.description}
                  </p>
                </div>
              ))
            )}
          </div>
        </>
      )}

      {view === "solutions" && (
        <>
          {/* Mesh Stats */}
          {meshStats && (
            <div className="grid grid-cols-2 gap-2">
              {[
                { label: "Solutions", value: meshStats.total_solutions },
                { label: "Verified", value: meshStats.fleet_verified },
                { label: "Hardened", value: meshStats.hardened },
                { label: "Cost", value: `$${meshStats.total_cost.toFixed(2)}` },
              ].map((s) => (
                <div
                  key={s.label}
                  className="bg-zinc-900 border border-zinc-800 rounded p-2 text-center"
                >
                  <p className="text-sm font-bold">{s.value}</p>
                  <p className="text-[9px] text-zinc-500">{s.label}</p>
                </div>
              ))}
            </div>
          )}

          {/* Solution List */}
          <div className="space-y-1">
            {solutions.length === 0 ? (
              <p className="text-[10px] text-zinc-600 text-center py-8">
                No mesh solutions
              </p>
            ) : (
              solutions.map((sol) => (
                <div
                  key={sol.id}
                  className="bg-zinc-900 border border-zinc-800 rounded p-2 space-y-1"
                >
                  <div className="flex items-center gap-1.5">
                    <span
                      className={`text-[8px] px-1 py-0.5 rounded border ${
                        sol.status === "Hardened"
                          ? "bg-green-500/20 text-green-400 border-green-500/30"
                          : sol.status === "FleetVerified"
                          ? "bg-blue-500/20 text-blue-400 border-blue-500/30"
                          : "bg-yellow-500/20 text-yellow-400 border-yellow-500/30"
                      }`}
                    >
                      {sol.status}
                    </span>
                    <span className="text-[10px] text-zinc-300 font-medium">
                      {sol.problem_key}
                    </span>
                  </div>
                  <p className="text-[10px] text-zinc-500">{sol.root_cause}</p>
                  <div className="flex gap-2 text-[9px] text-zinc-600">
                    <span>
                      {sol.success_count}ok / {sol.fail_count}fail
                    </span>
                    <span>{(sol.confidence * 100).toFixed(0)}%</span>
                    <span>{sol.fix_type}</span>
                  </div>
                </div>
              ))
            )}
          </div>
        </>
      )}
    </div>
  );
}
