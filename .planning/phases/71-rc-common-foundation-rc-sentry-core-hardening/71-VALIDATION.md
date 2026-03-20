---
phase: 71
slug: rc-common-foundation-rc-sentry-core-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 71 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p rc-common exec && cargo test -p rc-sentry` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-sentry && cargo build --bin rc-sentry && cargo tree -p rc-sentry` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common exec && cargo test -p rc-sentry`
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 71-01-01 | 01 | 1 | SHARED-01 | unit | `cargo test -p rc-common exec::tests::test_sync_timeout` | ❌ W0 | ⬜ pending |
| 71-01-02 | 01 | 1 | SHARED-02 | unit | `cargo test -p rc-common exec::tests::test_async_timeout` | ❌ W0 | ⬜ pending |
| 71-01-03 | 01 | 1 | SHARED-03 | build | `cargo tree -p rc-sentry \| grep tokio` | ✅ | ⬜ pending |
| 71-02-01 | 02 | 1 | SHARD-01 | integration | `cargo test -p rc-sentry test_exec_timeout` | ❌ W0 | ⬜ pending |
| 71-02-02 | 02 | 1 | SHARD-02 | integration | `cargo test -p rc-sentry test_exec_truncation` | ❌ W0 | ⬜ pending |
| 71-02-03 | 02 | 1 | SHARD-03 | integration | `cargo test -p rc-sentry test_exec_concurrency_429` | ❌ W0 | ⬜ pending |
| 71-02-04 | 02 | 1 | SHARD-04 | integration | `cargo test -p rc-sentry test_partial_read` | ❌ W0 | ⬜ pending |
| 71-02-05 | 02 | 1 | SHARD-05 | smoke | rc-sentry startup shows tracing output | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-common/src/exec.rs` — run_cmd_sync + run_cmd_async + ExecResult + unit tests
- [ ] `crates/rc-sentry/src/main.rs` — integration tests for hardened endpoints

*Existing cargo test infrastructure covers the framework requirement.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Tracing output visible on startup | SHARD-05 | stdout formatting requires visual check | Start rc-sentry, verify timestamp+level in output |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
