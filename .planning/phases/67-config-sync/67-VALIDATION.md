---
phase: 67
slug: config-sync
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 67 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual verification + node --check syntax validation |
| **Config file** | none |
| **Quick run command** | `node --check james/index.js && node --check bono/index.js` |
| **Full suite command** | `cd C:/Users/bono/racingpoint/comms-link && npm test` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --check` on modified files
- **After every plan wave:** Run `npm test` in comms-link
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 67-01-01 | 01 | 1 | SYNC-01 | integration | `node --check james/index.js` | TBD | pending |
| 67-01-02 | 01 | 1 | SYNC-02 | unit | `grep -q "sanitize\|allowlist\|SAFE_SECTIONS" james/config-sync.js` | TBD | pending |
| 67-02-01 | 02 | 2 | SYNC-03 | integration | `curl -s http://localhost:8766/relay/health` | TBD | pending |

*Status: pending · green · red · flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements — comms-link test suite already in place.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Config change triggers sync_push within 60s | SYNC-01 | Requires editing racecontrol.toml on live server | Edit toml on .23, watch James comms-link logs for sync_push |
| Bono applies config to cloud racecontrol | SYNC-03 | Requires Bono VPS to be running updated code | Check cloud racecontrol config after sync |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
