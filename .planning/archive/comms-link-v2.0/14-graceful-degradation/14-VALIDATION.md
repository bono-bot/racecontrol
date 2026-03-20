---
phase: 14
slug: graceful-degradation
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-20
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node 22.14.0) |
| **Config file** | None |
| **Quick run command** | `node --test test/connection-mode.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/connection-mode.test.js test/graceful-degradation.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| GD-01 | WS down → critical messages route to email | unit | `node --test test/connection-mode.test.js` | Wave 0 | pending |
| GD-02 | WS+email down → messages buffer to WAL | unit | `node --test test/connection-mode.test.js` | Wave 0 | pending |
| GD-03 | connectionMode visible in metrics/heartbeat | integration | `node --test test/graceful-degradation.test.js` | Wave 0 | pending |

---

## Wave 0 Requirements

- [ ] `test/connection-mode.test.js` -- GD-01, GD-02 (state machine)
- [ ] `test/graceful-degradation.test.js` -- GD-01, GD-02, GD-03 (integration wiring)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live email fallback delivery | GD-01 | Requires Gmail OAuth | Kill WS, trigger critical message, check bono@racingpoint.in |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-20
