---
phase: 174-health-monitoring-unified-deploy
plan: 04
subsystem: infra
tags: [bash, deploy, health-check, racecontrol, kiosk, comms-link, rc-sentry]

# Dependency graph
requires:
  - phase: 174-01
    provides: racecontrol /api/v1/health endpoint (status ok response)
  - phase: 174-02
    provides: comms-link /health endpoint (status ok response)
provides:
  - check-health.sh — polls all 5 services with PASS/FAIL, exits non-zero on any failure
  - deploy.sh — unified deploy for racecontrol/kiosk/web/comms-link with post-deploy health verification
affects: [deploy operations, service health verification, CI/CD workflows]

# Tech tracking
tech-stack:
  added: []
  patterns: [post-deploy health gate pattern, unified deploy orchestration via bash case statement]

key-files:
  created:
    - deploy-staging/check-health.sh
    - deploy-staging/deploy.sh
  modified: []

key-decisions:
  - "comms-link health check targets localhost:8766 (relay runs on James .27), all other services target server .23"
  - "deploy.sh uses schtasks for server restarts (survives SSH disconnect per standing rule), pm2 fallback for kiosk/web"
  - "rc-agent excluded from deploy.sh — pod-specific deploy handled separately via RCAGENT_SELF_RESTART sentinel"

patterns-established:
  - "Post-deploy health gate: bash check-health.sh || { echo FAILED; exit 1; } called after every service deploy"
  - "Health check: curl -sf with --max-time timeout + grep for status:ok, PASS/FAIL per service, aggregate exit code"

requirements-completed: [HLTH-02, HLTH-03, DEPL-02]

# Metrics
duration: 8min
completed: 2026-03-23
---

# Phase 174 Plan 04: Health Monitoring & Unified Deploy Summary

**check-health.sh polls 5 services (racecontrol/kiosk/web/comms-link/rc-sentry) with PASS/FAIL output; deploy.sh orchestrates racecontrol/kiosk/web/comms-link deploys and gates each on health check**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-23T10:57:00+05:30
- **Completed:** 2026-03-23T11:05:00+05:30
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- check-health.sh polls all 5 services with per-service PASS/FAIL, exits 1 if any service down
- deploy.sh covers racecontrol (cargo build + SCP + schtasks), kiosk/web (git pull + schtasks/pm2), comms-link (git pull + pm2)
- Both scripts validated with bash -n syntax check and committed/pushed to deploy-staging

## Task Commits

Each task was committed atomically:

1. **Task 1: Create check-health.sh** - `388db85` (feat)
2. **Task 2: Create deploy.sh with post-deploy health check** - `7676a0f` (feat)

## Files Created/Modified
- `deploy-staging/check-health.sh` - Central health check: polls 5 services, PASS/FAIL per service, exits non-zero on failure
- `deploy-staging/deploy.sh` - Unified deploy orchestrator: per-service deploy steps + post-deploy health gate

## Decisions Made
- comms-link health check targets localhost:8766 — the comms-link relay runs on James (.27), not on the server
- deploy.sh uses `schtasks /Run /TN StartRCTemp` for racecontrol restart per standing rule (survives SSH disconnect)
- rc-agent deliberately excluded — pod deploys use RCAGENT_SELF_RESTART sentinel and are pod-specific, not centralized
- kiosk/web use `schtasks || pm2` fallback pattern to handle both scheduled task and pm2-managed environments

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- check-health.sh and deploy.sh are ready for live verification once server (.23) is online
- Live test: `bash deploy-staging/check-health.sh` from James (.27) → prints PASS/FAIL per service
- Live test: `bash deploy-staging/deploy.sh racecontrol` → builds, deploys, runs health check

---
*Phase: 174-health-monitoring-unified-deploy*
*Completed: 2026-03-23*
