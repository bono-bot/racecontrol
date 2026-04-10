

## Phase 363-03 MMA Audit — Findings

### P0 blockers (must fix before deploy)

1. **[Restart safety] [billing.rs: hydrate_active_timers_from_db]** — Hydrated timers have `telemetry_seconds_covered = HashSet::new()` and `driving_seconds = 0` (from Default), so the very next billing tick will compute coverage as 0% and driving_seconds will restart from zero. When the grace window expires and deferred finalize runs, `compute_refund` will receive `driving_seconds ≈ 0` (only seconds accumulated post-restart), producing a near-full refund instead of the correct partial refund. This is a **money-losing bug on every restart with active sessions**.
   - why: `hydrate_active_timers_from_db` uses `..Default::default()` which sets `driving_seconds = 0`. The SELECT query fetches `allocated_seconds` but not `driving_seconds`. When `end_billing_session` runs the deferred finalize, it reads `timer.driving_seconds` (now ~0-5s post-restart) and computes a massive over-refund. For a 25-minute session that crashed at minute 25, the customer gets refunded as if they drove 0 minutes.
   - fix: Add `driving_seconds` to the hydration SELECT and populate the field from the DB row. Also hydrate `rate_paise_per_min`, `driver_id`, `driver_name`, `wallet_owner_id`, `pricing_tier_id`, `status`, and any other fields that `end_billing_session` / `compute_refund` reads from the timer. At minimum: `driving_seconds`, `status`, `rate_paise_per_min`, `wallet_owner_id`, `driver_id`.
   - confidence: 95% — `driving_seconds` is clearly defaulted to 0 and the finalize path reads it from the in-memory timer.

2. **[Concurrency] [billing.rs: tick_all_timers ~line 1460]** — Double-finalize race: two consecutive tick iterations can both see the same expired grace timer. The first tick clears `grace_until`/`pending_end_status` in the mutable snapshot, but the `deferred_finalizes` vec is processed AFTER the write lock is dropped. If the second tick fires before `end_billing_session` completes (which acquires its own lock), the timer is still in `active_timers` with cleared grace fields — it will now be processed as a normal active timer (no grace, no pending_end), potentially ticking driving_seconds and then expiring again into a NEW grace window, leading to double-finalize.
   - why: The pattern is: (1) write-lock timers, (2) clear grace fields on expired timer but leave timer in map, (3) push to `deferred_finalizes`, (4) drop lock, (5) call `end_billing_session`. Between steps 4 and 5, another tick can acquire the write lock and see the timer with `grace_until=None, pending_end_status=None, status=Active` — it looks like a normal running timer.
   - fix: Either (a) remove the timer from `active_timers` inside the write lock before dropping it (and let `end_billing_session` handle a missing timer gracefully), or (b) set `timer.status` to a sentinel/terminal value inside the write lock so the next tick skips it, or (c) add a `finalizing: bool` field that the tick checks.
   - confidence: 85% — depends on whether `end_billing_session` is idempotent and whether the 1s tick interval makes collision likely. Even if rare, double-finalize = double-refund = money loss.

### P1 important (fix in phase)

1. **[Restart safety] [billing.rs: hydrate_active_timers_from_db]** — `pending_end_status` hardcoded to `Completed` loses the actual end reason. If the session was ending as `EndedEarly` (customer pressed stop) or `Cancelled` (admin cancel), the grace window was set with that status. On restart, it becomes `Completed`, which may produce different refund calculations, different customer-facing messaging, and incorrect analytics.
   - why: The `pending_end_status` is not persisted to the DB — only `lap_reject_grace_until` is. On restart, the code guesses `Completed`. The original `pending_end_status` could have been `EndedEarly` (which is the F-05 scenario — early end with partial refund).
   - fix: Add a `pending_end_status TEXT` column to `billing_sessions`, persist it alongside `lap_reject_grace_until` in the grace-window-set step, and hydrate from DB. Alternatively, persist the end_status in the same UPDATE that writes `lap_reject_grace_until`.
   - confidence: 90%

2. **[Billing FSM] [billing.rs: tick_all_timers, expired block]** — Grace window always sets `pending_end_status = Some(Completed)` regardless of how the session ended. The diff only shows the `expired` code path (natural time expiry), but the description says GLD-C-04 applies to all session-end triggers. If `end_billing_session` is called directly (cancel, force-end, early-end), those paths appear to bypass the grace window entirely. A cancel arriving DURING an active grace window will call `end_billing_session` directly while the timer still has `grace_until` set — the deferred finalize will then also try to finalize, causing a double-finalize.
   - why: No guard in `end_billing_session` to clear `grace_until`/`pending_end_status` when it runs. The deferred finalize loop will later find the timer gone (if `end_billing_session` removed it) or still present with cleared fields.
   - fix: At the top of `end_billing_session`, clear `lap_reject_grace_until` and `pending_end_status` on the timer (and NULL the DB column) so the deferred finalize loop becomes a no-op for that session.
   - confidence: 80% — depends on whether `end_billing_session` removes the timer from `active_timers`; if it does, the deferred finalize will just fail to find it, which is logged but not harmful. Still, the DB `lap_reject_grace_until` column would remain non-NULL, which could cause a spurious hydration on next restart.

3. **[Hydration race] [main.rs + billing.rs]** — `hydrate_active_timers_from_db` runs at startup before the Axum server binds, so there's no race with incoming WS messages in the current code. However, `recover_active_sessions` (line 2746) also populates `active_timers` and runs at startup. If both run, they may conflict — `hydrate_active_timers_from_db` inserts by `pod_id` key, and `recover_active_sessions` also inserts by `pod_id`. Whichever runs second wins. The diff shows `hydrate_active_timers_from_db` is called in `main.rs` but doesn't show whether `recover_active_sessions` is also called and in what order.
   - why: Two hydration paths for the same data structure with no dedup or ordering guarantee.
   - fix: Either (a) merge the two hydration paths, or (b) document and enforce ordering (hydrate first, recover second, recover skips already-present pod_ids), or (c) remove `recover_active_sessions`'s timer hydration now that the new path exists.
   - confidence: 75% — need to verify `recover_active_sessions` call site ordering.

4. **[Lap rejection] [billing.rs: record_lap_rejection]** — No foreign key constraint on `lap_rejections.session_id`. The `CREATE TABLE` in tests doesn't show a FK, and the INSERT silently succeeds for non-existent `billing_session_id`. In production, a lap reject for a typo'd or stale session ID will create an orphan row with `grace_window_caught=false` and no way to trace it.
   - why: D-12 spec likely expects referential integrity. Orphan rows corrupt analytics.
   - fix: Add `FOREIGN KEY (session_id) REFERENCES billing_sessions(id)` to the migration. The INSERT already handles errors with `let _ =`, so FK violations would be silently swallowed — change to log the error.
   - confidence: 70% — depends on whether SQLite FK enforcement is enabled (`PRAGMA foreign_keys = ON`).

### P2 minor (can defer)

1. **[F-05 test brittleness] [billing.rs: test_end_billing_session_early_end_refund_amount]** — Test 2 hardcodes the exact SQL UPDATE shape. Any future change to the CAS UPDATE (adding a column, changing status names, reordering clauses) will break this test even if the change is correct. This is a maintenance burden, not a correctness issue — the test catches regressions but also produces false positives.
   - why: The test duplicates production SQL rather than calling the production function. It tests a copy, not the real code.
   - fix: Extract the CAS UPDATE SQL into a named constant or function shared between production code and test, so changes are automatically reflected. Alternatively, accept the brittleness as intentional (the comment says "forces re-reading the root cause doc").
   - confidence: 90%

2. **[Column naming] [billing.rs: record_lap_rejection]** — `lap_rejections.session_id` holds a `billing_session_id`, same as `laps.session_id`. The comment documents this, but the ambiguity with driver session IDs (auth sessions, WebSocket sessions) is a footgun for future developers. No runtime bug, but a readability/maintenance issue.
   - why: Naming inconsistency across the schema.
   - fix: Consider `billing_session_id` as the column name in a future migration, or add a CHECK constraint / comment in the migration SQL.
   - confidence: 95%

3. **[Grace window UX] [billing.rs]** — 6-second worst-case latency (5s grace + 1s tick) before the customer sees "session ended." No loading/processing state is broadcast during the grace window — the dashboard still shows the session as active (the `continue` in the tick loop skips the broadcast). Customer may try to start a new session on the same pod and get blocked.
   - why: No `BillingSessionChanged` event is broadcast when entering the grace window. The UI has no "finalizing" state.
   - fix: Broadcast a "finalizing" or "processing" status when entering the grace window so the dashboard reflects reality. Add a `Finalizing` status or use the existing event with a flag.
   - confidence: 85%

4. **[Error handling] [billing.rs: grace_window_sets, deferred_finalizes]** — Both the grace-window persist and the grace-clear UPDATE use `let _ =` to discard errors. If the DB write fails, the in-memory state and DB state diverge: the timer thinks grace is set but the DB doesn't know, so a restart loses the grace window.
   - why: Fire-and-forget DB writes for critical state transitions.
   - fix: At minimum, log the error. Ideally, retry or revert the in-memory state on failure.
   - confidence: 90%

### P3 nits

1. **[Style]** — `billing_grace` test module duplicates `make_grace_test_db()` which is nearly identical to the `create_test_db()` helper used in `billing::tests`. Consider reusing.

2. **[Style]** — Missing newline at end of file (`\ No newline at end of file` in diff).

3. **[Logging]** — `record_lap_rejection` logs at `info` level for every rejection, both caught and not-caught. In high-rejection scenarios this could be noisy. Consider `debug` for the not-caught case.

4. **[Test naming]** — `test_f05_refund_uses_original_debit` name doesn't mention the specific values being tested, making it harder to identify in CI output.

### F-05 formula verification

- `compute_refund(1800, 900, 70000)`:
  - `allocated_seconds = 1800` → 30-minute session
  - `driving_seconds = 900` → 15 minutes used
  - `wallet_debit_paise = 70000` → Rs.700 charged upfront
  - Per the test comments: `best_rate_for_minutes(15)` at Rs.25/min (2500 paise/min) = 15 × 2500 = 37500 paise
  - Refund = 70000 − 37500 = **32500 paise (Rs.325)**
  - The plan said 35000 (Rs.350). That would be simple proportional: `70000 × (1800−900)/1800 = 70000 × 0.5 = 35000`. But the code uses `best_rate_for_minutes` which does per-minute billing, not proportional.
- **Is 32500 correct?** Yes, IF `best_rate_for_minutes(15)` returns 37500 (i.e., 15 min × 2500 paise/min flat rate with no tier discount). The plan's 35000 was computed with simple proportional math, which is not what the code does. The test comment and formula are internally consistent. The auto-fix doc's claim that 32500 is correct because of `best_rate_for_minutes` is plausible.
- **Caveat:** I cannot see the `best_rate_for_minutes` implementation in this diff. If it applies tiered/discounted rates for 15 minutes (e.g., a lower per-minute rate for shorter durations), the actual cost could differ from 37500, making the refund different from 32500. The test would then lock a wrong value.
- **Recommended assertion value:** 32500, BUT add a companion test that directly asserts `best_rate_for_minutes(15, <rate>) == 37500` to make the dependency explicit and catch tier changes.

### Test coverage gaps (ranked by risk)

1. **HIGH — No integration test for the full deferred-finalize path** (tick detects expired grace → calls `end_billing_session` → refund computed correctly). All grace tests are unit-level logic replication, not calling `tick_all_timers` or `end_billing_session`. A bug in the actual wiring (e.g., wrong variable passed) would not be caught.

2. **HIGH — No test for cancel/force-end arriving during active grace window.** This is the double-finalize scenario from P0 #2. Critical path untested.

3. **HIGH — No test for hydrated timer going through a full tick cycle.** The P0 `driving_seconds=0` bug would be caught by such a test.

4. **MEDIUM — No test for `recover_active_sessions` + `hydrate_active_timers_from_db` interaction.** If both run, which wins? Untested.

5. **MEDIUM — No test for `record_lap_rejection` calling the actual function** (tests only exercise raw SQL INSERT, not the `record_lap_rejection` function with its grace-window-caught computation against `active_timers`).

6. **LOW — No test for grace window with `EndedEarly` status** (all tests use `Completed`). The F-05 scenario is specifically early-end, but the grace window always hardcodes `Completed`.

7. **LOW — No test for `best_rate_for_minutes(15)` returning 37500** — the F-05 regression test depends on this but doesn't verify it independently.

### Deploy readiness score (0-10)
- concurrency: 3/10 — double-finalize race is plausible and unguarded
- restart safety: 2/10 — `driving_seconds=0` on hydration is a money-losing bug
- F-05 regression: 7/10 — formula is likely correct but depends on unverified `best_rate_for_minutes`; SQL shape test is brittle but functional
- overall: 3/10

### Ready to ship?
**NO** — P0 #1 (hydrated timers have `driving_seconds=0`, causing over-refunds on restart) and P0 #2 (double-finalize race on consecutive ticks) are both money-losing production bugs that must be fixed and tested before deploy.
