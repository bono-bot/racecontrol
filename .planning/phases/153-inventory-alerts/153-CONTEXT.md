# Phase 153: Inventory Alerts - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Three-channel low-stock alert system: WhatsApp (via comms-link), admin dashboard banner, and email (via Gmail). Fires once per threshold breach with cooldown/dedup to prevent spam.

</domain>

<decisions>
## Implementation Decisions

### Alert Channels
- WhatsApp: send via existing comms-link WebSocket relay (send-message.js pattern or direct WS)
- Dashboard: warning banner at top of /cafe admin page showing items below threshold
- Email: send via existing email_alerts module (Gmail OAuth)
- All three channels fire on the same threshold breach event

### Cooldown/Dedup
- Track last_alert_at per item — don't re-alert for same item within cooldown period (e.g., 4 hours)
- Alert fires when stock drops to or below threshold (on sale/restock check)
- Reset cooldown when item is restocked above threshold

### Trigger Point
- Check thresholds after stock decrement (Phase 154 ordering) and after restock
- For now, implement the alert infrastructure — Phase 154 will call it on sale

### Claude's Discretion
- Exact cooldown duration (4-8 hours recommended)
- Alert message formatting for each channel
- Whether to batch multiple low-stock items into one alert or send individually
- Dashboard banner styling and dismiss behavior

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/whatsapp_alerter.rs` — existing WhatsApp alert module
- `crates/racecontrol/src/email_alerts.rs` — existing email alert module
- `comms-link/send-message.js` — WhatsApp via comms-link WS relay
- `web/src/app/cafe/page.tsx` — admin page to add alert banner

### Integration Points
- `cafe.rs` — add check_low_stock_alerts() function called after stock changes
- New cafe_alerts.rs module or extend cafe.rs
- Dashboard: add API endpoint for low-stock items, banner component in page.tsx

</code_context>

<specifics>
## Specific Ideas

No specific requirements.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
