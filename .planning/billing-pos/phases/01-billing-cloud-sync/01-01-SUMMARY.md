# Plan 01-01 Summary: Billing Events Cloud Sync + Extra Columns

**Status:** Complete
**Commit:** abf5f8c
**Duration:** ~8 minutes
**Tests:** 197 pass (2 new), 0 regressions

## What Changed

### crates/racecontrol/src/cloud_sync.rs
- Added `pause_count`, `total_paused_seconds`, `refund_paise` to billing_sessions json_object in `collect_push_payload()`
- Added billing_events collection block after wallet_transactions, using identical pattern: `SELECT json_object(...) FROM billing_events WHERE created_at >= ? ORDER BY created_at ASC LIMIT 500`
- Added `#[cfg(test)] mod tests` with 2 tests:
  - `push_payload_includes_billing_session_extra_columns` — verifies 3 new columns in JSON output
  - `push_payload_includes_billing_events` — verifies billing_events JSON structure

### crates/racecontrol/src/db/mod.rs
- Added `idx_billing_events_created` index on `billing_events(created_at)` for efficient sync queries

### crates/racecontrol/src/api/routes.rs
- Updated billing_sessions INSERT in `sync_push()`: added `pause_count`, `total_paused_seconds`, `refund_paise` to INSERT columns (?24-?26), ON CONFLICT DO UPDATE SET (with COALESCE), and 3 new .bind() calls
- Added billing_events INSERT OR IGNORE handler block before final response, following wallet_transactions pattern exactly

## Requirements Met

| Requirement | Status | How |
|-------------|--------|-----|
| SYNC-01 | Met | billing_sessions already pushed; 3 extra columns now included |
| SYNC-02 | Met | wallet_transactions already pushed (no changes) |
| SYNC-03 | Met | billing_events now pushed via collect_push_payload() + received via INSERT OR IGNORE |
| SYNC-04 | Met | SYNC_TABLES unchanged — billing tables NOT in pull path |
| SYNC-05 | Met | Existing relay + HTTP fallback infrastructure handles reconnect (no changes needed) |

## Key Decisions
- INSERT OR IGNORE for billing_events (immutable lifecycle records — never update)
- COALESCE for 3 new billing_sessions columns in ON CONFLICT (backward-compat with old venue code)
- Tests use in-memory SQLite with inline schema (no AppState construction needed)
