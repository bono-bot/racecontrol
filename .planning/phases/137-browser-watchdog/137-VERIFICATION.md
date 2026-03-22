---
phase: 137-browser-watchdog
verified: 2026-03-22T10:30:00+05:30
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 137: Browser Watchdog Verification Report

**Phase Goal:** rc-agent autonomously detects and recovers from Edge liveness failures and stack buildup without any human intervention or server involvement
**Verified:** 2026-03-22T10:30:00 IST
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | rc-agent polls Edge browser liveness every 30s and relaunches if Edge has died | VERIFIED | `browser_watchdog_interval: tokio::time::interval(Duration::from_secs(30))` at event_loop.rs:108; `is_browser_alive()` called at line 945; `launch_browser()` called at line 951 |
| 2 | rc-agent detects Edge stacking (>5 msedge.exe) and kills all before relaunching | VERIFIED | `edge_count > 5` check at event_loop.rs:933; `close_browser()` + `launch_browser()` called at lines 939-940; `count_edge_processes()` at lock_screen.rs:777 |
| 3 | close_browser() kills ALL msedge.exe and msedgewebview2.exe, not just the spawned child handle | VERIFIED | loop over `["msedge.exe", "msedgewebview2.exe"]` at lock_screen.rs:710; uses `taskkill /F /IM` for each |
| 4 | Browser watchdog is suppressed during safe mode — no taskkill while anti-cheat active | VERIFIED | `safe_mode_active.load(Relaxed)` gate at event_loop.rs:922 (skips whole tick); also in close_browser at lock_screen.rs:700 (skips taskkill loop) |
| 5 | close_browser() logs when it skips taskkill due to safe mode | VERIFIED | `tracing::info!(..., "Safe mode active — skipping taskkill for Edge/WebView2 processes")` at lock_screen.rs:701 |
| 6 | Watchdog does NOT fire when lock screen is Hidden | VERIFIED | `is_browser_expected()` returns `!matches!(*state, LockScreenState::Hidden)` at lock_screen.rs:769; checked at event_loop.rs:927 |
| 7 | Watchdog lifecycle events are logged | VERIFIED | `"Browser watchdog: Edge stacking detected..."` at event_loop.rs:936; `"Browser watchdog: Edge not alive — relaunching"` at line 948; both at warn level |
| 8 | Unit tests for close_browser and count_edge_processes compile and pass | VERIFIED | 3/3 tests passed: `test_count_edge_processes_returns_zero_in_test`, `test_close_browser_safe_mode_skips_taskkill`, `test_close_browser_normal_mode` |
| 9 | count_edge_processes() has cfg(test) guard — no real system calls in tests | VERIFIED | `#[cfg(test)] { return 0; }` early return at lock_screen.rs:778-779 |

**Score:** 9/9 truths verified

---

## Required Artifacts

### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/lock_screen.rs` | Hardened close_browser() with safe mode gate | VERIFIED | safe_mode_active.load at line 700; taskkill loop for both msedge.exe + msedgewebview2.exe at line 710; count_edge_processes() at line 777 with cfg(test) guard at line 778 |

### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-agent/src/event_loop.rs` | browser_watchdog_interval tick arm in tokio::select! | VERIFIED | Field declared at line 69; initialized at line 108 with Duration::from_secs(30); tick handler at lines 920-953 |
| `crates/rc-agent/src/lock_screen.rs` | is_browser_alive() method on LockScreenManager | VERIFIED | pub fn is_browser_alive() at line 737 (windows) and line 760 (non-windows stub) |

---

## Key Link Verification

### Plan 01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| lock_screen.rs close_browser() | safe_mode_active AtomicBool | Ordering::Relaxed load before taskkill | WIRED | `self.safe_mode_active.load(std::sync::atomic::Ordering::Relaxed)` at line 700, before taskkill loop at line 710 |

### Plan 02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| event_loop.rs browser_watchdog tick | LockScreenManager::is_browser_alive + count_edge_processes + close_browser + launch_browser | state.lock_screen method calls | WIRED | All four methods called in select! handler: count_edge_processes (line 932), close_browser (939, 950), launch_browser (940, 951), is_browser_alive (945) |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| BWDOG-01 | Plan 02 | rc-agent polls browser_process liveness every 30s and relaunches Edge if dead | SATISFIED | browser_watchdog_interval(30s) + is_browser_alive() check + launch_browser() relaunch in event_loop.rs:920-952 |
| BWDOG-02 | Plan 02 | rc-agent detects Edge stacking (>5 msedge.exe processes) and kills all before relaunching | SATISFIED | count_edge_processes() + edge_count > 5 check + close_browser() + launch_browser() in event_loop.rs:932-941 |
| BWDOG-03 | Plan 01 | close_browser() kills ALL msedge.exe and msedgewebview2.exe, not just the spawned child | SATISFIED | taskkill /F /IM loop over both executables in lock_screen.rs:710-727 |
| BWDOG-04 | Plan 01, Plan 02 | Browser watchdog suppressed during safe mode — no taskkill while protected game running | SATISFIED | Dual-layer gate: whole watchdog tick skipped at event_loop.rs:922, AND close_browser() taskkill skipped at lock_screen.rs:700 |

No orphaned requirements — BWDOG-05 and BWDOG-06 are mapped to future phases in REQUIREMENTS.md, not Phase 137.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| crates/rc-agent/src/lock_screen.rs | multiple | Pre-existing warnings (65 flagged by cargo) | Info | Pre-existing, not introduced by this phase; cargo check passes |

No TODO/FIXME/placeholder patterns found in modified code. No empty implementations. No stub handlers.

---

## Human Verification Required

### 1. Live Edge crash recovery on pod

**Test:** Deploy rc-agent to a pod, open lock screen, manually kill all msedge.exe processes via Task Manager.
**Expected:** Within 30 seconds, Edge relaunches automatically with the kiosk URL.
**Why human:** Cannot simulate real Edge process lifecycle in automated checks.

### 2. Edge stacking recovery threshold

**Test:** Deploy to pod, force 6+ Edge processes (e.g., by calling launch_browser repeatedly without cleanup), wait 30s.
**Expected:** Watchdog fires, kills all Edge instances, relaunches one clean instance.
**Why human:** count_edge_processes() returns 0 in test mode; real pod process counting not exercised by unit tests.

### 3. Safe mode non-interference

**Test:** Enable anti-cheat safe mode on a pod, induce Edge crash, observe for 90s.
**Expected:** Watchdog tick fires but skips all recovery actions; no taskkill executed; logged "skipping" only.
**Why human:** safe_mode_active is set by the game session controller — integration path not exercised by unit tests.

---

## Summary

Phase 137 achieved its goal. All four requirements (BWDOG-01 through BWDOG-04) are implemented and wired:

- **BWDOG-03 + BWDOG-04 (Plan 01):** close_browser() now performs system-wide msedge.exe + msedgewebview2.exe cleanup via two taskkill calls, gated by safe_mode_active before the loop. The safe mode gate causes early return with an info log. Three unit tests confirm behavior in both safe_mode=true and safe_mode=false paths. The cfg(test) early return on count_edge_processes() ensures no real system calls execute during test runs.

- **BWDOG-01 + BWDOG-02 (Plan 02):** A 30-second browser_watchdog_interval fires in the event loop select! block. Each tick: (1) skips if safe mode active, (2) skips if lock screen is Hidden via is_browser_expected(), (3) checks for Edge stacking >5 processes and kills+relaunches if detected, (4) checks is_browser_alive() via try_wait() and relaunches if the child process has exited. All four recovery methods (count_edge_processes, close_browser, launch_browser, is_browser_alive) are wired and called from the tick handler.

The deviation from Plan 02 (using is_browser_expected() instead of is_active()) was a correctness improvement — is_active() returned false for ScreenBlanked state, which would have suppressed the watchdog during blanked-screen sessions where a browser is still expected.

All four commits verified in git log: 291c2a9, 371bf37, 12624c1, e9c42f1.
cargo check passes. 3/3 watchdog unit tests pass.

---

_Verified: 2026-03-22T10:30:00 IST_
_Verifier: Claude (gsd-verifier)_
