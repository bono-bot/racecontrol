// Unit tests for deriveV2PodDisplayState — V2.0 pod display FSM.
// Covers all 10 ordered guards + field plumbing.
//
// Test inputs use type assertions (`as Pod`, `as BillingSession`) to construct
// minimal fixtures with only the fields the function reads. Type completeness
// is enforced at the production callsite (usePodDisplayState consuming the
// real WS-typed payloads); tests focus on FSM logic, not type matching.

import { describe, it, expect } from "vitest";
import { deriveV2PodDisplayState } from "../podDisplayState";
import type {
  Pod,
  BillingSession,
  GameLaunchInfo,
  BillingWarning,
} from "@/lib/types";

const pod = (overrides: Partial<Pod> = {}): Pod =>
  ({ id: "pod-1", number: 1, status: "idle", ...overrides } as Pod);

const billing = (overrides: Partial<BillingSession> = {}): BillingSession =>
  ({
    id: "session-1",
    pod_id: "pod-1",
    driver_id: "driver-1",
    driver_name: "Test Driver",
    status: "active",
    driving_seconds: 0,
    allocated_seconds: 1800,
    cost_paise: 0,
    ...overrides,
  } as BillingSession);

const game = (sim: string): GameLaunchInfo =>
  ({ pod_id: "pod-1", sim_type: sim, game_state: "running" } as GameLaunchInfo);

const warn = (remaining: number): BillingWarning => ({
  sessionId: "session-1",
  podId: "pod-1",
  remaining,
  timestamp: Date.now(),
});

describe("deriveV2PodDisplayState", () => {
  describe("Guard 1 — FAULT", () => {
    it("returns FAULT/offline when pod is undefined", () => {
      const state = deriveV2PodDisplayState(
        undefined,
        undefined,
        undefined,
        undefined,
      );
      expect(state).toEqual({ kind: "FAULT", reason: "offline" });
    });

    it("returns FAULT/offline when pod.status === 'offline'", () => {
      const state = deriveV2PodDisplayState(
        pod({ status: "offline" }),
        undefined,
        undefined,
        undefined,
      );
      expect(state).toEqual({ kind: "FAULT", reason: "offline" });
    });

    it("returns FAULT/disabled when pod.status === 'disabled'", () => {
      const state = deriveV2PodDisplayState(
        pod({ status: "disabled" }),
        undefined,
        undefined,
        undefined,
      );
      expect(state).toEqual({ kind: "FAULT", reason: "disabled" });
    });
  });

  describe("Guard 2 — BALANCE_RUNOUT supersedes", () => {
    it("returns BALANCE_RUNOUT when warning fires during active billing", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "active", driving_seconds: 1500 }),
        game("assetto_corsa"),
        warn(87),
      );
      expect(state.kind).toBe("BALANCE_RUNOUT");
      if (state.kind === "BALANCE_RUNOUT") {
        expect(state.graceCountdown).toBe(87);
      }
    });

    it("does NOT return BALANCE_RUNOUT when warning exists but billing is undefined", () => {
      const state = deriveV2PodDisplayState(pod(), undefined, undefined, warn(60));
      expect(state.kind).toBe("IDLE_WELCOMING");
    });
  });

  describe("Guard 3 — IDLE_WELCOMING", () => {
    it("returns IDLE_WELCOMING when no billing exists", () => {
      const state = deriveV2PodDisplayState(pod(), undefined, undefined, undefined);
      expect(state).toEqual({ kind: "IDLE_WELCOMING" });
    });
  });

  describe("Guard 4 — SESSION_END", () => {
    it("returns SESSION_END for status 'completed'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({
          status: "completed",
          driver_name: "Uday",
          driving_seconds: 1634,
          cost_paise: 45000,
        }),
        undefined,
        undefined,
      );
      expect(state).toEqual({
        kind: "SESSION_END",
        driverName: "Uday",
        droveMins: 27,
        droveSecs: 14,
        costCredits: 450,
      });
    });

    it("returns SESSION_END for status 'ended_early'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "ended_early", driving_seconds: 600, cost_paise: 30000 }),
        undefined,
        undefined,
      );
      expect(state.kind).toBe("SESSION_END");
    });

    it("returns SESSION_END for status 'cancelled'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "cancelled" }),
        undefined,
        undefined,
      );
      expect(state.kind).toBe("SESSION_END");
    });

    it("returns SESSION_END for status 'cancelled_no_playable'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "cancelled_no_playable" }),
        undefined,
        undefined,
      );
      expect(state.kind).toBe("SESSION_END");
    });

    it("computes droveMins/droveSecs correctly from driving_seconds", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "completed", driving_seconds: 125, cost_paise: 0 }),
        undefined,
        undefined,
      );
      if (state.kind === "SESSION_END") {
        expect(state.droveMins).toBe(2);
        expect(state.droveSecs).toBe(5);
      }
    });

    it("computes costCredits as floor(cost_paise / 100)", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "completed", driving_seconds: 0, cost_paise: 12399 }),
        undefined,
        undefined,
      );
      if (state.kind === "SESSION_END") {
        expect(state.costCredits).toBe(123);
      }
    });
  });

  describe("Guard 5 — GAME_LOADING", () => {
    it("returns GAME_LOADING for status 'pending'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "pending" }),
        game("assetto_corsa"),
        undefined,
      );
      expect(state).toEqual({ kind: "GAME_LOADING", simType: "assetto_corsa" });
    });

    it("returns GAME_LOADING for waiting_for_game when driving_seconds === 0", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "waiting_for_game", driving_seconds: 0 }),
        game("f1_25"),
        undefined,
      );
      expect(state).toEqual({ kind: "GAME_LOADING", simType: "f1_25" });
    });

    it("returns GAME_LOADING with empty simType when gameInfo is undefined", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "pending" }),
        undefined,
        undefined,
      );
      expect(state).toEqual({ kind: "GAME_LOADING", simType: "" });
    });
  });

  describe("Guard 6 — PAUSED_BETWEEN", () => {
    it("returns PAUSED_BETWEEN for waiting_for_game with driving_seconds > 0", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({
          status: "waiting_for_game",
          driving_seconds: 600,
          allocated_seconds: 1800,
        }),
        undefined,
        undefined,
      );
      expect(state.kind).toBe("PAUSED_BETWEEN");
      if (state.kind === "PAUSED_BETWEEN") {
        expect(state.autoEndCountdown).toBe(1200);
      }
    });
  });

  describe("Guard 7 — PAUSED_CRASH", () => {
    it("returns PAUSED_CRASH for status 'paused_game_pause'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "paused_game_pause", driving_seconds: 300 }),
        undefined,
        undefined,
      );
      expect(state.kind).toBe("PAUSED_CRASH");
    });

    it("returns PAUSED_CRASH for status 'paused_crash_recovery'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "paused_crash_recovery", driving_seconds: 300 }),
        undefined,
        undefined,
      );
      expect(state.kind).toBe("PAUSED_CRASH");
    });

    it("returns PAUSED_CRASH for status 'paused_disconnect'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "paused_disconnect", driving_seconds: 300 }),
        undefined,
        undefined,
      );
      expect(state.kind).toBe("PAUSED_CRASH");
    });
  });

  describe("Guard 8 — PAUSED_STAFF", () => {
    it("returns PAUSED_STAFF for status 'paused_manual'", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "paused_manual", driving_seconds: 300 }),
        undefined,
        undefined,
      );
      expect(state).toEqual({ kind: "PAUSED_STAFF" });
    });
  });

  describe("Guard 9 — DRIVING", () => {
    it("returns DRIVING for status 'active' with sim type plumbed through", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "active", driving_seconds: 120 }),
        game("assetto_corsa"),
        undefined,
      );
      expect(state).toEqual({ kind: "DRIVING", simType: "assetto_corsa" });
    });

    it("returns DRIVING with empty simType when gameInfo is undefined", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "active", driving_seconds: 120 }),
        undefined,
        undefined,
      );
      expect(state).toEqual({ kind: "DRIVING", simType: "" });
    });
  });

  describe("Guard 10 — fail-to-FAULT default", () => {
    it("returns FAULT/crash_unrecoverable for unknown status", () => {
      const state = deriveV2PodDisplayState(
        pod(),
        billing({ status: "unknown_status" as BillingSession["status"] }),
        undefined,
        undefined,
      );
      expect(state).toEqual({ kind: "FAULT", reason: "crash_unrecoverable" });
    });
  });
});
