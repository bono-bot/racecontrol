---
phase: 53
slug: deployment-automation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 53 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual verification (Task Scheduler + skills, no compiled code) |
| **Config file** | none |
| **Quick run command** | `schtasks /query /tn "RP-StagingHTTP" && schtasks /query /tn "RP-WebTerm"` |
| **Full suite command** | Above + `test -f .claude/skills/rp-deploy-fleet/SKILL.md` |
| **Estimated runtime** | ~2 seconds |

---

## Sampling Rate

- **After every task commit:** Verify scheduled task exists or skill file exists
- **After every plan wave:** Run schtasks /query + ls skills
- **Before `/gsd:verify-work`:** Cold reboot test for auto-start, deploy-fleet skill invocation
- **Max feedback latency:** 2 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 53-01-01 | 01 | 1 | DEPLOY-01 | schtasks | `schtasks /query /tn "RP-StagingHTTP" && schtasks /query /tn "RP-WebTerm"` | ❌ W0 | ⬜ pending |
| 53-02-01 | 02 | 1 | DEPLOY-02, DEPLOY-03 | file+content | `test -f .claude/skills/rp-deploy-fleet/SKILL.md && grep -q "verify.sh" .claude/skills/rp-deploy-fleet/SKILL.md` | ❌ W0 | ⬜ pending |

---

## Wave 0 Requirements

*No test framework needed. Verification is schtasks /query + file existence.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Staging HTTP + webterm auto-start after reboot | DEPLOY-01 | Requires actual reboot | Reboot James's machine, verify :9999 and HTTP server running within 60s |
| verify.sh runs after Pod 8 deploy | DEPLOY-02 | Requires live Pod 8 | Invoke /rp:deploy-fleet, check verify.sh output |
| Fleet deploy blocked until canary passes | DEPLOY-03 | Requires live pods | Deploy to Pod 8, verify prompt appears before fleet rollout |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 2s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
