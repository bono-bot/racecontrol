# Phase 171: Bug Fixes - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix 4 known bugs blocking daily operations: pods DB auto-seed on startup, orphan PowerShell kill on boot, process guard allowlist enablement, and Variable_dump.exe kill on boot. Code changes done now, deployment verification deferred until pods come online.

</domain>

<decisions>
## Implementation Decisions

### BUG-01: Pods DB Auto-Seed
- racecontrol server must auto-seed the pods table on startup when empty
- Currently: POST /api/v1/pods/seed (staff JWT required) creates 8 pod records manually
- Fix: on startup, if pods table is empty, auto-insert the 8 pod records
- Pod IPs from racecontrol.toml or hardcoded network map (192.168.31.x)
- No auth required for auto-seed — it's an internal startup operation

### BUG-02: Orphan PowerShell Kill
- start-rcagent.bat must kill orphan powershell.exe on boot
- Root cause: self_monitor.rs relaunch_self() uses PowerShell+DETACHED_PROCESS which leaks ~90MB per restart
- Fix: add `taskkill /F /IM powershell.exe` to start-rcagent.bat before launching rc-agent
- Must NOT kill the PowerShell running the bat itself
- Deploy to all 8 pods (deferred — pods offline)

### BUG-03: Process Guard Allowlist
- Scan all pods for running processes, build allowlist, enable process guard in report_only mode
- Pods offline — cannot scan. Write the enablement code, defer actual scan + deploy
- Keep Variable_dump.exe OUT of allowlist (causes game crashes)
- Set enabled = true, violation_action = "report_only" in racecontrol.toml

### BUG-04: Variable_dump.exe Kill
- start-rcagent.bat must kill Variable_dump.exe on boot
- Root cause: VSD Craft spawns Variable_dump.exe which crashes on pedal input, disrupts USB HID
- VSD Craft and SGP Sync App must STAY (needed for pedal PIN input)
- Only Variable_dump.exe needs killing
- Deploy to all 8 pods (deferred — pods offline)

### Deployment Note
- Server (.23) and all pods are currently offline
- Code changes and bat updates written now
- Deployment and verification deferred until infrastructure is back online
- Verification will be marked as human_needed

### Claude's Discretion
- Exact SQL for auto-seed migration
- How to detect "empty pods table" (COUNT query vs startup hook)
- Bat file kill command ordering (Variable_dump before powershell)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- deploy-staging/start-rcagent.bat — existing pod boot script
- crates/racecontrol/src/ — server startup code
- racecontrol.toml — server config with process guard settings

### Established Patterns
- SQLite migrations in crates/racecontrol/
- .bat files: clean ASCII + CRLF, goto labels (no parentheses in if/else)
- Standing rule: smallest reversible fix first

### Integration Points
- start-rcagent.bat deployed to C:\RacingPoint\start-rcagent.bat on each pod
- racecontrol.toml deployed to C:\RacingPoint\racecontrol.toml on server
- Auto-seed runs at racecontrol startup, before WebSocket connections

</code_context>

<specifics>
## Specific Ideas

- BUG-02/04 can share the same bat update — add both kills to start-rcagent.bat
- BUG-01 should use the same pod data as POST /api/v1/pods/seed
- BUG-03 enablement is a config change in racecontrol.toml

</specifics>

<deferred>
## Deferred Ideas

- Full process guard allowlist from live pod scan (needs pods online)
- Deploy bat files to all 8 pods (needs pods online)
- Visual verification of pod boot sequence (needs pods online)

</deferred>
