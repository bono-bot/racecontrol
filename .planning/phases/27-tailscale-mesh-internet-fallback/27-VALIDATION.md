---
phase: 27
slug: tailscale-mesh-internet-fallback
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-16
---

# Phase 27 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in) |
| **Config file** | `Cargo.toml` workspace + per-crate |
| **Quick run command** | `cargo test -p racecontrol-crate --lib 2>&1 \| tail -20` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p racecontrol-crate --lib 2>&1 | tail -20`
- **After every plan wave:** Run full suite command above
- **Before `/gsd:verify-work`:** Full suite must be green + Pod 8 canary verified (Tailscale IP reachable from James's machine)
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 27-W0-01 | 01 | 0 | TS-01 | unit | `cargo test -p racecontrol-crate config::tests::bono_config_defaults` | ❌ W0 | ⬜ pending |
| 27-W0-02 | 01 | 0 | TS-02 | unit | `cargo test -p racecontrol-crate bono_relay::tests::spawn_disabled` | ❌ W0 | ⬜ pending |
| 27-W0-03 | 01 | 0 | TS-03 | unit | `cargo test -p racecontrol-crate bono_relay::tests::spawn_no_url` | ❌ W0 | ⬜ pending |
| 27-W0-04 | 01 | 0 | TS-04 | unit | `cargo test -p racecontrol-crate bono_relay::tests::event_serialization` | ❌ W0 | ⬜ pending |
| 27-01-01 | 01 | 1 | TS-01,TS-06 | unit | `cargo test -p racecontrol-crate --lib 2>&1 \| tail -20` | ✅ after W0 | ⬜ pending |
| 27-02-01 | 02 | 2 | TS-02,TS-03,TS-04 | unit | `cargo test -p racecontrol-crate bono_relay::tests` | ✅ after W0 | ⬜ pending |
| 27-03-01 | 03 | 3 | TS-DEPLOY | smoke (manual) | `Invoke-Command -ComputerName 192.168.31.91 ... { tailscale.exe ip -4 }` | N/A | ⬜ pending |
| 27-04-01 | 04 | 3 | TS-05 | integration (manual) | curl relay endpoint without secret → expect 401 | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/racecontrol/src/config.rs` — add `BonoConfig` struct + `bono_config_defaults` test
- [ ] `crates/racecontrol/src/bono_relay.rs` — new file with test stubs: `spawn_disabled`, `spawn_no_url`, `event_serialization`

*Pattern: matches existing `watchdog_config_deserializes_with_defaults` test in config.rs*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Relay endpoint returns 401 with wrong secret | TS-05 | Requires live Tailscale network + running relay | `curl -X POST http://<bono-tailscale-ip>:8081/relay/command` without correct `X-Relay-Secret` header — expect 401 |
| All 8 pods show Tailscale IP | TS-DEPLOY | Requires live tailnet join after WinRM deploy | `Invoke-Command -ComputerName <pod-ip> -ScriptBlock { & "C:\Program Files\Tailscale\tailscale.exe" ip -4 }` for each pod |
| Pod 8 canary reachable from James | TS-DEPLOY | Requires live tailnet | `ping <pod8-tailscale-ip>` from James's machine after joining tailnet |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
