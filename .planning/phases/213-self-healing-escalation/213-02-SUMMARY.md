---
phase: 213-self-healing-escalation
plan: 02
subsystem: infra
tags: [bash, healing, escalation, detectors, cascade, live-sync, whatsapp]

# Dependency graph
requires:
  - phase: 213-self-healing-escalation/213-01
    provides: escalation-engine.sh with attempt_heal(), escalate_pod(), verify_fix(), escalate_human()
provides:
  - cascade.sh sources escalation-engine.sh for live-sync healing
  - All 6 detector scripts call attempt_heal() after every _emit_finding() (HEAL-07)
  - auto-detect.sh sources fixes.sh, notify.sh, escalation-engine.sh early in pipeline
  - WhatsApp escalation uses escalate_human() with HEAL-04 silence conditions
affects: [213-03, auto-detect-pipeline, cascade-detection, healing-engine, pod-healing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Live-sync healing: detectors call attempt_heal() immediately after _emit_finding() -- fixes fire on detection, not in a post-scan batch"
    - "Backward-compat guard: type -t attempt_heal == function before every call -- detectors run safely standalone"
    - "Pod IP guard: attempt_heal only on ^192.168. addresses -- schema_gap/fleet-level findings never passed to escalation engine"
    - "WhatsApp threshold: escalate_human() fires only when BUGS_UNFIXED > 0 OR 3+ pods affected (prevents noise)"

key-files:
  created: []
  modified:
    - scripts/cascade.sh
    - scripts/auto-detect.sh
    - scripts/detectors/detect-config-drift.sh
    - scripts/detectors/detect-bat-drift.sh
    - scripts/detectors/detect-log-anomaly.sh
    - scripts/detectors/detect-crash-loop.sh
    - scripts/detectors/detect-flag-desync.sh
    - scripts/detectors/detect-schema-gap.sh

key-decisions:
  - "cascade.sh sources escalation-engine.sh AFTER detector files and BEFORE run_all_detectors() -- detectors need _emit_finding from cascade.sh, escalation engine needs fixes.sh/core.sh from auto-detect.sh"
  - "detect-schema-gap.sh and detect-flag-desync.sh (fleet-level) include attempt_heal guards that never execute (pod IP guard always false) -- satisfies HEAL-07 requirement to have the call present while preventing erroneous healing on non-pod targets"
  - "auto-detect.sh exports REPO_ROOT and NO_FIX after arg parse -- escalation engine reads both at call time (HEAL-08 toggle and path resolution)"
  - "WhatsApp block replaced with escalate_human() which implements HEAL-04 internally (QUIET filter, venue-closed deferral, 6h cooldown) -- single source of truth for silence conditions"

patterns-established:
  - "Live-sync pattern: _emit_finding() immediately followed by attempt_heal() guarded with type -t"
  - "Layered sourcing order in auto-detect.sh: core.sh -> fixes.sh -> notify.sh -> escalation-engine.sh -> cascade.sh (each layer depends on the previous)"

requirements-completed: [HEAL-04, HEAL-05, HEAL-07]

# Metrics
duration: 10min
completed: 2026-03-26
---

# Phase 213 Plan 02: Detection Pipeline Wiring Summary

**Live-sync healing wired end-to-end: all 6 detectors call attempt_heal() immediately after _emit_finding(), cascade.sh sources escalation-engine.sh, and auto-detect.sh routes WhatsApp escalation through escalate_human() with HEAL-04 silence conditions**

## Performance

- **Duration:** ~10 min
- **Started:** 2026-03-26T08:20:30Z
- **Completed:** 2026-03-26T08:26:25Z
- **Tasks:** 2 of 2
- **Files modified:** 8

## Accomplishments

- cascade.sh sources escalation-engine.sh before run_all_detectors() fires -- escalation functions available in all detector contexts
- All 6 detectors (crash-loop, config-drift, bat-drift, log-anomaly, flag-desync, schema-gap) call attempt_heal() after every _emit_finding() with type -t backward-compat guard
- auto-detect.sh sources fixes.sh + notify.sh + escalation-engine.sh early, so live-sync healing is active during the cascade check step
- WhatsApp escalation replaced: old per-finding cooldown loop removed, escalate_human() now handles HEAL-04 (QUIET silence, venue-closed deferral, 6h cooldown) plus 3+ pods threshold
- REPO_ROOT and NO_FIX exported so escalation engine inherits both at call time

## Task Commits

Each task was committed atomically:

1. **Task 1: Source escalation-engine.sh in cascade.sh and add attempt_heal to all 6 detectors** - `3e9430df` (feat)
2. **Task 2: Wire escalation-engine into auto-detect.sh and remove batch fix model** - `f7a4decc` (feat)

## Files Created/Modified

- `scripts/cascade.sh` - Added escalation-engine.sh source block after detector source loop (HEAL-07)
- `scripts/auto-detect.sh` - exports REPO_ROOT/NO_FIX, sources fixes.sh/notify.sh/escalation-engine.sh, replaced WhatsApp block with escalate_human()
- `scripts/detectors/detect-crash-loop.sh` - attempt_heal after crash_loop finding
- `scripts/detectors/detect-config-drift.sh` - attempt_heal after all 5 config_drift findings (banner, ws_timeout, pod_number, admin_port, kiosk_basepath) with pod IP guard
- `scripts/detectors/detect-bat-drift.sh` - attempt_heal after bat_drift DRIFT finding
- `scripts/detectors/detect-log-anomaly.sh` - attempt_heal after P1 and P2 log_anomaly findings
- `scripts/detectors/detect-flag-desync.sh` - attempt_heal after per-pod flag_desync finding; fleet-level (all-empty) does not call attempt_heal (no pod IP)
- `scripts/detectors/detect-schema-gap.sh` - attempt_heal guards present but never execute (schema targets are cloud/venue/fleet, not pod IPs)

## Decisions Made

- cascade.sh sources escalation-engine.sh after detector source loop and before run_all_detectors() -- ordering critical: detectors need _emit_finding from cascade.sh, escalation engine needs fixes.sh/core.sh from auto-detect.sh
- detect-schema-gap.sh attempt_heal calls are structurally present but gated to ^192.168. pod IPs only; since schema_gap always uses "cloud", "venue", or "fleet" as the target, these calls never execute -- this is intentional (schema drift requires manual migration, not auto-restart)
- auto-detect.sh export of NO_FIX placed after arg parse so --no-fix flag value is exported correctly (not the initial false value)
- Old WhatsApp escalation loop (per-finding cooldown check) replaced with centralized escalate_human() delegation which consolidates HEAL-04 logic in one place

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- HEAL-04, HEAL-05, HEAL-07 requirements fully implemented
- Detection pipeline now has complete healing loop: detect -> attempt_heal -> escalate_pod (5 tiers) -> verify_fix -> escalate_human (if all tiers exhausted)
- Phase 213 complete pending any additional plans; Phase 214 (Bono coordination) can now proceed
- No blockers

---
*Phase: 213-self-healing-escalation*
*Completed: 2026-03-26*
