# Phase 135: Daemon Recovery - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Make the James comms-link daemon crash-proof and boot-proof. After a crash, it auto-restarts within 30s. After a reboot, it starts before any user interaction. Bono is notified on crash and recovery.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key guidance:
- James's existing watchdog is `C:\Users\bono\.claude\james_watchdog.ps1` — runs every 2 min via Task Scheduler "ClaudeWatchdog"
- Add comms-link process check to the existing watchdog (don't create a separate one)
- Boot start: HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run key (same pattern as rc-agent on pods)
- Start script: create `start-comms-link.bat` that runs `node james/index.js` from the correct CWD
- CWD must be `C:\Users\bono\racingpoint\comms-link` (imports use relative paths)
- Use `start /D C:\Users\bono\racingpoint\comms-link` pattern (same as start-rcagent.bat)
- Crash notification: use existing send_email.js or WhatsApp Evolution API (AlertManager pattern from comms-link v1.0)
- Recovery notification: watchdog sends "comms-link recovered" after successful restart
- .bat files: clean ASCII + CRLF (standing rule — use sed 's/$/\r/' via bash heredoc)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- C:\Users\bono\.claude\james_watchdog.ps1 — existing watchdog checking 6 services every 2 min
- comms-link/scripts/register-watchdog.js — Phase 4 pattern for Task Scheduler registration
- comms-link/scripts/register-supervisor.js — Phase 10 pattern for supervisor registration
- comms-link/james/watchdog-runner.js — existing watchdog runner with email notifications
- comms-link/shared/send-email.js — Gmail API email sender

### Established Patterns
- Task Scheduler: schtasks /create /tn "Name" /tr "command" /sc ONLOGON /rl HIGHEST
- HKLM Run key: reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v "Name" /t REG_SZ /d "command" /f
- .bat files: CRLF line endings, start /D for CWD, no parentheses in if/else

### Integration Points
- james_watchdog.ps1 — add comms-link health check (curl localhost:8766/relay/health)
- HKLM Run key — add CommsLink entry
- start-comms-link.bat — new bat file in comms-link root

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase following established patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
