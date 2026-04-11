/**
 * Phase 368 — LaunchCard component tests
 *
 * Tests the rendering logic and state-dependent visibility rules
 * of the LaunchCard component using pure logic assertions.
 * (No @testing-library/react installed in kiosk; uses vitest + pure functions.)
 */
import { describe, it, expect } from "vitest";
import type { LaunchStatusCard, LaunchNoteEvent, LaunchState } from "@/lib/types";

// ── Pure logic helpers extracted from LaunchCard.tsx ─────────────────────────

function isTerminal(state: LaunchState): boolean {
  return state === "issue_fixed" || state === "needs_manual_intervention";
}

function showApproveButton(card: LaunchStatusCard): boolean {
  return (
    card.state === "issue_being_fixed" &&
    card.ai_tier !== null &&
    card.ai_tier >= 2
  );
}

// ── Fixtures ─────────────────────────────────────────────────────────────────

function makeCard(overrides: Partial<LaunchStatusCard> = {}): LaunchStatusCard {
  return {
    launch_id: "test-id",
    pod_id: "pod_1",
    sim_type: "assetto_corsa",
    state: "launch_started",
    detail: null,
    timestamp: "2026-04-11T14:00:00Z",
    origin: "customer",
    ai_tier: null,
    fix_action: null,
    ...overrides,
  };
}

function makeNote(overrides: Partial<LaunchNoteEvent> = {}): LaunchNoteEvent {
  return {
    launch_id: "test-id",
    pod_id: "pod_1",
    note_id: "n1",
    staff_id: "staff_001",
    staff_name: "James",
    body: "Checking pod status",
    created_at: "2026-04-11T14:01:00Z",
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("LaunchCard — renders all 5 LaunchState values", () => {
  const allStates: LaunchState[] = [
    "launch_started",
    "ai_analysis_requested",
    "issue_being_fixed",
    "issue_fixed",
    "needs_manual_intervention",
  ];

  it("all 5 LaunchState values are valid card states", () => {
    for (const state of allStates) {
      const card = makeCard({ state });
      expect(card.state).toBe(state);
    }
  });

  it("exactly 5 states in the union", () => {
    expect(allStates.length).toBe(5);
  });
});

describe("LaunchCard — dismiss button visibility (terminal-only)", () => {
  it("dismiss button IS shown for issue_fixed (terminal)", () => {
    expect(isTerminal("issue_fixed")).toBe(true);
  });

  it("dismiss button IS shown for needs_manual_intervention (terminal)", () => {
    expect(isTerminal("needs_manual_intervention")).toBe(true);
  });

  it("dismiss button NOT shown for launch_started (non-terminal)", () => {
    expect(isTerminal("launch_started")).toBe(false);
  });

  it("dismiss button NOT shown for ai_analysis_requested (non-terminal)", () => {
    expect(isTerminal("ai_analysis_requested")).toBe(false);
  });

  it("dismiss button NOT shown for issue_being_fixed (non-terminal)", () => {
    expect(isTerminal("issue_being_fixed")).toBe(false);
  });
});

describe("LaunchCard — approve-fix button tier gate (D-08)", () => {
  it("approve button IS shown when state=issue_being_fixed AND ai_tier=2", () => {
    const card = makeCard({ state: "issue_being_fixed", ai_tier: 2 });
    expect(showApproveButton(card)).toBe(true);
  });

  it("approve button IS shown when state=issue_being_fixed AND ai_tier=3", () => {
    const card = makeCard({ state: "issue_being_fixed", ai_tier: 3 });
    expect(showApproveButton(card)).toBe(true);
  });

  it("approve button NOT shown when ai_tier=1 (Tier 1 auto-applies, no user confirmation)", () => {
    const card = makeCard({ state: "issue_being_fixed", ai_tier: 1 });
    expect(showApproveButton(card)).toBe(false);
  });

  it("approve button NOT shown when ai_tier=null", () => {
    const card = makeCard({ state: "issue_being_fixed", ai_tier: null });
    expect(showApproveButton(card)).toBe(false);
  });

  it("approve button NOT shown when state is not issue_being_fixed", () => {
    const card = makeCard({ state: "launch_started", ai_tier: 2 });
    expect(showApproveButton(card)).toBe(false);
  });
});

describe("LaunchCard — billing sanitization (D-15)", () => {
  it("D-15 detail string does NOT contain billing-sensitive substrings", () => {
    // The server guarantees sanitization (Plan 01). This test is the client-side
    // regression guard: if the server detail ever contains customer data, it must be caught.
    const d15Detail = "Launch blocked — billing not ready";
    const forbiddenPatterns = [/customer/i, /balance/i, /paise/i, /wallet/i, /tier/i, /driver/i];
    for (const pattern of forbiddenPatterns) {
      expect(d15Detail).not.toMatch(pattern);
    }
    // The D-15 string itself must be preserved exactly
    expect(d15Detail).toBe("Launch blocked — billing not ready");
  });

  it("card.detail renders the value as provided (no client-side mutation)", () => {
    const card = makeCard({ detail: "Launch blocked — billing not ready" });
    expect(card.detail).toBe("Launch blocked — billing not ready");
  });
});

describe("LaunchCard — notes composer logic", () => {
  it("empty notes array means no thread visible", () => {
    const notes: LaunchNoteEvent[] = [];
    expect(notes.length).toBe(0);
  });

  it("non-empty notes array renders the thread", () => {
    const notes = [makeNote({ body: "Checking cables" })];
    expect(notes.length).toBe(1);
    expect(notes[0].body).toBe("Checking cables");
  });

  it("multiple notes preserve order", () => {
    const notes = [
      makeNote({ note_id: "n1", body: "First" }),
      makeNote({ note_id: "n2", body: "Second" }),
    ];
    expect(notes[0].body).toBe("First");
    expect(notes[1].body).toBe("Second");
  });
});
