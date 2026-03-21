# Phase 102: Watchdog Core - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Add a background watchdog thread to rc-sentry that polls rc-agent health (localhost:8090/health) every 5s, uses 3-poll hysteresis (15s) before declaring crash, and reads startup_log + stderr after crash to build CrashContext. Pure std::thread + std::net — no tokio.

Requirements: DETECT-01, DETECT-02, DETECT-05

</domain>

<decisions>
## Implementation Decisions

### Watchdog FSM
- States: Healthy, Suspect(consecutive_failures: u8), Crashed
- Transition: Healthy → Suspect(1) on first poll failure, Suspect(N) → Suspect(N+1), Suspect(3) → Crashed
- Transition: Suspect(N) → Healthy on successful poll (reset counter)
- After Crashed: build CrashContext, emit to channel, return to Healthy (Phase 103 handles the actual fix+restart)

### Health Polling
- HTTP GET via std::net::TcpStream to localhost:8090/health
- connect_timeout: 3s, read_timeout: 3s
- Poll interval: 5 seconds (std::thread::sleep)
- Anti-cheat safe: no process APIs, just TCP connection

### Log Reading
- Read C:\RacingPoint\rc-agent-startup.log (startup_log.rs writes here)
- Read C:\RacingPoint\rc-agent-stderr.log (bat file redirect — Phase 105)
- Extract: panic message, exit code if present, last startup phase
- If stderr log doesn't exist yet (Phase 105 not deployed), gracefully skip

### Claude's Discretion
- Module file structure within rc-sentry
- CrashContext struct design (internal to sentry, not shared via rc-common)
- Logging format and detail level

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- rc-sentry main.rs: 515 lines, single-file binary, std::net TCP server, 6 endpoints
- v10.0 hysteresis FSM pattern (comms-link health monitor) — same 3-poll approach
- ClaudeWatchdog in comms-link watchdog.js — polling pattern reference

### Established Patterns
- rc-sentry is pure std (no tokio, no async) — watchdog must follow this
- Non-blocking TcpListener with sleep-based accept loop
- SHUTDOWN_REQUESTED AtomicBool for graceful shutdown
- tracing for structured logging

### Integration Points
- Watchdog thread spawned in main() before the accept loop
- CrashContext needs to be accessible to Phase 103 (fix functions) — use a channel or shared state
- rc-common::types::SentryCrashReport (Phase 101) used when reporting to server (Phase 105)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase with clear design from research.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
