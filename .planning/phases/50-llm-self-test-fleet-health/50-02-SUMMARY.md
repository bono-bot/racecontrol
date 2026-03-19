---
phase: 50-llm-self-test-fleet-health
plan: 02
subsystem: rc-agent/ai_debugger
tags: [auto-fix, ai-debugger, tdd, patterns-8-14]
dependency_graph:
  requires: []
  provides: [fix_directx_shader_cache, fix_memory_pressure, fix_dll_repair, fix_steam_restart, fix_performance_throttle, fix_network_adapter_reset]
  affects: [crates/rc-agent/src/ai_debugger.rs]
tech_stack:
  added: []
  patterns: [TDD red-green, keyword-based pattern matching, safe Windows command execution, no-unwrap on Command output]
key_files:
  modified:
    - crates/rc-agent/src/ai_debugger.rs
decisions:
  - "hidden_cmd() used for all new fix functions — consistent with existing patterns 1-7, ensures CREATE_NO_WINDOW on Windows"
  - "fix_memory_pressure uses non-destructive working set trim (MinWorkingSet) not kill — safer for active sessions"
  - "fix_dll_repair uses spawn() not output() — sfc /scannow takes 5-15 min, blocking would stall rc-agent"
  - "fix_steam_restart kills steam.exe with 2s sleep before restart — matches existing pattern from relaunch_self()"
  - "fix_network_adapter_reset falls back to 'Ethernet' adapter name when PowerShell query fails"
  - "false-positive guard test verified: unknown text still returns None after 6 new patterns added"
metrics:
  duration_min: 5
  completed_date: "2026-03-19"
  tasks_completed: 1
  files_modified: 1
---

# Phase 50 Plan 02: Auto-fix Patterns 8-14 Summary

Implemented 6 new deterministic fix functions (patterns 8-14) in `ai_debugger.rs` using TDD. These wire up the Modelfile diagnostic keywords that previously had no code backing them. rc-agent can now autonomously repair DirectX shader cache corruption, memory pressure, missing DLLs, stuck Steam updates, power plan throttling, and network adapter failures.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 (RED) | Failing tests for patterns 8-14 | 1fd8839 | ai_debugger.rs |
| 1 (GREEN) | Implement fix functions + match arms | 7013da5 | ai_debugger.rs |

## What Was Built

Six new fix functions added to `crates/rc-agent/src/ai_debugger.rs`:

- `fix_directx_shader_cache`: Clears NVIDIA GLCache and NV_Cache directories. Matches keywords: `directx`, `d3d`, `gpu driver`, `shader cache`, `pipeline cache`.
- `fix_memory_pressure`: Enumerates high-memory processes (>500MB) not in protected list, trims working set non-destructively via PowerShell. Matches: `out of memory`, `memory leak`.
- `fix_dll_repair`: Spawns `sfc /scannow` minimized in background (does not block). Matches: `dll missing`, `dll not found`.
- `fix_steam_restart`: Kills `steam.exe`, sleeps 2s, restarts from default install path. Matches: `steam` + `update` or `downloading`.
- `fix_performance_throttle`: Sets High Performance power plan via `powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c`. Matches: `low fps`, `frame drops`, `stuttering`.
- `fix_network_adapter_reset`: Detects active Ethernet adapter name via PowerShell, disable + re-enable via `netsh`. Falls back to `"Ethernet"` name on query failure. Matches: `network timeout`, `connection refused`.

Six match arms added to `try_auto_fix()` after the existing Pattern 5 (disk space).

## Test Results

- 16 tests run under `cargo test -p rc-agent-crate test_fix_pattern`
- 15 pattern-specific tests: all pass (GREEN)
- 1 false-positive guard: `test_fix_pattern_no_false_positive` — unknown text returns `None`
- All pre-existing tests unaffected (257 other tests filtered out)

## Decisions Made

1. `hidden_cmd()` used for all new fix functions — consistent with existing patterns 1-7, ensures `CREATE_NO_WINDOW` on Windows to prevent console flashes on pods.
2. `fix_memory_pressure` uses non-destructive working set trim (`MinWorkingSet`) not process kill — protected list enforced at PowerShell query level.
3. `fix_dll_repair` uses `spawn()` not `output()` — `sfc /scannow` takes 5-15 minutes; blocking would stall rc-agent's main loop.
4. `fix_steam_restart` includes a 2s sleep between kill and restart — consistent with relaunch_self() pattern.
5. `fix_network_adapter_reset` falls back to `"Ethernet"` adapter name when PowerShell query fails — safer than returning an error.

## Deviations from Plan

None — plan executed exactly as written. TDD RED/GREEN phases followed strictly.

Pre-existing uncommitted state from in-progress plan 50-01 (`self_test.rs`, `main.rs` changes) was present in the working tree. This had no impact on plan 50-02 execution — the `cargo test -p rc-agent-crate test_fix_pattern` command targets only unit tests in `ai_debugger.rs` and compiled successfully. The 50-01 state is deferred to that plan's execution.

## Self-Check

- [x] `fn fix_directx_shader_cache(` — found at line 754
- [x] `fn fix_memory_pressure(` — found at line 786
- [x] `fn fix_dll_repair(` — found at line 831
- [x] `fn fix_steam_restart(` — found at line 850
- [x] `fn fix_performance_throttle(` — found at line 868
- [x] `fn fix_network_adapter_reset(` — found at line 895
- [x] `8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c` — found at lines 869+871
- [x] Commit 1fd8839 (RED tests) — verified in git log
- [x] Commit 7013da5 (GREEN implementation) — verified in git log
- [x] 16 tests pass: `cargo test -p rc-agent-crate test_fix_pattern`

## Self-Check: PASSED
