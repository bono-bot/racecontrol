---
phase: 185-pod-healer-wol-coordination
plan: "01"
subsystem: racecontrol/pod_healer
tags: [coordination, recovery, pod-healer, process-ownership, graceful-relaunch]
dependency_graph:
  requires: [183-01]
  provides: [COORD-01, COORD-02, COORD-03]
  affects: [crates/racecontrol/src/pod_healer.rs, crates/racecontrol/src/state.rs, crates/racecontrol/src/recovery.rs, crates/rc-common/src/recovery.rs]
tech_stack:
  added: [RecoveryIntent, RecoveryIntentStore]
  patterns: [process-ownership-registry, intent-ttl-deconfliction, sentinel-file-check-via-http]
key_files:
  created: []
  modified:
    - crates/rc-common/src/recovery.rs
    - crates/racecontrol/src/recovery.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/pod_healer.rs
decisions:
  - "185-01: rc-agent.exe registered to RcSentry (not PodHealer) at startup — PodHealer must skip Tier 1 process restart and jump to AiEscalation when ownership mismatch found"
  - "185-01: GRACEFUL_RELAUNCH check via rc-sentry /files HTTP endpoint (port 8091) — tolerates unreachability (proceed on error, skip only on success)"
  - "185-01: RecoveryIntent TTL is 2 minutes (120 seconds); cleanup_expired called on every register to prevent unbounded growth"
  - "185-01: pod_needs_restart flag in heal_pod also guarded by ownership check — prevents race between heal_pod and run_graduated_recovery"
metrics:
  duration_minutes: 35
  completed_date: "2026-03-25"
  tasks_completed: 2
  files_modified: 4
---

# Phase 185 Plan 01: Pod Healer WoL Coordination Summary

**One-liner:** ProcessOwnership registry + RecoveryIntent 2-min TTL + GRACEFUL_RELAUNCH sentinel check wired into pod_healer to prevent concurrent restart loops between rc-sentry, pod_healer, and rc-watchdog.

## What Was Built

### Task 1: RecoveryIntent + RecoveryIntentStore data types and AppState fields

Added `RecoveryIntent` struct to `rc-common/src/recovery.rs`:
- Fields: `pod_id`, `process`, `authority`, `reason`, `created_at` (UTC)
- `is_expired()` returns true when older than 120 seconds (2-min TTL)
- Serializable (serde Serialize/Deserialize)

Added `RecoveryIntentStore` to `crates/racecontrol/src/recovery.rs`:
- `register()` — adds intent, auto-cleans expired entries first
- `has_active_intent(pod_id, process)` — returns first active (non-expired) match
- `cleanup_expired()` — removes expired intents
- `active_len()` — count of non-expired intents

Updated `AppState` in `state.rs`:
- Added `process_ownership: std::sync::Mutex<ProcessOwnership>` — initialized with `rc-agent.exe -> RcSentry` at startup
- Added `recovery_intents: std::sync::Mutex<RecoveryIntentStore>` — empty at startup
- Both use `unwrap_or_else(|e| e.into_inner())` poison recovery (no `.unwrap()` in production per standing rules)

### Task 2: Coordination gates in pod_healer

**Gate B — COORD-02 (RecoveryIntent TTL check):**
Inserted in `run_graduated_recovery` after the billing gate and before the tracker lookup. If any authority has an active intent for `pod.id + rc-agent.exe`, pod_healer logs remaining TTL and returns without acting.

**Gate C — COORD-03 (GRACEFUL_RELAUNCH sentinel):**
Inserted after Gate B. Queries `http://<pod_ip>:8091/files?path=C%3A%5CRacingPoint%5CGRACEFUL_RELAUNCH` with a 3-second timeout. If the sentinel file exists (rc-sentry returns 200), logs and returns — rc-agent is doing a planned self-restart. Unreachable rc-sentry = proceed (fail-open).

**Gate A — COORD-01 (ProcessOwnership check at TierOneRestart):**
Added inside `PodRecoveryStep::TierOneRestart` before the cascade guard check. If `owner_of("rc-agent.exe")` returns a non-PodHealer authority (e.g. RcSentry), skips Tier 1 restart and advances to `AiEscalation` instead.

**Intent registration:**
After the cascade guard check in `TierOneRestart`, PodHealer registers its own `RecoveryIntent` before attempting the restart — blocks concurrent action by other authorities for 2 minutes.

**Ownership gate in heal_pod:**
The `pod_needs_restart` flag in `heal_pod` (Rule 2 path) is now guarded: only set if `owner_of("rc-agent.exe")` is PodHealer or unregistered. Prevents `heal_pod` from flagging restarts when rc-sentry owns the process.

## Tests Added

| File | Test | Validates |
|------|------|-----------|
| `rc-common/src/recovery.rs` | `test_recovery_intent_not_expired_when_fresh` | Fresh intent is not expired |
| `rc-common/src/recovery.rs` | `test_recovery_intent_fields` | Intent fields populated correctly |
| `racecontrol/src/recovery.rs` | `test_intent_store_register_and_query` | Registered intent is retrievable |
| `racecontrol/src/recovery.rs` | `test_intent_store_expired_not_returned` | Expired intent is invisible |
| `racecontrol/src/recovery.rs` | `test_intent_store_different_pod_not_returned` | Pod scoping is correct |
| `racecontrol/src/recovery.rs` | `test_intent_store_cleanup_removes_expired` | Auto-cleanup on register |
| `racecontrol/src/pod_healer.rs` | `test_ownership_check_skips_when_not_owner` | COORD-01 logic |
| `racecontrol/src/pod_healer.rs` | `test_recovery_intent_prevents_concurrent_action` | COORD-02 logic |

## Verification Results

```
cargo test -p rc-common                     -> 171 passed, 0 failed
cargo test (racecontrol pod_healer)         -> 27 passed, 0 failed
cargo test (racecontrol all)                -> 467 passed (lib + integration), 0 failed
cargo build --release --bin racecontrol     -> Finished, 0 errors (3 pre-existing warnings)
```

## Deviations from Plan

None — plan executed exactly as written.

The plan's note about "ProcessOwnership check for run_graduated_recovery" vs "TierOneRestart specifically" was followed: Gate A is placed inside the `TierOneRestart` match arm (not at the top of run_graduated_recovery), because WoL is a machine-level action that doesn't need the ownership check — only process-level restart via pod-agent does.

## Self-Check

### Created files exist
- `.planning/phases/185-pod-healer-wol-coordination/185-01-SUMMARY.md` — this file

### Commits exist
- `faa8f37d` — feat(185-01): add RecoveryIntent+RecoveryIntentStore and wire into AppState
- `99f2102d` — feat(185-01): wire COORD-01/02/03 coordination gates into pod_healer

## Self-Check: PASSED
