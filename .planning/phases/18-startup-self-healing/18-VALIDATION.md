---
phase: 18
slug: startup-self-healing
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 18 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-common protocol::tests && cargo test -p rc-agent self_heal` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 18-01-01 | 01 | 1 | HEAL-01 | unit | `cargo test -p rc-agent self_heal` | ❌ W0 | ⬜ pending |
| 18-01-02 | 01 | 1 | HEAL-03 | unit | `cargo test -p rc-agent startup_log` | ❌ W0 | ⬜ pending |
| 18-02-01 | 02 | 1 | HEAL-02 | unit | `cargo test -p rc-common protocol::tests::test_startup_report` | ❌ W0 | ⬜ pending |
| 18-02-02 | 02 | 1 | HEAL-02 | integration | `cargo test -p rc-core` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Tests for config repair (missing file → regenerated with correct pod number)
- [ ] Tests for start script repair (correct CRLF line endings)
- [ ] Tests for registry key repair (command construction verification)
- [ ] Tests for startup log (phased write, append mode)
- [ ] Tests for StartupReport serde roundtrip
- [ ] Tests for config hash computation

*Existing test infrastructure covers framework needs. No new test dependencies required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Config regenerated from hostname | HEAL-01 | Requires actual pod hostname | Delete rc-agent.toml on Pod 8, restart rc-agent, verify new config has pod_number=8 |
| Registry key recreated after deletion | HEAL-01 | Requires admin + HKLM access | Delete RCAgent Run key on Pod 8, restart rc-agent, verify key restored via `reg query` |
| Start script survives CRLF check | HEAL-01 | Requires actual batch execution | Delete start-rcagent.bat on Pod 8, restart rc-agent, verify new script runs correctly |
| Startup report received by rc-core | HEAL-02 | Requires live WS connection | Check rc-core logs for StartupReport from Pod 8 after restart |
| Crash recovery flag set after kill | HEAL-02 | Requires force-killing rc-agent | Kill rc-agent on Pod 8, restart, verify crash_recovery=true in startup report |
| Startup log shows last phase on crash | HEAL-03 | Requires crash simulation | Kill rc-agent mid-startup on Pod 8, read rc-agent-startup.log for last phase |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
