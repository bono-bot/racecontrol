---
phase: 6
slug: alerting
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node.js v22.14.0) |
| **Config file** | None — uses package.json `test` script |
| **Quick run command** | `node --test test/alerting.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~3 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/alerting.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 06-01-01 | 01 | 1 | AL-01 | unit | `node --test test/alerting.test.js` | ❌ W0 | ⬜ pending |
| 06-01-02 | 01 | 1 | AL-02 | unit | `node --test test/alerting.test.js` | ❌ W0 | ⬜ pending |
| 06-01-03 | 01 | 1 | AL-04 | unit | `node --test test/alerting.test.js` | ❌ W0 | ⬜ pending |
| 06-02-01 | 02 | 1 | AL-02 | unit | `node --test test/alerting.test.js` | ❌ W0 | ⬜ pending |
| 06-02-02 | 02 | 1 | AL-03 | unit | `node --test test/alerting.test.js` | ❌ W0 | ⬜ pending |
| 06-02-03 | 02 | 1 | AL-04 | unit | `node --test test/alerting.test.js` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `test/alerting.test.js` — stubs for AL-01 through AL-04 behaviors
- [ ] `bono/alert-manager.js` — AlertManager class (tested via DI, no real HTTP calls)
- [ ] `shared/protocol.js` — extend MessageType with `recovery` (update protocol.test.js)

*Existing infrastructure (node:test, 97 tests across 11 files) covers test runner needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WhatsApp actually delivered to Uday's phone | AL-01, AL-02 | Requires real Evolution API credentials + phone | 1. Set env vars 2. Simulate james_down 3. Check Uday's phone |
| Email arrives in inbox (not spam) | AL-03 | Requires real Gmail delivery | 1. Disconnect WebSocket 2. Wait for cooldown cap 3. Check usingh@racingpoint.in inbox |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
