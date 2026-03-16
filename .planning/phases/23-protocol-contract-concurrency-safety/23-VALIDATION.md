---
phase: 23
slug: protocol-contract-concurrency-safety
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-16
---

# Phase 23 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust / cargo test |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test -p rc-common` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common`
- **After every plan wave:** Run full suite (all 3 crates)
- **Before `/gsd:verify-work`:** Full suite must be green (47 existing + new tests)
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 23-01-01 | 01 | 1 | PROTO-01 | unit | `cargo test -p rc-common` | ⬜ pending |
| 23-01-02 | 01 | 1 | PROTO-02 | compile | `cargo check -p racecontrol-crate && cargo check -p rc-agent-crate` | ⬜ pending |
| 23-01-03 | 01 | 1 | PROTO-03 | unit | `cargo test -p racecontrol-crate -- is_pod_in_recovery` | ⬜ pending |
| 23-01-04 | 01 | 1 | ALL | regression | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-common/src/types.rs` — `PodFailureReason` enum with 9 failure classes
- [ ] `crates/rc-common/src/protocol.rs` — 5 new `AgentMessage` variants
- [ ] `crates/racecontrol-crate/src/ws/mod.rs` — stub arms for all 5 new variants (must compile)
- [ ] `crates/racecontrol-crate/src/pod_healer.rs` — `is_pod_in_recovery()` predicate function + unit test

All additions are in existing files — no new test files required. Existing test infrastructure covers all phase requirements.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| None | — | All phase behaviors are type-system verified or have automated unit tests | — |

---

## Key Pitfalls (from research)

1. **ws/mod.rs exhaustive match** — Adding 5 new `AgentMessage` variants breaks compile immediately. Stub arms must be added in the same commit as rc-common changes.
2. **rc-agent check** — Run `cargo check -p rc-agent-crate` after rc-common changes to surface hidden match statements.
3. **is_pod_in_recovery() placement** — Lives in `racecontrol-crate`, NOT rc-common (WatchdogState is server-local). Extract pure predicate `fn is_recovering(state: &WatchdogState) -> bool` for unit testability.
4. **serde wire format** — Test assertions use `hardware_failure` (snake_case), NOT `HardwareFailure`.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
