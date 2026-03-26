# Phase 211: Safe Scheduling Foundation - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Register auto-detect.sh in Windows Task Scheduler (James) and correct Bono cron timing. Add PID file run guard, escalation cooldown state, sentinel-awareness before fixes, and venue-state-aware mode selection. Foundation scripts already exist (auto-detect.sh committed b54e4585, bono-auto-detect.sh deployed to VPS).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase.
- PID lock: file-based in /tmp or result directory
- Cooldown state: JSON file tracking last-alert timestamps per pod+issue
- Sentinel check: read OTA_DEPLOYING + MAINTENANCE_MODE via safe_remote_exec before each fix
- Venue state: reuse audit framework's venue_state_detect() function
- Task Scheduler: register via schtasks or PowerShell, daily trigger at 02:30

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- scripts/auto-detect.sh -- 6-step pipeline (already committed b54e4585)
- scripts/bono-auto-detect.sh -- Bono failover (deployed to VPS, cron partially active)
- audit/lib/core.sh -- venue_state_detect(), safe_remote_exec(), ist_now()
- audit/lib/fixes.sh -- is_pod_idle(), check_pod_sentinels(), APPROVED_FIXES
- audit/lib/notify.sh -- WhatsApp + Bono WS notification functions
- scripts/register-james-watchdog.bat -- existing Task Scheduler registration pattern

### Established Patterns
- PID file locking: comms-link watchdog uses PID file guard
- Sentinel files: OTA_DEPLOYING, MAINTENANCE_MODE, GRACEFUL_RELAUNCH
- Cooldown: notify.sh has 5-minute per-app cooldown (ALERT_COOLDOWN_SECS = 300)
- Venue detection: audit/lib/core.sh venue_state_detect() returns "open" or "closed"

### Integration Points
- auto-detect.sh Step 1 (audit) already calls audit.sh
- auto-detect.sh Step 6 (notify) already sends WS + INBOX.md
- Bono cron at 0 21 * * * (needs correction to 5 21 * * * for 5-min offset)

</code_context>

<specifics>
## Specific Ideas

No specific requirements -- infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope.

</deferred>
