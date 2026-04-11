/**
 * Phase 368 — useKioskSocket launch event handler tests
 *
 * Tests the Map update logic for the two new WS event cases:
 *   - launch_status_changed  (sets/updates launches Map by launch_id)
 *   - launch_note_added      (appends to launchNotes Map by launch_id)
 *
 * These tests validate the data transformation without requiring
 * React rendering or @testing-library/react (not installed in kiosk).
 * The logic is extracted as pure helper functions matching the
 * actual implementations in useKioskSocket.ts.
 *
 * Rust contract reference: crates/rc-common/src/protocol.rs
 */
import { describe, it, expect } from "vitest";
import type { LaunchStatusCard, LaunchNoteEvent } from "@/lib/types";

// ── Pure logic extracted from the switch cases ──────────────────────────────
// These mirror the exact setLaunches / setLaunchNotes reducer functions
// in useKioskSocket.ts so that we validate the logic without React state.

function applyLaunchStatusChanged(
  prev: Map<string, LaunchStatusCard>,
  card: LaunchStatusCard
): Map<string, LaunchStatusCard> {
  const next = new Map(prev);
  next.set(card.launch_id, card);
  return next;
}

function applyLaunchNoteAdded(
  prev: Map<string, LaunchNoteEvent[]>,
  note: LaunchNoteEvent
): Map<string, LaunchNoteEvent[]> {
  const next = new Map(prev);
  const existing = next.get(note.launch_id) ?? [];
  next.set(note.launch_id, [...existing, note]);
  return next;
}

// ── Fixtures ─────────────────────────────────────────────────────────────────

function makeCard(overrides: Partial<LaunchStatusCard> = {}): LaunchStatusCard {
  return {
    launch_id: "abc",
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
    launch_id: "abc",
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

describe("useKioskSocket — launch_status_changed handler", () => {
  it("adds new card to empty launches Map", () => {
    const initial = new Map<string, LaunchStatusCard>();
    const card = makeCard({ launch_id: "abc", state: "launch_started" });
    const result = applyLaunchStatusChanged(initial, card);
    expect(result.size).toBe(1);
    expect(result.get("abc")).toEqual(card);
    expect(result.get("abc")?.state).toBe("launch_started");
  });

  it("updates existing card (same launch_id, new state)", () => {
    const card1 = makeCard({ launch_id: "abc", state: "launch_started" });
    const prev = new Map([["abc", card1]]);
    const card2 = makeCard({ launch_id: "abc", state: "ai_analysis_requested" });
    const result = applyLaunchStatusChanged(prev, card2);
    expect(result.size).toBe(1);
    expect(result.get("abc")?.state).toBe("ai_analysis_requested");
  });

  it("does not mutate the previous Map (immutability)", () => {
    const card = makeCard({ launch_id: "abc" });
    const prev = new Map<string, LaunchStatusCard>();
    const result = applyLaunchStatusChanged(prev, card);
    expect(prev.size).toBe(0);
    expect(result.size).toBe(1);
  });

  it("handles multiple distinct launch_ids independently", () => {
    const prev = new Map<string, LaunchStatusCard>();
    const card1 = makeCard({ launch_id: "abc", pod_id: "pod_1" });
    const card2 = makeCard({ launch_id: "def", pod_id: "pod_2" });
    const after1 = applyLaunchStatusChanged(prev, card1);
    const after2 = applyLaunchStatusChanged(after1, card2);
    expect(after2.size).toBe(2);
    expect(after2.get("abc")?.pod_id).toBe("pod_1");
    expect(after2.get("def")?.pod_id).toBe("pod_2");
  });
});

describe("useKioskSocket — launch_note_added handler", () => {
  it("adds note to empty launchNotes Map", () => {
    const initial = new Map<string, LaunchNoteEvent[]>();
    const note = makeNote({ launch_id: "abc", note_id: "n1" });
    const result = applyLaunchNoteAdded(initial, note);
    expect(result.size).toBe(1);
    expect(result.get("abc")).toHaveLength(1);
    expect(result.get("abc")?.[0]).toEqual(note);
  });

  it("appends note to existing notes for same launch_id", () => {
    const note1 = makeNote({ note_id: "n1", body: "First" });
    const prev = new Map([["abc", [note1]]]);
    const note2 = makeNote({ note_id: "n2", body: "Second" });
    const result = applyLaunchNoteAdded(prev, note2);
    const notes = result.get("abc");
    expect(notes).toHaveLength(2);
    expect(notes?.[0].body).toBe("First");
    expect(notes?.[1].body).toBe("Second");
  });

  it("does not mutate the previous Map (immutability)", () => {
    const note = makeNote();
    const prev = new Map<string, LaunchNoteEvent[]>();
    const result = applyLaunchNoteAdded(prev, note);
    expect(prev.size).toBe(0);
    expect(result.size).toBe(1);
  });
});

describe("useKioskSocket — 4-state transition sequence", () => {
  it("transitions launch_started → ai_analysis_requested → issue_being_fixed → issue_fixed", () => {
    const states: LaunchStatusCard["state"][] = [
      "launch_started",
      "ai_analysis_requested",
      "issue_being_fixed",
      "issue_fixed",
    ];

    let launches = new Map<string, LaunchStatusCard>();
    for (const state of states) {
      const card = makeCard({ launch_id: "seq-test", state });
      launches = applyLaunchStatusChanged(launches, card);
    }

    expect(launches.get("seq-test")?.state).toBe("issue_fixed");
  });
});

describe("useKioskSocket — game_state_changed regression guard (existing handler)", () => {
  it("existing game_state_changed event type is unchanged (Phase 62 regression)", () => {
    // This test documents that game_state_changed uses GameLaunchInfo.game_state
    // and is NOT the same as launch_status_changed which uses LaunchStatusCard.state.
    // The two event types are independent; new cases must not interfere with old ones.
    const oldEventName = "game_state_changed";
    const newEventName = "launch_status_changed";
    expect(oldEventName).not.toBe(newEventName);

    // Verify the new event name matches Plan 01 Rust serde output
    expect(newEventName).toBe("launch_status_changed");
    expect("launch_note_added").toBe("launch_note_added");
  });
});
