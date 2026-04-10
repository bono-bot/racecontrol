## P0 Re-Audit

### P0-1 (broken stub): RESOLVED
Evidence: The old bug was that `hydrate_active_timers_from_db()` constructed full `BillingTimer` values using `..Default::default()`, leaving many fields zero/empty. In the diff, that function is replaced by `hydrate_grace_fields_from_db()`, which no longer creates timers at all.

Specific evidence:
- Old function removed/reworked at `billing.rs` around `5884+`.
- New function comment explicitly says it “only patches `lap_reject_grace_until` + `pending_end_status` on timers that are already present in `active_timers`.”
- Implementation uses:
  - `if let Some(timer) = timers.get_mut(pod_id) { ... patch fields ... }`
  - no `BillingTimer { ..Default::default() }`
  - no `timers.insert(pod_id, timer)` on the happy path
- Updated test `test_grace_window_restart_safe()` now pre-populates a recovered timer and verifies fields like `driving_seconds` and `driver_id` are preserved after hydration.

That fully removes the original failure mode.

### P0-2 (double-finalize): RESOLVED
Evidence: The original race was that expired grace timers had grace fields cleared but remained in `active_timers` until after the lock was dropped and `end_billing_session()` ran, allowing a subsequent tick to process them as normal active timers.

Specific evidence from `tick_all_timers`:
- `deferred_finalizes` changed from:
  - `Vec<(String, BillingSessionStatus)>`
  - to `Vec<(String, String, BillingSessionStatus)>` carrying `(pod_id, session_id, end_status)`
- When grace expires, code now pushes `pod_id.clone()` into `deferred_finalizes`.
- Before dropping the write lock:
  ```rust
  for (pod_id, _, _) in &deferred_finalizes {
      timers.remove(pod_id);
  }
  ```
- Comment explicitly states this is the P0-2 fix and explains the race prevention.
- Finalization loop after lock drop now iterates:
  ```rust
  for (_pod_id, sid, end_status) in deferred_finalizes
  ```
  meaning the timer was already removed before async finalization.

This matches the intended fix pattern and closes the original double-finalize window.

### P0-3 (hydrate/recover ordering): RESOLVED
Evidence: The original issue was startup ordering: hydrate ran first, then `recover_active_sessions()` rebuilt timers and reset grace fields to `None`, making restart-safety ineffective.

Specific evidence:
- In `main.rs`, ordering is now:
  1. `recover_active_sessions(&state).await?;`
  2. `hydrate_grace_fields_from_db(&state.billing, &state.db).await`
- Added comment explicitly says:
  - “MUST run AFTER recover”
  - “Original ordering (hydrate-first, recover-second) was broken”
- In `recover_active_sessions`, comments now clarify that grace fields are intentionally left `None` there and patched afterward.
- New hydration function only patches grace fields onto existing timers rather than constructing competing timers.

This directly fixes the clobbering/order bug.

### New issues introduced
1. **Does removing the timer before `end_billing_session` cause `end_billing_session` to fail?**
   - From this diff alone: **no direct evidence of failure**, but this remains a **dependency assumption**.
   - The code comment says:
     > `end_billing_session (called after lock drop) handles missing timers gracefully.`
   - That is supportive, but the diff does not show `end_billing_session` itself. So based strictly on evidence in this patch, I do **not** see a new bug introduced, but this specific claim is not fully proven by the provided diff.
   - Audit verdict for this check: **looks OK, but only as far as the comment/established pattern indicates**.

2. **Does `hydrate_grace_fields_from_db` correctly handle the case where timer exists vs doesn't exist?**
   - **Yes, mostly correctly.**
   - If timer exists:
     - patches `lap_reject_grace_until`
     - sets `pending_end_status = Some(Completed)`
   - If timer does not exist:
     - collects session id into `stale_session_ids`
     - later clears DB column `lap_reject_grace_until = NULL`
   - This is the intended split and avoids recreating broken timers.
   - One limitation remains: if multiple sessions somehow exist for one `pod_id`, `timers.get_mut(pod_id)` patches whichever active timer is keyed there, without validating `timer.session_id == sid`. In the normal invariant of one active timer per pod this is fine; if that invariant is violated, patching could hit the wrong timer. I would not call this a newly introduced P0/P1 from the shown change, but it is a mild robustness gap.

3. **Is the stale-grace-clearing path correct?**
   - **Yes, functionally.**
   - It only selects rows with `lap_reject_grace_until IS NOT NULL`.
   - For rows not represented in `active_timers`, it clears the stale DB column:
     ```rust
     UPDATE billing_sessions SET lap_reject_grace_until = NULL WHERE id = ?
     ```
   - Locking is handled correctly: stale ids are collected under lock, then DB updates occur after the guard is dropped.
   - This matches the stated intent for terminal/non-recovered sessions.

4. **Are there any new lock-safety violations?**
   - **No new lock-safety violations visible in the diff.**
   - Good patterns observed:
     - `hydrate_grace_fields_from_db` does not `await` while holding `active_timers.write()`.
     - stale DB updates happen after the write guard scope ends.
     - `tick_all_timers` still drops `pods` and `timers` before DB/broadcast/finalization work.
   - The deferred-finalize removal loop occurs while already holding the write lock, which is safe.
   - No obvious nested lock/await hazard introduced.

### Deploy readiness score: 8/10
### Ready to ship? YES — all three original P0 blockers appear resolved in this diff. Only remaining caution is that the `end_billing_session handles missing timers gracefully` assumption is asserted by comment/pattern rather than shown in the provided patch.
