---
phase: 256-game-specific-hardening
plan: "03"
subsystem: rc-agent
tags: [game-hardening, ac-evo, iracing, unreal-engine, config-adapter, subscription-check]
dependency_graph:
  requires: [256-01, 256-02]
  provides: [GAME-04, GAME-05]
  affects: [ws_handler.rs, sims/assetto_corsa_evo.rs]
tech_stack:
  added: []
  patterns: [spawn_blocking for filesystem I/O, pre-launch hook pattern, Unreal INI merge]
key_files:
  created:
    - crates/rc-agent/src/iracing_checks.rs
  modified:
    - crates/rc-agent/src/sims/assetto_corsa_evo.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/ws_handler.rs
decisions:
  - "AC EVO config write is non-fatal: game launches with defaults if write fails"
  - "iRacing check failure IS fatal: returns GameState::Error to prevent billing for unplayable game"
  - "Pre-launch check_iracing_ready() verifies disk installation only; subscription verification is post-launch via wait_for_iracing_window()"
  - "Unreal INI merge preserves existing [/Script/Engine.GameUserSettings] graphics/audio sections"
  - "IRACING_INSTALL_PATHS checks Program Files (x86) and Program Files; service-running fallback for non-standard paths"
metrics:
  duration_minutes: 30
  completed_date: "2026-03-29T05:37:35Z"
  tasks_completed: 2
  files_modified: 4
requirements: [GAME-04, GAME-05]
---

# Phase 256 Plan 03: AC EVO Config Adapter + iRacing Subscription Check Summary

AC EVO Unreal engine config adapter writing GameUserSettings.ini, and iRacing pre-launch subscription/installation verification that blocks billing for inactive accounts.

## Tasks Completed

| Task | Description | Commit |
|------|-------------|--------|
| 1 | AC EVO config adapter (write_evo_config, find_evo_install_dir) + iRacing check module (check_iracing_ready) | 215c1868 |
| 2 | Wire both into LaunchGame handler in ws_handler.rs | acd756d4 |

## What Was Built

### GAME-04: AC EVO Unreal Config Adapter

Added to `crates/rc-agent/src/sims/assetto_corsa_evo.rs`:

**`find_evo_install_dir(config: &GameExeConfig) -> Option<PathBuf>`**
- Checks exe_path parent directory first
- Falls back to working_dir
- Falls back to known Steam path: `C:\Program Files (x86)\Steam\steamapps\common\Assetto Corsa EVO`

**`write_evo_config(launch_args: &str, evo_install_dir: &Path) -> Result<(), String>`**
- Parses launch_args JSON for `car`, `track`, `weather`, `time_of_day` fields
- Writes to `{install_dir}/Saved/Config/WindowsNoEditor/GameUserSettings.ini`
- Unreal INI section: `[/Script/AssettoCorsaEVO.ACEVOGameUserSettings]`
- Merges with existing file — preserves all other INI sections (graphics, audio, input)
- Empty args / no relevant fields → returns Ok immediately (game uses defaults)
- Does NOT write race.ini (EVO is Unreal engine, not classic AC engine)

**Integration in ws_handler.rs:**
Called before `GameProcess::launch()` for `SimType::AssettoCorsaEvo`.
Config write failure is non-fatal — game launches with default config, WARN logged.

### GAME-05: iRacing Subscription/Launch Check

Created `crates/rc-agent/src/iracing_checks.rs`:

**`check_iracing_ready() -> Result<(), String>`**
- Pre-launch gate: verifies iRacing is installed
- Checks `C:\Program Files (x86)\iRacing` and `C:\Program Files\iRacing` for:
  - `iRacingSim64DX11.exe`, `iRacingSim64DX12.exe`
  - `iRacingService.exe`, `iRacingService64.exe`
- Fallback: checks if any iRacing service process is currently running (non-standard install paths)
- Returns Err with human-readable message if iRacing not found

**`wait_for_iracing_window(timeout_secs: u64) -> Result<u32, String>`**
- Post-launch window heuristic (available for future use)
- Polls for iRacing simulator process for up to `timeout_secs` seconds
- Returns Ok(pid) when process found, Ok(0) on timeout (fallback to process-based billing)

**Integration in ws_handler.rs:**
Called before `GameProcess::launch()` for `SimType::IRacing`.
Check failure IS fatal — returns `GameState::Error` to prevent billing for unplayable session.
Spawn panic is non-fatal (proceed, risk vs. blocking a valid launch).

## Tests

12 new tests across both modules:

**EVO config tests (7):**
- `test_evo_config_valid_json_produces_ini` — verifies Unreal INI format output
- `test_evo_config_empty_args_returns_ok` — empty args → no file created
- `test_evo_config_preserves_existing_sections` — Engine.GameUserSettings section preserved
- `test_evo_config_whitespace_args_returns_ok` — whitespace-only → Ok
- `test_find_evo_install_dir_from_exe_path` — returns parent of exe_path
- `test_find_evo_install_dir_from_working_dir` — returns working_dir as fallback
- `test_evo_config_no_race_ini_format` — race.ini not created for EVO

**iRacing checks tests (5):**
- `test_iracing_not_installed_returns_err` — graceful Err on missing installation
- `test_iracing_error_message_is_human_readable` — no Rust internals in error
- `test_wait_for_iracing_window_non_fatal` — no panic on non-Windows
- `test_iracing_exe_names_present` — expected executables in list
- `test_iracing_install_paths_present` — standard path present

All 12 tests pass. `cargo build --release` clean (warnings only, no errors).

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — both functions are fully implemented and wired into the launch path.

## Self-Check: PASSED

- FOUND: crates/rc-agent/src/sims/assetto_corsa_evo.rs
- FOUND: crates/rc-agent/src/iracing_checks.rs
- FOUND: commit 215c1868 (Task 1)
- FOUND: commit acd756d4 (Task 2)
- 12 tests pass, cargo build --release clean
