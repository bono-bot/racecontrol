# Phase 9: Health Monitoring + Alerts - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning
**Source:** Smart Discuss (infrastructure phase)

<domain>
## Phase Boundary

Add health monitoring and WhatsApp alerting for cloud services on Bono's VPS. When a PM2 process crashes/restarts or system resources are exhausted, Uday gets a WhatsApp alert within 2 minutes. PM2 already auto-restarts crashed processes — this phase adds the alerting layer.

**Key discovery:** VPS uses PM2 + nginx (not Docker containers). Success criteria references "containers" but should be read as "PM2 processes". Uptime Kuma already runs on port 3001 — can potentially be leveraged.

</domain>

<decisions>
## Implementation Decisions

### Monitoring Approach
- Use a lightweight cron-based health check script on the VPS (not a new service)
- Script checks: PM2 process status, disk usage, memory usage, swap usage
- Alert via existing WhatsApp Business API (Evolution API on VPS at port 53622)
- Check interval: every 2 minutes via cron
- Alert cooldown: don't re-alert for the same failure within 30 minutes

### What to Monitor
- PM2 processes: any process in "errored" or "stopped" state → alert
- PM2 restart count: if a process restarts >3 times in 10 minutes → alert (crash loop)
- Disk usage: >90% → alert
- Memory usage: >90% → alert
- Swap: not configured (0B) — add 2GB swap as part of this phase (was planned in Phase 1 but never applied)

### Alert Channel
- WhatsApp via Evolution API already running on VPS (port 53622)
- Send to Uday's number
- Also send to James via comms-link for logging

### Claude's Discretion
- Exact script implementation
- Whether to use Uptime Kuma instead of custom script
- Alert message format

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Evolution API on VPS port 53622 — existing WhatsApp integration
- Uptime Kuma on port 3001 — existing monitoring (may have notification channels)
- PM2 `jlist` command — JSON output of all process states
- Existing WhatsApp alerting in racecontrol (local) — similar pattern

### Integration Points
- Cron → health check script → WhatsApp API
- PM2 JSON API → process state detection

</code_context>

<specifics>
## Specific Ideas

- Keep it dead simple — a bash script + cron, not a new Node.js service
- 2GB swap still needs to be set up (missed in Phase 1)

</specifics>

<deferred>
## Deferred Ideas

None

</deferred>

---

*Phase: 09-health-monitoring-alerts*
*Context gathered: 2026-03-22 via Smart Discuss (infrastructure phase)*
