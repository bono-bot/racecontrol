# Phase 103: Pod Guard Module - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Create `process_guard.rs` module in rc-agent that runs as a tokio background task. Scans running processes against the whitelist fetched from racecontrol, kills confirmed violations, audits HKCU/HKLM Run keys and Startup folder, logs all actions, and reports violations via WebSocket. Covers 11 requirements: PROC-01 through PROC-05, AUTO-01, AUTO-02, AUTO-04, ALERT-01, ALERT-04, DEPLOY-01.

</domain>

<decisions>
## Implementation Decisions

### Enforcement Behavior
- Two-cycle grace period: process must appear in 2 consecutive scans before kill action — prevents transient Windows processes
- Self-exclusion: first filter skips current PID + parent PID + any process named `rc-agent.exe` unconditionally
- PID identity verification: verify process name + creation time match before kill to prevent PID reuse race
- Pod binary guard: detect `racecontrol.exe` on a pod = CRITICAL severity, zero grace period (standing rule #2)
- Severity tiers: KILL (immediate after grace), ESCALATE (log + WS alert, wait for staff), MONITOR (log only)
- Auto-start audit runs on startup + every 5 minutes (not every scan cycle — registry reads are heavier)
- Backup removed entries to `C:\RacingPoint\autostart-backup.json` before deletion

### Deployment & Rollout
- Initial deploy in `report_only` mode — log violations without killing. Switch to `kill_and_report` after Pod 8 canary
- rc-agent fetches merged whitelist from racecontrol via `GET /api/v1/guard/whitelist/pod-{N}` on WS connect, falls back to empty whitelist (report-only) if fetch fails
- Log file: `C:\RacingPoint\process-guard.log` with 512KB rotation
- Violations reported via `AgentMessage::ProcessViolation` over existing WS channel

### Claude's Discretion
- Internal data structures for tracking consecutive scan hits
- How to integrate the background task with the existing event_loop.rs select! macro
- sysinfo::System refresh strategy (reuse existing instance or create new)
- Wildcard matching implementation for process names

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- kiosk.rs — ALLOWED_PROCESSES static slice, sysinfo::System usage, process scanning pattern
- self_monitor.rs — log rotation pattern (512KB cap)
- event_loop.rs — tokio::spawn background task pattern, select! macro integration
- ws_handler.rs — AgentMessage sending pattern
- self_heal.rs — winreg crate usage for registry read/write
- game_process.rs — taskkill /F /PID pattern for process killing

### Established Patterns
- Background tasks spawned via tokio::spawn in event_loop.rs
- Process enumeration via sysinfo::System::refresh_processes()
- WS message sending via channel (tokio::sync::mpsc)
- Config loaded from AppState, accessed via Arc<RwLock>

### Integration Points
- event_loop.rs — spawn process_guard background task
- app_state.rs — store fetched whitelist
- ws_handler.rs — send ProcessViolation messages
- config.rs — ProcessGuardConfig (already exists from Phase 102 on racecontrol side)

</code_context>

<specifics>
## Specific Ideas

- The guard should be a self-contained module with a `start()` function that takes the necessary channels/state
- Use the same sysinfo::System instance as kiosk.rs if possible to avoid duplicate process snapshots
- The whitelist fetch should happen during the WS connect handshake, not as a separate HTTP call during scanning

</specifics>

<deferred>
## Deferred Ideas

- LLM classification for ESCALATE-tier unknowns (v12.2)
- Auto-whitelisting workflow via staff approval (v12.2)

</deferred>
