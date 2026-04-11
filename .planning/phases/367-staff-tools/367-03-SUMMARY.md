---
phase: 367-staff-tools
plan: 03
subsystem: api, ui
tags: [rust, axum, nextjs, telemetry, session-replay, admin-portal]

# Dependency graph
requires:
  - phase: 367-01
    provides: suspect session list and telemetry heatmap infrastructure
provides:
  - GET /admin/sessions/{id}/replay backend endpoint (manager-gated)
  - Admin portal page /sessions/[id]/replay with playback controls
  - EVENT_CAP = 10,000 guard preventing OOM on long sessions
  - lap_start / telemetry / lap_end event stream with truncated flag
affects: [367-04, 367-05, deploy-server-23, deploy-admin]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "telemetry_db pool fallback: use telemetry_db.as_ref().unwrap_or(&state.db)"
    - "event stream with cap: EVENT_CAP const + break 'outer label"
    - "client-side replay: setInterval at 16ms tick, speed multiplier on index advance"

key-files:
  created:
    - racingpoint-admin/src/app/(dashboard)/sessions/[id]/replay/page.tsx
  modified:
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "Manager-role gate for replay endpoint — operational QA data, not customer-visible"
  - "EVENT_CAP = 10,000 events hard cap — prevents OOM on sessions with dense telemetry"
  - "Client-side playback at 16ms tick with speed multiplier — no server-side streaming needed"
  - "telemetry_db pool fallback — matches Phase 251 dual-pool pattern"

patterns-established:
  - "lap_start / telemetry / lap_end event envelope for session playback"
  - "GaugeBar component pattern for live telemetry visualisation"

requirements-completed: [GLD-G-03]

# Metrics
duration: 25min
completed: 2026-04-11
---

# Phase 367 Plan 03: Session Replay Player Summary

**Manager-gated GET /admin/sessions/{id}/replay streams lap_start/telemetry/lap_end events (capped 10k) with a Next.js replay page featuring scrubber, 1x-10x speed, and live gauges**

## Performance

- **Duration:** 25 min
- **Started:** 2026-04-11T03:25:00Z
- **Completed:** 2026-04-11T03:55:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Backend route `GET /admin/sessions/{id}/replay` added to manager-role sub-router, returns ordered lap events with 10,000-event cap and truncation flag
- Admin portal page created at `/sessions/[id]/replay` with timeline scrubber, lap markers, 1x/2x/5x/10x speed controls, and live speed/gear/RPM/throttle/brake/steering gauges
- 959 racecontrol-crate lib tests pass, 0 failed; TypeScript clean (tsc --noEmit exit 0)

## Task Commits

1. **Task 01: Backend replay route** — already in `77fcb43b` (parallel plan pre-staged, verified present)
2. **Task 02: Admin replay page** — `2ad880a` (feat — new dynamic route page)

## Files Created/Modified

- `crates/racecontrol/src/api/routes.rs` — Route registration at manager sub-router + `session_replay_handler` function with EVENT_CAP=10_000, telemetry_db pool fallback, lap_start/telemetry/lap_end event construction
- `racingpoint-admin/src/app/(dashboard)/sessions/[id]/replay/page.tsx` — Full client-side replay page: GaugeBar component, scrubber with lap markers, play/pause/reset controls, 1x/2x/5x/10x speed buttons

## Decisions Made

- EVENT_CAP = 10,000 events: prevents OOM on very long sessions with dense telemetry data
- Manager-role gate: replay data is operational QA tooling, not customer-visible
- 16ms interval tick with speed multiplier for client-side playback — no server-side streaming needed for this volume
- telemetry_db pool fallback mirrors Phase 251 dual-pool pattern already established in `public_lap_telemetry`

## Deviations from Plan

None - plan executed exactly as written.

The backend route was already pre-staged by a parallel agent at commit `77fcb43b`. Verified all acceptance criteria passed before committing the admin frontend.

## Issues Encountered

- Initial edit to routes.rs targeted the wrong anchor (pre-existing 365 comment) — the route was already present from parallel agent `77fcb43b`. Confirmed via `git status` showing no diff, then verified via grep that route + handler were both present.
- Cargo check passed cleanly (0 errors); release build completed in 2m 21s.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- GLD-G-03 closed
- Deploy required: racecontrol binary rebuild + server .23 + cloud (Bono VPS); admin :3201 rebuild on server .23 + cloud
- Phase 367-04 (batch export) and 367-05 (retro-validation) can proceed independently

---
*Phase: 367-staff-tools*
*Completed: 2026-04-11*

## Self-Check

- [x] `racingpoint-admin/src/app/(dashboard)/sessions/[id]/replay/page.tsx` — FOUND (commit 2ad880a)
- [x] Backend route `session_replay_handler` — FOUND at routes.rs line 24932 (commit 77fcb43b)
- [x] Route registration `/admin/sessions/{id}/replay` — FOUND at routes.rs line 674
- [x] EVENT_CAP = 10_000 — FOUND at routes.rs line 24967
- [x] 959 tests pass 0 failed — CONFIRMED
- [x] tsc --noEmit exit 0 — CONFIRMED

## Self-Check: PASSED
