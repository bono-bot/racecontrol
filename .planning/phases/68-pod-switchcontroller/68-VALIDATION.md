---
phase: 68
slug: pod-switchcontroller
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 68 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (rc-common + rc-agent) |
| **Config file** | none |
| **Quick run command** | `cargo test -p rc-common && cargo test -p rc-agent` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo build --bin rc-agent` (compile check)
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 68-01-01 | 01 | 1 | FAIL-01 | unit | `cargo test -p rc-agent config_validation` | TBD | pending |
| 68-01-02 | 01 | 1 | FAIL-02, FAIL-03 | unit | `cargo test -p rc-common switch_controller` | TBD | pending |
| 68-02-01 | 02 | 2 | FAIL-04 | unit | `cargo test -p rc-agent self_monitor` | TBD | pending |
| 68-02-02 | 02 | 2 | FAIL-02, FAIL-03 | integration | Pod 8 canary test (manual) | N/A | pending |

*Status: pending · green · red · flaky*

---

## Wave 0 Requirements

*Existing cargo test infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pod 8 canary SwitchController round-trip | FAIL-02, FAIL-03 | Requires live pod with WS connection | Deploy to Pod 8, send SwitchController via racecontrol, verify reconnect to Bono VPS |
| self_monitor suppression during switch | FAIL-04 | Requires real timing over 60s window | Switch Pod 8, monitor logs for 60s, confirm no relaunch |
| Switch back to .23 | SC-4 | Requires live venue network | Send SwitchController back to .23 URL, confirm billing heartbeat resumes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
