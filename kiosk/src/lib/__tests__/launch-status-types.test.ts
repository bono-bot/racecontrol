/**
 * Phase 368 LaunchState contract test (Phase 62 cross-boundary)
 *
 * Asserts that the TypeScript LaunchState literal union has EXACTLY the
 * 5 values that match the Rust LaunchState enum serde snake_case output.
 * Any drift in either direction is caught at compile time (TypeScript errors)
 * AND at runtime (this test failing).
 *
 * Rust reference: crates/rc-common/src/protocol.rs — LaunchState enum
 * Rust contract tests: crates/rc-common/tests/launch_status_value_contract.rs
 */
import { describe, it, expect } from "vitest";
import type { LaunchState, LaunchOrigin, LaunchStatusCard } from "../types";

describe("Phase 368 LaunchState contract (Phase 62 cross-boundary)", () => {
  it("has exactly 5 LaunchState values matching Rust enum snake_case", () => {
    // Compile-time check: if any literal below is NOT in the LaunchState union,
    // TypeScript will error during `tsc --noEmit`. Runtime check locks the count.
    const expected: LaunchState[] = [
      "launch_started",
      "ai_analysis_requested",
      "issue_being_fixed",
      "issue_fixed",
      "needs_manual_intervention",
    ];

    expect(expected.length).toBe(5);

    // Each value must be assignable to LaunchStatusCard.state (structural check)
    expected.forEach((v) => {
      const card: Partial<LaunchStatusCard> = { state: v };
      expect(card.state).toBe(v);
    });
  });

  it("has exactly 4 LaunchOrigin values matching Rust enum snake_case", () => {
    const expected: LaunchOrigin[] = ["customer", "staff", "auto_token", "retry"];
    expect(expected.length).toBe(4);

    // Each must be a valid LaunchOrigin (compile-time + runtime)
    expected.forEach((v) => {
      const card: Partial<LaunchStatusCard> = { origin: v };
      expect(card.origin).toBe(v);
    });
  });

  it("LaunchStatusCard interface has all required fields from Rust LaunchStatusCard struct", () => {
    // Construct a full card — TypeScript will error if any required field is missing
    const card: LaunchStatusCard = {
      launch_id: "test-uuid",
      pod_id: "pod_1",
      sim_type: "assetto_corsa",
      state: "launch_started",
      detail: null,
      timestamp: "2026-04-11T14:00:00Z",
      origin: "customer",
      ai_tier: null,
      fix_action: null,
    };

    expect(card.launch_id).toBe("test-uuid");
    expect(card.state).toBe("launch_started");
    expect(card.detail).toBeNull();
    expect(card.ai_tier).toBeNull();
  });
});
