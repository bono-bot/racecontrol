---
phase: 16
slug: firewall-auto-config
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 16 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | crates/rc-agent/Cargo.toml |
| **Quick run command** | `cargo test -p rc-agent-crate firewall` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate firewall`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 16-01-01 | 01 | 1 | FW-01 | unit | `cargo test -p rc-agent-crate firewall::tests` | ❌ W0 | ⬜ pending |
| 16-01-02 | 01 | 1 | FW-02 | unit | `cargo test -p rc-agent-crate firewall::tests::test_idempotent` | ❌ W0 | ⬜ pending |
| 16-01-03 | 01 | 1 | FW-03 | integration | `cargo test -p rc-agent-crate` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent/src/firewall.rs` — new module with configure() and unit tests
- [ ] Tests for FirewallResult enum variants
- [ ] Tests for run_netsh helper (command construction verification)

*Existing test infrastructure covers framework needs. No new test dependencies required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Firewall rules survive reboot | FW-01 | Requires physical pod reboot | Reboot Pod 8, verify `ping` and `curl :8090/health` work from server (.23) |
| Rules apply to all profiles | FW-02 | Requires `netsh show rule` on actual Windows | Run `netsh advfirewall firewall show rule name=RacingPoint-ICMP` on Pod 8, check `Profiles: Domain,Private,Public` |
| Log ordering (firewall before HTTP bind) | FW-03 | Requires reading rc-agent startup log | Check Pod 8 log shows "Firewall configured" before "Remote ops server started on port 8090" |
| No duplicate accumulation after 10 restarts | FW-02 | Requires repeated rc-agent restarts on pod | Start/stop rc-agent 10 times on Pod 8, run `netsh advfirewall firewall show rule name=RacingPoint-ICMP`, verify exactly one rule |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
