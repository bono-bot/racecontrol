"use client";

import { useState, useEffect } from "react";
import { useParams } from "next/navigation";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { PodKioskView } from "@/components/PodKioskView";
import { api } from "@/lib/api";
import type { KioskExperience } from "@/lib/types";

export default function PodKioskPage() {
  const params = useParams();
  const podNumber = Number(params.number);
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);

  const {
    connected,
    pods,
    latestTelemetry,
    billingTimers,
    billingWarnings,
    gameStates,
    pendingAuthTokens,
  } = useKioskSocket();

  useEffect(() => {
    api.listExperiences().then((res) => setExperiences(res.experiences || []));
  }, []);

  // Find pod by number
  const pod = Array.from(pods.values()).find((p) => p.number === podNumber);

  if (!connected) {
    return (
      <div className="h-screen w-screen bg-rp-black flex items-center justify-center">
        <div className="text-center">
          <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin mx-auto mb-4" />
          <p className="text-rp-grey text-lg">Connecting to RaceControl...</p>
        </div>
      </div>
    );
  }

  if (!pod) {
    return (
      <div className="h-screen w-screen bg-rp-black flex items-center justify-center">
        <div className="text-center">
          <p className="text-2xl font-bold text-white mb-2">Pod {podNumber}</p>
          <p className="text-rp-grey">Pod not found or offline</p>
        </div>
      </div>
    );
  }

  const billing = billingTimers.get(pod.id);
  const telemetry = latestTelemetry.get(pod.id);
  const gameInfo = gameStates.get(pod.id);
  const authToken = pendingAuthTokens.get(pod.id);
  const warning = billingWarnings.find((w) => w.podId === pod.id);

  const handleSelectExperience = async (experienceId: string) => {
    try {
      await api.podLaunchExperience(pod.id, experienceId);
    } catch (err) {
      console.error("Failed to launch experience:", err);
    }
  };

  const handleRelaunchGame = async () => {
    try {
      await api.relaunchGame(pod.id);
    } catch (err) {
      console.error("Failed to relaunch game:", err);
    }
  };

  return (
    <PodKioskView
      pod={pod}
      billing={billing}
      telemetry={telemetry}
      gameInfo={gameInfo}
      authToken={authToken}
      experiences={experiences}
      mode="standalone"
      onSelectExperience={handleSelectExperience}
      onRelaunchGame={handleRelaunchGame}
      warning={warning}
    />
  );
}
