# Phase 189: Core Scaffold and Shared Primitives - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the audit runner framework: `audit/audit.sh` entry point with mode parsing and prerequisites, `audit/lib/core.sh` shared library with result emission, HTTP helpers, remote exec wrappers (cmd.exe quoting safety, curl quote stripping, SSH banner protection), JSON schema definition, venue-open/closed detection, and IST timestamp conversion. One proof-of-concept phase (Phase 1: Fleet Inventory) runs end-to-end to validate the framework.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from REQUIREMENTS.md and research:
- Pure bash, no compiled deps (jq is the only external tool)
- Result JSON schema must include: phase, tier, host, status, severity, message, timestamp, mode, venue_state
- cmd.exe quoting: use bat-file wrapper pattern or JSON file + curl -d @file for rc-agent /exec
- curl sanitization: always pipe through `tr -d '"'` or use `jq -r`
- Auth PIN from `$AUDIT_PIN` env var, never hardcoded
- Venue detection: fleet health API (any active billing session = open), time-of-day fallback (09:00-22:00 IST)
- All files go in `audit/` directory within racecontrol repo

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- AUDIT-PROTOCOL v3.0 (`AUDIT-PROTOCOL.md`) — all 60 phase bash commands to port
- comms-link `send-message.js` — for Bono notifications (Phase 193)
- Fleet health API: `GET http://192.168.31.23:8080/api/v1/fleet/health`
- Auth endpoint: `POST http://192.168.31.23:8080/api/v1/terminal/auth`
- rc-agent exec: `POST http://<pod_ip>:8090/exec` with `{"cmd":"..."}`
- rc-sentry exec: `POST http://<pod_ip>:8091/exec` with `{"cmd":"..."}`

### Established Patterns
- Pod IP array: `PODS="192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91"`
- Server IP: 192.168.31.23 (ports 8080 racecontrol, 8090 server_ops)
- James local services: comms-link :8766, go2rtc :1984, Ollama :11434
- Bono VPS: 100.70.177.44 (Tailscale), comms-link relay preferred over SSH

### Integration Points
- audit/audit.sh → lib/core.sh (sourced)
- audit/phases/tierN/phaseNN.sh → lib/core.sh functions
- Results → audit/results/YYYY-MM-DD_HH-MM/ directory

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Research recommends:
- `set -e` must NOT be used in the main runner (collects FAIL results without aborting)
- Background jobs use temp file per pod, assembled with jq after wait
- 200ms launch stagger for parallel pod queries

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
