---
phase: 19
slug: watchdog-service
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 19 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-common && cargo test -p rc-watchdog` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate && cargo test -p rc-watchdog` |
| **Estimated runtime** | ~25 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 25 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 19-01-01 | 01 | 1 | SVC-03 | unit | `cargo test -p rc-common types::tests::test_watchdog_crash_report` | ❌ W0 | ⬜ pending |
| 19-01-02 | 01 | 1 | SVC-02 | unit | `cargo test -p rc-watchdog service::tests` | ❌ W0 | ⬜ pending |
| 19-01-03 | 01 | 1 | SVC-01 | unit | `cargo test -p rc-watchdog` | ❌ W0 | ⬜ pending |
| 19-02-01 | 02 | 2 | SVC-03 | unit | `cargo test -p racecontrol-crate watchdog` | ❌ W0 | ⬜ pending |
| 19-02-02 | 02 | 2 | SVC-04 | integration | manual | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] New `crates/rc-watchdog` crate with Cargo.toml and workspace member
- [ ] Tests for WatchdogCrashReport serde roundtrip (rc-common)
- [ ] Tests for process detection logic (rc-watchdog)
- [ ] Tests for double-restart prevention (rc-watchdog)
- [ ] Tests for crash report handler (racecontrol)

*New crate required — entire rc-watchdog structure is Wave 0.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Service installs and runs as SYSTEM | SVC-01 | Requires admin + SCM on real pod | Run install.bat on Pod 8, verify `sc.exe query RCWatchdog` shows RUNNING |
| rc-agent restarted in Session 1 after kill | SVC-02 | Requires live pod with user session | Kill rc-agent on Pod 8, verify `tasklist /v` shows Session# = 1 within 10s |
| Crash report received by racecontrol | SVC-03 | Requires live WS + HTTP connection | Kill rc-agent on Pod 8, check racecontrol logs for crash report within 30s |
| SCM failure actions configured | SVC-04 | Requires admin SCM access | Run `sc.exe qfailure RCWatchdog` on Pod 8, verify restart actions present |
| Boot without login → lock screen appears | SVC-02 | Requires pod reboot with auto-login | Reboot Pod 8, verify lock screen appears within 60s |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 25s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
