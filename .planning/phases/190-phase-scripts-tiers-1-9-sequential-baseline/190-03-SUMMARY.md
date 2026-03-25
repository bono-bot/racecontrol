---
phase: 190-phase-scripts-tiers-1-9-sequential-baseline
plan: 03
subsystem: testing
tags: [audit, bash, shell, fleet-audit, phase-scripts, tier7, tier8, tier9]

requires:
  - phase: 190-01
    provides: Tier 1-6 phase scripts (phases 01-34) and reference patterns
  - phase: 190-02
    provides: audit/lib/core.sh shared primitives (http_get, emit_result, safe_remote_exec, safe_ssh_capture, get_session_token, venue_state_detect)

provides:
  - Tier 7 phase scripts: phase35 (cloud sync), phase36 (DB schema), phase37 (activity log), phase38 (Bono relay)
  - Tier 8 phase scripts: phase39 (feature flags), phase40 (scheduler), phase41 (config push/OTA), phase42 (error aggregator)
  - Tier 9 phase scripts: phase43 (camera pipeline), phase44 (face detection)
  - audit.sh: load_phases() sources all 9 tier directories; full mode/tier/phase dispatch (EXEC-05, EXEC-06)

affects:
  - 190-04 (parallel engine — consumes all 44 phase functions via load_phases)
  - 191 (Tiers 10-18 added to load_phases in Phase 191)
  - Any audit run using standard/full/pre-ship/post-incident modes

tech-stack:
  added: []
  patterns:
    - "source_tier() helper sources all phase*.sh in a tier directory — enables Phase 191 to add tiers 10-18 by just creating the directory"
    - "run_tier_1_to_2() / run_tier_3_to_9() helper functions keep mode dispatch clean and composable"
    - "QUIET override pattern: venue_state=closed + (FAIL|WARN) -> QUIET on all camera/AI checks"
    - "phase_padded: printf '%02d' normalizes --phase 7 to run_phase07"

key-files:
  created:
    - audit/phases/tier7/phase35.sh
    - audit/phases/tier7/phase36.sh
    - audit/phases/tier7/phase37.sh
    - audit/phases/tier7/phase38.sh
    - audit/phases/tier8/phase39.sh
    - audit/phases/tier8/phase40.sh
    - audit/phases/tier8/phase41.sh
    - audit/phases/tier8/phase42.sh
    - audit/phases/tier9/phase43.sh
    - audit/phases/tier9/phase44.sh
  modified:
    - audit/audit.sh

key-decisions:
  - "source_tier() scans tier directory for all phase*.sh at runtime — no hardcoded file list needed for load_phases"
  - "standard/full/pre-ship/post-incident all load tiers 1-9 (tiers 10-18 deferred to Phase 191)"
  - "pre-ship mode runs tiers 1-2 + phases 35 (cloud sync) and 39 (feature flags) as critical gates"
  - "post-incident mode runs tiers 1-2 + all tier 8 (advanced systems/recovery focused)"
  - "phase37.sh PII check bug in plan corrected: missing assignment operator fixed before writing"

patterns-established:
  - "Tier 9 (cameras/AI): all 3 checks apply QUIET when venue_state=closed — cameras expected offline at night"
  - "Phase 36: uses safe_ssh_capture for cloud DB check (banner protection per standing rule)"
  - "Phase 38: uses localhost:8766 for relay test (not SSH) per standing rule (verify against running system)"
  - "Phase 43: go2rtc at localhost:1984 (NOT :8096 — standing rule annotation inline)"

requirements-completed:
  - RUN-04
  - EXEC-05
  - EXEC-06

duration: 20min
completed: 2026-03-25
---

# Phase 190 Plan 03: Tier 7-9 Phase Scripts and Full audit.sh Dispatcher Summary

**10 audit phase scripts (tiers 7-9) plus full load_phases()/dispatch rewrite wiring all 44 phases across 5 modes and 9 tiers**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-25T14:20:00+05:30
- **Completed:** 2026-03-25T14:40:00+05:30
- **Tasks:** 2
- **Files modified:** 11 (10 created + 1 updated)

## Accomplishments

- Created 10 phase scripts for tiers 7 (Data & Sync), 8 (Advanced Systems), and 9 (Cameras & AI)
- Rewrote audit.sh load_phases() with source_tier() helper that auto-discovers all scripts in a tier directory
- Implemented full mode/tier/phase dispatch: 5 modes, 9 tier cases, --phase N with 2-digit padding
- Bash -n syntax passes on all 10 new files and audit.sh; dry-run exits 0

## Task Commits

Each task committed atomically:

1. **Task 1: Create Tier 7/8/9 phase scripts (phases 35-44)** - `9611c79e` (feat)
2. **Task 2: Update audit.sh — load_phases() + full dispatch** - `70f3f083` (feat)

## Files Created/Modified

- `audit/phases/tier7/phase35.sh` - Cloud sync bidirectional check (build ID venue vs cloud)
- `audit/phases/tier7/phase36.sh` - DB schema & migrations (venue + cloud spot check via safe_ssh_capture)
- `audit/phases/tier7/phase37.sh` - Activity log & compliance (audit trail, PII check, retention config)
- `audit/phases/tier7/phase38.sh` - Bono relay & failover (REALTIME mode, bidirectional exec, comms-link git)
- `audit/phases/tier8/phase39.sh` - Feature flags (flags API, DB count, rc-agent fetch log)
- `audit/phases/tier8/phase40.sh` - Scheduler & action queue (logs, status breakdown, stale items)
- `audit/phases/tier8/phase41.sh` - Config push & OTA (push logs, OTA state, pod TOML spot check)
- `audit/phases/tier8/phase42.sh` - Error aggregator & fleet alerts (aggregator, dispatch, error rate)
- `audit/phases/tier9/phase43.sh` - Camera pipeline (go2rtc streams count, NVR reachable, process check)
- `audit/phases/tier9/phase44.sh` - Face detection & people counter (rc-sentry-ai, audit log, :8095)
- `audit/audit.sh` - load_phases() rewrite + full mode/tier/phase dispatch

## Decisions Made

- source_tier() helper auto-discovers scripts by glob — Phase 191 can add tiers 10-18 by just creating directories without touching load_phases()
- pre-ship mode targets phases 35 + 39 as the critical gates (cloud sync integrity + feature flag state)
- post-incident mode targets all of tier 8 — covers advanced systems most likely involved in incidents
- Tiers 3-6 already existed from Plan 190-01; source_tier() sources them alongside new tiers 7-9

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed missing assignment operator in phase37.sh PII check message**
- **Found during:** Task 1 (phase37.sh creation)
- **Issue:** Plan source had `message "..."` (missing `=`) which is a bash syntax error; would fail `bash -n`
- **Fix:** Changed to `message="..."` (correct bash variable assignment)
- **Files modified:** audit/phases/tier7/phase37.sh
- **Verification:** `bash -n audit/phases/tier7/phase37.sh` passes
- **Committed in:** `9611c79e` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Required fix for syntax correctness. No scope creep.

## Issues Encountered

None — all phase scripts passed `bash -n` on first run after the phase37 fix.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- All 44 phase functions now loaded by audit.sh for standard/full modes
- `bash audit/audit.sh --mode standard` will run phases 01-44 sequentially
- Phase 190-04 (parallel engine) can source load_phases() and invoke any run_phaseNN function
- Phase 191 can add Tiers 10-18 by creating directories; load_phases() will auto-discover them

---
*Phase: 190-phase-scripts-tiers-1-9-sequential-baseline*
*Completed: 2026-03-25*
