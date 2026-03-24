---
phase: 184-rc-sentry-crash-handler-upgrade
plan: 02
subsystem: infra
tags: [rust, rc-sentry, crash-handler, ollama, whatsapp, escalation, watchdog]

# Dependency graph
requires:
  - phase: 184-rc-sentry-crash-handler-upgrade
    plan: 01
    provides: "CrashHandlerResult struct, graduated handle_crash() flow, Tier 1+2+spawn verify"

provides:
  - "Tier 3 Ollama diagnosis — fires for unknown patterns with failed spawn verification"
  - "Tier 4 WhatsApp escalation — fires after 3+ consecutive failed recoveries via /api/v1/fleet/alert"
  - "5-minute escalation cooldown to prevent alert spam"
  - "consecutive_failures counter tracking spawn-verified failure streaks"
  - "Complete 4-tier graduated crash recovery pipeline (Tier 1 deterministic -> Tier 2 memory -> restart -> spawn verify -> Tier 3 Ollama -> Tier 4 WhatsApp)"

affects:
  - 184-rc-sentry-crash-handler-upgrade
  - rc-sentry runtime behavior on all pods

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Pass &mut Option<Instant> for cooldown state to avoid global mutable statics"
    - "#[cfg(feature = \"tier1-fixes\")] gate on all Tier 4 variables prevents dead_code warnings when feature is off"
    - "Tier 3+4 wired after handle_crash() using result fields (spawn_verified, restarted, pattern_key)"

key-files:
  created: []
  modified:
    - "crates/rc-sentry/src/tier1_fixes.rs"
    - "crates/rc-sentry/src/main.rs"

key-decisions:
  - "184-02: escalate_to_whatsapp takes &mut Option<Instant> for cooldown — no global mutable state required"
  - "184-02: Tier 3 fires on !spawn_verified && restarted (not !restarted) — Ollama is for unknown patterns that restarted but didn't come up, not for patterns handled by Tier 1"
  - "184-02: removed should_escalate_pattern() and PATTERN_ESCALATION_THRESHOLD — replaced entirely by Tier 3+4 flow"
  - "184-02: consecutive_failures resets to 0 on spawn_verified=true to count only unbroken failure streaks"

patterns-established:
  - "Tier 4 cooldown: pass Option<Instant> by &mut ref to avoid global mutable; check elapsed() < ESCALATION_COOLDOWN"
  - "Fire-and-forget HTTP POST pattern (same TcpStream pattern as post_recovery_event)"

requirements-completed: [GRAD-03, GRAD-04]

# Metrics
duration: 15min
completed: 2026-03-25
---

# Phase 184 Plan 02: Tier 3 Ollama + Tier 4 WhatsApp Escalation Summary

**Tier 3 Ollama diagnosis and Tier 4 WhatsApp escalation wired into graduated crash handler — completing the 4-tier recovery pipeline with spawn-failure-triggered AI diagnosis and 5-min-cooldown staff alerts**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-25T02:50:00+05:30
- **Completed:** 2026-03-25T03:05:00+05:30
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Added `escalate_to_whatsapp()` to tier1_fixes.rs — POSTs `{"pod_id","message","severity":"critical"}` to `/api/v1/fleet/alert` with 5-minute cooldown
- Wired Tier 3 Ollama in main.rs: fires when `!spawn_verified && restarted && pattern not in Tier 2 memory`
- Wired Tier 4 WhatsApp in main.rs: fires when `consecutive_failures >= 3`
- Removed old `should_escalate_pattern` / `PATTERN_ESCALATION_THRESHOLD` (replaced by Tier 3+4)
- Made `get_pod_id()` public for use from main.rs crash handler
- 59 tests pass, release build clean

## Task Commits

1. **Task 1: Add Tier 4 WhatsApp escalation and wire Tier 3+4 into crash handler** - `ad4e6d56` (feat)

## Files Created/Modified

- `crates/rc-sentry/src/tier1_fixes.rs` — added `escalate_to_whatsapp()`, `ESCALATION_COOLDOWN`, made `get_pod_id()` public, added 3 cooldown tests
- `crates/rc-sentry/src/main.rs` — added `consecutive_failures` + `last_escalation` counters, new Tier 3+4 blocks, removed old `should_escalate_pattern` + 2 stale tests

## Decisions Made

- **Cooldown via &mut Option<Instant>:** Passed from the crash handler loop so no global mutable state is needed; caller owns the cooldown state lifetime
- **Tier 3 trigger changed:** Old code triggered Ollama on `!result.restarted`. New trigger is `!result.spawn_verified && result.restarted` — Ollama is only useful when the restart was attempted but the process never came up alive, not when restart wasn't even tried
- **Removed should_escalate_pattern():** The old pattern escalation block only logged `EscalateToAi` to the recovery logger without actually doing anything. The new Tier 4 sends an actual WhatsApp alert. Removing the dead code keeps the flow clean.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed dead should_escalate_pattern() function and its tests**
- **Found during:** Task 1
- **Issue:** After removing the old ai-diagnosis block, `should_escalate_pattern()` and `PATTERN_ESCALATION_THRESHOLD` became unused (only referenced from the block that was replaced). This caused a `dead_code` compiler warning.
- **Fix:** Removed `fn should_escalate_pattern()`, `const PATTERN_ESCALATION_THRESHOLD`, and the 2 associated tests. The `PATTERN_ESCALATION_THRESHOLD` value (3) is now baked directly into `consecutive_failures >= 3` in Tier 4.
- **Files modified:** `crates/rc-sentry/src/main.rs`
- **Verification:** Release build, 59 tests pass with no new errors
- **Committed in:** `ad4e6d56` (Task 1 commit)

**2. [Rule 2 - Missing Critical] Made get_pod_id() public**
- **Found during:** Task 1
- **Issue:** The plan calls `tier1_fixes::get_pod_id()` from main.rs, but the function was private (`fn get_pod_id()`). This would fail to compile.
- **Fix:** Changed to `pub fn get_pod_id()`.
- **Files modified:** `crates/rc-sentry/src/tier1_fixes.rs`
- **Verification:** Compiles and tests pass
- **Committed in:** `ad4e6d56` (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 bug/dead-code cleanup, 1 missing visibility modifier)
**Impact on plan:** Both fixes necessary for correctness. No scope creep.

## Issues Encountered

None — plan executed cleanly after the two auto-fixes above.

## Next Phase Readiness

- Full 4-tier graduated recovery pipeline is complete: Tier 1 (deterministic fixes) -> Tier 2 (pattern memory) -> restart -> spawn verify -> Tier 3 Ollama (unknown patterns + failed spawn) -> Tier 4 WhatsApp (3+ consecutive failures)
- rc-sentry binary is ready to rebuild and deploy to pods
- Server must have `/api/v1/fleet/alert` endpoint deployed for Tier 4 to deliver alerts

---
*Phase: 184-rc-sentry-crash-handler-upgrade*
*Completed: 2026-03-25*
