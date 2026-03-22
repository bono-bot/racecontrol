# Phase 162: James Watchdog Migration - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Replace james_watchdog.ps1 (blind 2min PowerShell service checker) with a Rust-based monitor binary using AI debugger pattern memory, graduated response, and Bono alert on repeated failures.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- james_watchdog.ps1 runs as Task Scheduler every 2min on James (.27)
- Services it monitors: Ollama (:11434), Claude Code, comms-link, kiosk (:3300), webterm (:9999)
- New binary: rc-watchdog crate already exists (crates/rc-watchdog/) — extend or rewrite
- Must use rc-common/src/recovery.rs: RecoveryAuthority::JamesMonitor, RecoveryLogger
- Graduated response per service: 1st fail → retry, 2nd → restart, 3rd+ → alert Bono via comms-link WS
- Pattern memory: debug-memory.json on James machine
- Must register in Task Scheduler + HKLM Run like current watchdog
- Alert Bono: use comms-link send-message.js for WS notification
- Standing rule: do NOT restart services that are deliberately stopped

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- crates/rc-watchdog/ — existing crate, may need inspection for current state
- C:\Users\bono\.claude\james_watchdog.ps1 — current PS1 watchdog to replace
- rc-common/src/recovery.rs — RecoveryAuthority::JamesMonitor, RecoveryLogger (Phase 159)
- comms-link send-message.js — Bono notification mechanism

### Integration Points
- Task Scheduler: replace james_watchdog.ps1 task with rc-watchdog.exe
- HKLM Run key: add rc-watchdog.exe for boot start
- recovery-log.jsonl at RECOVERY_LOG_JAMES path
- Bono comms: spawn node send-message.js for alerts

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
