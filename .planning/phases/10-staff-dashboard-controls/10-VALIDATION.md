---
phase: 10
slug: staff-dashboard-controls
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) |
| **Config file** | Cargo.toml per crate |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common`
- **After every plan wave:** Run `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-core`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 10-01-01 | 01 | 1 | KIOSK-01 | unit | `cargo test -p rc-core -- lockdown` | ❌ W0 | ⬜ pending |
| 10-01-02 | 01 | 1 | KIOSK-02 | unit | `cargo test -p rc-core -- lockdown_all` | ❌ W0 | ⬜ pending |
| 10-01-03 | 01 | 1 | PWR-01 | unit | `cargo test -p rc-core -- wol` | ❌ W0 | ⬜ pending |
| 10-01-04 | 01 | 1 | PWR-02 | unit | `cargo test -p rc-core -- wol` | ❌ W0 | ⬜ pending |
| 10-01-05 | 01 | 1 | PWR-03 | unit | `cargo test -p rc-core -- wol` | ❌ W0 | ⬜ pending |
| 10-01-06 | 01 | 1 | PWR-04/05/06 | unit | `cargo test -p rc-core -- bulk` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-core/src/wol.rs` — add unit tests for `parse_mac` (happy path, colon + dash separators, error cases)
- [ ] `crates/rc-core/src/api/routes.rs` or new `tests/lockdown_tests.rs` — unit tests for lockdown route logic (billing guard, disconnected sender guard)
- [ ] No framework install needed — `cargo test` already works (85 tests passing)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Lockdown toggle UI in kiosk | KIOSK-01/02 | Next.js UI interaction | Click toggle, verify pod state changes |
| Wake-on-LAN pod power-on | PWR-03/06 | Requires physical pod hardware | Send WoL, verify pod boots |
| Pod shutdown/restart effect | PWR-01/02/04/05 | Requires active pods | Click button, verify pod goes offline/reboots |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
