# Phase 57: Session-End Safety - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix the stuck-rotation bug so wheelbases return to center safely when any game session ends. Replace the current ESTOP-based session-end with a proper close-ConspitLink-then-HID-commands sequence using `fxm.reset` + `axis.idlespring`. Ensure gradual force ramp-up to prevent snap-back injuries. Keep ESTOP available as a separate emergency-only code path.

</domain>

<decisions>
## Implementation Decisions

### Hardware Fleet
- Fleet is primarily **Ares 12Nm** (7 pods) with one **Ares 8Nm** as hot-swap spare
- Both use OpenFFBoard firmware (VID: 0x1209, PID: 0xFFB0) — same HID protocol
- Power cap: **80% for both** (9.6Nm on 12Nm, 6.4Nm on 8Nm)
- Presets must work on both torque levels

### Shutdown Sequence (routine session end)
1. **Close ConspitLink** — send WM_CLOSE with **5 second timeout**
2. If ConspitLink doesn't close within 5s: **skip and send HID commands anyway** (accept P-20 risk, don't force-kill)
3. **Send HID commands** — `fxm.reset` (clear orphaned DirectInput effects) then `axis.idlespring` (apply centering spring)
4. **Restart ConspitLink immediately** after HID commands — don't wait for wheel to physically center
5. **Block main loop briefly (2-3s)** during sequence — no new session can start until wheel is safe

### Centering Behavior
- After session end: wheel gets a **gentle centering spring** — returns to center if pushed but feels loose/light
- Centering spring ramps from 0 to target over **500ms** (minimum safe ramp)
- Ramp method: **Claude decides after testing** — try single idlespring command first, add stepped HID writes if it snaps
- Per-game customization: **Claude decides** — start universal, customize only if testing reveals game-specific issues

### Emergency ESTOP (separate code path)
- ESTOP fires in these scenarios ONLY:
  1. **Panic hook** — rc-agent crash (no time for gentle sequence)
  2. **USB disconnect** — wheelbase connection drops
  3. **Manual trigger** — staff/admin sends explicit emergency stop
  4. **ConspitLink won't close** — escalation if WM_CLOSE fails AND HID commands also fail
- Recovery after ESTOP: **Claude decides** based on safety analysis

### Testing Strategy
- Canary pod: **whichever currently has the 8Nm** — less injury risk during testing
- Validation: **manual test all 4 games** — launch each, play briefly, end session, confirm wheel centers
- Must pass: wheel centers every time on all 4 games (AC, F1 25, ACC/AC EVO, AC Rally)

### Claude's Discretion
- Exact idlespring value/range (needs empirical testing on hardware)
- Whether `fxm.reset` is available on Conspit's OpenFFBoard fork (test first)
- Ramp implementation (single command vs stepped writes)
- Per-game session-end differences (start universal)
- ESTOP recovery behavior (try gentle centering after, or stay limp)
- Exact WM_CLOSE implementation for ConspitLink window (reuse overlay.rs pattern)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### FFB Controller (existing code to modify)
- `crates/rc-agent/src/ffb_controller.rs` — Current ESTOP + FFB_ACTIVE implementation, send_vendor_cmd_to_class() for adding new commands
- `crates/rc-agent/src/main.rs` lines 1300-2730 — 8 session-end call sites where FFB zero is invoked before enforce_safe_state()

### ConspitLink Process Management (existing code to modify)
- `crates/rc-agent/src/ac_launcher.rs` lines 1332-1409 — ensure_conspit_link_running(), minimize_conspit_window()
- `crates/rc-agent/src/ac_launcher.rs` lines 1586-1618 — enforce_safe_state() (primary session-end entry point)

### WM_CLOSE Pattern (reusable)
- `crates/rc-agent/src/overlay.rs` lines 1035-1056 — Existing WM_CLOSE via PostMessageW, reusable pattern for ConspitLink

### Research
- `.planning/research/conspit-link/STACK.md` — OpenFFBoard HID commands (idlespring, fxm.reset, curpos, power)
- `.planning/research/conspit-link/PITFALLS.md` — P-20 contention, orphaned DirectInput effects, snap-back injury risk
- `.planning/research/conspit-link/ARCHITECTURE.md` — ConspitLink runtime path discovery (C:\RacingPoint\Global.json)

### Requirements
- `.planning/REQUIREMENTS.md` — SAFE-01 through SAFE-07 (v10.0 section)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `FfbController` struct with `open_vendor_device()` — already handles HID open/close with VID/PID filtering
- `send_vendor_cmd_to_class(class_id, cmd_id, data)` — generic HID command sender, supports any class (add fxm.reset, axis.idlespring, axis.power)
- `zero_force_with_retry(attempts, delay_ms)` — retry pattern reusable for new commands
- `PostMessageW(hwnd, WM_CLOSE, 0, 0)` in overlay.rs — exact pattern needed for ConspitLink close
- `is_process_running("ConspitLink2.0.exe")` — already exists for watchdog

### Established Patterns
- Session-end: `spawn_blocking(|| ffb.zero_force())` then `enforce_safe_state()` — modify to new sequence
- ConspitLink restart: `cmd /c start "" <path>` with 4s delay then minimize — reuse for post-HID restart
- HID report format: 26-byte vendor report (Report_ID 0xA1, class/cmd/data layout) — same for all new commands

### Integration Points
- 8 session-end call sites in main.rs — all need to switch from `zero_force()` to new `safe_session_end()` function
- `enforce_safe_state()` currently calls `ensure_conspit_link_running()` — needs coordination with new close/restart cycle
- Panic hook in main.rs — needs to keep using ESTOP (sync-safe, no async)
- `FfbZeroed` WebSocket message to server — rename or add `FfbCentered` variant

</code_context>

<specifics>
## Specific Ideas

- The fleet is primarily 12Nm with 8Nm as hot-swap spare — both must work with same session-end sequence
- Session-end must block briefly (2-3s) so no new session starts on a stuck wheel
- ConspitLink is never force-killed — WM_CLOSE only, with graceful fallback to "skip and try HID anyway"
- Power cap is 80% universally (applied via `axis.power` HID command at startup)

</specifics>

<deferred>
## Deferred Ideas

- Per-game session-end customization — start universal, revisit in Phase 61 (FFB Preset Tuning) if needed
- `axis.curpos` position verification — nice to have but not blocking for Phase 57
- Crash-count tracking for ConspitLink — Phase 58 (Process Hardening)
- Config file backup/verification — Phase 58 (Process Hardening)

</deferred>

---

*Phase: 57-session-end-safety*
*Context gathered: 2026-03-20*
