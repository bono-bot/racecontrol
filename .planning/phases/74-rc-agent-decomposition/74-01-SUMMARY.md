---
phase: 74-rc-agent-decomposition
plan: "01"
subsystem: rc-agent
tags: [rust, refactor, config, module-extraction]
requirements: [DECOMP-01]

dependency_graph:
  requires: []
  provides: [crates/rc-agent/src/config.rs]
  affects: [crates/rc-agent/src/main.rs, crates/rc-agent/src/billing_guard.rs]

tech_stack:
  added: []
  patterns:
    - Config module extraction with pub/pub(crate) visibility gating
    - mod config; declaration in main.rs root for Rust module system

key_files:
  created:
    - crates/rc-agent/src/config.rs
  modified:
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/billing_guard.rs

decisions:
  - AgentConfig fields made pub (not pub(crate)) — needed for cross-module access in later extractions
  - detect_installed_games made pub(crate) — crate-internal, not public API
  - validate_config made pub(crate) — crate-internal, not public API
  - load_config made pub — called from main.rs fn main(), future app_state.rs
  - AgentConfig removed from use imports in main.rs — type inferred from load_config() return

metrics:
  duration_minutes: 28
  completed_date: "2026-03-21"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 2
---

# Phase 74 Plan 01: Config Module Extraction Summary

**One-liner:** Extracted all TOML config types and load/validate/detect functions from 3000-line main.rs into a new config.rs module (575 lines, 7 structs, 20 tests).

## What Was Built

Created `crates/rc-agent/src/config.rs` containing:
- 7 config structs: `AgentConfig`, `PodConfig`, `CoreConfig`, `WheelbaseConfig`, `TelemetryPortsConfig`, `GamesConfig`, `KioskConfig`
- 6 default value functions: `default_sim_ip`, `default_sim_port`, `default_core_url`, `default_wheelbase_vid`, `default_wheelbase_pid`, `default_telemetry_ports`
- `default_auto_end_orphan_session_secs` (pub(crate))
- `detect_installed_games` (pub(crate))
- `is_steam_app_installed` (private)
- `validate_config` (pub(crate))
- `config_search_paths` (pub(crate))
- `load_config` (pub)
- 20 config tests moved from main.rs

`main.rs` changes:
- Added `mod config;` to mod declarations
- Added `use config::{load_config, detect_installed_games};`
- Removed `use serde::Deserialize;` (no longer needed in root)
- Removed `use game_process::GameExeConfig;` (only used in config types)
- Removed `AiDebuggerConfig` from ai_debugger import (kept `PodStateSnapshot`)
- All 7 config struct definitions removed (~170 lines)
- `validate_config`, `config_search_paths`, `load_config` removed (~80 lines)
- 20 config tests removed from main.rs tests module

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] billing_guard.rs referenced moved function via crate root**
- **Found during:** Task 1 (cargo test compilation)
- **Issue:** `billing_guard.rs` test at line 256 used `crate::default_auto_end_orphan_session_secs()` which is now in `crate::config::` module
- **Fix:** Updated reference to `crate::config::default_auto_end_orphan_session_secs()`
- **Files modified:** `crates/rc-agent/src/billing_guard.rs`
- **Commit:** 02019c6

## Verification

- `cargo build --bin rc-agent`: Finished (0 errors)
- `cargo build --bin rc-sentry`: Finished (0 errors, no tokio contamination)
- `cargo build --tests -p rc-agent-crate`: Finished (0 errors)
- Test binary execution blocked by Windows Application Control policy (pre-existing constraint on this machine, not introduced by this change)
- All 7 acceptance criteria: PASS

## Self-Check: PASSED

- `crates/rc-agent/src/config.rs` exists: FOUND
- `mod config;` in main.rs: FOUND
- `pub struct AgentConfig` in config.rs: FOUND
- `pub fn load_config` in config.rs: FOUND
- `struct AgentConfig` NOT in main.rs: CONFIRMED
- `fn load_config` NOT in main.rs: CONFIRMED
- `grep -c "struct.*Config" crates/rc-agent/src/config.rs` = 7: CONFIRMED
- `grep -c "struct.*Config" crates/rc-agent/src/main.rs` = 0: CONFIRMED
- Commit 02019c6: FOUND in git log
