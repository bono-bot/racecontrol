---
phase: 92-retention-loops
plan: 01
subsystem: psychology
tags: [rust, axum, sqlite, sqlx, nudge-queue, wallet, retention, streaks, membership, variable-rewards, rand, chrono]

# Dependency graph
requires:
  - phase: 89-psychology-foundation
    provides: psychology.rs with queue_notification, update_streak, evaluate_badges, nudge_queue table
  - phase: 91-pb-broadcast
    provides: PbAchieved broadcast in lap_tracker.rs, personal_bests table
  - phase: 90-streaks
    provides: streaks table with current_streak, longest_streak, last_visit_date, grace_expires_date
provides:
  - variable_reward_log DB table with driver_id+month index for monthly cap audit
  - notify_pb_beaten_holders — queues pb_beaten WhatsApp nudge for active drivers within 5% of new PB
  - maybe_grant_variable_reward — 15%/10% probability bonus credits capped at 5% monthly spend
  - check_streak_at_risk — daily 10AM IST sweep for streaks expiring in 2 days
  - check_membership_expiry_warnings — daily loss-framed nudge for memberships expiring in 3 days
  - Passport API extended with grace_expires_date, longest_streak, last_visit_date
affects: [pwa-passport-page, admin-retention-dashboard, whatsapp-nudge-templates]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ThreadRng evaluated in scoped block before first .await to satisfy tokio::spawn Send bound"
    - "Daily check guard: now_ist.hour() == 10 && now_ist.minute() < 2 in 60s scheduler tick"
    - "Deduplication via NOT EXISTS on nudge_queue with template + status + 8-day window"
    - "Monthly cap: total_debited_paise / 20 = 5% ceiling enforced via variable_reward_log SUM"
    - "Fan-out cap: LIMIT 5 on notify_pb_beaten_holders to prevent notification storms"
    - "Closeness filter: pb.best_lap_ms <= new_time * 105 / 100 integer math (avoids float)"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/psychology.rs
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/scheduler.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "ThreadRng scoped block pattern: evaluate gen_bool before any .await to avoid !Send error in tokio::spawn futures"
  - "Closeness filter 5% uses integer math (best_lap_ms <= new_time * 105 / 100) to avoid float comparison"
  - "LIMIT 5 fan-out cap on notify_pb_beaten_holders prevents notification spam on popular track+car combos"
  - "variable_reward_log uses reward_id (UUID) as both log PK and wallet transaction reference_id for auditability"
  - "Passport API returns full streak row fields (grace_expires_date, longest_streak, last_visit_date) for PWA urgency display"

patterns-established:
  - "Psychology functions are fire-and-forget in tokio::spawn — errors logged, never panic"
  - "Scheduler daily checks use IST FixedOffset with hour()==10 && minute()<2 guard against duplicate runs"

requirements-completed: [RET-01, RET-02, RET-03, RET-04, RET-05, RET-06]

# Metrics
duration: 15min
completed: 2026-03-21
---

# Phase 92 Plan 01: Retention Loops Summary

**Four retention functions in psychology.rs (PB rivalry nudges, surprise credits, streak-at-risk warnings, loss-framed membership expiry) wired from lap_tracker/billing/scheduler with variable_reward_log cap table and extended passport API**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-21T07:45:00Z
- **Completed:** 2026-03-21T08:00:57Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Added `variable_reward_log` table with `(driver_id, month)` index for RET-06 monthly 5% spend cap enforcement
- Implemented all four retention psychology functions: `notify_pb_beaten_holders`, `maybe_grant_variable_reward`, `check_streak_at_risk`, `check_membership_expiry_warnings`
- Wired PB retention hooks in lap_tracker (tokio::spawn after PbAchieved broadcast), milestone reward in billing.rs post_session_hooks, daily checks in scheduler.rs at 10AM IST
- Extended passport API summary with `grace_expires_date`, `longest_streak`, `last_visit_date` for PWA streak urgency UI

## Task Commits

Each task was committed atomically:

1. **Task 1: Add variable_reward_log table and four retention functions** - `fc329f2` (feat)
2. **Task 2: Wire retention triggers in lap_tracker, billing, scheduler, passport API** - `1dca228` (feat)

**Plan metadata:** (this commit, docs)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` - Added variable_reward_log CREATE TABLE + index migration
- `crates/racecontrol/src/psychology.rs` - Four new public async functions + `use rand::Rng` import
- `crates/racecontrol/src/lap_tracker.rs` - tokio::spawn block after PbAchieved broadcast for PB retention hooks
- `crates/racecontrol/src/billing.rs` - maybe_grant_variable_reward("milestone") call in post_session_hooks
- `crates/racecontrol/src/scheduler.rs` - Daily IST 10AM guard calling check_streak_at_risk + check_membership_expiry_warnings; added Utc+FixedOffset to chrono imports
- `crates/racecontrol/src/api/routes.rs` - Passport summary extended with grace_expires_date, longest_streak, last_visit_date

## Decisions Made

- **ThreadRng Send fix:** `rand::thread_rng()` is `!Send` — cannot be held across `.await`. Fixed by scoping rng evaluation into a block before any await point: `let should_proceed = { let mut rng = rand::thread_rng(); rng.gen_bool(threshold) };`
- **Fan-out cap:** `LIMIT 5` on notify_pb_beaten_holders prevents notification storms on popular combos
- **Closeness filter:** `best_lap_ms <= new_time * 105 / 100` integer math avoids float comparison issues; only drivers within 5% of new PB are notified
- **Passport query change:** Replaced simple `query_scalar` for streak_weeks with `query_as` fetching all four streak columns; unwrap to defaults `(0, 0, None, None)` maintains backward compatibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed ThreadRng !Send compilation error in maybe_grant_variable_reward**
- **Found during:** Task 2 (cargo build after wiring lap_tracker tokio::spawn)
- **Issue:** `rand::thread_rng()` returns `ThreadRng` which is `!Send`. Holding it across `.await` inside a `tokio::spawn` future causes compile error "future cannot be sent between threads safely"
- **Fix:** Wrapped rng evaluation in a scoped block before any await: `let should_proceed = { let mut rng = rand::thread_rng(); rng.gen_bool(threshold) };`
- **Files modified:** crates/racecontrol/src/psychology.rs
- **Verification:** cargo build succeeds with no errors
- **Committed in:** 1dca228 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Required fix for tokio async correctness. No scope creep.

## Issues Encountered

- ThreadRng !Send error only surfaced during Task 2 when `maybe_grant_variable_reward` was called from a `tokio::spawn` context in lap_tracker. Psychology.rs functions called directly (billing.rs, scheduler.rs) are unaffected since they're called inline from async functions that don't need Send bounds. The scoped block fix is clean and idiomatic.

## User Setup Required

None - no external service configuration required. All changes are backend-only Rust code.

## Next Phase Readiness

- All six RET-* requirements have backend support
- nudge_queue pipeline (Phase 89) will deliver queued WhatsApp nudges
- PWA passport page can now consume grace_expires_date to show streak urgency UI
- WhatsApp template strings (pb_beaten, streak_at_risk, membership_expiry) need to be registered in the resolve_template function (Phase 93+ if not already done)

---
*Phase: 92-retention-loops*
*Completed: 2026-03-21*
