---
phase: 72
slug: rc-sentry-endpoint-expansion-integration-tests
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 72 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in, no external harness) |
| **Config file** | none — inline #[cfg(test)] module in crates/rc-sentry/src/main.rs |
| **Quick run command** | `cargo test -p rc-sentry` |
| **Full suite command** | `cargo test -p rc-sentry && cargo tree -p rc-sentry \| grep tokio` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-sentry`
- **After every plan wave:** Run `cargo test -p rc-sentry && cargo tree -p rc-sentry | grep tokio`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 72-01-01 | 01 | 1 | SEXP-01 | integration | `cargo test -p rc-sentry test_health_fields` | ❌ W0 | ⬜ pending |
| 72-01-01 | 01 | 1 | SEXP-02 | integration | `cargo test -p rc-sentry test_version_fields` | ❌ W0 | ⬜ pending |
| 72-01-01 | 01 | 1 | SEXP-03 | integration | `cargo test -p rc-sentry test_files_directory` | ❌ W0 | ⬜ pending |
| 72-01-01 | 01 | 1 | SEXP-04 | integration | `cargo test -p rc-sentry test_processes_fields` | ❌ W0 | ⬜ pending |
| 72-01-02 | 01 | 1 | SHARD-06 | manual | N/A — OS signal not injectable from unit tests | N/A | ⬜ pending |
| 72-02-01 | 02 | 2 | TEST-04 | integration | `cargo test -p rc-sentry` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-sentry/build.rs` — copy from rc-agent/build.rs for GIT_HASH embedding (SEXP-02)
- [ ] `crates/rc-sentry/Cargo.toml` — add sysinfo = "0.33" and winapi target dep with consoleapi feature
- [ ] Integration test module in `crates/rc-sentry/src/main.rs` — 7 test functions (Plan 72-02)
- [ ] START_TIME OnceLock static + VERSION/BUILD_ID constants in main.rs
- [ ] SHUTDOWN_REQUESTED AtomicBool static + ctrl_handler function in main.rs

*Plan 72-01 creates the endpoint code; Plan 72-02 creates the tests.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Ctrl+C graceful shutdown | SHARD-06 | OS signal not injectable from unit tests | Build release binary. Run `rc-sentry.exe 9191`. Press Ctrl+C. Confirm "shutdown requested" log line and clean process exit. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
