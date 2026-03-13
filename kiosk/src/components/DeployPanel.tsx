"use client";

import { useState } from "react";
import type { DeployState } from "@/lib/types";

// ─── Color mapping for deploy states ──────────────────────────────────────────

function deployStateColor(state: DeployState): string {
  switch (state.state) {
    case "idle":
      return "#5A5A5A"; // Gunmetal Grey
    case "waiting_session":
      return "#5A5A5A"; // Grey — queued
    case "complete":
      return "#22c55e"; // Green
    case "failed":
      return "#E10600"; // Racing Red
    case "killing":
    case "waiting_dead":
    case "downloading":
    case "size_check":
    case "starting":
    case "verifying_health":
      return "#eab308"; // Yellow — in-progress
    default:
      return "#5A5A5A";
  }
}

function deployStateLabel(state: DeployState): string {
  switch (state.state) {
    case "idle":
      return "Idle";
    case "killing":
      return "Stopping";
    case "waiting_dead":
      return "Stopping...";
    case "downloading":
      return `Downloading (${state.detail.progress_pct}%)`;
    case "size_check":
      return "Verifying";
    case "starting":
      return "Starting";
    case "verifying_health":
      return "Checking";
    case "complete":
      return "Done";
    case "failed":
      return "Failed";
    case "waiting_session":
      return "Queued";
    default:
      return "Unknown";
  }
}

// ─── DeployPodCard ────────────────────────────────────────────────────────────

interface DeployPodCardProps {
  podId: string;
  podNumber: number;
  state: DeployState;
  isCanary: boolean;
}

function DeployPodCard({ podNumber, state, isCanary }: DeployPodCardProps) {
  const color = deployStateColor(state);
  const label = deployStateLabel(state);

  return (
    <div
      className="relative rounded p-3 border"
      style={{
        backgroundColor: "#1A1A1A",
        borderColor: color,
      }}
    >
      {isCanary && (
        <span
          className="absolute top-1 right-1 text-[9px] px-1 rounded"
          style={{ backgroundColor: "#E10600", color: "white" }}
        >
          CANARY
        </span>
      )}
      <div className="text-xs text-gray-400 mb-1">Pod {podNumber}</div>
      <div
        className="text-sm font-semibold"
        style={{ color }}
      >
        {label}
      </div>
      {state.state === "failed" && (
        <div className="text-[10px] text-gray-500 mt-1 truncate" title={state.detail.reason}>
          {state.detail.reason}
        </div>
      )}
      {state.state === "waiting_session" && (
        <div className="text-[10px] text-gray-500 mt-1">
          Waiting for session
        </div>
      )}
    </div>
  );
}

// ─── DeployPanel ──────────────────────────────────────────────────────────────

interface DeployPanelProps {
  deployStates: Map<string, DeployState>;
  onDeploy: (binaryUrl: string) => void;
}

const IDLE_STATE: DeployState = { state: "idle" };

export function DeployPanel({ deployStates, onDeploy }: DeployPanelProps) {
  const [binaryUrl, setBinaryUrl] = useState(
    "http://192.168.31.27:9998/rc-agent.exe"
  );
  const [deploying, setDeploying] = useState(false);

  // Detect when all pods return to idle/complete after a deploy
  const anyActive = Array.from(deployStates.values()).some(
    (s) =>
      s.state !== "idle" &&
      s.state !== "complete" &&
      s.state !== "failed" &&
      s.state !== "waiting_session"
  );

  // Reset deploying flag when all activity stops
  if (deploying && !anyActive) {
    setDeploying(false);
  }

  const handleDeploy = () => {
    if (!binaryUrl.trim()) return;
    setDeploying(true);
    onDeploy(binaryUrl.trim());
  };

  return (
    <div className="space-y-4">
      <div>
        <h2
          className="text-lg font-semibold text-white mb-1"
          style={{ fontFamily: "Enthocentric, sans-serif" }}
        >
          Deploy rc-agent
        </h2>
        <p className="text-xs text-gray-400">
          Deploys canary to Pod 8 first. Pods with active billing are queued
          and deployed automatically when their session ends.
        </p>
      </div>

      {/* URL Input + Deploy Button */}
      <div className="flex gap-2">
        <input
          type="text"
          value={binaryUrl}
          onChange={(e) => setBinaryUrl(e.target.value)}
          placeholder="http://192.168.31.27:9998/rc-agent.exe"
          className="flex-1 px-3 py-2 text-sm text-white rounded border focus:outline-none focus:border-rp-red"
          style={{
            backgroundColor: "#1A1A1A",
            borderColor: "#333333",
          }}
        />
        <button
          onClick={handleDeploy}
          disabled={deploying || !binaryUrl.trim()}
          className="px-4 py-2 rounded text-sm font-semibold text-white transition-opacity disabled:opacity-40"
          style={{ backgroundColor: "#E10600" }}
        >
          {deploying ? "Deploying..." : "Deploy All"}
        </button>
      </div>

      {/* Per-pod progress cards */}
      <div className="grid grid-cols-4 gap-2">
        {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => {
          const podId = `pod_${n}`;
          const state = deployStates.get(podId) ?? IDLE_STATE;
          return (
            <DeployPodCard
              key={podId}
              podId={podId}
              podNumber={n}
              state={state}
              isCanary={n === 8}
            />
          );
        })}
      </div>
    </div>
  );
}
