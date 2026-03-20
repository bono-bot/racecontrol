---
phase: 13
slug: observability
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-03-20
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in, Node 22.14.0) |
| **Config file** | None |
| **Quick run command** | `node --test test/metrics-collector.test.js` |
| **Full suite command** | `node --test test/*.test.js` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/metrics-collector.test.js test/system-metrics.test.js`
- **After every plan wave:** Run `node --test test/*.test.js`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists | Status |
|--------|----------|-----------|-------------------|-------------|--------|
| OBS-01 | Heartbeat includes pod status, queue depth, deployment | unit | `node --test test/system-metrics.test.js` | Exists (extend) | pending |
| OBS-02 | MetricsCollector accumulates counters | unit | `node --test test/metrics-collector.test.js` | Wave 0 | pending |
| OBS-03 | GET /relay/metrics returns JSON | integration | `node --test test/metrics-collector.test.js` | Wave 0 | pending |
| OBS-04 | Email send validated E2E | smoke | manual | N/A | pending |

---

## Wave 0 Requirements

- [ ] `test/metrics-collector.test.js` -- covers OBS-02, OBS-03
- [ ] Extend `test/system-metrics.test.js` -- covers OBS-01

*Wave 0 items created by TDD plans.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Email fallback E2E | OBS-04 | Requires Gmail OAuth + network | Run send_email.js, verify receipt in bono@racingpoint.in |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 5s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-03-20
