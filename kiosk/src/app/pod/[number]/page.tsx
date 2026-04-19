"use client";

import { useState, useEffect } from "react";
import { useParams, useRouter } from "next/navigation";
import { useKioskSocket } from "@/hooks/useKioskSocket";
import { PodKioskView } from "@/components/PodKioskView";
import { api } from "@/lib/api";
import type { KioskExperience } from "@/lib/types";

export default function PodKioskPage() {
  const params = useParams();
  const router = useRouter();
  const podNumber = Number(params.number);
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);
  const [hydrated, setHydrated] = useState(false);
  const [hasToken, setHasToken] = useState(false);
  const [launchError, setLaunchError] = useState<string | null>(null);

  // Hydration-safe auth check (CLAUDE.md: never read sessionStorage in useState init)
  useEffect(() => {
    const token = sessionStorage.getItem("kiosk_staff_token");
    setHasToken(!!token);
    setHydrated(true);
  }, []);

  // Redirect unauthenticated visitors to staff PIN flow.
  // Without this guard, the experience grid renders but every click silently
  // fails because /kiosk/pod-launch-experience is gated by require_staff_jwt.
  useEffect(() => {
    if (hydrated && !hasToken) {
      const ret = encodeURIComponent(`/kiosk/pod/${podNumber}`);
      router.replace(`/staff?return=${ret}`);
    }
  }, [hydrated, hasToken, podNumber, router]);

  // Cross-component signal from api.ts when 401 fires for a request that had
  // no prior token (token expired between page load and click) — surface it
  // instead of swallowing.
  useEffect(() => {
    const onAuthRequired = () => {
      setHasToken(false);
      setLaunchError("Staff session expired. Please sign in again.");
    };
    window.addEventListener("kiosk:auth-required", onAuthRequired);
    return () => window.removeEventListener("kiosk:auth-required", onAuthRequired);
  }, []);

  useEffect(() => {
    api.listExperiences().then((res) => setExperiences(res.experiences || [])).catch(() => {});
  }, []);

  const {
    connected,
    pods,
    latestTelemetry,
    billingTimers,
    billingWarnings,
    gameStates,
    pendingAuthTokens,
    recentLaps,
  } = useKioskSocket();

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
  // Filter recent laps to this pod's current driver (HUD shows sector deltas)
  const podLaps = recentLaps.filter(
    (lap) => billing && lap.driver_id === billing.driver_id
  );

  const handleSelectExperience = async (experienceId: string) => {
    setLaunchError(null);
    try {
      await api.podLaunchExperience(pod.id, experienceId);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Launch failed";
      console.error("Failed to launch experience:", err);
      setLaunchError(msg);
    }
  };

  const handleRelaunchGame = async () => {
    setLaunchError(null);
    try {
      await api.relaunchGame(pod.id);
    } catch (err) {
      const msg = err instanceof Error ? err.message : "Relaunch failed";
      console.error("Failed to relaunch game:", err);
      setLaunchError(msg);
    }
  };

  return (
    <>
      {launchError && (
        <div
          role="alert"
          className="fixed top-0 left-0 right-0 z-50 bg-rp-red/95 text-white px-6 py-3 flex items-center justify-between shadow-lg"
        >
          <div className="flex-1 text-sm font-medium">{launchError}</div>
          <button
            type="button"
            onClick={() => {
              if (launchError?.toLowerCase().includes("session")) {
                const ret = encodeURIComponent(`/kiosk/pod/${podNumber}`);
                router.push(`/staff?return=${ret}`);
              } else {
                setLaunchError(null);
              }
            }}
            className="ml-4 px-3 py-1 rounded bg-white/20 hover:bg-white/30 text-xs uppercase tracking-wide"
          >
            {launchError?.toLowerCase().includes("session") ? "Sign In" : "Dismiss"}
          </button>
        </div>
      )}
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
        recentLaps={podLaps}
      />
    </>
  );
}
