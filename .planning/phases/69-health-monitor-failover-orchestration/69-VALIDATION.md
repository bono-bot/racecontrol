---
phase: 69
slug: health-monitor-failover-orchestration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 69 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node --test (comms-link) + cargo test (racecontrol + rc-agent) |
| **Config file** | none |
| **Quick run command** | `node --check james/index.js && cargo build --bin racecontrol --bin rc-agent` |
| **Full suite command** | `cd C:/Users/bono/racingpoint/comms-link && npm test && cd C:/Users/bono/racingpoint/racecontrol && cargo test -p rc-common && cargo test -p racecontrol` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick compile check on modified files
- **After every plan wave:** Run full suite
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| TBD | 01 | 1 | HLTH-01, HLTH-02 | unit | `node --test test/health-monitor.test.js` | pending |
| TBD | 01 | 1 | HLTH-03 | unit | Verify 60s window in test | pending |
| TBD | 02 | 2 | ORCH-01, ORCH-02 | unit | `node --test test/failover-orchestrator.test.js` | pending |
| TBD | 02 | 2 | ORCH-03 | unit | `cargo test -p rc-agent lan_probe` | pending |
| TBD | 02 | 2 | ORCH-04 | unit | notification send check | pending |
| TBD | 03 | 3 | HLTH-04 | unit | `node --test test/bono-watchdog.test.js` | pending |

---

## Wave 0 Requirements

*Existing test infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| End-to-end failover with real server power-off | All | Requires physical server shutdown at venue | Power off .23, wait 60s, verify all pods switch to Bono VPS |
| Split-brain with partial network | ORCH-03 | Requires network manipulation | Disconnect specific pods from .23 while others stay connected |
| Uday receives email + WhatsApp | ORCH-04 | External service delivery | Trigger failover, check Uday's phone/email |

---

## Validation Sign-Off

- [ ] All tasks have automated verify or Wave 0 dependencies
- [ ] Sampling continuity maintained
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
