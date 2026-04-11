---
gsd_state_version: 1.0
milestone: v40.0
milestone_name: Game Launch Reliability
status: verifying
stopped_at: Completed 361-03-PLAN.md
last_updated: "2026-04-11T02:08:00.901Z"
last_activity: 2026-04-11
progress:
  total_phases: 4
  completed_phases: 4
  total_plans: 4
  completed_plans: 4
  percent: 14
---

# Project State

## Project Reference

See: .planning/PROJECT.md

**Core value (v46.0):** Move game-launch verification from "is the game alive?" to "is it running correctly AND recording everything?" Close all 21 silent data-loss points (3 P0, 4 P1, 14 P2+) between kiosk session setup and race results so the venue can trust that "session succeeded" means "session succeeded correctly."

**Current focus:** Phase 363 — data-recording-verification

## Parallel Milestone (PAUSED)

**v47.0 Admin Dashboard Venue-Ready Hardening** is in progress and has been temporarily paused at user directive to finish v46.0 first. Its full state is preserved at `.planning/milestones/v47.0-STATE-snapshot.md`. When v46.0 ships, restore it via:

```bash
cp .planning/milestones/v47.0-STATE-snapshot.md .planning/STATE.md
```

v47.0 stopped at: Phase 346-01 complete; cutover (346-02) and phases 347-355 pending. 12 phases total, 2 complete, 1 partial, 19% progress.

## Current Position

Phase: 363 (data-recording-verification) — EXECUTING
Plan: 3 of 3 (363-02 complete; 363-03 is next)
Status: Phase complete — ready for verification
Last activity: 2026-04-11

Progress: [█░░░░░░░░░] 14% (v46.0 — 1 of 7 phases shipped, Phase 362 a9b5eaa3)

## Accumulated Context

### Milestone origin

v46.0 was opened retroactively on 2026-04-09 after Phase 362 (Layer 3 post-launch config verification) shipped ad-hoc the same day as build `a9b5eaa3` to all 8 pods. The milestone captures work to close 21 silent data-loss points identified in the prior `project_game_diagnostics_plan.md` analysis:

- **3 P0 (immediate data loss):** billing race on lap reject, `presetValidity` unused in kiosk, CSV fallback not auto-synced
- **4 P1 (likely data loss):** WS `let _ =` silent drops, `try_send` buffer overflow, no session→laps reconciliation, `TelemetryGap` ignored
- **14 P2+:** pre-lap telemetry discarded, suspect laps hidden, 10Hz cap, flush overflow, nightly cleanup orphans, no AI behavior validation, no concurrent-session guard, content drift invisible, and more

### Shipped phase (retroactive)

**Phase 362: Post-Launch Config Verification (Layer 3)** — build `a9b5eaa3`, all 8 pods, 2026-04-09. Pod 8 canary visually confirmed. SessionConfig struct + read_session_config() on 5 sim adapters (AC, ACR, F1 25, iRacing, LMU) + verify_launch_config Stage 5 + ConfigMismatchDetected WS + admin broadcast + WhatsApp alert. Atomic race.ini write, readback verification, AI car content validation. Session-type semantic-mismatch fix (kiosk "trackday" vs SHM "practice") + car/track name fuzzy matching.

**Known deferred tests (tracked as GLD-G-05 in Phase 367):**

1. Deliberate mismatch → WhatsApp alert E2E (no synthetic fire yet)
2. ACR (Assetto Corsa Evo) adapter runtime verification (built, no live launch)
3. LMU adapter runtime verification (built, no live launch)
4. 8-pod concurrent-mismatch load test

### Scope decisions (2026-04-09)

- **Phase numbering:** 361-367 (7 phases total — 1 shipped, 6 remaining). No collision with v47.0's 344-360.
- **Runs parallel with v47.0:** User directive to finish v46.0 before shipping v47.0.
- **Phase B retroactive:** Pre-marked `[x]` with evidence annotation in v46.0-ROADMAP.md and MILESTONES.md.
- **Subagent gates per phase:**
  - 361 (kiosk + admin UI): gsd-ui-researcher + gsd-ui-auditor, gsd-nyquist-auditor
  - 363 (billing + data): gsd-nyquist-auditor, MMA audit
  - 364 (telemetry hot path): gsd-nyquist-auditor, MMA audit
  - 365 (MMA integration): gsd-nyquist-auditor, MMA audit
  - 366 (fleet + admin UI): gsd-ui-researcher + gsd-ui-auditor, MMA audit
  - 367 (heavy admin UI): gsd-ui-researcher + gsd-ui-auditor, MMA audit before ship

### Phase wave plan

<<<<<<< Updated upstream
Last session: 2026-04-11T02:08:00.897Z
Stopped at: Completed 361-03-PLAN.md

**Phase 361-01 COMPLETE AND DEPLOYED (2026-04-11):**

- Server .23: build `4c6d53b2` — inventory endpoint + validity gate live
- All 8 pods: rc-agent `4c6d53b2` — /debug/content-dirs live
- Cloud (Bono VPS): racecontrol `f0e7089e` — inventory endpoint verified
- NYQUIST: PASS (11/11 unit tests + live regression test)
- v46.0 Phase A (361) Plan 01 DONE. Plans 02 (kiosk filtering) and 03 (drift detection) still pending.

=======
**Wave 1 — Foundation (no dependencies):**

- Phase 361: Kiosk preset filtering + server gate (closes 2 silent-loss points)
- Phase 363: Data recording verification (closes 3 P0s)
- Phase 364: Session quality monitor (closes 4 P1s)

>>>>>>> Stashed changes

**Wave 2 — Depends on Wave 1 data pipeline:**

- Phase 365: AI behavior validation via MMA
- Phase 366: Fleet intelligence (uses Phase 361 content drift groundwork)

**Wave 3 — Depends on everything:**

- Phase 367: Staff tools + Phase 362 retro-validation (GLD-G-05)

### Blockers/Concerns

- **Context budget:** Session that opened the milestone was already heavy. `/gsd:autonomous --from 361` must run from a fresh `/clear` state for H2/H3 compliance.
- **v47.0 pressure:** v47.0 has a hard venue-opening deadline. v46.0 must not drag — if any phase stalls, pause v46.0 and restore v47.0 STATE.md per the restore command above.
- **Phase 362 runtime tests:** GLD-G-05 in Phase 367 is the gating retro-test. If it fails, Phase 362 may need a follow-up fix before v46.0 can ship.

## Session Continuity

Last session: 2026-04-09T21:25:26.253Z
Stopped at: Completed 363-data-recording-verification-03-PLAN.md
Resume file: None
Restore v47.0 when done: `cp .planning/milestones/v47.0-STATE-snapshot.md .planning/STATE.md`
