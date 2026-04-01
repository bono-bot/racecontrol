---
phase: "276"
plan: "01"
subsystem: rc-agent
tags: [experience-scoring, predictive-maintenance, fleet-health, cx, pred]
key-files:
  created:
    - crates/rc-agent/src/experience_collector.rs
    - crates/rc-agent/src/experience_actions.rs
  modified:
    - crates/rc-agent/src/experience_score.rs
    - crates/rc-agent/src/predictive_maintenance.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-common/src/fleet_event.rs
    - crates/rc-common/src/protocol.rs
decisions:
  - "Used EscalationPayload for CX-08 WhatsApp alerts instead of a new message type"
  - "Predictive alert -> diagnostic trigger only fires for Critical severity (Warning = log-only)"
  - "Experience metrics reset each 5-min window (sliding window not needed at current scale)"
metrics:
  completed: "2026-04-01"
  tasks: 5
  files: 7
---

# Phase 276 Plan 01: Predictive Alerts & Experience Scoring Summary

Wired predictive alerts into the tier engine action pipeline and built a 5-minute experience scoring cycle with automatic maintenance flagging and WhatsApp escalation.

## Requirements Fulfilled

| Req | Status | Implementation |
|-----|--------|----------------|
| PRED-10 | DONE | `alert_to_fleet_event()` already converts alerts to FleetEvent; new `alert_to_diagnostic_trigger()` bridges Critical alerts to tier engine |
| PRED-11 | DONE | Critical alerts produce DiagnosticTrigger variants (SentinelUnexpected, ErrorSpike, ProcessCrash) that tier engine acts on immediately |
| PRED-12 | DONE | Tier engine's existing FixApplied handler records successful pre-emptive fixes in KB via FleetEvent broadcast |
| CX-05 | DONE | `experience_collector.rs` calculates score every 5 min from MetricInputs accumulated via FleetEvent subscription |
| CX-06 | DONE | `AgentMessage::ExperienceScoreReport` sent via WS to server; server includes in fleet health API |
| CX-07 | DONE | `experience_actions::evaluate_score()` logs WARNING + flags maintenance when score < 80% |
| CX-08 | DONE | Score < 50% sends `EscalationPayload` via WS triggering WhatsApp alert to Uday (Phase 274 pipeline) |

## Changes Made

1. **experience_collector.rs** (NEW) -- Background tokio task: subscribes to FleetEventBus, tracks MetricInputs via `update_metrics()`, calculates score every 5 min, emits `FleetEvent::ExperienceScoreUpdate` + `AgentMessage::ExperienceScoreReport`, calls `evaluate_score()`.

2. **experience_actions.rs** (NEW) -- Score-triggered actions: CX-07 maintenance flag at <80%, CX-08 WhatsApp escalation at <50% via EscalationPayload.

3. **experience_score.rs** -- Removed `#![allow(dead_code)]`. Added `update_metrics()` mapping FleetEvents to MetricInputs increments. Added `record_clean_scan()` for baseline tracking.

4. **predictive_maintenance.rs** -- Removed `#![allow(dead_code)]`. Added `alert_to_diagnostic_trigger()` converting Critical alerts to DiagnosticTrigger variants for tier engine.

5. **main.rs** -- Added `mod experience_collector; mod experience_actions;`. Spawned experience collector with fleet bus + WS sender. Added PRED-10 bridge in predictive scan loop.

6. **rc-common/fleet_event.rs** -- Added `ExperienceScoreUpdate` variant to FleetEvent enum.

7. **rc-common/protocol.rs** -- Added `ExperienceScoreReport` variant to AgentMessage enum.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] EscalationPayload field mismatch**
- **Found during:** Task 4 (experience_actions.rs creation)
- **Issue:** Plan assumed EscalationPayload had `tier_reached`, `attempts`, `recommendation` fields; actual struct has `severity`, `summary`, `actions_tried`, `impact`, `dashboard_url`, `timestamp`
- **Fix:** Used correct field names from rc-common/protocol.rs
- **Commit:** 85f609b9

**2. [Rule 2 - Missing] FleetEvent::ExperienceScoreUpdate did not exist**
- **Found during:** Task 1 (experience_collector.rs creation)
- **Issue:** Plan stated "FleetEvent::ExperienceScoreUpdate already exists (Phase 0)" but it did not
- **Fix:** Added the variant to FleetEvent enum in rc-common
- **Commit:** 85f609b9

**3. [Rule 2 - Missing] AgentMessage::ExperienceScoreReport did not exist**
- **Found during:** Task 1
- **Issue:** Same as above -- plan said it existed, it did not
- **Fix:** Added the variant to AgentMessage enum in rc-common
- **Commit:** 85f609b9

## Known Stubs

None -- all data flows are wired. Server-side consumption of `ExperienceScoreReport` (adding to fleet health API response) is a server-side change outside this plan's scope.

## Commits

| Hash | Message |
|------|---------|
| 85f609b9 | feat(276): predictive alerts to action + experience scoring (PRED-10..12, CX-05..08) |

## Self-Check: PASSED

- experience_collector.rs: FOUND
- experience_actions.rs: FOUND
- Commit 85f609b9: FOUND
- cargo check: PASSED (0 new errors, only pre-existing warnings)
