---
phase: 02-api-pwa-cloud-deploy
plan: 02
subsystem: infra
tags: [docker, pwa, deploy, caddy, vps, cloud]

requires:
  - phase: 02-api-pwa-cloud-deploy/02-01
    provides: Corrected compose.yml, Dockerfile build args, manifest scope
  - phase: 01-cloud-infrastructure
    provides: VPS with Docker Compose, Caddy, DNS, TLS
provides:
  - Customer PWA deployed at app.racingpoint.cloud
  - Deployment instructions sent to Bono via comms-link
  - PWA accessible over HTTPS with valid TLS certificate
affects: [phase-6-admin-deploy, phase-7-dashboard-deploy]

tech-stack:
  added: []
  patterns:
    - "Deploy coordination: James pushes code, sends instructions via comms-link, Bono pulls and rebuilds on VPS"

key-files:
  created: []
  modified:
    - comms-link/INBOX.md

key-decisions:
  - "Approved deployment with known API issue: api.racingpoint.cloud unreachable due to racecontrol binary not running on VPS host"
  - "PWA deployment proceeded independently of API availability"

patterns-established:
  - "VPS deploy cycle: push -> notify Bono via comms-link (websocket + INBOX.md) -> Bono pulls + rebuilds -> James verifies"

requirements-completed: [PWA-01, PWA-05]

duration: 45min
completed: 2026-03-22
---

# Phase 2 Plan 02: VPS Deployment Summary

**Customer PWA deployed at app.racingpoint.cloud via Docker Compose coordination with Bono; API unreachable pending racecontrol binary start on VPS**

## Performance

- **Duration:** ~45 min (includes Bono coordination wait time)
- **Started:** 2026-03-22T03:20:00+05:30
- **Completed:** 2026-03-22T04:05:00+05:30
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Deployment instructions sent to Bono via both comms-link websocket and INBOX.md audit trail
- Bono pulled latest code and ran Docker Compose on VPS
- PWA container (pwa + Caddy) running at app.racingpoint.cloud with HTTPS

## Task Commits

Each task was committed atomically:

1. **Task 1: Send deployment instructions to Bono** - `8693868` (chore) — in comms-link repo
2. **Task 2: Verify PWA and API live on cloud** - checkpoint:human-verify, approved with known issue

**Plan metadata:** (this commit)

## Files Created/Modified
- `comms-link/INBOX.md` - Phase 2 deployment instructions for Bono

## Decisions Made
- Approved deployment with known issue: api.racingpoint.cloud is unreachable because the racecontrol binary is not running on the VPS host. The Caddy reverse proxy for api.racingpoint.cloud routes to host.docker.internal:8080, but racecontrol needs to be started on the VPS.
- PWA-01 and PWA-05 (PWA serving + installability) are met. PWA-02 through PWA-04 and API-01/API-02 require the racecontrol binary to be running on VPS and will be verified once that is resolved.

## Deviations from Plan

### Known Issues at Approval

**1. api.racingpoint.cloud unreachable**
- **Found during:** Task 2 (checkpoint verification)
- **Issue:** The racecontrol binary is not running on the VPS host at port 8080. Caddy proxies api.racingpoint.cloud to host.docker.internal:8080, but nothing is listening.
- **Impact:** API-dependent features (login, wallet, sessions, leaderboards) cannot be verified from cloud PWA
- **Resolution:** Racecontrol binary needs to be started on VPS. This is a separate operational task, not a code issue.
- **Requirements deferred:** PWA-02, PWA-03, PWA-04, API-01, API-02 remain unchecked until API is live

---

**Total deviations:** 1 known issue (API host binary not running)
**Impact on plan:** PWA deployment itself succeeded. API availability is an operational dependency outside this plan's scope (requires starting racecontrol binary on VPS host).

## Issues Encountered
- api.racingpoint.cloud returns connection errors because racecontrol binary is not running on VPS host. This is not a configuration or code issue — the Docker Compose setup and Caddy routing are correct. The racecontrol binary simply needs to be started on the VPS.

## User Setup Required
None - no external service configuration required. Starting racecontrol on VPS is an operational task for a future plan or manual action.

## Next Phase Readiness
- PWA container infrastructure is proven and running
- Once racecontrol binary is started on VPS, all Phase 2 success criteria will be met
- Phase 6 (Admin) and Phase 7 (Dashboard) can follow the same deploy pattern established here
- Phases 3, 4, 5 (sync hardening, booking, kiosk) are already complete from prior work

## Self-Check: PASSED
- SUMMARY.md: FOUND
- Task 1 commit (8693868 in comms-link repo): FOUND
- Task 2: checkpoint:human-verify approved by user

---
*Phase: 02-api-pwa-cloud-deploy*
*Completed: 2026-03-22*
