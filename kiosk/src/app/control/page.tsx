"use client";

import { useState, useEffect } from "react";
import { useRouter } from "next/navigation";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { KioskHeader } from "@/components/KioskHeader";
import { PodKioskView } from "@/components/PodKioskView";
import { api } from "@/lib/api";
import type { KioskExperience, KioskSettings, Pod } from "@/lib/types";

export default function ControlPage() {
  const router = useRouter();
  const [staffName, setStaffName] = useState<string | null>(null);
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);
  const [venueName, setVenueName] = useState("Racing Point");
  const [lockedPods, setLockedPods] = useState<Set<string>>(new Set());

  const {
    connected,
    pods,
    latestTelemetry,
    billingTimers,
    billingWarnings,
    gameStates,
    pendingAuthTokens,
  } = useKioskSocket();

  // Auth gate
  useEffect(() => {
    const name = sessionStorage.getItem("kiosk_staff_name");
    if (!name) {
      router.replace("/staff");
      return;
    }
    setStaffName(name);
  }, [router]);

  // Fetch experiences + settings once
  useEffect(() => {
    api.listExperiences().then((res) => setExperiences(res.experiences || []));
    api.getSettings().then((res) => {
      if (res.settings?.venue_name) setVenueName(res.settings.venue_name);
    });
  }, []);

  const handleSignOut = () => {
    sessionStorage.removeItem("kiosk_staff_name");
    sessionStorage.removeItem("kiosk_staff_id");
    sessionStorage.removeItem("kiosk_staff_token");
    router.replace("/staff");
  };

  // Sort pods by number
  const sortedPods = Array.from(pods.values()).sort((a, b) => a.number - b.number);
  // Ensure we always show 8 slots
  const podSlots: (Pod | null)[] = [];
  for (let i = 1; i <= 8; i++) {
    podSlots.push(sortedPods.find((p) => p.number === i) || null);
  }

  const handleTogglePod = async (pod: Pod) => {
    if (pod.status === "disabled") {
      await api.enablePod(pod.id);
    } else {
      await api.disablePod(pod.id);
    }
  };

  const handleSelectExperience = async (podId: string, experienceId: string) => {
    await api.podLaunchExperience(podId, experienceId);
  };

  const handleWakePod = async (podId: string) => {
    await api.wakePod(podId);
  };

  const handleRestartPod = async (podId: string) => {
    await api.restartPod(podId);
  };

  const handleShutdownPod = async (podId: string) => {
    if (!window.confirm("Shutdown this pod?")) return;
    await api.shutdownPod(podId);
  };

  const handleWakeAll = async () => {
    await api.wakeAllPods();
  };

  const handleShutdownAll = async () => {
    if (!window.confirm("Shutdown ALL pods? This will force-close everything.")) return;
    await api.shutdownAllPods();
  };

  const handleRestartAll = async () => {
    if (!window.confirm("Restart ALL pods? Active sessions will be interrupted.")) return;
    await api.restartAllPods();
  };

  const handleLockAll = async () => {
    if (!window.confirm("Lock ALL pods? Taskbar and keyboard will be restricted.")) return;
    const allOnlineIds = sortedPods
      .filter((p) => p.status !== "offline" && p.status !== "disabled")
      .map((p) => p.id);
    setLockedPods(new Set(allOnlineIds));
    await api.lockdownAllPods(true);
  };

  const handleUnlockAll = async () => {
    setLockedPods(new Set());
    await api.lockdownAllPods(false);
  };

  const handleToggleLockdown = async (podId: string) => {
    const isCurrentlyLocked = lockedPods.has(podId);
    const newLocked = !isCurrentlyLocked;
    setLockedPods((prev) => {
      const next = new Set(prev);
      if (newLocked) next.add(podId);
      else next.delete(podId);
      return next;
    });
    await api.lockdownPod(podId, newLocked);
  };

  if (!staffName) return null;

  return (
    <div className="h-screen flex flex-col bg-rp-black">
      <KioskHeader
        connected={connected}
        pods={pods}
        venueName={venueName}
        staffName={staffName}
        onSignOut={handleSignOut}
      />

      {/* Bulk power actions */}
      <div className="flex items-center gap-2 px-2 pt-2">
        <button
          onClick={handleWakeAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-green-900/50 text-green-400 border border-green-800 hover:bg-green-800/60 transition-colors"
        >
          Wake All
        </button>
        <button
          onClick={handleShutdownAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-red-900/50 text-red-400 border border-red-800 hover:bg-red-800/60 transition-colors"
        >
          Shutdown All
        </button>
        <button
          onClick={handleRestartAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-yellow-900/50 text-yellow-400 border border-yellow-800 hover:bg-yellow-800/60 transition-colors"
        >
          Restart All
        </button>
        <button
          onClick={handleLockAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-orange-900/50 text-orange-400 border border-orange-800 hover:bg-orange-800/60 transition-colors"
        >
          Lock All
        </button>
        <button
          onClick={handleUnlockAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-zinc-700 text-zinc-300 border border-zinc-600 hover:bg-zinc-600 transition-colors"
        >
          Unlock All
        </button>
      </div>

      <div className="flex-1 grid grid-cols-4 grid-rows-2 gap-2 p-2 min-h-0">
        {podSlots.map((pod, i) => {
          const podNumber = i + 1;
          if (!pod) {
            return (
              <div
                key={podNumber}
                className="bg-rp-card rounded-lg border border-rp-border flex flex-col items-center justify-center opacity-40"
              >
                <p className="text-sm text-rp-grey">Pod {podNumber}</p>
                <p className="text-[10px] text-rp-grey">Offline</p>
              </div>
            );
          }

          const billing = billingTimers.get(pod.id);
          const telemetry = latestTelemetry.get(pod.id);
          const gameInfo = gameStates.get(pod.id);
          const authToken = pendingAuthTokens.get(pod.id);
          const warning = billingWarnings.find((w) => w.podId === pod.id);
          const isOnline = pod.status !== "offline" && pod.status !== "disabled";

          const isIdle = pod.status === "idle" && !billing && !authToken;

          return (
            <div key={pod.id} className="flex flex-col min-h-0">
              {/* Cell header */}
              <div className="flex items-center justify-between px-2 py-1 bg-rp-card border border-rp-border border-b-0 rounded-t-lg">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-bold text-white">Pod {pod.number}</span>
                  {/* Book/Launch — shown when pod is idle and online */}
                  {isOnline && isIdle && (
                    <button
                      onClick={() => router.push(`/book?pod=${pod.id}&staff=true`)}
                      className="px-2 py-0.5 rounded text-[10px] font-semibold bg-rp-red/20 text-rp-red border border-rp-red/50 hover:bg-rp-red hover:text-white transition-colors"
                    >
                      Book
                    </button>
                  )}
                </div>
                <div className="flex items-center gap-1">
                  {/* Power On (WOL) — shown when offline/disabled */}
                  {!isOnline && (
                    <button
                      onClick={() => handleWakePod(pod.id)}
                      className="w-6 h-6 flex items-center justify-center rounded text-[10px] bg-green-900/40 text-green-400 hover:bg-green-800/50 transition-colors"
                      title="Power on (WOL)"
                    >
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
                      </svg>
                    </button>
                  )}
                  {/* Restart — shown when online */}
                  {isOnline && (
                    <button
                      onClick={() => handleRestartPod(pod.id)}
                      className="w-6 h-6 flex items-center justify-center rounded text-[10px] bg-yellow-900/40 text-yellow-400 hover:bg-yellow-800/50 transition-colors"
                      title="Restart pod"
                    >
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                      </svg>
                    </button>
                  )}
                  {/* Shutdown — shown when online */}
                  {isOnline && (
                    <button
                      onClick={() => handleShutdownPod(pod.id)}
                      className="w-6 h-6 flex items-center justify-center rounded text-[10px] bg-red-900/40 text-red-400 hover:bg-red-800/50 transition-colors"
                      title="Shutdown pod"
                    >
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M5.636 5.636a9 9 0 1012.728 0M12 3v9" />
                      </svg>
                    </button>
                  )}
                  {/* Lockdown toggle — shown when online */}
                  {isOnline && (
                    <button
                      onClick={() => handleToggleLockdown(pod.id)}
                      className={`w-6 h-6 flex items-center justify-center rounded text-[10px] transition-colors ${
                        lockedPods.has(pod.id)
                          ? "bg-orange-700/60 text-orange-300 hover:bg-orange-600/50"
                          : "bg-orange-900/40 text-orange-400 hover:bg-orange-800/50"
                      }`}
                      title={lockedPods.has(pod.id) ? "Unlock kiosk" : "Lock kiosk"}
                    >
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        {lockedPods.has(pod.id) ? (
                          <path strokeLinecap="round" strokeLinejoin="round" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
                        ) : (
                          <path strokeLinecap="round" strokeLinejoin="round" d="M8 11V7a4 4 0 118 0m-4 8v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2z" />
                        )}
                      </svg>
                    </button>
                  )}
                  {/* Enable/Disable toggle */}
                  <button
                    onClick={() => handleTogglePod(pod)}
                    className={`w-6 h-6 flex items-center justify-center rounded text-[10px] transition-colors ${
                      pod.status === "disabled"
                        ? "bg-zinc-700 text-rp-grey hover:bg-zinc-600"
                        : "bg-green-900/40 text-green-400 hover:bg-green-800/50"
                    }`}
                    title={pod.status === "disabled" ? "Enable kiosk" : "Disable kiosk"}
                  >
                    {pod.status === "disabled" ? (
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M18.364 18.364A9 9 0 005.636 5.636m12.728 12.728A9 9 0 015.636 5.636m12.728 12.728L5.636 5.636" />
                      </svg>
                    ) : (
                      <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
                        <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                      </svg>
                    )}
                  </button>
                </div>
              </div>
              {/* Pod kiosk view */}
              <div className="flex-1 min-h-0 rounded-b-lg overflow-hidden">
                <PodKioskView
                  pod={pod}
                  billing={billing}
                  telemetry={telemetry}
                  gameInfo={gameInfo}
                  authToken={authToken}
                  experiences={experiences}
                  mode="control"
                  onSelectExperience={(expId) => handleSelectExperience(pod.id, expId)}
                  warning={warning}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
