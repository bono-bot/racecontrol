---
phase: 26
slug: lap-filter-pin-security-telemetry-multiplayer
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-16
---

# Phase 26 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) |
| **Config file** | `Cargo.toml` (workspace) |
| **Quick run command** | `cargo test -p racecontrol-crate` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate` (covers lap_tracker, bot_coordinator, auth changes)
- **After every plan wave:** Run full suite (all 3 crates)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 26-01-01 | 01 | 0 | LAP-01/02/03 | unit RED stubs | `cargo test -p racecontrol-crate lap_tracker` | ✅ lap_tracker.rs exists | ⬜ pending |
| 26-01-02 | 01 | 0 | PIN-01/02 | unit RED stubs | `cargo test -p racecontrol-crate auth` | ✅ auth/mod.rs exists | ⬜ pending |
| 26-01-03 | 01 | 0 | TELEM-01/MULTI-01 | unit RED stubs | `cargo test -p racecontrol-crate bot_coordinator` | ✅ bot_coordinator.rs exists | ⬜ pending |
| 26-02-01 | 02 | 1a | LAP-01 | unit GREEN | `cargo test -p racecontrol-crate lap_tracker` | ✅ | ⬜ pending |
| 26-02-02 | 02 | 1a | LAP-02 | unit GREEN | `cargo test -p racecontrol-crate lap_tracker` | ✅ | ⬜ pending |
| 26-02-03 | 02 | 1a | LAP-03 | unit GREEN | `cargo test -p racecontrol-crate lap_tracker` | ✅ | ⬜ pending |
| 26-03-01 | 03 | 1b | PIN-01/02 | unit GREEN | `cargo test -p racecontrol-crate auth` | ✅ | ⬜ pending |
| 26-04-01 | 04 | 2 | TELEM-01 | unit GREEN | `cargo test -p racecontrol-crate bot_coordinator` | ✅ | ⬜ pending |
| 26-04-02 | 04 | 2 | MULTI-01 | unit GREEN | `cargo test -p racecontrol-crate bot_coordinator` | ✅ | ⬜ pending |
| 26-04-03 | 04 | 2 | rc-agent side | unit GREEN | `cargo test -p rc-agent-crate failure_monitor` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/lap_tracker.rs` — 4 RED test stubs (LAP-01 valid=false, LAP-02 review_required, LAP-03 session_type, LAP-02 catalog floor read)
- [ ] `crates/racecontrol/src/auth/mod.rs` or `auth.rs` — 3 RED test stubs (PIN-01 separate counters, PIN-02 staff not locked by customer, PIN-02 counter increment)
- [ ] `crates/racecontrol/src/bot_coordinator.rs` — 4 RED test stubs (TELEM-01 game state guard, TELEM-01 email fired, MULTI-01 lock→end→log ordering, MULTI-01 not fired when billing inactive)

*Wave 0 gate: all RED stubs committed and compile before Wave 1a/1b begins.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TELEM-01 email delivery to james@racingpoint.in | TELEM-01 | Requires live EmailAlerter + SMTP | Artificially silence UDP on Pod 8, wait 65s, check inbox |
| LAP-02 minimum floor values correct for tracks | LAP-02 | Requires real lap timing knowledge | Verify 80s Monza, 90s Silverstone, 120s Spa with Uday |
| MULTI-01 pod returns to idle lock screen | MULTI-01 | Requires AC multiplayer session | Disconnect from AC server mid-race, confirm lock screen appears |
| PIN-02 staff can still unlock after customer lockout | PIN-02 | Requires live lock screen flow | Exhaust customer PIN attempts, verify staff PIN still accepted |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
