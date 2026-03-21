# Phase 104: Server Guard Module + Alerts - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Server-side violation handling in racecontrol: receive ProcessViolation from pods via WS, store in-memory per-pod, surface in fleet health endpoint, display kiosk badge, email escalation on repeat offenders, and run the server's own process guard (detect rc-agent.exe on server = CRITICAL).

</domain>

<decisions>
## Implementation Decisions

### Server Guard + Alert Design
- In-memory `HashMap<u8, VecDeque<ProcessViolation>>` in FleetHealthStore — per-pod, capped at 100 entries, no DB schema
- Repeat offender threshold: 3 kills of same process within 5 minutes triggers email to Uday via send_email.js
- Kiosk badge: add `violation_count` field to existing pod status WS broadcast — kiosk reads it in fleet grid
- Server's own guard: extend existing racecontrol process_guard.rs (from Phase 102) with a scan loop matching rc-agent's pattern — detect rc-agent.exe on server as CRITICAL
- `GET /api/v1/fleet/health` extended with `violation_count_24h` and `last_violation_at` per pod

### Claude's Discretion
- How to wire the ProcessViolation WS handler arm in racecontrol's ws message handling
- Whether to add a dedicated `/api/v1/guard/violations` endpoint or just extend fleet/health
- Email body format for repeat offender alerts
- Server guard scan interval (recommend same 60s as pods)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- racecontrol/src/process_guard.rs — already has merge logic + whitelist endpoint from Phase 102
- FleetHealthStore pattern — in-memory per-pod state with aggregation
- Email alerting via send_email.js shell-out — existing pattern in alert code
- WS broadcast to kiosk — existing pattern for pod status updates

### Established Patterns
- WS message handling in racecontrol matches on AgentMessage variants
- Fleet health endpoint serializes PodFleetStatus structs
- Email alerts use Command::new("node").arg(send_email_path) pattern

### Integration Points
- racecontrol WS handler — add ProcessViolation match arm
- FleetHealthStore — add violation tracking fields
- fleet/health endpoint — extend response with violation data
- Kiosk Next.js — read violation_count from pod status

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard server-side wiring following existing patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
