---
phase: 102-whitelist-schema-config-fetch-endpoint
plan: "01"
subsystem: infra
tags: [process-guard, config, toml, serde, rust, whitelist]

requires:
  - phase: 101-protocol-foundation
    provides: MachineWhitelist and ProcessGuard protocol types in rc-common

provides:
  - ProcessGuardConfig, AllowedProcess, ProcessGuardOverride structs in racecontrol/src/config.rs
  - Config.process_guard field wired with #[serde(default)]
  - C:/RacingPoint/racecontrol.toml populated with 185 global allowed entries + 3 per-machine overrides
  - 6 TDD round-trip deserialization tests for process guard config

affects:
  - 102-02 (HTTP endpoint reads Config.process_guard from AppState)
  - 103-process-scan (uses config to evaluate running processes)
  - 104-reporting (uses violation_action to decide kill vs report)
  - 105-enforcement (uses per-machine overrides for kill decisions)

tech-stack:
  added: []
  patterns:
    - "Manual Default impl for ProcessGuardConfig — serde default= functions not called by #[derive(Default)]; explicit impl required"
    - "ProcessGuardOverride uses #[derive(Default)] because all fields are Vec/bool which have natural defaults"
    - "Per-machine overrides stored in HashMap<String, ProcessGuardOverride> for dynamic machine key lookup"

key-files:
  created:
    - C:/RacingPoint/racecontrol.toml
  modified:
    - crates/racecontrol/src/config.rs

key-decisions:
  - "racecontrol.toml is outside git repo — created at C:/RacingPoint/racecontrol.toml directly"
  - "Steam processes (steam.exe, steamservice.exe, etc.) go in pod deny_processes only — not in global allowed list"
  - "ollama.exe in both global allowed (machines=[pod] for pod-8) AND james overrides — allows ollama on both james and pod-8"
  - "racecontrol.exe server-only, rc-agent.exe pod-only — cross-machine binary presence = violation"
  - "violation_action = report_only locked — no kills until false-positive validation on Pod 8"
  - "enabled = false by default — safe rollout; operator must explicitly opt in"

patterns-established:
  - "Process guard TOML: [[process_guard.allowed]] array-of-tables for global list, [process_guard.overrides.{machine}] for per-machine"
  - "config.rs pattern: struct fields with #[serde(default = 'fn_name')] + Manual Default impl invoking same fns"

requirements-completed: [GUARD-01, GUARD-02, GUARD-03]

duration: 35min
completed: 2026-03-21
---

# Phase 102 Plan 01: Whitelist Schema + Config Section Summary

**ProcessGuardConfig/AllowedProcess/ProcessGuardOverride structs with serde Deserialize added to racecontrol/src/config.rs; C:/RacingPoint/racecontrol.toml populated with 185 global allowed entries and 3 per-machine override sections covering all 11 Racing Point machines**

## Performance

- **Duration:** 35 min
- **Started:** 2026-03-21T09:35:00Z
- **Completed:** 2026-03-21T10:10:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `ProcessGuardConfig`, `AllowedProcess`, `ProcessGuardOverride` structs to `crates/racecontrol/src/config.rs` with correct serde Deserialize and manual Default impl
- Wired `Config.process_guard: ProcessGuardConfig` with `#[serde(default)]` and added to `default_config()`
- Wrote 6 TDD round-trip tests (RED then GREEN): all pass, zero regressions
- Created `C:/RacingPoint/racecontrol.toml` with comprehensive deny-by-default whitelist: 185 `[[process_guard.allowed]]` entries covering all kiosk.rs ALLOWED_PROCESSES categories
- Added three per-machine override sections: `james` (dev tools + ollama), `pod` (deny steam + dev tools), `server` (deny rc-agent + games)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ProcessGuardConfig structs to racecontrol/src/config.rs** - `17750da` (feat)

Task 2 (racecontrol.toml) is not in git — the file lives at `C:/RacingPoint/racecontrol.toml` outside the repo (server-only config, as documented in plan).

## Files Created/Modified

- `crates/racecontrol/src/config.rs` - Added AllowedProcess, ProcessGuardOverride, ProcessGuardConfig structs; extended Config struct; 6 new tests
- `C:/RacingPoint/racecontrol.toml` - Created with [process_guard] section: 185 global allowed entries + james/pod/server overrides

## Decisions Made

- racecontrol.toml is outside git repo — plan confirmed this, created directly at `C:/RacingPoint/`
- Steam processes in pod `deny_processes` only (not in global allowed) — enforces the v12.1 trigger incident rule
- Ollama in both global `allowed` (machines=["pod"]) for pod-8 AND in `james` `allow_extra_processes` — both machines need it
- `racecontrol.exe` machines=["server"], `rc-agent.exe` machines=["pod"] — cross-machine binary presence is an explicit violation
- `violation_action = "report_only"` hardcoded, `enabled = false` — zero risk on initial deploy

## Deviations from Plan

None - plan executed exactly as written. Pre-existing billing rate cache test failures (3 integration tests) confirmed out-of-scope before and after changes.

## Issues Encountered

- Package name is `racecontrol-crate` (not `racecontrol`) in cargo metadata — discovered when `-p racecontrol` failed. Corrected to `-p racecontrol-crate` for all test/build commands.

## User Setup Required

None - racecontrol.toml was created at `C:/RacingPoint/racecontrol.toml`. No external service configuration required.

## Next Phase Readiness

- Plan 02 (HTTP endpoint) can now read `config.process_guard` from `AppState` — `Config.process_guard` field exists and deserializes cleanly
- All 6 config tests passing; full test suite at 63/66 pass (3 pre-existing billing rate failures, unrelated)
- `enabled = false` in TOML — safe to deploy; no guard activity until explicitly enabled

## Self-Check: PASSED

- config.rs: FOUND
- C:/RacingPoint/racecontrol.toml: FOUND
- 102-01-SUMMARY.md: FOUND
- commit 17750da: FOUND
- All 6 new process_guard tests: PASSING
- Zero compile errors

---
*Phase: 102-whitelist-schema-config-fetch-endpoint*
*Completed: 2026-03-21*
