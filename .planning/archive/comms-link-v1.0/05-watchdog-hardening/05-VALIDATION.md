---
phase: 5
slug: watchdog-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node.js v22.14.0) |
| **Config file** | None — uses package.json `test` script |
| **Quick run command** | `node --test test/watchdog.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/watchdog.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 3 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 05-01-01 | 01 | 1 | WD-04 | unit | `node --test test/watchdog.test.js` | Extend existing | ⬜ pending |
| 05-01-02 | 01 | 1 | WD-04 | unit | `node --test test/watchdog.test.js` | Extend existing | ⬜ pending |
| 05-01-03 | 01 | 1 | WD-05 | unit | `node --test test/watchdog.test.js` | Extend existing | ⬜ pending |
| 05-01-04 | 01 | 1 | WD-05 | unit | `node --test test/watchdog.test.js` | Extend existing | ⬜ pending |
| 05-02-01 | 02 | 1 | WD-06 | unit | `node --test test/watchdog.test.js` | New test | ⬜ pending |
| 05-02-02 | 02 | 1 | WD-07 | unit | `node --test test/watchdog.test.js` | New test | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `test/watchdog.test.js` — EscalatingCooldown unit tests (new describe block)
- [ ] `test/watchdog.test.js` — WD-04 integration tests (cooldown + poll loop)
- [ ] `test/watchdog.test.js` — WD-05 self-test event tests (self_test_passed / self_test_failed)
- [ ] `test/watchdog.test.js` — WD-06 CommsClient wiring tests (mock CommsClient, verify connect())
- [ ] `test/watchdog.test.js` — WD-07 email notification tests (mock execFile, verify args)

*All new tests go in the existing `test/watchdog.test.js` file as new `describe` blocks.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WebSocket reconnects to Bono after restart | WD-06 | Requires live Bono VPS | Kill Claude Code, verify WS reconnects in watchdog-runner logs |
| Email received by Bono | WD-07 | Requires Gmail delivery | Kill Claude Code, check bono@racingpoint.in inbox |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 3s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
