---
phase: 5
slug: content-validation-filtering
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-14
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml per crate |
| **Quick run command** | `cargo test -p rc-common && cargo test -p rc-agent` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common && cargo test -p rc-agent`
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | CONT-07 | unit | `cargo test -p rc-agent content_scanner` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | CONT-05, CONT-06 | unit | `cargo test -p rc-agent content_scanner` | ❌ W0 | ⬜ pending |
| 05-01-03 | 01 | 1 | CONT-07 | unit | `cargo test -p rc-common manifest` | ❌ W0 | ⬜ pending |
| 05-02-01 | 02 | 2 | CONT-01, CONT-02 | unit | `cargo test -p rc-core catalog_filter` | ❌ W0 | ⬜ pending |
| 05-02-02 | 02 | 2 | CONT-04, SESS-07 | unit | `cargo test -p rc-core catalog_filter` | ❌ W0 | ⬜ pending |
| 05-02-03 | 02 | 2 | CONT-04 | unit | `cargo test -p rc-core launch_validation` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stubs for content scanner (rc-agent)
- [ ] Test stubs for catalog filtering (rc-core)
- [ ] Test stubs for launch validation (rc-core)

*Existing test infrastructure covers framework needs. Only test stubs needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pod content scan accuracy | CONT-07 | Requires real AC filesystem on pod | Deploy to Pod 8, verify manifest matches actual content/cars/ and content/tracks/ folders |
| PWA shows only pod-installed content | CONT-01, CONT-02 | Requires running PWA against live pod | Browse catalog on PWA for Pod 8, confirm all listed cars/tracks exist on pod filesystem |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
