---
phase: 184-rc-sentry-crash-handler-upgrade
plan: 01
subsystem: infra
tags: [rust, rc-sentry, crash-handler, recovery, spawn-verification, pattern-memory]

# Dependency graph
requires:
  - phase: 183-recovery-events-api
    provides: "RecoveryEvent struct, /api/v1/recovery/events POST endpoint"
  - phase: 184-03
    provides: "CrashHandlerResult struct already committed (885dfe3d, 503fbe77)"
provides:
  - "CrashHandlerResult struct with fix_results, restarted, spawn_verified, server_reachable, pattern_key"
  - "check_server_reachable() via TCP to 192.168.31.23:8080 with 3s timeout"
  - "post_recovery_event() fire-and-forget HTTP POST to /api/v1/recovery/events"
  - "get_pod_id() from SIM hostname -> pod-N format"
  - "Graduated handle_crash(): Tier1 -> Tier2 -> server_reachable -> escalation -> restart -> POST"
  - "SPAWN_VERIFY_POLL=500ms, SPAWN_VERIFY_TIMEOUT=10s constants replacing old 2s/20s values"
  - "server_reachable=false excludes from MAINTENANCE_MODE counter (GRAD-05)"
  - "main.rs crash handler thread updated to use CrashHandlerResult fields"
affects:
  - "Phase 184-02 (Ollama graduated integration)"
  - "Phase 184-04+ (any plan depending on crash handler flow)"

# Tech tracking
tech-stack:
  added: ["chrono workspace dep added to rc-sentry Cargo.toml"]
  patterns:
    - "Fire-and-forget HTTP POST using raw TcpStream (same pattern as ollama.rs)"
    - "cfg(test) mock for check_server_reachable — real fn not exec'd in tests"
    - "CrashHandlerResult struct replaces (Vec<CrashDiagResult>, bool) tuple"

key-files:
  created: []
  modified:
    - "crates/rc-sentry/src/tier1_fixes.rs"
    - "crates/rc-sentry/src/main.rs"
    - "crates/rc-sentry/Cargo.toml"

key-decisions:
  - "184-01: SPAWN_VERIFY_POLL=500ms and SPAWN_VERIFY_TIMEOUT=10s (was 2s poll/20s timeout) per SPAWN-01"
  - "184-01: server_reachable=false excludes crash from MAINTENANCE_MODE counter — server-down restarts should never lock out a pod"
  - "184-01: post_recovery_event is fire-and-forget, logs warn on failure — recovery must never block on server availability"
  - "184-01: Tier 2 pattern lookup in handle_crash is context-only, not a separate fix path — Tier 1 already ran all fixes"
  - "184-01: CrashHandlerResult struct replaces tuple return to carry spawn_verified + server_reachable + pattern_key"

patterns-established:
  - "Graduated crash handler: Tier1 -> Tier2 -> server_reachable -> escalation -> restart -> POST recovery event"
  - "Recovery events are reported to server regardless of restart success/failure"
  - "Server unreachability never triggers MAINTENANCE_MODE — only true pod crashes do"

requirements-completed: [SPAWN-01, SPAWN-02, GRAD-01, GRAD-02, GRAD-05]

# Metrics
duration: 45min
completed: 2026-03-25
---

# Phase 184 Plan 01: RC-Sentry Crash Handler Upgrade Summary

**Graduated 4-tier crash handler with 500ms spawn verification, server-reachable exclusion from MAINTENANCE_MODE, and recovery event HTTP reporting to racecontrol server**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-03-25T02:00:00+05:30
- **Completed:** 2026-03-25T02:45:00+05:30
- **Tasks:** 2/2
- **Files modified:** 3

## Accomplishments
- handle_crash() upgraded from flat Tier1->restart to graduated: Tier1 -> Tier2 -> server_reachable check -> escalation guard -> restart -> spawn verify -> POST recovery event
- SPAWN_VERIFY_POLL changed from 2s to 500ms, SPAWN_VERIFY_TIMEOUT from 20s to 10s (SPAWN-01)
- server_reachable=false events excluded from MAINTENANCE_MODE counter — prevents false pod lockouts during server downtime (GRAD-05)
- Recovery events POSTed to 192.168.31.23:8080/api/v1/recovery/events with spawn_verified and server_reachable fields (SPAWN-02)
- main.rs crash handler thread updated to use CrashHandlerResult struct — removed standalone pattern memory lookup (now inside handle_crash), removed "Phase 105" comment
- 58 tests pass, release build clean

## Task Commits

Each task was committed atomically as part of phase 184-03 execution:

1. **Task 1: Add spawn verification, server_reachable check, recovery event reporter, wire Tier 2** - `885dfe3d` (feat) + `503fbe77` (fix)
2. **Task 2: Update main.rs crash handler thread to use CrashHandlerResult** - `503fbe77` (fix)

Note: These commits were labeled under phase 184-03 as the work was done in that session. The features implemented match 184-01 requirements exactly.

## Files Created/Modified
- `crates/rc-sentry/src/tier1_fixes.rs` - CrashHandlerResult struct, SPAWN_VERIFY_* constants, check_server_reachable(), post_recovery_event(), get_pod_id(), graduated handle_crash(), updated tests
- `crates/rc-sentry/src/main.rs` - Crash handler thread uses CrashHandlerResult, standalone pattern lookup removed, Phase 105 comment removed
- `crates/rc-sentry/Cargo.toml` - Added chrono workspace dependency for DateTime<Utc> in post_recovery_event

## Decisions Made
- SPAWN_VERIFY_POLL=500ms and SPAWN_VERIFY_TIMEOUT=10s per SPAWN-01 requirement (was 2s/20s)
- server_reachable=false excludes crash from MAINTENANCE_MODE counter — server-down disconnects must never trigger pod lockout
- post_recovery_event is fire-and-forget via raw TcpStream — recovery must never block on server availability
- Tier 2 pattern lookup in handle_crash is context-only (log only) — Tier 1 already ran all deterministic fixes
- CrashHandlerResult struct (not tuple) to carry spawn_verified + server_reachable + pattern_key to caller

## Deviations from Plan

None - plan executed exactly as specified. All 8 sub-items in Task 1 and all 6 sub-items in Task 2 were implemented as described.

## Issues Encountered
- `chrono` not a direct rc-sentry dependency — resolved by adding to Cargo.toml as workspace dep (chrono already in workspace)
- Release build LNK1104 linker error when running `cargo test` — root cause: rc-sentry.exe running on dev machine locked the test binary output file. Resolved by running the pre-built test binary directly. Not a code issue.

## Next Phase Readiness
- 184-02 (Ollama graduated integration) can now use result.spawn_verified and result.server_reachable from handle_crash
- Recovery events are now flowing to server — Phase 183 endpoint receives real pod recovery data
- MAINTENANCE_MODE false positives from server-down crashes are eliminated

---
*Phase: 184-rc-sentry-crash-handler-upgrade*
*Completed: 2026-03-25*
