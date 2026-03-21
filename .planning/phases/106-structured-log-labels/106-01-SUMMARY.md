---
phase: 106-structured-log-labels
plan: 01
subsystem: infra
tags: [tracing, logging, structured-logs, build-id, rust]

# Dependency graph
requires: []
provides:
  - "build_id field on root rc-agent tracing span (propagates to all log lines automatically)"
  - "LOG_TARGET const for structured target: labels on all 65 main.rs tracing calls"
  - "No legacy bracket prefixes in main.rs tracing messages"
affects: [106-02, 106-03, 106-04, 106-05, 106-06]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "const LOG_TARGET: &str = \"rc-agent\" — module-level target for all tracing calls"
    - "const BUILD_ID: &str = env!(\"GIT_HASH\") — compile-time build ID via build.rs"
    - "tracing::info!(target: LOG_TARGET, ...) — structured target on every call site"
    - "info_span with build_id field — automatic propagation to all child spans/logs"

key-files:
  created: []
  modified:
    - "crates/rc-agent/src/main.rs"

key-decisions:
  - "build_id placed on root span (not per-call) — single point propagates to all 487 call sites across entire binary"
  - "LOG_TARGET = 'rc-agent' matches crate identity — consistent with remote_ops.rs pattern"
  - "Bracket prefixes stripped from messages (e.g. [allowlist], [ws-grace]) — structured target: replaces them"

patterns-established:
  - "Pattern 1: const LOG_TARGET at top of each module, target: LOG_TARGET on every tracing call"
  - "Pattern 2: const BUILD_ID via env!(GIT_HASH) for compile-time version embedding"

requirements-completed: [LOG-01, LOG-02, LOG-03]

# Metrics
duration: 20min
completed: 2026-03-21
---

# Phase 106 Plan 01: Structured Log Labels — main.rs Migration Summary

**build_id added to rc-agent root tracing span + all 65 main.rs tracing calls migrated to target: LOG_TARGET with legacy bracket prefixes removed**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-21T10:00:00+05:30 IST
- **Completed:** 2026-03-21T10:20:00+05:30 IST
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments

- Added `const BUILD_ID: &str = env!("GIT_HASH")` and `const LOG_TARGET: &str = "rc-agent"` to main.rs
- Extended root `tracing::info_span!` to include `build_id = BUILD_ID` — every log line across the entire binary now carries build version automatically
- Migrated all 65 `tracing::info!`, `warn!`, `error!`, `debug!`, `trace!` calls to use `target: LOG_TARGET`
- Stripped legacy bracket prefixes from 3 messages (`[allowlist]`, `[ws-grace]` x2)
- cargo check passes with 0 errors

## Task Commits

1. **Task 1: Add build_id to root span and migrate main.rs tracing calls** - `6ca77f0` (feat)

## Files Created/Modified

- `crates/rc-agent/src/main.rs` - BUILD_ID + LOG_TARGET consts, root span build_id field, target: LOG_TARGET on 65 tracing calls, bracket prefix removal

## Decisions Made

- build_id placed on root span rather than per-call site: the root span propagates the field to all child spans automatically, giving build version on every log line without 487 individual changes
- LOG_TARGET const matches remote_ops.rs BUILD_ID pattern already established in the codebase
- `[ws-grace]` and `[allowlist]` bracket prefixes stripped from messages — the target: field conveys the same routing information in structured form

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Duplicate `target: LOG_TARGET` inserted by formatter**
- **Found during:** Task 1
- **Issue:** rust-analyzer reformatted file between reads, causing some edits to produce `target: LOG_TARGET, target: LOG_TARGET,` on 49 lines
- **Fix:** Used `replace_all` to strip all duplicate occurrences in one pass
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** grep -c 'target: LOG_TARGET, target: LOG_TARGET' returns 0
- **Committed in:** 6ca77f0 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (formatter interaction, Rule 1 bug)
**Impact on plan:** Minor — formatter reformatted file mid-edit, discovered and corrected before commit. No scope creep.

## Issues Encountered

- rust-analyzer reformatted the file between edits, causing duplicate `target:` tokens. Fixed with replace_all before cargo check.

## Next Phase Readiness

- main.rs complete. Pattern established: `const LOG_TARGET + target: LOG_TARGET on every call site`
- Plans 02-06 will apply the same pattern to remaining 422 call sites across 20 modules
- root span build_id propagation active — all subsequent log lines will carry build version automatically

---
*Phase: 106-structured-log-labels*
*Completed: 2026-03-21*
