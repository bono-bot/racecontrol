---
gsd_state_version: 1.0
milestone: v5.5
milestone_name: Billing Credits
status: defining_requirements
stopped_at: "Milestone v5.5 started — defining requirements"
last_updated: "2026-03-17T00:00:00.000Z"
last_activity: 2026-03-17 — Milestone v5.5 Billing Credits started
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-17)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.
**Current focus:** v5.5 Billing Credits — defining requirements.

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-03-17 — Milestone v5.5 Billing Credits started

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

### Roadmap Evolution

- Phase 22 added: Pod 6/7/8 Recovery and Remote Restart Reliability
- Phases 23-26 added: v5.0 RC Bot Expansion roadmap (2026-03-16)
- Phase 27 added: Tailscale Mesh + Internet Fallback (2026-03-16)
- v5.5 started: Billing Credits (2026-03-17) — phases TBD after roadmap creation

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Phase 22 plan 22-02 still pending: build release binary + fleet deploy
- TELEM-01 and MULTI-01 live verification pending (needs real pod session)

## Session Continuity

Last session: 2026-03-17T00:00:00.000Z
Stopped at: Milestone v5.5 started — defining requirements
Resume file: None
