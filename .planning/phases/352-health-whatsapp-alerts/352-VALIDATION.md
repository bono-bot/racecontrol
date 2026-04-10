---
phase: 352
slug: health-whatsapp-alerts
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-04-10
---

# Phase 352 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + node -c (JS syntax) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p racecontrol-crate -- subsystem_health` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~30 seconds (quick) / ~120 seconds (full) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate -- subsystem_health`
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 352-01-01 | 01 | 1 | OPS-01, OPS-04, OPS-05 | unit | `cargo test -p racecontrol-crate -- subsystem_health` | ❌ W0 | ⬜ pending |
| 352-01-02 | 01 | 1 | OPS-01, OPS-02 | integration | `cargo test -p racecontrol-crate -- subsystem_health::tests::health_endpoint` | ❌ W0 | ⬜ pending |
| 352-02-01 | 02 | 2 | OPS-03 | syntax | `node -c comms-link/james/index.js` | ✅ | ⬜ pending |
| 352-02-02 | 02 | 2 | OPS-03 | unit | `cargo test -p racecontrol-crate -- subsystem_health::tests::relay_fallback` | ❌ W0 | ⬜ pending |
| 352-03-01 | 03 | 2 | OPS-06 | unit | `cargo test -p racecontrol-crate -- subsystem_health::tests::log_format` | ❌ W0 | ⬜ pending |
| 352-03-02 | 03 | 2 | OPS-07 | manual | SSH to Bono VPS, check log files | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/subsystem_health.rs` — tests module with unit tests for probes, dedup, alert dispatch
- [ ] Schema migration test for alert_incidents ALTER TABLE

*Tests will be created inline during Plan 01 execution (subsystem_health.rs includes a tests module).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Log rsync to Bono VPS | OPS-07 | Requires SSH access to remote server | SSH to Bono VPS, check `/root/backups/venue-logs/` for recent `.jsonl` files |
| WhatsApp message delivery | OPS-03 | Requires Evolution API + WhatsApp | Trigger a subsystem degradation, check Uday's WhatsApp for alert message |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
