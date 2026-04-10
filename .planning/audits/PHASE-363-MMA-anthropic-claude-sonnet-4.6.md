## Phase 363-03 MMA Audit — Findings

### P0 blockers (must fix before deploy)

**1. [concurrency] billing.rs:1440-1480 — Double-finalize race: two tick iterations can both see the same expired grace timer**

- **why:** The grace expiry check reads `timer.lap_reject_grace_until` and `timer.pending_end_status`, clears them in-memory, then calls `continue` — but `end_billing_session` runs *after* the lock is dropped. If the tick loop fires again (1s cadence) before `end_billing_session` completes and removes the timer from `active_timers`, the *next* tick iteration will see the timer still present (with cleared grace fields, so it falls through to normal tick processing). However, the more dangerous race is: `end_billing_session` itself re-acquires `active_timers.write()` to remove the timer. Between the grace-clear (`timer.lap_reject_grace_until = None`) and `end_billing_session` completing, a *concurrent* tick iteration acquires the write lock, sees the timer with both grace fields `None` and `driving_seconds` potentially at/past `allocated_seconds`, and pushes it into `expired_sessions` *again* — triggering a second finalize path (the non-deferred one, which sets a new grace window). This creates a loop: grace → clear → normal expiry → new grace → clear → ...
- **fix:** Set a sentinel status (e.g., `BillingSessionStatus::Finalizing`) on the timer when collecting `deferred_finalizes`, so subsequent ticks skip it. Or: remove the timer from `active_timers` *before* dropping the lock (inside the loop), then call `end_billing_session` with the extracted data. The latter is cleaner and matches how `expired_sessions` should work.
- **confidence:** High — reproducible whenever tick period (1s) < finalize latency (DB round-trip).

---

**2. [restart-safety] billing.rs:5900 — hydrate_active_timers_from_db races against recover_active_sessions and incoming WS messages**

- **why:** `main.rs` calls `hydrate_active_timers_from_db` at line ~769. `recover_active_sessions` is called earlier (line ~750, before the diff's insertion point — confirmed by the existing Phase 177 comment ordering). Both write to `active_timers`. If `recover_active_sessions` already inserted timers for active sessions, `hydrate_active_timers_from_db` will overwrite them with *less complete* timers (missing `rate_paise_per_minute`, `wallet_owner_id`, `driver_name`, `low_balance_warning_paise`, etc. — all defaulted to 0/""). The hydrated timer has `allocated_seconds` from DB but everything else is `Default::default()`, meaning `rate_paise_per_minute = 0`. When the deferred finalize runs `compute_refund(allocated, driving, wallet_debit_paise)`, the rate is correct (it comes from `wallet_debit_paise`), but `debit_wallet` calls in the tick loop will charge **0 paise/minute** for any session that was hydrated and is still running.
- **fix:** Either (a) call `hydrate_active_timers_from_db` *before* `recover_active_sessions` and have `recover_active_sessions` skip sessions already in `active_timers`, or (b) merge hydration into `recover_active_sessions` so grace fields are populated there, or (c) in `hydrate_active_timers_from_db`, only insert timers that are NOT already present in `active_timers` (check before insert). Option (c) is the minimal safe fix.
- **confidence:** High — the ordering in `main.rs` makes this deterministic on every restart where active sessions exist.

---

**3. [restart-safety] billing.rs:5900 — hydrate_active_timers_from_db inserts timer keyed by pod_id but pod_id may be stale/reused**

- **why:** The hydration query returns `pod_id` from `billing_sessions` and inserts into `active_timers` keyed by `pod_id`. If a pod was reassigned between crash and restart (e.g., pod `p-01` now has a *different* session), the hydrated timer for the old session silently overwrites whatever `recover_active_sessions` built for the new session. There is no check that the `pod_id` in `billing_sessions` matches the current pod state.
- **fix:** After hydration, cross-check against `state.pods` (or the result of `recover_active_sessions`) and drop any hydrated timer whose `session_id` doesn't match the pod's current active session. Log a warning for each dropped timer.
- **confidence:** High — pod reassignment is a normal operational event.

---

### P1 important (fix in phase)

**1. [restart-safety] billing.rs:5880 — pending_end_status always defaults to Completed; cancellation sessions silently become Completed**

- **why:** The comment says "conservative default: Completed." But if the session was being *cancelled* (operator force-end, balance exhaustion, etc.) and the server crashed during the grace window setup, the session will be finalized as `Completed` instead of `Cancelled`/`EndedEarly`. This means the customer gets a refund computed against `wallet_debit_paise` (correct amount) but the `end_reason` and `status` in `billing_sessions` will say `completed` instead of `cancelled` — breaking downstream reporting, dispute resolution, and any FSM guards that check terminal status.
- **fix:** Persist `pending_end_status` to `billing_sessions` as a separate column (e.g., `pending_end_status TEXT`), hydrate it on restart. If the column is NULL and grace is set, *then* default to Completed.
- **confidence:** High — the grace window is currently only triggered on normal expiry (hardcoded `Completed` at line ~1648), so this is a latent bug for future paths, but it's the wrong architecture to ship.

**2. [FSM invariant] billing.rs:1629-1660 — cancel/force-end arriving during grace window is silently ignored**

- **why:** When a timer is in grace window, the tick loop hits `continue` for *both* the expired-grace branch and the still-active-grace branch. This means the timer receives no normal tick processing. But more critically: if `end_billing_session` is called directly (cancel path, balance exhaustion path) while the timer is in grace window, it will attempt to finalize a session that is already "pending finalize." The grace window fields are still set in-memory. When the deferred finalize fires on the next tick, it will call `end_billing_session` again on a session that may already be in a terminal state. The CAS UPDATE in `end_billing_session` should protect against double-finalize at the DB level, but the `end_billing_session` function returning `false` is only logged as an error — no cleanup of the in-memory timer occurs.
- **fix:** In `end_billing_session`, after a CAS miss (returns false), ensure the timer is removed from `active_timers`. Also: when cancel/force-end is called, clear `lap_reject_grace_until` and `pending_end_status` before proceeding, and clear the DB column.
- **confidence:** High.

**3. [concurrency] billing.rs:5832 — record_lap_rejection grace_window_caught check has TOCTOU gap**

- **why:** The function reads `lap_reject_grace_until` under a read lock, drops the lock, then inserts into DB. Between the read and the insert, the grace window can expire and `end_billing_session` can run. The `grace_window_caught=true` record is inserted *after* finalize has already run — meaning the refund was computed without this lap rejection's data, but the record claims it was "caught." This is a data integrity issue for auditing.
- **fix:** The check and insert should be atomic with respect to the grace window. One approach: do the insert first with `grace_window_caught = (grace_until IS NOT NULL AND grace_until > datetime('now'))` computed in SQL, eliminating the Rust-side TOCTOU. Alternatively, accept the race but document it clearly.
- **confidence:** Medium-high — 5s window makes this unlikely but not impossible under load.

**4. [F-05 regression test] billing.rs:8800 — test_end_billing_session_early_end_refund_amount tests a COPY of the UPDATE, not the actual production UPDATE**

- **why:** The test hardcodes the UPDATE SQL string. If the production CAS UPDATE in `end_billing_session` is refactored (column added, WHERE clause changed, etc.) for unrelated reasons, this test continues to pass while the production code may have regressed. The test is testing a *replica* of the query, not the actual query. This is the "rot" scenario called out in the audit brief.
- **fix:** Extract the CAS UPDATE SQL into a named constant (`const CAS_UPDATE_SQL: &str = "..."`) shared between production code and the test. The test should bind to that constant. This way, any change to the production UPDATE is immediately reflected in the test.
- **confidence:** High — this is a structural weakness, not a timing issue.

**5. [schema] billing.rs:5832 — lap_rejections has no FK constraint on session_id; no migration shown**

- **why:** The `CREATE TABLE` in tests has no `FOREIGN KEY (session_id) REFERENCES billing_sessions(id)`. A lap rejection for a non-existent `billing_session_id` silently inserts. More importantly: the diff shows no migration file for adding `lap_reject_grace_until` to `billing_sessions` or creating `lap_rejections`. If these columns/tables don't exist in production schema, the server will crash on first use.
- **fix:** Confirm migration files exist (they may be outside this diff). If not, add them. Add FK constraint (with `PRAGMA foreign_keys = ON` in connection setup, or at minimum document the omission).
- **confidence:** High on the migration concern; medium on FK (SQLite FK enforcement is opt-in).

---

### P2 minor (can defer)

**1. [restart-safety] billing.rs:5870 — telemetry_seconds_covered=HashSet::new() on hydration causes suspect=true for hydrated sessions**

- **why:** Per D-05, empty coverage set → NULL `telemetry_coverage_pct` → session flagged UNVERIFIED. For sessions that were running normally before restart and had good coverage, they will now show as UNVERIFIED after restart. This is documented as intentional ("D-05 — coverage bucket lost on crash") but it means *every* restart degrades session quality flags for all active sessions. In a production kart track, a server restart mid-session (deploy, crash) will mark all running sessions as suspect.
- **fix:** Persist coverage histogram to DB periodically (separate concern, larger change). For now, document the customer-visible impact: "sessions active during restart will show UNVERIFIED coverage."
- **confidence:** High on behavior, P2 because it's pre-existing and documented.

**2. [billing FSM] billing.rs:1648 — grace window hardcodes Completed; EndedEarly sessions get wrong terminal status**

- **why:** The expiry path that triggers the grace window sets `pending_end_status = Some(BillingSessionStatus::Completed)`. But `tick_all_timers` can also trigger session end for reasons other than time expiry (balance exhaustion → `sessions_to_auto_end`). If `sessions_to_auto_end` is processed and calls `end_billing_session` with `EndedEarly`, but the timer also has an expiry-triggered grace window, the status conflict is unresolved. Currently the grace window only fires on time expiry (the `expired` branch), so `EndedEarly` from balance exhaustion bypasses grace entirely — but this is fragile and undocumented.
- **fix:** Add a comment explicitly stating grace window is only for time-expiry, not balance-exhaustion. Add an assertion or FSM guard.
- **confidence:** Medium.

**3. [observability] billing.rs:1663-1695 — grace window DB persist is fire-and-forget with `let _ =`**

- **why:** If the `UPDATE billing_sessions SET lap_reject_grace_until = ?` fails (DB locked, disk full), the grace deadline is in memory but not persisted. On restart, the session will be hydrated without a grace window and will be treated as a normal active session — the deferred finalize will never fire. The error is silently swallowed.
- **fix:** Log the error explicitly: `if let Err(e) = ... { tracing::error!(...) }`. Consider whether a failed persist should abort the grace window (safer) or proceed (current behavior).
- **confidence:** High on the silent failure; P2 because DB failures are rare.

**4. [latency] 6s worst-case additional billing latency**

- **why:** 5s grace + up to 1s tick delay = 6s after session time expires before finalize runs. Customer sees the session as "still running" for up to 6 extra seconds. The kart may have stopped but the billing UI shows active. This is a UX issue, not a correctness issue.
- **fix:** Send a `BillingSessionChanged` event when entering grace window (currently suppressed — the `events_to_broadcast.push(...)` call was removed in the diff). Consider a "grace/processing" UI state.
- **confidence:** High on behavior.

---

### P3 nits

**1. [code quality] billing.rs:1629 — `now_for_grace` computed once before the loop but `chrono::Utc::now()` is also called inside the loop body (line ~1648 for `grace_until = chrono::Utc::now() + 5s`). Minor inconsistency — use `now_for_grace + Duration::seconds(5)` for the grace deadline.**

**2. [test quality] billing_grace tests use `make_grace_test_db()` which omits many columns present in production schema (driver_id, wallet_debit_paise, etc.). Tests pass but don't validate against real schema. Use a shared test schema fixture.**

**3. [code style] billing.rs:5943 — `hydrate_active_timers_from_db` takes `&BillingManager` but all other billing functions take `&Arc<AppState>`. Inconsistent API surface. Minor, but makes it harder to add state-dependent logic later.**

**4. [test] The two lap_rejection tests (`test_lap_reject_within_grace_window_caught`, `test_lap_reject_outside_grace_window_not_caught`) test raw SQL inserts, not `record_lap_rejection()`. They don't exercise the actual function under test.**

**5. [docs] `billing.rs:5832` — the doc comment says "Column name is `session_id` per CONTEXT.md D-12 (holds billing_session_id at runtime, consistent with laps.session_id)." This is confusing — `laps.session_id` likely refers to a *game* session ID, not a billing session ID. The comment should explicitly state these are different namespaces to prevent future confusion.**

---

### F-05 formula verification

**Setup:** 30-minute session (1800s allocated), ended at 15 minutes (900s driven), original wallet debit = Rs.700 = 70000 paise.

**What `compute_refund(1800, 900, 70000)` must compute:**

The test comment states: `best_rate_for_minutes(15, 2500) = 37500`, therefore `refund = 70000 - 37500 = 32500`.

Let's verify: 15 minutes × 2500 paise/min = 37,500 paise = Rs.375 actual cost. Refund = 70,000 − 37,500 = **32,500 paise = Rs.325**.

**The plan said 35,000.** Let's check where 35,000 comes from: 70,000 / 2 = 35,000 (simple proportional: half the session used = half refunded). This is the *wrong* formula if billing uses per-minute rates rather than proportional refund. The per-minute formula (32,500) is correct *if* the rate is 2500 paise/min flat.

**However:** The test comment also says `best_rate_for_minutes` is called internally. If `best_rate_for_minutes` applies *tiered* pricing (e.g., first 10 min at 2500, next 5 min at 2000), the actual cost could differ. The test asserts 32,500 assuming flat 2500/min. **The audit cannot verify this without seeing `best_rate_for_minutes` source**, but the math is internally consistent given the stated rate.

**The plan's 35,000 figure appears to be a rough estimate using proportional math, not the actual billing formula.** The test value of 32,500 is consistent with per-minute billing at 2500 paise/min.

- **compute_refund(1800, 900, 70000) = 32,500** (per-minute billing at 2500/min)
- **Is 32,500 correct?** Yes, *if* `best_rate_for_minutes(15)` returns 37,500. The plan's 35,000 was an approximation.
- **Is 35,000 correct?** Only if billing is purely proportional (half time = half cost), which contradicts the per-minute model.
- **Recommended assertion value:** **32,500** — but add a comment explicitly showing the rate tier lookup result (37,500) so future readers can verify the rate hasn't changed.
- **Risk:** If `best_rate_for_minutes` is ever changed (new pricing tier), this test will catch it — which is the desired behavior. The test is locking the *current* formula, which is correct.

---

### Test coverage gaps (ranked by risk)

| Risk | Gap | Why it matters |
|------|-----|----------------|
| **P0** | No test for double-finalize (grace timer seen by two consecutive ticks) | The race described in P0-finding-1 is untested; it's the most dangerous concurrency bug |
| **P0** | No test for cancel-during-grace-window | Cancel + grace window interaction is completely untested; FSM corruption possible |
| **P1** | No test for hydrate_active_timers_from_db + recover_active_sessions ordering | The P0-finding-2 overwrite race is untested |
| **P1** | No test for `record_lap_rejection()` function itself (only raw SQL tested) | The actual function's TOCTOU and DB insert path are untested |
| **P1** | No test for grace window with EndedEarly status (only Completed tested) | Wrong terminal status on early-end sessions |
| **P1** | No test for grace window DB persist failure (fire-and-forget) | Silent data loss on DB error |
| **P2** | No test for lap rejection arriving for non-existent billing_session_id | FK violation behavior undefined |
| **P2** | No test for hydration when pod_id in billing_sessions doesn't match current pod state | Stale pod mapping silently accepted |
| **P2** | No test for `compute_refund` with tiered rates (only flat 2500/min tested) | Rate tier changes would break formula silently |
| **P3** | No test for grace window UI event (BillingSessionChanged suppressed during grace) | Customer-visible "frozen" session state untested |
| **P3** | No test for 6s latency bound (grace=5s + tick=1s) | Latency regression undetectable |

---

### Deploy readiness score (0-10)

- **concurrency: 3/10** — Double-finalize race (P0) and TOCTOU in lap rejection (P1) are unresolved. The snapshot-drop-then-finalize pattern is structurally unsafe without a sentinel status.
- **restart safety: 4/10** — hydrate vs. recover_active_sessions ordering bug (P0) will corrupt active timers on every restart with live sessions. pending_end_status not persisted (P1) means cancel-during-grace loses status on restart.
- **F-05 regression: 7/10** — Formula math is correct (32,500). Test 1 is solid. Test 2 tests a copy of the SQL, not the real query — structural weakness but not wrong.
- **overall: 4/10**

---

### Ready to ship?

**NO** — Two P0 blockers (double-finalize race in tick loop; hydrate_active_timers_from_db overwrites recover_active_sessions timers with zero-rate defaults on every restart with active sessions) must be resolved before deploy.
