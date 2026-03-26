---
phase: 212
slug: detection-expansion
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-26
---

# Phase 212 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | bash + jq (inline assertions in detection scripts) |
| **Config file** | none — scripts self-validate via exit codes |
| **Quick run command** | `bash scripts/auto-detect.sh --mode quick --no-fix --no-notify` |
| **Full suite command** | `bash scripts/auto-detect.sh --mode standard --no-fix --no-notify` |
| **Estimated runtime** | ~30 seconds (quick), ~120 seconds (standard) |

---

## Sampling Rate

- **After every task commit:** Run detection module standalone with test fixture
- **After every plan wave:** Run full auto-detect pipeline in dry-run
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 212-01-01 | 01 | 1 | DET-01 | integration | `bash audit/phases/detect/config-drift.sh <test_toml>` | ❌ W0 | ⬜ pending |
| 212-01-02 | 01 | 1 | DET-02 | integration | `bash audit/phases/detect/bat-drift.sh` | ❌ W0 | ⬜ pending |
| 212-02-01 | 02 | 1 | DET-03 | integration | `bash audit/phases/detect/log-anomaly.sh <test_jsonl>` | ❌ W0 | ⬜ pending |
| 212-02-02 | 02 | 1 | DET-04 | integration | `bash audit/phases/detect/crash-loop.sh <test_jsonl>` | ❌ W0 | ⬜ pending |
| 212-03-01 | 03 | 1 | DET-05 | integration | `bash audit/phases/detect/flag-desync.sh` | ❌ W0 | ⬜ pending |
| 212-03-02 | 03 | 1 | DET-06,DET-07 | integration | `bash audit/phases/detect/schema-gap.sh` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Detection script stubs with correct exit code behavior
- [ ] Test fixture TOML/JSONL files for offline validation

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Config drift on live pod | DET-01 | Requires SCP to real pod | SCP rc-agent.toml from Pod 8, inject wrong value, run detector |
| Flag desync on live fleet | DET-05 | Requires multiple pods online | Query /api/v1/flags on 2+ pods, verify diff detection |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
