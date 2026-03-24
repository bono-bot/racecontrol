# Phase 184: rc-sentry Crash Handler Upgrade - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Upgrade rc-sentry's crash handler from blind restart-loop to 4-tier graduated recovery with spawn verification. Changes confined to rc-sentry crate (watchdog.rs, tier1_fixes.rs, debug_memory.rs, ollama.rs) plus a new recovery event reporter. No changes to rc-agent, racecontrol server, or frontends.

</domain>

<decisions>
## Implementation Decisions

### Spawn Verification
- Poll /health at 500ms intervals for 10s after spawn before declaring restart success
- HTTP 200 from /health endpoint = verified alive (anti-cheat safe, no process inspection)
- POST recovery event to server's /api/v1/recovery/events immediately after spawn attempt with spawn_verified: true/false
- Use rc-watchdog's Session 1 spawn path (WTSQueryUserToken + CreateProcessAsUser) for GUI process launches — never std::process::Command for interactive processes

### Graduated Recovery Flow
- Order: Tier 1 deterministic fixes → Tier 2 pattern memory lookup → restart → spawn verify → Tier 3 Ollama only if restart fails
- Distinguish server-down from crash: check server WebSocket state, set server_reachable flag in recovery event
- Keep MAINTENANCE_MODE threshold at 3 restarts in 10min, but exclude events where server_reachable=false (server-down disconnects never trigger pod lockout)
- Tier 4 WhatsApp escalation: after 3+ failed spawn-verified recovery attempts on same pod, POST to /api/v1/fleet/alert

### Pattern Memory
- Use existing debug-memory.json at C:\RacingPoint\ (debug_memory.rs already exists)
- Write patterns only after successful recovery: crash signature (panic + exit code + last phase) → fix applied
- Exact fingerprint match for pattern lookup (no fuzzy matching)

### Claude's Discretion
- Internal module organization within rc-sentry (how to split handle_crash refactoring)
- CrashContext struct extensions (which new fields to add for server_reachable)
- Test structure and mock patterns for spawn verification tests

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `watchdog.rs` — WatchdogState FSM (Healthy/Suspect/Crashed), health_check() via raw TcpStream HTTP, CrashContext struct
- `tier1_fixes.rs` — RestartTracker (3-in-10min, backoff steps), deterministic fixes, MAINTENANCE_MODE/GRACEFUL_RELAUNCH sentinel handling
- `debug_memory.rs` — Pattern memory with debug-memory.json persistence
- `ollama.rs` — Tier 3 LLM query to James .27:11434
- `rc-common/src/recovery.rs` — RecoveryAuthority, RecoveryEvent struct, ProcessOwnership registry
- `racecontrol/src/recovery.rs` — POST/GET /api/v1/recovery/events (Phase 183, just shipped)
- `racecontrol/src/fleet_alert.rs` — POST /api/v1/fleet/alert for WhatsApp escalation

### Established Patterns
- rc-sentry uses pure std (no tokio, no async) — all operations synchronous
- Health checks via std::net::TcpStream with connect_timeout/read_timeout
- Crash handling: watchdog detects → tier1_fixes::handle_crash() runs fixes → restart
- Anti-cheat safe: process kill by name (taskkill /IM), never PID; health poll only, never debug APIs

### Integration Points
- handle_crash() in tier1_fixes.rs is the main entry point — this is where Tier 2 and Tier 3 get wired in
- RestartTracker already counts restarts for MAINTENANCE_MODE — extend to exclude server_reachable=false
- Recovery event reporting: HTTP POST to server .23:8080/api/v1/recovery/events after each recovery attempt
- Fleet alert: HTTP POST to server .23:8080/api/v1/fleet/alert for Tier 4 escalation

</code_context>

<specifics>
## Specific Ideas

- The Tier 2 gap is one code change: insert DebugMemory::instant_fix() lookup between Tier 1 and restart in handle_crash()
- server_reachable can be determined by attempting TcpStream connect to server :8080 (same pattern as health_check)
- Spawn verification should use the same health_check() function already in watchdog.rs, just with different timeout
- Recovery event reporting is a simple HTTP POST (same TcpStream pattern used for health checks)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
