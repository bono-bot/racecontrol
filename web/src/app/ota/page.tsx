"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type { OtaStatusResponse, DeployRecord, PipelineState } from "@/lib/api";
import { useWebSocket } from "@/hooks/useWebSocket";

// Wave definitions matching crates/racecontrol/src/ota_pipeline.rs
const WAVES = [
  { label: "Canary", pods: [8] },
  { label: "Rollout A", pods: [1, 2, 3, 4] },
  { label: "Rollout B", pods: [5, 6, 7] },
];

// Pipeline state color classes
function stateColors(state: PipelineState): string {
  switch (state) {
    case "idle":
    case "completed":
      return "bg-emerald-900/50 text-emerald-400";
    case "building":
    case "staging":
    case "canary":
    case "staged_rollout":
    case "health_checking":
      return "bg-amber-900/50 text-amber-400";
    case "rolling_back":
      return "bg-red-900/50 text-red-400";
    default:
      return "bg-neutral-700 text-neutral-400";
  }
}

function isActiveState(state: PipelineState): boolean {
  return !["idle", "completed"].includes(state);
}

// Check if response is a full DeployRecord (not idle message)
function isDeployRecord(res: OtaStatusResponse): res is DeployRecord {
  return "waves_completed" in res;
}

// Format timestamp to IST
function formatIST(ts: string | null | undefined): string {
  if (!ts) return "--";
  try {
    return new Date(ts).toLocaleString("en-IN", { timeZone: "Asia/Kolkata" });
  } catch {
    return ts;
  }
}

// Example TOML manifest for placeholder text
const TOML_PLACEHOLDER = `# Example release manifest
[release]
version = "v1.2.3"
binary = "rc-agent"

[source]
repo = "racecontrol"
commit = "abc1234"

[rollout]
canary_duration_secs = 300
health_check_interval_secs = 30`;

interface WaveStepperProps {
  wavesCompleted: number;
  pipelineActive: boolean;
  failedPods: string[];
}

function WaveStepper({ wavesCompleted, pipelineActive, failedPods }: WaveStepperProps) {
  const { billingTimers } = useWebSocket();

  // Determine the current active wave (0-indexed)
  const activeWaveIdx = pipelineActive ? wavesCompleted : -1;

  return (
    <div className="space-y-4">
      {/* Stepper timeline */}
      <div className="flex items-center justify-between">
        {WAVES.map((wave, idx) => {
          const isComplete = idx < wavesCompleted;
          const isActive = idx === activeWaveIdx;
          const isPending = !isComplete && !isActive;

          return (
            <div key={wave.label} className="flex items-center flex-1">
              {/* Wave circle + content */}
              <div className="flex flex-col items-center flex-shrink-0">
                <div
                  className={`w-10 h-10 rounded-full flex items-center justify-center text-sm font-bold ${
                    isComplete
                      ? "bg-emerald-500 text-white"
                      : isActive
                      ? "bg-amber-500 text-black animate-pulse"
                      : "bg-neutral-700 text-neutral-400"
                  }`}
                >
                  {isComplete ? (
                    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
                    </svg>
                  ) : (
                    idx + 1
                  )}
                </div>
                <span
                  className={`mt-1.5 text-xs font-medium ${
                    isComplete
                      ? "text-emerald-400"
                      : isActive
                      ? "text-amber-400"
                      : "text-neutral-500"
                  }`}
                >
                  {wave.label}
                </span>
                {/* Pod badges */}
                <div className="flex gap-1 mt-1">
                  {wave.pods.map((podNum) => {
                    const isFailed = failedPods.includes(`pod_${podNum}`) || failedPods.includes(String(podNum));
                    return (
                      <span
                        key={podNum}
                        className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] font-medium ${
                          isFailed
                            ? "bg-red-900/50 text-red-400"
                            : isComplete
                            ? "bg-emerald-900/30 text-emerald-400"
                            : isActive
                            ? "bg-amber-900/30 text-amber-400"
                            : "bg-neutral-800 text-neutral-500"
                        }`}
                      >
                        Pod {podNum}
                      </span>
                    );
                  })}
                </div>
                {/* Draining status for pods in active wave */}
                {isActive && (
                  <div className="mt-2 space-y-1">
                    {wave.pods.map((podNum) => {
                      // Look up billing session for this pod
                      // billingTimers is keyed by pod_id — try common formats
                      const session =
                        billingTimers.get(`pod-${podNum}`) ||
                        billingTimers.get(`pod_${podNum}`) ||
                        billingTimers.get(String(podNum));
                      if (!session) return null;
                      return (
                        <div
                          key={`drain-${podNum}`}
                          className="text-[10px] bg-amber-900/20 border border-amber-800/30 rounded px-2 py-1"
                        >
                          <span className="inline-flex items-center px-1 py-0.5 rounded bg-amber-900/50 text-amber-400 text-[10px] font-medium mr-1">
                            Draining
                          </span>
                          <span className="text-amber-300">
                            {session.driver_name} - {Math.ceil(session.remaining_seconds / 60)}min left
                          </span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
              {/* Connecting line */}
              {idx < WAVES.length - 1 && (
                <div
                  className={`h-0.5 flex-1 mx-2 ${
                    idx < wavesCompleted ? "bg-emerald-500" : "bg-neutral-700"
                  }`}
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

export default function OtaReleasesPage() {
  const [status, setStatus] = useState<OtaStatusResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [tomlText, setTomlText] = useState("");
  const [deploying, setDeploying] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const pollStatus = useCallback(async () => {
    try {
      const res = await api.getOtaStatus();
      setStatus(res);
      setLoading(false);
    } catch {
      setLoading(false);
    }
  }, []);

  // Initial fetch
  useEffect(() => {
    pollStatus();
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [pollStatus]);

  // Adaptive polling interval
  useEffect(() => {
    if (intervalRef.current) clearInterval(intervalRef.current);
    const active = status && isDeployRecord(status) && isActiveState(status.state);
    const interval = active ? 3000 : 30000;
    intervalRef.current = setInterval(pollStatus, interval);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [status, pollStatus]);

  const handleDeploy = async () => {
    if (!tomlText.trim()) {
      alert("Please paste a release manifest before deploying.");
      return;
    }
    setDeploying(true);
    try {
      const result = await api.triggerOtaDeploy(tomlText);
      if (result.ok) {
        setTomlText("");
        alert("Deploy started for version " + (result.version || "unknown"));
        pollStatus();
      } else if (result.error) {
        alert(result.error);
      }
    } catch (err) {
      alert("Deploy failed: " + (err instanceof Error ? err.message : "Unknown error"));
    }
    setDeploying(false);
  };

  const handleRollback = () => {
    if (!window.confirm("Are you sure? This will revert all pods to the previous binary.")) {
      return;
    }
    // Rollback endpoint may not exist yet — placeholder until backend exposes it
    alert("Rollback endpoint not yet implemented");
  };

  // Extract pipeline data
  const record = status && isDeployRecord(status) ? status : null;
  const pipelineState: PipelineState = record ? record.state : "idle";
  const pipelineActive = record ? isActiveState(record.state) : false;

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">OTA Releases</h1>
        <p className="text-sm text-rp-grey">Deploy and monitor over-the-air updates</p>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading pipeline status...</div>
      ) : (
        <div className="space-y-6">
          {/* Section 1: Pipeline Status */}
          <div className="bg-rp-card border border-rp-border rounded-lg p-5">
            <h2 className="text-sm font-medium text-neutral-400 mb-4">Pipeline Status</h2>
            <div className="flex items-center gap-4 mb-4">
              <span
                className={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-sm font-bold ${stateColors(pipelineState)} ${
                  pipelineActive ? "animate-pulse" : ""
                }`}
              >
                <span
                  className={`w-2 h-2 rounded-full ${
                    pipelineState === "idle" || pipelineState === "completed"
                      ? "bg-emerald-400"
                      : pipelineState === "rolling_back"
                      ? "bg-red-400"
                      : "bg-amber-400"
                  }`}
                />
                {pipelineState.replace(/_/g, " ").toUpperCase()}
              </span>
              {record && (
                <span className="text-xs text-neutral-500">
                  v{record.manifest_version}
                </span>
              )}
            </div>

            {record && (
              <div className="grid grid-cols-2 lg:grid-cols-4 gap-4 text-sm">
                <div>
                  <span className="text-rp-grey text-xs block">Version</span>
                  <span className="text-neutral-300 font-mono">{record.manifest_version}</span>
                </div>
                <div>
                  <span className="text-rp-grey text-xs block">Started</span>
                  <span className="text-neutral-300">{formatIST(record.started_at)}</span>
                </div>
                <div>
                  <span className="text-rp-grey text-xs block">Last Updated</span>
                  <span className="text-neutral-300">{formatIST(record.updated_at)}</span>
                </div>
                <div>
                  <span className="text-rp-grey text-xs block">Waves Complete</span>
                  <span className="text-neutral-300 font-mono">{record.waves_completed} / 3</span>
                </div>
              </div>
            )}

            {/* Rollback reason */}
            {record?.rollback_reason && (
              <div className="mt-4 p-3 bg-red-900/30 border border-red-800/50 rounded-lg">
                <p className="text-sm text-red-400 font-medium">Rollback Reason</p>
                <p className="text-sm text-red-300 mt-1">{record.rollback_reason}</p>
              </div>
            )}

            {/* Failed pods */}
            {record && record.failed_pods.length > 0 && (
              <div className="mt-4">
                <span className="text-xs text-red-400 font-medium block mb-1">Failed Pods</span>
                <div className="flex gap-2">
                  {record.failed_pods.map((pod) => (
                    <span
                      key={pod}
                      className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-red-900/50 text-red-400"
                    >
                      {pod}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {/* Idle message */}
            {!record && (
              <p className="text-sm text-neutral-500">
                No deployments have been triggered. Paste a release manifest below to start.
              </p>
            )}
          </div>

          {/* Section 2: Wave Progress Stepper */}
          <div className="bg-rp-card border border-rp-border rounded-lg p-5">
            <h2 className="text-sm font-medium text-neutral-400 mb-4">Wave Progress</h2>
            <WaveStepper
              wavesCompleted={record?.waves_completed ?? 0}
              pipelineActive={pipelineActive}
              failedPods={record?.failed_pods ?? []}
            />
          </div>

          {/* Section 3: Deploy Controls */}
          <div className="bg-rp-card border border-rp-border rounded-lg p-5">
            <h2 className="text-sm font-medium text-neutral-400 mb-4">Deploy Controls</h2>

            <div className="space-y-4">
              {/* Rollback button */}
              {pipelineActive && (
                <div>
                  <button
                    onClick={handleRollback}
                    className="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-sm font-medium rounded transition-colors"
                  >
                    Rollback
                  </button>
                  <p className="text-xs text-neutral-500 mt-1">
                    Revert all pods to the previous binary version.
                  </p>
                </div>
              )}

              {/* Deploy trigger */}
              <div>
                <label className="text-xs text-neutral-400 block mb-2">
                  Release Manifest (TOML)
                </label>
                <textarea
                  value={tomlText}
                  onChange={(e) => setTomlText(e.target.value)}
                  placeholder={TOML_PLACEHOLDER}
                  rows={10}
                  className="w-full bg-neutral-900 border border-rp-border rounded-lg p-3 text-sm text-neutral-300 font-mono placeholder-neutral-600 focus:outline-none focus:border-rp-red/50 resize-y"
                />
                <div className="flex items-center gap-3 mt-3">
                  <button
                    onClick={handleDeploy}
                    disabled={deploying || pipelineActive || !tomlText.trim()}
                    className={`px-4 py-2 text-sm font-medium rounded transition-colors ${
                      deploying || pipelineActive || !tomlText.trim()
                        ? "bg-neutral-700 text-neutral-500 cursor-not-allowed"
                        : "bg-emerald-600 hover:bg-emerald-500 text-white"
                    }`}
                  >
                    {deploying ? "Deploying..." : "Deploy"}
                  </button>
                  {pipelineActive && (
                    <span className="text-xs text-amber-400">
                      Deploy disabled while pipeline is active
                    </span>
                  )}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}
    </DashboardLayout>
  );
}
