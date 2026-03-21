# Phase 105: Port Audit + Scheduled Tasks + James Binary - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Three secondary enforcement vectors: listening port audit via netstat, scheduled task audit via schtasks, and a standalone rc-process-guard binary for James workstation that reports violations via HTTP POST to racecontrol. Covers PORT-01, PORT-02, AUTO-03, DEPLOY-03.

</domain>

<decisions>
## Implementation Decisions

### Port + Scheduled Task Audit
- Port audit: netstat -ano shell-out parsed for LISTENING state, match owning PID against whitelist allowed_ports
- Scheduled task audit: schtasks /query /fo CSV /nh parsed for non-system tasks
- Whitelisted scheduled tasks: RacingPoint-StagingHTTP, RacingPoint-WebTerm, CommsLink-Watchdog, RCAgent — all others flagged
- Both audits added to existing process_guard.rs in rc-agent (extend the module)
- Port audit runs every scan cycle (60s), schtasks audit runs every 5 minutes with autostart audit

### James Standalone Binary
- New crates/rc-process-guard/ crate in workspace — standalone binary importing rc-common types
- Reports via HTTP POST to http://192.168.31.23:8080/api/v1/guard/report (racecontrol adds POST endpoint)
- Fetches whitelist via GET /api/v1/guard/whitelist/james on startup + every 5 min refresh
- Same scan + autostart audit as pods but with James-specific whitelist (Ollama, node, python, VS Code, cargo, comms-link)
- Never uses WebSocket — standing rule: never run pod binaries on James
- Binary name: rc-process-guard.exe

### Claude's Discretion
- netstat output parsing implementation details
- schtasks CSV parsing approach
- rc-process-guard main.rs structure and error handling
- POST /api/v1/guard/report endpoint handler in racecontrol

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-agent/src/process_guard.rs — scan loop, kill, log rotation patterns to replicate
- rc-common types — MachineWhitelist, ProcessViolation already defined
- racecontrol process_guard.rs — whitelist endpoint already exists
- reqwest — already in workspace for HTTP client

### Established Patterns
- Shell-out via Command::new() for Windows tools (netstat, schtasks)
- Workspace crate structure with shared rc-common dependency
- HTTP client via reqwest (used elsewhere in the project)

### Integration Points
- racecontrol routes — add POST /api/v1/guard/report endpoint
- rc-agent process_guard.rs — extend with port_audit() and schtasks_audit()
- Cargo workspace — add rc-process-guard member

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond what research and context cover.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
