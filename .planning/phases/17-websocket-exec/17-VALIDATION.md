---
phase: 17
slug: websocket-exec
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-15
---

# Phase 17 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml workspace |
| **Quick run command** | `cargo test -p rc-common protocol::tests && cargo test -p rc-agent-crate` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~20 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-common && cargo test -p rc-agent-crate`
- **After every plan wave:** Run `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 20 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 17-01-01 | 01 | 1 | WSEX-01 | unit | `cargo test -p rc-common protocol::tests::test_exec` | ❌ W0 | ⬜ pending |
| 17-01-02 | 01 | 1 | WSEX-03 | unit | `cargo test -p rc-common protocol::tests::test_exec_result` | ❌ W0 | ⬜ pending |
| 17-02-01 | 02 | 1 | WSEX-02 | unit | `cargo test -p rc-agent-crate ws_exec` | ❌ W0 | ⬜ pending |
| 17-02-02 | 02 | 1 | WSEX-01 | integration | `cargo test -p rc-agent-crate` | ❌ W0 | ⬜ pending |
| 17-03-01 | 03 | 1 | WSEX-01 | integration | `cargo test -p racecontrol-crate` | ❌ W0 | ⬜ pending |
| 17-03-02 | 03 | 1 | WSEX-04 | integration | `cargo test -p racecontrol-crate deploy` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `protocol::tests::test_exec_roundtrip` — CoreToAgentMessage::Exec serde roundtrip
- [ ] `protocol::tests::test_exec_wire_format` — JSON wire format matches snake_case tag
- [ ] `protocol::tests::test_exec_result_roundtrip` — AgentMessage::ExecResult serde roundtrip
- [ ] `protocol::tests::test_exec_result_success_and_error` — Both success and error variants
- [ ] `protocol::tests::test_exec_default_timeout` — Default timeout_ms behavior

*Existing test infrastructure covers framework needs. No new test dependencies required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| WS exec returns correct output from live pod | WSEX-01 | Requires live WebSocket + real shell | Send `whoami` via WS to Pod 8, verify stdout contains hostname |
| WS works when HTTP :8090 is blocked | WSEX-04 | Requires manual firewall rule deletion | Delete `RacingPoint-RemoteOps` on Pod 8, deploy via racecontrol, verify WS fallback succeeds |
| HTTP and WS exec do not compete | WSEX-02 | Requires concurrent load test | Fill 4 HTTP exec slots with `timeout 30`, simultaneously send WS exec `echo test`, verify WS returns immediately |
| Request ID correlation under concurrency | WSEX-03 | Requires multiple concurrent WS commands | Send 3 WS commands with different request_ids, verify each response has correct matching ID |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 20s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
