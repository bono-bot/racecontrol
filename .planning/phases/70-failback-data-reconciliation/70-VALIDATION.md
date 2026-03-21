---
phase: 70
slug: failback-data-reconciliation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 70 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node --test (comms-link) + cargo test (racecontrol) |
| **Config file** | none |
| **Quick run command** | `node --check james/health-monitor.js && node --check james/failover-orchestrator.js && cargo build --bin racecontrol` |
| **Full suite command** | `cd C:/Users/bono/racingpoint/comms-link && npm test && cd C:/Users/bono/racingpoint/racecontrol && cargo test -p racecontrol` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Quick compile check
- **After every plan wave:** Full suite
- **Max feedback latency:** 45 seconds

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Recovery detection after real outage | BACK-01 | Requires .23 power-off then power-on | Power off .23, wait for failover, power on, verify 2-up triggers failback |
| Session merge from cloud to local | BACK-02 | Requires sessions created during real failover | Create billing session on VPS during failover, verify it appears in local DB |
| Pod reconnect to .23 after failback | BACK-03 | Requires live pods | Verify all 8 pods reconnect to .23 WS within 30s |
| Uday notification with outage duration | BACK-04 | External service delivery | Check email + WhatsApp on Uday's devices |

---

## Validation Sign-Off

- [ ] All tasks have automated verify
- [ ] Sampling continuity maintained
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
