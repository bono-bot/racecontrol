---
phase: 92-retention-loops
plan: 02
subsystem: ui
tags: [nextjs, typescript, pwa, passport, streak, retention, grace-period, urgency]

# Dependency graph
requires:
  - phase: 92-retention-loops
    plan: 01
    provides: grace_expires_date and longest_streak fields in /customer/passport API response
  - phase: 90-streaks
    provides: streaks table with current_streak, longest_streak, last_visit_date, grace_expires_date
provides:
  - PWA passport page enhanced streak card with grace urgency indicator (red border + days countdown)
  - PassportData TypeScript type extended with longest_streak, last_visit_date, grace_expires_date
  - Longest streak "Best: Nw" context shown when it exceeds current streak
affects: [pwa-passport-page, retention-ui, customer-engagement]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "useEffect + state for date computation avoids hydration mismatch (new Date() differs server/client)"
    - "IIFE inside JSX for local const derivation (streakAtRisk, longestStreak) without polluting component scope"
    - "IST timezone suffix (+05:30) appended to date string before Date() parse for correct local midnight semantics"

key-files:
  created: []
  modified:
    - pwa/src/app/passport/page.tsx
    - pwa/src/lib/api.ts
    - crates/racecontrol/src/psychology.rs

key-decisions:
  - "useEffect pattern chosen over inline IIFE for daysLeft — avoids hydration mismatch from new Date() differing between SSR and client renders"
  - "Streak at risk condition: daysLeft <= 7 AND streak_weeks >= 1 — zero-streak customers don't get false urgency"
  - "Longest streak shown only when it exceeds current — motivational framing without cluttering steady-state display"
  - "ThreadRng scoped block fix (psychology.rs) committed in Plan 02 — was leftover uncommitted change from Plan 01"

patterns-established:
  - "Date urgency UI pattern: fetch date string, convert to IST Date in useEffect, store as number state for hydration safety"

requirements-completed: [RET-01]

# Metrics
duration: 15min
completed: 2026-03-21
---

# Phase 92 Plan 02: Retention Loops — Streak Urgency PWA Display Summary

**Passport page streak card enhanced with red border + days-remaining countdown when grace period within 7 days, and longest-streak motivational context**

## Performance

- **Duration:** 15 min
- **Started:** 2026-03-21T08:10:00Z
- **Completed:** 2026-03-21T08:25:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Extended `PassportData` TypeScript type with `longest_streak`, `last_visit_date`, `grace_expires_date` fields
- Streak card shows red Racing Red (#E10600) border when grace expires within 7 days (streak at risk)
- Countdown copy "Nd left" or "Expires today!" rendered in red below streak number
- Longest-streak "Best: Nw" line shown only when historical best exceeds current (motivational, not cluttering)
- Full Rust build confirmed clean (392 tests pass); pre-existing `test_exec_echo` failure documented as out-of-scope
- All 6 RET requirements (RET-01 through RET-06) verified present in codebase

## Task Commits

1. **Task 1: Enhance passport streak card with grace urgency and longest streak** - `fe16023` (feat)
2. **Task 2: Fix ThreadRng scope (leftover from Plan 01) + build verification** - `11f37b4` (fix)
3. **Chore: Update tsbuildinfo after TypeScript check** - `020cdbb` (chore)

## Files Created/Modified
- `pwa/src/app/passport/page.tsx` - Added daysLeft state + useEffect, enhanced streak card with urgency border/text/best
- `pwa/src/lib/api.ts` - Extended PassportData summary type with 3 new nullable fields
- `crates/racecontrol/src/psychology.rs` - ThreadRng scoped block fix (Plan 01 leftover, committed here)

## Decisions Made
- Used `useEffect` + state for `daysLeft` computation instead of inline IIFE — avoids Next.js hydration mismatch when `new Date()` differs between server render and client hydration
- `streakAtRisk` requires `streak_weeks >= 1` — prevents showing urgency to customers who have never built a streak
- Longest streak displayed only when `longestStreak > streak_weeks` — pure motivational signal, hidden during steady-state
- ThreadRng fix committed here (Plan 02) rather than amending Plan 01 commits — clean forward commit per GSD protocol

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Committed leftover ThreadRng scope fix from Plan 01**
- **Found during:** Task 2 (build verification)
- **Issue:** `psychology.rs` had an uncommitted modification — ThreadRng scoped block to avoid `!Send` across `.await` in tokio::spawn. The fix was documented in Plan 01 SUMMARY but never committed.
- **Fix:** Committed the change in a dedicated `fix(92-02)` commit
- **Files modified:** `crates/racecontrol/src/psychology.rs`
- **Verification:** `cargo build -p racecontrol-crate` exits 0
- **Committed in:** `11f37b4`

---

**Total deviations:** 1 auto-fixed (Rule 1 - pre-existing uncommitted fix)
**Impact on plan:** Essential for correct build state. No scope creep.

## Issues Encountered
- `server_ops::tests::test_exec_echo` fails with status 500 vs 200 — pre-existing failure unrelated to retention phase (remote exec test requires specific terminal auth). 392/393 tests pass. Not introduced by this plan.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 92 complete: all 6 RET requirements implemented (backend Plan 01) and surfaced to customers (PWA Plan 02)
- Backend retention functions (notify_pb_beaten_holders, maybe_grant_variable_reward, check_streak_at_risk, check_membership_expiry_warnings) are live in scheduler and post-lap hooks
- Passport page now shows streak urgency — customers will see red border + countdown when at risk of losing streak
- Ready for next phase

---
*Phase: 92-retention-loops*
*Completed: 2026-03-21*
