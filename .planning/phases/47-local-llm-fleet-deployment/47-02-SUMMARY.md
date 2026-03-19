---
phase: 47-local-llm-fleet-deployment
plan: 02
subsystem: testing
tags: [bash, ollama, llm, e2e, fleet, rp-debug, curl]

requires:
  - phase: 41-test-foundation
    provides: lib/common.sh and lib/pod-map.sh shared test helpers
  - phase: 45-close-wait-fix-connection-hygiene
    provides: close-wait.sh as the pattern this script follows
  - phase: 46-crash-safety-panic-hook
    provides: startup-verify.sh as second reference pattern

provides:
  - "tests/e2e/fleet/ollama-health.sh — E2E fleet test verifying Ollama + rp-debug on all 8 pods"
  - "Gate 1: rp-debug model presence via /api/tags on each pod"
  - "Gate 2: rp-debug response latency <5s via /api/generate with wall-clock timing"

affects: [run-all.sh, Phase 47 LLM deployment verification, future AI diagnostic pipeline]

tech-stack:
  added: []
  patterns:
    - "Wall-clock timing in bash using date +%s%3N around curl call for latency gates"
    - "Two-gate health check pattern: presence check then performance check"
    - "Python3 inline JSON parsing from exec response stdout field"

key-files:
  created:
    - tests/e2e/fleet/ollama-health.sh
  modified: []

key-decisions:
  - "Use wall-clock timing (date +%s%3N before/after outer curl) rather than powershell Stopwatch — simpler and matches close-wait.sh style"
  - "Gate 2 still attempted even if Gate 1 fails — provides richer diagnostic info when model is missing"
  - "Outer curl max-time 15s + exec timeout_ms 10000 + inner curl max-time 8s — gives safe margins for cold LLM start"
  - "Python3 inline used for JSON parsing (consistent with startup-verify.sh pattern)"

patterns-established:
  - "ollama-health.sh: two-gate per-pod health check — presence then latency"

requirements-completed: [LLM-01, LLM-02]

duration: 1min
completed: 2026-03-19
---

# Phase 47 Plan 02: Local LLM Fleet Deployment Summary

**Bash E2E test verifying rp-debug model presence and <5s response latency on all 8 pods via Ollama :11434 over :8090/exec**

## Performance

- **Duration:** 1 min
- **Started:** 2026-03-19T01:38:47Z
- **Completed:** 2026-03-19T01:39:55Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Created `tests/e2e/fleet/ollama-health.sh` — 103-line fleet health test following exact close-wait.sh conventions
- Gate 1: checks `/api/tags` via `:8090/exec` to confirm rp-debug model is installed on the pod
- Gate 2: checks `/api/generate` with wall-clock timing to verify rp-debug responds in under 5 seconds
- Unreachable pods are skipped (not failed); exec failures are gracefully handled
- Script passes `bash -n` syntax check and all 10 acceptance criteria

## Task Commits

Each task was committed atomically:

1. **Task 1: Create tests/e2e/fleet/ollama-health.sh E2E test** - `26d108c` (feat)

**Plan metadata:** (this summary commit)

## Files Created/Modified

- `tests/e2e/fleet/ollama-health.sh` - E2E fleet test for Ollama + rp-debug model health on all 8 pods

## Decisions Made

- Wall-clock timing with `date +%s%3N` before/after the outer curl call — same style as close-wait.sh, no powershell dependency
- Gate 2 still runs even when Gate 1 fails, to give more diagnostic detail on partial deployments
- Outer curl uses `--max-time 15` and exec uses `timeout_ms: 10000`, inner Ollama curl uses `--max-time 8` — layered timeouts prevent hangs on unresponsive pods
- Python3 inline JSON parsing for stdout extraction — consistent with startup-verify.sh

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- `ollama-health.sh` is ready to run standalone: `bash tests/e2e/fleet/ollama-health.sh`
- Can be wired into `run-all.sh` as the fleet LLM health gate
- Phase 47 Plan 02 complete — LLM-01 and LLM-02 requirements verified

---
*Phase: 47-local-llm-fleet-deployment*
*Completed: 2026-03-19*
