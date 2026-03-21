---
phase: 8
slug: coordination-daily-ops
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node.js 22.14.0) |
| **Config file** | none — scripts.test in package.json |
| **Quick run command** | `node --test test/coordination.test.js test/daily-summary.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~3 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/coordination.test.js test/daily-summary.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | CO-01 | unit | `node --test test/coordination.test.js` | ❌ W0 | ⬜ pending |
| 08-01-02 | 01 | 1 | CO-01 | unit | `node --test test/coordination.test.js` | ❌ W0 | ⬜ pending |
| 08-02-01 | 02 | 1 | AL-05 | unit | `node --test test/daily-summary.test.js` | ❌ W0 | ⬜ pending |
| 08-02-02 | 02 | 1 | AL-05 | unit | `node --test test/daily-summary.test.js` | ❌ W0 | ⬜ pending |
| 08-02-03 | 02 | 1 | AL-05 | unit | `node --test test/daily-summary.test.js` | ❌ W0 | ⬜ pending |
| 08-03-01 | 03 | 2 | CO-02, CO-03 | manual-only | Review PROTOCOL.md + email | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `test/coordination.test.js` — stubs for CO-01 (message routing, ack-then-execute, bidirectional)
- [ ] `test/daily-summary.test.js` — stubs for AL-05 (scheduler timing, accumulator, WhatsApp/email formatting)
- No framework install needed — node:test already in use
- No shared fixtures needed — existing mock patterns (makeMockWss, makeMockMonitor) can be reused

*Existing infrastructure covers framework requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Protocol documentation with Mermaid diagrams | CO-02 | Documentation quality, diagram accuracy | Review PROTOCOL.md for completeness and correctness |
| [FAILSAFE] retirement instructions sent to Bono | CO-03 | Cross-system coordination via email | Verify email sent with transition plan |
| WhatsApp summary scannable in notification preview | AL-05 | UX/formatting on mobile device | Check WhatsApp preview on Uday's phone |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
