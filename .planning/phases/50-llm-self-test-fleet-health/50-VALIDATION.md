---
phase: 50
slug: llm-self-test-fleet-health
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-19
---

# Phase 50 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo-nextest (workspace) |
| **Config file** | none — inherits workspace nextest config |
| **Quick run command** | `cargo test -p rc-agent-crate 2>&1 \| tail -30` |
| **Full suite command** | `cargo nextest run -p rc-agent-crate && cargo nextest run -p racecontrol-crate` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate 2>&1 | tail -30`
- **After every plan wave:** Run `cargo nextest run -p rc-agent-crate && cargo nextest run -p racecontrol-crate`
- **Before `/gsd:verify-work`:** Full suite must be green + `bash tests/e2e/fleet/pod-health.sh` passes on all reachable pods
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 50-01-01 | 01 | 1 | SELFTEST-01 | unit | `cargo test -p rc-agent-crate test_probe_results_` | ❌ W0 | ⬜ pending |
| 50-01-02 | 01 | 1 | SELFTEST-01 | unit | `cargo test -p rc-agent-crate test_probe_timeout` | ❌ W0 | ⬜ pending |
| 50-01-03 | 01 | 1 | SELFTEST-01 | unit | `cargo test -p rc-agent-crate test_probe_udp_port_parse` | ❌ W0 | ⬜ pending |
| 50-02-01 | 02 | 1 | SELFTEST-02 | unit | `cargo test -p rc-agent-crate test_verdict_parse` | ❌ W0 | ⬜ pending |
| 50-02-02 | 02 | 1 | SELFTEST-02 | unit | `cargo test -p rc-agent-crate test_verdict_fallback_critical` | ❌ W0 | ⬜ pending |
| 50-02-03 | 02 | 1 | SELFTEST-02 | unit | `cargo test -p rc-agent-crate test_self_test_report_json` | ❌ W0 | ⬜ pending |
| 50-03-01 | 03 | 2 | SELFTEST-03 | unit | `cargo test -p rc-common test_self_test_result_roundtrip` | ❌ W0 | ⬜ pending |
| 50-03-02 | 03 | 2 | SELFTEST-04 | unit | `cargo test -p rc-agent-crate test_fix_patterns_8_to_14` | ❌ W0 | ⬜ pending |
| 50-04-01 | 04 | 2 | SELFTEST-05 | smoke | `bash -n tests/e2e/fleet/pod-health.sh` | ❌ W0 | ⬜ pending |
| 50-04-02 | 04 | 2 | SELFTEST-05 | e2e | `bash tests/e2e/fleet/pod-health.sh` | ❌ W0 | ⬜ pending |
| 50-01-04 | 01 | 1 | SELFTEST-06 | unit | `cargo test -p rc-agent-crate test_startup_self_test` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent/src/self_test.rs` — new module with all 18 probe stubs + SelfTestReport struct
- [ ] `crates/rc-agent/src/self_test.rs` — unit tests for ProbeResult serde, verdict parsing, timeout behavior
- [ ] `crates/rc-common/src/protocol.rs` — `RunSelfTest` + `SelfTestResult` variants (with `#[serde(default)]` roundtrip test)
- [ ] `crates/racecontrol/src/state.rs` — `pending_self_tests` field addition
- [ ] `tests/e2e/fleet/pod-health.sh` — E2E fleet health test

*Existing cargo test + nextest infrastructure covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| GPU temp probe on all pods | SELFTEST-01 | nvidia-smi availability unverified on pods | SSH to pod, run `nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader` — if command not found, probe must return Skip |
| Fleet-wide self-test via server API | SELFTEST-03 | Requires all 8 pods connected to live server | `curl http://192.168.31.23:8080/api/v1/pods/pod-8/self-test` — verify JSON response with all 18 probes + verdict within 30s |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
