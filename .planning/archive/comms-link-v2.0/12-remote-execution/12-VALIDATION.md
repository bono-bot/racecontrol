---
phase: 12
slug: remote-execution
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-20
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node 22.14.0) |
| **Config file** | None |
| **Quick run command** | `node --test test/exec-*.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/exec-handler.test.js test/exec-protocol.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| EXEC-01 | Send exec_request, receive exec_result | unit | `node --test test/exec-handler.test.js` | Wave 0 | pending |
| EXEC-02 | Array-args only, shell:false enforced | unit | `node --test test/exec-protocol.test.js` | Wave 0 | pending |
| EXEC-03 | Auto-approve tier executes immediately | unit | `node --test test/exec-handler.test.js` | Wave 0 | pending |
| EXEC-04 | Notify tier executes + sends notification | unit | `node --test test/exec-handler.test.js` | Wave 0 | pending |
| EXEC-05 | Approve tier pauses, waits for approval | unit | `node --test test/exec-handler.test.js` | Wave 0 | pending |
| EXEC-06 | Default-deny after timeout | unit | `node --test test/exec-handler.test.js` | Wave 0 | pending |
| EXEC-07 | exec_result includes stdout/stderr/exitCode | unit | `node --test test/exec-handler.test.js` | Wave 0 | pending |
| EXEC-08 | Sanitized env (PATH/SYSTEMROOT/TEMP only) | unit | `node --test test/exec-protocol.test.js` | Wave 0 | pending |

---

## Wave 0 Requirements

- [ ] `test/exec-protocol.test.js` -- covers EXEC-02, EXEC-08
- [ ] `test/exec-handler.test.js` -- covers EXEC-01, EXEC-03..07

*Wave 0 items created by TDD plans.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WhatsApp notification on notify/approve tier | EXEC-04, EXEC-05 | Requires Evolution API | Trigger notify-tier command, verify WhatsApp received |
| Approval via HTTP relay | EXEC-05 | Requires human interaction | Send exec_request with approve tier, POST to /relay/exec/approve |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-20
