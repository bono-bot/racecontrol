# Phase 10: Process Supervisor - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Standalone process supervisor that monitors the comms-link daemon (james/index.js) and auto-restarts it mid-session if it crashes. Includes a Windows Task Scheduler watchdog-of-watchdog to supervise the supervisor itself. Replaces the deprecated ping-heartbeat.js entirely.

</domain>

<decisions>
## Implementation Decisions

### Health Check Strategy
- Health check via HTTP GET to `http://127.0.0.1:8766/relay/health` every 15 seconds
- Healthy = HTTP 200 + response body parses as valid JSON (catches port hijacking)
- WebSocket connection state is informational only -- WS disconnected is NOT a restart trigger (WS may be reconnecting temporarily)
- 3 consecutive health check failures trigger a restart (45 seconds of no response)
- Console logging with ISO timestamps on every health check result (consistent with watchdog-runner.js pattern)

### Restart Behavior (Claude's Discretion)
- Kill daemon processes before restart (use `tasklist` + `taskkill`, NOT deprecated `wmic`)
- Spawn daemon as detached process with `{ detached: true, stdio: 'ignore' }` + `child.unref()`
- Pass environment variables (COMMS_PSK, COMMS_URL, LOGBOOK_PATH, SEND_EMAIL_PATH) from supervisor's own env
- Wait for daemon to come up before self-test (2-3 seconds, matching watchdog.js pattern)
- Reuse EscalatingCooldown class from watchdog.js (5s, 15s, 30s, 60s, 5min steps)

### Single Instance Guard (Claude's Discretion)
- PID lockfile at `supervisor.pid` (separate from existing `watchdog.pid`)
- Check existing PID file on startup with `process.kill(pid, 0)` -- exit if supervisor already running
- Clean up PID file on graceful shutdown (process.on 'SIGINT', 'SIGTERM')

### Task Scheduler Watchdog-of-Watchdog (Claude's Discretion)
- Task Scheduler task runs every 5 minutes
- Check if supervisor process is alive (PID file + process.kill check)
- If supervisor is dead, restart it
- Register via `schtasks.exe` (same pattern as Claude Code watchdog)

### Migration from ping-heartbeat.js (Claude's Discretion)
- Remove `node ping-heartbeat.js` from `start-comms-link.bat`
- Add `node james/process-supervisor.js` instead
- ping-heartbeat.js kept in repo but marked deprecated (delete after v2.0 stabilizes)

### Claude's Discretion
- Exact process detection method (tasklist filter for james/index.js vs node.exe process tree)
- Self-test verification approach (HTTP health check on restarted daemon)
- Task Scheduler task naming and configuration details
- Whether to emit EventEmitter events (following ClaudeWatchdog pattern) or keep simpler
- Exact wait times between kill/spawn/self-test steps

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing supervisor (being replaced)
- `ping-heartbeat.js` -- Current daemon supervisor, 5-min poll, wmic usage, 3-failure threshold. Read to understand current behavior being replaced.

### Reusable patterns
- `james/watchdog.js` -- EscalatingCooldown class (lines 110-169), ClaudeWatchdog pattern (EventEmitter, DI, polling, restart flow). Primary pattern reference.
- `james/watchdog-runner.js` -- PID file guard (lines 395-422), event wiring, runner entry point. Pattern for supervisor runner.

### Daemon entry point
- `james/index.js` -- HTTP relay server on :8766, `GET /relay/health` returns `{ connected: boolean }`. Supervised process.

### Auto-start mechanism
- `start-comms-link.bat` -- HKLM Run key, spawns daemon + heartbeat. Must be updated to spawn supervisor instead.

### Research
- `.planning/research/PITFALLS.md` -- Pitfall #23 (duplicate process instances), Pitfall #25 (NTFS locking)
- `.planning/research/ARCHITECTURE.md` -- Phase 10 component design (process-supervisor.js)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EscalatingCooldown` (watchdog.js:110-169): Complete, tested escalating delay class with DI. Reuse directly for restart cooldown.
- `ClaudeWatchdog` (watchdog.js:189-305): EventEmitter-based watchdog with crash detection, zombie kill, auto-restart. Adapt pattern for daemon supervision.
- PID file guard (watchdog-runner.js:395-422): `process.kill(pid, 0)` check + file write. Copy pattern for supervisor PID.

### Established Patterns
- Dependency injection for all external calls (process detection, kill, spawn) -- enables full test coverage via mocking
- EventEmitter for lifecycle events (crash_detected, restart_success, etc.)
- `node:test` built-in test runner with Object.freeze enums
- ISO timestamp logging to console
- Detached process spawning with `child.unref()`

### Integration Points
- `start-comms-link.bat` -- Replace `node ping-heartbeat.js` with `node james/process-supervisor.js`
- `GET http://127.0.0.1:8766/relay/health` -- Daemon health endpoint (already exists)
- Task Scheduler -- Register new task for supervisor watchdog-of-watchdog (alongside existing Claude watchdog task)
- `RELAY_PORT` env var -- Configurable health check port (default 8766)

</code_context>

<specifics>
## Specific Ideas

- Follow the watchdog.js DI pattern closely for testability
- Use `tasklist /FI "IMAGENAME eq node.exe" /FO CSV` instead of deprecated `wmic` for process detection
- The supervisor itself should be as simple as possible -- it's a loop that checks health and restarts

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 10-process-supervisor*
*Context gathered: 2026-03-20*
