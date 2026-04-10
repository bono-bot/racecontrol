# Phase 343-02 Resume Instructions

**Left off:** Plan read complete, no code written yet.
**343-01 shipped:** `b31c38e0` — cloud-authority 409 guard on 4 staff mutation endpoints. NOT deployed (git only).

## What 343-02 does

Post-write verification for staff PIN mutations. Two layers:

1. **Immediate verify** — after UPDATE, re-read the row. If PIN doesn't match what we just wrote, return 500 instead of 200. Adds `verified: true` to success responses.

2. **Delayed verify** — `tokio::spawn` a task that sleeps `sync_interval_secs + 5` seconds, then re-reads. If PIN was silently reverted by cloud sync, INSERT into `alert_incidents` table (P1 severity). This catches the exact Vishal incident failure mode.

## Files to modify

- `crates/racecontrol/src/api/routes.rs` — add `post_write_verify_staff_pin()` helper + `spawn_delayed_sync_verify()` + wire into `update_staff` and `reset_staff_pin` after their UPDATE queries
- `crates/racecontrol/src/db/mod.rs` — add `CREATE TABLE IF NOT EXISTS alert_incidents` migration (id TEXT PK, severity TEXT, source TEXT, message TEXT, metadata TEXT, created_at TEXT)

## Key code from the plan (343-02-PLAN.md lines 52-145)

- `post_write_verify_staff_pin(state, staff_id, expected_pin)` — returns `Result<(), String>`
- `spawn_delayed_sync_verify(state, staff_id, expected_pin, correlation_id)` — fire-and-forget tokio::spawn with sleep
- Integration point in `update_staff`: after the `Ok(_)` match arm (line ~13019), check if `req.pin` was set, generate correlation_id, call immediate verify, spawn delayed verify
- Integration point in `reset_staff_pin`: after the UPDATE query succeeds (line ~13093), same pattern with the `new_pin` variable

## Watch out for

- `sync_interval_secs` is `u64` on `CloudConfig`, accessed via `state.config.cloud.sync_interval_secs` (NOT `Option` — has a default of 30)
- The 4 staff functions now return `impl IntoResponse` (changed in 343-01). All new returns must use `.into_response()`.
- `alert_incidents` table may not exist yet — add the CREATE TABLE in the migration chain in `db/mod.rs` (after the Phase 363 migration block around line 4021)

## After 343-02, do 343-04

343-04 is an e2e test spec in `e2e-regression/`. Read `343-04-PLAN.md` for details. It's a Playwright-style test that exercises the full staff-pin-lifecycle: create → verify PIN works → change PIN on cloud → verify venue gets the new PIN after sync → verify old PIN fails.

## Then: binary rebuild + deploy

After 343-01 + 343-02 + 343-04 are all committed, rebuild racecontrol binary and deploy to server .23 + cloud Bono VPS. This unblocks v47.0 Phase 347 (Admin Staff Management UI).
