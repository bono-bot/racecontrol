---
phase: 9
slug: protocol-foundation
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-20
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node 22.14.0) |
| **Config file** | None (uses `node --test test/*.test.js`) |
| **Quick run command** | `node --test test/ack-tracker.test.js test/message-queue.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~3 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/ack-tracker.test.js test/message-queue.test.js test/protocol.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | REL-01..06 | unit (TDD) | `node --test test/ack-tracker.test.js test/protocol.test.js` | No -- Wave 0 | pending |
| 09-02-01 | 02 | 1 | TQ-01..04 | unit (TDD) | `node --test test/message-queue.test.js` | No -- Wave 0 | pending |

*Status: pending / green / red / flaky*

---

## Wave 0 Requirements

- [ ] `test/ack-tracker.test.js` -- covers REL-01 through REL-06
- [ ] `test/message-queue.test.js` -- covers TQ-01 through TQ-04
- [ ] `test/protocol.test.js` -- extend with msg_ack type + isControlMessage() tests
- [ ] No framework install needed -- node:test is built-in

*Wave 0 items are created by the TDD plans themselves (RED phase writes tests first).*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-20
