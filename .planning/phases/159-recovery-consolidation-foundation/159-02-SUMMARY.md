---
phase: 159-recovery-consolidation-foundation
plan: 02
subsystem: racecontrol/cascade_guard
tags: [safety, recovery, cascade-protection, whatsapp-alert]
dependency_graph:
  requires:
    - 159-01 (rc-common RecoveryDecision/RecoveryAuthority/RecoveryLogger)
  provides:
    - CascadeGuard struct with record(), is_paused(), resume()
    - AppState.cascade_guard field (Arc<Mutex<CascadeGuard>>)
    - pod_healer cascade guard check + RecoveryDecision logging
  affects:
    - crates/racecontrol/src/cascade_guard.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/pod_healer.rs
    - crates/racecontrol/src/lib.rs
tech_stack:
  added:
    - CascadeAlertConfig (extracted from Config to avoid Clone requirement)
    - tokio::runtime::Handle::try_current() guard (safe spawn in non-async tests)
  patterns:
    - Sliding window ring buffer with Instant-based time injection for tests
    - Arc<Mutex<CascadeGuard>> in AppState (shared mutable state pattern)
    - unwrap_or_else(|e| e.into_inner()) for poisoned mutex recovery
key_files:
  created:
    - crates/racecontrol/src/cascade_guard.rs
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/pod_healer.rs
    - crates/racecontrol/src/lib.rs
decisions:
  - "159-02: CascadeAlertConfig instead of Arc<Config> — Config doesn't implement Clone; extracting only the 4 needed fields avoids requiring Clone on the entire Config struct"
  - "159-02: tokio::runtime::Handle::try_current() guards tokio::spawn in record_at — tests run without a Tokio runtime and would panic on spawn; no-op in tests is correct behavior"
  - "159-02: CascadeGuard stored as Arc<Mutex<>> not Arc<RwLock<>> — record() takes &mut self; write-only semantics, Mutex is simpler and correct"
  - "159-02: heal_all_pods() checks cascade guard once at cycle start + per-action mid-loop — cycle-start check prevents unnecessary pod iteration; per-action check catches cascades triggered within a single cycle"
metrics:
  duration: 38m
  completed_date: "2026-03-22"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 3
---

# Phase 159 Plan 02: Anti-Cascade Guard Summary

**One-liner:** CascadeGuard with 60s sliding window, 3-authority threshold, server-startup exemption, and WhatsApp alert wired into AppState and pod_healer.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Implement CascadeGuard with server-down detection | 4bc02f36 | cascade_guard.rs (created), lib.rs |
| 2 | Wire CascadeGuard into AppState and pod_healer | 55c3ee97 | cascade_guard.rs, state.rs, pod_healer.rs |

## What Was Built

**cascade_guard.rs** — New module implementing `CascadeGuard`:
- 60-second sliding window of `ActionRecord` entries (authority + reason + timestamp)
- `record(&RecoveryDecision) -> bool` — prunes window, adds entry, checks threshold
- Threshold: 3+ distinct `RecoveryAuthority` values in window → pause for 5 minutes
- Server-startup exemption: if ALL window entries have reason containing `"server_startup_recovery"`, cascade check is skipped (8 pods reconnecting after server restart is normal)
- Single-authority burst exemption: 3+ actions from same authority do NOT trigger (coordinated, not cascade)
- `is_paused()`, `resume()`, `pause_remaining()` public API
- `send_cascade_alert()` via Evolution API — best-effort, warns on failure, never panics
- `CascadeAlertConfig` struct to carry only needed config fields (avoids Config needing Clone)
- `record_with_ts()` test-only method for time-travel in unit tests
- 9 unit tests covering all threshold, exemption, window expiry, resume, and return value scenarios

**state.rs** — AppState additions:
- `cascade_guard: Arc<Mutex<CascadeGuard>>` field
- `CascadeAlertConfig::from_config(&config)` extraction before config moves into struct
- Shared `http_client` built once and reused by both `http_client` field and `CascadeGuard`

**pod_healer.rs** — Integration:
- Cycle-level guard check in `heal_all_pods()` — skips entire heal cycle if paused
- Per-action guard check in heal loop — records `RecoveryDecision` before executing each `HealAction`
- Aborts heal loop immediately if cascade is triggered mid-cycle
- Every HealAction logged to `RECOVERY_LOG_SERVER` (recovery-log.jsonl) via `RecoveryLogger`

## Verification Results

- `cargo build --release --bin racecontrol` — clean (pre-existing unused-import warnings only)
- `cargo test -p racecontrol-crate cascade_guard` — 9/9 passed
- `cargo test -p rc-common` — 158/158 passed
- `grep -n "cascade_guard" state.rs` — field declaration confirmed
- `grep -n "cascade_guard" pod_healer.rs` — 2 is_paused() checks, 1 record() call confirmed
- `grep -n "is_paused\|record\|resume" cascade_guard.rs` — all three methods confirmed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] CascadeAlertConfig refactor**
- **Found during:** Task 2 wiring
- **Issue:** Plan specified `Arc<Config>` for CascadeGuard but `Config` doesn't implement `Clone` in the codebase, making it impossible to create `Arc<Config>` without cloning
- **Fix:** Introduced `CascadeAlertConfig` struct carrying only `evolution_url`, `evolution_api_key`, `evolution_instance`, `uday_phone` — extracted from Config before it moves into AppState
- **Files modified:** cascade_guard.rs, state.rs
- **Commit:** 55c3ee97

**2. [Rule 1 - Bug] tokio::spawn panic in non-async tests**
- **Found during:** Task 1 TDD RED phase
- **Issue:** `tokio::spawn` inside `record_at()` panics with "no reactor running" when called from synchronous test context
- **Fix:** `tokio::runtime::Handle::try_current().is_ok()` guard before spawn — no-op in tests, fires normally in production
- **Files modified:** cascade_guard.rs
- **Commit:** 4bc02f36

## Self-Check: PASSED

- cascade_guard.rs: FOUND
- state.rs: FOUND
- pod_healer.rs: FOUND
- Commit 4bc02f36: FOUND
- Commit 55c3ee97: FOUND
