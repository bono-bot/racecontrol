# Phase 56: WhatsApp Alerting + Weekly Report - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

P0 events trigger a WhatsApp message to Uday within 60 seconds; a resolved message fires when the condition clears. Every Monday at 08:00 IST, an email lands in Uday's inbox summarizing the previous week's fleet performance (sessions, uptime %, credits, incidents).

</domain>

<decisions>
## Implementation Decisions

### P0 Event Definition
- All pods offline (ws_connected=false for every pod in fleet_health) is a P0
- Error rate threshold breach (ErrorCountLayer already fires via mpsc channel) is a P0
- Billing crash (unrecoverable billing error) is a P0
- Each P0 type has a distinct message template

### WhatsApp Delivery
- Reuse existing Evolution API pattern from billing.rs send_whatsapp_receipt()
- Same config fields: evolution_url, evolution_api_key, evolution_instance
- New config field: `[alerting] uday_phone = "919XXXXXXXXX"` in racecontrol.toml
- 5-second timeout, best-effort (never block the main loop)
- Rate limit: 1 WhatsApp alert per P0 type per 30 minutes (prevent spam during flapping)

### WhatsApp Message Format
- Concise operational format for non-technical recipient
- Alert: "[RP ALERT] {event_type} - {summary}. {pod_count} pods affected. {IST timestamp}"
- Resolved: "[RP RESOLVED] {event_type} cleared. All {pod_count} pods online. Duration: {minutes}m. {IST timestamp}"

### Recovery Notification
- Resolved fires when ALL pods reconnect after an all-pods-offline P0
- For error rate P0: resolved fires when no new threshold breach for 5 minutes
- Track P0 start time to include duration in resolved message

### Weekly Report Content
- Total sessions count for the week
- Uptime % per pod (calculated from fleet_health poll intervals or WS connected time)
- Total credits billed (sum of wallet_debit_paise)
- Numbered incident list from error_rate alert log (timestamps + descriptions)
- Period: Monday 00:00 to Sunday 23:59 IST

### Weekly Report Delivery
- Email via existing send_email.js shell-out pattern (same as EmailAlerter)
- Recipient: usingh@racingpoint.in (default_email_recipient)
- HTML table format for readability
- Scheduled via Windows Task Scheduler (same pattern as Phase 53 ONLOGON tasks)
- Monday 08:00 IST trigger

### Claude's Discretion
- Exact HTML template for weekly report email
- Whether to add uptime tracking table to SQLite or compute from logs
- Error rate incident log storage format (file vs SQLite)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### WhatsApp Integration
- `crates/racecontrol/src/billing.rs` L2168-2270 -- Evolution API send pattern (send_whatsapp_receipt)
- `crates/racecontrol/src/config.rs` L221-223 -- evolution_url, evolution_api_key, evolution_instance config fields

### Alerting Infrastructure
- `crates/racecontrol/src/error_rate.rs` -- ErrorCountLayer tracing subscriber, mpsc alert channel
- `crates/racecontrol/src/email_alerts.rs` -- EmailAlerter with per-pod and venue-wide rate limiting, send_email.js shell-out

### Event System
- `crates/racecontrol/src/bono_relay.rs` -- BonoEvent enum (PodOffline, PodOnline events already defined)
- `crates/racecontrol/src/pod_monitor.rs` -- Pod health monitoring, WS connection tracking
- `crates/racecontrol/src/state.rs` L86-87 -- bono_event_tx broadcast channel

### Scheduling
- `.planning/phases/53-deployment-automation/53-01-PLAN.md` -- Task Scheduler ONLOGON pattern from Phase 53

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Evolution API client pattern in billing.rs: reqwest POST with apikey header, 5s timeout, best-effort
- EmailAlerter: rate-limited email sending with per-pod and venue-wide cooldowns
- ErrorCountLayer: sliding window error counter with mpsc alert channel (already wired in Phase 54)
- BonoEvent enum: PodOffline/PodOnline events already defined and broadcast

### Established Patterns
- send_email.js shell-out for email delivery (config.watchdog.email_script_path)
- broadcast::Sender for event distribution (bono_event_tx)
- reqwest client with timeout for external API calls
- schtasks ONLOGON trigger for persistent scheduled tasks (Phase 53)

### Integration Points
- error_rate.rs alert_tx mpsc channel -- subscribe for error rate P0 events
- pod_monitor.rs -- detect all-pods-offline condition
- bono_event_tx broadcast -- receive PodOffline/PodOnline events
- racecontrol.toml [alerting] section -- new config for uday_phone and alert settings
- main.rs -- spawn whatsapp_alerter task alongside existing email_alerter

</code_context>

<specifics>
## Specific Ideas

- WhatsApp messages go to Uday (boss) who is non-technical -- keep language simple, no stack traces
- IST timestamps always (project-wide rule)
- Weekly report should be scannable in 30 seconds on a phone

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 56-whatsapp-alerting-weekly-report*
*Context gathered: 2026-03-20*
