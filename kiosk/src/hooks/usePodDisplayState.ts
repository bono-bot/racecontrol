"use client";

// V2.0 Pod Display state hook.
//
// Wraps useKioskSocket and derives the V2PodDisplayState union for a single
// pod number. All FSM logic lives in deriveV2PodDisplayState (pure, testable).
// This hook only filters the raw WS data to the relevant pod's slice.

import { useKioskSocket } from "./useKioskSocket";
import {
  deriveV2PodDisplayState,
  type V2PodDisplayState,
} from "@/lib/podDisplayState";
import type { BillingSession, TelemetryFrame } from "@/lib/types";

export interface UsePodDisplayStateResult {
  state: V2PodDisplayState;
  telemetry: TelemetryFrame | undefined;
  billing: BillingSession | undefined;
  connected: boolean;
}

export function usePodDisplayState(podNumber: number): UsePodDisplayStateResult {
  const {
    pods,
    billingTimers,
    billingWarnings,
    gameStates,
    latestTelemetry,
    connected,
  } = useKioskSocket();

  const pod = Array.from(pods.values()).find((p) => p.number === podNumber);
  const podId = pod?.id;
  const billing = podId ? billingTimers.get(podId) : undefined;
  const gameInfo = podId ? gameStates.get(podId) : undefined;
  const telemetry = podId ? latestTelemetry.get(podId) : undefined;
  const warning = podId
    ? billingWarnings.find((w) => w.podId === podId)
    : undefined;

  const state = deriveV2PodDisplayState(pod, billing, gameInfo, warning);

  return { state, telemetry, billing, connected };
}
