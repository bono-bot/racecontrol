---
phase: 215-self-improving-intelligence
plan: 04
subsystem: intelligence
tags: [bash, self-patch, learning, autonomy, ce-methodology, proposals]

# Dependency graph
requires:
  - phase: 215-03
    provides: approval-sync.sh queuing new_audit_check/self_patch proposals as queued_for_selfpatch
  - phase: 215-02
    provides: suggestion-engine.sh PROPOSALS_DIR and SUGGESTIONS_JSONL paths
  - phase: 215-01
    provides: pattern-tracker.sh LEARN-01 pipeline, auto-detect.sh intelligence integration pattern

provides:
  - self_patch_loop() — processes one queued_for_selfpatch proposal per run with CE methodology
  - _self_patch_enabled() — returns 1 by default, 0 only when config explicitly has self_patch_enabled=true
  - auto-detect.sh sources self-patch.sh as 4th intelligence script in chain
  - self_patch_enabled=false field in auto-detect-config.json (independent of auto_fix_enabled)

affects:
  - 215-05 (if any further intelligence phases)
  - 216-testing (tests should cover self_patch_loop toggle + scope safety)
  - auto-detect.sh full intelligence chain

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "CE methodology applied to autonomous script self-modification (document symptom, identify target, apply, verify, revert)"
    - "Toggle pattern with default=false (self_patch_enabled) mirrors existing _auto_fix_enabled() but inverted default"
    - "Blast radius limit: process ONE proposal per loop invocation"
    - "Scope safety via realpath comparison against ALLOWED_PATCH_DIRS"

key-files:
  created:
    - scripts/intelligence/self-patch.sh
  modified:
    - audit/results/auto-detect-config.json
    - scripts/auto-detect.sh

key-decisions:
  - "Default=false for self_patch_enabled — requires explicit operator action to enable script self-modification (LEARN-09)"
  - "Missing config = disabled (unlike auto_fix_enabled which defaults to true on missing config) — explicit opt-in semantics"
  - "Process only ONE proposal per loop run — limits blast radius from autonomous code modification"
  - "Scope restriction enforced via realpath comparison — prevents path traversal attacks targeting non-detector files"
  - "bash -n + threshold presence check as minimum verification — no --test flag dependency needed"
  - "git checkout -- <file> for revert (not git stash) — simpler, reliable for single-file restores"

patterns-established:
  - "CE methodology self-patch: Document symptom → grep ALLOWED_PATCH_DIRS → threshold sed → bash -n verify → commit or checkout revert"
  - "SELFPATCH_ATTEMPT/SELFPATCH_APPLIED/SELFPATCH_REVERTED as audit trail in suggestions.jsonl"
  - "Guard-wrap pattern for intelligence function calls: type -t check before calling"
  - "Self-patch scope: only scripts/detectors/ and scripts/healing/ — never auto-detect.sh or audit/lib/"

requirements-completed: [LEARN-07, LEARN-08, LEARN-09]

# Metrics
duration: 18min
completed: 2026-03-26
---

# Phase 215 Plan 04: Self-Patch Loop Summary

**Self-modifying intelligence loop with CE methodology: threshold-only patches, realpath scope safety, bash -n verification, auto-revert on failure, independent self_patch_enabled=false toggle**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-26T09:20:00Z
- **Completed:** 2026-03-26T09:38:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Created scripts/intelligence/self-patch.sh with full CE methodology: document symptom, find target in ALLOWED_PATCH_DIRS via realpath, apply threshold sed, verify with bash -n + value check, commit+push or auto-revert
- Added self_patch_enabled=false field to auto-detect-config.json as independent toggle from auto_fix_enabled
- Wired self-patch.sh as 4th intelligence script in auto-detect.sh — sourced after suggestion-engine.sh, self_patch_loop called after run_suggestion_engine with guard-wrap

## Task Commits

Each task was committed atomically:

1. **Task 1: Create self-patch.sh** - `70253ce0` (feat)
2. **Task 2: Add toggle to config + wire into auto-detect.sh** - `a0882cce` (feat)

## Files Created/Modified

- `scripts/intelligence/self-patch.sh` — self_patch_loop() and _self_patch_enabled(), 413 lines, full CE methodology with auto-revert
- `audit/results/auto-detect-config.json` — self_patch_enabled=false field added, independent from auto_fix_enabled
- `scripts/auto-detect.sh` — source self-patch.sh line 74, self_patch_loop guard-call at end of generate_report_and_notify

## Decisions Made

- Default=false for self_patch_enabled (unlike auto_fix_enabled which defaults to true): script self-modification is more sensitive than fix execution, requires explicit operator consent
- Missing config = disabled: explicit opt-in semantics for self_patch_enabled, preventing accidental activation on new deployments
- ONE proposal per loop run: blast radius limit for autonomous code changes — safe approach for initial deployment
- Scope restriction via realpath comparison: canonical path prevents symlink/traversal attacks targeting auto-detect.sh or audit/lib/
- bash -n + threshold value presence as verification: lightweight, zero-dependency check sufficient for threshold-only patches

## Deviations from Plan

None — plan executed exactly as written.

## Issues Encountered

None.

## Next Phase Readiness

- Full 4-script intelligence chain complete: pattern-tracker → trend-analyzer → suggestion-engine → self-patch
- LEARN-07/08/09 requirements fulfilled
- Self-patching is disabled by default — operator must set self_patch_enabled=true in auto-detect-config.json to activate
- Phase 216 (tests) can now validate: toggle behavior, scope rejection, revert on verification failure, SELFPATCH_APPLIED/REVERTED logging

---
*Phase: 215-self-improving-intelligence*
*Completed: 2026-03-26*
