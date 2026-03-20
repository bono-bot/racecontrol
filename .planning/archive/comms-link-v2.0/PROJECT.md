# James-Bono Comms Link

## What This Is

A persistent, real-time communication system between James (on-site AI at Racing Point venue, 192.168.31.27) and Bono (cloud AI on VPS srv1422716.hstgr.cloud / 72.60.101.58). Includes WebSocket transport, process supervision, LOGBOOK sync, alerting to Uday, and bidirectional AI-to-AI coordination with command execution capabilities.

## Core Value

James and Bono are always connected and always in sync — if the link drops, both sides know immediately and recovery is automatic.

## Current Milestone: v2.0 Reliable AI-to-AI Communication

**Goal:** Redesign comms-link with production-hardened reliability, bidirectional messaging with delivery guarantees, remote command execution, and full observability — based on 8 days of v1.0 production learnings.

**Target features:**
- Bulletproof process supervision (mid-session watchdog, crash recovery without reboot)
- Message ACK protocol (sequence numbers, delivery confirmation, transactional queue)
- Bidirectional task routing (structured request/response replacing one-way INBOX)
- Remote command execution (Bono sends commands to James and vice versa, with approval flow)
- Observability (metrics export: uptime, reconnects, latency, health snapshots for Bono dashboard)

## Requirements

### Validated

Shipped in v1.0 and confirmed working in production:

- WebSocket connection with PSK auth and state machine (WS-01, WS-03, WS-04)
- Auto-reconnect with exponential backoff + message queue replay (WS-02, WS-05)
- Application-level heartbeat with system metrics (HB-01..04)
- Claude Code watchdog with zombie cleanup and auto-restart (WD-01..03)
- Watchdog hardening: escalating cooldown, self-test, email notification (WD-04..07)
- WhatsApp + email alerting to Uday with flapping suppression (AL-01..04)
- LOGBOOK.md sync with conflict detection (LS-01..05)
- Coordination messaging + daily health summary (CO-01..03, AL-05)

### Active

- [ ] Mid-session process supervision — daemon crash recovery without reboot
- [ ] Message ACK protocol — sequence numbers + delivery confirmation
- [ ] Transactional message queue — replace append-only INBOX.md
- [ ] Bidirectional task routing — structured request/response channel
- [ ] Remote command execution — Bono sends commands, James executes (and vice versa) with approval
- [ ] Health snapshots — pod status + deployment state in heartbeats
- [ ] Metrics export — uptime, reconnect count, message latency for Bono dashboard
- [ ] Email fallback E2E validation
- [ ] Cloud relay E2E validation (sync_push/action)

### Out of Scope

- Replacing email entirely — email remains as secondary/fallback channel
- Voice/video between AIs — text messaging only
- Customer-facing features — this is internal AI-to-AI infrastructure
- GUI dashboard — metrics exported for Bono to consume, not a standalone UI
- Changing Bono's VPS core setup — coordinate via protocol, Bono implements his side

## Context

- v1.0 shipped Mar 12, 2026 (8 phases, 14 plans, 222 tests)
- **15-hour blind outage (Mar 17-18):** Both daemon + heartbeat died on reboot, no auto-start key was registered. Fixed post-incident with HKCU Run key.
- **Mid-session gap:** If daemon crashes after login, stays offline until next reboot — no watchdog-of-watchdog
- **One-way INBOX:** James sends to INBOX.md, Bono has no ACK or response channel
- **`wmic` deprecated:** Health check uses deprecated Windows API (wmic)
- **INBOX file races:** appendFileSync can collide with git operations (no file locking)
- **Email fallback untested:** Never validated end-to-end in production
- **Cloud relay partial:** sync_push/action code exists but no E2E validation
- James runs Claude Code CLI on Windows 11 (192.168.31.27, RTX 4070)
- Bono runs Claude Code on VPS (srv1422716.hstgr.cloud, 72.60.101.58)
- James is behind NAT — WebSocket initiated outbound from James to Bono

## Constraints

- **NAT**: James behind LAN — all connections must be outbound from James
- **Platform**: Windows 11 — must handle Windows process management correctly (no deprecated APIs)
- **No zombies**: Must cleanly kill old processes before restarting
- **Bono coordination**: Protocol changes require Bono-side updates — coordinate via comms-link itself
- **Reliability**: Process supervision must survive daemon crashes, not just reboots
- **Security**: Remote command execution must have approval/authorization flow — no blind execution

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| WebSocket primary, email fallback | Real-time needed; email too slow but reliable backup | Good |
| James initiates connection | Behind NAT, can't receive inbound | Good |
| Standalone watchdog process | Must survive Claude Code crashes | Good |
| Sync LOGBOOK.md over comms link | Keeps both AIs current without git pull delays | Good |
| PSK auth via Bearer header | Avoids server log leaks vs query params | Good |
| HKCU Run key for auto-start | Ensures process starts in Session 1 at login | Revisit — need mid-session recovery too |
| Append-only INBOX.md | Simple but no ACK, no locking, races with git | Revisit — replace with transactional queue |
| `wmic` for process detection | Works but deprecated in Windows 11 | Revisit — replace with modern API |

---
*Last updated: 2026-03-20 after v2.0 milestone initialization*
