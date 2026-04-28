"use client";

import { useEffect, useState, useCallback, useMemo, useRef } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api";
import { KioskHeader } from "@/components/KioskHeader";
import LaunchCard from "@/components/LaunchCard";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import type {
  Pod,
  PodHealth,
  DebugPlaybook,
  DebugIncident,
  DebugDiagnosis,
  DebugActivityData,
  PodActivityEntry,
  PodDiagnosticEvent,
  Lap,
  TelemetryFrame,
  BillingSession,
  PodStateProbe,
  HaloProbe,
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
    return d.toLocaleTimeString("en-IN", { timeZone: "Asia/Kolkata", hour12: false });
  } catch {
    return "--:--:--";
  }
}

function getPodLabel(podNumber: number, podId: string, nodeType?: string, name?: string): string {
  if (nodeType === 'pos') return name || 'POS';
  if (podNumber > 0) return `Pod ${podNumber}`;
  const last = podId.replace("pod_", "");
  return `Pod ${last}`;
}

// ─── Component ──────────────────────────────────────────────────────────────

export default function DebugPage() {
  const router = useRouter();
  const { connected, pods, activityLog, launches, launchNotes, removeLaunch, recentLaps, billingTimers, latestTelemetry } = useKioskSocket();

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
  const [diagnoseError, setDiagnoseError] = useState<string | null>(null);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);

  // v27.0: Pod diagnostic events from tier engine
  const [podEvents, setPodEvents] = useState<PodDiagnosticEvent[]>([]);
  const [podEventsLoading, setPodEventsLoading] = useState(false);

  // Probes (read-only diagnostics)
  const [podProbe, setPodProbe] = useState<PodStateProbe | null>(null);
  const [probeLoading, setProbeLoading] = useState(false);
  const [postFixProbe, setPostFixProbe] = useState<PodStateProbe | null>(null);
  const [haloCatalog, setHaloCatalog] = useState<HaloProbe[]>([]);
  const [haloByStatus, setHaloByStatus] = useState<Record<string, number>>({});
  const [fleetMiEvents, setFleetMiEvents] = useState<Map<string, PodDiagnosticEvent[]>>(new Map());

  // Quick fix
  const [applyingFix, setApplyingFix] = useState<string | null>(null);
  const [fixResult, setFixResult] = useState<{ ok: boolean; action?: string; error?: string } | null>(null);
  const [suggestedActions, setSuggestedActions] = useState<string[]>([]);

  // Resolve flow
  const [resolveText, setResolveText] = useState("");
  const [resolveEffectiveness, setResolveEffectiveness] = useState(3);
  const [resolving, setResolving] = useState(false);

  // Auth
  useEffect(() => {
    if (typeof window !== "undefined") {
      const name = sessionStorage.getItem("kiosk_staff_name");
      if (!name) {
        router.push("/staff");
        return;
      }
      setStaffName(name);
    }
  }, [router]);

  // Phase 368: feature flag — kiosk_launch_cards_enabled
  const [launchCardsEnabled, setLaunchCardsEnabled] = useState(false);
  useEffect(() => {
    const load = async () => {
      try {
        const flags = await api.listFlags();
        const flag = flags.find((f) => f.name === "kiosk_launch_cards_enabled");
        setLaunchCardsEnabled(flag?.enabled ?? false);
      } catch (e) {
        console.warn("[debug] failed to load feature flags", e);
        setLaunchCardsEnabled(false);
      }
    };
    void load();
    const i = setInterval(load, 60000); // 60s re-fetch per Phase 177+ boot-resilience pattern
    return () => clearInterval(i);
  }, []);

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
      setLoadError(null);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("Failed to load debug data:", msg);
      setLoadError(msg);
    }
  }, []);

  // D-14: Remove 30s poll only when flag=true AND WS connected. Retain as fallback otherwise.
  const shouldPoll = !launchCardsEnabled || !connected;
  useEffect(() => {
    loadData();
    if (!shouldPoll) return;
    const interval = setInterval(loadData, 30000); // refresh incidents less often
    return () => clearInterval(interval);
  }, [loadData, shouldPoll]);

  // v27.0: Load pod diagnostic events when a pod is selected
  useEffect(() => {
    if (!selectedPodId) {
      setPodEvents([]);
      return;
    }
    let cancelled = false;
    async function loadPodEvents() {
      setPodEventsLoading(true);
      try {
        const res = await api.podDiagnosticEvents(selectedPodId!, 10);
        if (!cancelled) setPodEvents(res.events || []);
      } catch {
        if (!cancelled) setPodEvents([]);
      } finally {
        if (!cancelled) setPodEventsLoading(false);
      }
    }
    loadPodEvents();
    // Same customer-active gate as podProbe — pod-events proxies through
    // rc-agent /events/recent on :8090 so it shares blast radius.
    // (predicate inlined here because isPodCustomerActive is declared further
    // down the component body; useEffect deps array hits TDZ otherwise)
    const billing = billingTimers.get(selectedPodId);
    const billingActive = !!billing && (
      billing.status === "active" ||
      billing.status === "waiting_for_game" ||
      billing.status === "paused_manual" ||
      billing.status === "paused_disconnect" ||
      billing.status === "paused_game_pause" ||
      billing.status === "paused_crash_recovery"
    );
    const pod = pods.get(selectedPodId);
    const driving = pod?.driving_state === "active";
    const gameActive = pod?.game_state && pod.game_state !== "idle" && pod.game_state !== "error";
    const customerActive = Boolean(billingActive || driving || gameActive);
    if (customerActive) return () => { cancelled = true; };
    const interval = setInterval(loadPodEvents, 15000);
    return () => { cancelled = true; clearInterval(interval); };
  }, [selectedPodId, pods, billingTimers]);

  // Customer-active predicate — single source of truth for "is this pod
  // currently being used by a customer?" Used to gate background pod-probing
  // operations (probe poll, pod-events poll, fleet-wide MI poll) so we don't
  // add /exec or pod-proxy traffic during a session — especially during
  // game-switch transitions (AC→F1 25) where rc-agent is busy spawning
  // processes and managing windows.
  const isPodCustomerActive = useCallback((podId: string): boolean => {
    const pod = pods.get(podId);
    const billing = billingTimers.get(podId);
    // Any non-terminal billing state means a customer paid for this pod and
    // is mid-session (or about to be). Terminal states: completed, ended_early,
    // cancelled, cancelled_no_playable.
    const billingActive = !!billing && (
      billing.status === "active" ||
      billing.status === "waiting_for_game" ||
      billing.status === "paused_manual" ||
      billing.status === "paused_disconnect" ||
      billing.status === "paused_game_pause" ||
      billing.status === "paused_crash_recovery"
    );
    const driving = pod?.driving_state === "active";
    // Transitional game states are the highest-risk window — rc-agent is
    // actively spawning/closing processes. Steady "running" is also customer-
    // active but lower risk; we treat all non-idle game states as active.
    const gameActive = pod?.game_state && pod.game_state !== "idle" && pod.game_state !== "error";
    return Boolean(billingActive || driving || gameActive);
  }, [pods, billingTimers]);

  // Memo of the predicate's value for the currently selected pod (for badge).
  const probeAutoPaused = useMemo(() => {
    if (!selectedPodId) return false;
    return isPodCustomerActive(selectedPodId);
  }, [selectedPodId, isPodCustomerActive]);

  useEffect(() => {
    if (!selectedPodId) {
      setPodProbe(null);
      return;
    }
    let cancelled = false;
    async function loadProbe() {
      setProbeLoading(true);
      try {
        const res = await api.podStateProbe(selectedPodId!);
        if (!cancelled) setPodProbe(res);
      } catch (e) {
        if (!cancelled) console.warn("[debug] pod probe failed", e);
      } finally {
        if (!cancelled) setProbeLoading(false);
      }
    }
    // Always do an initial probe on pod-select — but only auto-refresh if the
    // pod is not customer-active. This gives staff one snapshot at select time
    // without poking the pod every 30s during a session.
    loadProbe();
    if (probeAutoPaused) return () => { cancelled = true; };
    const iv = setInterval(loadProbe, 30000);
    return () => { cancelled = true; clearInterval(iv); };
  }, [selectedPodId, probeAutoPaused]);

  // HALO catalog — load once at mount
  useEffect(() => {
    let cancelled = false;
    async function loadCatalog() {
      try {
        const res = await api.haloCatalog();
        if (!cancelled) {
          setHaloCatalog(res.probes || []);
          setHaloByStatus(res.by_status || {});
        }
      } catch (e) {
        console.warn("[debug] halo catalog load failed", e);
      }
    }
    loadCatalog();
    return () => { cancelled = true; };
  }, []);

  // Fleet-wide MI events when no pod selected — fetch only the pods that are
  // NOT in a customer-active state. Skipping customer-active pods avoids
  // adding /exec or pod-proxy traffic during a session.
  useEffect(() => {
    if (selectedPodId) return; // pod-specific panel handles single-pod view
    if (pods.size === 0) return;
    let cancelled = false;
    async function loadFleetEvents() {
      const ids: string[] = [];
      for (const id of pods.keys()) {
        if (isPodCustomerActive(id)) continue;
        ids.push(id);
      }
      if (ids.length === 0) {
        setFleetMiEvents(new Map());
        return;
      }
      const results = await Promise.allSettled(
        ids.map((id) => api.podDiagnosticEvents(id, 5).then((r) => ({ id, events: r.events || [] })))
      );
      if (cancelled) return;
      const map = new Map<string, PodDiagnosticEvent[]>();
      for (const r of results) {
        if (r.status === "fulfilled" && r.value.events.length > 0) {
          map.set(r.value.id, r.value.events);
        }
      }
      setFleetMiEvents(map);
    }
    loadFleetEvents();
    const iv = setInterval(loadFleetEvents, 30000);
    return () => { cancelled = true; clearInterval(iv); };
  }, [selectedPodId, pods, isPodCustomerActive]);

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
    setDiagnoseError(null);
    try {
      // Auto-context-snapshot: capture probe + WS state at submit time so
      // Race Engineer diagnoses with full context, not just the description.
      let clientContext: Record<string, unknown> | undefined = undefined;
      if (selectedPodId) {
        // Best-effort live probe at submit (separate from the polled probe state).
        let liveProbe: PodStateProbe | null = null;
        try { liveProbe = await api.podStateProbe(selectedPodId); } catch { /* tolerate */ }
        const tele = latestTelemetry.get(selectedPodId) || null;
        const billing = billingTimers.get(selectedPodId) || null;
        const recentActivity = activityLog
          .filter((e) => e.pod_id === selectedPodId)
          .slice(0, 10);
        clientContext = {
          probe: liveProbe || podProbe,
          telemetry: tele,
          billing_session: billing,
          recent_activity: recentActivity,
          recent_pod_events: podEvents.slice(0, 5),
          ws_connected: connected,
        };
      }
      const res = await api.createDebugIncident(
        issueText.trim(),
        selectedPodId || undefined,
        clientContext,
      ) as {
        incident: DebugIncident;
        playbook?: DebugPlaybook;
        suggested_actions?: string[];
        auto_fix?: { ok: boolean; action?: string; error?: string };
      };
      setSuggestedActions(res.suggested_actions || []);
      setDiagnosis(null);
      setIssueText("");
      setDiagnosticsOpen(true);

      // If auto-fix was applied and succeeded, show result and clear incident
      if (res.auto_fix?.ok) {
        setFixResult({ ok: true, action: res.auto_fix.action });
        setCurrentIncident(null);
        setCurrentPlaybook(null);
      } else if (res.auto_fix && !res.auto_fix.ok) {
        // Auto-fix was attempted but failed — show error, keep incident open for manual retry
        setFixResult({ ok: false, error: res.auto_fix.error || "Auto-fix failed" });
        setCurrentIncident(res.incident);
        setCurrentPlaybook(res.playbook || null);
      } else {
        // No auto-fix attempted (display-only category or no pod selected)
        setFixResult(null);
        setCurrentIncident(res.incident);
        setCurrentPlaybook(res.playbook || null);
      }
      loadData();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("Failed to create incident:", msg);
      setDiagnoseError(`Failed to submit: ${msg}`);
    } finally {
      setSubmitting(false);
    }
  }

  async function handleDiagnose() {
    if (!currentIncident) return;
    setDiagnosing(true);
    setDiagnoseError(null);
    try {
      const res = await api.diagnoseIncident(currentIncident.id);
      // Backend returns { error: "..." } with HTTP 200 on failure
      if ((res as unknown as { error?: string }).error) {
        setDiagnoseError((res as unknown as { error: string }).error);
        return;
      }
      setDiagnosis(res);
      if (res.playbook) setCurrentPlaybook(res.playbook);
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      console.error("Failed to diagnose:", msg);
      setDiagnoseError(msg);
    } finally {
      setDiagnosing(false);
    }
  }

  async function handleApplyFix(action: string) {
    if (!currentIncident) return;
    setApplyingFix(action);
    setFixResult(null);
    setPostFixProbe(null);
    try {
      const res = await api.applyDebugFix(currentIncident.id, action, selectedPodId || undefined);
      setFixResult(res);
      if (res.ok) {
        // Post-fix verification probe: rc-agent ok:true means the action was
        // dispatched, NOT that the symptom is gone. Wait briefly for the fix
        // to take effect, then re-probe so staff sees the actual delta.
        const probePodId = selectedPodId || currentIncident.pod_id || undefined;
        if (probePodId) {
          await new Promise((r) => setTimeout(r, 4000));
          try {
            const after = await api.podStateProbe(probePodId);
            setPostFixProbe(after);
            setPodProbe(after); // refresh the panel display too
          } catch (e) {
            console.warn("[debug] post-fix probe failed", e);
          }
        }
        // Backend auto-resolves the incident on ok:true. Keep current behavior
        // but leave fixResult+postFixProbe rendered so staff can sanity-check.
        setCurrentIncident(null);
        setCurrentPlaybook(null);
        setDiagnosis(null);
        setDiagnoseError(null);
        loadData();
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setFixResult({ ok: false, error: msg });
    } finally {
      setApplyingFix(null);
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
      setDiagnoseError(null);
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
    try {
      await api.resolveDebugIncident(currentIncident.id, "dismissed");
      setCurrentIncident(null);
      setCurrentPlaybook(null);
      setDiagnosis(null);
      setDiagnoseError(null);
      loadData();
    } catch {
      setDiagnoseError("Failed to dismiss incident");
    }
  }

  function handleSignOut() {
    sessionStorage.removeItem("kiosk_staff_name");
    sessionStorage.removeItem("kiosk_staff_id");
    sessionStorage.removeItem("kiosk_staff_token");
    router.push("/");
  }

  if (!staffName) return null;

  // Note: ServerLogViewer is defined below as a standalone component

  // ─── Render ───────────────────────────────────────────────────────────────

  return (
    <div className="h-screen bg-rp-black flex flex-col overflow-hidden">
      <KioskHeader
        connected={connected}
        pods={pods}
        staffName={staffName}
        onSignOut={handleSignOut}
      />

      {loadError && (
        <div className="mx-4 mt-2 px-4 py-2 bg-red-900/30 border border-red-600/50 rounded-lg flex items-center justify-between">
          <span className="text-red-400 text-sm font-mono">{loadError}</span>
          <button onClick={() => { setLoadError(null); loadData(); }} className="text-xs text-red-400 hover:text-white border border-red-600/50 rounded px-2 py-1">
            Retry
          </button>
        </div>
      )}

      <div className="flex-1 flex gap-4 p-4 min-h-0">
        {/* ─── LEFT SIDEBAR: Pod Grid ──────────────────────────────── */}
        {/* ── DEBUG-PAGE-INCIDENTS-REGION-START ── */}
        <div className="w-48 flex-shrink-0 flex flex-col gap-3 overflow-y-auto">
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
        {/* ── DEBUG-PAGE-INCIDENTS-REGION-END ── */}

        {/* ─── MAIN: Live Activity Feed ────────────────────────────── */}
        <div className="flex-1 flex flex-col gap-3 min-w-0 min-h-0 overflow-hidden">
          {/* Header */}
          <div className="flex items-center justify-between flex-shrink-0">
            <h2 className="text-sm font-semibold text-rp-grey uppercase tracking-wider">
              Live Activity
              {selectedPodId && (
                <span className="text-white ml-2 normal-case">
                  — {(() => { const p = podHealth.find((p) => p.pod_id === selectedPodId); return getPodLabel(p?.pod_number || 0, selectedPodId, p?.node_type, p?.name); })()}
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

          {/* Activity Feed / LaunchCard panel — conditional on kiosk_launch_cards_enabled flag */}
          <div className="flex-1 min-h-[100px] bg-rp-card border border-rp-border rounded-xl overflow-y-auto p-2">
            {launchCardsEnabled ? (
              launches.size === 0 ? (
                // D-12 empty state
                <div
                  className="flex items-center gap-2 p-4"
                  data-testid="launches-empty-state"
                >
                  <div
                    className={`h-2 w-2 rounded-full ${connected ? "bg-green-400 animate-pulse" : "bg-red-500"}`}
                    data-testid="ws-connection-dot"
                  />
                  <span className="text-sm text-rp-grey">
                    No active launches — waiting for next game start
                  </span>
                </div>
              ) : (
                // Newest-first, grouped visually by pod_id per D-10
                Array.from(launches.values())
                  .sort((a, b) => b.timestamp.localeCompare(a.timestamp))
                  .map((card) => (
                    <LaunchCard
                      key={card.launch_id}
                      card={card}
                      notes={launchNotes.get(card.launch_id) ?? []}
                      onAddNote={async (body) => {
                        try {
                          await api.postLaunchNote(card.launch_id, body);
                          // Note arrives via WS launch_note_added — no local state mutation needed
                        } catch (e) {
                          console.error("[debug] postLaunchNote failed", e);
                        }
                      }}
                      onApproveFix={async () => {
                        try {
                          await api.approveLaunchFix(card.launch_id);
                        } catch (e) {
                          console.error("[debug] approveLaunchFix failed", e);
                        }
                      }}
                      onDismiss={async () => {
                        try {
                          await api.dismissLaunch(card.launch_id);
                          removeLaunch(card.launch_id);
                        } catch (e) {
                          console.error("[debug] dismissLaunch failed", e);
                        }
                      }}
                    />
                  ))
              )
            ) : (
              // Existing flat activity feed — preserved as-is when flag=false (D-14 fallback)
              filteredActivity.length === 0 ? (
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
                            {podHealth.find(p => p.pod_id === entry.pod_id)?.node_type === 'pos' ? 'POS' : `Pod ${entry.pod_number}`}
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
              )
            )}
          </div>

          {/* ─── Server Logs (collapsible) ─────────────────────── */}
          <ServerLogViewer />

          {/* ─── Lap Health (zero-laps verification) ──────────────── */}
          <LapHealthPanel
            wsLaps={recentLaps}
            billingTimers={billingTimers}
            latestTelemetry={latestTelemetry}
            pods={pods}
          />

          {/* ─── Pod State Probe (read-only diagnostics) ───────────── */}
          {selectedPodId && (
            <ProbePanel
              probe={podProbe}
              loading={probeLoading}
              postFix={postFixProbe}
              autoPaused={probeAutoPaused}
              onRefresh={async () => {
                if (!selectedPodId) return;
                setProbeLoading(true);
                try {
                  const res = await api.podStateProbe(selectedPodId);
                  setPodProbe(res);
                } catch (e) {
                  console.warn("[debug] manual probe refresh failed", e);
                } finally {
                  setProbeLoading(false);
                }
              }}
            />
          )}

          {/* ─── Fleet-wide MI Events (when no pod selected) ───────── */}
          {!selectedPodId && fleetMiEvents.size > 0 && (
            <div className="flex-shrink-0 bg-rp-card border border-rp-border rounded-xl p-3 max-h-[30%] overflow-y-auto">
              <h2 className="text-xs font-semibold text-amber-400 uppercase tracking-wider mb-2">
                Meshed Intelligence — Fleet Events ({fleetMiEvents.size} pods reporting)
              </h2>
              <div className="space-y-2">
                {Array.from(fleetMiEvents.entries()).map(([podId, events]) => {
                  const pod = pods.get(podId);
                  return (
                    <div key={podId} className="border-l-2 border-amber-500/40 pl-2">
                      <button
                        onClick={() => setSelectedPodId(podId)}
                        className="text-xs font-semibold text-amber-400 hover:text-white"
                      >
                        Pod {pod?.number || "?"} — {events.length} event{events.length === 1 ? "" : "s"}
                      </button>
                      {events.slice(0, 3).map((evt, i) => (
                        <div key={`${evt.timestamp}-${i}`} className="text-[10px] text-zinc-400 font-mono mt-0.5">
                          {formatTime(evt.timestamp)} · tier {evt.tier} · {evt.outcome} ·
                          {" "}{evt.trigger.replace(/^Variant\d+/, "").replace(/[()]/g, "").slice(0, 60)}
                        </div>
                      ))}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* ─── Pod Tier Engine Events (v27.0) ────────────────────── */}
          {selectedPodId && (
            <div className="flex-shrink-0 bg-rp-card border border-rp-border rounded-xl p-3 max-h-[30%] overflow-y-auto">
              <h2 className="text-xs font-semibold text-amber-400 uppercase tracking-wider mb-2">
                Meshed Intelligence — Pod Diagnostics
              </h2>
              {podEventsLoading && podEvents.length === 0 ? (
                <p className="text-xs text-zinc-500">Loading pod events...</p>
              ) : podEvents.length === 0 ? (
                <p className="text-xs text-zinc-500">No recent diagnostic events</p>
              ) : (
                <div className="space-y-1">
                  {podEvents.map((evt, i) => (
                    <div
                      key={`${evt.timestamp}-${i}`}
                      className={`flex items-start gap-2 px-2 py-1.5 rounded text-xs font-mono ${
                        evt.outcome === "fixed"
                          ? "bg-green-900/20 border border-green-800/30"
                          : evt.outcome === "failed_to_fix"
                          ? "bg-red-900/20 border border-red-800/30"
                          : evt.source === "staff"
                          ? "bg-amber-900/20 border border-amber-800/30"
                          : "bg-zinc-800/50 border border-zinc-700/30"
                      }`}
                    >
                      <span className={`flex-shrink-0 w-1.5 h-1.5 rounded-full mt-1 ${
                        evt.outcome === "fixed" ? "bg-green-400" :
                        evt.outcome === "failed_to_fix" ? "bg-red-400" :
                        evt.source === "staff" ? "bg-amber-400" : "bg-zinc-500"
                      }`} />
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2">
                          <span className="text-zinc-400">
                            {formatTime(evt.timestamp)}
                          </span>
                          {evt.tier > 0 && (
                            <span className="px-1 py-0.5 bg-zinc-700 rounded text-[10px] text-zinc-300">
                              Tier {evt.tier}
                            </span>
                          )}
                          <span className={`px-1 py-0.5 rounded text-[10px] ${
                            evt.outcome === "fixed" ? "bg-green-800 text-green-200" :
                            evt.outcome === "manual" ? "bg-amber-800 text-amber-200" :
                            "bg-zinc-700 text-zinc-300"
                          }`}>
                            {evt.outcome}
                          </span>
                          {evt.source === "staff" && (
                            <span className="px-1 py-0.5 bg-blue-800 rounded text-[10px] text-blue-200">
                              staff
                            </span>
                          )}
                        </div>
                        <p className="text-zinc-300 truncate mt-0.5">
                          {evt.trigger.replace(/^Variant\d+/, "").replace(/[()]/g, "")}
                          {evt.action ? ` → ${evt.action}` : ""}
                        </p>
                        {evt.confidence > 0 && evt.confidence < 1 && (
                          <p className="text-zinc-500 text-[10px]">
                            Confidence: {(evt.confidence * 100).toFixed(0)}%
                            {evt.fix_type !== "none" ? ` | ${evt.fix_type}` : ""}
                          </p>
                        )}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* ─── HALO Catalog (collapsible probe registry) ───────── */}
          {haloCatalog.length > 0 && (
            <HaloPanel probes={haloCatalog} byStatus={haloByStatus} />
          )}

          {/* ─── Diagnostics Section (collapsible) ───────────────── */}
          <div className="flex-shrink-0 bg-rp-card border border-rp-border rounded-xl max-h-[40%] overflow-y-auto">
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
                          {p.node_type === 'pos' ? (p.name || 'POS') : `Pod ${p.pod_number}`}
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
                      <div className="space-y-2">
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
                        {diagnoseError && (
                          <div className="text-xs text-red-400 bg-red-900/20 border border-red-600/30 rounded-lg px-3 py-2">
                            {diagnoseError}
                          </div>
                        )}
                      </div>
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

                        {diagnosis.past_resolutions?.length > 0 && (
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

                    {/* Quick Actions — apply fix directly */}
                    {selectedPodId && (
                      <div className="space-y-2">
                        <h3 className="text-xs font-semibold text-rp-grey uppercase tracking-wider">
                          Quick Actions
                          {suggestedActions.length > 0 && (
                            <span className="text-amber-500 normal-case ml-2 font-normal">
                              — suggested based on diagnosis
                            </span>
                          )}
                        </h3>
                        <div className="grid grid-cols-2 gap-2">
                          {([
                            { action: "restart_pod", label: "Restart Pod", color: "bg-orange-600/20 border-orange-600/50 text-orange-400 hover:bg-orange-600/30" },
                            { action: "wake_pod", label: "Wake Pod (WoL)", color: "bg-green-600/20 border-green-600/50 text-green-400 hover:bg-green-600/30" },
                            { action: "relaunch_edge", label: "Relaunch Edge", color: "bg-blue-600/20 border-blue-600/50 text-blue-400 hover:bg-blue-600/30" },
                            { action: "kill_game", label: "Kill Game", color: "bg-red-600/20 border-red-600/50 text-red-400 hover:bg-red-600/30" },
                            { action: "restart_audio", label: "Restart Audio", color: "bg-purple-600/20 border-purple-600/50 text-purple-400 hover:bg-purple-600/30" },
                            { action: "restart_steam", label: "Restart Steam", color: "bg-cyan-600/20 border-cyan-600/50 text-cyan-400 hover:bg-cyan-600/30" },
                            { action: "dismiss_dialogs", label: "Dismiss Dialogs", color: "bg-pink-600/20 border-pink-600/50 text-pink-400 hover:bg-pink-600/30" },
                          ] as const).map(({ action, label, color }) => {
                            const isSuggested = suggestedActions.includes(action);
                            return (
                              <button
                                key={action}
                                onClick={() => handleApplyFix(action)}
                                disabled={applyingFix !== null}
                                className={`border rounded-lg py-1.5 px-3 text-xs font-medium transition-colors disabled:opacity-50 ${color} ${
                                  isSuggested ? "ring-1 ring-amber-500/50 shadow-[0_0_6px_rgba(245,158,11,0.15)]" : ""
                                }`}
                              >
                                {applyingFix === action ? (
                                  <span className="flex items-center justify-center gap-1">
                                    <span className="w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin" />
                                    Applying...
                                  </span>
                                ) : (
                                  <>
                                    {isSuggested && <span className="text-amber-500 mr-1">&#9733;</span>}
                                    {label}
                                  </>
                                )}
                              </button>
                            );
                          })}
                        </div>
                        {fixResult && !fixResult.ok && (
                          <div className="text-xs text-red-400 bg-red-900/20 border border-red-600/30 rounded-lg px-3 py-2">
                            {fixResult.error}
                          </div>
                        )}
                        {fixResult?.ok && (
                          <div className="text-xs text-green-400 bg-green-900/20 border border-green-600/30 rounded-lg px-3 py-2">
                            Fix applied successfully — incident resolved.
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

// ─── Server Log Viewer ──────────────────────────────────────────────────────

const LOG_LEVEL_COLORS: Record<string, string> = {
  ERROR: "text-red-400",
  WARN: "text-amber-400",
  INFO: "text-zinc-300",
  DEBUG: "text-zinc-500",
  TRACE: "text-zinc-600",
};

function classifyLogLevel(line: string): string {
  if (line.includes(" ERROR ")) return "ERROR";
  if (line.includes(" WARN ")) return "WARN";
  if (line.includes(" DEBUG ")) return "DEBUG";
  if (line.includes(" TRACE ")) return "TRACE";
  return "INFO";
}

type LogFilter = "ALL" | "ERROR" | "WARN" | "INFO";

function ServerLogViewer() {
  const [open, setOpen] = useState(false);
  const [lines, setLines] = useState<string[]>([]);
  const [total, setTotal] = useState(0);
  const [filter, setFilter] = useState<LogFilter>("ALL");
  const [autoScroll, setAutoScroll] = useState(true);
  const [loading, setLoading] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  const fetchLogs = useCallback(async () => {
    if (!open) return;
    setLoading(true);
    try {
      const level = filter === "ALL" ? undefined : filter;
      const res = await api.serverLogs(500, level);
      setLines(res.lines || []);
      setTotal(res.total || 0);
    } catch {
      // silent — logs panel is secondary
    } finally {
      setLoading(false);
    }
  }, [open, filter]);

  useEffect(() => {
    fetchLogs();
    if (!open) return;
    const iv = setInterval(fetchLogs, 3000);
    return () => clearInterval(iv);
  }, [fetchLogs, open]);

  // Auto-scroll to bottom
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [lines, autoScroll]);

  const handleScroll = () => {
    if (!scrollRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current;
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 40);
  };

  const errorCount = lines.filter((l) => l.includes(" ERROR ")).length;
  const warnCount = lines.filter((l) => l.includes(" WARN ")).length;

  return (
    <div className="flex-shrink-0 bg-rp-card border border-rp-border rounded-xl overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between p-3 text-left"
      >
        <div className="flex items-center gap-3">
          <h2 className="text-xs font-semibold text-rp-grey uppercase tracking-wider">
            Server Logs
          </h2>
          {!open && errorCount > 0 && (
            <span className="bg-red-600 text-white text-[10px] font-bold px-1.5 py-0.5 rounded-full">
              {errorCount} ERR
            </span>
          )}
          {!open && warnCount > 0 && (
            <span className="bg-amber-600 text-white text-[10px] font-bold px-1.5 py-0.5 rounded-full">
              {warnCount} WARN
            </span>
          )}
          {!open && lines.length > 0 && (
            <span className="text-[10px] text-zinc-600">{total} total lines</span>
          )}
        </div>
        <svg
          className={`w-4 h-4 text-zinc-500 transition-transform ${open ? "rotate-180" : ""}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {open && (
        <div className="px-3 pb-3 space-y-2">
          {/* Filter buttons */}
          <div className="flex items-center gap-2">
            {(["ALL", "ERROR", "WARN", "INFO"] as LogFilter[]).map((lvl) => (
              <button
                key={lvl}
                onClick={() => setFilter(lvl)}
                className={`text-[10px] font-bold px-2 py-1 rounded transition-colors ${
                  filter === lvl
                    ? lvl === "ERROR"
                      ? "bg-red-600/30 text-red-400 border border-red-600/50"
                      : lvl === "WARN"
                      ? "bg-amber-600/30 text-amber-400 border border-amber-600/50"
                      : "bg-zinc-600/30 text-white border border-zinc-500/50"
                    : "text-zinc-500 border border-rp-border hover:text-white"
                }`}
              >
                {lvl}
              </button>
            ))}
            <span className="text-[10px] text-zinc-600 ml-auto">
              {lines.length} lines {loading && "..."}
            </span>
            {!autoScroll && (
              <button
                onClick={() => {
                  setAutoScroll(true);
                  if (scrollRef.current)
                    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
                }}
                className="text-[10px] text-rp-red border border-rp-red/50 px-2 py-0.5 rounded hover:bg-rp-red/10"
              >
                Scroll to bottom
              </button>
            )}
          </div>

          {/* Log content */}
          <div
            ref={scrollRef}
            onScroll={handleScroll}
            className="bg-rp-black border border-rp-border rounded-lg p-2 font-mono text-[11px] leading-relaxed overflow-y-auto max-h-[300px] min-h-[120px]"
          >
            {lines.length === 0 ? (
              <div className="text-zinc-600 text-center py-8">
                {loading ? "Loading logs..." : "No log entries yet"}
              </div>
            ) : (
              lines.map((line, i) => {
                const level = classifyLogLevel(line);
                return (
                  <div key={i} className={`${LOG_LEVEL_COLORS[level]} whitespace-pre-wrap break-all`}>
                    {line}
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ─── Lap Health Panel (zero-laps verification) ─────────────────────────────

function LapHealthPanel({
  wsLaps,
  billingTimers,
  latestTelemetry,
  pods,
}: {
  wsLaps: Lap[];
  billingTimers: Map<string, BillingSession>;
  latestTelemetry: Map<string, TelemetryFrame>;
  pods: Map<string, Pod>;
}) {
  const [open, setOpen] = useState(false);
  const [dbLaps, setDbLaps] = useState<
    { id: string; driver_id: string; track: string; car: string; lap_time_ms: number; valid: boolean }[]
  >([]);
  const [dbLoading, setDbLoading] = useState(false);
  const [dbError, setDbError] = useState<string | null>(null);

  const fetchDbLaps = useCallback(async () => {
    if (!open) return;
    setDbLoading(true);
    try {
      const res = await api.listLaps(50);
      setDbLaps(res.laps || []);
      setDbError(null);
    } catch (e) {
      setDbError(e instanceof Error ? e.message : String(e));
    } finally {
      setDbLoading(false);
    }
  }, [open]);

  useEffect(() => {
    fetchDbLaps();
    if (!open) return;
    const iv = setInterval(fetchDbLaps, 30000);
    return () => clearInterval(iv);
  }, [fetchDbLaps, open]);

  // Detect zero-lap anomalies: pods with active billing + telemetry but no WS laps
  const anomalies = useMemo(() => {
    const results: {
      podId: string;
      podNumber: number;
      driverName: string;
      drivingSeconds: number;
      telemetryLapNumber: number;
    }[] = [];
    billingTimers.forEach((session, podId) => {
      if (session.status !== "active" && session.status !== "waiting_for_game") return;
      const drivingSec = session.driving_seconds || 0;
      if (drivingSec < 120) return; // skip sessions under 2 min

      const telemetry = latestTelemetry.get(podId);
      const telemetryLap = telemetry?.lap_number || 0;

      // Flag if driving 2+ min with telemetry showing laps but zero WS lap_completed events
      if (telemetryLap > 1 && wsLaps.length === 0) {
        const pod = pods.get(podId);
        results.push({
          podId,
          podNumber: pod?.number || 0,
          driverName: session.driver_name || "Unknown",
          drivingSeconds: drivingSec,
          telemetryLapNumber: telemetryLap,
        });
      }
    });
    return results;
  }, [billingTimers, latestTelemetry, wsLaps, pods]);

  const hasAnomaly = anomalies.length > 0;

  return (
    <div
      className={`flex-shrink-0 bg-rp-card border rounded-xl overflow-hidden ${
        hasAnomaly ? "border-red-600" : "border-rp-border"
      }`}
    >
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between p-3 text-left"
      >
        <div className="flex items-center gap-3">
          <h2 className="text-xs font-semibold text-rp-grey uppercase tracking-wider">
            Lap Health
          </h2>
          {hasAnomaly && (
            <span className="bg-red-600 text-white text-[10px] font-bold px-1.5 py-0.5 rounded-full animate-pulse">
              ZERO LAPS
            </span>
          )}
          {!hasAnomaly && wsLaps.length > 0 && (
            <span className="bg-green-600 text-white text-[10px] font-bold px-1.5 py-0.5 rounded-full">
              {wsLaps.length} live
            </span>
          )}
          {!open && dbLaps.length > 0 && (
            <span className="text-[10px] text-zinc-600">{dbLaps.length} in DB</span>
          )}
        </div>
        <svg
          className={`w-4 h-4 text-zinc-500 transition-transform ${open ? "rotate-180" : ""}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {open && (
        <div className="px-3 pb-3 space-y-3">
          {/* Zero-lap anomaly alerts */}
          {anomalies.map((a) => (
            <div
              key={a.podId}
              className="bg-red-900/30 border border-red-600/50 rounded-lg px-3 py-2 text-xs"
            >
              <div className="flex items-center gap-2 text-red-400 font-semibold">
                <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse" />
                Pod {a.podNumber} — NO LAPS RECORDED
              </div>
              <div className="text-zinc-400 mt-1">
                Driver: {a.driverName} | Driving:{" "}
                {Math.floor(a.drivingSeconds / 60)}m {a.drivingSeconds % 60}s |
                Telemetry lap: {a.telemetryLapNumber} | WS laps: 0
              </div>
              <div className="text-zinc-500 mt-0.5">
                Check: python.ini on pod, AC SHM plugin, rc-agent lap_tracker
              </div>
            </div>
          ))}

          {/* Active sessions summary */}
          <div>
            <h3 className="text-[10px] font-semibold text-zinc-500 uppercase mb-1">
              Active Sessions
            </h3>
            {billingTimers.size === 0 ? (
              <p className="text-xs text-zinc-600">No active billing sessions</p>
            ) : (
              <div className="space-y-1">
                {Array.from(billingTimers.entries()).map(([podId, session]) => {
                  const pod = pods.get(podId);
                  const telemetry = latestTelemetry.get(podId);
                  const isActive = session.status === "active";
                  return (
                    <div key={podId} className="flex items-center gap-2 text-xs font-mono">
                      <span
                        className={`w-1.5 h-1.5 rounded-full ${
                          isActive ? "bg-green-400" : "bg-zinc-500"
                        }`}
                      />
                      <span className="text-zinc-400 w-12">Pod {pod?.number || "?"}</span>
                      <span className="text-white w-24 truncate">{session.driver_name}</span>
                      <span className="text-zinc-500 w-16">
                        {Math.floor((session.driving_seconds || 0) / 60)}m
                      </span>
                      <span className="text-zinc-500 w-16">
                        Lap {telemetry?.lap_number || 0}
                      </span>
                      <span
                        className={`px-1 py-0.5 rounded text-[10px] ${
                          isActive
                            ? "bg-green-800 text-green-200"
                            : "bg-zinc-700 text-zinc-300"
                        }`}
                      >
                        {session.status}
                      </span>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* WS laps received this session */}
          <div>
            <h3 className="text-[10px] font-semibold text-zinc-500 uppercase mb-1">
              WebSocket Laps (this session: {wsLaps.length})
            </h3>
            {wsLaps.length === 0 ? (
              <p className="text-xs text-zinc-600">
                No lap_completed events received via WebSocket
              </p>
            ) : (
              <div className="space-y-0.5 max-h-[100px] overflow-y-auto">
                {wsLaps.slice(0, 10).map((lap, i) => (
                  <div
                    key={`${lap.id}-${i}`}
                    className="flex items-center gap-2 text-xs font-mono text-zinc-400"
                  >
                    <span
                      className={`w-1.5 h-1.5 rounded-full ${
                        lap.valid ? "bg-green-400" : "bg-zinc-600"
                      }`}
                    />
                    <span className="w-24 truncate">{lap.track}</span>
                    <span className="w-20 truncate">{lap.car}</span>
                    <span className="w-20">
                      {(lap.lap_time_ms / 1000).toFixed(3)}s
                    </span>
                    <span
                      className={`text-[10px] ${
                        lap.valid ? "text-green-600" : "text-red-600"
                      }`}
                    >
                      {lap.valid ? "valid" : "invalid"}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* DB laps */}
          <div>
            <h3 className="text-[10px] font-semibold text-zinc-500 uppercase mb-1">
              Database Laps (last {dbLaps.length})
              {dbLoading && <span className="text-zinc-600 ml-2">loading...</span>}
            </h3>
            {dbError && <div className="text-xs text-red-400 mb-1">{dbError}</div>}
            {dbLaps.length === 0 && !dbLoading ? (
              <p className="text-xs text-red-400 font-semibold">
                0 laps in database — zero-laps bug likely active
              </p>
            ) : (
              <div className="space-y-0.5 max-h-[100px] overflow-y-auto">
                {dbLaps.slice(0, 10).map((lap, i) => (
                  <div
                    key={`${lap.id}-${i}`}
                    className="flex items-center gap-2 text-xs font-mono text-zinc-400"
                  >
                    <span
                      className={`w-1.5 h-1.5 rounded-full ${
                        lap.valid ? "bg-green-400" : "bg-zinc-600"
                      }`}
                    />
                    <span className="w-24 truncate">{lap.track}</span>
                    <span className="w-20 truncate">{lap.car}</span>
                    <span className="w-20">{(lap.lap_time_ms / 1000).toFixed(3)}s</span>
                    <span
                      className={`text-[10px] ${
                        lap.valid ? "text-green-600" : "text-red-600"
                      }`}
                    >
                      {lap.valid ? "valid" : "invalid"}
                    </span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ─── Probe Panel (read-only diagnostics) ────────────────────────────────────

function sentinelBadge(label: string, value: boolean | null | undefined) {
  let bg = "bg-zinc-700";
  let txt = "text-zinc-400";
  let dot = "bg-zinc-500";
  if (value === true) {
    bg = "bg-red-900/40 border-red-600/50";
    txt = "text-red-300";
    dot = "bg-red-500 animate-pulse";
  } else if (value === false) {
    bg = "bg-zinc-800/40 border-zinc-700/40";
    txt = "text-zinc-400";
    dot = "bg-zinc-600";
  }
  return (
    <div key={label} className={`flex items-center gap-1.5 px-2 py-1 rounded border ${bg}`}>
      <span className={`w-1.5 h-1.5 rounded-full ${dot}`} />
      <span className={`text-[10px] font-mono ${txt}`}>{label}</span>
    </div>
  );
}

function ProbePanel({
  probe,
  loading,
  postFix,
  autoPaused,
  onRefresh,
}: {
  probe: PodStateProbe | null;
  loading: boolean;
  postFix: PodStateProbe | null;
  autoPaused: boolean;
  onRefresh: () => void;
}) {
  const [open, setOpen] = useState(true);
  return (
    <div className="flex-shrink-0 bg-rp-card border border-rp-border rounded-xl overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between p-3 text-left"
      >
        <div className="flex items-center gap-3">
          <h2 className="text-xs font-semibold text-cyan-400 uppercase tracking-wider">
            Pod State Probe
          </h2>
          {probe && probe.lock_screen_state && (
            <span className="text-[10px] font-mono text-zinc-400">
              {probe.lock_screen_state}
            </span>
          )}
          {probe?.errors && probe.errors.length > 0 && (
            <span className="bg-red-600/30 text-red-400 text-[10px] font-bold px-1.5 py-0.5 rounded">
              {probe.errors.length} err
            </span>
          )}
          {autoPaused && (
            <span className="bg-amber-600/30 text-amber-300 text-[10px] font-bold px-1.5 py-0.5 rounded border border-amber-600/40">
              auto-paused — customer active
            </span>
          )}
          {loading && <span className="text-[10px] text-zinc-500">probing...</span>}
        </div>
        <svg
          className={`w-4 h-4 text-zinc-500 transition-transform ${open ? "rotate-180" : ""}`}
          fill="none" viewBox="0 0 24 24" stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {open && probe && (
        <div className="px-3 pb-3 space-y-2">
          {/* Top row: build_id + uptime + session + native window */}
          <div className="flex flex-wrap gap-2 text-[10px] font-mono">
            <span className="px-2 py-1 rounded bg-zinc-800 text-zinc-300">
              build {probe.build_id?.slice(0, 8) ?? "?"}
            </span>
            <span className="px-2 py-1 rounded bg-zinc-800 text-zinc-300">
              uptime {probe.uptime_secs ?? "?"}s
            </span>
            {probe.rc_agent_session && (
              <span className={`px-2 py-1 rounded ${
                probe.rc_agent_session === "Console"
                  ? "bg-green-900/40 text-green-300"
                  : "bg-red-900/40 text-red-300"
              }`}>
                Session: {probe.rc_agent_session}
                {probe.rc_agent_session === "Services" && " (Session 0 — GUI broken)"}
              </span>
            )}
            <span className={`px-2 py-1 rounded ${
              probe.native_window_alive
                ? "bg-green-900/30 text-green-300"
                : "bg-zinc-800 text-zinc-400"
            }`}>
              native window: {probe.native_window_alive === null ? "?" : probe.native_window_alive ? "alive" : "down"}
            </span>
            {typeof probe.conspitlink_count === "number" && (
              <span className={`px-2 py-1 rounded ${
                probe.conspitlink_count === 1
                  ? "bg-green-900/30 text-green-300"
                  : probe.conspitlink_count === 0
                  ? "bg-red-900/40 text-red-300"
                  : "bg-amber-900/40 text-amber-300"
              }`}>
                ConspitLink ×{probe.conspitlink_count}
                {probe.conspitlink_count > 1 && " (multiplication)"}
                {probe.conspitlink_count === 0 && " (down)"}
              </span>
            )}
          </div>

          {/* Sentinel grid */}
          <div>
            <div className="text-[10px] text-zinc-500 uppercase mb-1">Sentinel files</div>
            <div className="flex flex-wrap gap-1.5">
              {sentinelBadge("MAINTENANCE_MODE", probe.sentinels.maintenance_mode)}
              {sentinelBadge("OTA_DEPLOYING", probe.sentinels.ota_deploying)}
              {sentinelBadge("GRACEFUL_RELAUNCH", probe.sentinels.graceful_relaunch)}
              {sentinelBadge("DEPLOY_IN_PROGRESS", probe.sentinels.deploy_in_progress)}
              {sentinelBadge("restart-sentinel", probe.sentinels.restart_sentinel)}
            </div>
          </div>

          {/* Post-fix probe (after apply-fix re-probe) */}
          {postFix && (
            <div className="border-t border-rp-border pt-2">
              <div className="text-[10px] text-amber-400 uppercase mb-1">
                Post-fix probe — observe before declaring resolved
              </div>
              <div className="text-[10px] font-mono text-zinc-300">
                lock_screen_state: {postFix.lock_screen_state ?? "?"} ·
                native_window_alive: {String(postFix.native_window_alive)} ·
                build {postFix.build_id?.slice(0, 8) ?? "?"} · uptime {postFix.uptime_secs ?? "?"}s
              </div>
            </div>
          )}

          {/* Errors */}
          {probe.errors.length > 0 && (
            <div className="border-t border-rp-border pt-2">
              <div className="text-[10px] text-red-400 uppercase mb-1">Sub-probe errors</div>
              {probe.errors.map((e, i) => (
                <div key={i} className="text-[10px] font-mono text-red-300">{e}</div>
              ))}
            </div>
          )}

          <div className="flex items-center justify-between text-[10px] text-zinc-500 pt-1">
            <span>probed {probe.probed_at.slice(11, 19)} UTC</span>
            <button
              onClick={onRefresh}
              className="text-cyan-400 hover:text-white border border-cyan-600/40 rounded px-2 py-0.5"
              disabled={loading}
            >
              {loading ? "..." : "Re-probe now"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

// ─── HALO Catalog Panel (registry view; no runners yet) ─────────────────────

function HaloPanel({
  probes,
  byStatus,
}: {
  probes: HaloProbe[];
  byStatus: Record<string, number>;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState<"ALL" | "PROPOSED" | "CALIBRATING" | "LIVE">("ALL");
  const filtered = filter === "ALL" ? probes : probes.filter((p) => p.status === filter);

  // Group by layer
  const byLayer = new Map<string, HaloProbe[]>();
  for (const p of filtered) {
    const arr = byLayer.get(p.layer) || [];
    arr.push(p);
    byLayer.set(p.layer, arr);
  }
  const layerNames: Record<string, string> = {
    V: "Venue (.23 + Pods + POS)",
    R: "Racecontrol",
    X: "Comms",
    M: "Memory",
    C: "Cloud",
    B: "Bots",
  };

  return (
    <div className="flex-shrink-0 bg-rp-card border border-rp-border rounded-xl overflow-hidden">
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between p-3 text-left"
      >
        <div className="flex items-center gap-3">
          <h2 className="text-xs font-semibold text-purple-400 uppercase tracking-wider">
            HALO Catalog
          </h2>
          <span className="text-[10px] text-zinc-500">
            {probes.length} probes · {byStatus.LIVE || 0} LIVE · {byStatus.CALIBRATING || 0} CAL · {byStatus.PROPOSED || 0} PROP
          </span>
          <span className="text-[10px] text-amber-400/70 italic">
            scaffolding until Mesh Intelligence ships
          </span>
        </div>
        <svg
          className={`w-4 h-4 text-zinc-500 transition-transform ${open ? "rotate-180" : ""}`}
          fill="none" viewBox="0 0 24 24" stroke="currentColor"
        >
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
        </svg>
      </button>

      {open && (
        <div className="px-3 pb-3 space-y-2">
          {/* Filter row */}
          <div className="flex items-center gap-2">
            {(["ALL", "PROPOSED", "CALIBRATING", "LIVE"] as const).map((s) => (
              <button
                key={s}
                onClick={() => setFilter(s)}
                className={`text-[10px] font-bold px-2 py-1 rounded transition-colors ${
                  filter === s
                    ? "bg-purple-600/30 text-purple-300 border border-purple-600/50"
                    : "text-zinc-500 border border-rp-border hover:text-white"
                }`}
              >
                {s}
              </button>
            ))}
          </div>

          {/* Probes grouped by layer */}
          {Array.from(byLayer.entries()).sort().map(([layer, items]) => (
            <div key={layer}>
              <h3 className="text-[10px] font-semibold text-zinc-500 uppercase mt-2 mb-1">
                HALO-{layer} — {layerNames[layer] || layer} ({items.length})
              </h3>
              <div className="space-y-1">
                {items.map((p) => (
                  <div
                    key={p.id}
                    className="bg-rp-black border border-rp-border rounded p-2 text-[11px]"
                  >
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-purple-400 font-bold">{p.id}</span>
                      <span className="text-white flex-1 truncate">{p.name}</span>
                      <span className={`px-1.5 py-0.5 rounded text-[9px] font-bold ${
                        p.status === "LIVE" ? "bg-green-700 text-green-100" :
                        p.status === "CALIBRATING" ? "bg-amber-700 text-amber-100" :
                        p.status === "PROPOSED" ? "bg-zinc-700 text-zinc-300" :
                        "bg-zinc-800 text-zinc-500"
                      }`}>{p.status}</span>
                      <span className="px-1.5 py-0.5 rounded text-[9px] bg-zinc-800 text-zinc-400">
                        {p.bug_class}
                      </span>
                    </div>
                    <div className="text-zinc-500 mt-0.5 text-[10px]">
                      <span className="text-zinc-400">target:</span> {p.target}
                    </div>
                    <div className="text-zinc-500 mt-0.5 text-[10px]">
                      <span className="text-zinc-400">invariant:</span> {p.invariant}
                    </div>
                    {p.sub_pact !== "TBD" && (
                      <div className="text-zinc-600 text-[10px] mt-0.5">
                        sub-PACT: {p.sub_pact}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}

          <div className="text-[10px] text-amber-400/60 italic pt-2 border-t border-rp-border">
            HALO probes are catalogued read-only. Mesh Intelligence will execute them autonomously
            once venue stabilizes — see HALO-CHARTER.md for lifecycle.
          </div>
        </div>
      )}
    </div>
  );
}
