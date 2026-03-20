---
phase: 55
slug: netdata-fleet-deploy
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 55 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Shell scripts (curl + sc query via rc-agent exec) |
| **Config file** | none |
| **Quick run command** | `curl -sf http://192.168.31.23:19999/api/v1/info \| head -1` |
| **Full suite command** | Check all 9 hosts (server + 8 pods) for Netdata API response |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Verify Netdata service running on target host
- **After every plan wave:** Check all deployed hosts respond on :19999
- **Before `/gsd:verify-work`:** Full fleet check — all 9 hosts
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 55-01-01 | 01 | 1 | MON-04 | curl | `curl -sf http://192.168.31.23:19999/api/v1/info` | ❌ W0 | ⬜ pending |
| 55-02-01 | 02 | 2 | MON-05 | curl | `curl -sf http://192.168.31.91:19999/api/v1/info` (Pod 8 canary) | ❌ W0 | ⬜ pending |

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Netdata survives pod reboot | MON-05 | Requires rebooting a pod | Reboot Pod 8, verify Netdata service auto-starts |
| Dashboard accessibility | MON-04/05 | May be UI-locked for free tier | Browse to :19999, check if metrics are visible |

---

## Validation Sign-Off

- [ ] All tasks have automated verify
- [ ] Sampling continuity
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
