"use client";

import { useState, useEffect, useCallback } from "react";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { useSetupWizard } from "@/hooks/useSetupWizard";
import { KioskHeader } from "@/components/KioskHeader";
import { KioskPodCard } from "@/components/KioskPodCard";
import { SidePanel } from "@/components/SidePanel";
import { SetupWizard } from "@/components/SetupWizard";
import { LiveSessionPanel } from "@/components/LiveSessionPanel";
import { StaffLoginScreen } from "@/components/StaffLoginScreen";
import { AssistanceAlert } from "@/components/AssistanceAlert";
import { GamePickerPanel } from "@/components/GamePickerPanel";
import { GameLaunchRequestBanner } from "@/components/GameLaunchRequestBanner";
import { useToast } from "@/components/Toast";
import { api } from "@/lib/api";
import type { AuthTokenInfo, PanelMode, RecentSession } from "@/lib/types";

export default function StaffTerminal() {
  const [staffName, setStaffName] = useState<string | null>(null);
  const [staffId, setStaffId] = useState<string | null>(null);
  const [hydrated, setHydrated] = useState(false);

  const { toastError } = useToast();

  // Restore auth from sessionStorage after hydration (SSR can't access sessionStorage)
  useEffect(() => {
    setStaffName(sessionStorage.getItem("kiosk_staff_name"));
    setStaffId(sessionStorage.getItem("kiosk_staff_id"));
    setHydrated(true);
  }, []);

  // 30-minute inactivity timeout — auto-logout staff
  useEffect(() => {
    if (!staffName) return;
    let timer: ReturnType<typeof setTimeout>;
    const resetTimer = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        sessionStorage.removeItem("kiosk_staff_name");
        sessionStorage.removeItem("kiosk_staff_id");
        sessionStorage.removeItem("kiosk_staff_token");
        // SEC-P2-9: Clear server-side auth cookie on logout/timeout
        document.cookie = "kiosk_staff_jwt=; path=/; max-age=0; SameSite=Strict";
        setStaffName(null);
        setStaffId(null);
      }, 30 * 60 * 1000);
    };
    resetTimer();
    const events = ["pointerdown", "keydown"] as const;
    events.forEach((e) => window.addEventListener(e, resetTimer));
    return () => {
      clearTimeout(timer);
      events.forEach((e) => window.removeEventListener(e, resetTimer));
    };
  }, [staffName]);

  const {
    connected,
    pods,
    latestTelemetry,
    billingTimers,
    billingWarnings,
    gameStates,
    pendingAuthTokens,
    assistanceRequests,
    dismissAssistance,
    gameLaunchRequests,
    dismissGameRequest,
    sendCommand,
    pendingSplitContinuation,
    clearPendingSplitContinuation,
    acServerInfo,
    multiplayerGroup,
  } = useKioskSocket();

  // ─── Panel State ──────────────────────────────────────────────────────
  const [selectedPodId, setSelectedPodId] = useState<string | null>(null);
  const [panelMode, setPanelMode] = useState<PanelMode>(null);

  // Pending assign: holds driver/pricing data until game is selected, then billing starts
  const [pendingAssign, setPendingAssign] = useState<{
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    driver_name: string;
  } | null>(null);


  // Recent sessions + refund state
  const [recentSessions, setRecentSessions] = useState<RecentSession[]>([]);
  const [recentSessionsOpen, setRecentSessionsOpen] = useState(false);
  const [refundTarget, setRefundTarget] = useState<RecentSession | null>(null);

  // Setup wizard hook
  const wizard = useSetupWizard();

  // Fetch recent completed sessions
  const fetchRecentSessions = useCallback(async () => {
    try {
      const res = await api.recentSessions(10);
      setRecentSessions(res.sessions || []);
    } catch {
      // Non-critical — silent fail
    }
  }, []);

  useEffect(() => {
    fetchRecentSessions();
    const interval = setInterval(fetchRecentSessions, 30000);
    return () => clearInterval(interval);
  }, [fetchRecentSessions]);

  // Sort pods by number for consistent 4x2 grid
  const sortedPods = Array.from(pods.values()).sort((a, b) => a.number - b.number);
  const displayPods = sortedPods.length > 0 ? sortedPods : [];
  const isPanelOpen = panelMode !== null && selectedPodId !== null;
  const selectedPod = selectedPodId ? pods.get(selectedPodId) : null;

  // ─── Panel Mode Derivation ────────────────────────────────────────────
  const handlePodSelect = (podId: string) => {
    const pod = pods.get(podId);
    if (!pod) return;

    const billing = billingTimers.get(podId);
    const authToken = pendingAuthTokens.get(podId);

    setSelectedPodId(podId);

    if (billing && (billing.status === "active" || billing.status === "paused_manual")) {
      setPanelMode("live_session");
    } else if (authToken && authToken.status === "pending") {
      setPanelMode("waiting");
    } else {
      // Idle or ending — start setup
      setPanelMode("setup");
      wizard.reset();
    }
  };

  const handleStartSession = (podId: string) => {
    setSelectedPodId(podId);
    setPanelMode("setup");
    wizard.reset();
  };

  const closePanel = () => {
    setSelectedPodId(null);
    setPanelMode(null);
    setPendingAssign(null);
  };

  // ─── Split Continuation ──────────────────────────────────────────────
  // When a sub-session completes and more splits remain, auto-open the setup wizard
  // for the same pod so staff can pick the next track/car.
  const [isSplitContinuation, setIsSplitContinuation] = useState(false);
  const [isLaunching, setIsLaunching] = useState(false);

  useEffect(() => {
    if (pendingSplitContinuation) {
      setSelectedPodId(pendingSplitContinuation.pod_id);
      setIsSplitContinuation(true);
      wizard.reset();
      // Pre-select AC as the game (splits are AC-only)
      wizard.setField("selectedGame", "assetto_corsa");
      setPanelMode("setup");
      // Skip to track/car selection — game is already known
      wizard.goToStep("select_track");
    }
  }, [pendingSplitContinuation]);

  // ─── Session Controls ─────────────────────────────────────────────────
  const handleGameLaunch = async (simType: string, launchArgs: string) => {
    if (!selectedPodId || isLaunching) return;
    setIsLaunching(true);

    try {
      if (isSplitContinuation && pendingSplitContinuation) {
        // Split continuation — no billing start, just continue the split
        try {
          const result = await api.continueSplit({
            pod_id: selectedPodId,
            sim_type: simType,
            launch_args: launchArgs,
          });
          if (result.error) {
            toastError(`Continue split failed: ${result.error}`);
            return;
          }
        } catch (err) {
          toastError(`Failed to continue split: ${err instanceof Error ? err.message : "Network error"}`);
          return;
        }
        setIsSplitContinuation(false);
        clearPendingSplitContinuation();
      } else {
        // Normal billing start
        const driver = wizard.state.selectedDriver;
        const tier = wizard.state.selectedTier;
        if (driver && tier) {
          try {
            const result = await api.startBilling({
              pod_id: selectedPodId,
              driver_id: driver.id,
              pricing_tier_id: tier.id,
              staff_id: staffId || undefined,
              ...(wizard.state.splitCount > 1 && {
                split_count: wizard.state.splitCount,
                split_duration_minutes: wizard.state.splitDurationMinutes ?? undefined,
              }),
            });

            if (result.error) {
              toastError(`Billing failed: ${result.error}`);
              return;
            }
          } catch (err) {
            toastError(`Failed to start billing: ${err instanceof Error ? err.message : "Network error"}`);
            return;
          }
        }

        // Launch game — handle errors to avoid silent failure
        try {
          const launchResult = await api.launchGame(selectedPodId, simType, launchArgs);
          if (!launchResult.ok) {
            toastError(`Game launch failed: ${launchResult.error || "Unknown error"}`);
            return;
          }
          if (launchResult.warning) {
            toastError(launchResult.warning);
          }
        } catch (err) {
          toastError(`Failed to launch game: ${err instanceof Error ? err.message : "Network error"}`);
          return;
        }
      }
      // Switch to live session view
      setPanelMode("live_session");
    } finally {
      setIsLaunching(false);
    }
  };

  const handleEndSession = (billingSessionId: string) => {
    sendCommand("end_billing", { billing_session_id: billingSessionId });
    closePanel();
  };

  const handlePauseSession = (billingSessionId: string) => {
    sendCommand("pause_billing", { billing_session_id: billingSessionId });
  };

  const handleResumeSession = (billingSessionId: string) => {
    sendCommand("resume_billing", { billing_session_id: billingSessionId });
  };

  const handleExtendSession = (billingSessionId: string) => {
    sendCommand("extend_billing", {
      billing_session_id: billingSessionId,
      additional_seconds: 600,
    });
  };

  const handleCancelAssignment = (tokenId: string) => {
    sendCommand("cancel_assignment", { token_id: tokenId });
    closePanel();
  };

  const handleStartNow = async (authToken: AuthTokenInfo) => {
    const result = await api.startNow(authToken.id);
    if (result.error) {
      console.error("Start Now failed:", result.error);
      return;
    }
    // Switch to setup for game config
    setPanelMode("setup");
    wizard.reset();
    // Skip driver registration — driver is already assigned
    wizard.goToStep("select_game");
  };

  const handleWakePod = async (podId: string) => {
    try {
      await api.wakePod(podId);
    } catch (err) {
      toastError(`Wake failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleRestartPod = async (podId: string) => {
    try {
      await api.restartPod(podId);
    } catch (err) {
      toastError(`Restart failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleShutdownPod = async (podId: string) => {
    if (!window.confirm("Shutdown this pod?")) return;
    try {
      await api.shutdownPod(podId);
    } catch (err) {
      toastError(`Shutdown failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleAcknowledgeAssistance = (podId: string) => {
    sendCommand("acknowledge_assistance", { pod_id: podId });
    dismissAssistance(podId);
  };

  const handleOpenRefund = (session: RecentSession) => {
    setRefundTarget(session);
    setSelectedPodId(session.pod_id);
    setPanelMode("refund");
  };

  const handleSignOut = () => {
    setStaffName(null);
    setStaffId(null);
    sessionStorage.removeItem("kiosk_staff_name");
    sessionStorage.removeItem("kiosk_staff_id");
    sessionStorage.removeItem("kiosk_staff_token");
  };

  // ─── Panel Title Derivation ───────────────────────────────────────────
  const getPanelTitle = (): string => {
    if (!selectedPod) return "";
    switch (panelMode) {
      case "setup":
        return `Setup — Pod ${selectedPod.number}`;
      case "live_session":
        return `Live Session — Pod ${selectedPod.number}`;
      case "waiting":
        return `Waiting — Pod ${selectedPod.number}`;
      case "refund":
        return `Refund Session`;
      case "game_picker":
        return `Select Game — Pod ${selectedPod.number}`;
      default:
        return "";
    }
  };

  // ─── Auth Gate ──────────────────────────────────────────────────────
  // Wait for hydration before deciding — SSR can't read sessionStorage
  if (!hydrated) {
    return <div className="h-screen bg-rp-black" />;
  }
  if (!staffName) {
    return (
      <StaffLoginScreen
        onAuthenticated={(id, name) => {
          setStaffId(id);
          setStaffName(name);
          sessionStorage.setItem("kiosk_staff_id", id);
          sessionStorage.setItem("kiosk_staff_name", name);
        }}
      />
    );
  }

  return (
    <div className="h-screen flex flex-col">
      <KioskHeader connected={connected} pods={pods} staffName={staffName} onSignOut={handleSignOut} />

      {/* Assistance Alert Banner */}
      <AssistanceAlert
        requests={assistanceRequests}
        onAcknowledge={handleAcknowledgeAssistance}
      />

      {/* PWA Game Launch Request Banner */}
      <GameLaunchRequestBanner
        requests={gameLaunchRequests}
        onConfirm={async (req) => {
          try {
            await api.launchGame(req.pod_id, req.sim_type, undefined);
          } catch (err) {
            toastError(
              `Launch failed. Check pod connection and try again. (${err instanceof Error ? err.message : "Network error"})`
            );
          }
          dismissGameRequest(req.request_id);
        }}
        onDismiss={(requestId) => dismissGameRequest(requestId)}
      />

      {/* Main Content: Grid + Side Panel */}
      <main className="flex-1 flex overflow-hidden relative">
        {/* Pod Grid */}
        <div className={`p-4 transition-all duration-300 ${isPanelOpen ? "w-[40%]" : "w-full"}`}>
          {displayPods.length === 0 ? (
            <div className="h-full flex flex-col items-center justify-center gap-4">
              <div className="w-12 h-12 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
              <p className="text-rp-grey text-sm">
                {connected ? "Waiting for pods to connect..." : "Connecting to RaceControl..."}
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-4 grid-rows-2 gap-3 h-full">
              {displayPods.map((pod) => {
                const billing = billingTimers.get(pod.id);
                const driverId = billing?.driver_id;
                return (
                  <KioskPodCard
                    key={pod.id}
                    pod={pod}
                    telemetry={latestTelemetry.get(pod.id)}
                    billing={billing}
                    warning={billingWarnings.find((w) => w.podId === pod.id)}
                    gameInfo={gameStates.get(pod.id)}
                    authToken={pendingAuthTokens.get(pod.id)}
                    compact={isPanelOpen}
                    isSelected={selectedPodId === pod.id}
                    onSelect={handlePodSelect}
                    onStartSession={handleStartSession}
                    onEndSession={handleEndSession}
                    onPauseSession={handlePauseSession}
                    onResumeSession={handleResumeSession}
                    onExtendSession={handleExtendSession}
                    onCancelAssignment={handleCancelAssignment}
                    onLaunchGame={(podId) => {
                      setSelectedPodId(podId);
                      // Show game picker panel — non-AC launches directly, AC opens wizard
                      setPanelMode("game_picker");
                    }}
                    onRelaunchGame={async (podId) => {
                      try {
                        await api.relaunchGame(podId);
                      } catch (err) {
                        toastError(`Relaunch failed: ${err instanceof Error ? err.message : "Network error"}`);
                      }
                    }}
                    onStartNow={handleStartNow}
                    onWakePod={handleWakePod}
                    onRestartPod={handleRestartPod}
                    onShutdownPod={handleShutdownPod}
                    acSessionId={
                      multiplayerGroup?.pod_ids.includes(pod.id)
                        ? multiplayerGroup.ac_session_id
                        : undefined
                    }
                    onRetryJoin={
                      multiplayerGroup
                        ? async (podId) => {
                            try {
                              await api.retryPodJoin(multiplayerGroup.ac_session_id, podId);
                            } catch (err) {
                              toastError(`Retry join failed: ${err instanceof Error ? err.message : "Network error"}`);
                            }
                          }
                        : undefined
                    }
                  />
                );
              })}
            </div>
          )}
        </div>

        {/* Side Panel */}
        <SidePanel
          isOpen={isPanelOpen}
          title={getPanelTitle()}
          onClose={closePanel}
        >
          {/* Setup Wizard */}
          {panelMode === "setup" && selectedPod && (
            <SetupWizard
              podId={selectedPodId!}
              podNumber={selectedPod.number}
              wizardState={wizard.state}
              setField={wizard.setField}
              goToStep={wizard.goToStep}
              goBack={wizard.goBack}
              goNext={wizard.goNext}
              isFirstStep={wizard.isFirstStep}
              onLaunch={handleGameLaunch}
              isLaunching={isLaunching}
              buildLaunchArgs={wizard.buildLaunchArgs}
              onCancel={closePanel}
            />
          )}

          {/* Game Picker — direct launch for non-AC games, wizard for AC */}
          {panelMode === "game_picker" && selectedPod && (
            <GamePickerPanel
              podId={selectedPodId!}
              podNumber={selectedPod.number}
              installedGames={selectedPod.installed_games ?? ["assetto_corsa"]}
              onLaunch={async (podId, simType) => {
                if (simType === "assetto_corsa") {
                  // AC uses the full setup wizard
                  setPanelMode("setup");
                  wizard.reset();
                  wizard.goToStep("select_game");
                } else {
                  // Non-AC: direct launch with no launch args
                  try {
                    await api.launchGame(podId, simType, undefined);
                  } catch (err) {
                    toastError(
                      `Launch failed. Check pod connection and try again. (${err instanceof Error ? err.message : "Network error"})`
                    );
                    return;
                  }
                  setPanelMode("live_session");
                }
              }}
              onClose={closePanel}
            />
          )}

          {/* Live Session */}
          {panelMode === "live_session" && selectedPod && billingTimers.get(selectedPodId!) && (
            <LiveSessionPanel
              pod={selectedPod}
              telemetry={latestTelemetry.get(selectedPodId!)}
              billing={billingTimers.get(selectedPodId!)!}
              warning={billingWarnings.find((w) => w.podId === selectedPodId)}
              gameInfo={gameStates.get(selectedPodId!)}
              onEndSession={handleEndSession}
              onPauseSession={handlePauseSession}
              onResumeSession={handleResumeSession}
              onExtendSession={handleExtendSession}
              onLaunchGame={(podId) => {
                setPanelMode("setup");
                wizard.reset();
                wizard.goToStep("select_game");
              }}
              onRelaunchGame={async (podId) => {
                try {
                  await api.relaunchGame(podId);
                } catch (err) {
                  toastError(`Relaunch failed: ${err instanceof Error ? err.message : "Network error"}`);
                }
              }}
            />
          )}

          {/* Waiting state */}
          {panelMode === "waiting" && selectedPod && pendingAuthTokens.get(selectedPodId!) && (
            <WaitingPanel
              authToken={pendingAuthTokens.get(selectedPodId!)!}
              onStartNow={handleStartNow}
              onCancel={handleCancelAssignment}
            />
          )}

          {/* Refund */}
          {panelMode === "refund" && refundTarget && (
            <RefundPanel
              session={refundTarget}
              onClose={() => {
                setRefundTarget(null);
                closePanel();
              }}
              onSuccess={() => {
                setRefundTarget(null);
                closePanel();
                fetchRecentSessions();
              }}
            />
          )}
        </SidePanel>
        {/* Recent Sessions (collapsible) */}
        {!isPanelOpen && (
          <div className="absolute bottom-0 left-0 right-0 bg-rp-black border-t border-rp-border">
            <button
              onClick={() => setRecentSessionsOpen((v) => !v)}
              className="w-full flex items-center justify-between px-4 py-2 text-xs text-rp-grey hover:text-white transition-colors"
            >
              <span className="uppercase tracking-wider font-medium">Recent Sessions ({recentSessions.length})</span>
              <span>{recentSessionsOpen ? "\u25B2" : "\u25BC"}</span>
            </button>
            {recentSessionsOpen && recentSessions.length > 0 && (
              <div className="max-h-48 overflow-y-auto px-4 pb-2 space-y-1">
                {recentSessions.map((s) => (
                  <div key={s.id} className="flex items-center justify-between bg-rp-surface border border-rp-border rounded-lg px-3 py-1.5 text-xs">
                    <div className="flex items-center gap-3 text-white">
                      <span className="font-medium">{s.driver_name}</span>
                      <span className="text-rp-grey">Pod {s.pod_number}</span>
                      <span className="text-rp-grey">{Math.floor(s.driving_seconds / 60)}m</span>
                      {s.cost_paise != null && (
                        <span className="text-rp-grey">{(s.cost_paise / 100).toFixed(0)} cr</span>
                      )}
                      {s.ended_at && (
                        <span className="text-zinc-500">
                          {new Date(s.ended_at).toLocaleTimeString("en-IN", { timeZone: "Asia/Kolkata", hour12: true, hour: "numeric", minute: "2-digit" })}
                        </span>
                      )}
                    </div>
                    <button
                      onClick={() => handleOpenRefund(s)}
                      className="px-2 py-0.5 border border-amber-600/40 text-amber-400 hover:bg-amber-600/10 rounded text-xs transition-colors"
                    >
                      Refund
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </main>
    </div>
  );
}

// ─── Refund Panel (inline) ───────────────────────────────────────────────
function RefundPanel({
  session,
  onClose,
  onSuccess,
}: {
  session: RecentSession;
  onClose: () => void;
  onSuccess: () => void;
}) {
  const [amountCredits, setAmountCredits] = useState(0);
  const [method, setMethod] = useState<"wallet" | "cash" | "upi">("wallet");
  const [reason, setReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const { toastSuccess, toastError } = useToast();

  const canSubmit = amountCredits > 0 && reason.trim().length > 0 && !busy;

  const handleSubmit = async () => {
    if (!canSubmit) return;
    setBusy(true);
    setError("");
    try {
      const result = await api.refundSession(session.id, {
        amount_paise: amountCredits * 100,
        method,
        reason: reason.trim(),
      });
      if (result.error) {
        setError(result.error);
        toastError(`Refund failed: ${result.error}`);
      } else {
        toastSuccess(`Refund of ${amountCredits} credits processed`);
        onSuccess();
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Network error";
      setError(msg);
      toastError(`Refund failed: ${msg}`);
    }
    setBusy(false);
  };

  return (
    <div className="flex flex-col h-full p-5 gap-4">
      <div className="bg-rp-surface border border-rp-border rounded-xl p-4 space-y-2">
        <p className="text-white font-semibold">{session.driver_name}</p>
        <div className="flex gap-3 text-xs text-rp-grey">
          <span>Pod {session.pod_number}</span>
          <span>{session.pricing_tier_name}</span>
          <span>{Math.floor(session.driving_seconds / 60)} min</span>
          {session.cost_paise != null && (
            <span>{(session.cost_paise / 100).toFixed(0)} credits</span>
          )}
        </div>
      </div>

      <div className="space-y-3">
        <div>
          <label className="text-xs text-rp-grey block mb-1">Refund Amount (credits)</label>
          <input
            type="number"
            min={1}
            value={amountCredits || ""}
            onChange={(e) => setAmountCredits(Math.max(0, parseInt(e.target.value) || 0))}
            className="w-full bg-zinc-800 border border-rp-border rounded-lg px-3 py-2 text-white text-sm"
            placeholder="0"
          />
        </div>

        <div>
          <label className="text-xs text-rp-grey block mb-1">Method</label>
          <div className="flex gap-2">
            {(["wallet", "cash", "upi"] as const).map((m) => (
              <button
                key={m}
                onClick={() => setMethod(m)}
                className={`flex-1 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                  method === m
                    ? "bg-rp-red text-white"
                    : "border border-rp-border text-rp-grey hover:text-white"
                }`}
              >
                {m.charAt(0).toUpperCase() + m.slice(1)}
              </button>
            ))}
          </div>
        </div>

        <div>
          <label className="text-xs text-rp-grey block mb-1">Reason (required)</label>
          <textarea
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            rows={2}
            className="w-full bg-zinc-800 border border-rp-border rounded-lg px-3 py-2 text-white text-sm resize-none placeholder:text-zinc-500"
            placeholder="Why is this refund being issued?"
          />
        </div>

        {error && <p className="text-red-400 text-xs">{error}</p>}
      </div>

      <div className="mt-auto space-y-2">
        <button
          onClick={handleSubmit}
          disabled={!canSubmit}
          className="w-full py-3 bg-amber-600 hover:bg-amber-500 text-white font-semibold rounded-xl transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {busy ? "Processing..." : `Refund ${amountCredits > 0 ? amountCredits + " credits" : ""}`}
        </button>
        <button
          onClick={onClose}
          className="w-full py-2.5 border border-rp-border text-rp-grey hover:text-white rounded-lg text-sm transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}

// ─── Waiting Panel (inline) ──────────────────────────────────────────────
function WaitingPanel({
  authToken,
  onStartNow,
  onCancel,
}: {
  authToken: AuthTokenInfo;
  onStartNow: (token: AuthTokenInfo) => void;
  onCancel: (tokenId: string) => void;
}) {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 p-8">
      <p className="text-amber-400 text-sm font-medium uppercase tracking-wider">
        Waiting for Customer
      </p>
      <p className="text-lg text-white font-semibold">{authToken.driver_name}</p>
      {authToken.auth_type === "pin" && (
        <p className="text-5xl font-bold tracking-[0.4em] text-white font-mono">
          {authToken.token}
        </p>
      )}
      {authToken.auth_type === "qr" && (
        <p className="text-rp-grey">Scan QR code at the pod</p>
      )}
      <p className="text-sm text-rp-grey">{authToken.pricing_tier_name}</p>
      <p className="text-xs text-rp-grey">
        Expires {new Date(authToken.expires_at).toLocaleTimeString("en-IN", { timeZone: "Asia/Kolkata", hour12: true })}
      </p>
      <div className="flex gap-3 mt-4">
        <button
          onClick={() => onStartNow(authToken)}
          className="px-6 py-2.5 bg-rp-red hover:bg-rp-red-hover text-white font-semibold rounded-lg transition-colors"
        >
          Start Now
        </button>
        <button
          onClick={() => onCancel(authToken.id)}
          className="px-6 py-2.5 border border-rp-border text-rp-grey hover:text-white hover:border-rp-grey rounded-lg transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
