---
phase: 10-operational-hardening
plan: 01
subsystem: api, sync
tags: [rate-limiting, tower-governor, cloud-sync, debit-intents, outage-resilience]

# Dependency graph
requires:
  - phase: 03-sync-hardening
    provides: "Bidirectional cloud sync with debit intent lifecycle"
  - phase: 05-kiosk-pin-launch
    provides: "Rate-limited auth endpoints including kiosk redeem-pin"
provides:
  - "Verification audit confirming SYNC-05 and API-05 requirements are met"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns: []

key-files:
  created:
    - pwa/.planning/phases/10-operational-hardening/10-01-SUMMARY.md
  modified: []

key-decisions:
  - "Both SYNC-05 and API-05 confirmed fully implemented with no gaps"

patterns-established: []

requirements-completed: [SYNC-05, API-05]

# Metrics
duration: 3min
completed: 2026-03-22
---

# Phase 10 Plan 01: Operational Hardening Verification Summary

**SYNC-05 and API-05 verified as fully implemented: tower_governor rate limiting on all 7 auth endpoints at 5 req/min per IP, and bidirectional cloud sync with relay(2s)/HTTP(30s) fallback, debit intent lifecycle, and origin-based anti-loop filtering**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-22T03:50:08Z
- **Completed:** 2026-03-22T03:53:00Z
- **Tasks:** 1
- **Files modified:** 1 (this SUMMARY only -- verification/audit task)

## Accomplishments
- Verified API-05: all 7 auth endpoints behind tower_governor rate limiting (5 req/min per IP)
- Verified SYNC-05: outage-resilient bidirectional sync with debit intent lifecycle
- All existing tests pass (5 rate_limit unit tests, 3 sync integration tests)

## API-05 Verification: Auth Rate Limiting

| # | Check | Verdict | Reference |
|---|-------|---------|-----------|
| 1 | `auth_rate_limit_layer()` creates GovernorLayer with `per_second(12)` + `burst_size(5)` = 5 req/60s per IP | **PASS** | `crates/racecontrol/src/auth/rate_limit.rs:14-18` |
| 2a | `/customer/login` behind rate limit | **PASS** | `crates/racecontrol/src/api/routes.rs:55` |
| 2b | `/customer/verify-otp` behind rate limit | **PASS** | `crates/racecontrol/src/api/routes.rs:56` |
| 2c | `/kiosk/redeem-pin` behind rate limit | **PASS** | `crates/racecontrol/src/api/routes.rs:59` |
| 2d | `/auth/validate-pin` behind rate limit | **PASS** | `crates/racecontrol/src/api/routes.rs:57` |
| 2e | `/auth/kiosk/validate-pin` behind rate limit | **PASS** | `crates/racecontrol/src/api/routes.rs:58` |
| 2f | `/staff/validate-pin` behind rate limit | **PASS** | `crates/racecontrol/src/api/routes.rs:60` |
| 2g | `/auth/admin-login` behind rate limit | **PASS** | `crates/racecontrol/src/api/routes.rs:61` |
| 3 | `PeerIpKeyExtractor` used (per-IP, not global) | **PASS** | `crates/racecontrol/src/auth/rate_limit.rs:3,13` |
| 4 | Unit tests pass: first-5-succeed, 6th-returns-429, different-IPs-separate | **PASS** | 5 tests passed (`rate_limit_first_five_requests_succeed`, `rate_limit_sixth_rapid_request_returns_429`, `rate_limit_different_ips_have_separate_limits` + 2 lib tests) |

**API-05 Overall Verdict: PASS** -- All 7 authentication endpoints are protected by tower_governor rate limiting at 5 requests per 60 seconds per IP address.

## SYNC-05 Verification: Split-Brain/Outage Handling

| # | Check | Verdict | Reference |
|---|-------|---------|-----------|
| 1 | `SYNC_TABLES` includes "reservations" and "debit_intents" | **PASS** | `crates/racecontrol/src/cloud_sync.rs:23` -- full value: `"drivers,wallets,pricing_tiers,pricing_rules,billing_rates,kiosk_experiences,kiosk_settings,auth_tokens,reservations,debit_intents"` |
| 2 | Sync loop has relay (2s) + HTTP fallback (30s) dual-mode | **PASS** | `crates/racecontrol/src/cloud_sync.rs:26` (`RELAY_INTERVAL_SECS: 2`), line 131 (`fallback_interval_secs`), lines 212-224 (mode selection with hysteresis) |
| 3 | `process_debit_intents()` processes pending -> completed/failed after sync pull | **PASS** | `crates/racecontrol/src/cloud_sync.rs:291-381` -- queries `WHERE status = 'pending'`, debits wallet, marks intent `completed` or `failed`, updates reservation status |
| 4 | `sync_push` has origin filtering to prevent loops | **PASS** | `crates/racecontrol/src/api/routes.rs:7855-7861` -- compares `incoming_origin` with `my_origin`, rejects same-origin pushes with `"reason": "same_origin"` |
| 5 | Outage scenario trace: cloud reservation -> pending_debit -> outage -> sync paused -> connectivity returns -> sync resumes -> process_debit_intents fires -> debit completes -> status syncs back | **PASS** | See trace below |
| 6 | Sync integration tests pass | **PASS** | 3 tests passed: `test_sync_targeted_telemetry`, `test_wallet_transaction_sync_payload`, `test_sync_competitive_tables` |

### Outage Scenario Trace (Check 5)

1. **Cloud creates reservation** with `pending_debit` status -- reservation and debit_intent rows created on cloud
2. **Sync pull** brings `reservations` and `debit_intents` tables to local (both in `SYNC_TABLES`, line 23)
3. **During outage:** relay check fails (`is_relay_available` returns false, line 84-106), hysteresis counter increments (lines 186-190), after 3 consecutive failures effective_relay_up transitions to false (line 193-194)
4. **Fallback engages:** HTTP fallback runs every 30s (lines 219-224), maintaining sync at reduced frequency
5. **If fully offline:** sync_once_http fails, logged as error (line 221), no data lost -- local DB unchanged
6. **Connectivity returns:** relay health check succeeds, after 2 consecutive successes (RELAY_UP_THRESHOLD=2, line 32) hysteresis transitions to relay mode (line 195-196)
7. **process_debit_intents fires** after sync pull (line 833-836): queries pending intents, debits wallet, marks intent `completed`, updates reservation to `confirmed`
8. **Next sync push** pushes updated debit_intent status and reservation status back to cloud via `collect_push_payload` (origin tagged, line 392-395)
9. **Anti-loop:** Cloud's sync_push handler rejects data with matching origin_id (routes.rs:7855-7861)

**SYNC-05 Overall Verdict: PASS** -- Cloud bookings survive as pending_debit during outage and resolve automatically when connectivity returns. Sync fallback from relay (2s) to HTTP (30s) is automatic with hysteresis-based mode switching.

## Test Results

```
# Rate limit tests (5 passed)
cargo test -p racecontrol-crate -- rate_limit
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 405 filtered out

# Sync tests (3 passed)
cargo test -p racecontrol-crate -- sync
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 63 filtered out
```

## Task Commits

1. **Task 1: Verify API-05 and SYNC-05 requirements against existing code** - (this is a verification-only task; commit contains this SUMMARY)

## Files Created/Modified
- `pwa/.planning/phases/10-operational-hardening/10-01-SUMMARY.md` - This verification audit document

## Decisions Made
- Both SYNC-05 and API-05 confirmed fully implemented with no gaps -- no code changes required

## Deviations from Plan
None - plan executed exactly as written.

## Issues Encountered
- Package name is `racecontrol-crate` (not `racecontrol`) in Cargo.toml -- corrected test command accordingly
- `--no-fail-fast` flag is not recognized by the test binary (it's a cargo flag, not a test binary flag when passed after `--`) -- dropped it, tests passed without it

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 10 (Operational Hardening) complete -- this was the final verification plan
- v1.0 milestone fully verified: all requirements confirmed implemented and tested

---
*Phase: 10-operational-hardening*
*Completed: 2026-03-22*
