---
phase: 189
plan: 03
subsystem: audit-framework
tags: [audit, bash, fleet-inventory, phase-runner, phase01]
dependency_graph:
  requires: [189-01, 189-02]
  provides: [audit-end-to-end, phase01-fleet-inventory, phase-runner-loop]
  affects: [audit/audit.sh, audit/phases/tier1/phase01.sh]
tech_stack:
  added: []
  patterns: [bash-phase-runner, emit-result-pattern, quiet-override-pattern, load-phases-sourcing]
key_files:
  created:
    - audit/phases/tier1/phase01.sh
    - audit/phases/tier1/.gitkeep
  modified:
    - audit/audit.sh
decisions:
  - "Phase scripts source lib/core.sh functions via parent scope (sourced at load_phases time, not re-sourced in each phase)"
  - "load_phases() uses [[ -f ]] guard so audit.sh gracefully degrades if a phase script is missing"
  - "QUIET override applied per-check inside run_phase01 loop (not in emit_result) -- gives each check independent venue-state logic"
  - "run_phase01 always returns 0 -- FAILs encoded in JSON, never propagated as bash exit codes"
  - "James local comms-link emits WARN (not FAIL) -- expected offline when not on james machine"
  - "Bono VPS emits WARN (not FAIL) -- degraded for venue ops but not critical P1"
metrics:
  duration: "~15 minutes"
  completed: "2026-03-25"
  tasks_completed: 2
  tasks_total: 2
  files_created: 2
  files_modified: 1
---

# Phase 189 Plan 03: Phase Runner and Fleet Inventory Summary

**One-liner:** Phase 01 Fleet Inventory (server + 8 pods rc-agent/rc-sentry + Bono VPS) wired end-to-end through load_phases runner into audit.sh, producing IST-timestamped 9-field JSON records per check.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create phase01.sh Fleet Inventory check | e8f7a706 | audit/phases/tier1/phase01.sh, audit/phases/tier1/.gitkeep |
| 2 | Update audit.sh phase runner | d5e04511 | audit/audit.sh |

## What Was Built

### audit/phases/tier1/phase01.sh

`run_phase01` function implementing Fleet Inventory:
- Server racecontrol :8080/api/v1/health (PASS if `build_id` in response, FAIL P1 otherwise)
- Server server_ops :8090/health (same pattern)
- Pod loop over all 8 IPs: rc-agent :8090 and rc-sentry :8091 per pod
  - QUIET override: when `venue_state=closed`, FAIL/WARN become QUIET P3
- James local comms-link :8766/relay/health (WARN if offline, not FAIL)
- Full mode only: Ollama :11434 and go2rtc :1984 checks
- Bono VPS :8080/api/v1/health (WARN P2, not FAIL -- not critical for venue ops)
- No `set -e`, always `return 0`
- `export -f run_phase01` for subshell use

### audit/audit.sh (updated)

- Added `load_phases()` function that sources `phases/tier1/phase01.sh` with `[[ -f ]]` guard
- Replaced placeholder dispatch comment with:
  - `VENUE_STATE=$(venue_state_detect ...)` detection before phase run
  - `load_phases "$AUDIT_MODE"` call
  - `AUDIT_PHASE`/`AUDIT_TIER`/default mode dispatch (calls `run_phase01`)
  - Phase runner complete message with result dir path
- Updated init `emit_result` to pass `$AUDIT_MODE` and `$VENUE_STATE` args

## Verification Results

```
bash -n audit/audit.sh         # PASS
bash -n audit/phases/tier1/phase01.sh  # PASS
bash -n audit/lib/core.sh      # PASS

AUDIT_PIN=test bash audit/audit.sh --mode quick
# Created: audit/results/2026-03-25_13-39/
# 21 JSON files produced
# Status summary: 19 PASS, 1 FAIL (server-ops :8090 offline), 1 WARN (bono-vps)
# All timestamps end with +05:30 (IST confirmed)
# All 9 fields present in every record

AUDIT_PIN=test bash audit/audit.sh --mode quick --dry-run
# Exit 0, no phase JSON files created
```

### Sample JSON record (pod-89-rcsentry):
```json
{
  "phase": "01",
  "tier": "1",
  "host": "pod-89-rcsentry",
  "status": "PASS",
  "severity": "P3",
  "message": "rc-sentry healthy",
  "mode": "quick",
  "venue_state": "open",
  "timestamp": "2026-03-25T13:39:19+05:30"
}
```

## Deviations from Plan

None - plan executed exactly as written.

## Decisions Made

1. **Phase scripts source lib/core.sh functions via parent scope** — `load_phases()` sources phase01.sh after core.sh is already sourced in audit.sh, so `emit_result`, `http_get`, etc. are available in the function's scope without re-sourcing.

2. **QUIET override per-check inside run_phase01** — gives each check independent venue-state logic. The alternative (centralizing in emit_result) would remove per-check flexibility for future phases.

3. **James local checks emit WARN not FAIL** — comms-link relay is expected to be offline when audit runs on a pod or the server. FAIL would create noise in non-james contexts.

4. **Bono VPS is WARN (P2) not FAIL (P1)** — Bono VPS downtime is degraded but doesn't affect venue operations (billing, pods, kiosk all run locally).

## Self-Check: PASSED

- FOUND: audit/phases/tier1/phase01.sh
- FOUND: audit/phases/tier1/.gitkeep
- FOUND: audit/audit.sh (modified)
- FOUND: 189-03-SUMMARY.md
- FOUND commit e8f7a706 (Task 1)
- FOUND commit d5e04511 (Task 2)
