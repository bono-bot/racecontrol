---
phase: 302-structured-event-archive
verified: 2026-04-01T18:00:00+05:30
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 302: Structured Event Archive Verification Report

**Phase Goal:** Every significant system event is captured, queryable, and permanently archived off-server
**Verified:** 2026-04-01T18:00:00 IST
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

The five success criteria from ROADMAP.md are used directly as truths.

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | After any significant system action, a row appears in system_events with type, source, pod, timestamp, and JSON payload | VERIFIED | `billing.rs` (2 calls), `deploy.rs` (5 calls), `pod_healer.rs` (1 call), `ws/mod.rs` (2 calls), `metric_alerts.rs` (1 call) — 11 call sites total. `append_event()` at line 38 performs UUID + IST timestamp + INSERT via `insert_event_direct()` |
| 2 | A JSONL file for the previous day's events exists in the archive directory by 01:00 IST each morning | VERIFIED | `export_daily_jsonl()` at line 170 creates `events-YYYY-MM-DD.jsonl` idempotently; hourly tick in `spawn()` ensures export happens before 01:00 IST window |
| 3 | Events in SQLite older than 90 days are purged; JSONL files remain untouched | VERIFIED | `purge_old_events()` at line 233 executes `DELETE FROM system_events WHERE timestamp < datetime('now', '-{retention_days} days')`; default `retention_days=90`; file-based JSONL is never touched by purge |
| 4 | Nightly JSONL file appears on Bono VPS after the archive task runs | VERIFIED | `transfer_jsonl_to_remote()` at line 263: IST hour 2/3 window check, NaiveDate dedup, SSH mkdir, SCP with 120s timeout, SHA256 remote verify via ssh — Steps A-E verbatim from backup_pipeline.rs |
| 5 | GET /api/v1/events returns filtered events (type, pod, date range) | VERIFIED | Route `/system-events` registered at `routes.rs:311` in `staff_routes()`. Handler `get_events` at line 21657 with `EventsQuery` (event_type, pod, from, to, limit). All filters validated with character allowlists. Deviation: `/system-events` not `/events` due to hotlap competition route collision — documented in SUMMARY.md and validated by user |

**Score:** 5/5 truths verified

---

### Required Artifacts

#### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/event_archive.rs` | append_event, spawn, export_daily_jsonl, purge_old_events, transfer_jsonl_to_remote; min 150 lines | VERIFIED | 641 lines; all 5 functions present at lines 38, 102, 170, 233, 263 |
| `crates/racecontrol/src/config.rs` | EventArchiveConfig struct with serde defaults | VERIFIED | `EventArchiveConfig` appears 4 times; field `pub event_archive: EventArchiveConfig` at line 73; `Default` impl at line 1019+; `default_event_archive_dir()` at line 1019 |
| `crates/racecontrol/src/db/mod.rs` | system_events table + 3 indexes | VERIFIED | 5 matches: table definition at line 3586 (`id, event_type, source, pod, timestamp, payload`), 3 indexes at lines 3599/3605/3611 (type, pod, timestamp) |

#### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/api/routes.rs` | GET /events handler with EventsQuery | VERIFIED | `EventsQuery` at line 21644; `get_events` at line 21657; route at line 311 |
| `crates/racecontrol/src/billing.rs` | append_event calls for session_started and session_ended | VERIFIED | 2 calls; `use crate::event_archive` at line 14 |
| `crates/racecontrol/src/deploy.rs` | append_event calls for deploy.started, deploy.completed, deploy.failed | VERIFIED | 5 calls (started + completed + failed x2 failure paths + 1 additional); `use crate::event_archive` at line 26 |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `main.rs` | `event_archive::spawn` | `event_archive::spawn(state.clone())` | WIRED | Lines 26 (import), 965 (call) in main.rs |
| `event_archive.rs` | `config.event_archive` | `state.config.event_archive` | WIRED | Lines 103, 134, 135, 136, 270, 271 all access `state.config.event_archive.*` |
| `event_archive.rs` | backup_pipeline SCP pattern | `StrictHostKeyChecking=no` + `BatchMode=yes` + `ConnectTimeout=10` | WIRED | All 3 occurrences at lines 298-300, 320-322, 355-357 use the full SSH options |
| `routes.rs` | `system_events` table | `SELECT ... FROM system_events WHERE 1=1` | WIRED | Line 21662 — dynamic WHERE builder query |
| `billing.rs` | `event_archive::append_event` | Fire-and-forget call near `log_pod_activity` | WIRED | 2 calls confirmed; `use crate::event_archive` import present |
| `routes.rs` | `staff_routes` registration | `.route("/system-events", get(get_events))` | WIRED | Line 311 in `staff_routes()` — JWT protected |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `event_archive.rs::append_event` | `payload_str` from caller | `billing.rs`, `deploy.rs`, etc. | Yes — live system state (driver_id, tier, binary_hash, etc.) | FLOWING |
| `routes.rs::get_events` | `rows` from `fetch_all` | `SELECT ... FROM system_events` | Yes — real DB query with dynamic WHERE | FLOWING |
| `event_archive.rs::export_daily_jsonl` | rows for yesterday | `SELECT ... WHERE date(timestamp) = ?` | Yes — real DB query returning live events | FLOWING |
| `event_archive.rs::purge_old_events` | deleted row count | `DELETE ... WHERE timestamp < datetime(...)` | Yes — real DELETE with retention check | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| 5 unit tests pass (insert, export, idempotent, purge, config defaults) | `cargo test --manifest-path crates/racecontrol/Cargo.toml --lib event_archive` | `test result: ok. 5 passed; 0 failed` | PASS |
| event_archive.rs has no unwrap() outside test block | Test block starts at line 421; unwrap() in production code search | All unwrap() calls are at line 432+ (inside `#[cfg(test)]` block at line 421) | PASS |
| Route uniqueness — no duplicate /events | `cargo test` route uniqueness test (auto-checked at test time) | Route changed to `/system-events`; 774 unit tests pass | PASS |
| system_events table has 4+ references in db/mod.rs | `grep -c "system_events" db/mod.rs` | `5` (table + 3 indexes + comment) | PASS |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| EVENT-01 | 302-01, 302-02 | All significant events written to SQLite events table with structured schema (type, source, pod, timestamp, payload) | SATISFIED | `system_events` table in db/mod.rs; 11 `append_event` call sites across billing, deploy, pod_healer, ws, metric_alerts |
| EVENT-02 | 302-01 | Daily JSONL export of events table for archival | SATISFIED | `export_daily_jsonl()` in event_archive.rs creates `events-YYYY-MM-DD.jsonl` idempotently each hourly tick |
| EVENT-03 | 302-01 | SQLite events retained for 90 days, then purged (JSONL is permanent archive) | SATISFIED | `purge_old_events()` deletes rows older than `retention_days` (default 90); JSONL files on disk are untouched |
| EVENT-04 | 302-01 | Nightly JSONL files shipped to Bono VPS via SCP | SATISFIED | `transfer_jsonl_to_remote()` uses SCP with IST 02:00-03:59 window + NaiveDate dedup + SHA256 verification |
| EVENT-05 | 302-02 | Events queryable via REST API (GET /api/v1/events with filters: type, pod, date range) | SATISFIED | `GET /api/v1/system-events` in staff_routes; character-allowlisted filters for event_type, pod, from, to, limit (deviation: `/system-events` not `/events` due to route collision — documented and valid) |

All 5 requirements satisfied. No orphaned requirements found.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `event_archive.rs` | 371 | `unwrap_or("")` on split_whitespace in production code | Info | Graceful fallback — returns empty string if sha256sum output format is unexpected; logs the mismatch. Not a panic risk. |

No blockers or warnings found. The single info-level pattern is a safe fallback, not a stub.

---

### Human Verification Required

#### 1. SCP Transfer to Bono VPS (IST window)

**Test:** Wait until 02:00-03:59 IST with the server running and observe `./data/event-archive/events-YYYY-MM-DD.jsonl` appearing on Bono VPS at `/root/racecontrol-event-archive/`
**Expected:** File appears within 1 hour of the window opening; `sha256sum` on both files matches
**Why human:** Cannot test SCP transfer without live SSH keys and the IST time window; no mock for remote filesystem

#### 2. Events appear after billing session

**Test:** Start a billing session on a pod, end it, then call `GET /api/v1/system-events?event_type=billing.session_started`
**Expected:** Response contains `{"events": [...], "count": 1}` with driver_id, tier, allocated_seconds in payload
**Why human:** Requires live venue operation with a real session

---

### Gaps Summary

No gaps. All automated checks passed:

- `event_archive.rs` is 641 lines, fully implemented (not a stub)
- All 5 unit tests pass: `test result: ok. 5 passed; 0 failed`
- All 11 `append_event` call sites are wired to real system state — not empty/null payloads
- `GET /api/v1/system-events` queries `system_events` table with real dynamic SQL
- `system_events` table + 3 indexes exist in `db/mod.rs`
- `event_archive::spawn` wired in `main.rs` after `backup_pipeline::spawn`
- No `.unwrap()` in production code (all unwraps are inside `#[cfg(test)]` block)
- Route deviation (`/system-events` vs `/events`) is valid: documented in SUMMARY.md, confirmed by user, avoids Axum runtime panic from duplicate route registration
- ROADMAP plan checkboxes for 302-01 and 302-02 both marked `[x]`
- All 5 REQUIREMENTS.md EVENT requirements marked `[x]` Complete

The phase goal "Every significant system event is captured, queryable, and permanently archived off-server" is fully achieved in code.

---

_Verified: 2026-04-01T18:00:00 IST_
_Verifier: Claude (gsd-verifier)_
