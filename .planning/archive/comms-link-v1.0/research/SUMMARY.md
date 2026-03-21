# Research Summary: James-Bono Comms Link

**Domain:** AI-to-AI persistent communication + process watchdog
**Researched:** 2026-03-12
**Overall confidence:** HIGH

## Executive Summary

This project is a well-understood domain problem: a persistent WebSocket connection between a NAT-ed Windows client and a Linux VPS, with a process watchdog on the Windows side and WhatsApp/email alerting. There are no exotic requirements. The entire system can be built with one npm dependency (`ws`) per side plus Node.js built-ins, targeting roughly 500 lines of JavaScript total across both sides.

The technology landscape in 2026 strongly favors a minimal approach. Node.js 22 LTS ships with everything needed: global `fetch` for HTTP, `crypto` for hashing, `child_process` for process management, `fs/promises` for file I/O. The `ws` library (v8.19.0) is the undisputed standard for Node.js WebSocket -- actively maintained, zero dependencies, 22.7k GitHub stars. Socket.IO, the main alternative, is overengineered for a 1:1 connection. The `reconnecting-websocket` wrapper hasn't been updated in 6 years and its reconnection logic is trivially implementable in ~30 lines.

The primary risk is Windows-specific process management. Racing Point already encountered and solved the critical pattern: Windows Session 0 isolation prevents GUI processes launched by services from being visible (rc-agent pods had this exact issue). The watchdog must run in the user's session (via Task Scheduler or HKLM Run key), not as a NSSM service, to avoid this trap. Zombie process cleanup via `taskkill /F /T` (tree kill) is mandatory before every restart -- Claude Code leaves orphan node.exe processes that cause EBUSY file lock errors.

The alerting path leverages existing infrastructure: Evolution API (already running on Bono's VPS via PM2) for WhatsApp, and the `@racingpoint/google` package (already proven) for Gmail fallback. No new external services need to be provisioned.

## Key Findings

**Stack:** Node.js 22 ESM + `ws` ^8.19.0. One npm dependency per side. No TypeScript, no build step.

**Architecture:** Single Node.js daemon on James that is both watchdog and comms client. WebSocket initiated outbound from James (NAT constraint). Bono runs a WebSocket server behind PM2. JSON messages over WSS with pre-shared key auth.

**Critical pitfall:** Session 0 isolation -- if the watchdog runs as a Windows service (NSSM), Claude Code spawns in Session 0 with no desktop access. Use Task Scheduler with "Run only when user is logged on" instead. This is a known, already-solved pattern from rc-agent deployment.

## Implications for Roadmap

Based on research, suggested phase structure:

1. **Phase 1: WebSocket + Heartbeat** - Foundation transport layer
   - Addresses: WebSocket connection, auto-reconnect with backoff, heartbeat ping/pong, connection state machine
   - Avoids: Building watchdog before having a way to report its status remotely
   - Bono coordination needed: He must deploy the WebSocket server endpoint

2. **Phase 2: Watchdog + Process Management** - Claude Code supervision
   - Addresses: Process monitoring, zombie cleanup, auto-restart, escalating cooldown
   - Avoids: Session 0 isolation (use Task Scheduler, not NSSM service)
   - Depends on: Phase 1 (reports status over WebSocket)

3. **Phase 3: Alerting** - WhatsApp + email notifications
   - Addresses: WhatsApp alerts via Evolution API, email fallback, alert deduplication
   - Avoids: Duplicate alerting (must coordinate with Bono to retire existing failsafe)
   - Depends on: Phase 2 (watchdog detects events that trigger alerts)

4. **Phase 4: LOGBOOK.md Sync** - File synchronization
   - Addresses: File watching, hash-based change detection, full file sync over WebSocket, atomic writes
   - Avoids: Git-based sync (index.lock race condition with Claude Code's git polling), CRDT complexity
   - Depends on: Phase 1 (uses WebSocket as transport)

**Phase ordering rationale:**
- WebSocket first because everything else depends on having a communication channel
- Watchdog second because the core value proposition is "James stays alive and reports status"
- Alerting third because alerts are triggered by watchdog events and delivered over the WebSocket
- LOGBOOK sync last because it's the most independent feature and has the most pitfalls (line endings, atomic writes, git locks) that benefit from having the rest of the system stable first

**Research flags for phases:**
- Phase 1: Standard patterns, unlikely to need research. The `ws` library docs cover everything.
- Phase 2: Needs investigation of Claude Code's exact process name and startup behavior. Session 0 vs Session 1 is a known issue with a known fix, but the watchdog startup mechanism (Task Scheduler vs HKLM Run key) needs validation on this specific machine.
- Phase 3: Needs coordination with Bono to get Evolution API instance name and API key. Also need to agree on retiring the existing `[FAILSAFE]` heartbeat to avoid duplicate alerts.
- Phase 4: May need research into Claude Code's git polling behavior (`git status` rate) to understand index.lock collision risk. Consider bypassing git entirely for the sync path.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | ws is the clear winner. Node.js built-ins cover everything else. Verified versions and maintenance status. |
| Features | HIGH | Well-understood domain. Table stakes are clear. Anti-features are well-reasoned. |
| Architecture | HIGH | Single-process daemon is the proven pattern. Supervisor tree, event-driven state machine, JSON protocol -- all standard. |
| Pitfalls | HIGH | 17 pitfalls documented, several directly observed on this machine (zombie processes, Session 0). Evidence-based, not speculative. |

## Gaps to Address

- **Claude Code CLI exact startup command:** Need to verify the correct command to spawn Claude Code (is it `claude`, `claude-code`, or a full path through the UWP-style directory?). The existing PowerShell watchdog has dynamic path discovery that should be ported.
- **Evolution API instance name and API key:** Need to get these from Bono's VPS configuration. Required for Phase 3.
- **Bono's VPS port availability:** Need to coordinate which port the WebSocket server will listen on. Port 443 (wss://) is ideal to avoid ISP blocking but may conflict with existing services.
- **Existing failsafe retirement plan:** Need to coordinate with Bono on how to transition from the current `[FAILSAFE]` heartbeat mechanism to the new comms-link without creating alert gaps.
- **LOGBOOK.md location:** PROJECT.md says it's in racecontrol repo root. Need to confirm both AIs have consistent paths configured.

## Technology Versions Summary

| Technology | Version | Status | Last Verified |
|------------|---------|--------|---------------|
| Node.js | 22.14.0 | Installed on James | 2026-03-12 (confirmed) |
| ws | 8.19.0 | Latest on npm | 2026-03-12 (npm search) |
| NSSM | 2.24 | Latest stable | 2026-03-12 (nssm.cc) |
| Evolution API | v2 | Running on Bono's VPS | 2026-03-12 (per PROJECT.md) |
| node-windows | 1.0.0-beta.8 | REJECTED (stale, 3 years) | 2026-03-12 (npm search) |
| reconnecting-websocket | 4.4.0 | REJECTED (abandoned, 6 years) | 2026-03-12 (npm search) |
| Socket.IO | 4.x | REJECTED (overengineered) | 2026-03-12 (research) |

## Sources

All sources are documented with confidence levels in the individual research files:
- STACK.md: 12 sources (HIGH: npm, GitHub, official docs; MEDIUM: comparison articles)
- FEATURES.md: 10 sources (HIGH: official docs, GitHub issues; MEDIUM: blog posts)
- ARCHITECTURE.md: 11 sources (HIGH: library docs, GitHub; MEDIUM: architecture guides)
- PITFALLS.md: 17 sources (HIGH: local machine logs, GitHub issues; MEDIUM: official docs; LOW: ISP behavior)
