---
phase: 137-browser-watchdog
plan: "01"
subsystem: rc-agent/lock_screen
tags: [browser-watchdog, safe-mode, anti-cheat, taskkill, tdd]
dependency_graph:
  requires: []
  provides: [BWDOG-03, BWDOG-04, count_edge_processes helper for Plan 02]
  affects: [crates/rc-agent/src/lock_screen.rs]
tech_stack:
  added: []
  patterns: [cfg(test) early return guards system calls in unit tests, safe mode AtomicBool gate before taskkill]
key_files:
  created: []
  modified:
    - crates/rc-agent/src/lock_screen.rs
decisions:
  - "close_browser() child handle kill is always unconditional — only kills our own spawned process"
  - "taskkill loop gated by safe_mode_active.load(Relaxed) — returns early with info log"
  - "count_edge_processes uses #[cfg(test)] early return to avoid real system calls in unit tests (standing rule #74)"
  - "count_edge_processes is a static method (no &self) since it only queries system state"
metrics:
  duration: "3m 28s"
  completed: "2026-03-22T09:26:18 IST"
  tasks_completed: 2
  files_modified: 1
---

# Phase 137 Plan 01: Browser Watchdog — close_browser Safe Mode Gate Summary

Hardened close_browser() in LockScreenManager with safe_mode_active gate before all taskkill commands, and added count_edge_processes() helper with cfg(test) guard plus three passing unit tests proving the safe mode gating behavior.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Gate close_browser() taskkill behind safe_mode_active, add count_edge_processes() | 291c2a9 | crates/rc-agent/src/lock_screen.rs |
| 2 | Unit tests for close_browser safe mode gating and count_edge_processes | 371bf37 | crates/rc-agent/src/lock_screen.rs |

## What Was Built

**close_browser() safe mode gate (BWDOG-04):**
- Child handle kill block unchanged — always runs (only kills our own spawned process)
- Before taskkill loop: checks `self.safe_mode_active.load(Ordering::Relaxed)`
- When safe mode is active: logs "Safe mode active — skipping taskkill for Edge/WebView2 processes" and returns early
- When safe mode is false: proceeds with full msedge.exe + msedgewebview2.exe taskkill + 500ms sleep

**count_edge_processes() helper (BWDOG-02 prep):**
- Public static method on LockScreenManager
- `#[cfg(windows)]` version uses `hidden_cmd("tasklist")` with CSV output, counts lines containing "msedge.exe"
- `#[cfg(test)]` early return returns 0 immediately — no real system calls in test runs
- `#[cfg(not(windows))]` stub returns 0
- Ready for Plan 02 stacking detection

**Three new unit tests (BWDOG-03, BWDOG-04):**
- `test_count_edge_processes_returns_zero_in_test` — confirms cfg(test) guard fires
- `test_close_browser_safe_mode_skips_taskkill` — safe_mode=true, no panic or hang
- `test_close_browser_normal_mode` — safe_mode=false, no browser_process, completes cleanly

## Verification

- `cargo check -p rc-agent-crate`: passes (warnings only, pre-existing)
- `cargo test -p rc-agent-crate -- test_count_edge_processes test_close_browser`: 3/3 pass

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- crates/rc-agent/src/lock_screen.rs modified: FOUND
- Commit 291c2a9: FOUND
- Commit 371bf37: FOUND
- 3 tests pass: CONFIRMED
