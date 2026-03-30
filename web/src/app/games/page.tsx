"use client";

import { useState } from "react";
import Link from "next/link";
import DashboardLayout from "@/components/DashboardLayout";
import { EmptyState } from "@/components/Skeleton";
import StatusBadge from "@/components/StatusBadge";
import GameLaunchModal from "@/components/GameLaunchModal";
import AiDebugPanel from "@/components/AiDebugPanel";
import { Gamepad2 } from "lucide-react";
import { useWebSocket } from "@/hooks/useWebSocket";
import { api } from "@/lib/api";
import type { Pod } from "@/lib/api";

const simLabels: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  assetto_corsa_evo: "AC EVO",
  assetto_corsa_rally: "AC Rally",
  f1_25: "F1 25",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  forza: "Forza Motorsport",
  forza_horizon_5: "Forza Horizon 5",
};

export default function GamesPage() {
  const { pods, gameStates, aiDebugSuggestions } = useWebSocket();
  const [modalPod, setModalPod] = useState<Pod | null>(null);
  const [stopping, setStopping] = useState<Set<string>>(new Set());

  const sortedPods = [...pods].sort((a, b) => a.number - b.number);
  const activeCount = Array.from(gameStates.values()).filter(
    (g) => g.game_state !== "idle"
  ).length;

  async function handleLaunch(simType: string, launchArgs?: string) {
    if (!modalPod) return;
    try {
      await api.launchGame(modalPod.id, simType, launchArgs);
    } catch (e) {
      console.error("Failed to launch game:", e);
    }
    setModalPod(null);
  }

  async function handleStop(podId: string) {
    setStopping((prev) => new Set(prev).add(podId));
    try {
      await api.stopGame(podId);
    } catch (e) {
      console.error("Failed to stop game:", e);
    }
    setTimeout(() => {
      setStopping((prev) => {
        const next = new Set(prev);
        next.delete(podId);
        return next;
      });
    }, 3000);
  }

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Game Launcher</h1>
          <p className="text-sm text-rp-grey">
            Remote launch and monitor games on pods
          </p>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-rp-grey">
            {activeCount} game{activeCount !== 1 ? "s" : ""} running
          </span>
          <Link
            href="/games/reliability"
            className="text-xs px-3 py-1.5 rounded-lg bg-rp-card border border-rp-border text-neutral-300 hover:border-rp-red hover:text-white transition-colors"
          >
            Reliability Matrix
          </Link>
        </div>
      </div>

      {/* AI Debug Suggestions */}
      <AiDebugPanel suggestions={aiDebugSuggestions} pods={pods} />

      {/* Pod Grid */}
      {sortedPods.length === 0 ? (
        <EmptyState
          icon={<Gamepad2 className="w-10 h-10" />}
          headline="No pods connected"
          hint="Pods appear automatically when rc-agent connects from a sim PC."
        />
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {sortedPods.map((pod) => {
            const gameInfo = gameStates.get(pod.id);
            const gameState = gameInfo?.game_state || "idle";
            const isRunning =
              gameState === "running" || gameState === "launching";
            const isError = gameState === "error";
            const isStopping = stopping.has(pod.id) || gameState === "stopping";

            return (
              <div
                key={pod.id}
                className={`rounded-lg border p-4 transition-all ${
                  isRunning
                    ? "border-emerald-500/50 bg-emerald-500/5"
                    : isError
                    ? "border-red-500/50 bg-red-500/5"
                    : isStopping
                    ? "border-amber-500/50 bg-amber-500/5"
                    : pod.status === "offline"
                    ? "border-rp-border bg-rp-card/50"
                    : "border-rp-border bg-rp-card"
                }`}
              >
                {/* Pod header */}
                <div className="flex items-center justify-between mb-3">
                  <div className="flex items-center gap-2">
                    <span className="text-xl font-bold text-neutral-300">
                      {String(pod.number).padStart(2, "0")}
                    </span>
                    <span className="text-sm text-rp-grey">{pod.name}</span>
                  </div>
                  <StatusBadge
                    status={
                      gameState !== "idle" ? gameState : pod.status
                    }
                  />
                </div>

                {/* Game info when active */}
                {gameInfo && gameState !== "idle" && (
                  <div className="space-y-1.5 text-xs mb-3">
                    <div className="flex justify-between">
                      <span className="text-rp-grey">Game</span>
                      <span className="text-emerald-400 font-medium">
                        {simLabels[gameInfo.sim_type] || gameInfo.sim_type}
                      </span>
                    </div>
                    <div className="flex justify-between items-center">
                      <span className="text-rp-grey">State</span>
                      <StatusBadge status={gameInfo.game_state} />
                    </div>
                    {gameInfo.pid && (
                      <div className="flex justify-between">
                        <span className="text-rp-grey">PID</span>
                        <span className="text-neutral-400 font-mono">
                          {gameInfo.pid}
                        </span>
                      </div>
                    )}
                    {gameInfo.launched_at && (
                      <div className="flex justify-between">
                        <span className="text-rp-grey">Launched</span>
                        <span className="text-neutral-400">
                          {new Date(gameInfo.launched_at).toLocaleTimeString()}
                        </span>
                      </div>
                    )}
                    {isError && gameInfo.error_message && (
                      <div className="bg-red-500/10 border border-red-500/20 rounded px-2 py-1.5 mt-1">
                        <span className="text-red-400 text-xs">
                          {gameInfo.error_message}
                        </span>
                      </div>
                    )}
                  </div>
                )}

                {/* Pod meta when idle */}
                {(!gameInfo || gameState === "idle") && (
                  <div className="space-y-1.5 text-xs mb-3">
                    <div className="flex justify-between">
                      <span className="text-rp-grey">Sim</span>
                      <span className="text-neutral-300">
                        {simLabels[pod.sim_type] || pod.sim_type}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span className="text-rp-grey">IP</span>
                      <span className="text-neutral-400 font-mono">
                        {pod.ip_address || "\u2014"}
                      </span>
                    </div>
                  </div>
                )}

                {/* Action buttons */}
                <div className="pt-1">
                  {isRunning || isStopping ? (
                    <button
                      onClick={() => handleStop(pod.id)}
                      disabled={isStopping}
                      className={`w-full rounded-lg py-2.5 text-sm font-semibold transition-all ${
                        isStopping
                          ? "bg-rp-card text-rp-grey cursor-not-allowed"
                          : "bg-red-500/20 text-red-400 hover:bg-red-500/30"
                      }`}
                    >
                      {isStopping ? "Stopping..." : "Stop Game"}
                    </button>
                  ) : isError ? (
                    <div className="flex gap-2">
                      <button
                        onClick={() => setModalPod(pod)}
                        className="flex-1 rounded-lg py-2.5 text-sm font-semibold bg-rp-red text-white hover:bg-rp-red active:bg-rp-red transition-all"
                      >
                        Retry
                      </button>
                      <button
                        onClick={() => handleStop(pod.id)}
                        className="rounded-lg px-3 py-2.5 text-sm font-medium bg-rp-card text-neutral-400 hover:bg-rp-card transition-colors"
                      >
                        Clear
                      </button>
                    </div>
                  ) : (
                    <button
                      onClick={() => setModalPod(pod)}
                      disabled={pod.status === "offline"}
                      className={`w-full rounded-lg py-2.5 text-sm font-semibold transition-all ${
                        pod.status === "offline"
                          ? "bg-rp-card text-rp-grey cursor-not-allowed"
                          : "bg-rp-red text-white hover:bg-rp-red active:bg-rp-red"
                      }`}
                    >
                      Launch Game
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Launch Modal */}
      {modalPod && (
        <GameLaunchModal
          podId={modalPod.id}
          podName={`Pod ${String(modalPod.number).padStart(2, "0")} - ${modalPod.name}`}
          onClose={() => setModalPod(null)}
          onLaunch={handleLaunch}
        />
      )}
    </DashboardLayout>
  );
}
