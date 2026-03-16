---
phase: 7
slug: server-side-pinning
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (rc-common, rc-agent, racecontrol) — 47 tests |
| **Config file** | `Cargo.toml` (workspace) |
| **Quick run command** | `cargo test -p rc-common && cargo test -p rc-agent-crate` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~30 seconds |

No JavaScript test framework exists for the kiosk. Phase 7 tests are primarily operational smoke tests via curl.

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common && cargo test -p rc-agent-crate` (add `-p racecontrol-crate` if CORS patch applied)
- **After every plan wave:** Run full suite + smoke tests
- **Before `/gsd:verify-work`:** Full suite + all smoke tests must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-xx-01 | TBD | 1 | HOST-01 | smoke | `curl -s http://192.168.31.23:3300/kiosk \| findstr "RacingPoint"` | ❌ W0 | ⬜ pending |
| 07-xx-02 | TBD | 1 | HOST-02 | manual | Reboot server, wait 60s, verify :3300/kiosk | N/A | ⬜ pending |
| 07-xx-03 | TBD | 1 | HOST-03 | manual | `ipconfig` on server confirms .23 | N/A | ⬜ pending |
| 07-xx-04 | TBD | 1 | HOST-04 | smoke | `curl -s http://kiosk.rp:3300/kiosk` from James | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] No kiosk JavaScript test framework — operational smoke tests via curl sufficient
- [ ] CORS patch to `crates/racecontrol/src/main.rs` requires `cargo test -p racecontrol-crate`

*Existing Rust test infrastructure covers compilation verification for CORS changes.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Kiosk auto-starts after reboot | HOST-02 | Requires physical server reboot | Reboot server → wait 60s → `curl http://192.168.31.23:3300/kiosk` |
| Server IP persists after router restart | HOST-03 | Requires router restart | Check router admin DHCP reservation → reboot server → `ipconfig /all` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
