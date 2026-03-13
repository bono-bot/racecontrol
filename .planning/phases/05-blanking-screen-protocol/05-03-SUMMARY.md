---
phase: 05-blanking-screen-protocol
plan: 03
subsystem: infra
tags: [rust, powershell, deploy, kiosk, anti-cheat, pod]

# Dependency graph
requires:
  - phase: 05-blanking-screen-protocol
    provides: LaunchSplash, screen-ordering fix, DIALOG_PROCESSES, PinSource, INVALID_PIN_MESSAGE, pod-lockdown.ps1
provides:
  - Phase 5 deployment deferred — all code complete, ready for manual pod deployment at venue
  - Verification deferred to on-site execution (anti-cheat, screen transitions, pod lockdown visual check)
affects: [venue-operations, pod-setup-procedures]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Deploy-and-verify plans may be deferred when pod network is unreachable — code verified via automated tests, hardware verification deferred to on-site execution

key-files:
  created: []
  modified: []

key-decisions:
  - "Phase 5 deployment deferred to manual execution at venue — pod network unreachable from development machine"
  - "Anti-cheat gate (iRacing, F1 25, LMU) pre-approved based on 05-02 test results — on-site confirmation still required before full 8-pod rollout"
  - "All Phase 5 requirements (SCREEN-01, SCREEN-02, SCREEN-03, AUTH-01, PERF-02) closed in plans 05-01 and 05-02 — no new code in 05-03"

patterns-established: []

requirements-completed: [SCREEN-01, SCREEN-02, SCREEN-03, AUTH-01, PERF-02]

# Metrics
duration: ~5min (deferred plan — no code execution)
completed: 2026-03-13
---

# Phase 5 Plan 03: Deploy to Pod 8 + Verification Summary

**Phase 5 deployment deferred to manual on-site execution — all code verified via automated tests (210 tests green), binary ready in deploy-staging/**

## Performance

- **Duration:** ~5 min (administrative close — no live pod deployment)
- **Started:** 2026-03-13
- **Completed:** 2026-03-13
- **Tasks:** 0 of 2 executed (both deferred)
- **Files modified:** 0

## Accomplishments
- Phase 5 code verified complete via automated test suite (210 tests across 3 crates)
- All 5 Phase 5 requirements closed by plans 05-01 and 05-02
- Deployment artifact ready: deploy-staging/rc-agent.exe and deploy/pod-lockdown.ps1
- User approved deferred deployment — will execute manually at venue when pod network is accessible

## Task Commits

No task commits — this plan was a deploy + verify plan. All code was committed in 05-01 and 05-02.

| Task | Status | Reason |
|------|--------|--------|
| Task 1: Build and deploy updated rc-agent to Pod 8 | DEFERRED | Pod network (192.168.31.x) unreachable from development machine |
| Task 2: Anti-cheat + visual verification checkpoint | DEFERRED | Depends on Task 1; user will verify on-site at venue |

## Files Created/Modified

None — no code changes in this plan.

## Decisions Made

- Deployment deferred to manual execution at venue. User acknowledged and approved.
- Code changes from 05-01 and 05-02 are complete and fully tested via automated test suite.
- Anti-cheat pre-approval from 05-02 summary carries forward; on-site confirmation with iRacing, F1 25, and LMU required before declaring full 8-pod rollout safe.

## Deviations from Plan

None — plan execution deferred by user decision (pod network unavailable from development machine). This is not a deviation from code correctness; it is a scheduling decision.

## Manual Deployment Checklist

When at venue, follow these steps to complete Phase 5 deployment:

**Step 1: Build and deploy rc-agent to Pod 8**
```bash
export PATH="$PATH:/c/Users/bono/.cargo/bin"
cd /c/Users/bono/racingpoint/racecontrol
cargo test -p rc-agent && cargo test -p rc-common && cargo test -p rc-core
cargo build -p rc-agent --release
cp target/release/rc-agent.exe /c/Users/bono/racingpoint/deploy-staging/
python3 -m http.server 9998 --directory /c/Users/bono/racingpoint/deploy-staging --bind 0.0.0.0
```

**Step 2: Deploy to Pod 8 via pod-agent**
- Kill old rc-agent on Pod 8
- Download new binary from http://192.168.31.27:9998/rc-agent.exe
- Verify binary size > 10MB
- Start new rc-agent

**Step 3: Deploy pod-lockdown.ps1 to Pod 8**
- Copy deploy/pod-lockdown.ps1 to deploy-staging/
- Download on Pod 8 via pod-agent /exec
- Run: `powershell -ExecutionPolicy Bypass -File C:\RacingPoint\pod-lockdown.ps1`

**Step 4: Verify on Pod 8 (in order)**
1. Anti-cheat: launch iRacing, F1 25, LMU — play 2 min each, verify no kick/ban
2. Screen transitions: confirm lock screen appears BEFORE game closes on session end
3. LaunchSplash: confirm branded "PREPARING YOUR SESSION" shows during game load
4. Pod lockdown: taskbar hidden, Win key blocked, Ctrl+Esc blocked
5. PIN error: confirm identical message on pod lock screen and kiosk

**Step 5: Roll out to all 8 pods after Pod 8 passes**
- Use rolling deploy via rc-core /api/deploy/rolling
- Or deploy to each pod individually via pod-agent /exec

## Issues Encountered

None — plan deferred cleanly. No blocking issues discovered.

## User Setup Required

**Phase 5 deployment required at venue.** All code is ready; only the on-site hardware verification step is pending.

Deploy commands:
```
# Pod lockdown apply:
powershell -ExecutionPolicy Bypass -File C:\RacingPoint\pod-lockdown.ps1

# Pod lockdown undo (if needed):
powershell -ExecutionPolicy Bypass -File C:\RacingPoint\pod-lockdown.ps1 -Undo
```

## Next Phase Readiness

- Phase 5 (Blanking Screen Protocol) code is COMPLETE — all 5 requirements closed (SCREEN-01, SCREEN-02, SCREEN-03, AUTH-01, PERF-02)
- Pending: on-site hardware deployment and verification on Pod 8 before 8-pod rollout
- No code blockers for any future phase
- The 5-phase reliability roadmap is code-complete; only on-site verification and rollout remain

---
*Phase: 05-blanking-screen-protocol*
*Completed: 2026-03-13*
