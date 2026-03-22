---
phase: 141-warn-log-scanner
plan: "01"
subsystem: infra
tags: [rust, pod-healer, warn-scanner, logging, cooldown, appstate]

# Dependency graph
requires:
  - phase: 140-ai-action-executor
    provides: pod_healer.rs heal_all_pods() function already calling AI escalation

provides:
  - scan_warn_logs() function in pod_healer.rs reading JSONL log for WARN entries in 5min window
  - warn_scanner_last_escalated cooldown field in AppState
  - heal_all_pods() calls scan_warn_logs() at end of every healer cycle

affects:
  - 141-02-warn-log-scanner (plan 02 will add deduplication + AI escalation to scan_warn_logs)
  - pod_healer.rs (extended with WARN scanner constants and logic)
  - state.rs (new cooldown field)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Rolling-window log scan: read JSONL file, filter by timestamp cutoff, count matching entries"
    - "Cooldown gate: RwLock<Option<DateTime<Utc>>> prevents repeated escalations within 10min window"
    - "Graceful I/O degradation: log read failures return early (debug log only), never interrupt healer cycle"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/pod_healer.rs

key-decisions:
  - "141-01: scan_warn_logs is pub(crate) async fn — callable from plan 02 escalation bridge"
  - "141-01: warn_lines captured as Vec<String> and dropped (let _ = warn_lines) — plan 02 will consume"
  - "141-01: WARN_THRESHOLD comparison is <= so exactly 50 is below threshold, >50 triggers escalation"

patterns-established:
  - "WARN scanner pattern: rolling-window JSON line parse + cooldown RwLock"

requirements-completed:
  - WARN-01
  - WARN-02

# Metrics
duration: 15min
completed: "2026-03-22"
---

# Phase 141 Plan 01: WARN Log Scanner Foundation Summary

**Rolling-window WARN log scanner integrated into heal_all_pods() with 5-min count window, 50-entry threshold, and 10-min cooldown gating via AppState.warn_scanner_last_escalated**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-22T11:20:00+05:30
- **Completed:** 2026-03-22T11:36:00+05:30
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `warn_scanner_last_escalated: RwLock<Option<DateTime<Utc>>>` to AppState struct and initializer
- Implemented `scan_warn_logs()` with JSONL parsing, 5-min rolling window, and threshold + cooldown logic
- Wired `scan_warn_logs(state).await` at end of `heal_all_pods()` per-pod loop
- Zero `.unwrap()` in new code; I/O errors degrade gracefully

## Task Commits

1. **Task 1: Extend AppState with WARN scanner cooldown field** - `9f9cf94` (feat)
2. **Task 2: Implement scan_warn_logs() and wire into healer cycle** - `1859c84` (feat)

**Plan metadata:** (docs commit below)

## Files Created/Modified

- `crates/racecontrol/src/state.rs` - Added `warn_scanner_last_escalated` field + initializer in AppState::new()
- `crates/racecontrol/src/pod_healer.rs` - Added WARN scanner constants, scan_warn_logs() function, call site in heal_all_pods()

## Decisions Made

- `scan_warn_logs` is `pub(crate)` so plan 02 can call it or share its constants without a public API
- `warn_lines` captured and dropped with `let _ = warn_lines` as explicit handoff point for plan 02
- `WARN_THRESHOLD: usize = 50` with `warn_count <= WARN_THRESHOLD` means exactly 50 is safe; >50 escalates

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

The package name in Cargo is `racecontrol-crate` (not `racecontrol`) - used `-p racecontrol-crate` for build verification.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `scan_warn_logs()` is wired and running every healer cycle (every 2 min)
- `warn_scanner_last_escalated` cooldown field ready in AppState
- `warn_lines: Vec<String>` placeholder visible to plan 02 for deduplication + AI escalation
- Plan 02 should add `escalate_warn_surge(state, warn_lines)` call replacing `let _ = warn_lines`

---
*Phase: 141-warn-log-scanner*
*Completed: 2026-03-22*
