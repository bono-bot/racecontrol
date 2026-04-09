---
phase: 363-data-recording-verification
plan: 02
subsystem: api, rc-agent, billing, cloud-sync
tags: [csv-fallback, telemetry-fallback, service-key-auth, multipart, retry, gld-c-03]

# Dependency graph
requires:
  - phase: 363-01
    provides: "billing_sessions.csv_fallback_received_at column (written by this plan)"
provides:
  - "POST /api/v1/sessions/{id}/telemetry-fallback — service-key-gated endpoint, 50MB limit"
  - "push_csv_fallback() async fn in rc-agent with exponential retry + read-before-clear"
  - "push_csv_fallback_inner() parameterised inner fn for testability"
  - "SessionEnded handler spawns push_csv_fallback detached (non-blocking)"
  - "reqwest multipart feature enabled in rc-agent Cargo.toml"
affects: [363-03, cloud-sync, billing-fsm, rc-agent-deploy]

# Tech tracking
tech-stack:
  added:
    - "reqwest multipart feature (added to rc-agent Cargo.toml)"
  patterns:
    - "Read-before-clear: buffer CSV into Vec<u8> FIRST, POST, remove_file ONLY on HTTP 200"
    - "push_csv_fallback_inner: parameterised over path+backoffs for testability; production wrapper uses CSV_PATH + PRODUCTION_BACKOFFS"
    - "Service-key auth: inline X-Service-Key check (same pattern as mesh_audit_seed_service)"
    - "Detached spawn: tokio::spawn in SessionEnded, #[cfg(feature = http-client)] gate"
    - "Server URL derivation in ws_handler: config.core.url ws:// → http://, split /ws"
    - "RCAGENT_SERVICE_KEY env var as the outbound service key from rc-agent to server"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs
    - crates/rc-agent/src/csv_lap_fallback.rs
    - crates/rc-agent/src/ws_handler.rs
    - crates/rc-agent/Cargo.toml

key-decisions:
  - "Service key for outbound calls: RCAGENT_SERVICE_KEY env var (same value as sentry_service_key on server)"
  - "Server URL derivation: ws_handler derives http base from config.core.url (no new AppState field needed)"
  - "clear_csv_laps() replaced by inline tokio::fs::remove_file(csv_path) in inner fn — enables testability with tempdir paths"
  - "Route goes in service_routes() alongside mesh_audit_seed_service (both use inline service-key auth)"
  - "backoffs = &[0u64, 0u64] in test_no_clear_on_failure → 3 total attempts with no sleep (fast CI)"

requirements-completed: [GLD-C-03]

# Metrics
duration: 45min
completed: 2026-04-09
---

# Phase 363 Plan 02: CSV Fallback Auto-Sync — Server Endpoint + rc-agent Push Helper Summary

**POST /api/v1/sessions/{id}/telemetry-fallback (service-key-gated, 50MB) + rc-agent push_csv_fallback (read-before-clear, 7-attempt exponential retry) + SessionEnded detached spawn — 9 new tests green, closes GLD-C-03 silent data-loss point**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-04-09 (continuation session)
- **Completed:** 2026-04-09
- **Tasks:** 2 of 2
- **Files modified:** 4

## Accomplishments

- Server endpoint `POST /api/v1/sessions/{id}/telemetry-fallback`: inline service-key gate (sentry_service_key), multipart body, path traversal guard, 50MB body limit via DefaultBodyLimit layer, writes to `C:\RacingPoint\telemetry-fallback\{session_id}.csv`, updates `billing_sessions.csv_fallback_received_at = now()`.
- rc-agent `push_csv_fallback_inner`: reads CSV into memory, POSTs multipart with X-Service-Key, removes file ONLY on HTTP 200 (read-before-clear invariant preserved), retries on non-2xx and network errors.
- Production wrapper `push_csv_fallback`: uses `C:\RacingPoint\laps-offline.csv` + PRODUCTION_BACKOFFS (2,4,8,16,32,64,128s = ~254s envelope, < 10min budget).
- `reqwest multipart` feature added to rc-agent Cargo.toml (was missing; `json` feature alone was insufficient).
- `ws_handler.rs` SessionEnded handler: `tokio::spawn` detached push, `#[cfg(feature = "http-client")]` gated, server URL derived from `config.core.url`, service key from `RCAGENT_SERVICE_KEY`.

## Task Commits

1. **Task 1: Server endpoint POST /api/v1/sessions/{id}/telemetry-fallback** — `09be10e6` (feat)
2. **Task 2: rc-agent push_csv_fallback + SessionEnded integration** — `aadefeb6` (feat)

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` — `telemetry_fallback_handler` function + route in `service_routes()` with DefaultBodyLimit(50MB) + 5 tests in `telemetry_fallback_tests` module
- `crates/rc-agent/src/csv_lap_fallback.rs` — `push_csv_fallback`, `push_csv_fallback_inner`, `PRODUCTION_BACKOFFS` const + 4 tests in `csv_fallback_tests` module
- `crates/rc-agent/src/ws_handler.rs` — SessionEnded handler GLD-C-03 block: detached tokio::spawn with cfg gate
- `crates/rc-agent/Cargo.toml` — `reqwest` multipart feature added

## Decisions Made

- **Service key outbound:** rc-agent uses `RCAGENT_SERVICE_KEY` env var when calling server's telemetry-fallback endpoint. This is the same value as `sentry_service_key` in racecontrol.toml — both are set to the same shared secret at venue setup.
- **clear_csv_laps() replaced:** The sync `clear_csv_laps()` (which uses hardcoded CSV_PATH) was NOT called directly. Instead `push_csv_fallback_inner` calls `tokio::fs::remove_file(csv_path)` — this enables tests to pass a tempdir path without touching production files.
- **Route placement:** `service_routes()` was chosen (NOT `staff_routes()` or `public_routes()`). `service_routes()` already hosts `mesh_audit_seed_service` with the same inline service-key pattern. The endpoint is machine-to-machine (rc-agent → server), not user-facing.
- **`{id}` not `:id`:** Axum 0.8 uses `{id}` capture syntax. Initial `:id` in route caused runtime panic; fixed before first test run.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] `clear_csv_laps()` hardcoded path breaks testability**

- **Found during:** Task 2 test execution (`test_push_on_session_end` failed)
- **Issue:** `push_csv_fallback_inner` called the existing `clear_csv_laps()` which removes `C:\RacingPoint\laps-offline.csv` — not the tempdir test file. Test asserted the tempdir file was cleared → FAIL.
- **Fix:** Replaced `clear_csv_laps()` call with `tokio::fs::remove_file(csv_path)` inline in `push_csv_fallback_inner`. The parameterised `csv_path` is used for both the read AND the delete, preserving the read-before-clear invariant while enabling tempdir-based tests.
- **Files modified:** `crates/rc-agent/src/csv_lap_fallback.rs`
- **Committed in:** `aadefeb6` (Task 2 commit)

**2. [Rule 1 - Bug] Axum 0.8 route syntax: `:id` → `{id}`**

- **Found during:** Task 1 test execution (all 5 tests panicked at runtime)
- **Issue:** Route string `/api/v1/sessions/:id/telemetry-fallback` used v0.7 `:id` capture syntax. Axum 0.8 requires `{id}`. Causes runtime panic: "Path segments must not start with `:`."
- **Fix:** Updated route strings in both `service_routes()` and the test module router to use `{id}`.
- **Files modified:** `crates/racecontrol/src/api/routes.rs`
- **Committed in:** `09be10e6` (Task 1 commit, fixed before commit)

**3. [Rule 2 - Missing Feature] `bytes::Bytes` crate not available**

- **Found during:** Task 1 compile
- **Issue:** Used `bytes::Bytes` in handler but `bytes` is not a direct dependency of `racecontrol`. Changed to `Vec<u8>` (stdlib) — no crate needed.
- **Fix:** Changed `Option<bytes::Bytes>` → `Option<Vec<u8>>`, `.to_vec()` on field bytes.
- **Committed in:** `09be10e6`

---

**Total deviations:** 3 auto-fixed (2 bugs, 1 missing feature). All functional requirements met.

## Known Stubs

None — endpoint writes real CSV data to disk and updates real DB column. No placeholder data.

## MMA Audit Note

The plan mandates an MMA audit before deployment (cross-system bridge: rc-agent → racecontrol HTTP POST). The code is complete and committed. MMA audit must be run by the phase orchestrator before binary deployment to pods + server. See verification section in 363-02-PLAN.md.

## Next Phase Readiness

- Phase 363-03 (billing grace window + F-05 integration tests) can proceed immediately
- Deploy: both `racecontrol` and `rc-agent` binaries must be built and deployed
  - Server (.23): racecontrol binary (creates telemetry-fallback dir automatically on first POST)
  - Pods 1-8: rc-agent binary (push_csv_fallback wired in SessionEnded)
  - Bono VPS (cloud parity): racecontrol binary
- Runtime prerequisites: `RCAGENT_SERVICE_KEY` env var must match `sentry_service_key` in racecontrol.toml

---
*Phase: 363-data-recording-verification*
*Completed: 2026-04-09*
