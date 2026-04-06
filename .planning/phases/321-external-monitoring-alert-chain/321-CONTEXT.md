# Phase 321: External Monitoring & Alert Chain — Context

Generated: 2026-04-06 (auto mode)

## Domain Boundary

rc-sentry independently monitors rc-agent health and alerts staff/Bono when rc-agent dies — independent of rc-agent's own API. Foundation for v42.0 MI migration.

## Existing Assets (from codebase scout)

| Asset | Location | Reusable? |
|-------|----------|-----------|
| Watchdog FSM (health polling, hysteresis, crash detection) | `rc-sentry/src/watchdog.rs` (485 lines) | YES — extend, don't rewrite |
| Tier 1 fixes + WhatsApp escalation | `rc-sentry/src/tier1_fixes.rs` (1414 lines) | YES — `escalate_to_whatsapp()` already works |
| MAINTENANCE_MODE handling + auto-clear | `rc-sentry/src/main.rs` (crash event handler) | YES — backoff + cooldown exists |
| Server pod_healer sentry coordination | `racecontrol/src/pod_healer.rs` (checks :8091) | EXTEND — add restart-via-sentry path |
| Session 1 spawn (WTSQueryUserToken) | `rc-sentry/src/session1_spawn.rs` (204 lines) | YES — for restart in interactive session |
| Debug memory ring buffer | `rc-sentry/src/debug_memory.rs` (238 lines) | YES — store monitoring events |
| Sentry config (TOML) | `rc-sentry/src/sentry_config.rs` (176 lines) | EXTEND — add COMMS_PSK, WhatsApp URL |

## Decisions

### MON-01: Detection Method
**Decision:** Dual detection — health poll (existing) + `tasklist /FI "IMAGENAME eq rc-agent.exe"` as secondary.
**Why:** Health poll catches unresponsive agent. Tasklist catches dead process faster (no 15s hysteresis needed). Anti-cheat safe — only inspects our own binary, not game processes.
**Implementation:** Add `check_process_alive()` to watchdog.rs. If tasklist says dead + health fails = immediate crash (skip hysteresis). If only health fails = keep existing hysteresis.

### MON-02: Server Recovery via Sentry
**Decision:** pod_healer gets a sentry fallback path. When :8090 unreachable but :8091 reachable, send restart command through rc-sentry /exec endpoint.
**Why:** Current pod_healer only detects partition (both unreachable). When rc-agent is dead but rc-sentry alive, we can recover without WoL/reboot.
**Implementation:** In `pod_healer.rs`, after detecting `:8090` down + `:8091` up, POST to `http://{pod_ip}:8091/exec` with `{"cmd": "schtasks /Run /TN StartRCAgent"}`. rc-sentry already has /exec endpoint.

### MON-03: Backoff + MAINTENANCE_MODE
**Decision:** Keep existing implementation. Already meets MON-03 requirements (3 restarts in 10 min → stop + auto-clear after backoff window).
**Why:** This was built in v11.2 and hardened in v31.0. No changes needed.

### MON-04: WhatsApp Alert Path
**Decision:** Add direct HTTP POST to WhatsApp Evolution API from rc-sentry. Deploy COMMS_PSK to all pods via rc-sentry.toml.
**Why:** Currently WhatsApp alerts go through comms-link relay on James's machine. If James is down, alerts are lost. Direct HTTP gives independence.
**Implementation:** Add `alert_config` section to rc-sentry.toml: `whatsapp_url`, `whatsapp_number`, `comms_psk`. New `fn send_whatsapp_alert()` in tier1_fixes.rs using std::net TcpStream HTTP POST (no reqwest — keep pure std).

### MON-05: Screenshot-Based Blanking Verification
**Decision:** Use Windows GDI `BitBlt` + `GetPixel` sampling in rc-sentry directly (not external binary). Sample 9 points on each monitor. If >80% match expected blanking color (#1A1A1A ± tolerance), blanking is verified.
**Why:** External binary adds deploy complexity. GDI calls are safe from rc-sentry's Session 1 context. 9-point sampling is fast (<50ms) and avoids the taskbar auto-hide false positive from full CopyFromScreen.
**Implementation:** New module `screen_verify.rs` with `fn verify_blanking() -> BlankingStatus`. Called after watchdog detects rc-agent restart + blanking command sent.

## Canonical References

- `crates/rc-sentry/src/watchdog.rs` — Current watchdog FSM
- `crates/rc-sentry/src/tier1_fixes.rs` — Escalation + WhatsApp + crash handling
- `crates/rc-sentry/src/main.rs` — Crash event loop, MAINTENANCE_MODE logic
- `crates/rc-sentry/src/session1_spawn.rs` — WTSQueryUserToken for Session 1
- `crates/racecontrol/src/pod_healer.rs` — Server-side pod recovery
- `crates/racecontrol/src/pod_monitor.rs` — Pod status tracking
- `CLAUDE.md` — Standing rules: Session 1, MAINTENANCE_MODE, restart paths

## Deferred Ideas

- Full Playwright screenshot comparison (belongs in v43.0, already done)
- rc-sentry → server WS channel for real-time monitoring dashboard (future phase)
- Multi-monitor blanking verification per-display (current 9-point sampling sufficient for MVP)

## Scope Guard

This phase adds MONITORING + ALERTING only. No MI engine migration (Phase 322), no MMA (Phase 323), no mesh gossip (Phase 324).
