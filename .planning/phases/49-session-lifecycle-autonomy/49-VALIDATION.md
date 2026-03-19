---
phase: 49
slug: session-lifecycle-autonomy
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 49 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | bash + lib/common.sh (E2E shell), cargo nextest (unit) |
| **Config file** | none — scripts are self-contained |
| **Quick run command** | `cargo test -p rc-agent` |
| **Full suite command** | `bash tests/e2e/api/session-lifecycle.sh && cargo test -p rc-agent` |
| **Estimated runtime** | ~30 seconds (unit) + ~60 seconds (E2E) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent`
- **After every plan wave:** Run `bash tests/e2e/api/session-lifecycle.sh && bash tests/e2e/api/billing.sh`
- **Before `/gsd:verify-work`:** Full suite must be green (`bash tests/e2e/run-all.sh`)
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| TBD | 01 | 1 | SESSION-01 | integration | `bash tests/e2e/api/session-lifecycle.sh` | ❌ W0 | ⬜ pending |
| TBD | 01 | 1 | SESSION-02 | integration | included in session-lifecycle.sh Gate 4 | ❌ W0 | ⬜ pending |
| TBD | 01 | 1 | SESSION-03 | unit + manual | `cargo test -p rc-agent` (state machine) | ❌ W0 | ⬜ pending |
| TBD | 01 | 1 | SESSION-04 | unit + manual | `cargo test -p rc-agent` (ws grace) | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/e2e/api/session-lifecycle.sh` — E2E test for SESSION-01 + SESSION-02 (billing create → orphan timeout → auto-end → pod reset)
- [ ] No framework install needed — bash + python3 already available (matching billing.sh pattern)

*Existing cargo nextest infrastructure covers unit tests. Only E2E shell script is new.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Game crash → billing paused → relaunch → resume or auto-end | SESSION-03 | Requires controlled game crash on live pod hardware | 1. Start billing on Pod 8. 2. Kill game process. 3. Verify billing paused within 5s (check /api/v1/pods). 4. Wait for relaunch. 5. Kill game again. 6. Verify auto-end after 2nd failure. |
| WS drop < 30s → no Disconnected screen, no self-relaunch | SESSION-04 | Requires network disruption on live pod | 1. Start session on Pod 8. 2. Disconnect pod network for 10s. 3. Reconnect. 4. Verify no "Disconnected" screen shown. 5. Verify rc-agent did NOT relaunch (PID unchanged). |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
