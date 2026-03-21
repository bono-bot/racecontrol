# Phase 4: Safety Enforcement - Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Safety-critical settings are always enforced regardless of session type, and force feedback is handled safely at session boundaries. Covers: tyre grip locked at 100%, damage locked at 0%, FFB zeroed before game kill, crash detection with FFB safety.

</domain>

<decisions>
## Implementation Decisions

### FFB Zeroing Timing
- **Zero at ALL transition points:** session end (normal), game crash, rc-agent startup. Belt-and-suspenders approach.
- **500ms delay** after zeroing before killing game process. Gives HID command time to reach wheelbase.
- **Failure handling:** If wheelbase is disconnected (USB gone), log warning and continue with game kill. Don't block the session-end sequence.
- **Report to core:** Send a WebSocket message to rc-core when FFB is zeroed, so dashboard shows per-pod FFB safety status.

### Conspit Link Preset Selection
- **Auto-select game preset** in Conspit Link when launching AC, so the steering wheel display works correctly.
- **Needs investigation:** Unknown if Conspit Link 2.0 has a config file, CLI, or requires UI automation. Researcher should investigate.

### Grip & Damage Override Scope
- **Tyre Grip:** Always 100%, no exceptions. Even staff cannot override. Enforced in both race.ini (single-player) and server_cfg.ini (multiplayer).
- **Damage Multiplier:** Always 0%, no exceptions. Enforced in both race.ini and server_cfg.ini.
- **Customer PWA:** Damage/grip settings hidden completely. Customers never see these options.
- **Staff kiosk:** Settings visible but locked/read-only. Shows "100% grip / 0% damage" with explanation why.
- **Post-write verification:** After writing race.ini, re-read and verify grip=100%/damage=0% before launching AC. If verification fails, refuse to launch.

### Session End Sequence
- **Ordering:** FFB zero -> 500ms wait -> kill acs.exe + Content Manager -> window cleanup -> lock screen re-engage
- **Kill CM in same sequence** as AC (cleanup_after_session() already does this)
- **Trigger:** Core sends StopGame via WebSocket, agent runs the full safe sequence
- **Lock screen:** Always re-engages after cleanup. Customer sees PIN/QR screen.

### Game Crash Safety
- **Detection:** Process monitor — poll acs.exe existence every 2-3 seconds. If process disappears while billing is active, that's a crash.
- **Crash response:** Zero FFB immediately -> notify core (GameCrash message) -> wait for core's decision
- **Billing during crash:** Pause billing using existing PausedGamePause state from Phase 3
- **Auto-relaunch:** Core allows 1 retry. First crash = auto-relaunch with same settings. Second crash in same session = end session (matches Phase 3 launch failure pattern).

### Claude's Discretion
- Exact process polling interval (2-3 seconds suggested)
- Whether to use a separate crash-detection thread or integrate into existing main loop
- HID command retry logic within the 500ms window
- Crash notification message format for WebSocket protocol
- Whether to add a `SafetyEvent` enum for dashboard event bus

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ffb_controller.rs`: `FfbController` with `zero_torque()` method — uses OpenFFBoard HID interface (VID:0x1209, PID:0xFFB0, usage page 0xFF00)
- `cleanup_after_session()` in `ac_launcher.rs:1242` — kills AC/CM, dismisses dialogs, re-engages lock screen. Needs FFB zeroing step added BEFORE game kill.
- `enforce_safe_state()` in `ac_launcher.rs:1297` — broader cleanup. Also needs FFB zeroing.
- `ai_debugger.rs` — `try_auto_fix()` already detects stale acs.exe processes and kills them. Could be extended for crash detection.
- Phase 3's `PausedGamePause` billing state — reuse for crash-pause billing

### Established Patterns
- INI writer: composable section writers in `ac_launcher.rs` (18 functions). Grip/damage go in `write_assists_section()` and `write_realism_section()`.
- WebSocket protocol: `AgentMessage` enum for agent->core, `CoreToAgentMessage` for core->agent. New messages need serde round-trip tests in rc-common.
- ac_launcher.rs already writes `DAMAGE=0` and `TYRE_BLANKETS=1`, but damage comes from user params (not enforced).
- No `TYRE_GRIP` or `GRIP_LEVEL` currently in the INI writer — needs to be added.

### Integration Points
- `cleanup_after_session()` — insert FFB zeroing as step 0
- `enforce_safe_state()` — insert FFB zeroing as step 0
- `write_assists_section()` — hardcode DAMAGE=0 regardless of params
- `write_realism_section()` — add GRIP_LEVEL=100 enforcement
- `generate_server_cfg_ini()` — enforce grip/damage in server config too
- `StopGame` handler in main.rs — call safe session-end sequence

</code_context>

<specifics>
## Specific Ideas

- Conspit Link has per-game presets that control the steering wheel LCD display. rc-agent should auto-select the correct preset when launching AC. Automation method unknown — needs research.
- The safe session-end sequence mirrors Phase 3's launch-failure pattern: one retry on crash, end session on double failure. Keeps customer-facing behavior consistent.

</specifics>

<deferred>
## Deferred Ideas

- USB mass storage lockdown (Group Policy / registry) — separate infrastructure phase
- Conspit Link preset management UI in staff kiosk — future phase
- Mid-session assist toggles (DIFF-06 through DIFF-10) — Phase 6

</deferred>

---

*Phase: 04-safety-enforcement*
*Context gathered: 2026-03-14*
