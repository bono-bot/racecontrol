---
phase: 09-health-monitoring-alerts
plan: 01
subsystem: infra
tags: [bash, cron, pm2, whatsapp, evolution-api, health-monitoring, swap]

requires:
  - phase: 01-cloud-infrastructure
    provides: VPS with PM2 + nginx, Evolution API on port 53622
provides:
  - Cron-based VPS health monitoring with WhatsApp + comms-link alerts
  - 2GB swap setup script for VPS memory management
affects: [cloud-operations, vps-maintenance]

tech-stack:
  added: [jq (JSON parsing in bash)]
  patterns: [cron health check with cooldown state files, Evolution API WhatsApp integration]

key-files:
  created:
    - cloud/health-check.sh
    - cloud/setup-swap.sh
  modified: []

key-decisions:
  - "Cooldown state stored in /tmp/rc-health-alerts/ with per-key timestamp files"
  - "Crash loop detection uses delta comparison of restart counts stored in /tmp/rc-health-state/"
  - "jq used for JSON message body escaping in WhatsApp curl calls"

patterns-established:
  - "VPS alert pattern: detect issue -> check cooldown -> send WhatsApp + comms-link -> update cooldown"
  - "Config placeholders at script top (CHANGE_ME) for secrets that Bono fills in on VPS"

requirements-completed: [INFRA-05]

duration: 2min
completed: 2026-03-22
---

# Phase 9 Plan 1: Health Monitoring + Alerts Summary

**Cron-based VPS health monitor detecting PM2 failures, crash loops, and resource exhaustion with WhatsApp + comms-link alerting and 30-min cooldown**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-22T03:14:03Z
- **Completed:** 2026-03-22T03:15:12Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- PM2 process state monitoring (errored/stopped detection) with WhatsApp and comms-link alerts
- Crash loop detection (>3 restarts in 10 minutes) using delta-based restart count tracking
- Disk and memory usage alerts at >90% threshold
- 30-minute per-failure-key cooldown to prevent alert storms
- 2GB swap setup script (idempotent, fstab-persistent)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create health-check.sh monitoring script** - `b6e23af` (feat)
2. **Task 2: Create swap setup script** - `0911bcd` (feat)

## Files Created/Modified
- `cloud/health-check.sh` - VPS health monitor: PM2 status, crash loops, disk/memory checks, WhatsApp + comms-link alerts (170 lines)
- `cloud/setup-swap.sh` - Idempotent 2GB swap creation with fstab persistence (26 lines)

## Decisions Made
- Cooldown state stored as timestamp files in /tmp/rc-health-alerts/ (one file per failure key)
- Crash loop detection stores restart counts in /tmp/rc-health-state/ and compares deltas
- jq used to safely escape alert message text for JSON payload in curl calls
- Each check function wrapped with `|| true` in main so one failure does not skip remaining checks

## Deviations from Plan

None - plan executed exactly as written.

## User Setup Required

On VPS (Bono must configure):
- Edit `cloud/health-check.sh` config section: set `EVOLUTION_API_KEY` and `UDAY_WHATSAPP` to real values
- Run `bash /root/racingpoint/racecontrol/cloud/setup-swap.sh` once to create swap
- Add cron entry: `*/2 * * * * /root/racingpoint/racecontrol/cloud/health-check.sh >> /var/log/rc-health-check.log 2>&1`
- Ensure `jq` is installed: `apt install -y jq`

## Issues Encountered
None

## Next Phase Readiness
- Health monitoring ready for VPS deployment after config values are set
- Phase 10 (final phase) can proceed independently

---
*Phase: 09-health-monitoring-alerts*
*Completed: 2026-03-22*
