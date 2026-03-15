# AC Launch Reliability

## What This Is

Fixes the billing-to-game lifecycle in RaceControl — ensuring games stop when billing ends, billing pauses when games crash, launches are validated against active sessions, and pods reset cleanly after every session. Targets rc-core (billing.rs, game_launcher.rs) and rc-agent (ac_launcher.rs, game_process.rs, lock_screen.rs, main.rs).

## Core Value

No customer ever plays for free and no customer ever pays for downtime — the billing timer and game process are always in sync.

## Requirements

### Active (v5.0)

- [ ] Game killed within 10s of billing end
- [ ] Billing pauses when game crashes
- [ ] Game launch blocked without active billing
- [ ] Content Manager fallback on timeout/failure
- [ ] Pod resets to idle lock screen after session end

### Out of Scope

- New game integrations (F1 25, Forza) — AC only
- Billing algorithm changes — already done in credits migration (cc3da21)
- HUD overlay changes — separate milestone
- Cloud dashboard — separate GSD (billing-pos)
- Lock screen visual redesign — only lifecycle transitions

## Context

- **Revenue loss bug:** Billing expires → game keeps running → customer plays free. This is the #1 operational issue.
- **Customer pays for downtime:** Game crashes → billing keeps counting → customer charged for broken time.
- **Race condition:** Staff can launch game without billing session, or launch twice rapidly.
- **CM failures:** Content Manager hangs → 15s timeout → fallback to direct acs.exe, but failure not reported back.
- **Pod stuck:** Session ends → lock screen shows "Session Complete!" forever → never returns to idle.
- **Key files:**
  - `crates/rc-core/src/billing.rs` — BillingManager, timer lifecycle
  - `crates/rc-core/src/game_launcher.rs` — GameTracker, launch flow
  - `crates/rc-agent/src/ac_launcher.rs` — AC launch (1,400+ lines)
  - `crates/rc-agent/src/game_process.rs` — process monitoring, cleanup
  - `crates/rc-agent/src/lock_screen.rs` — lock screen states
  - `crates/rc-agent/src/main.rs` — message handling
  - `crates/rc-common/src/protocol.rs` — CoreToAgent/AgentToCore messages

## Constraints

- **Rust only:** rc-core and rc-agent stay Rust/Axum
- **No new protocol messages without backward compat:** Use serde(default) for new fields
- **10s max latency:** Game kill must happen within 10s of billing end
- **Billing stays authoritative:** rc-core owns billing state, rc-agent is a client
- **No breaking changes:** Existing billing, auth, and overlay must keep working

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Separate sub-project from v4.0 | v4.0 is infrastructure (Services, firewall); this is application logic (billing-game sync) | — Pending |
| 3 phases: Lifecycle → Crash → Launch | Lifecycle fixes revenue loss first, crash recovery second, launch resilience third | — Pending |

---
*Last updated: 2026-03-15 after milestone creation*
