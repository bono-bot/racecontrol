# Phase 274: WhatsApp Escalation - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Tier 5 escalations and critical alerts reach Uday's WhatsApp within 30 seconds via Bono VPS Evolution API, with deduplication preventing alert fatigue. Fallback to comms-link INBOX.md if WhatsApp fails.

Requirements: ESC-01, ESC-02, ESC-03, ESC-04

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion. Key patterns from MMA council research:
- Evolution API v2: POST /message/sendText/:instance with apikey header
- Number format: international without + (e.g., 919XXXXXXXXX)
- Formatting: *bold*, _italic_, - bullets, ```monospace```
- Dedup by incident_id with 30-minute suppression window
- Fallback: comms-link INBOX.md entry + git push if WhatsApp send fails
- Route through Bono VPS relay (curl to localhost:8766/relay/exec/run with command)
- OR direct HTTP to Evolution API on Bono VPS (need to determine which approach)

### WhatsApp Alert Template
```
*Tier 5 Escalation*
Severity: {severity}
Pod: {pod_id}
Issue: {summary}
AI tried: {actions_list}
Impact: {impact}
Dashboard: {url}
```

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tier_engine.rs` — tier5_human_escalation() is a STUB returning TierResult::Stub
- `comms-link/send-message.js` — existing WS message sender to Bono
- Bono VPS relay: curl -s -X POST http://localhost:8766/relay/exec/run
- Fleet event bus (Phase 273) — FleetEvent::Escalated variant exists

### Integration Points
- tier_engine.rs:1790 — tier5_human_escalation() stub to replace
- main.rs — may need comms-link relay URL from config
- WhatsApp goes through Bono VPS Evolution API (ws://srv1422716.hstgr.cloud:8765)
- Uday's number: from racecontrol.toml or env var
- Evolution API instance name and API key: from Bono VPS config

### Approach Options
1. **Rust HTTP client (reqwest)** — rc-agent calls Bono VPS Evolution API directly via Tailscale
2. **Comms-link relay** — rc-agent sends WS message to James, James relays to Bono via comms-link
3. **Server-side** — racecontrol server handles WhatsApp (pods never talk to Evolution API directly)

Option 3 is best — matches standing rule "WhatsApp via Bono VPS, not direct from venue."
Server receives escalation via WS from pod, server calls Bono relay to send WhatsApp.

</code_context>

<specifics>
## Specific Ideas

- Add WhatsApp escalation to racecontrol server (not rc-agent on pods)
- Server receives Tier 5 FleetEvent via WS → formats message → calls Bono relay → Evolution API
- Dedup map on server side (HashMap<String, Instant> with 30-min TTL)
- Fallback: if relay fails, write to comms-link INBOX.md via git

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
