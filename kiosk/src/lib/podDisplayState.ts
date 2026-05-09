// V2.0 Pod Display State Machine
//
// Doctrine anchor: project_v2_pod_display_state_channel_premise.md (Captain
// fully RATIFIED 2026-05-08 ~21:17 IST). Pod display = state-channel surface,
// NOT live driving HUD. Live HUD lives in-game + future Layer 2 PWA.
//
// State 3 (DRIVING) is V2.1-deferred. The type slot is preserved so the
// FSM transitions are accurate, but the rendering component falls through
// to ActiveSessionInfoView in V2.0. See DrivingViewRouter for the V2.1
// plug-in point. V2.0 HUD scope = Assetto Corsa only.

import type {
  Pod,
  BillingSession,
  GameLaunchInfo,
  BillingWarning,
} from "@/lib/types";

export type V2PodDisplayState =
  | { kind: "IDLE_WELCOMING" }
  | { kind: "GAME_LOADING"; simType: string }
  | { kind: "DRIVING"; simType: string }
  | { kind: "PAUSED_BETWEEN"; autoEndCountdown: number }
  | { kind: "PAUSED_CRASH"; autoRecoveryCountdown: number }
  | { kind: "PAUSED_STAFF" }
  | { kind: "BALANCE_RUNOUT"; graceCountdown: number; balanceCredits?: number }
  | {
      kind: "SESSION_END";
      driverName: string;
      droveMins: number;
      droveSecs: number;
      costCredits: number;
    }
  | { kind: "FAULT"; reason: "offline" | "disabled" | "crash_unrecoverable" };

// Pure derivation from raw WS state. Single place where V2 FSM logic lives.
//
// Order of guards matters:
//   1. FAULT supersedes everything (pod offline/disabled = no session)
//   2. BALANCE_RUNOUT supersedes active billing (warning beats DRIVING display)
//   3. No billing = IDLE_WELCOMING
//   4. SESSION_END for any terminal billing status
//   5. GAME_LOADING for pre-active billing (Pending or initial WaitingForGame)
//   6. PAUSED_BETWEEN for mid-stream WaitingForGame (driving_seconds > 0)
//   7. PAUSED_CRASH for game pause / crash recovery / network disconnect
//   8. PAUSED_STAFF for manual staff pause
//   9. DRIVING for active billing (renders ActiveSessionInfoView in V2.0)
//   10. Default: FAULT (unknown status — fail safe)
export function deriveV2PodDisplayState(
  pod: Pod | undefined,
  billing: BillingSession | undefined,
  gameInfo: GameLaunchInfo | undefined,
  warning: BillingWarning | undefined,
): V2PodDisplayState {
  // Guard 1: FAULT for offline / disabled / missing pod
  if (!pod || pod.status === "offline" || pod.status === "disabled") {
    return {
      kind: "FAULT",
      reason: pod?.status === "disabled" ? "disabled" : "offline",
    };
  }

  // Guard 2: BALANCE_RUNOUT supersedes any active state
  if (warning && billing) {
    return {
      kind: "BALANCE_RUNOUT",
      graceCountdown: warning.remaining,
      balanceCredits: undefined, // wallet balance not yet on billing_tick (Q-DECISION QD-2)
    };
  }

  // Guard 3: No billing = IDLE_WELCOMING
  if (!billing) {
    return { kind: "IDLE_WELCOMING" };
  }

  const status = billing.status;

  // Guard 4: SESSION_END for terminal statuses
  if (
    status === "completed" ||
    status === "ended_early" ||
    status === "cancelled" ||
    status === "cancelled_no_playable"
  ) {
    const drivingSeconds = billing.driving_seconds ?? 0;
    const costPaise = billing.cost_paise ?? 0;
    return {
      kind: "SESSION_END",
      driverName: billing.driver_name ?? "",
      droveMins: Math.floor(drivingSeconds / 60),
      droveSecs: drivingSeconds % 60,
      costCredits: Math.floor(costPaise / 100),
    };
  }

  // Guard 5: GAME_LOADING for pre-active billing
  // (Pending = session created, game not launched; WaitingForGame with no
  // accumulated driving time = initial launch in progress)
  const drivingSeconds = billing.driving_seconds ?? 0;
  if (
    status === "pending" ||
    (status === "waiting_for_game" && drivingSeconds === 0)
  ) {
    return {
      kind: "GAME_LOADING",
      simType: gameInfo?.sim_type ?? "",
    };
  }

  // Guard 6: PAUSED_BETWEEN for mid-stream game change
  // (WaitingForGame with driving_seconds > 0 means a game ran and stopped)
  if (status === "waiting_for_game" && drivingSeconds > 0) {
    return {
      kind: "PAUSED_BETWEEN",
      autoEndCountdown: billing.allocated_seconds
        ? billing.allocated_seconds - drivingSeconds
        : 0,
    };
  }

  // Guard 7: PAUSED_CRASH for game pause / crash recovery / disconnect
  if (
    status === "paused_game_pause" ||
    status === "paused_crash_recovery" ||
    status === "paused_disconnect"
  ) {
    return {
      kind: "PAUSED_CRASH",
      autoRecoveryCountdown: billing.allocated_seconds
        ? billing.allocated_seconds - drivingSeconds
        : 0,
    };
  }

  // Guard 8: PAUSED_STAFF
  if (status === "paused_manual") {
    return { kind: "PAUSED_STAFF" };
  }

  // Guard 9: DRIVING for active billing
  // V2.0: DrivingViewRouter falls through to ActiveSessionInfoView (no live HUD).
  // V2.1: AC-only HUD will plug in here, gated on Phase 5 Task 3 human-verify.
  if (status === "active") {
    return {
      kind: "DRIVING",
      simType: gameInfo?.sim_type ?? "",
    };
  }

  // Default: unknown status, fail to FAULT
  return { kind: "FAULT", reason: "crash_unrecoverable" };
}
