---
phase: 1
slug: websocket-connection
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in Node.js 22 test runner) |
| **Config file** | none — uses node:test defaults |
| **Quick run command** | `node --test tests/` |
| **Full suite command** | `node --test tests/` |
| **Estimated runtime** | ~3 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test tests/`
- **After every plan wave:** Run `node --test tests/`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 1-01-01 | 01 | 1 | WS-01 | integration | `node --test tests/connection.test.js` | ❌ W0 | ⬜ pending |
| 1-01-02 | 01 | 1 | WS-03 | unit | `node --test tests/auth.test.js` | ❌ W0 | ⬜ pending |
| 1-01-03 | 01 | 1 | WS-04 | unit | `node --test tests/state-machine.test.js` | ❌ W0 | ⬜ pending |
| 1-02-01 | 02 | 1 | WS-01 | integration | `node --test tests/messaging.test.js` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/connection.test.js` — stubs for WS-01 (connection establishment)
- [ ] `tests/auth.test.js` — stubs for WS-03 (PSK authentication)
- [ ] `tests/state-machine.test.js` — stubs for WS-04 (state transitions)
- [ ] `tests/messaging.test.js` — stubs for WS-01 (bidirectional JSON messaging)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| NAT traversal works | WS-01 | Requires real network topology | Connect from James (LAN) to Bono (VPS), verify WebSocket opens |
| Connection survives idle | WS-01 | Requires real NAT router timeout | Leave connection open 10+ minutes, verify still connected |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
