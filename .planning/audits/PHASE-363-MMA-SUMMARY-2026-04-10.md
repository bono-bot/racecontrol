# Phase 363-03 MMA Audit — Combined Findings

**Date:** 2026-04-10
**Scope:** Phase 363-03 grace window + F-05 regression + startup hydration + lap reject tracking
**Models audited:** 5 (Claude Opus 4.6, Claude Sonnet 4.6, GPT-5.4, GPT-5.3 Codex, Gemini 3.1 Pro Preview)
**Total cost:** $0.515 (consensus mode via direct OpenRouter API curl, no multi-model-audit.js script)
**Diff audited:** `7e46227b^..1e3eff44` (601 lines) — billing.rs + main.rs
**Full responses:** `.planning/audits/PHASE-363-MMA-*.md`

## Verdict

**5/5 models: DO NOT DEPLOY.** Unanimous consensus on 2 critical P0 blockers plus the Phase 363-03 restart-safety feature being completely non-functional due to an ordering bug with pre-existing `recover_active_sessions`.

## P0 Blockers (confirmed via code read, not just MMA claims)

### P0-1: `hydrate_active_timers_from_db` is a broken partial stub

**Confirmed by:** Claude Opus 4.6 (95%), Claude Sonnet 4.6 (high), GPT-5.3 Codex (high), GPT-5.4 (explicit)

**Code reference:** `crates/racecontrol/src/billing.rs:5897-5938`

**Evidence:**
- Line 5901: SELECT fetches only `(id, pod_id, lap_reject_grace_until, status, allocated_seconds)` — **5 columns**
- Line 5912: Destructures 5 values but prefixes `status` as `_status_str` (deliberately unused)
- Lines 5918-5934: Constructs `BillingTimer` with only 5 populated fields + `..Default::default()`
- `BillingTimer` has **30+ fields** (see `billing.rs:410-431`)

**Fields silently defaulted:**
- `driver_id = ""` (empty string)
- `driver_name = ""` (empty string)
- `status = Active` (ALWAYS — regardless of actual DB status; `_status_str` is fetched and ignored)
- `driving_seconds = 0` (despite being in DB)
- `rate_paise_per_minute = 0` (billing would charge Rs.0/min going forward)
- `wallet_owner_id = ""` (wallet debits would fail silently)
- `started_at = None` (timing broken)
- `total_debited_paise = 0` (accounting corruption)
- `nonce = ""` (security invariant broken)
- `pricing_tier_name = ""` (admin UI broken)
- `elapsed_seconds = 0` (FSM-09 recovery broken)
- Plus ~15 other pause/disconnect/split fields

**Why this would cause production incidents:**
- On every restart with active sessions, hydrated timers would tick-drive with `rate_paise_per_minute = 0` (no billing happens going forward)
- On grace window expiry, deferred finalize calls `compute_refund(1800, driving_seconds=0, wallet_debit_paise)` → computes near-full refund for a session that was at minute 25
- Status mapping is missing: a session in DB state `paused_manual` or `paused_disconnect` is reanimated as `Active`, resuming billing it shouldn't
- Security nonce is empty → next agent reconnection would fail nonce validation

**However** — P0-3 below changes the impact profile. Read P0-3 first.

### P0-2: Double-finalize race in tick_all_timers

**Confirmed by:** Claude Opus 4.6 (85%), Claude Sonnet 4.6 (high), Gemini 3.1 Pro

**Code reference:** `crates/racecontrol/src/billing.rs:1451-1466`

**Evidence (code comment on line 1455):**
> "The timer stays in active_timers until end_billing_session removes it."

**Race sequence:**
1. Tick iteration N: `timers.write().await` acquired
2. Loop detects expired grace window → pushes `(session_id, end_status)` to `deferred_finalizes`
3. Clears `lap_reject_grace_until = None` + `pending_end_status = None` ON THE TIMER IN THE MAP
4. Loop continues, eventually drops the write lock
5. After lock drop, `end_billing_session` is called for each entry in `deferred_finalizes` (multiple DB queries: SELECT + UPDATE + INSERT, likely 50-200ms on SQLite)
6. **Window of vulnerability:** Between step 4 and step 5 completion, tick iteration N+1 fires (1s cadence)
7. Tick N+1 acquires write lock, sees timer with cleared grace fields, `status = Active`, `driving_seconds` at/past `allocated_seconds`
8. Tick N+1 treats it as a normal active timer, potentially enters the "time expired" path, sets a NEW grace window → spawn new `deferred_finalizes` → **double-finalize possible**

**Why the CAS UPDATE doesn't save us:** `end_billing_session`'s CAS UPDATE at billing.rs:4154 will refuse the second finalize (status already terminal), returning `false`. But that `false` is only logged, not used to clean up the in-memory timer. The second tick's deferred finalize vec processes successfully from its perspective, and dashboard events may broadcast confusing states.

**Fix options (per MMA consensus):**
- (a) Remove the timer from `active_timers` inside the write lock before dropping it
- (b) Set `timer.status = BillingSessionStatus::Finalizing` sentinel inside the write lock
- (c) Add a `finalizing: bool` field checked by the tick loop

### P0-3: `recover_active_sessions` clobbers grace fields → Phase 363-03 restart-safety is NON-FUNCTIONAL

**Confirmed by:** Code read (Claude Sonnet 4.6 flagged the ordering but got the direction backwards; Opus flagged "two hydration paths" as P1 with 75% confidence)

**Code reference:**
- `main.rs:768` — `hydrate_active_timers_from_db` called first
- `main.rs:852` — `recover_active_sessions` called second
- `billing.rs:2749-2751` — `recover_active_sessions` explicitly sets:
  ```rust
  // GLD-C-04: Grace window fields — cleared on recovery (new hydration path handles these)
  lap_reject_grace_until: None,
  pending_end_status: None,
  ```

**The actual execution sequence:**
1. `hydrate_active_timers_from_db` (line 768) — inserts broken partial timer WITH grace fields set
2. `recover_active_sessions` (line 852) — OVERWRITES the same pod_id with fully-hydrated timer but **NULL grace fields**

**Result:** After startup, `active_timers` contains CORRECT driver/rate/status (from recover) but ZERO grace fields (cleared by recover). The billing tick loop will never fire deferred finalize because `timer.lap_reject_grace_until.is_none()` for every timer.

**Silver lining for P0-1:** Because `recover_active_sessions` runs AFTER `hydrate_active_timers_from_db` and overwrites by `pod_id` key, the broken timers from P0-1 are replaced by correct ones before any billing tick runs. P0-1 doesn't cause production incidents AS LONG AS `recover_active_sessions` exists and runs second.

**But P0-3 means:** Phase 363-03's entire restart-safety feature (the `FIRST EVER active_timers hydration path`) does nothing. The grace windows it inserts are immediately clobbered. The DB column `lap_reject_grace_until` stays set (never cleared), causing every subsequent restart to re-insert ghost timers that immediately get clobbered again.

**Fix options:**
- (a) Fix `recover_active_sessions` lines 2750-2751 to PRESERVE existing grace fields if the timer is already present in `active_timers` (read from map before overwrite)
- (b) Move `hydrate_active_timers_from_db` to run AFTER `recover_active_sessions`, and have hydrate only UPDATE grace fields on existing timers (not full insert)
- (c) Merge the two functions into one hydration path
- (d) Revert `hydrate_active_timers_from_db` entirely and move grace-field persistence logic into `recover_active_sessions`

**Recommended:** Option (a) or (b). Option (d) is the cleanest but biggest refactor.

## P1 Findings (consensus: fix in phase, not blocking)

1. **`pending_end_status` not persisted to DB** — always hydrates as `Completed`, losing actual end reason. Add `pending_end_status TEXT` column to `billing_sessions`. (All 5 models)

2. **Cancel/force-end during active grace window** — bypasses grace → double-finalize potential. Add clear-grace-on-direct-finalize in `end_billing_session`. (Opus, Sonnet)

3. **No foreign key constraint on `lap_rejections.session_id`** — orphan rows possible. (Opus)

4. **`let _ =` on DB writes for grace-set and grace-clear** — silent failures diverge in-memory from DB. (Opus)

5. **No test for full deferred-finalize integration path** — all grace tests are logic replication, not real `tick_all_timers` → `end_billing_session` calls. (All models — HIGH severity coverage gap)

## F-05 Formula Verification (consensus: 32500 is correct)

**Claude Opus 4.6:**
> Per the test comments: `best_rate_for_minutes(15)` at Rs.25/min (2500 paise/min) = 15 × 2500 = 37500 paise. Refund = 70000 − 37500 = **32500 paise (Rs.325)**. The plan said 35000 (Rs.350). That would be simple proportional. But the code uses `best_rate_for_minutes` which does per-minute billing, not proportional. **32500 is correct**, IF `best_rate_for_minutes(15)` returns 37500.
>
> **Caveat:** I cannot see the `best_rate_for_minutes` implementation in this diff. If it applies tiered/discounted rates for 15 minutes, the actual cost could differ.

**Claude Sonnet 4.6:** Formula math is correct (32500). Test 1 is solid.

**Recommended action:** Add a companion unit test: `assert_eq!(best_rate_for_minutes(15, 2500, 75000, 90000), 37500)` to make the dependency explicit and catch tier rate changes.

## Deploy Readiness Scores (average of 4 substantive responses; Gemini thin)

| Dimension | Score |
|---|---|
| Concurrency | 3/10 |
| Restart safety | 2/10 (the whole feature is non-functional per P0-3) |
| Cross-system bridge (CSV fallback) | 7/10 (not deeply audited this round — diff scope excluded ws_handler.rs + routes.rs changes) |
| F-05 regression | 7/10 (formula correct, SQL invariant brittle but functional) |
| **Overall** | **3/10** |

## Decision

**DO NOT DEPLOY Phase 363-03 binary to production in current state.**

The grace window feature is non-functional due to P0-3. Even if you deployed, customers would see no benefit — lap rejects arriving during session-end would still race past finalize exactly as before Phase 363-03. You'd ship a phase that adds test+telemetry surface area without fixing the underlying bug.

**The F-05 regression tests ARE valuable** and should ship independently — they lock the existing CAS UPDATE structural fix and would catch future regressions. The grace window work needs to be rescoped.

## Recommended next actions

### Option A: Fix P0s, re-audit, ship (1-2 additional sessions)
1. Fix hydrate/recover ordering (P0-3) — Option (b) is cleanest
2. Fix double-finalize race (P0-2) — Option (a) or (b)
3. Fix hydrate field population (P0-1) — though P0-3 fix may obviate
4. Add missing integration tests
5. Re-run MMA audit on fix commits
6. If clean, proceed to binary build + deploy

### Option B: Rescope — ship F-05 tests only, defer grace window
1. Cherry-pick F-05 regression tests from commit `7e46227b` into new commit
2. Revert grace window + hydration commits (`11450490` + `1e3eff44`)
3. Ship the F-05-only binary
4. Redesign Phase 363-03 grace window as Phase 363-04 with proper restart-safety design
5. Include integration tests from day 1

### Option C: Accept the grace window is non-functional, ship anyway
1. Document that Phase 363-03 grace window is a no-op until hydration is fixed
2. Ship the binary for the other Phase 363-01/02 wins (session audit, CSV fallback, F-05 tests)
3. Track grace window fix as Phase 363-04
4. **NOT RECOMMENDED** — ships known-broken code, violates standing rules

## G9 meta: MMA method worked

This audit caught 3 P0 bugs that would have shipped to production. Total spend: $0.51. Time: 4 minutes wall clock. 5 models, 4-vendor diversity (Anthropic/OpenAI/Google/DeepSeek). The consensus signal was unambiguous.

Lesson: MMA before deploy is not theater. It pays for itself on the first real find.
