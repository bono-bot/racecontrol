---
phase: 7
slug: logbook-sync
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node.js 22.14.0) |
| **Config file** | none — invoked via `node --test test/*.test.js` |
| **Quick run command** | `node --test test/logbook-watcher.test.js test/logbook-merge.test.js test/logbook-sync.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~3 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/logbook-watcher.test.js test/logbook-merge.test.js test/logbook-sync.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 0 | LS-01 | unit | `node --test test/logbook-watcher.test.js` | ❌ W0 | ⬜ pending |
| 07-01-02 | 01 | 0 | LS-03 | unit | `node --test test/logbook-watcher.test.js` | ❌ W0 | ⬜ pending |
| 07-01-03 | 01 | 0 | LS-04 | unit | `node --test test/logbook-merge.test.js` | ❌ W0 | ⬜ pending |
| 07-01-04 | 01 | 0 | LS-02, LS-05 | integration | `node --test test/logbook-sync.test.js` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `test/logbook-watcher.test.js` — stubs for LS-01, LS-03 (polling, hashing, echo suppression, atomic write)
- [ ] `test/logbook-merge.test.js` — stubs for LS-04 (append detection, auto-merge, conflict flagging)
- [ ] `test/logbook-sync.test.js` — stubs for LS-02, LS-05 (end-to-end wiring, ack flow, reconnect sync)
- [ ] `james/logbook-watcher.js` — LogbookWatcher class
- [ ] `shared/logbook-merge.js` — Pure merge/conflict functions

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| 30-second end-to-end sync between James and Bono | LS-05 | Requires live WebSocket between two machines | 1. Edit LOGBOOK.md on James's side 2. Wait 30s 3. Verify Bono's copy matches |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
