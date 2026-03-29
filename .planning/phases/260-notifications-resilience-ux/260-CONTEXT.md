# Phase 260: Notifications, Resilience & UX - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped)

<domain>
## Phase Boundary

Notifications are durable, hardware disconnects are detected, anomalies are caught early, and customers have a reliable queue and receipt experience. Final phase of v27.0.

Requirements: UX-01 (notification outbox), UX-02 (OTP fallback), UX-03 (customer receipt), UX-04 (leaderboard integrity), UX-05 (leaderboard segmentation), UX-06 (lap evidence), UX-07 (telemetry adapter crash handling), UX-08 (queue management), RESIL-04 (hardware heartbeat), RESIL-05 (negative balance alert), RESIL-06 (crash rate anomaly), RESIL-07 (controls.ini reset), RESIL-08 (clock sync)

Depends on: Phase 252 (receipts need committed session data), Phase 254 (PII masking), Phase 255 (receipt includes GST breakup)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
Key guidance:
- UX-01: Create notification_outbox table (id, recipient, channel, payload, status, retry_count, next_retry_at, created_at). Background worker processes pending notifications with exponential backoff. Status: pending→sent→delivered→failed→exhausted.
- UX-02: If WhatsApp delivery fails after 2 retries, generate on-screen OTP display URL (one-time token). Fallback chain: WhatsApp → on-screen display.
- UX-03: After session end, auto-generate receipt JSON (driver, session, duration, charges, GST breakup from Phase 255, refund if any, before/after balance). Endpoint: GET /customer/sessions/{id}/receipt.
- UX-04: Leaderboard entries only created by server from verified session records. No manual entry endpoint. verify_lap_submission() checks session_id exists + has verified telemetry.
- UX-05: Leaderboard queries support filters: game, track, car_class, assist_tier. Separate board per combination.
- UX-06: Create lap_events table (session_id, lap_number, lap_time_ms, sector_times, validity, assist_config_hash, created_at). Populated from telemetry adapter.
- UX-07: If telemetry adapter crashes mid-lap, mark affected laps with validity='unverifiable'. Never silently drop.
- UX-08: Queue management: virtual_queue table (driver_id, position, estimated_wait_minutes, status). PWA endpoint to join/check queue.
- RESIL-04: Agent polls USB devices every 5s via sysinfo. If wheel/pedal VID:PID disappears, pause billing + alert staff.
- RESIL-05: After any wallet debit, check balance. If negative, ERROR log + WhatsApp alert + block further sessions.
- RESIL-06: Track crash_events per pod. If >3 in 1 hour, set pod maintenance flag + alert.
- RESIL-07: In ac_launcher.rs, write fresh controls.ini FFB/control config EVERY launch (already done partially — ensure no leakage).
- RESIL-08: Agent sends local timestamp in heartbeat. Server compares with own clock. If drift >5s, WARN log in fleet health.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/whatsapp_alerter.rs` — existing WhatsApp sending
- `crates/racecontrol/src/api/routes.rs` — session endpoints, leaderboard
- `crates/racecontrol/src/billing.rs` — session end, receipt data
- `crates/racecontrol/src/accounting.rs` — GST journal entries from Phase 255
- `crates/rc-agent/src/ac_launcher.rs` — controls.ini writing
- `crates/rc-agent/src/event_loop.rs` — heartbeat, telemetry processing

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
