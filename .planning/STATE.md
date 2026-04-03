---
gsd_state_version: 1.0
milestone: v42.0
milestone_name: Meshed Intelligence Migration
status: ready_to_plan
stopped_at: null
last_updated: "2026-04-03T18:00:00.000Z"
last_activity: 2026-04-03
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-03)

**Core value:** Move MI tier engine from rc-agent to rc-sentry — eliminate blind spot where rc-agent death kills the self-healing system.
**Current focus:** Phase 321 — External Monitoring & Alert Chain (v42.0 start)

## Current Position

Phase: 321 of 324 (Phase 321: External Monitoring & Alert Chain)
Plan: 0 of TBD in current phase
Status: Ready to plan
Last activity: 2026-04-03 — v42.0 roadmap created (Phases 321-324, 14/14 requirements mapped)

Progress: [░░░░░░░░░░] 0% (v42.0)

## Accumulated Context

### From v41.0 (carry forward)

- v40.0 Phase 312 WS ACK confirmed deployed (b7359a02)
- combo_reliability table + GamePresetWithReliability exist from Phase 298 — extend, do not rebuild
- Crash loop detection already in fleet_health.rs — Phase 321 extends it into rc-sentry, does NOT re-implement
- WhatsApp alerts route through EscalationRequest WS path — never direct Evolution API
- Phase 319 (Reliability Dashboard) still pending — UI phases require gsd-ui-researcher before plan-phase

### v42.0 Architecture decisions

- 4 phases: MON first (quick wins / incident fix), MIG split into core engines (322) + heavy engines (323), MESH last (new capability)
- rc-sentry stays std threads (no tokio) — all migrated MI code must use std::sync channels + blocking reqwest
- rc-agent kept fully working throughout via thin proxy (MIG-05) — backward compatibility is non-negotiable
- COMMS_PSK must be deployed to all 8 pods before Phase 321 alert chain can be verified end-to-end

## Decisions

- [Roadmap 2026-04-03]: 4 phases for 14 reqs (standard granularity). MON delivers the immediate Pod 1+7 fix. MIG-01/02/03/05 in Phase 322 (std-thread safe), MIG-04/06 in Phase 323 (MMA needs blocking HTTP). MESH-01/02/03 in Phase 324 (new networking capability).
- [Phase 322]: Migration must be incremental — port one engine at a time, verify compile+tests before removing from rc-agent
- [Phase 323]: MMA engine uses reqwest::blocking in rc-sentry context. Budget cap enforced at sentry level.
- [Phase 324]: Pod-to-pod peer port needs firewall rule on all 8 pods (standing rule: fix ALL systems)

## Blockers/Concerns

- COMMS_PSK not yet deployed to pods — required for Phase 321 (MON-04 alert chain). Deploy this before or as part of Phase 321.
- 21 MI source files in crates/rc-agent/src/ — Phase 322 migration complexity. Incremental approach required.
- Phase 321 depends on nothing — safe to plan immediately.

## Session Continuity

Last session: 2026-04-03 (roadmap creation)
Stopped at: v42.0 roadmap written. REQUIREMENTS.md traceability updated. Ready to plan Phase 321.
Resume file: None
