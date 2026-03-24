---
phase: 188-james-watchdog-rc-watchdog-grace-window
plan: 01
subsystem: infra
tags: [rust, rc-watchdog, rc-sentry, rc-common, ollama, watchdog, spawn-verification, breadcrumb]

# Dependency graph
requires:
  - phase: 187-james-watchdog-self-monitor
    provides: rc-agent self_monitor sentry-aware relaunch with TCP :8091 check
  - phase: 185-james-watchdog-tier1
    provides: rc-sentry tier1_fixes sentry-restart-breadcrumb.txt write logic
provides:
  - rc-common exports shared ollama module (OllamaResult, query_crash, query_async) via pure TcpStream
  - james_monitor uses rc_common::ollama for Tier 3 AI diagnosis (no inline reqwest)
  - james_monitor polls service health at 500ms/10s after every restart (spawn_verified)
  - rc-watchdog pod service reads sentry-restart-breadcrumb.txt with 30s grace window
  - rc-watchdog pod service verifies rc-agent spawn via tasklist poll (500ms/10s)
affects: [rc-watchdog, rc-sentry, rc-common, james-ai-healer, spawn-verification]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SPAWN-01: poll health at 500ms for 10s after any spawn to confirm child alive"
    - "COORD: sentry breadcrumb grace window prevents rc-watchdog/rc-sentry double-restart race"
    - "Shared ollama module: pure std TcpStream in rc-common, no reqwest dependency"

key-files:
  created:
    - crates/rc-common/src/ollama.rs
  modified:
    - crates/rc-common/src/lib.rs
    - crates/rc-sentry/src/main.rs
    - crates/rc-watchdog/src/james_monitor.rs
    - crates/rc-watchdog/src/service.rs
  deleted:
    - crates/rc-sentry/src/ollama.rs

key-decisions:
  - "Moved ollama.rs to rc-common so both rc-sentry and rc-watchdog share one implementation (no duplication)"
  - "james_monitor uses OLLAMA_HOST_PORT=127.0.0.1:11434 (TcpStream host:port format) not a URL"
  - "spawn_verified stored in RecoveryDecision.context alongside failure_count and ai_diagnosis"
  - "sentry_breadcrumb_active uses std::fs::metadata().modified().elapsed() — no system clock dependency"
  - "grace_secs=0 test pattern to verify stale file detection without sleep"

patterns-established:
  - "SPAWN-01: Always verify a spawned process is alive — poll at 500ms for 10s after spawn"
  - "COORD: Breadcrumb files coordinate between independent recovery systems to prevent double-restart"

requirements-completed: [JAMES-01, JAMES-02, JAMES-03]

# Metrics
duration: 25min
completed: 2026-03-25
---

# Phase 188 Plan 01: James Watchdog RC-Watchdog Grace Window Summary

**Shared ollama module in rc-common (pure TcpStream), spawn verification (500ms/10s) in james_monitor and rc-watchdog, and 30s sentry-breadcrumb grace window to prevent double-restart coordination failures**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-25T~IST
- **Completed:** 2026-03-25T~IST
- **Tasks:** 2
- **Files modified:** 5 (1 created, 3 modified, 1 deleted)

## Accomplishments
- rc-common now exports `pub mod ollama` with `OllamaResult`, `query_crash`, `query_async` — single shared implementation for all crates
- james_monitor Tier 3 AI diagnosis now calls `rc_common::ollama::query_crash` instead of inline reqwest HTTP; `ai_diagnose()` is 10 lines instead of 30
- james_monitor `attempt_restart()` returns `bool` (spawn_verified) after polling service health at 500ms for 10s; result logged and stored in `RecoveryDecision.context`
- rc-watchdog pod service reads `sentry-restart-breadcrumb.txt` mtime before attempting restart; skips with log message if within 30s grace window
- rc-watchdog pod service polls `is_rc_agent_running()` at 500ms for 10s after session1 spawn; logs `spawn_verified=true/false`
- 5 new tests: 2 in rc-common (ollama), 3 in rc-watchdog service (breadcrumb grace window)

## Task Commits

1. **Task 1: Move ollama.rs to rc-common, rewire consumers** - `1962154d` (feat)
2. **Task 2: Sentry breadcrumb grace window for pod watchdog service** - `7c06a364` (feat)

## Files Created/Modified
- `crates/rc-common/src/ollama.rs` - Created: shared Ollama module (moved from rc-sentry, pure std TcpStream)
- `crates/rc-common/src/lib.rs` - Added `pub mod ollama;`
- `crates/rc-sentry/src/main.rs` - Removed `mod ollama;`, replaced all `ollama::` refs with `rc_common::ollama::`
- `crates/rc-sentry/src/ollama.rs` - Deleted (moved to rc-common)
- `crates/rc-watchdog/src/james_monitor.rs` - Replaced inline `ai_diagnose` (reqwest) with `rc_common::ollama::query_crash`; added `verify_spawn()` + updated `attempt_restart()` to return bool
- `crates/rc-watchdog/src/service.rs` - Added `sentry_breadcrumb_active()`, `SENTRY_BREADCRUMB_PATH`, `SENTRY_GRACE_SECS`; inserted breadcrumb check + spawn verification loop in poll loop; 3 new tests

## Decisions Made
- Used `OLLAMA_HOST_PORT = "127.0.0.1:11434"` (host:port format for `TcpStream::connect_timeout`) not a URL — the shared `query_crash` API takes `ollama_url: Option<&str>` as `host:port`
- Kept `OLLAMA_MODEL` constant in james_monitor.rs (james monitor uses `qwen2.5:3b` locally, may differ from fleet default)
- `spawn_verified` bool added to `RecoveryDecision.context` string alongside `failure_count` and `ai_diagnosis` to preserve audit trail without schema changes

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 188 (FINAL phase of v17.1 Watchdog-to-AI Migration) is complete
- rc-watchdog binary built successfully with release profile
- All 35 rc-watchdog tests pass, rc-common ollama tests pass, rc-sentry cargo check clean
- Deploy rc-watchdog to James's machine when ready (no pod deploy needed — runs on .27 only)

---
*Phase: 188-james-watchdog-rc-watchdog-grace-window*
*Completed: 2026-03-25*
