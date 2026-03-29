---
phase: 260-notifications-resilience-ux
plan: 02
subsystem: database, api, leaderboard
tags: [sqlite, leaderboard, assist-tracking, sha256, integrity, segmentation]

# Dependency graph
requires:
  - phase: 252-financial-atomicity-core
    provides: billing_sessions table + active billing timers (UX-04 gate depends on these)
  - phase: 256-game-specific-hardening
    provides: kiosk_experiences table + car_class column on laps (leaderboard context)
provides:
  - laps table: assist_config_hash, assist_tier, billing_session_id, validity columns
  - kiosk_experiences table: assist_config column for per-experience assist defaults
  - compute_assist_evidence(): SHA-256 fingerprint + pro/semi-pro/amateur/unknown tier derivation
  - mark_laps_unverifiable(): UX-07 telemetry adapter crash handler
  - public_leaderboard: game + car_class + assist_tier filters + UX-04/UX-07 integrity gates
  - public_track_leaderboard: assist_tier in response + all integrity filters
  - track_leaderboard (staff): consistent car_class + assist_tier filters
affects:
  - 260-01: notification_outbox (shares db/mod.rs migrations section)
  - 260-03: rc-agent resilience (may call mark_laps_unverifiable on adapter disconnect)
  - future-leaderboard-frontend: can now segment by assist_tier in public API

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "UX-04 integrity gate: billing_session_id IS NOT NULL in lap INSERT + leaderboard queries = manual entry structurally impossible"
    - "UX-07 unverifiable marking: UPDATE validity='unverifiable' WHERE session_id=? AND lap_number>=? AND validity='valid'"
    - "Assist evidence: SHA-256 of BTreeMap-serialized JSON for canonical ordering = reproducible fingerprint"
    - "Tier derivation: ideal_line=true -> amateur; TC=SC=ABS=0 -> pro; otherwise semi-pro"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lap_tracker.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "billing_session_id lookup from active_timers by pod_id (not LapData) — avoids protocol change, reuses existing billing state"
  - "assist_config sourced from kiosk_experience.assist_config column (new) — per-experience fallback until telemetry sends per-lap config"
  - "validity column added to laps (NOT NULL DEFAULT 'valid') — coexists with existing 'valid' BOOLEAN for backward compat"
  - "ideal_line = amateur (strongest assist); TC+SC+ABS all zero = pro; any other = semi-pro (UX-06 derivation rule)"
  - "UX-04 gate rejects lap at persist_lap() level (not just query) — structural impossibility, no code path exists"
  - "billing_session_id IS NOT NULL uses LEFT JOIN laps in track_records query (tr.lap_id may predate this column)"

patterns-established:
  - "Leaderboard integrity triple: billing_session_id IS NOT NULL + validity='valid' + suspect=0"
  - "Assist tier is always included in leaderboard response JSON for frontend display"

requirements-completed: [UX-04, UX-05, UX-06, UX-07]

# Metrics
duration: 35min
completed: 2026-03-29
---

# Phase 260 Plan 02: Leaderboard Integrity Summary

**Lap assist evidence (SHA-256 hash + pro/semi-pro/amateur tier), billing-session gate blocking manual entry, and assist_tier segmentation across all three leaderboard endpoints**

## Performance

- **Duration:** 35 min
- **Started:** 2026-03-29T05:30:00Z
- **Completed:** 2026-03-29T06:05:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Laps now carry immutable assist evidence: SHA-256 fingerprint + derived tier from TC/SC/ABS/ideal_line config
- UX-04 integrity gate in persist_lap(): laps without an active billing session are rejected before INSERT
- UX-07 mark_laps_unverifiable(): batch UPDATE for adapter crash scenarios, WARN-logged
- All three leaderboard endpoints (public, public/track, staff/track) segmented by game + car_class + assist_tier
- `billing_session_id IS NOT NULL AND validity='valid'` hardened into all leaderboard SQL — manual entry structurally impossible

## Task Commits

Each task was committed atomically:

1. **Task 1: Lap evidence schema + assist tracking + unverifiable marking** - `a4059766` (feat)
2. **Task 2: Leaderboard segmentation by game + track + car_class + assist_tier** - `b7de59f6` (feat)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` — ALTER TABLE laps (4 columns) + ALTER TABLE kiosk_experiences (assist_config) + index
- `crates/racecontrol/src/lap_tracker.rs` — compute_assist_evidence(), mark_laps_unverifiable(), UX-04 gate, updated INSERT
- `crates/racecontrol/src/api/routes.rs` — PublicLeaderboardQuery + LeaderboardQuery + StaffTrackLeaderboardQuery all updated

## Decisions Made

- billing_session_id is looked up from `state.billing.active_timers` by pod_id (same source as car_class lookup). LapData doesn't carry it — avoids rc-common protocol changes for this plan.
- assist_config is sourced from `kiosk_experiences.assist_config` (new column). This is per-experience, not per-lap. Future phases can add per-lap assist config from telemetry.
- validity column added as `TEXT NOT NULL DEFAULT 'valid'` — coexists cleanly with the existing `valid BOOLEAN` column. Different semantics: `valid` = telemetry flag from agent, `validity` = server lifecycle status.
- In leaderboard track_records JOIN: used `LEFT JOIN laps ON l.id = tr.lap_id` with `(l.billing_session_id IS NOT NULL OR tr.lap_id IS NULL)` — old records without a lap_id still appear, new laps require billing session.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added kiosk_experiences.assist_config column**
- **Found during:** Task 1 (assist tracking implementation)
- **Issue:** lap_tracker.rs queries `ke.assist_config` but the column didn't exist in kiosk_experiences schema
- **Fix:** Added `ALTER TABLE kiosk_experiences ADD COLUMN assist_config TEXT` migration before the laps migrations
- **Files modified:** crates/racecontrol/src/db/mod.rs
- **Verification:** Column referenced in query matches migrated schema
- **Committed in:** a4059766 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 missing critical column)
**Impact on plan:** Essential for correctness — query would have failed at runtime without the column. No scope creep.

## Issues Encountered

- Pre-existing compile errors from parallel plan changes (`agent_timestamp` in PodInfo, `PodAlert` in DashboardEvent) were in unrelated files (main.rs, ws/mod.rs). My changes to lap_tracker.rs, db/mod.rs, and routes.rs compiled without errors.

## User Setup Required

None - schema migrations are idempotent (ALTER TABLE wrapped in `let _ =`). No external service configuration required.

## Next Phase Readiness

- UX-04, UX-05, UX-06, UX-07 requirements complete
- mark_laps_unverifiable() is ready to be called from rc-agent event_loop (260-03) on adapter disconnect
- Leaderboard endpoints accept `assist_tier` query param — frontend can add picker immediately
- kiosk_experiences can be updated with assist_config JSON per experience to populate tier data

## Self-Check: PASSED

- SUMMARY.md: FOUND
- lap_tracker.rs: FOUND
- db/mod.rs: FOUND
- routes.rs: FOUND
- Commit a4059766: FOUND
- Commit b7de59f6: FOUND

---
*Phase: 260-notifications-resilience-ux*
*Completed: 2026-03-29*
