---
phase: 3
slug: websocket-resilience
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-13
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in, `#[cfg(test)]` inline modules) |
| **Config file** | None — uses inline `#[cfg(test)] mod tests` blocks |
| **Quick run command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Full suite command** | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | CONN-01 | unit | `cargo test -p rc-core ws_ping_keepalive` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | CONN-03 | unit | `cargo test -p rc-agent reconnect_delay_for_attempt` | ❌ W0 | ⬜ pending |
| 03-01-03 | 01 | 1 | PERF-03 | unit | `cargo test -p rc-core ws_round_trip_slow_logs_warn` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 1 | CONN-02 | manual | Browser dev tools — disconnect rc-core, verify kiosk stays green 15s | N/A | ⬜ pending |
| 03-02-02 | 02 | 1 | PERF-04 | manual | React DevTools Profiler — trigger pod_update, verify only affected pod re-renders | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit test for `reconnect_delay_for_attempt(attempt: u32)` — cover attempt 0,1,2 (→1s), attempt 3 (→2s), attempt 7+ (→30s cap) in `rc-agent/src/main.rs` test module
- [ ] Unit test for WS ping interval logic — extract ping logic into testable fn or use mock ws_sender in `rc-core/src/ws/mod.rs`
- [ ] Unit test for Pong round-trip warn threshold — mock Instant or Duration injection in `rc-core/src/ws/mod.rs`

*Pure function extraction enables inline `#[cfg(test)]` tests — no new infrastructure needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Kiosk stays green during 15s debounce window | CONN-02 | Requires browser + live WS connection | 1. Open kiosk in Chrome 2. Stop rc-core 3. Verify header stays "Connected" for 15s 4. After 15s verify "Disconnected" appears |
| Only changed pod card re-renders | PERF-04 | Requires React DevTools Profiler | 1. Open kiosk with React DevTools 2. Trigger pod_update for Pod 1 3. Check Profiler — Pods 2-8 should show no render |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
