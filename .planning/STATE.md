---
gsd_state_version: 1.0
milestone: v47.0
milestone_name: Admin Dashboard Venue-Ready Hardening
status: executing
stopped_at: "Phase 346-01 complete; cutover (346-02) and phases 347-355 pending"
last_updated: "2026-04-09T19:30:00.000Z"
last_activity: 2026-04-09
progress:
  total_phases: 12
  completed_phases: 2
  partial_phases: 1
  total_plans: 36
  completed_plans: 7
  percent: 19
---

# Project State

## Project Reference

See: .planning/PROJECT.md

**Core value (v47.0):** Make the admin dashboard a venue-ready, resilient single source of truth before customer opening. Close 18 audit findings from the 2026-04-09 Vishal-PIN incident and absorb Phase 343 Plan 03 (superseded admin PIN UI).

**Current focus:** Phase 344 — Unbreakable Deploys (first P0 phase)

## Current Position

Phase: 344 (not started)
Plan: Pending
Status: Defining requirements → roadmap → phase planning
Last activity: 2026-04-09 — Milestone v47.0 scaffolded, research complete

Progress: [░░░░░░░░░░] 0% (v47.0 — 0 of 12 phases)

## Accumulated Context

### Milestone origin
v47.0 was triggered by the 2026-04-09 Admin Dashboard audit which found:
- Cloud admin fully down (login 500 from missing RC_URL env, static assets 404)
- Local admin better-sqlite3 ABI mismatch (Node 24 vs binding built for Node 22)
- Cafe menu editor wired to dead `admin.db.menu_items` table (never reaches POS/kiosk)
- No racecontrol.db replication between venue and cloud (210 drivers venue vs 21 cloud)
- 3 missing rc endpoints (`/customer/drivers`, `/customer/membership/active`, `/customer/membership/tiers`)
- Phase 343 Vishal-PIN incident findings (18 code + data gaps)

### Scope decisions (2026-04-09)
- **Sync topology: Option A confirmed** — Litestream venue→cloud read replica
- **Research: full 4-agent parallel** completed (2 via agents, 2 via direct write due to API overload)
- **Scope: 11 themes** (added "Admin Staff Management" from superseded 343-03)
- **Phase numbering: starts at 344** (continues from Phase 343)
- **Hard dependency:** Phase 343 Plans 01+02+04 must ship BEFORE Phase 347 (Admin Staff Management UI)

### Phase wave plan (from SUMMARY-v47.md)

**Wave 1 (no Phase 343 dependency, can start immediately):**
- Phase 344: Unbreakable deploys
- Phase 345: Backend resilience
- Phase 346: Cafe menu proxy rewrite

**Wave 2 (Wave 1 must be green):**
- Phase 348: Auth resilience
- Phase 349: Litestream sync contract
- Phase 352: Health + WhatsApp alerts
- Phase 354: UI hardening

**Wave 3 (Phase 343 must be shipped in racecontrol):**
- Phase 347: Admin Staff Management
- Phase 350: Playwright contract tests

**Wave 4 (final):**
- Phase 351: Data durability
- Phase 353: Runbook + staff training
- Phase 355: Venue-ready readiness review

### Blockers/Concerns

- **Phase 343 Plans 01+02+04 not yet executed** in racecontrol (scaffolded in commit 49314feb by another session, not yet built/tested/deployed). v47.0 Phase 347 is blocked until those ship.
- **Node 24 on venue .23** — must downgrade to 22 LTS as pre-work OR bundle with Phase 344 if safe to do in one deploy window.
- **Research agent API overload** — STACK + FEATURES + SUMMARY were written directly by the session AI instead of subagents. Quality may be lower than full 4-agent research.

## Session Continuity

Last session: 2026-04-09T18:20:00 IST — milestone v47.0 scaffolded
Stopped at: Research complete, drafting REQUIREMENTS.md + ROADMAP.md
Resume file: None — session continuing autonomously per user directive
