---
phase: 04-multiplayer-server-lifecycle
plan: 01
subsystem: rc-core
tags: [multiplayer, ac-server, billing, kiosk, lifecycle]
dependency_graph:
  requires: [ac_server::start_ac_server, ac_server::stop_ac_server, multiplayer::book_multiplayer, billing::end_billing_session, billing::tick_billing]
  provides: [multiplayer::book_multiplayer_kiosk, billing::check_and_stop_multiplayer_server, kiosk_book_multiplayer_endpoint]
  affects: [multiplayer.rs, billing.rs, routes.rs]
tech_stack:
  added: []
  patterns: [idempotent-ddl, fire-and-forget-server-start, all-pods-done-check]
key_files:
  created: []
  modified:
    - crates/rc-core/src/multiplayer.rs
    - crates/rc-core/src/billing.rs
    - crates/rc-core/src/api/routes.rs
decisions:
  - AC server start is fire-and-forget — booking succeeds even if server fails to start
  - ac_session_id column added via idempotent ALTER TABLE (safe for rolling deploy)
  - check_and_stop_multiplayer_server wired at three billing-end paths (tick-expired, manual, orphan)
  - Kiosk multiplayer uses unique PINs per pod (not shared group PIN)
metrics:
  duration: 8min
  completed: 2026-03-15
---

# Phase 4 Plan 01: Multiplayer Server Lifecycle Wiring Summary

Wire existing multiplayer booking to AC server lifecycle: auto-start acServer.exe on booking, auto-stop when all billing ends, self-service kiosk multiplayer endpoint with unique PINs per pod.

## One-liner

Multiplayer booking auto-starts AC server, billing end auto-stops it, kiosk self-service endpoint with per-pod PINs.

## What Changed

### Task 1: Wire book_multiplayer() to AC server auto-start (4fb7655)

**multiplayer.rs changes:**

1. Added `use crate::ac_server` import
2. Idempotent `ALTER TABLE group_sessions ADD COLUMN ac_session_id TEXT` for rolling deploy
3. After `build_group_session_info()` in `book_multiplayer()`, resolve game/track/car from experience or custom payload, then call `ac_server::start_ac_server()` if game is `assetto_corsa`. Store `ac_session_id` on `group_sessions` row for later stop lookup. Fire-and-forget — booking succeeds even if server fails.
4. New `book_multiplayer_kiosk()` function: host pays for all pods, each participant gets a unique PIN via `auth::create_auth_token()`, creates group session with status 'active', sends ShowPinLockScreen to each pod, auto-starts AC server.
5. New types: `KioskMultiplayerResult`, `KioskMultiplayerAssignment` for kiosk response.

### Task 2: Wire billing end to AC server auto-stop + kiosk endpoint (06587f7)

**billing.rs changes:**

1. New `check_and_stop_multiplayer_server()` function: looks up group_session for the pod, checks if any group pod still has active billing timer, if all done calls `stop_ac_server()` and marks group session completed.
2. Wired at three billing-end paths:
   - After tick-expired sessions loop (for each expired pod)
   - After manual `end_billing_session()` clears pod status
   - After orphaned session cleanup path

**routes.rs changes:**

1. New route: `.route("/kiosk/book-multiplayer", post(kiosk_book_multiplayer))`
2. New handler: `kiosk_book_multiplayer()` — extracts driver_id from Bearer token, accepts `pricing_tier_id`, `pod_count`, `experience_id` or `custom`, calls `book_multiplayer_kiosk()`, returns assignments with per-pod PIN + pod_number.

## Decisions Made

1. **Fire-and-forget AC server start**: Booking succeeds even if `start_ac_server()` fails. Server can be started manually. Error is logged but does not fail the booking.
2. **Idempotent DDL**: `ALTER TABLE group_sessions ADD COLUMN ac_session_id TEXT` is safe to run multiple times (SQLite ignores if column exists). Supports rolling deploy.
3. **Three billing-end paths**: The multiplayer check runs at tick-expired, manual stop, AND orphaned session cleanup — covers all ways a billing session can end.
4. **Unique PINs per pod in kiosk mode**: Unlike PWA multiplayer (shared PIN), kiosk multiplayer generates individual auth tokens/PINs per pod since friends aren't pre-registered.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Critical] Added multiplayer check to orphaned session path**
- **Found during:** Task 2
- **Issue:** The plan specified wiring check_and_stop_multiplayer_server at two paths (tick-expired and manual stop), but end_billing_session has a third path for orphaned sessions (sessions in DB but no in-memory timer, after rc-core restart).
- **Fix:** Added check_and_stop_multiplayer_server call in the orphan cleanup path too.
- **Files modified:** billing.rs
- **Commit:** 06587f7

## Verification

- `cargo build -p rc-core` -- compiles clean (no new warnings)
- `cargo test -p rc-core --lib` -- 238 tests pass
- `cargo test -p rc-common` -- 106 tests pass
- `grep start_ac_server multiplayer.rs` -- confirms wiring exists (line 336, 1728)
- `grep check_and_stop_multiplayer_server billing.rs` -- confirmed at 3 call sites + definition
- `grep kiosk/book-multiplayer routes.rs` -- confirmed endpoint registered

## Requirements Addressed

| Requirement | Status | How |
|-------------|--------|-----|
| MULTI-01 | Complete | book_multiplayer() + book_multiplayer_kiosk() call start_ac_server() for AC games |
| MULTI-02 | Complete | check_and_stop_multiplayer_server() at all 3 billing-end paths |
| MULTI-03 | Complete (backend) | POST /kiosk/book-multiplayer endpoint exists; kiosk UI is Plan 04-02 |
| MULTI-04 | Complete (backend) | Each pod gets unique PIN via create_auth_token(), returned in assignments[] |

## Commits

| Task | Commit | Description |
|------|--------|-------------|
| 1 | 4fb7655 | Wire book_multiplayer to AC server auto-start + add kiosk self-service booking |
| 2 | 06587f7 | Wire billing end to AC server auto-stop + add kiosk multiplayer endpoint |
