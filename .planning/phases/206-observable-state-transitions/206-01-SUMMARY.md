---
phase: 206-observable-state-transitions
plan: 01
subsystem: infra
tags: [rust, tracing, rc-agent, rc-sentry, racecontrol, process-guard, watchdog, observable-state]

# Dependency graph
requires:
  - phase: 205-verification-chain-foundation
    provides: VerificationError and rc-common foundation types used across crates

provides:
  - Config fallback warn! logging at all unwrap_or sites in rc-agent main.rs
  - racecontrol config.rs load_or_default() structured warn! on file-not-found and parse-failure
  - Process guard empty allowlist auto-switch to report_only with EMPTY_ALLOWLIST startup_log entry
  - rc-sentry watchdog FSM transition logging to RecoveryLogger JSONL and tracing target:"state"
  - self_monitor lifecycle events (start, first_decision, exit) and sentinel write logging

affects: [207, 208, 209, 210, debugging-silent-failures, process-guard-enablement]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Structured tracing with target:'state' for all degraded state transitions"
    - "Dual logging: eprintln! (always) + tracing::warn! (post-init) for pre-init fallbacks"
    - "RecoveryLogger used in watchdog thread to write FSM transitions to JSONL"
    - "Empty allowlist auto-switch writes to MachineWhitelist directly before scan loop"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/process_guard.rs
    - crates/rc-agent/src/self_monitor.rs
    - crates/rc-sentry/src/watchdog.rs
    - crates/racecontrol/src/config.rs

key-decisions:
  - "All config fallback sites in rc-agent main.rs are after tracing init — use tracing::warn! directly (no pre-init buffer needed)"
  - "Empty allowlist detection writes override directly to MachineWhitelist under write lock, so all downstream scan paths see report_only without extra coordination"
  - "RecoveryLogger for FSM transitions is created inside the watchdog thread (not passed in), pointing to RECOVERY_LOG_POD — same path as crash-handler logger, both append JSONL safely"
  - "self_monitor lifecycle exit log is unreachable by design (loop never breaks) but retained as a sentinel for future exit paths"

patterns-established:
  - "Pattern: target:'state' tracing — all degraded state transitions use this target for log filtering"
  - "Pattern: eprintln! + tracing::warn! dual logging — racecontrol config fallbacks always emit both for pre-init visibility"
  - "Pattern: EMPTY_ALLOWLIST startup_log phase — process guard empty allowlist writes startup_log phase before auto-switching"

requirements-completed: [OBS-02, OBS-03, OBS-05]

# Metrics
duration: 45min
completed: 2026-03-26
---

# Phase 206 Plan 01: Observable State Transitions Summary

**Silent config fallback eliminated: rc-agent 5 unwrap_or sites + racecontrol load_or_default() + process guard empty allowlist + all 4 FSM transitions now emit observable signals at the moment each degraded state occurs**

## Performance

- **Duration:** 45 min
- **Started:** 2026-03-26T03:50:00Z
- **Completed:** 2026-03-26T04:35:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- rc-agent/main.rs: 5 config fallback sites (server_ip, api_base_url, allowlist_poll_url, whitelist_url, feature_flag_url) now emit `tracing::warn!(target: "state", field=..., source=..., fallback=...)` with structured fields
- racecontrol/config.rs: load_or_default() emits structured warn! for parse failure (with SSH banner corruption note) and file-not-found, always with eprintln! as belt-and-suspenders for pre-init visibility
- rc-agent/process_guard.rs: empty allowlist with kill_and_report auto-switches to report_only under write lock, emits eprintln! error + tracing::error!(target:"state") + startup_log::write_phase("EMPTY_ALLOWLIST", ...), with first-scan check for persistent empty state
- rc-sentry/watchdog.rs: all 4 FSM transitions (Healthy→Suspect(1), Suspect(n)→Healthy, Suspect(n)→Suspect(n+1), Suspect(n)→Crashed) emit tracing with target:"state" AND write to RecoveryLogger JSONL
- rc-agent/self_monitor.rs: lifecycle events logged (started, first_decision, exit); both sentinel file writes emit `tracing::warn!(target: "state", sentinel="GRACEFUL_RELAUNCH", ...)`

## Task Commits

Each task was committed atomically:

1. **Task 1: Config fallback warn! logging + empty allowlist auto-response** - `7ba7d093` (feat)
2. **Task 2: rc-sentry FSM transition logging + self_monitor lifecycle** - `5602a64c` (feat)

## Files Created/Modified

- `crates/rc-agent/src/main.rs` - 5 unwrap_or sites use unwrap_or_else with tracing::warn!(target:"state") fallback logging
- `crates/racecontrol/src/config.rs` - load_or_default() adds structured warn! and eprintln! on parse/not-found paths
- `crates/rc-agent/src/process_guard.rs` - empty allowlist detection + auto-switch + EMPTY_ALLOWLIST startup_log + first-scan threshold check
- `crates/rc-sentry/src/watchdog.rs` - RecoveryLogger in watchdog thread, tracing target:"state" on all 4 FSM transition arms
- `crates/rc-agent/src/self_monitor.rs` - lifecycle logs (started/first_decision/exit) + sentinel write state-target tracing

## Decisions Made

- All config fallback sites in rc-agent main.rs are after tracing init (tracing subscriber initialized at line ~290, all config URL derivations at line 600+) — use tracing::warn! directly without pre-init buffer
- Empty allowlist detection writes override directly to MachineWhitelist under write lock rather than maintaining a separate effective_violation_action variable — cleaner and ensures all downstream code paths (autostart audit, port audit, etc.) see the overridden action
- RecoveryLogger for FSM transitions is created inside the watchdog thread pointing to RECOVERY_LOG_POD — same as crash-handler logger, both safely append JSONL without coordination since files are opened per-write
- self_monitor lifecycle exit log is technically unreachable (the inner loop never breaks) but retained as an observable sentinel for when process::exit(0) is called during relaunch

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Field name correction: plan used wl.allowed_processes but actual struct has wl.processes**
- **Found during:** Task 1 (process_guard.rs empty allowlist check)
- **Issue:** Plan's pseudocode used `wl.allowed_processes` and `wl.allowed_startup` but MachineWhitelist struct has `wl.processes` and `wl.autostart_keys`
- **Fix:** Used actual field names from types.rs definition
- **Files modified:** crates/rc-agent/src/process_guard.rs
- **Verification:** cargo check -p rc-agent-crate passes without errors
- **Committed in:** 7ba7d093 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - field name mismatch in plan pseudocode)
**Impact on plan:** Essential correction, no scope change.

## Issues Encountered

- The `crypto::encryption::tests::load_keys_valid_hex` test in racecontrol was already failing before our changes (unrelated to config.rs modifications — that test sets env vars for encryption keys, not TOML parsing). Pre-existing failure, out of scope.

## Next Phase Readiness

- OBS-02, OBS-03, OBS-05 requirements complete
- All 4 crates compile cleanly with no errors
- rc-common (190 tests), rc-sentry (62 tests) pass fully
- Ready for Phase 207 (coverage or boot resilience)

---
*Phase: 206-observable-state-transitions*
*Completed: 2026-03-26*
