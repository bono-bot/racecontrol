# James-Bono Comms Link

## What This Is

A persistent, real-time communication system between James (on-site AI at Racing Point venue, 192.168.31.27) and Bono (cloud AI on VPS srv1422716.hstgr.cloud / 72.60.101.58). Includes a watchdog that keeps Claude Code (James) alive, syncs LOGBOOK.md between both AIs, and alerts Uday via WhatsApp when James goes down or recovers.

## Core Value

James and Bono are always connected and always in sync — if the link drops, both sides know immediately and recovery is automatic.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Persistent WebSocket connection from James → Bono's VPS (James initiates, since LAN has no public IP)
- [ ] Bidirectional real-time messaging over WebSocket (coordination messages, not just heartbeat)
- [ ] Heartbeat sent by James every N seconds; Bono detects missing heartbeat within seconds
- [ ] LOGBOOK.md sync — on every commit, the committing side pushes the update to the other via the comms link
- [ ] Watchdog process on James's machine that monitors Claude Code and auto-restarts on crash
- [ ] Clean restart — kill zombie processes before relaunching Claude Code
- [ ] Auto-reconnect — after restart, watchdog re-establishes WebSocket to Bono
- [ ] James auto-emails Bono on restart: "I'm back online"
- [ ] WhatsApp notification to Uday when James goes down (via Bono's bot / Evolution API)
- [ ] WhatsApp notification to Uday when James comes back online
- [ ] Email as secondary fallback — same information sent via email when WebSocket is down
- [ ] Both AIs always have current LOGBOOK.md (no stale state)

### Out of Scope

- Replacing email — email remains as secondary/fallback channel
- Voice/video between AIs — text messaging only
- Customer-facing features — this is internal AI-to-AI infrastructure
- Changing Bono's VPS setup — James-side only for now (Bono implements his side)

## Context

- James runs Claude Code CLI on Windows 11 (192.168.31.27, RTX 4070)
- Bono runs Claude Code on VPS (srv1422716.hstgr.cloud, 72.60.101.58)
- James is behind NAT (router 192.168.31.1) — cannot receive inbound connections
- Bono already has a heartbeat/failsafe monitor in the WhatsApp bot that sends `[FAILSAFE]` messages when James is unresponsive
- Current communication: email only (james@racingpoint.in ↔ bono@racingpoint.in) — too slow for real-time coordination
- LOGBOOK.md lives in racecontrol repo root, maintained by both AIs
- WhatsApp bot runs on Bono's VPS via PM2 (Evolution API)
- The watchdog must be a standalone process (not inside Claude Code) so it survives Claude Code crashes

## Constraints

- **NAT**: James behind LAN — WebSocket must be initiated FROM James TO Bono (outbound)
- **Platform**: Watchdog runs on Windows 11 — must handle Windows process management (tasklist, taskkill)
- **No zombies**: Must cleanly kill old Claude Code processes before restarting
- **Bono coordination**: Bono needs to implement the WebSocket server endpoint on his VPS — requires email coordination
- **Reliability**: Watchdog itself must be resilient — run as a Windows service or scheduled task

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| WebSocket primary, email fallback | Real-time needed for coordination; email too slow but reliable backup | — Pending |
| James initiates connection | Behind NAT, can't receive inbound | — Pending |
| Standalone watchdog process | Must survive Claude Code crashes | — Pending |
| Sync LOGBOOK.md over comms link | Keeps both AIs current without git pull delays | — Pending |

---
*Last updated: 2026-03-12 after initialization*
