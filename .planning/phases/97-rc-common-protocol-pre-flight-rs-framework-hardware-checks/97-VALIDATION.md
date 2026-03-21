---
phase: 97
slug: rc-common-protocol-pre-flight-rs-framework-hardware-checks
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 97 — Validation Strategy

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo build (built-in) |
| **Quick run command** | `cargo build --bin rc-agent && cargo test -p rc-agent-crate` |
| **Full suite command** | `cargo build --bin rc-agent --bin racecontrol && cargo test -p rc-common && cargo test -p rc-agent-crate` |
| **Estimated runtime** | ~30 seconds |

## Sampling Rate

- **After every task commit:** `cargo build --bin rc-agent && cargo test -p rc-agent-crate`
- **After every plan wave:** Full suite
- **Max feedback latency:** 30 seconds

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 97-01-01 | 01 | 1 | PF-07 | build | `cargo build --bin rc-agent --bin racecontrol` | ✅ | ⬜ pending |
| 97-02-01 | 02 | 2 | PF-01,02,03,HW-01,02,03,SYS-01 | build+test | `cargo build --bin rc-agent && cargo test -p rc-agent-crate` | ❌ W0 | ⬜ pending |

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pre-flight blocks session on hardware failure | HW-01 | Requires disconnecting wheelbase USB | Unplug wheelbase, trigger BillingStarted, verify MaintenanceRequired |
| ConspitLink auto-restart | HW-03 | Requires killing ConspitLink on live pod | taskkill ConspitLink, trigger BillingStarted, verify auto-restart |

## Validation Sign-Off

- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
