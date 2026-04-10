## P0 Re-Audit

### P0-1 (broken stub): RESOLVED
Evidence: The fix is architectural — `hydrate_grace_fields_from_db` no longer creates `BillingTimer` instances at all. The `..Default::default()` call is entirely gone. The function only calls `timers.get_mut(pod_id)` and patches two fields onto an existing timer that `recover_active_sessions` already fully populated. The 25+ zeroed fields problem cannot occur because this function never constructs a timer. The test confirms `driving_seconds = 900` and `driver_id = "test-driver"` survive the hydration call unchanged.

---

### P0-2 (double-finalize): RESOLVED
Evidence: Two concrete changes close the race:

1. `deferred_finalizes` tuple expanded from `(String, BillingSessionStatus)` to `(String, String, BillingSessionStatus)` — pod_id is now carried.
2. A new loop at line ~1672 calls `timers.remove(pod_id)` for every deferred-finalize entry **before** `drop(timers)`. This is inside the same write-lock scope that cleared `lap_reject_grace_until` and `pending_end_status`. The next tick cannot see the timer at all — it's gone from the map. The comment "end_billing_session handles missing timers gracefully" is the key assumption (addressed in new issues below).

The original race: tick clears grace fields → drops lock → next tick sees timer with no grace fields → treats as normal active timer. That path is now impossible because the timer is removed before the lock drops.

---

### P0-3 (hydrate/recover ordering): RESOLVED
Evidence: `main.rs` diff is unambiguous:
```
recover_active_sessions(&state).await?;   // line 850 — runs first
hydrate_grace_fields_from_db(...)         // line 856 — runs second
```
The old `hydrate_active_timers_from_db` call (which ran before `recover`) is gone entirely. The new function's logic explicitly checks `timers.get_mut(pod_id)` — it only patches timers that `recover` already inserted. The `recover_active_sessions` diff shows grace fields are still initialized to `None` there, which is now correct because hydrate runs after and overwrites them. The test validates the full sequence: insert timer with `None` grace fields → call hydrate → assert grace fields populated AND other fields preserved.

---

### New issues introduced

**1. Does removing the timer before `end_billing_session` cause it to fail?**

**RISK: LOW but unverified.** The comment says "end_billing_session handles missing timers gracefully" but the diff does not show `end_billing_session`'s implementation. This is a load-bearing assumption. If `end_billing_session` does `active_timers.write().await` → `timers.remove(pod_id)` and then uses the returned `Option` with `.expect()` or `?` propagation that returns early, the DB finalization (UPDATE billing_sessions SET status=...) would be skipped, leaving the session permanently in a non-terminal state in the DB. **This needs to be verified against `end_billing_session`'s source.** The existing `pause_timeout_end` pattern cited in the PR description suggests this pattern is already established, which is reassuring, but the audit cannot confirm without seeing that code.

**2. Does `hydrate_grace_fields_from_db` correctly handle timer-exists vs timer-doesn't-exist?**

**Mostly correct, one edge case.** The `if let Some(grace_until) = grace_until` guard means a row with an unparseable `lap_reject_grace_until` string silently does nothing — no patch, no stale-clear. That row will persist in the DB with a corrupt grace string and confuse every future restart. This should be a `tracing::warn!` + stale-clear path, not silent skip. Low probability but worth noting.

The exists/doesn't-exist branching itself is correct: `timers.get_mut` → patch vs push to `stale_session_ids` → clear DB.

**3. Is the stale-grace-clearing path correct?**

**Structurally correct, one logical gap.** Sessions that `recover` didn't pick up because they have terminal status (e.g., `Completed`, `Cancelled`) with a stale grace column are correctly identified and cleared. However: a session could also be absent from `active_timers` because it has a *non-terminal* status that `recover` failed to process due to a DB error mid-recovery. In that case, clearing the grace column is still safe (the session is broken anyway), but the session itself is now silently orphaned with no timer. This is pre-existing behavior from `recover`'s error handling, not introduced by this fix — just worth noting the stale-clear doesn't distinguish the two cases.

**4. Lock-safety violations?**

**No new violations.** The fix correctly drops the write lock before any `.await` on DB operations:
```rust
{
    let mut timers = billing.active_timers.write().await;
    // ... synchronous work only ...
} // guard dropped here
// DB .await calls happen here
for sid in &stale_session_ids { sqlx::query(...).await }
```
This is the correct pattern. The original `hydrate_active_timers_from_db` held the write lock across `fetch_all(pool).await` — that anti-pattern is gone.

**5. One new minor issue: `pending_end_status` column not persisted (acknowledged as P1)**

The code comment acknowledges this: *"The actual end_status was not persisted (tracked as P1)"*. On restart after a crash during grace window, `pending_end_status` is always reconstructed as `Completed` regardless of what it was. If the original end was `Cancelled` or `Timeout`, the session gets finalized as `Completed`. This is a data-correctness issue but is pre-existing and explicitly tracked. Not introduced by this fix.

---

### Deploy readiness score: 7/10

**Deductions:**
- `-2`: `end_billing_session` behavior with missing timer is unverified in this diff. If it silently returns early, sessions can be permanently stuck non-terminal in DB. This is the highest remaining risk.
- `-1`: Unparseable grace string silently skips both patch and stale-clear paths.

### Ready to ship? NO

**Reason:** The P0s are correctly fixed. The ordering is right, the double-finalize race is closed, and the broken stub is eliminated. However, **the fix's correctness for P0-2 depends entirely on `end_billing_session` tolerating a missing timer**, and that function's source is not in this diff. Before deploying, confirm that `end_billing_session` either (a) does not attempt to remove the timer from `active_timers` and fail on `None`, or (b) handles `None` gracefully and still executes the DB `UPDATE`. If that's confirmed (e.g., by pointing to the `pause_timeout_end` pattern as precedent), the score rises to 9/10 and the answer becomes YES.
