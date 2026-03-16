---
phase: 14-events-and-championships
verified: 2026-03-17T00:00:00Z
status: passed
score: 16/16 must-haves verified
re_verification: false
---

# Phase 14: Events and Championships Verification Report

**Phase Goal:** Staff can create and manage hotlap events and championships. Valid laps auto-enter matching events. Multiplayer group sessions are scored with F1 2010 points. Championship standings are computed with F1 tiebreaker rules. Public read endpoints expose event leaderboards, group results, and championship standings to the PWA.
**Verified:** 2026-03-17
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Staff can create hotlap events with track, car class, dates, reference time | VERIFIED | `create_hotlap_event` in routes.rs line 11868; INSERT INTO hotlap_events line 11921; check_terminal_auth gated |
| 2 | Staff can list, get, and update hotlap events | VERIFIED | `list_staff_events`, `get_staff_event`, `update_hotlap_event` registered at lines 298-299 |
| 3 | Staff can create championships and assign event rounds | VERIFIED | `create_championship`, `add_championship_round`, `link_group_session_to_event` registered at lines 301-304 |
| 4 | Valid laps auto-enter matching events by track+car_class+sim_type+date range | VERIFIED | `auto_enter_event()` called from `persist_lap()` at lap_tracker.rs lines 287-305; query matches active/upcoming events in date range |
| 5 | A faster lap replaces the existing entry; a slower lap is ignored | VERIFIED | `existing_ms <= lap_time_ms` skip logic in auto_enter_event (lap_tracker.rs lines 351-356) |
| 6 | Gold/Silver/Bronze badges computed from reference_time_ms (within 2%/5%/8%) | VERIFIED | Badge logic in lap_tracker.rs lines 359-373; NULL badge when no reference |
| 7 | 107% rule uses integer math: lap_ms * 100 <= leader_ms * 107 | VERIFIED | `recalculate_event_positions()` at lap_tracker.rs line 450; test_107_boundary passes |
| 8 | When staff completes a group session, F1 points scored from multiplayer_results | VERIFIED | `score_group_event()` in lap_tracker.rs lines 493-577; `complete_group_session` calls it at routes.rs line 12381 |
| 9 | DNS/DNF entries receive 0 points regardless of position | VERIFIED | `f1_points_for_position()` returns 0 if dnf=true; test_dns_dnf_zero_points passes |
| 10 | Gap-to-leader is lap_time_ms minus leader lap_time_ms | VERIFIED | Computed inline in score_group_event (lap_tracker.rs lines 526-533) and recalculate_event_positions |
| 11 | Championship standings computed with F1 tiebreaker (wins, P2s, P3s) | VERIFIED | `compute_championship_standings()` + `assign_championship_positions()` in lap_tracker.rs lines 586-686; ORDER BY total_points DESC, wins DESC, p2_count DESC, p3_count DESC |
| 12 | Public event list excludes cancelled, sorted active-first | VERIFIED | `public_events_list` with CASE status ordering (routes.rs lines 9045-9059); test_public_events_list passes |
| 13 | Public event leaderboard with badges, 107% flags, gap, PII-safe names | VERIFIED | `public_event_leaderboard` in routes.rs lines 9093-9198; PII check confirms no email/phone/wallet; test_public_event_leaderboard passes |
| 14 | Public championship standings with per-round breakdown | VERIFIED | `public_championship_standings` in routes.rs lines 9245-9368; BTreeMap per-round grouping present |
| 15 | Public group event sessions with F1 points and gap-to-leader | VERIFIED | `public_event_sessions` in routes.rs lines 9375-9456; f1_points_for_position called inline |
| 16 | Schema migrations: group_sessions.hotlap_event_id and championship_standings.p2_count/p3_count | VERIFIED | db/mod.rs lines 1973-1980; three idempotent ALTER TABLE statements |

**Score:** 16/16 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/db/mod.rs` | 3 ALTER TABLE migrations | VERIFIED | Lines 1973-1980: hotlap_event_id, p2_count, p3_count — all idempotent with `let _ =` |
| `crates/racecontrol/src/lap_tracker.rs` | auto_enter_event(), recalculate_event_positions(), F1_2010_POINTS, score_group_event(), compute_championship_standings(), assign_championship_positions() | VERIFIED | All 6 pub functions present; ~370 lines of implementation; no stubs |
| `crates/racecontrol/src/api/routes.rs` | 9 staff endpoints + 5 public endpoints + scoring handler | VERIFIED | Routes registered at lines 265-305; handlers at lines 8920, 9032, 9093, 9203, 9245, 9375, 11868, 11950, 11999, 12046, 12100, 12158, 12204, 12282, 12339, 12391 |
| `crates/racecontrol/tests/integration.rs` | 19 tests covering all core logic | VERIFIED | 19 Phase 14 tests all passing GREEN (62 total integration tests pass; 0 failures) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `lap_tracker.rs persist_lap()` | `hotlap_event_entries` table | `auto_enter_event()` called at lines 287-305 after lap INSERT | WIRED | Fires only when suspect_flag==0 and car_class is known |
| `lap_tracker.rs auto_enter_event()` | `hotlap_events` table | SELECT WHERE track = ? AND car_class = ? AND sim_type = ? (lines 326-338) | WIRED | Date range filtering via datetime() comparisons |
| `lap_tracker.rs auto_enter_event()` | `hotlap_event_entries` table | ON CONFLICT(event_id, driver_id) UPSERT at lines 379-403 | WIRED | Faster lap wins; recalculate_event_positions called after |
| `routes.rs create_hotlap_event` | `hotlap_events` table | INSERT INTO hotlap_events at line 11921 | WIRED | check_terminal_auth gated |
| `routes.rs create_championship` | `championships` table | INSERT INTO championships at line 12133 | WIRED | check_terminal_auth gated |
| `routes.rs complete_group_session` | `score_group_event()` | `crate::lap_tracker::score_group_event()` at line 12381 | WIRED | Returns error JSON if scoring fails |
| `routes.rs score_group_event()` | `multiplayer_results` table | SELECT driver_id, position, best_lap_ms, dnf (lap_tracker.rs lines 499-508) | WIRED | Results fetched and scored into hotlap_event_entries |
| `routes.rs public_championship_standings` | `compute_championship_standings()` call chain | Live SQL at read time (routes.rs lines 9282-9303) — not delegating to pub fn but duplicates the live-compute pattern | WIRED | Computes standings inline from hotlap_event_entries JOIN championship_rounds |
| `routes.rs public_event_leaderboard` | `hotlap_event_entries` JOIN `drivers` | SELECT with LEFT JOIN drivers at routes.rs lines 9134-9147 | WIRED | PII excluded by construction (no email/phone/wallet in SELECT) |
| `routes.rs public_event_sessions` | `multiplayer_results` JOIN `drivers` | SELECT with LEFT JOIN at routes.rs lines 9398-9406 | WIRED | f1_points_for_position called per row |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| EVT-01 | 14-02 | Staff can create a hotlap event | SATISFIED | `create_hotlap_event` handler; INSERT INTO hotlap_events |
| EVT-02 | 14-01, 14-03 | Laps automatically enter matching hotlap events | SATISFIED | `auto_enter_event()` called from `persist_lap()`; 5 tests green |
| EVT-03 | 14-05 | User can view public event leaderboard | SATISFIED | `GET /public/events/{id}` — `public_event_leaderboard` |
| EVT-04 | 14-05 | Event leaderboard with car class grouping | SATISFIED | Car classes query + `car_classes` field in response |
| EVT-05 | 14-01, 14-03 | 107% rule: slow laps flagged | SATISFIED | Integer math `lap_ms * 100 <= leader_ms * 107`; 2 tests green |
| EVT-06 | 14-01, 14-03 | Gold/Silver/Bronze badges from reference_time_ms | SATISFIED | Ratio thresholds 1.02/1.05/1.08; 4 tests green |
| EVT-07 | 14-05 | User can browse all events | SATISFIED | `GET /public/events` — `public_events_list` with status filter |
| GRP-01 | 14-01, 14-04 | Group sessions auto-scored with F1 points | SATISFIED | `score_group_event()` + `POST /staff/group-sessions/{id}/complete` |
| GRP-02 | 14-04, 14-05 | User can view group event summary | SATISFIED | `GET /public/events/{id}/sessions` with F1 points and gaps |
| GRP-03 | 14-04, 14-05 | User can view per-session breakdowns | SATISFIED | `public_event_sessions` returns per-session multiplayer results |
| GRP-04 | 14-01, 14-04 | Gap-to-leader in group results | SATISFIED | gap_to_leader_ms computed in score_group_event; 1 test green |
| CHP-01 | 14-02 | Staff can create a championship and assign rounds | SATISFIED | `create_championship` + `add_championship_round` endpoints |
| CHP-02 | 14-01, 14-04 | Championship standings from F1 points sum | SATISFIED | `compute_championship_standings()` aggregates across rounds; test green |
| CHP-03 | 14-04, 14-05 | User can view championship standings | SATISFIED | `GET /public/championships/{id}` and `GET /public/championships/{id}/standings` |
| CHP-04 | 14-01, 14-04 | F1 tiebreaker: wins, P2s, P3s | SATISFIED | `assign_championship_positions()` ORDER BY wins DESC, p2_count DESC, p3_count DESC; 2 tests green |
| CHP-05 | 14-01, 14-02 | result_status (finished/DNS/DNF/pending) | SATISFIED | result_status column with CHECK constraint; DNF gets 0 points |

**Note on SYNC-01, SYNC-02, SYNC-03:** These were listed in RESEARCH.md as phase research requirements but were NOT included in the phase prompt requirement list (EVT-01 to CHP-05) and do not appear in any plan's `requirements:` frontmatter. SYNC-01 and SYNC-02 have test scaffolding (tests pass via direct DB queries) but cloud_sync.rs SYNC_TABLES was not extended with competitive tables. This matches the research note that competitive sync is deferred — it is outside the phase scope as stated.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/api/routes.rs` | 12546 | `unused import: Value` | Info | Harmless pre-existing warning, not a phase 14 issue |
| `crates/racecontrol/src/api/routes.rs` | various | Pre-existing unused variable warnings (sid, new_balance, tier_name) | Info | All pre-existing; none from Phase 14 handlers |

No blocker or warning-level anti-patterns found in Phase 14 code. All new handlers use check_terminal_auth() first and avoid .unwrap() on production query paths.

### Human Verification Required

#### 1. Live endpoint smoke test

**Test:** Start the racecontrol server and hit `GET http://localhost:8080/public/events` with curl or browser.
**Expected:** Returns `{ "events": [] }` with HTTP 200 — no server crash, no auth required.
**Why human:** Requires running server; automated tests use in-memory SQLite, not the HTTP layer.

#### 2. Staff auth gate on event endpoints

**Test:** Hit `POST http://localhost:8080/staff/events` without an Authorization header.
**Expected:** Returns `{ "error": "..." }` with access denied — not a 500.
**Why human:** check_terminal_auth behavior with production token config requires running server.

#### 3. Auto-entry fires on real lap receive

**Test:** On a live pod playing AC, complete a timed lap while an active event for that track/car_class is configured.
**Expected:** The driver's entry appears in the event leaderboard within seconds.
**Why human:** End-to-end UDP telemetry → persist_lap → auto_enter_event flow requires real game traffic.

### Gaps Summary

No gaps found. All 16 phase requirements are satisfied by substantive implementations. All 62 integration tests pass with 0 failures. The test suite directly covers all core business logic: auto-entry matching (5 tests), 107% rule (2 tests), badges (4 tests), F1 scoring (3 tests), championship standings and tiebreaker (3 tests), and public endpoint behavior (2 tests). All key wiring points — persist_lap hook, scoring trigger from staff completion endpoint, public read endpoints — are fully connected, not orphaned.

---
_Verified: 2026-03-17_
_Verifier: Claude (gsd-verifier)_
