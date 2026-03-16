---
gsd_state_version: 1.0
milestone: v5.5
milestone_name: Billing Credits
status: completed
stopped_at: Completed 14-05-PLAN.md — 5 public read endpoints, 2 new tests, 331 tests GREEN
last_updated: "2026-03-16T20:10:46.439Z"
last_activity: 2026-03-17 — Roadmap created (3 phases, 18 requirements mapped)
progress:
  total_phases: 4
  completed_phases: 1
  total_plans: 5
  completed_plans: 5
  percent: 100
---

---
gsd_state_version: 1.0
milestone: v5.0
milestone_name: RC Bot Expansion
status: complete
stopped_at: Completed 14-02-PLAN.md — 9 staff CRUD endpoints for hotlap events and championships, cargo build clean
last_updated: "2026-03-16T19:29:30.779Z"
last_activity: 2026-03-17 — Milestone v5.5 Billing Credits started
progress:
  [██████████] 100%
  completed_phases: 5
  total_plans: 19
  completed_plans: 19
  percent: 100
---

---
gsd_state_version: 1.0
milestone: v5.5
milestone_name: Billing Credits
status: roadmap_ready
stopped_at: "Roadmap created — 3 phases (33, 34, 35), 18 requirements mapped, ready for plan-phase 33"
last_updated: "2026-03-17T00:00:00.000Z"
last_activity: 2026-03-17 — Roadmap created for v5.5 Billing Credits
progress:
  [__________] 0%
  completed_phases: 0
  total_phases: 3
  total_plans: 0
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-17)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.
**Current focus:** v5.5 Billing Credits — Phase 33: DB Schema + Billing Engine.

## Current Position

Phase: 33 — DB Schema + Billing Engine
Plan: Not started
Status: Roadmap ready, awaiting plan-phase 33
Last activity: 2026-03-17 — Roadmap created (3 phases, 18 requirements mapped)

## Accumulated Context

### Decisions

(From prior milestones — carried forward)
- Build order for v5.0 is non-negotiable: rc-common first (Phase 23) — cross-crate compile dependency
- All bot fix functions must gate on billing_active inside the fix itself — pattern memory replay bypasses call-site guards
- billing.rs characterization tests required before any billing bot code (BILL-01 is a prerequisite gate, not a deliverable)
- Wallet sync fence required before recover_stuck_session() ships — CRDT MAX(updated_at) race documented in CONCERNS.md P1
- Multiplayer scope: detection + safe teardown only — auto-rejoin deferred (no AC session token path exists)
- Lap filter: game-reported isValidLap is authoritative; bot analysis sets review_required flag only, never hard-deletes
- PIN counters: strict type separation — customer and staff counters never share state
- Internal storage stays in paise for backward compat — display divides by 100
- compute_session_cost() called every second per active pod — must stay fast (iterate 3 tiers, no DB)
- PWA already shows "credits" — no PWA changes needed for this milestone
- [Phase 14-events-and-championships]: COALESCE UPDATE pattern for update_hotlap_event: bind Option<T> per field, avoids dynamic SQL building which doesn't compile with sqlx query() type
- [Phase 14-events-and-championships]: add_championship_round uses 3 cascading SQL statements for championship_rounds insert + hotlap_events.championship_id update + championships.total_rounds increment

(v5.5 Billing Credits — new decisions)
- rc-common protocol.rs rename ships in Phase 33 alongside the schema, not as a separate pre-phase — both consuming crates are updated atomically in the same PR
- Phase 33 is the non-negotiable foundation: billing_rates table + BillingRateTier type + cache + algorithm must exist before Phase 34 CRUD routes can reference them
- Phase 34 cache invalidation is synchronous on write — PUT/DELETE handlers call cache.invalidate() before returning 200/204, so the next billing tick (1s) uses fresh rates
- Phase 35 is a pure frontend pass — no Rust changes expected; all formatINR callsites in Next.js kiosk/admin replaced in one phase
- [Phase 14-events-and-championships]: lap_id is Option<str> in auto_enter_event — allows None in test calls without seeding laps table (FK constraint workaround)
- [Phase 14-events-and-championships]: auto_enter_event/recalculate_event_positions pub for direct test invocation — avoids full AppState construction in integration tests
- [Phase 14-events-and-championships]: assign_championship_positions is a separate pub fn from compute_championship_standings — allows tiebreaker tests to call it without seeding hotlap_event_entries
- [Phase 14-events-and-championships]: score_group_event computes gap_to_leader_ms inline from multiplayer_results.best_lap_ms — not via recalculate_event_positions which operates on hotlap leaderboard order
- [Phase 14-events-and-championships]: GET /public/championships/{id} is a full-detail endpoint distinct from Plan 04's /standings — both remain registered
- [Phase 14-events-and-championships]: public_event_sessions computes gap_to_leader_ms inline from multiplayer_results.best_lap_ms min — consistent with score_group_event

### Roadmap Evolution

- Phase 22 added: Pod 6/7/8 Recovery and Remote Restart Reliability
- Phases 23-26 added: v5.0 RC Bot Expansion roadmap (2026-03-16)
- Phase 27 added: Tailscale Mesh + Internet Fallback (2026-03-16)
- v5.5 started: Billing Credits (2026-03-17)
- Phases 33-35 added: v5.5 Billing Credits roadmap (2026-03-17)

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Phase 22 plan 22-02 still pending: build release binary + fleet deploy
- TELEM-01 and MULTI-01 live verification pending (needs real pod session)

## Session Continuity

Last session: 2026-03-16T20:00:22.098Z
Stopped at: Completed 14-05-PLAN.md — 5 public read endpoints, 2 new tests, 331 tests GREEN
Resume file: None
Next action: `/gsd:plan-phase 33`
