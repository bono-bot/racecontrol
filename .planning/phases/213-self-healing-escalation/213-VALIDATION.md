---
phase: 213
slug: self-healing-escalation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 213 -- Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | bash + jq (inline assertions in healing scripts) |
| **Config file** | audit/results/auto-detect-config.json |
| **Quick run command** | `bash scripts/auto-detect.sh --mode quick --no-notify` |
| **Full suite command** | `bash scripts/auto-detect.sh --mode standard --no-notify` |
| **Estimated runtime** | ~30s quick, ~120s standard |

---

## Sampling Rate

- **After every task commit:** Run healing module standalone with mock finding
- **After every plan wave:** Run full auto-detect in dry-run
- **Before verify-work:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 213-01-01 | 01 | 1 | HEAL-01/02/03 | integration | `bash -n scripts/healing/escalation-engine.sh` | pending |
| 213-01-02 | 01 | 1 | HEAL-04/05 | integration | `bash -n scripts/healing/escalation-engine.sh` | pending |
| 213-02-01 | 02 | 2 | HEAL-06/07/08 | integration | `bash -n scripts/healing/escalation-engine.sh` | pending |

---

## Wave 0 Requirements

- Existing infrastructure covers all phase requirements (fixes.sh, notify.sh, core.sh)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WoL wakes pod | HEAL-01 | Requires physical pod | Send WoL to Pod 8, verify boot |
| WhatsApp delivery | HEAL-03 | External API | Trigger escalation, check Uday phone |

---

## Validation Sign-Off

- [ ] All tasks have automated verify
- [ ] Sampling continuity maintained
- [ ] Wave 0 covers all MISSING references
- [ ] Feedback latency < 30s
- [ ] nyquist_compliant: true set in frontmatter

**Approval:** pending
