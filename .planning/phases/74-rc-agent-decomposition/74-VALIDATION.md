---
phase: 74
slug: rc-agent-decomposition
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-20
---

# Phase 74 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p rc-agent-crate` |
| **Full suite command** | `cargo test -p rc-agent-crate && cargo build --release --bin rc-agent` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate` (Phase 73 tests must stay green)
- **After every plan wave:** Run `cargo test -p rc-agent-crate && cargo build --release --bin rc-agent`
- **Before `/gsd:verify-work`:** Full suite + `wc -l crates/rc-agent/src/main.rs` < 500
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 74-01-01 | 01 | 1 | DECOMP-01 | build+test | `cargo test -p rc-agent-crate && test -f crates/rc-agent/src/config.rs` | ✅ | ⬜ pending |
| 74-02-01 | 02 | 2 | DECOMP-02 | build+test | `cargo test -p rc-agent-crate && test -f crates/rc-agent/src/app_state.rs` | ✅ | ⬜ pending |
| 74-03-01 | 03 | 3 | DECOMP-03 | build+test | `cargo test -p rc-agent-crate && test -f crates/rc-agent/src/ws_handler.rs` | ✅ | ⬜ pending |
| 74-04-01 | 04 | 4 | DECOMP-04 | build+test | `cargo test -p rc-agent-crate && test -f crates/rc-agent/src/event_loop.rs` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*No Wave 0 needed — all verification uses existing cargo test infrastructure from Phase 73. New module files are created by the extraction tasks themselves.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Pod 8 canary self-test | DECOMP-04 | Requires live pod hardware | Deploy release binary to Pod 8, run self-test, verify all 22 probes pass |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
