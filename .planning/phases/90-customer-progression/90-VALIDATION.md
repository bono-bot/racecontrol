---
phase: 90
slug: 90-customer-progression
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 90 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in test framework (cargo test) + manual PWA verification |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p racecontrol-crate --lib` |
| **Full suite command** | `cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate --lib`
- **After every plan wave:** Run `cargo test -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 90-01-01 | 01 | 1 | PROG-04 | unit | `cargo test -p racecontrol-crate --lib psychology` | W0 | pending |
| 90-01-02 | 01 | 1 | PROG-01, PROG-03 | integration | `cargo check -p racecontrol-crate` | W0 | pending |
| 90-02-01 | 02 | 2 | PROG-01, PROG-02 | manual | PWA page loads with passport data | N/A | pending |
| 90-02-02 | 02 | 2 | PROG-04, PROG-05 | manual | Profile badge showcase displays | N/A | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] Existing psychology tests still pass after driving_passport population logic added
- [ ] `cargo check` passes after new API endpoints

*Wave 0 tests are validated by running existing test suite after modifications.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Passport page shows track/car grid | PROG-01 | Frontend rendering | Navigate to /passport in PWA, verify grid displays |
| Tiered collections display | PROG-02 | Visual layout | Verify Starter/Explorer/Legend sections with correct counts |
| Backfill populates on first load | PROG-03 | Requires existing lap data | Customer with lap history opens passport, verify backfill |
| Badge showcase on profile | PROG-05 | Frontend rendering | Navigate to /profile, verify badges section |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
