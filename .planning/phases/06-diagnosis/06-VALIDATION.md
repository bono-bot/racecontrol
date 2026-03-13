---
phase: 6
slug: diagnosis
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-13
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | N/A — diagnosis phase, no code changes |
| **Config file** | none |
| **Quick run command** | N/A |
| **Full suite command** | N/A |
| **Estimated runtime** | N/A |

---

## Sampling Rate

- **After every task:** Verify output file exists and contains expected data
- **Before `/gsd:verify-work`:** All 4 DIAG reports must be present with data
- **Max feedback latency:** Immediate (manual verification of command output)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 6-01-01 | 01 | 1 | DIAG-01 | manual | Visual inspection of collected logs | N/A | pending |
| 6-01-02 | 01 | 1 | DIAG-02 | manual | Verify netstat output captured | N/A | pending |
| 6-01-03 | 01 | 1 | DIAG-03 | manual | Verify Edge registry values captured | N/A | pending |
| 6-01-04 | 01 | 1 | DIAG-04 | manual | Verify ipconfig output captured | N/A | pending |

*Status: pending*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. No test framework needed — this is a diagnosis-only phase.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Error logs collected from all pods | DIAG-01 | Information gathering via pod-agent exec | Run log collection commands, verify output files |
| Port audit on server | DIAG-02 | Server has no pod-agent, requires direct access | Run netstat -ano on server, inspect output |
| Edge settings baseline | DIAG-03 | Registry query via pod-agent exec | Run reg query on all pods, compare values |
| Server IP/MAC confirmed | DIAG-04 | Server requires direct access | Run ipconfig /all on server, record MAC |

---

## Validation Sign-Off

- [x] All tasks have manual verify instructions
- [x] No code changes — sampling not applicable
- [x] Wave 0 not needed — no test framework required
- [x] No watch-mode flags
- [x] Feedback latency: immediate
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
