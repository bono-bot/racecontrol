---
phase: 1
slug: cloud-infrastructure
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | bash + curl/wget (infrastructure validation scripts) |
| **Config file** | none — Wave 0 installs |
| **Quick run command** | `ssh bono@srv1422716.hstgr.cloud 'docker compose -f /opt/racingpoint/compose.yml ps --format json'` |
| **Full suite command** | `bash pwa/.planning/phases/01-cloud-infrastructure/verify-infra.sh` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | INFRA-06 | infra | `ssh VPS 'swapon --show \| grep swapfile'` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | INFRA-03 | infra | `ssh VPS 'ufw status \| grep -E "80\|443"'` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | INFRA-01 | infra | `curl -sI https://app.racingpoint.cloud \| grep HTTP` | ❌ W0 | ⬜ pending |
| 01-02-02 | 02 | 1 | INFRA-02 | infra | `ssh VPS 'docker compose ps --format json'` | ❌ W0 | ⬜ pending |
| 01-02-03 | 02 | 1 | INFRA-07 | infra | `ssh VPS 'docker stats --no-stream --format json'` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `pwa/.planning/phases/01-cloud-infrastructure/verify-infra.sh` — full verification script
- [ ] SSH access to VPS confirmed from James machine

*Infrastructure phase — no unit test framework needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| DNS A records point to 72.60.101.58 | INFRA-01 | Requires Cloudflare dashboard or dig | `dig +short app.racingpoint.cloud` must return 72.60.101.58 |
| Let's Encrypt certs are valid (not staging) | INFRA-01 | Browser verification needed | Open each subdomain in browser, check cert issuer is "Let's Encrypt" |
| Containers survive memory pressure | INFRA-07 | Requires stress test on VPS | Run `stress --vm 1 --vm-bytes 3G` on VPS, verify containers stay up |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
