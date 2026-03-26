---
phase: 212-detection-expansion
plan: 02
subsystem: infra
tags: [bash, jq, detection, crash-loop, flag-desync, schema-gap, auto-detect, cascade]

requires:
  - phase: 212-01
    provides: cascade.sh with _emit_finding, run_all_detectors, DETECTOR_FINDINGS accumulator
  - phase: 211-safe-scheduling-foundation
    provides: auto-detect.sh 6-step pipeline with BUGS_FOUND/RESULT_DIR/LOG_FILE env contracts

provides:
  - scripts/detectors/detect-crash-loop.sh — DET-04 crash loop detection via JSONL timestamps
  - scripts/detectors/detect-flag-desync.sh — DET-05 feature flag sync check vs server canonical
  - scripts/detectors/detect-schema-gap.sh — DET-06 schema drift detection across venue and cloud DBs
  - Full 6-detector pipeline validated via auto-detect.sh --dry-run exit 0

affects:
  - 213-healing (findings.json with crash_loop/flag_desync/schema_gap feeds auto-fix engine)
  - bono-auto-detect.sh (same cascade.sh sourcing — inherits new detectors automatically)

tech-stack:
  added: []
  patterns:
    - "DET-04: JSONL log via findstr (not rc-agent-startup.log — truncated on restart per Pitfall 1)"
    - "DET-04: UTC date for JSONL filename (logs roll at UTC midnight, not IST midnight — Pitfall 4)"
    - "DET-04: ISO 8601 lexicographic string comparison for timestamp filtering within window"
    - "DET-05: Compares enabled flag NAMES only (not full flag objects) via jq select(.enabled==true)|.name"
    - "DET-05: Pitfall 3 awareness — tracks empty_count, emits fleet-level finding if all 8 pods return empty"
    - "DET-05: comm -23 / comm -13 for precise missing/extra flag set computation"
    - "DET-06: SELECT column FROM table LIMIT 1 via sqlite3 — 'no such column' in stderr = migration gap"
    - "DET-06: Venue DB via safe_remote_exec :8090; cloud DB via SSH (relay custom_command unsupported)"
    - "DET-06: Unknown (unreachable) treated as skip, not false positive"

key-files:
  created:
    - scripts/detectors/detect-crash-loop.sh
    - scripts/detectors/detect-flag-desync.sh
    - scripts/detectors/detect-schema-gap.sh
  modified: []

key-decisions:
  - "DET-04 reads JSONL log not startup.log — startup.log truncates on each restart making historical count impossible (Pitfall 1 from RESEARCH.md)"
  - "DET-05 uses server URL from ${SERVER_URL} env var (inherited from auto-detect.sh) for canonical flag source"
  - "DET-06 cloud DB check uses SSH not relay — relay /relay/exec/run goes to Bono VPS process, not a raw bash exec endpoint"
  - "DET-06 SCHEMA_CHECKS targets known ALTER TABLE additions from db/mod.rs — columns that exist in CREATE TABLE but may be missing from older DBs"

metrics:
  duration: "~2 min"
  tasks_completed: 2
  files_created: 3
  files_modified: 0
  commits: 2

completed: 2026-03-26
---

# Phase 212 Plan 02: Detection Expansion Summary

**3 remaining detection modules (crash loop DET-04, flag desync DET-05, schema gap DET-06) added to scripts/detectors/ completing the full 6-detector cascade pipeline; all scripts pass bash -n syntax and auto-detect.sh --dry-run exits 0**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-03-26T07:26:27Z
- **Completed:** 2026-03-26T07:28:26Z
- **Tasks:** 2 of 2
- **Files created:** 3

## Accomplishments

- Created `scripts/detectors/detect-crash-loop.sh` (DET-04): reads UTC-dated JSONL log via `safe_remote_exec` + `findstr`, counts startup events (`config_loaded`/`listening on`) within 30-minute window using ISO 8601 timestamp string comparison, emits P1 `crash_loop` finding when restart count exceeds threshold of 3
- Created `scripts/detectors/detect-flag-desync.sh` (DET-05): queries server `${SERVER_URL}/api/v1/flags` for canonical enabled-flag-name set, compares per pod using `comm -23`/`comm -13` to compute specific missing/extra flag names, emits P2 `flag_desync` finding per divergent pod; detects all-pods-empty (Pitfall 3) as fleet-level finding
- Created `scripts/detectors/detect-schema-gap.sh` (DET-06): checks 6 known late-added columns (`drivers.updated_at`, `drivers.membership_type`, `billing_sessions.payment_method`, `billing_sessions.staff_discount`, `feature_flags.description`, `game_catalog.category`) via SELECT LIMIT 1 on both venue DB (server :8090 + sqlite3.exe) and cloud DB (SSH to Bono VPS + sqlite3), emits P2 `schema_gap` finding for venue-only, cloud-only, or both-missing cases

## Task Commits

1. **Task 1: DET-04 crash loop and DET-05 flag desync detectors** - `df25fa0f` (feat)
2. **Task 2: DET-06 schema gap detector and full pipeline validation** - `ee8e6ece` (feat)

## Files Created/Modified

- `scripts/detectors/detect-crash-loop.sh` — DET-04: JSONL-based restart counting, UTC date, 3/30min threshold, P1 crash_loop finding
- `scripts/detectors/detect-flag-desync.sh` — DET-05: canonical flag name set comparison, comm diff, Pitfall 3 all-empty fleet detection
- `scripts/detectors/detect-schema-gap.sh` — DET-06: 6 SCHEMA_CHECKS pairs, venue via safe_remote_exec, cloud via SSH, graceful unreachable handling

## Decisions Made

- DET-04 reads JSONL (not startup.log): `rc-agent-startup.log` is truncated on every restart — impossible to count historical restarts. JSONL is append-only and UTC-dated (Pitfall 4).
- DET-05 server URL from `${SERVER_URL}` env var (inherited from `auto-detect.sh`) — consistent with all other server calls
- DET-06 cloud check uses SSH (not relay): relay `/relay/exec/run` dispatches to Bono VPS registered commands, not raw shell exec. SSH to `root@100.70.177.44` is the documented fallback per STATE.md.
- DET-06 "unknown" (unreachable) = skip: an offline DB is not a schema gap — prevents false positives when either side is temporarily down

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- All 6 detectors write to `$RESULT_DIR/findings.json` with category/severity/pod_ip/message/issue_type fields — Phase 213 auto-fix engine can read and act on findings
- DET-04 crash_loop P1 + DET-05 flag_desync P2 + DET-06 schema_gap P2 are discoverable by the cooldown gate in Step 6
- `bono-auto-detect.sh` sources same `cascade.sh` — will auto-inherit all 6 detectors

## Self-Check

- [x] `scripts/detectors/detect-crash-loop.sh` exists — FOUND
- [x] `scripts/detectors/detect-flag-desync.sh` exists — FOUND
- [x] `scripts/detectors/detect-schema-gap.sh` exists — FOUND
- [x] Commit `df25fa0f` — FOUND
- [x] Commit `ee8e6ece` — FOUND
- [x] All 6 detectors + cascade.sh + auto-detect.sh pass `bash -n` — PASS
- [x] `auto-detect.sh --dry-run --no-notify` exits 0 — PASS

## Self-Check: PASSED

---
*Phase: 212-detection-expansion*
*Completed: 2026-03-26*
