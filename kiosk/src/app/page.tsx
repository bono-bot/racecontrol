"use client";

import { useState } from "react";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { KioskHeader } from "@/components/KioskHeader";
import { KioskPodCard } from "@/components/KioskPodCard";
import { DriverRegistration } from "@/components/DriverRegistration";
import { ExperienceSelector } from "@/components/ExperienceSelector";
import { api } from "@/lib/api";
import type { KioskExperience } from "@/lib/types";

export default function StaffTerminal() {
  const {
    connected,
    pods,
    latestTelemetry,
    billingTimers,
    billingWarnings,
    gameStates,
    pendingAuthTokens,
    sendCommand,
  } = useKioskSocket();

  // Modal state
  const [registerPodId, setRegisterPodId] = useState<string | null>(null);
  const [experiencePodId, setExperiencePodId] = useState<string | null>(null);

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
    // Launch the game for this pod with the experience config
    await api.launchGame(experiencePodId, experience.game);
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

  return (
    <div className="h-screen flex flex-col">
      <KioskHeader connected={connected} pods={pods} />

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
            {displayPods.map((pod) => (
              <KioskPodCard
                key={pod.id}
                pod={pod}
                telemetry={latestTelemetry.get(pod.id)}
                billing={billingTimers.get(pod.id)}
                warning={billingWarnings.find((w) => w.podId === pod.id)}
                gameInfo={gameStates.get(pod.id)}
                authToken={pendingAuthTokens.get(pod.id)}
                onStartSession={handleStartSession}
                onEndSession={handleEndSession}
                onPauseSession={handlePauseSession}
                onResumeSession={handleResumeSession}
                onExtendSession={handleExtendSession}
                onCancelAssignment={handleCancelAssignment}
              />
            ))}
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
    </div>
  );
}
