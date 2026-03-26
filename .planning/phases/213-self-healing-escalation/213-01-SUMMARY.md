---
phase: 213-self-healing-escalation
plan: 01
subsystem: infra
tags: [bash, healing, escalation, wol, whatsapp, sentinel, billing-gate, audit-trail]

# Dependency graph
requires:
  - phase: 212-detection-expansion
    provides: cascade.sh detectors and _emit_finding pipeline that will call attempt_heal
  - phase: 211-safe-scheduling-foundation
    provides: _is_cooldown_active, _record_alert, PID file run guard, venue_state_detect
  - phase: 23-automated-fleet-audit
    provides: audit/lib/fixes.sh (is_pod_idle, check_pod_sentinels, emit_fix, APPROVED_FIXES), audit/lib/core.sh, audit/lib/notify.sh
provides:
  - "5-tier escalation engine: retry, restart, WoL, cloud failover, WhatsApp human escalation"
  - "3 new APPROVED_FIXES: wol_pod, clear_old_maintenance_mode, replace_stale_bat"
  - "Runtime toggle: auto-detect-config.json with auto_fix_enabled + wol_enabled"
  - "verify_fix with 6 per-issue-type verification functions"
  - "attempt_heal entry point for Phase 213 Plan 02 detector wiring"
affects:
  - 213-02-detection-wiring
  - 214-bono-coordination

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Cause Elimination methodology: Hypothesis comment + test hypothesis + fix + emit_fix audit trail"
    - "Sentinel gate before every tier: check_pod_sentinels blocks OTA_DEPLOYING and MAINTENANCE_MODE"
    - "Runtime toggle pattern: jq reads JSON config at call time, fail-safe enabled when missing"
    - "WoL guard: WOL_ENABLED=false default until manual test on 2 pods (gates are first-class citizens)"
    - "Tier escalation with verify-on-resolve: fix applied AND verified before declaring resolved"

key-files:
  created:
    - scripts/healing/escalation-engine.sh
    - audit/results/auto-detect-config.json
  modified:
    - audit/lib/fixes.sh

key-decisions:
  - "WOL_ENABLED defaults to false -- manual test on 2 pods required before enabling (prevents spurious WoL on online pods)"
  - "clear_old_maintenance_mode guarded to venue=closed only -- open-hours MAINTENANCE_MODE may be intentional staff action"
  - "Missing auto-detect-config.json = auto_fix_enabled (fail-safe) -- prevents accidental detect-only mode from a missing config"
  - "verify_fix missing verify fn = PASS -- unknown issue types do not block fix progression"
  - "Self-test uses BASH_SOURCE[0]==${0} guard -- prevents infinite re-source loop when file is both sourced and executed"
  - "attempt_cloud_failover writes JSON to temp file -- bash string escaping standing rule compliance"

patterns-established:
  - "Hypothesis: comment pattern for all new fix functions (HEAL-06 Cause Elimination)"
  - "Guard-first pattern: check WOL_ENABLED / venue_state / staging server BEFORE attempting fix"
  - "emit_fix audit trail on all outcomes including skips -- every decision is logged"

requirements-completed: [HEAL-01, HEAL-02, HEAL-03, HEAL-06, HEAL-08]

# Metrics
duration: 7min
completed: 2026-03-26
---

# Phase 213 Plan 01: Self-Healing Escalation Engine Summary

**5-tier graduated escalation engine (retry → restart → WoL → cloud failover → WhatsApp) with 3 new APPROVED_FIXES, sentinel-aware billing-gated execution, and runtime JSON toggle for auto_fix_enabled/wol_enabled**

## Performance

- **Duration:** 7min
- **Started:** 2026-03-26T08:11:25Z
- **Completed:** 2026-03-26T08:18:30Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Built 5-tier escalation loop in `scripts/healing/escalation-engine.sh` with all 10 exported functions
- Added 3 new APPROVED_FIXES (wol_pod, clear_old_maintenance_mode, replace_stale_bat) with Hypothesis: methodology comments
- Created `audit/results/auto-detect-config.json` runtime toggle (auto_fix_enabled=true, wol_enabled=false)
- All functions pass --self-test: all 10 functions defined, config readable, exit 0

## Task Commits

Each task was committed atomically:

1. **Task 1: Expand APPROVED_FIXES with 3 new fix functions** - `2ad9ed50` (feat)
2. **Task 2: Create escalation-engine.sh and auto-detect-config.json** - `28ff1c60` (feat)

**Plan metadata:** TBD (docs: complete plan)

## Files Created/Modified
- `audit/lib/fixes.sh` - Added wol_pod(), clear_old_maintenance_mode(), replace_stale_bat() + _pod_mac_address() helper; APPROVED_FIXES now 6 entries
- `scripts/healing/escalation-engine.sh` - New file: 5-tier escalation engine, 10+ exported functions, --self-test mode
- `audit/results/auto-detect-config.json` - New file: runtime toggle config

## Decisions Made
- `WOL_ENABLED` defaults to false and `wol_enabled` in config also false — WoL is off until someone manually tests on at least 2 pods. The tier skips (returns UNRESOLVED) without blocking other escalation tiers.
- `clear_old_maintenance_mode` only clears when `venue_state_detect` returns "closed" — during open hours, MAINTENANCE_MODE may be an intentional staff action (pre-maintenance prep).
- Missing `auto-detect-config.json` = auto_fix_enabled (fail-safe) — prevents a missing config file from silently disabling all healing.
- Self-test guard `[[ "${BASH_SOURCE[0]}" == "${0}" ]]` prevents infinite re-source loop when the file is both sourced and executed with `--self-test`.
- `attempt_cloud_failover` writes relay JSON to `mktemp` temp file before `curl` POST — compliance with standing rule "Git Bash JSON: write to file then curl -d @file".

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed infinite re-source loop in self-test mode**
- **Found during:** Task 2 (self-test execution)
- **Issue:** Original `--self-test` block used `source "${BASH_SOURCE[0]}"` to re-load itself and get all functions defined, causing infinite recursion (the re-sourced file re-enters the `--self-test` block).
- **Fix:** Restructured self-test as `_run_self_test()` function called only when `BASH_SOURCE[0] == $0` (executed directly, not sourced). Removed self-re-source pattern entirely.
- **Files modified:** scripts/healing/escalation-engine.sh
- **Verification:** `bash scripts/healing/escalation-engine.sh --self-test` exits 0 cleanly
- **Committed in:** `28ff1c60` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Necessary correctness fix for self-test mode. No scope change.

## Issues Encountered
- Self-test infinite loop diagnosed by observing repeated "[escalation-engine] --self-test mode" output. Root cause: `source $BASH_SOURCE` inside conditional re-triggers the same conditional. Fixed by restructuring into a guarded main-execution block.

## User Setup Required
None - no external service configuration required. WoL remains disabled (wol_enabled=false in config) until manual test.

## Next Phase Readiness
- `attempt_heal(pod_ip, issue_type, severity)` entry point ready for Phase 213 Plan 02 detector wiring
- `escalate_pod()` implements full 5-tier loop; Plan 02 only needs to call `attempt_heal` from `_emit_finding`
- `auto-detect-config.json` can be toggled at runtime without pipeline restart (HEAL-08 fulfilled)
- WoL tier (Tier 3) will remain UNRESOLVED until `wol_enabled: true` set in config after manual test

## Self-Check: PASSED

- FOUND: scripts/healing/escalation-engine.sh
- FOUND: audit/lib/fixes.sh
- FOUND: audit/results/auto-detect-config.json
- FOUND: .planning/phases/213-self-healing-escalation/213-01-SUMMARY.md
- FOUND commit: 2ad9ed50
- FOUND commit: 28ff1c60

---
*Phase: 213-self-healing-escalation*
*Completed: 2026-03-26*
