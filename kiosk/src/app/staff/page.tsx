"use client";

import { useState, useEffect, useCallback } from "react";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { useSetupWizard } from "@/hooks/useSetupWizard";
import { KioskHeader } from "@/components/KioskHeader";
import { KioskPodCard } from "@/components/KioskPodCard";
import { SidePanel } from "@/components/SidePanel";
import { SetupWizard } from "@/components/SetupWizard";
import { LiveSessionPanel } from "@/components/LiveSessionPanel";
import { WalletTopupPanel } from "@/components/WalletTopupPanel";
import { StaffLoginScreen } from "@/components/StaffLoginScreen";
import { AssistanceAlert } from "@/components/AssistanceAlert";
import { api } from "@/lib/api";
import type { AuthTokenInfo, PanelMode } from "@/lib/types";

export default function StaffTerminal() {
  const [staffName, setStaffName] = useState<string | null>(null);
  const [staffId, setStaffId] = useState<string | null>(null);
  const [hydrated, setHydrated] = useState(false);

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
    sendCommand,
    pendingSplitContinuation,
    clearPendingSplitContinuation,
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

  // Wallet topup state
  const [topUpDriverId, setTopUpDriverId] = useState<string | null>(null);
  const [topUpDriverName, setTopUpDriverName] = useState("");
  const [topUpBalance, setTopUpBalance] = useState(0);

  // Wallet balances cache: driver_id → balance_paise
  const [walletBalances, setWalletBalances] = useState<Map<string, number>>(new Map());

  // Setup wizard hook
  const wizard = useSetupWizard();

  // Fetch wallet balances for active billing sessions
  const fetchWalletBalances = useCallback(async () => {
    const driverIds = new Set<string>();
    billingTimers.forEach((billing) => {
      if (billing.driver_id) driverIds.add(billing.driver_id);
    });

    for (const driverId of driverIds) {
      try {
        const res = await api.getWallet(driverId);
        if (res.wallet) {
          setWalletBalances((prev) => {
            const next = new Map(prev);
            next.set(driverId, res.wallet!.balance_paise);
            return next;
          });
        }
      } catch {
        // ignore fetch errors
      }
    }
  }, [billingTimers]);

  useEffect(() => {
    fetchWalletBalances();
    const interval = setInterval(fetchWalletBalances, 15000);
    return () => clearInterval(interval);
  }, [fetchWalletBalances]);

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
    setTopUpDriverId(null);
  };

  // ─── Split Continuation ──────────────────────────────────────────────
  // When a sub-session completes and more splits remain, auto-open the setup wizard
  // for the same pod so staff can pick the next track/car.
  const [isSplitContinuation, setIsSplitContinuation] = useState(false);

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
    if (!selectedPodId) return;

    if (isSplitContinuation && pendingSplitContinuation) {
      // Split continuation — no billing start, just continue the split
      try {
        const result = await api.continueSplit({
          pod_id: selectedPodId,
          sim_type: simType,
          launch_args: launchArgs,
        });
        if (result.error) {
          alert(`Continue split failed: ${result.error}`);
          return;
        }
      } catch (err) {
        alert(`Failed to continue split: ${err instanceof Error ? err.message : "Network error"}`);
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
            alert(`Billing failed: ${result.error}`);
            return;
          }
        } catch (err) {
          alert(`Failed to start billing: ${err instanceof Error ? err.message : "Network error"}`);
          return;
        }
      }

      await api.launchGame(selectedPodId, simType, launchArgs);
    }
    // Switch to live session view
    setPanelMode("live_session");
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
      alert(`Wake failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleRestartPod = async (podId: string) => {
    try {
      await api.restartPod(podId);
    } catch (err) {
      alert(`Restart failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleShutdownPod = async (podId: string) => {
    if (!window.confirm("Shutdown this pod?")) return;
    try {
      await api.shutdownPod(podId);
    } catch (err) {
      alert(`Shutdown failed: ${err instanceof Error ? err.message : "Network error"}`);
    }
  };

  const handleAcknowledgeAssistance = (podId: string) => {
    sendCommand("acknowledge_assistance", { pod_id: podId });
    dismissAssistance(podId);
  };

  const handleTopUp = (driverId: string) => {
    let name = "Customer";
    let balance = 0;
    billingTimers.forEach((billing) => {
      if (billing.driver_id === driverId) {
        name = billing.driver_name;
      }
    });
    balance = walletBalances.get(driverId) || 0;
    setTopUpDriverId(driverId);
    setTopUpDriverName(name);
    setTopUpBalance(balance);
    setPanelMode("wallet_topup");
  };

  const handleTopUpSuccess = (newBalance: number) => {
    if (topUpDriverId) {
      setWalletBalances((prev) => {
        const next = new Map(prev);
        next.set(topUpDriverId!, newBalance);
        return next;
      });
    }
    // Go back to live session
    setTopUpDriverId(null);
    setPanelMode("live_session");
  };

  const handleSignOut = () => {
    setStaffName(null);
    setStaffId(null);
    sessionStorage.removeItem("kiosk_staff_name");
    sessionStorage.removeItem("kiosk_staff_id");
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
      case "wallet_topup":
        return `Top Up Wallet`;
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

      {/* Main Content: Grid + Side Panel */}
      <main className="flex-1 flex overflow-hidden">
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
                    walletBalance={driverId ? walletBalances.get(driverId) : undefined}
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
                      setPanelMode("setup");
                      wizard.reset();
                      wizard.goToStep("select_game");
                    }}
                    onStartNow={handleStartNow}
                    onTopUp={handleTopUp}
                    onWakePod={handleWakePod}
                    onRestartPod={handleRestartPod}
                    onShutdownPod={handleShutdownPod}
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
              buildLaunchArgs={wizard.buildLaunchArgs}
              onCancel={closePanel}
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
              walletBalance={
                billingTimers.get(selectedPodId!)?.driver_id
                  ? walletBalances.get(billingTimers.get(selectedPodId!)!.driver_id)
                  : undefined
              }
              onEndSession={handleEndSession}
              onPauseSession={handlePauseSession}
              onResumeSession={handleResumeSession}
              onExtendSession={handleExtendSession}
              onLaunchGame={(podId) => {
                setPanelMode("setup");
                wizard.reset();
                wizard.goToStep("select_game");
              }}
              onTopUp={handleTopUp}
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

          {/* Wallet Top-Up */}
          {panelMode === "wallet_topup" && topUpDriverId && (
            <WalletTopupPanel
              driverId={topUpDriverId}
              driverName={topUpDriverName}
              currentBalance={topUpBalance}
              onClose={() => setPanelMode("live_session")}
              onSuccess={handleTopUpSuccess}
            />
          )}
        </SidePanel>
      </main>
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
        Expires {new Date(authToken.expires_at).toLocaleTimeString()}
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
