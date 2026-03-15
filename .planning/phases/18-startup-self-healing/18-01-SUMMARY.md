---
phase: 18-startup-self-healing
plan: 01
subsystem: rc-agent
tags: [self-heal, startup-log, config-repair, registry, crash-recovery]
dependency_graph:
  requires: [deploy/rc-agent.template.toml]
  provides: [self_heal::run, self_heal::SelfHealResult, startup_log::write_phase, startup_log::detect_crash_recovery]
  affects: [crates/rc-agent/src/main.rs]
tech_stack:
  added: []
  patterns: [synchronous-self-heal, phased-startup-log, embedded-template, cfg-windows-gating]
key_files:
  created:
    - crates/rc-agent/src/self_heal.rs
    - crates/rc-agent/src/startup_log.rs
  modified:
    - crates/rc-agent/src/main.rs
decisions:
  - include_str! path is 3 levels up from src/ (../../../deploy/rc-agent.template.toml)
  - START_SCRIPT_CONTENT embedded as const with explicit CRLF for platform-independent writes
  - detect_pod_number_from() exposed as pub for testability; detect_pod_number() private using COMPUTERNAME env var
  - repair_config_for_pod() is cfg(test) only helper to bypass COMPUTERNAME dependency
  - startup_log uses AtomicBool for first-write-truncates semantics without mutex overhead
  - config_hash uses DefaultHasher (deterministic within process, not across Rust versions -- acceptable for startup reports)
metrics:
  duration: 6m33s
  completed: 2026-03-15T09:19:36Z
  tasks: 2
  tests_added: 15
  tests_total: 510
  files_created: 2
  files_modified: 1
---

# Phase 18 Plan 01: Self-Heal + Startup Log Modules Summary

Self-heal and phased startup log modules for rc-agent: config/script/registry repair on every boot, crash recovery detection, and timestamped phase logging for post-mortem diagnosis.

## What Was Built

### self_heal.rs (255 lines, 8 tests)

Config repair, start script repair, and registry key repair module that runs before load_config() on every startup.

- `pub fn run(exe_dir: &Path) -> SelfHealResult` -- checks config/script/registry, repairs what is missing
- `pub fn detect_pod_number_from(hostname: &str) -> Result<u32>` -- parses Pod-N hostname (1-8 range)
- `pub fn config_hash(config_path: &Path) -> String` -- deterministic hash for StartupReport (Plan 02)
- `SelfHealResult` struct with config_repaired, script_repaired, registry_repaired, errors
- Config template embedded via `include_str!("../../../deploy/rc-agent.template.toml")`
- Start script content embedded as const with CRLF line endings
- Registry operations use `#[cfg(windows)]` CREATE_NO_WINDOW pattern from firewall.rs
- All repairs are non-fatal: failures logged and collected, never panics

### startup_log.rs (165 lines, 7 tests)

Phased startup log at `C:\RacingPoint\rc-agent-startup.log` for post-mortem crash analysis.

- `pub fn write_phase(phase, details)` -- first call truncates (fresh log), subsequent append
- `pub fn detect_crash_recovery() -> bool` -- true if previous run did not reach phase=complete
- Testable variants with explicit path and AtomicBool for isolation in unit tests
- Timestamps via chrono UTC RFC3339

### main.rs Wiring (36 lines inserted)

Startup sequence with self-heal before load_config and startup log at each phase:

1. detect_crash_recovery() -- reads previous log
2. phase=init -- truncates log
3. Early lock screen
4. phase=lock_screen
5. self_heal::run() -- repairs config/script/registry
6. phase=self_heal
7. load_config()
8. phase=config_loaded
9. Firewall auto-config
10. phase=firewall
11. Remote ops HTTP server
12. phase=http_server
13. WebSocket connect + register
14. phase=websocket + phase=complete (once, via startup_complete_logged flag)

Variables `crash_recovery`, `heal_result`, and `exe_dir` remain in scope for Plan 02 StartupReport.

## Test Results

- self_heal: 8 tests passing (pod number parsing, config generation, script CRLF, hash, no-repair-when-exists)
- startup_log: 7 tests passing (write, append, truncate, crash detection complete/incomplete/no-file)
- Full suite: 510 tests green (rc-common: 98, rc-agent: 199, rc-core: 213)
- No new warnings from self_heal.rs or startup_log.rs

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 7092f24 | self_heal.rs + startup_log.rs with 15 unit tests |
| 2 | 92ce2ec | Wire into main.rs startup sequence |

## Deviations from Plan

None -- plan executed exactly as written.

## Self-Check: PASSED

- FOUND: crates/rc-agent/src/self_heal.rs
- FOUND: crates/rc-agent/src/startup_log.rs
- FOUND: commit 7092f24
- FOUND: commit 92ce2ec
