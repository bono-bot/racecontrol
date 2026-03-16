---
phase: 25
slug: billing-guard-server-bot-coordinator
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-16
---

# Phase 25 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` |
| **Config file** | `Cargo.toml` (workspace) |
| **Quick run command** | `cargo test -p racecontrol-crate -- billing` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate -- billing`
- **After every plan wave:** Run full suite (all 3 crates)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 25-01-W0 | 01 | 0 | BILL-01 gate | unit characterization | `cargo test -p racecontrol-crate -- billing::tests` | ⬜ pending |
| 25-01-01 | 01 | 0 | BILL-01 | unit | `cargo test -p racecontrol-crate -- billing::tests::game_exit_while_billing_ends_session` | ⬜ pending |
| 25-01-02 | 01 | 0 | BILL-01 | unit | `cargo test -p racecontrol-crate -- billing::tests::idle_drift_condition_check` | ⬜ pending |
| 25-01-03 | 01 | 0 | BILL-01 | unit | `cargo test -p racecontrol-crate -- billing::tests::end_session_removes_timer` | ⬜ pending |
| 25-01-04 | 01 | 0 | BILL-01 | unit | `cargo test -p racecontrol-crate -- billing::tests::stuck_session_condition` | ⬜ pending |
| 25-01-05 | 01 | 0 | BILL-02 | compile | `cargo check -p rc-agent-crate` | ⬜ pending |
| 25-02-01 | 02 | 1 | BILL-02 | unit | `cargo test -p rc-agent-crate -- billing_guard::tests` | ⬜ pending |
| 25-02-02 | 02 | 1 | BILL-03 | unit | `cargo test -p rc-agent-crate -- billing_guard::tests::idle_drift_fires_at_5min` | ⬜ pending |
| 25-03-01 | 03 | 1 | BOT-01 | unit | `cargo test -p racecontrol-crate -- bot_coordinator::tests` | ⬜ pending |
| 25-03-02 | 03 | 1 | BILL-02 | unit | `cargo test -p racecontrol-crate -- bot_coordinator::tests::recover_no_timer_noop` | ⬜ pending |
| 25-03-03 | 03 | 1 | BILL-03 | unit | `cargo test -p racecontrol-crate -- bot_coordinator::tests::idle_drift_alerts_not_ends` | ⬜ pending |
| 25-04-01 | 04 | 2 | BILL-04 | integration | `cargo test -p racecontrol-crate -- integration::billing_bot_sync_fence` | ⬜ pending |
| 25-04-02 | 04 | 2 | ALL | regression | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

BILL-01 is a **prerequisite gate** — NO billing bot code may be written until these pass:

- [ ] `crates/racecontrol/src/billing.rs` — add 4 characterization tests (`game_exit_while_billing_ends_session`, `idle_drift_condition_check`, `end_session_removes_timer`, `stuck_session_condition`)
- [ ] `crates/rc-agent/src/failure_monitor.rs` — add `driving_state: Option<DrivingState>` field to `FailureMonitorState` + update `send_modify` site in main.rs

*No new test files required — all Wave 0 additions land in existing `billing.rs` test module.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Stuck session recovery on Pod 8 | BILL-02 | Requires real billing session + game kill | Start billing on Pod 8, kill game process, wait 70s, verify rc-agent logs show BillingAnomaly sent + server logs show end_session called |
| Idle drift alert reaches staff | BILL-03 | Requires email delivery + real session | Start billing on Pod 8, leave idle 6 minutes, verify email alert received at usingh@racingpoint.in |
| Wallet balance consistent after bot-ended session | BILL-04 | Requires real cloud sync | Check wallet in PWA before/after bot-triggered end_session — balance must match expected debit |

---

## Key Pitfalls (from research)

1. **DrivingState missing from FailureMonitorState** — must be added in Wave 0 before billing_guard.rs can read it. `Option<DrivingState>` field + one `send_modify` site in main.rs at the `DrivingStateUpdate` match arm.
2. **Agent never calls end_session** — billing_guard.rs sends `AgentMessage::BillingAnomaly` ONLY. The server (bot_coordinator.rs) owns `end_billing_session_public()`. Violating this crosses the agent→server boundary.
3. **StopSession → SessionUpdate::Finished ordering** — bot_coordinator must call in this exact order. Reversed order corrupts billing state (session shows as active after end).
4. **session_id from agent may be stale** — server resolves session by pod_id lookup via `active_timers`, not by trusting the agent's session_id field.
5. **BILL-03 is alert-only** — idle drift must NEVER auto-end the session. Only BILL-02 (stuck session) triggers end_session. These are two separate BillingAnomaly reason variants.
6. **Cloud sync fence (BILL-04)** — check `relay_available` AtomicBool; wait up to 5s for one relay cycle after `end_billing_session()`. Full CRDT echo acknowledgment is deferred to v6.0.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
