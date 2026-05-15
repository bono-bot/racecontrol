"use client";

import { useState, useEffect, useCallback } from "react";
import { useRouter } from "next/navigation";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { KioskHeader } from "@/components/KioskHeader";
import { PodKioskView } from "@/components/PodKioskView";
import { SidePanel } from "@/components/SidePanel";
import { CafeMenuPanel } from "@/components/CafeMenuPanel";
import { ConfirmDialog, type ConfirmDialogState } from "@/components/ConfirmDialog";
import { useToast } from "@/components/Toast";
import { api } from "@/lib/api";
import type { KioskExperience, KioskSettings, Pod } from "@/lib/types";

export default function ControlPage() {
  const router = useRouter();
  const [staffName, setStaffName] = useState<string | null>(null);
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);
  const [venueName, setVenueName] = useState("Racing Point");
  const [lockedPods, setLockedPods] = useState<Set<string>>(new Set());
  const [showCafeMenu, setShowCafeMenu] = useState(false);
  const [pendingConfirm, setPendingConfirm] = useState<ConfirmDialogState | null>(null);
  const dismissConfirm = useCallback(() => setPendingConfirm(null), []);

  const { toastError } = useToast();

  const {
    connected,
    pods,
    latestTelemetry,
    billingTimers,
    billingWarnings,
    gameStates,
    pendingAuthTokens,
  } = useKioskSocket();

  // Auth gate — check sessionStorage first, fall back to JWT cookie
  useEffect(() => {
    const name = sessionStorage.getItem("kiosk_staff_name");
    if (name) {
      setStaffName(name);
      return;
    }
    // sessionStorage is tab-scoped — try to recover name from JWT cookie
    const cookie = document.cookie
      .split("; ")
      .find((c) => c.startsWith("kiosk_token="));
    if (cookie) {
      try {
        const token = cookie.split("=")[1];
        const payload = JSON.parse(atob(token.split(".")[1]));
        const cookieName = payload.name || payload.sub || "Staff";
        sessionStorage.setItem("kiosk_staff_name", cookieName);
        setStaffName(cookieName);
        return;
      } catch {
        // malformed JWT — fall through to redirect
      }
    }
    router.replace("/staff");
  }, [router]);

  // Fetch experiences + settings once
  useEffect(() => {
    api.listExperiences()
      .then((res) => setExperiences(res.experiences || []))
      .catch(() => toastError("Failed to load experiences"));
    api.getSettings()
      .then((res) => {
        if (res.settings?.venue_name) setVenueName(res.settings.venue_name);
      })
      .catch(() => toastError("Failed to load settings"));
  }, [toastError]);

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
    try {
      if (pod.status === "disabled") {
        await api.enablePod(pod.id);
      } else {
        await api.disablePod(pod.id);
      }
    } catch (err) {
      toastError(`Toggle pod failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleSelectExperience = async (podId: string, experienceId: string) => {
    try {
      await api.podLaunchExperience(podId, experienceId);
    } catch (err) {
      toastError(`Launch experience failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleWakePod = async (podId: string) => {
    try {
      await api.wakePod(podId);
    } catch (err) {
      toastError(`Wake failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleRestartPod = (podId: string) => {
    setPendingConfirm({
      title: `Restart ${podId}?`,
      description: "Any active session on this pod will be interrupted. The pod will be unreachable for ~90 seconds.",
      confirmLabel: "Restart Pod",
      variant: "destructive",
      onConfirm: async () => {
        try {
          await api.restartPod(podId);
        } catch (err) {
          toastError(`Restart failed: ${err instanceof Error ? err.message : "Network error"}`);
        }
      },
    });
  };

  const handleShutdownPod = (podId: string) => {
    setPendingConfirm({
      title: `Shutdown ${podId}?`,
      description: "The pod will power off. Someone will need to physically press the power button to restart it.",
      confirmLabel: "Shutdown",
      variant: "destructive",
      onConfirm: async () => {
        try {
          await api.shutdownPod(podId);
        } catch (err) {
          toastError(`Shutdown failed: ${err instanceof Error ? err.message : "Network error"}`);
        }
      },
    });
  };

  const handleWakeAll = async () => {
    try {
      await api.wakeAllPods();
    } catch (err) {
      toastError(`Wake all failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleShutdownAll = () => {
    setPendingConfirm({
      title: "Shutdown ALL pods?",
      description: "Every online pod will power off. Someone will need to physically press each pod's power button to restart. This will force-close any active sessions.",
      confirmLabel: "Shutdown All",
      variant: "destructive",
      onConfirm: async () => {
        try {
          await api.shutdownAllPods();
        } catch (err) {
          toastError(`Shutdown all failed: ${err instanceof Error ? err.message : "Network error"}`);
        }
      },
    });
  };

  const handleRestartAll = () => {
    setPendingConfirm({
      title: "Restart ALL pods?",
      description: "Every online pod will restart. Active sessions will be interrupted. Fleet will be unreachable for ~90 seconds.",
      confirmLabel: "Restart All",
      variant: "destructive",
      onConfirm: async () => {
        try {
          await api.restartAllPods();
        } catch (err) {
          toastError(`Restart all failed: ${err instanceof Error ? err.message : "Network error"}`);
        }
      },
    });
  };

  const handleLockAll = () => {
    setPendingConfirm({
      title: "Lock ALL pods?",
      description: "Taskbar and keyboard will be restricted on every online pod. Customers cannot exit kiosk mode until unlocked.",
      confirmLabel: "Lock All",
      variant: "destructive",
      onConfirm: async () => {
        const allOnlineIds = sortedPods
          .filter((p) => p.status !== "offline" && p.status !== "disabled")
          .map((p) => p.id);
        setLockedPods(new Set(allOnlineIds));
        try {
          await api.lockdownAllPods(true);
        } catch (err) {
          toastError(`Lock all failed: ${err instanceof Error ? err.message : "Network error"}`);
        }
      },
    });
  };

  const handleUnlockAll = async () => {
    setLockedPods(new Set());
    try {
      await api.lockdownAllPods(false);
    } catch (err) {
      toastError(`Unlock all failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
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
    try {
      await api.lockdownPod(podId, newLocked);
    } catch (err) {
      toastError(`Lockdown toggle failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
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
          className="px-3 py-1 rounded text-xs font-semibold bg-rp-green/20 text-rp-green border border-rp-green/40 hover:bg-rp-green/30 transition-colors"
        >
          Wake All
        </button>
        <button
          onClick={handleShutdownAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-rp-red/20 text-rp-red border border-rp-red/40 hover:bg-rp-red/30 transition-colors"
        >
          Shutdown All
        </button>
        <button
          onClick={handleRestartAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-rp-yellow/20 text-rp-yellow border border-rp-yellow/40 hover:bg-rp-yellow/30 transition-colors"
        >
          Restart All
        </button>
        <button
          onClick={handleLockAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-rp-orange/20 text-rp-orange border border-rp-orange/40 hover:bg-rp-orange/30 transition-colors"
        >
          Lock All
        </button>
        <button
          onClick={handleUnlockAll}
          className="px-3 py-1 rounded text-xs font-semibold bg-rp-card text-rp-grey border border-rp-border hover:bg-rp-surface transition-colors"
        >
          Unlock All
        </button>
        <div className="flex-1" />
        <button
          onClick={() => setShowCafeMenu((v) => !v)}
          className={`px-3 py-1 rounded text-xs font-semibold border transition-colors ${
            showCafeMenu
              ? "bg-rp-red text-white border-rp-red"
              : "bg-rp-card text-rp-grey border-rp-border hover:text-white hover:border-rp-red"
          }`}
        >
          Cafe Menu
        </button>
      </div>

      <div className="flex-1 flex min-h-0">
        <div className="flex-1 grid grid-cols-4 grid-rows-2 gap-2 p-2 min-h-0 overflow-hidden">
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
                      className="w-6 h-6 flex items-center justify-center rounded text-[10px] bg-rp-green/20 text-rp-green hover:bg-rp-green/30 transition-colors"
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
                      className="w-6 h-6 flex items-center justify-center rounded text-[10px] bg-rp-yellow/20 text-rp-yellow hover:bg-rp-yellow/30 transition-colors"
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
                      className="w-6 h-6 flex items-center justify-center rounded text-[10px] bg-rp-red/20 text-rp-red hover:bg-rp-red/30 transition-colors"
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
                          ? "bg-rp-orange/30 text-rp-orange hover:bg-rp-orange/40"
                          : "bg-rp-orange/20 text-rp-orange hover:bg-rp-orange/30"
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
                        ? "bg-rp-card text-rp-grey hover:bg-rp-surface"
                        : "bg-rp-green/20 text-rp-green hover:bg-rp-green/30"
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
        <SidePanel
          isOpen={showCafeMenu}
          title="Cafe Menu"
          subtitle="Available items and prices"
          onClose={() => setShowCafeMenu(false)}
        >
          <CafeMenuPanel />
        </SidePanel>
      </div>

      {/* Destructive-action confirmation modal (replaces window.confirm) */}
      <ConfirmDialog state={pendingConfirm} onDismiss={dismissConfirm} />
    </div>
  );
}
