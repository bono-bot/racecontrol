---
phase: 11
slug: reliable-delivery-wiring
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-20
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node 22.14.0) |
| **Config file** | None (convention: test/*.test.js) |
| **Quick run command** | `node --test test/reliable-delivery.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/reliable-delivery.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | TQ-05, BDR-01..03 | unit | `node --test test/reliable-delivery.test.js` | No -- Wave 0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `test/reliable-delivery.test.js` -- covers TQ-05, BDR-01, BDR-02, BDR-03
- [ ] May need `test/comms-client.test.js` updates if sendRaw() added to CommsClient

*Wave 0 items are created by the TDD plans themselves (RED phase writes tests first).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Deploy Bono first, then James | BDR-01 | Coordinated deploy | Deploy bono/index.js, verify ACKs sent for unknown messages. Then deploy james/index.js. |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-20
