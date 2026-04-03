---
gsd_state_version: 1.0
milestone: v42.0
milestone_name: Meshed Intelligence Migration
status: defining
stopped_at: null
last_updated: "2026-04-03T17:30:00.000Z"
last_activity: 2026-04-03
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-03)

**Core value:** Move MI tier engine from rc-agent to rc-sentry — eliminate blind spot where rc-agent death kills the self-healing system.
**Current focus:** Defining requirements for v42.0

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-04-03 — Milestone v42.0 started

Progress: [░░░░░░░░░░] 0% (0/0 plans complete)

## Accumulated Context

- v40.0 Phase 312 WS ACK confirmed deployed (b7359a02) — Phase 318 dependency satisfied
- content_scanner.rs only scans AC content; Steam/non-Steam games invisible to system
- combo_reliability table + GamePresetWithReliability exist from Phase 298 — extend, do not rebuild
- Game Doctor (12-point check) runs reactively at launch only — Phase 316 adds proactive boot path
- Meshed Intelligence tier_engine.rs handles GameLaunchFail — Phase 315 adds GameLaunchTimeout + CrashLoop
- Crash loop detection already in fleet_health.rs — Phase 317 extends it, does NOT re-implement in rc-agent

## Decisions

- Phase 315: DiagnosticTrigger gets #[serde(other)] Unknown before new variants — protects existing KB entries
- Phase 316: libraryfolders.vdf parsing required (not hardcoded C/D/E paths) — correctness constraint
- Phase 316: Boot combo validation gated on preset push received via watch channel (Pitfall 5)
- Phase 317: Crash loop single source of truth stays in fleet_health.rs (Pitfall 7)
- Phase 317: WhatsApp alerts route through EscalationRequest WS path — never direct Evolution API (Pitfall 8)
- Phase 319 + 320: UI phases require gsd-ui-researcher before plan, gsd-ui-auditor before ship
- [Phase 316]: validate_ac_combos_at internal variant enables testing without global path injection
- [Phase 316]: unwrap_or_default on spawn_blocking JoinHandle is intentional — empty vec on panic
- [Phase 317]: incident_id=crash_loop_{pod_id} gives 30-min built-in dedup in WhatsAppEscalation
- [Phase 317]: ChainFailureState alerted flag prevents repeated escalation within same 10-min window
- [Phase 318-launch-intelligence]: LaunchTimedOut → GameLaunchTimeout path: server detects timeout → sends WS → agent feeds tier engine for Game Doctor recovery
- [Phase 318]: launch_id generated in rc-agent at LaunchGame receipt — keeps agent self-contained
- [Phase 318]: launch tracking fields in ConnectionState not AppState — resets per WS connection matching launch lifecycle
- [Phase 319]: Route /launch-timeline/recent registered before /:launch_id to prevent Axum treating 'recent' as param
- [Phase 320-kiosk-game-filtering]: pod-inventory in public_routes (no JWT); unknown pod returns 200 empty; sim_type converted from Rust Debug to snake_case at API boundary

## Blockers/Concerns

- v40.0 Phase 313 (Game State Resilience) in progress — Phase 315 can start now (independent), but confirm Phase 313 shipped before planning Phase 318 (shared GameTracker)
- Phase 319 (Reliability Dashboard) requires gsd-ui-researcher subagent before plan-phase is called

## Session Continuity

Last session: 2026-04-03T09:24:22.058Z
Stopped at: Completed 320-01-PLAN.md
Resume file: None
