## Phase 363-03 MMA Audit — Findings

### P0 blockers (must fix before deploy)
1. [restart safety / hydration correctness] [billing.rs:5879-5916] - `hydrate_active_timers_from_db()` rebuilds almost-empty `BillingTimer`s via `..Default::default()`, dropping persisted runtime state and likely corrupting resumed billing/finalize behavior after restart
   - why: The hydrated timer only sets `session_id`, `pod_id`, `allocated_seconds`, `telemetry_seconds_covered`, `lap_reject_grace_until`, and `pending_end_status`. It does **not** hydrate `status`, `driver_id`, `driver_name`, `wallet_owner_id`, `driving_seconds`, pause counters, pricing context, etc. Because `tick_all_timers()` and `end_billing_session()` operate on in-memory timers, a restarted server can now:
     - tick/finalize a session using default/empty driver/wallet fields,
     - overwrite or emit wrong session info/events,
     - fail downstream finalize logic that assumes timer state is populated,
     - replace a live timer inserted earlier with an incomplete hydrated one because the map key is `pod_id`.
     This is more dangerous than “no hydration” because it creates a false appearance of restart safety while reconstructing invalid state.
   - fix: Either:
     1. fully hydrate all `BillingTimer` fields required by ticking/finalization from DB, including exact `status`, `driving_seconds`, driver/wallet/pricing fields, or
     2. narrow hydration scope to only deferred-finalize/grace-window rows and drive finalize directly from DB state without reconstructing active timers, or
     3. store a separate persisted-finalize queue rather than reviving incomplete timers.
   - confidence: high

2. [concurrency / startup race] [main.rs:764-770, billing.rs:5879-5916] - startup hydration can race with incoming traffic and overwrite newly created in-memory timers
   - why: `main()` now calls hydration after feature-flag loading, but there is no shown guarantee that no WS/API handlers are active yet. If a new session starts while hydration is running, `hydrate_active_timers_from_db()` takes a write lock and blindly `insert(pod_id, timer)`. If that `pod_id` already has a fresh live timer, hydration can replace it with the incomplete defaulted timer above. Even if startup is “usually single-threaded”, this first-ever path should not rely on implied sequencing.
   - fix: Run hydration before accepting traffic, or make hydration idempotent/safe by using “insert only if absent” plus logging, or compare DB row/session ids before overwriting. Best is to complete hydration before server listeners/tasks start.
   - confidence: medium-high

### P1 important (fix in phase)
1. [billing FSM invariant] [billing.rs:1602-1614] - grace window hardcodes `pending_end_status = Completed`, which can mis-finalize sessions that should end as `EndedEarly`/other terminal state
   - why: The new defer path is triggered on `expired`, but the code discards the actual terminal status and stores only `Completed`. Your own audit prompt calls out “What if the session was ending as EndedEarly?”; from this diff, that concern is real. Deferred finalize later calls `end_billing_session(state, &sid, end_status)` using the stored value, so any non-Completed terminal intent is lost.
   - fix: Persist the real intended terminal status, not a constant. If expiry truly always means `Completed`, prove that with an invariant/test; otherwise store and hydrate the actual status.
   - confidence: medium

2. [restart safety] [billing.rs:5900-5906] - hydration assumes any persisted grace window implies `pending_end_status = Completed`, including cancelled/forced-end sessions
   - why: This repeats the same terminal-state loss across restarts. If a cancel/force-end path can interleave with a grace window or a crash occurs after grace persistence but before terminal transition, restart will always finalize as `Completed`. The code comments call this “conservative”, but it is not conservative if it changes money/status semantics.
   - fix: Persist `pending_end_status` explicitly in DB, or derive it from persisted billing session state/end reason instead of hardcoding.
   - confidence: medium

3. [concurrency / duplicate finalize] [billing.rs:1447-1460, 1669-1680] - same session can be scheduled for deferred finalize more than once across overlapping tick invocations
   - why: Within one tick call, clearing in-memory grace fields prevents duplicate collection. But if `tick_all_timers()` itself is invoked concurrently by two scheduler iterations/tasks, both can observe the same expired grace before either finishes DB finalize. There is no outer single-flight guard around the tick loop in the diff. `end_billing_session()` may be CAS-safe, but duplicate finalize attempts can still produce duplicate logs, duplicate side-effects around notifications/refunds if finalize is not fully idempotent, and noisy error paths.
   - fix: Ensure only one tick loop runs at a time, or add a per-session “finalize_in_progress” in-memory/DB guard before calling `end_billing_session()`.
   - confidence: medium

4. [grace-window correctness] [billing.rs:1646-1655] - persisting grace deadline ignores SQL errors, so a crash after in-memory deferral but before DB persistence loses restart safety
   - why: The code comment says restart-safe, but `UPDATE billing_sessions SET lap_reject_grace_until = ?` drops the result. If the DB write fails, the process continues with only RAM state. A crash in that 5s window then loses the deferred finalize marker. Since this phase’s main claim is restart safety, silent persistence failure undermines it.
   - fix: Log and surface DB write failures; ideally do not arm in-memory grace unless DB persistence succeeded, or mark the session unhealthy and retry aggressively.
   - confidence: high

5. [billing FSM / cancel interplay] [billing.rs:1447-1460, 1602-1614] - no demonstrated handling for cancel/force-end arriving during an active grace window
   - why: During grace, normal ticking is skipped and the timer remains in `active_timers` with `pending_end_status` set. If a cancel arrives during those 5 seconds, this diff shows no explicit branch that clears the grace or overrides pending terminal state. Depending on existing cancel path, you may either bypass grace correctly or later run stale deferred finalize with `Completed`.
   - fix: Add explicit semantics and tests: cancel/force-end during grace should cancel the deferred-completed finalize and finalize immediately with the overriding status.
   - confidence: medium

### P2 minor (can defer)
1. [F-05 regression test quality] [billing.rs:8648-8700] - SQL “shape lock” test is brittle and weakly coupled to production code
   - why: The test does not call production finalize code; it replays a handwritten SQL statement copied from source. It will catch the exact regression if someone edits the same clause and updates nothing else, but it can also:
     - fail for harmless refactors to the `UPDATE`,
     - pass while production regresses via a different SQL path/function,
     - encourage cargo-cult syncing of copied SQL.
   - fix: Prefer an integration test that executes `end_billing_session()` against an in-memory DB and asserts `wallet_debit_paise` remains unchanged after early end. Keep a smaller unit test for `compute_refund()`.
   - confidence: high

2. [restart-safety / finalize crash window] [billing.rs:1669-1680] - clearing DB `lap_reject_grace_until` before `end_billing_session()` widens the crash hole
   - why: On expired grace, code first executes `UPDATE ... SET lap_reject_grace_until = NULL`, then calls `end_billing_session()`. If the process crashes in between, restart hydration will not see a pending grace row, while the session may still be non-terminal. This is exactly the kind of “during finalize” hole the prompt asks about.
   - fix: Let `end_billing_session()` clear the column as part of the same terminal-state update, or use a transactional/atomic state transition.
   - confidence: high

3. [telemetry coverage semantics] [billing.rs:5896-5899] - hydrated sessions lose `telemetry_seconds_covered`, likely degrading coverage classification after restart
   - why: The comment admits coverage buckets are lost on crash. If finalization computes suspect/unverified based on this set, a restarted active session may later finalize with NULL/0 coverage despite having had valid telemetry pre-crash. This may be accepted by current D-05, but it is still a behavior regression introduced by first-ever hydration because now restarted sessions continue rather than being manually recovered.
   - fix: Defer if product accepts UNVERIFIED-after-restart, otherwise persist telemetry coverage aggregates needed for finalization.
   - confidence: medium

4. [schema clarity] [billing.rs:5838-5870] - `lap_rejections.session_id` holding `billing_session_id` is ambiguous and test schema lacks FK
   - why: The comment explains it, but the column name can be confused with a driver/game session id, especially since `laps.session_id` may or may not mean the same thing in all contexts. Also, inserts ignore failures, so rejects for non-existent billing sessions disappear silently if a real FK exists, or create orphan rows if it doesn’t.
   - fix: Defer rename if migration cost is high, but document in schema and add FK if intended. At minimum log insert failures in `record_lap_rejection()`.
   - confidence: medium

5. [UX latency] [billing.rs:1602-1614, 1669-1680] - finalize latency becomes 5-6 seconds with no visible state signaling in this diff
   - why: Customers may see a session that has “ended” physically but billing/refund not finalized for up to one extra tick. If UI still shows active/processing unclearly, support noise follows.
   - fix: Consider surfacing a “processing final laps” / “finalizing” status or timestamp. Likely not deploy-blocking if product is aware.
   - confidence: medium

### P3 nits
1. [observability] [billing.rs:1650-1661, 5838-5870] - multiple DB writes intentionally discard errors without logging
   - why: Silent failure makes operational diagnosis hard.
   - fix: log `Err(e)` on grace persistence and lap rejection insert.
   - confidence: high

2. [tests] [billing_grace module] - several grace tests replicate snippets of production logic instead of invoking production entrypoints
   - why: They validate assumptions, not full behavior.
   - fix: Add end-to-end tests through `tick_all_timers()` / `record_lap_rejection()`.
   - confidence: high

### F-05 formula verification
- compute_refund(1800, 900, 70000) = ? (show math)
  - Allocated = 1800s = 30 min
  - Used = 900s = 15 min
  - Original upfront debit = 70000 paise
  - Per the test/comments, `compute_refund()` uses `best_rate_for_minutes(15)`, not straight prorating from 30 min package
  - If the best 15-minute rate is 2500 paise/min, actual cost = `15 * 2500 = 37500`
  - Refund = `70000 - 37500 = 32500`
- is 32500 correct? or is 35000 correct?
  - **32500 is correct**, assuming the implementation contract really is “refund against best_rate_for_minutes(used_minutes)” and 15 min maps to 2500 paise/min.
  - **35000** would only be correct under simple proportional refund of a 30-minute package (`70000 * (15/30)`), which the comments say is **not** how `compute_refund()` works.
- recommended assertion value:
  - **32500**

### Test coverage gaps (ranked by risk)
1. **High risk:** No test that runs real `end_billing_session()` after an early end and verifies `wallet_debit_paise` survives plus refund is correct.
   - Current SQL test is a copy, not execution of production path.

2. **High risk:** No end-to-end test for “timer expires → grace persisted → late lap reject arrives → deferred finalize uses updated lap count”.
   - Current tests prove grace detection/caught flag separately, not billing outcome.

3. **High risk:** No test for restart during grace with a **non-Completed** pending end state (cancel/force-end/ended-early).
   - This is where hardcoded `Completed` can bite.

4. **High risk:** No test for crash window between clearing `lap_reject_grace_until` and terminal finalize.
   - Current restart-safe test only hydrates a persisted grace row.

5. **Medium risk:** No test that hydration before/after traffic cannot overwrite an already-live timer for the same pod/session.
   - Important since hydration is newly wired at startup.

6. **Medium risk:** No test proving only one tick loop runs or duplicate expired-grace finalization is harmless/idempotent.
   - Concurrency assumption currently untested.

7. **Medium risk:** No test for cancel/force-end arriving during active grace.
   - Billing FSM precedence unclear.

8. **Low-medium risk:** No test for `record_lap_rejection()` behavior when `billing_session_id` does not exist / FK violation / DB error.
   - Silent failure path.

9. **Low risk:** No UX/state test for what dashboard/client sees during the 5-6s grace.
   - Product concern more than correctness.

### Deploy readiness score (0-10)
- concurrency: 5/10
- restart safety: 3/10
- F-05 regression: 7/10
- overall: 4/10

### Ready to ship?
NO + hydration currently reconstructs incomplete timers and can mis-handle restarted sessions, which is too risky for deploy.
