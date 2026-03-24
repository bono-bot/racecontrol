---
phase: 183-recovery-events-api
verified: 2026-03-25T00:00:00+05:30
status: passed
score: 4/4 must-haves verified
re_verification: false
---

# Phase 183: Recovery Events API Verification Report

**Phase Goal:** The racecontrol server exposes a recovery events endpoint so all recovery authorities (rc-sentry, pod_healer, self_monitor) can report attempts and query each other's recent actions -- enabling cross-machine recovery visibility without any pod-to-pod communication
**Verified:** 2026-03-25 IST
**Status:** PASSED
**Re-verification:** No -- initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | POST /api/v1/recovery/events accepts a recovery event JSON and returns 201 | VERIFIED | `post_recovery_event` handler returns `StatusCode::CREATED`; route registered in `public_routes()` at line 108; live curl on .23 returned 201 (commit `54874f5f`) |
| 2 | GET /api/v1/recovery/events?pod_id=pod-8&since_secs=120 returns filtered events | VERIFIED | `get_recovery_events` handler reads `RecoveryEventsQuery{pod_id, since_secs}` and calls `store.query()`; live curl on .23 with `?pod_id=pod-8` returned 1 event; `?pod_id=pod-99` returned `[]` |
| 3 | Ring buffer is capped at 200 events -- oldest are evicted when full | VERIFIED | `MAX_EVENTS: usize = 200`; `push()` calls `pop_front()` when `len >= MAX_EVENTS`; `test_store_eviction_at_cap` pushes 250, asserts `len == 200` and first remaining is `event-50` |
| 4 | POST /api/v1/fleet/alert accepts {pod_id, message, severity} and sends WhatsApp alert, returns 202 | VERIFIED | `post_fleet_alert` handler returns `StatusCode::ACCEPTED`; calls `whatsapp_alerter::send_admin_alert()`; route registered in `public_routes()` at line 111; live curl on .23 returned 202 |

**Score:** 4/4 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/recovery.rs` | RecoveryEvent struct shared between server and pods | VERIFIED | `pub struct RecoveryEvent` at line 245 with all 9 fields: pod_id, process, authority, action, spawn_verified, server_reachable, reason, context, timestamp |
| `crates/racecontrol/src/recovery.rs` | POST + GET handlers and RecoveryEventStore ring buffer | VERIFIED | Created; exports `post_recovery_event`, `get_recovery_events`, `RecoveryEventStore`; 208 lines including 7 unit tests |
| `crates/racecontrol/src/fleet_alert.rs` | POST /api/v1/fleet/alert handler for Tier 4 WhatsApp escalation | VERIFIED | Created; exports `post_fleet_alert`; calls `whatsapp_alerter::send_admin_alert()` at line 44 |
| `crates/racecontrol/src/state.rs` | recovery_events field on AppState | VERIFIED | `pub recovery_events: std::sync::Mutex<RecoveryEventStore>` at line 198; initialized at line 262 |
| `crates/racecontrol/src/api/routes.rs` | Route registration in public_routes | VERIFIED | Lines 107-111: all 3 routes registered inside `public_routes()`, confirmed closed at line 112 |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `api/routes.rs` | `recovery.rs` | route registration in public_routes() | WIRED | Lines 108-109: `post(recovery::post_recovery_event)` and `get(recovery::get_recovery_events)` |
| `api/routes.rs` | `fleet_alert.rs` | route registration in public_routes() | WIRED | Line 111: `post(fleet_alert::post_fleet_alert)` |
| `fleet_alert.rs` | `whatsapp_alerter.rs` | send_admin_alert() call | WIRED | Line 44: `whatsapp_alerter::send_admin_alert(&state.config, &action, &req.message).await` |
| `recovery.rs` | `state.rs` | AppState.recovery_events field access | WIRED | Lines 84, 96: `state.recovery_events.lock()` in both handlers |
| `rc-common/recovery.rs` | `racecontrol/recovery.rs` | RecoveryEvent type import | WIRED | Line 17: `use rc_common::recovery::RecoveryEvent` |
| `state.rs` | `recovery.rs` | RecoveryEventStore import | WIRED | Line 19 of state.rs: `use crate::recovery::RecoveryEventStore` |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| COORD-04 | 183-01-PLAN.md | Server recovery events API (POST + GET with pod_id/since_secs filter) provides cross-machine recovery visibility | SATISFIED | POST /api/v1/recovery/events and GET /api/v1/recovery/events fully implemented, tested, and live on server .23. All 3 endpoints public (no auth). Ring buffer at 200 events. |

**Note -- Requirements file status:** REQUIREMENTS-v17.1.md still marks COORD-04 as `- [ ]` (Pending) at line 21 and `| COORD-04 | Phase 183 | Pending |` at line 76. The implementation is complete; the file was not updated after shipping. This is a documentation-only discrepancy -- the requirement is satisfied in the codebase.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | — | — | — |

Production code paths in both `recovery.rs` and `fleet_alert.rs` contain zero `.unwrap()` calls. Mutex poisoning recovery uses `unwrap_or_else(|poisoned| poisoned.into_inner())` -- correct pattern per standing rules.

---

## Human Verification Required

### 1. WhatsApp delivery confirmation

**Test:** POST `{"pod_id":"pod-8","message":"phase-183 deploy verification test","severity":"info"}` to `http://192.168.31.23:8080/api/v1/fleet/alert`
**Expected:** Uday receives a WhatsApp message containing `[ADMIN] FLEET ALERT [INFO] pod-8 -- phase-183 deploy verification test`
**Why human:** WhatsApp delivery is best-effort fire-and-forget. The server logs a warn and returns 202 regardless of delivery outcome. Confirming the message reached Uday's phone requires a human to check.

*Note:* The SUMMARY documents that the deploy verification test sent a real WhatsApp and Uday received it. If that was confirmed at deploy time, this item is already resolved.

---

## Test Coverage Summary

| Test Suite | Count | Status |
|------------|-------|--------|
| `cargo test -p rc-common -- recovery` | 12 tests | PASS (per SUMMARY) |
| `cargo test -p racecontrol-crate -- recovery` | 21 tests | PASS (per SUMMARY) |
| `cargo test -p racecontrol-crate -- fleet_alert` | 2 tests | PASS (per SUMMARY) |
| `cargo check --bin racecontrol` | — | CLEAN (0 errors, per SUMMARY) |

Tests verified in code against test bodies in `recovery.rs` and `fleet_alert.rs`:
- `test_recovery_event_serde_roundtrip` (rc-common) -- all 9 fields serialized and verified
- `test_store_push_and_len` -- basic push
- `test_store_eviction_at_cap` -- 250 push, assert 200, assert first is event-50
- `test_query_by_pod_id` -- filters by pod id correctly
- `test_query_by_since_secs` -- filters by time window, old event excluded
- `test_query_by_both_filters` -- intersection of pod + time filter
- `test_empty_store_query_returns_empty` -- empty case
- `test_fleet_alert_request_deserializes` -- {pod_id, message, severity} roundtrip
- `test_fleet_alert_request_deserializes_critical` -- severity "critical" case

---

## Commit Trail

| Commit | Description | Verified |
|--------|-------------|---------|
| `838a631e` | feat(183-01): add recovery events API and fleet alert endpoint | EXISTS in git log |
| `54874f5f` | chore(183-01): deploy and verify recovery events API on server .23 | EXISTS in git log |

---

## Overall Assessment

Phase 183 goal is fully achieved. The racecontrol server now exposes:

- **POST /api/v1/recovery/events** -- accepts RecoveryEvent JSON, server-stamps timestamp, stores in ring buffer, returns 201
- **GET /api/v1/recovery/events** -- returns filtered events by pod_id and/or since_secs window
- **POST /api/v1/fleet/alert** -- accepts {pod_id, message, severity}, fires WhatsApp via send_admin_alert, returns 202

All three endpoints are in `public_routes()` (no auth required), consistent with rc-sentry calling from pods without a staff JWT. The ring buffer caps at 200 events with FIFO eviction. All key links are wired. No production `.unwrap()` calls. Deployed and live at server .23 build_id `6345a0c2`.

The only open item is a documentation-only discrepancy: REQUIREMENTS-v17.1.md still shows COORD-04 as `[ ]` Pending. The implementation satisfies the requirement; the checkbox was not ticked.

Downstream phases 184-188 can now POST recovery events and fleet alerts to this API.

---

_Verified: 2026-03-25 IST_
_Verifier: Claude (gsd-verifier)_
