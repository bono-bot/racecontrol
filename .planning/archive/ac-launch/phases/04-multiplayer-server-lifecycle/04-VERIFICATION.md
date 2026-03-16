---
phase: 04-multiplayer-server-lifecycle
verified: 2026-03-15T18:26:59Z
status: passed
score: 10/10 must-haves verified
---

# Phase 4 Verification: Multiplayer Server Lifecycle

**Phase Goal:** When staff or customer books multiplayer, the AC server starts automatically. When billing ends, the server stops. Customers can book multiplayer directly from the kiosk without staff -- friends walk in, pick a game, get PINs, and drive together.

**Verified:** 2026-03-15T18:26:59Z
**Status:** PASSED
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | book_multiplayer() calls start_ac_server() with correct config | VERIFIED | multiplayer.rs:336 -- `ac_server::start_ac_server(state, ac_config, pod_ids.clone(), None)` after group session creation |
| 2 | book_multiplayer_kiosk() calls start_ac_server() for AC games | VERIFIED | multiplayer.rs:1728 -- same pattern in kiosk self-service path |
| 3 | When ALL group pods billing ends, acServer.exe stops | VERIFIED | billing.rs:2467-2532 -- `check_and_stop_multiplayer_server()` checks active_timers for all member pods, calls `stop_ac_server()` when none remain |
| 4 | check_and_stop wired at tick-expired billing path | VERIFIED | billing.rs:1018 -- called in loop over expired_sessions |
| 5 | check_and_stop wired at manual billing stop path | VERIFIED | billing.rs:1928 -- called after pod status cleared in end_billing_session() |
| 6 | check_and_stop wired at orphan cleanup path | VERIFIED | billing.rs:2094 -- called after orphaned session force-ended |
| 7 | POST /kiosk/book-multiplayer endpoint exists | VERIFIED | routes.rs:173 -- `.route("/kiosk/book-multiplayer", post(kiosk_book_multiplayer))` |
| 8 | Each friend gets unique PIN and pod number | VERIFIED | multiplayer.rs:1641 -- `auth::create_auth_token()` per pod in loop, PIN retrieved at line 1654-1660, pod_number at 1663-1666, returned in KioskMultiplayerAssignment |
| 9 | Kiosk UI "Play with Friends" flow with pod count selector | VERIFIED | page.tsx:797 -- multi button, page.tsx:905-937 -- pod count grid (2-8), page.tsx:1195 -- conditional booking handler |
| 10 | Success screen shows per-friend PIN + pod number | VERIFIED | page.tsx:558-615 -- isMulti branch renders card per assignment with Rig number and PIN digits |

**Score:** 10/10 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/multiplayer.rs` | book_multiplayer() calls start_ac_server, book_multiplayer_kiosk() with PINs | VERIFIED | start_ac_server at lines 336 and 1728; KioskMultiplayerResult struct at 1505; KioskMultiplayerAssignment at 1515; book_multiplayer_kiosk at 1527 |
| `crates/racecontrol/src/billing.rs` | check_and_stop_multiplayer_server() at all billing-end paths | VERIFIED | Function defined at 2467; called at 1018 (tick-expired), 1928 (manual stop), 2094 (orphan cleanup) |
| `crates/racecontrol/src/api/routes.rs` | POST /kiosk/book-multiplayer endpoint | VERIFIED | Route at line 173; handler kiosk_book_multiplayer at 5913-5967 with Bearer auth, calls book_multiplayer_kiosk |
| `kiosk/src/lib/types.ts` | KioskMultiplayerAssignment + KioskMultiplayerResult interfaces | VERIFIED | Lines 163-176; fields match Rust structs exactly |
| `kiosk/src/lib/api.ts` | kioskBookMultiplayer() API client | VERIFIED | Lines 277-295; POST to /kiosk/book-multiplayer with Bearer auth, typed return |
| `kiosk/src/app/book/page.tsx` | Multiplayer flow + multi-success screen | VERIFIED | Pod count state (90-92), handleBookMultiplayer (317-354), pod count selector (905-937), conditional review button (1195), multi success screen (558-615) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| multiplayer::book_multiplayer() | ac_server::start_ac_server() | Direct call after group session creation | WIRED | multiplayer.rs:336 -- called with AcLanSessionConfig built from experience or custom |
| multiplayer::book_multiplayer_kiosk() | ac_server::start_ac_server() | Direct call after kiosk group session creation | WIRED | multiplayer.rs:1728 -- same pattern |
| billing tick expired_sessions loop | check_and_stop_multiplayer_server() | Called for each expired pod | WIRED | billing.rs:1016-1018 |
| billing::end_billing_session() | check_and_stop_multiplayer_server() | Called after manual stop clears pod status | WIRED | billing.rs:1928 |
| billing orphan cleanup | check_and_stop_multiplayer_server() | Called after orphaned session force-ended | WIRED | billing.rs:2094 |
| check_and_stop_multiplayer_server() | ac_server::stop_ac_server() | Called when all group pods have no active timer | WIRED | billing.rs:2520 |
| page.tsx handleBookMultiplayer() | api.kioskBookMultiplayer() | API call with Bearer auth | WIRED | page.tsx:340 |
| api.kioskBookMultiplayer() | POST /kiosk/book-multiplayer | HTTP POST with JSON body | WIRED | api.ts:286 -> routes.rs:173 |
| handleSelectPlayerMode("multi") | multiplayer_lobby (pod count step) | Wizard step progression | WIRED | page.tsx:797 -> wizard.goNext() -> step "multiplayer_lobby" at 905 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| MULTI-01 | 04-01 | acServer.exe auto-starts with selected track/car/session config on multiplayer booking | SATISFIED | multiplayer.rs:325-358 -- AcLanSessionConfig built from experience/custom, start_ac_server called for AC games; same at 1717-1748 for kiosk path |
| MULTI-02 | 04-01 | acServer.exe auto-stops within 10s when all billing ends | SATISFIED | billing.rs:2467-2532 -- checks active_timers, calls stop_ac_server when none remain; wired at all 3 billing-end paths |
| MULTI-03 | 04-01 + 04-02 | Customer self-serve "Play with Friends" on kiosk | SATISFIED | Backend: routes.rs:173 + 5913-5967; Frontend: page.tsx pod count selector (905-937), handleBookMultiplayer (317-354), conditional review button (1195) |
| MULTI-04 | 04-01 + 04-02 | Each friend gets unique PIN + pod number | SATISFIED | Backend: multiplayer.rs:1641 create_auth_token per pod, PIN/pod_number in KioskMultiplayerAssignment; Frontend: page.tsx:581-614 renders card per friend with Rig number + PIN digits |

### Success Criteria Verification

| # | Criterion | Status | Evidence |
|---|-----------|--------|----------|
| 1 | Staff books multiplayer -> acServer.exe starts automatically | VERIFIED | multiplayer.rs:336 calls start_ac_server after book_multiplayer creates group session |
| 2 | All pods billing end -> acServer.exe stops within 10s | VERIFIED | billing.rs:2467-2532 check runs on every billing end tick (1s interval), calls stop_ac_server when no timers remain |
| 3 | Customer self-serve multiplayer from kiosk | VERIFIED | Full flow: page.tsx "Play with Friends" (797) -> pod count (905) -> experience -> review -> handleBookMultiplayer (317) -> api.kioskBookMultiplayer -> POST /kiosk/book-multiplayer -> book_multiplayer_kiosk |
| 4 | Each friend sees PIN + pod number | VERIFIED | page.tsx:581-614 renders per-friend card with `a.pod_number` and `a.pin.split("").map(...)` |
| 5 | Single-player flow unchanged | VERIFIED | handleBook() (277-313) untouched, still calls api.customerBook; review button branches on playerMode at 1195; success screen branches on multiAssignments.length at 558 |

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| multiplayer.rs | 1624 | `.bind("0000") // Placeholder` comment | INFO | Not a real placeholder -- shared_pin column needs a value for group_sessions row, but kiosk mode uses per-pod PINs via create_auth_token(). The "0000" is intentional for the DB schema; actual PINs are generated per pod at line 1641. |

No `.unwrap()` calls in multiplayer.rs. No TODO/FIXME in any modified files. No stub implementations found.

### Human Verification Required

### 1. Multiplayer Booking End-to-End

**Test:** On kiosk touchscreen, tap "Play with Friends", select 3 rigs, pick an experience, confirm booking.
**Expected:** Booking succeeds. Screen shows 3 cards, each with a Rig number (e.g., 3, 4, 5) and a unique 4-digit PIN. acServer.exe visible in Task Manager on Racing-Point-Server (.23).
**Why human:** Requires venue hardware (kiosk touchscreen, running racecontrol, connected pods, AC server on .23).

### 2. AC Server Auto-Stop on Billing End

**Test:** With multiplayer session active on 3 pods, let all 3 billing sessions expire (or manually stop all 3).
**Expected:** acServer.exe process terminates on .23 within 10 seconds of last billing end.
**Why human:** Requires active billing sessions and process monitoring on the server.

### 3. Single-Player Regression

**Test:** On kiosk, complete a single-player booking (phone auth -> select plan -> select game -> solo -> experience -> book).
**Expected:** Single pod number + single PIN shown on success screen. No multiplayer UI appears.
**Why human:** Visual regression check on kiosk touchscreen.

### 4. Pod Lock Screen Shows After Kiosk Multiplayer Booking

**Test:** After kiosk multiplayer booking, check each assigned pod.
**Expected:** Each pod shows PIN lock screen with driver name and pricing tier info.
**Why human:** Requires physical pods with rc-agent running.

## Summary

All 10 must-have truths verified against actual codebase. All 4 requirements (MULTI-01 through MULTI-04) satisfied with evidence at specific file:line locations. All key links wired -- no orphaned artifacts, no stubs, no missing connections. The multiplayer server lifecycle is fully integrated: booking triggers start_ac_server, billing end triggers check_and_stop_multiplayer_server at all 3 billing-end paths, kiosk UI provides the complete self-serve flow from pod count selection through PIN display. Single-player flow is cleanly preserved via conditional branching.

Phase 4 goal achieved. Ready to proceed to Phase 5 (Synchronized Group Play).

---

_Verified: 2026-03-15T18:26:59Z_
_Verifier: Claude (gsd-verifier)_
