## Phase 363-03 MMA Audit — Findings

### P0 blockers (must fix before deploy)
1. **[restart-safety correctness] [billing.rs: `hydrate_active_timers_from_db` loop (~5890+)] - Hydration rebuilds timers with `..Default()` and ignores DB `status` (and likely runtime counters), reanimating paused sessions as `Active`.**
   - why: In hydration, `_status_str` is fetched but never mapped into `BillingTimer.status`; `..Default()` sets `status=Active`. That means a session persisted as `paused_manual`/`paused_disconnect` etc. resumes as active after restart. This can incorrectly resume billing and state progression.
   - fix: Parse `status` string into `BillingSessionStatus` and set `timer.status` explicitly. Also hydrate core counters needed for correctness (`driving_seconds`, elapsed/base timestamps, pause metadata as applicable), or do not hydrate non-grace sessions until full-fidelity hydration is implemented.
   - confidence: **high**

2. **[restart-safety correctness] [billing.rs: `hydrate_active_timers_from_db` SELECT/constructor (~5875-5925)] - Hydration appears incomplete for active sessions (alloc only + defaults), which can reset runtime accounting after restart.**
   - why: Only `allocated_seconds` + optional grace are hydrated; remaining billing state defaults (e.g., driving/elapsed/started_at) likely reset. If tick/finalize logic depends on these fields, restart can grant extra time or compute wrong final/refund values.
   - fix: Hydrate minimum billing-critical fields from DB and reconstruct consistent in-memory timer state. Add an integration test: create mid-session row (`driving_seconds>0`, paused state), restart/hydrate, assert next tick/finalize matches pre-restart behavior.
   - confidence: **medium-high** (exact impact depends on how `BillingTimer` fields are used elsewhere, but risk is large)

### P1 important (fix in phase)
1. **[F-05 regression test strength] [billing.rs tests: `test_end_billing_session_early_end_refund_amount` (~8640+)] - SQL invariant test does not execute production code path.**
   - why: The test replays a copied SQL statement in test code. If production SQL regresses (adds `wallet_debit_paise` in SET), this test may still pass unless manually kept in sync.
   - fix: Call `end_billing_session()` end-to-end against in-memory DB seeded with realistic row and assert `wallet_debit_paise` unchanged. If possible, also assert resulting refund.
   - confidence: **high**

2. **[FSM invariant] [billing.rs: tick deferred finalize block (~1668+)] - `pending_end_status`/grace are cleared in-memory before finalize attempt; on finalize failure, retry semantics degrade and status intent can be lost.**
   - why: Timer fields are nulled before `end_billing_session().await`. If finalize fails transiently, next tick re-enters grace logic from generic expired path (`Completed`) rather than original deferred intent, and adds extra delay cycles.
   - fix: Clear in-memory grace fields only after successful finalize, or keep a separate `finalize_in_flight`/retry marker.
   - confidence: **medium**

### P2 minor (can defer)
1. **[concurrency + overlap] [tick loop design] - Potential duplicate deferred finalize if tick invocations overlap.**
   - why: If scheduler can run `tick_all_timers` concurrently, both can observe same expired timer and call finalize. CAS should protect DB row, but duplicate side-effects/log noise possible.
   - fix: Enforce single-flight tick (mutex/serialized loop) or per-session in-flight finalize guard.
   - confidence: **medium**

2. **[lap rejection persistence] [billing.rs: `record_lap_rejection` (~5760+)] - DB insert errors are swallowed silently.**
   - why: `let _ = ...execute().await;` without error logging. FK/schema issues or DB failures lose audit trail invisibly.
   - fix: Log errors at warn/error with `session_id`, `lap_number`.
   - confidence: **high**

3. **[column semantics/FK] [lap_rejections schema usage] - `session_id` ambiguity and FK enforcement not validated in tests.**
   - why: Comment says `session_id` stores billing session id; if DB has no FK, orphan rejects can be inserted.
   - fix: Add FK to `billing_sessions(id)` (if migration allows) and test behavior for non-existent session id.
   - confidence: **medium**

### P3 nits
1. **[customer UX] [grace behavior] - Up to ~6s added finalize latency (5s grace + 1s tick) should surface as explicit “processing final laps” state.**
   - why: Avoid “stuck/broken” perception right at session end.
   - fix: dashboard/API status hint during grace.
   - confidence: **high**

---

### F-05 formula verification
- compute_refund(1800, 900, 70000) = **32500 paise**
  - Used time = 900s = 15 min  
  - `best_rate_for_minutes(15)` under current model = `15 * 2500 = 37500` paise  
  - Refund = original debit `70000 - 37500 = 32500`
- is 32500 correct? or is 35000 correct?
  - **32500 is correct** if `compute_refund()` uses `best_rate_for_minutes` (as described in code/comments).  
  - 35000 is only the naive proportional half-refund and does **not** match current function contract.
- recommended assertion value:
  - **32500**

---

### Test coverage gaps (ranked by risk)
1. **High:** Hydration fidelity test for paused/active status and billing counters after restart.  
2. **High:** End-to-end F-05 regression via real `end_billing_session()` (not copied SQL).  
3. **Medium:** Cancel/force-end arriving during grace window (ordering/race invariant).  
4. **Medium:** Finalize failure retry behavior (DB error during deferred finalize).  
5. **Medium:** Non-existent billing_session_id in `record_lap_rejection` (FK/error path).  
6. **Low-Medium:** Explicit concurrent tick overlap test (if architecture permits overlap).

---

### Deploy readiness score (0-10)
- concurrency: **7/10**
- restart safety: **3/10**
- F-05 regression: **6/10**
- overall: **4/10**

### Ready to ship?
**NO** — hydration currently appears to restore active timers with incorrect state (status/counters), which is a production billing correctness risk.
