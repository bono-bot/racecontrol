---
phase: 91
slug: 91-session-experience
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 91 — Validation Strategy

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
| 91-01-01 | 01 | 1 | SESS-03 | unit | `cargo test -p racecontrol-crate --lib` | W0 | pending |
| 91-01-02 | 01 | 1 | SESS-02, SESS-04 | integration | `cargo check -p racecontrol-crate` | W0 | pending |
| 91-02-01 | 02 | 2 | SESS-01, SESS-02 | manual | PWA confetti + peak-end report render | N/A | pending |
| 91-02-02 | 02 | 2 | SESS-04 | manual | PWA PB toast renders during active session | N/A | pending |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Confetti animation on PB | SESS-01 | Visual animation | Open /sessions/[id] for a session with PB, verify confetti fires |
| Peak-end report layout | SESS-02 | Visual layout | Session detail shows peak moment card before averages |
| Percentile ranking display | SESS-03 | Visual rendering | Session detail shows "Faster than X% of drivers" |
| Real-time PB toast | SESS-04 | Requires live session | During active session, set PB, verify toast appears |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
