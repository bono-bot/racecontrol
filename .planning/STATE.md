---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: unknown
stopped_at: Completed 05-01-PLAN.md
last_updated: "2026-03-12T05:02:02.384Z"
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 5
  completed_plans: 1
---

# Project State

## Project Reference
See: .planning/PROJECT.md (updated 2026-03-11)
**Core value:** Customers must never be at risk of wrist injury, and drivers must see their lap/sector data clearly.
**Current focus:** Phase 5
**Last session:** 2026-03-12T05:02:02.381Z
**Stopped at:** Completed 05-01-PLAN.md

## Current Phase
Phase 5: Watchdog Hardening — Plan 1 of 2 complete

## Progress
| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 1 | FFB Safety | Not Started | 4 requirements (FFB-01 to FFB-04), safety-critical |
| 2 | HUD Infrastructure | Not Started | 2 requirements (INFRA-01, INFRA-02), blocked by Phase 1 |
| 3 | HUD Layout and Display | Not Started | 9 requirements (HUD-01 to HUD-09), blocked by Phase 2 |
| 4 | HUD Data Accuracy | Not Started | 4 requirements (DATA-01 to DATA-04), blocked by Phase 3 |
| 5 | Watchdog Hardening | In Progress (1/2 plans) | Plan 01 done: EscalatingBackoff + EmailAlerter + WatchdogConfig + AppState |

## Decisions Log
- [05-01] EscalatingBackoff uses Vec<Duration> steps with clamping to last element for cap behavior
- [05-01] EmailAlerter enforces dual rate limits: per-pod 30min AND venue-wide 5min must both pass
- [05-01] Email sending uses 15s tokio timeout with kill_on_drop(true) to prevent blocking watchdog loop

## Known Issues (from Pod 8 test, Mar 11)
- **FFB too slow:** Wheelbase takes too long to zero after session ends — needs to be faster (zero BEFORE game kill, tighter timeout)
- **Timer not synced:** HUD timer starts before game launches. Fix: launch game FIRST, then start timer. Apply to ALL games.
- **Time format wrong:** Sector times and lap times must be MM:SS.mmm (hundredths of a second), not current format
- **No lap times showing:** Even invalid laps should display — show invalid laps in GREY
- **No RPM bar visible:** The full-width RPM bar is not rendering
- **RPM number too small:** RPM font size still needs significant increase
- **Deploy Session 0 issue (RESOLVED):** pod-agent starts rc-agent in Session 0. Fix: reboot pod after deploy, or user manually starts from Console.
- **Run key/auto-login:** Pod 8 may not have HKLM Run key or auto-login configured — verify all pods

## Accumulated Context

### Roadmap Evolution
- Phase 5 added: Watchdog Hardening — escalating cooldown, post-restart self-test, WebSocket re-establishment verification, email notifications

---
*Created: 2026-03-11*
