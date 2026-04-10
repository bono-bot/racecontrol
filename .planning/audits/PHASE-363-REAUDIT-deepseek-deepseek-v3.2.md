## P0 Re-Audit

### P0-1 (broken stub): RESOLVED
**Evidence:** The original `hydrate_active_timers_from_db` function has been completely removed. The new `hydrate_grace_fields_from_db` function no longer creates full `BillingTimer` instances with `..Default::default()`. Instead, it only patches existing timers that were already populated by `recover_active_sessions`. The function signature and implementation confirm it only touches `lap_reject_grace_until` and `pending_end_status` fields.

### P0-2 (double-finalize): RESOLVED
**Evidence:** The fix modifies `deferred_finalizes` to include `pod_id` alongside `session_id` and `end_status`. After identifying expired grace windows, the timer is immediately removed from `active_timers` while still holding the write lock (lines 1670-1673). This prevents the next tick from seeing the timer with cleared grace fields and treating it as a normal active timer. The comment explicitly states: "Remove deferred-finalize timers BEFORE dropping the write lock... prevents the double-finalize race."

### P0-3 (hydrate/recover ordering): RESOLVED
**Evidence:** The function has been renamed to `hydrate_grace_fields_from_db` and is now called **after** `recover_active_sessions` in `main.rs` (lines 850-860). The function only patches grace fields onto timers that already exist in `active_timers`. The original ordering bug where `recover_active_sessions` would clobber grace fields is fixed because `recover_active_sessions` now leaves grace fields as `None`, and the new hydration function runs afterward to restore them from the database.

### New issues introduced
1. **Removing timer before `end_billing_session`**: The timer is removed from `active_timers` before calling `end_billing_session`. However, `end_billing_session` uses the `session_id` to look up the session in the database, not the in-memory timer map. The function already handles missing timers gracefully (as noted in the comment). This is safe.

2. **`hydrate_grace_fields_from_db` timer existence handling**: The function correctly handles both cases:
   - If timer exists in `active_timers`: patches grace fields (lines 2947-2955)
   - If timer doesn't exist: clears stale grace column in DB (lines 2957-2961)
   This matches the intended behavior described in the comments.

3. **Stale-grace-clearing path**: Correct. When a session with `lap_reject_grace_until` is not in `active_timers` (e.g., terminal status rows), the function clears the stale column via an UPDATE query (lines 2973-2984). This prevents confusion on future restarts.

4. **Lock-safety violations**: No new issues found. The function properly:
   - Acquires write lock only for the minimal section (lines 2940-2963)
   - Releases lock before any `.await` (database operations)
   - Uses a separate vector (`stale_session_ids`) to collect work outside the lock

### Deploy readiness score: 9/10
### Ready to ship? YES + reason
All three P0 issues have been properly resolved with clear evidence in the code changes. The fixes address the root causes without introducing new critical bugs. The only minor consideration is that `pending_end_status` is hardcoded to `Completed` during hydration (line 2951), but this is noted as a conservative default and tracked as a P1 issue for future improvement. The system is now restart-safe for grace windows and prevents double-finalization races.
