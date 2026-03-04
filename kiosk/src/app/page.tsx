"use client";

import { useState, useEffect, useCallback } from "react";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { KioskHeader } from "@/components/KioskHeader";
import { KioskPodCard } from "@/components/KioskPodCard";
import { DriverRegistration } from "@/components/DriverRegistration";
import { ExperienceSelector } from "@/components/ExperienceSelector";
import { AssistanceAlert } from "@/components/AssistanceAlert";
import WalletTopup from "@/components/WalletTopup";
import { api } from "@/lib/api";
import type { KioskExperience, AuthTokenInfo } from "@/lib/types";

export default function StaffTerminal() {
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
  } = useKioskSocket();

  // Modal state
  const [registerPodId, setRegisterPodId] = useState<string | null>(null);
  const [experiencePodId, setExperiencePodId] = useState<string | null>(null);
  const [topUpDriverId, setTopUpDriverId] = useState<string | null>(null);
  const [topUpDriverName, setTopUpDriverName] = useState("");
  const [topUpBalance, setTopUpBalance] = useState(0);

  // Wallet balances cache: driver_id → balance_paise
  const [walletBalances, setWalletBalances] = useState<Map<string, number>>(new Map());

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
    const interval = setInterval(fetchWalletBalances, 15000); // refresh every 15s
    return () => clearInterval(interval);
  }, [fetchWalletBalances]);

  // Sort pods by number for consistent 4x2 grid
  const sortedPods = Array.from(pods.values()).sort((a, b) => a.number - b.number);
  const displayPods = sortedPods.length > 0 ? sortedPods : [];

  const handleStartSession = (podId: string) => {
    setRegisterPodId(podId);
  };

  const handleAssignDriver = async (data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    auth_type: string;
  }) => {
    await api.assignCustomer(data);
    setRegisterPodId(null);
  };

  const handleSelectExperience = async (experience: KioskExperience) => {
    if (!experiencePodId) return;
    const launchArgs = JSON.stringify({
      car: experience.car,
      track: experience.track,
      driver: billingTimers.get(experiencePodId)?.driver_name || "Driver",
      track_config: "",
      skin: "00_default",
    });
    await api.launchGame(experiencePodId, experience.game, launchArgs);
    setExperiencePodId(null);
  };

  const handleEndSession = (billingSessionId: string) => {
    sendCommand("end_billing", { billing_session_id: billingSessionId });
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
  };

  const handleStartNow = async (authToken: AuthTokenInfo) => {
    const result = await api.startNow(authToken.id);
    if (result.error) {
      console.error("Start Now failed:", result.error);
      return;
    }
    setExperiencePodId(authToken.pod_id);
  };

  const handleAcknowledgeAssistance = (podId: string) => {
    sendCommand("acknowledge_assistance", { pod_id: podId });
    dismissAssistance(podId);
  };

  const handleTopUp = (driverId: string) => {
    // Find the driver name and current balance from billing
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
  };

  const handleTopUpSuccess = (newBalance: number) => {
    if (topUpDriverId) {
      setWalletBalances((prev) => {
        const next = new Map(prev);
        next.set(topUpDriverId!, newBalance);
        return next;
      });
    }
    setTopUpDriverId(null);
  };

  return (
    <div className="h-screen flex flex-col">
      <KioskHeader connected={connected} pods={pods} />

      {/* Assistance Alert Banner */}
      <AssistanceAlert
        requests={assistanceRequests}
        onAcknowledge={handleAcknowledgeAssistance}
      />

      {/* 4x2 Pod Grid */}
      <main className="flex-1 p-4">
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
                  onStartSession={handleStartSession}
                  onEndSession={handleEndSession}
                  onPauseSession={handlePauseSession}
                  onResumeSession={handleResumeSession}
                  onExtendSession={handleExtendSession}
                  onCancelAssignment={handleCancelAssignment}
                  onLaunchGame={(podId) => setExperiencePodId(podId)}
                  onStartNow={handleStartNow}
                  onTopUp={handleTopUp}
                />
              );
            })}
          </div>
        )}
      </main>

      {/* Driver Registration Modal */}
      {registerPodId && (
        <DriverRegistration
          podId={registerPodId}
          onAssign={handleAssignDriver}
          onCancel={() => setRegisterPodId(null)}
        />
      )}

      {/* Experience Selector Modal */}
      {experiencePodId && (
        <ExperienceSelector
          podId={experiencePodId}
          onSelect={handleSelectExperience}
          onCancel={() => setExperiencePodId(null)}
        />
      )}

      {/* Wallet Top-Up Modal */}
      {topUpDriverId && (
        <WalletTopup
          driverId={topUpDriverId}
          driverName={topUpDriverName}
          currentBalance={topUpBalance}
          onClose={() => setTopUpDriverId(null)}
          onSuccess={handleTopUpSuccess}
        />
      )}
    </div>
  );
}
