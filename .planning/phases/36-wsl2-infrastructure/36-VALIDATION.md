---
phase: 36
slug: wsl2-infrastructure
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-17
---

# Phase 36 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual infrastructure verification (no Rust code in this phase) |
| **Config file** | none — no test framework changes |
| **Quick run command** | `cargo test` (regression only — no new tests) |
| **Full suite command** | `cargo test` (335 tests: 269 unit + 66 integration) |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test` (regression baseline)
- **After every plan wave:** Run `cargo test` + manual verification commands
- **Before `/gsd:verify-work`:** Full suite must be green + all success criteria verified from Pod 8
- **Max feedback latency:** 30 seconds (cargo test) + manual verification

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 36-01-01 | 01 | 1 | INFRA-01 | manual | `wsl --list --verbose` + check `.wslconfig` | N/A | ⬜ pending |
| 36-01-02 | 01 | 1 | INFRA-01 | manual | `wsl -e cat /etc/wsl.conf` | N/A | ⬜ pending |
| 36-01-03 | 01 | 1 | INFRA-02 | manual | `wsl -e salt-master --version` | N/A | ⬜ pending |
| 36-01-04 | 01 | 1 | INFRA-03 | manual | `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 | N/A | ⬜ pending |
| 36-02-01 | 02 | 2 | INFRA-04 | manual | `curl http://192.168.31.27:8000/login` from server (.23) | N/A | ⬜ pending |
| 36-02-02 | 02 | 2 | INFRA-05 | manual | Reboot James → verify salt-master running within 60s | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No new test files needed — Phase 36 is pure infrastructure with manual verification.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WSL2 mirrored networking reachable from pods | INFRA-01 | Requires physical pod on LAN | `Test-NetConnection 192.168.31.27 -Port 4505` from Pod 8 |
| salt-master listening on 4505/4506 | INFRA-02 | WSL2 service state | `wsl -e salt-master --version` + `wsl -e ss -tlnp \| grep 4505` |
| Hyper-V + Defender firewall open | INFRA-03 | Two separate firewall layers | `Get-NetFirewallHyperVVMSetting` + `Get-NetFirewallRule -DisplayName '*Salt*'` |
| salt-api accessible from server | INFRA-04 | Cross-machine HTTP call | `curl http://192.168.31.27:8000/login` from .23 |
| Auto-start after reboot | INFRA-05 | Requires full reboot cycle | Reboot James → wait 60s → `wsl -e service salt-master status` |

---

## Validation Sign-Off

- [x] All tasks have manual verify steps documented
- [x] Sampling continuity: every task has a verification command
- [x] Wave 0: no new test infrastructure needed (infrastructure phase)
- [x] No watch-mode flags
- [x] Feedback latency < 60s (manual commands)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
