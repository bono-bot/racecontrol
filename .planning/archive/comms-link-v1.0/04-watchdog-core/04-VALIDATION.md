---
phase: 4
slug: watchdog-core
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-12
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | node:test (built-in) |
| **Config file** | none — node:test requires no config |
| **Quick run command** | `node --test tests/` |
| **Full suite command** | `node --test tests/` |
| **Estimated runtime** | ~3 seconds |

---

## Sampling Rate

- **After every task commit:** Run `node --test test/`
- **After every plan wave:** Run `node --test test/`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 3 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 04-01-F1 | 01 | 1 | WD-01, WD-02 | unit (TDD) | `node --test test/watchdog.test.js` | Created by TDD task | ⬜ pending |
| 04-02-T1 | 02 | 2 | WD-03 | integration | `node scripts/register-watchdog.js && schtasks /query /tn "CommsLink-Watchdog" /v /fo LIST` | ✅ | ⬜ pending |
| 04-02-T2 | 02 | 2 | WD-03 | checkpoint | Manual: reboot + verify watchdog starts | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*No separate Wave 0 needed — tests are created inline by TDD tasks (red-green cycle). Existing node:test infrastructure from Phase 1/2/3 is sufficient.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Task Scheduler runs watchdog in Session 1 | WD-03 | Requires Windows Task Scheduler | Create scheduled task, verify it runs in interactive session after reboot |
| Claude Code actually relaunches after kill | WD-02/WD-03 | Requires live process management | Kill claude.exe, verify watchdog detects within 5s, kills zombies, relaunches |
| Watchdog survives reboot | WD-03 | Requires machine reboot | Reboot James, verify watchdog starts and monitors Claude Code |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 3s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
