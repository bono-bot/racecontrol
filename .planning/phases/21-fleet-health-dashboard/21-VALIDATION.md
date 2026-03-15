---
phase: 21
slug: fleet-health-dashboard
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 21 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust) + Next.js build check |
| **Config file** | Cargo.toml workspace + kiosk/package.json |
| **Quick run command** | `cargo test -p rc-core` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-core && cd kiosk && npx next build` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 21-01-01 | 01 | 1 | FLEET-01 | unit | `cargo test -p rc-core fleet` | ✅ | ⬜ pending |
| 21-01-02 | 01 | 1 | FLEET-02 | unit | `cargo test -p rc-core fleet` | ✅ | ⬜ pending |
| 21-01-03 | 01 | 1 | FLEET-03 | unit | `cargo test -p rc-core fleet` | ✅ | ⬜ pending |
| 21-02-01 | 02 | 2 | FLEET-01 | build | `cd kiosk && npx next build` | ✅ | ⬜ pending |
| 21-02-02 | 02 | 2 | FLEET-01 | manual | Phone browser check | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No new test frameworks needed.

- [x] `crates/rc-core/src/` — existing test infrastructure with 225+ unit tests
- [x] `kiosk/` — existing Next.js app with build pipeline

*All test infrastructure already exists.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| /fleet page loads on phone within 3s | FLEET-01 | Requires real browser on mobile device | Open http://192.168.31.23:3300/fleet on Uday's phone, confirm grid loads |
| WS vs HTTP indicators visually distinct | FLEET-02 | Visual UI verification | Disconnect one pod's HTTP, verify indicators differ from fully healthy pod |
| Version + uptime visible after deploy | FLEET-03 | Requires live fleet deploy | Deploy new version, verify all 8 pod cards show updated version string |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
