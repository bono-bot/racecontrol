---
phase: 24
slug: crash-hang-launch-usb-bot-patterns
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-16
---

# Phase 24 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` |
| **Config file** | `Cargo.toml` (workspace) |
| **Quick run command** | `cargo test -p rc-agent-crate` |
| **Full suite command** | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| **Estimated runtime** | ~45 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p rc-agent-crate`
- **After every plan wave:** Run full suite (all 3 crates)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 45 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 24-01-W0 | 01 | 0 | CRASH-01,02,03 | unit stubs | `cargo test -p rc-agent-crate` | ⬜ pending |
| 24-01-01 | 01 | 1 | CRASH-01 | unit | `cargo test -p rc-agent-crate -- freeze` | ⬜ pending |
| 24-01-02 | 01 | 1 | CRASH-02 | unit | `cargo test -p rc-agent-crate -- fix_launch_timeout` | ⬜ pending |
| 24-01-03 | 01 | 1 | CRASH-03 | unit | `cargo test -p rc-agent-crate -- ffb_zero_before_kill_ordering` | ⬜ pending |
| 24-01-04 | 01 | 1 | UI-01 | unit | `cargo test -p rc-agent-crate -- kill_error_dialogs_extended` | ⬜ pending |
| 24-02-01 | 02 | 1 | USB-01 | unit | `cargo test -p rc-agent-crate -- auto_fix_usb_reconnect` | ⬜ pending |
| 24-02-02 | 02 | 1 | USB-01 | unit | `cargo test -p rc-agent-crate -- hardware_failure_disconnect_msg` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/rc-agent-crate/src/failure_monitor.rs` — new file, stubs for detection tests
- [ ] `crates/rc-agent-crate/src/ai_debugger.rs` — test stubs for 5 new fix arms + billing gate
- [ ] `PodStateSnapshot` — add `Default` derive before struct update syntax tests work

*No new test framework needed — `#[test]` is built-in.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Game freeze bot fires on Pod 8 | CRASH-01 | Requires real pod + UDP block | Block UDP port 9996 on Pod 8, verify rc-agent logs show freeze detection + game restart |
| CM kill on 90s timeout | CRASH-02 | Requires real pod + game launch | Launch AC on Pod 8, wait 90s without game starting, verify CM killed + retry |
| USB reconnect within 10s | USB-01 | Requires physical hardware | Unplug/replug Conspit Ares on Pod 8, verify HardwareFailure log + FFB restart within 10s |

---

## Key Pitfalls (from research)

1. **FFB zero ordering** — `ffb_zero_force()` MUST be called before `kill_game_process()` inside `fix_frozen_game()`. DebugMemory can replay the fix function in isolation — the fix itself must enforce ordering, not rely on the call site.
2. **billing_active gate** — Every new fix function must check `snapshot.billing_active` at entry. DebugMemory `instant_fix()` bypasses call-site guards.
3. **is_pod_in_recovery() is server-side** — Use a local `recovery_in_progress: AtomicBool` flag in rc-agent, set when server sends a recovery command. Do NOT call the server-side function.
4. **sysinfo two-refresh** — CPU usage requires `refresh_processes()` called twice with 500ms gap. Use `spawn_blocking` + `sleep`.
5. **Content Manager names** — Kill both `"Content Manager.exe"` and `"acmanager.exe"` — name varies by install.
6. **hidapi on Windows** — `device_list()` may briefly fail during USB re-enumeration. Retry once on error before treating as persistent disconnect.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 45s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
