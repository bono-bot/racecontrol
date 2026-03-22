# Phase 136: Chain Endpoint + Visibility - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix /relay/chain/run timeout (chain_result not routed to broker), enhance /relay/health with connection details, return 503 on /relay/exec/run when WS is down, and update exec skills with health probes.

All implementation in C:/Users/bono/racingpoint/comms-link (james/index.js + skills).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key guidance:
- CHAIN-10/11: james/index.js already has a chain_result WS handler (line ~640). It calls execResultBroker.handleResult() but uses chainId as the key. The /relay/chain/run endpoint also waits on chainId. The issue is likely that the WS handler's handleResult call format doesn't match what waitFor expects, or the handler runs before the HTTP endpoint sets up the waitFor. Debug by reading both paths.
- VIS-01: /relay/health currently returns {"connected":true}. Add connectionMode state and last heartbeat timestamp from healthMonitor.
- VIS-02: /relay/exec/run already checks `if (!sent)` and returns 503. But the `client.send()` might return true even when WS is disconnected (queued). Need to check connectionMode.currentMode before sending.
- VIS-03: Update .claude/skills/rp-bono-exec/SKILL.md with guidance to check /relay/health first.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- james/index.js — existing chain_result handler (~line 640), /relay/health handler (~line 715), /relay/exec/run handler
- shared/exec-result-broker.js — ExecResultBroker.waitFor(id, timeoutMs) / handleResult(payload)
- shared/connection-mode.js — ConnectionMode with currentMode property
- james/health-monitor.js — HealthMonitor with lastHeartbeat

### Integration Points
- james/index.js chain_result WS handler — needs broker routing fix
- james/index.js /relay/health — needs enhanced response
- james/index.js /relay/exec/run — needs connection check
- .claude/skills/rp-bono-exec/SKILL.md — needs health probe guidance

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase with known fixes.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
