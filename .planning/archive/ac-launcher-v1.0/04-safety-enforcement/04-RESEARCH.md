# Phase 4: Safety Enforcement - Research

**Researched:** 2026-03-14
**Domain:** AC INI safety overrides, OpenFFBoard HID FFB zeroing, game crash detection
**Confidence:** HIGH

## Summary

Phase 4 addresses three safety-critical requirements: locking tyre grip at 100%, locking damage at 0%, and zeroing FFB torque before game kill. The codebase is ~85% ready -- FFB zeroing infrastructure (`FfbController::zero_force()`) already exists and is called at startup and all session-end paths, but the **ordering is wrong** in most paths: FFB is zeroed AFTER game kill, not before. The grip/damage override requires only small INI writer changes plus a post-write verification step.

The research domain is well-understood: AC's INI format for grip and damage, OpenFFBoard's HID vendor protocol for FFB zeroing, and Windows process polling for crash detection. All three are established patterns in this codebase with existing code to extend.

**Primary recommendation:** Fix FFB zeroing ORDER in all session-end paths (currently zero happens after game.stop()), hardcode DAMAGE=0 in both INI writers (race.ini + assists.ini), add DYNAMIC_TRACK SESSION_START=100 for grip enforcement, and add post-write verification. Defer Conspit Link preset automation to a future phase (no CLI/API available).

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions

**FFB Zeroing Timing:**
- Zero at ALL transition points: session end (normal), game crash, rc-agent startup. Belt-and-suspenders approach.
- 500ms delay after zeroing before killing game process. Gives HID command time to reach wheelbase.
- Failure handling: If wheelbase is disconnected (USB gone), log warning and continue with game kill. Don't block the session-end sequence.
- Report to core: Send a WebSocket message to rc-core when FFB is zeroed, so dashboard shows per-pod FFB safety status.

**Conspit Link Preset Selection:**
- Auto-select game preset in Conspit Link when launching AC, so the steering wheel display works correctly.
- Needs investigation: Unknown if Conspit Link 2.0 has a config file, CLI, or requires UI automation. Researcher should investigate.

**Grip & Damage Override Scope:**
- Tyre Grip: Always 100%, no exceptions. Even staff cannot override. Enforced in both race.ini (single-player) and server_cfg.ini (multiplayer).
- Damage Multiplier: Always 0%, no exceptions. Enforced in both race.ini and server_cfg.ini.
- Customer PWA: Damage/grip settings hidden completely. Customers never see these options.
- Staff kiosk: Settings visible but locked/read-only. Shows "100% grip / 0% damage" with explanation why.
- Post-write verification: After writing race.ini, re-read and verify grip=100%/damage=0% before launching AC. If verification fails, refuse to launch.

**Session End Sequence:**
- Ordering: FFB zero -> 500ms wait -> kill acs.exe + Content Manager -> window cleanup -> lock screen re-engage
- Kill CM in same sequence as AC (cleanup_after_session() already does this)
- Trigger: Core sends StopGame via WebSocket, agent runs the full safe sequence
- Lock screen: Always re-engages after cleanup. Customer sees PIN/QR screen.

**Game Crash Safety:**
- Detection: Process monitor -- poll acs.exe existence every 2-3 seconds. If process disappears while billing is active, that's a crash.
- Crash response: Zero FFB immediately -> notify core (GameCrash message) -> wait for core's decision
- Billing during crash: Pause billing using existing PausedGamePause state from Phase 3
- Auto-relaunch: Core allows 1 retry. First crash = auto-relaunch with same settings. Second crash in same session = end session (matches Phase 3 launch failure pattern).

### Claude's Discretion
- Exact process polling interval (2-3 seconds suggested)
- Whether to use a separate crash-detection thread or integrate into existing main loop
- HID command retry logic within the 500ms window
- Crash notification message format for WebSocket protocol
- Whether to add a SafetyEvent enum for dashboard event bus

### Deferred Ideas (OUT OF SCOPE)
- USB mass storage lockdown (Group Policy / registry) -- separate infrastructure phase
- Conspit Link preset management UI in staff kiosk -- future phase
- Mid-session assist toggles (DIFF-06 through DIFF-10) -- Phase 6

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BILL-03 | Tyre Grip is always 100% -- enforced in race.ini and server config, not overridable | Grip enforcement via DYNAMIC_TRACK SESSION_START=100 in race.ini (already present) + server_cfg.ini DYNAMIC_TRACK section (needs override). AC has no `TYRE_GRIP` field in race.ini; grip is controlled by DYNAMIC_TRACK.SESSION_START. |
| BILL-04 | Damage Multiplier is always 0% -- enforced in race.ini and server config, not overridable | DAMAGE=0 in [ASSISTS] section of race.ini + assists.ini + DAMAGE_MULTIPLIER=0 in server_cfg.ini. Currently DAMAGE comes from user params -- must hardcode to 0. |
| BILL-05 | FFB torque zeroed on wheelbase BEFORE game process is killed (safety ordering) | FfbController::zero_force() exists and works. Current issue: ALL session-end paths call it AFTER game.stop(), not before. Must reorder + add 500ms delay. |

</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| hidapi | (in Cargo.toml) | HID device access for OpenFFBoard vendor commands | Already used by FfbController -- the only way to send FFB zero to Conspit Ares wheelbase |
| sysinfo | (in Cargo.toml) | Process alive checks for crash detection | Already used in game_process.rs for orphan cleanup and PID scanning |
| serde/serde_json | (in Cargo.toml) | WebSocket protocol message serialization | Established pattern for AgentMessage/CoreToAgentMessage enums |
| tokio | (in Cargo.toml) | Async runtime, spawn_blocking for HID writes | All async coordination in main.rs uses tokio |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dirs-next | (in Cargo.toml) | Find AC Documents/cfg path for race.ini verification | Already used in write_race_ini() and write_assists_ini() |
| winapi | (in Cargo.toml) | Windows process handles for is_process_alive() | Already used in game_process.rs |
| tracing | (in Cargo.toml) | Structured logging for safety events | All logging in codebase uses tracing |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Process polling for crash detection | Windows WMI event subscription | WMI is more efficient but adds complexity; 2s polling via existing game_check_interval is already in place and simple |
| Direct HID for FFB zero | Conspit Link API | Conspit Link has NO CLI/API (confirmed by research); direct HID is the only option |

**Installation:** No new dependencies required. All libraries already in Cargo.toml.

## Architecture Patterns

### Existing Code Topology (relevant files)

```
crates/rc-agent/src/
  main.rs           # Main event loop with game_check_interval (2s), StopGame handler, crash recovery timer
  ffb_controller.rs # FfbController::zero_force() -- OpenFFBoard vendor HID e-stop
  ac_launcher.rs    # write_race_ini(), write_assists_ini(), cleanup_after_session(), enforce_safe_state()
  game_process.rs   # GameProcess::is_running(), ::stop(), crash detection
  ai_debugger.rs    # PodStateSnapshot, crash analysis (unmodified)

crates/rc-common/src/
  protocol.rs       # AgentMessage, CoreToAgentMessage enums (needs new variants)
  types.rs          # AcLanSessionConfig (damage_multiplier field, needs default override)

crates/rc-core/src/
  ac_server.rs      # generate_server_cfg_ini() (DAMAGE_MULTIPLIER from config, needs override)
```

### Pattern 1: Safe Session-End Sequence (CURRENT vs. REQUIRED)

**Current (WRONG ordering) in ALL session-end paths:**
```
// BillingStopped, SessionEnded, SubSessionEnded, crash recovery, disconnect -- ALL do this:
lock_screen.show_active_session(...);
tokio::time::sleep(500ms).await;
if let Some(ref mut game) = game_process {
    let _ = game.stop();       // <-- Game killed FIRST
    game_process = None;
}
{ let f = ffb.clone();
  tokio::task::spawn_blocking(move || {
    f.zero_force().ok();       // <-- FFB zeroed AFTER (too late!)
    ac_launcher::enforce_safe_state();
  });
}
```

**Required (CORRECT ordering):**
```
// Step 0: Zero FFB BEFORE anything else
let f = ffb.clone();
tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok();
tokio::time::sleep(Duration::from_millis(500)).await;

// Step 1: Lock screen covers desktop
lock_screen.show_active_session(...);

// Step 2: Kill game process AFTER FFB is zeroed
if let Some(ref mut game) = game_process {
    let _ = game.stop();
    game_process = None;
}

// Step 3: Full cleanup
ac_launcher::enforce_safe_state();

// Step 4: Report FFB status to core
let msg = AgentMessage::FfbZeroed { pod_id: pod_id.clone() };
```

### Pattern 2: INI Safety Hardcoding

**What:** Override user-provided damage value with hardcoded 0 in write_assists_section() and write_assists_ini(). Verify DYNAMIC_TRACK.SESSION_START stays at 100.
**When to use:** Every time race.ini or assists.ini is written.
**Example:**
```rust
// In write_assists_section():
// BEFORE (unsafe -- damage from user params):
let damage = params.conditions.as_ref().map(|c| c.damage).unwrap_or(0);

// AFTER (safe -- always 0):
let _ = writeln!(ini, "DAMAGE=0");  // SAFETY: always 0, ignoring params.conditions.damage
let _ = writeln!(ini, "VISUAL_DAMAGE=0");
```

### Pattern 3: Post-Write Verification

**What:** After writing race.ini, re-read the file and verify safety-critical values.
**When to use:** In the launch sequence, between write_race_ini() and acs.exe launch.
**Example:**
```rust
fn verify_safety_settings(race_ini_path: &Path) -> Result<()> {
    let content = std::fs::read_to_string(race_ini_path)?;

    // Check DAMAGE=0 in [ASSISTS]
    if !content.contains("DAMAGE=0") {
        anyhow::bail!("SAFETY: race.ini DAMAGE is not 0 -- refusing to launch");
    }

    // Check SESSION_START=100 in [DYNAMIC_TRACK]
    if !content.contains("SESSION_START=100") {
        anyhow::bail!("SAFETY: race.ini SESSION_START is not 100 -- refusing to launch");
    }

    tracing::info!("Safety verification passed: DAMAGE=0, SESSION_START=100");
    Ok(())
}
```

### Pattern 4: Game Crash Detection (Enhancement of Existing)

**What:** The existing `game_check_interval` (2s tick in main.rs line 689) already detects game crashes and sends GameStateUpdate with GameState::Error. Enhancement needed: zero FFB immediately on crash detection (currently only happens in the enforce_safe_state() call which is spawned but NOT awaited).
**When to use:** In the game_check_interval handler when `!still_alive && was_active`.
**Example:**
```rust
// In the crash detection branch (main.rs ~line 713-776):
if !still_alive && was_active {
    // FIRST: Zero FFB immediately (safety-critical)
    { let f = ffb.clone();
      tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }

    // THEN: existing crash handling (GameStateUpdate, AI debugger, etc.)
    // ...
}
```

### Anti-Patterns to Avoid
- **Spawning FFB zero without awaiting:** Current code uses `tokio::task::spawn_blocking(...)` without `.await` -- the FFB zero runs concurrently with game kill. Must `.await.ok()` to ensure ordering.
- **Reading damage from user params:** Never trust `params.conditions.damage` for the INI writer. Always hardcode 0.
- **Grip via TYRE_GRIP field:** AC does NOT have a `TYRE_GRIP` or `GRIP_LEVEL` field in race.ini. Grip is controlled by `DYNAMIC_TRACK.SESSION_START` (already set to 100 in the codebase) and the server's DYNAMIC_TRACK config.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| FFB zeroing | Custom FFB protocol implementation | Existing `FfbController::zero_force()` | Already proven on all 8 pods; uses correct OpenFFBoard vendor HID protocol (report ID 0xA1, CMD_ESTOP 0x0A) |
| Game crash detection | Custom process watcher thread | Existing `game_check_interval` (2s tick) + `GameProcess::is_running()` | Already detects crashes, sends GameStateUpdate, triggers AI debugger; just needs FFB zero added to the handler |
| INI parsing for verification | Full INI parser library | Simple `content.contains()` checks | We control the INI writer; exact string matching is reliable and avoids parser dependency |
| Process killing | Direct Windows API | Existing `game.stop()` + `ac_launcher::enforce_safe_state()` | Already handles Child/PID/persisted-PID fallback; kills all game processes + error dialogs |
| WebSocket protocol messages | Custom serialization | Existing serde-tagged enum pattern in `AgentMessage` / `CoreToAgentMessage` | Add new variants to existing enums; serde roundtrip tests verify correctness |

**Key insight:** This phase is 85% reordering/hardcoding existing code, 15% new code (verification function, new WS messages, crash FFB zero). Do NOT refactor the session-end paths into a shared function -- there are 6+ places with slightly different behavior (lock screen text, billing state, overlay, etc.). Fix each in place.

## Common Pitfalls

### Pitfall 1: FFB Zero Spawned But Not Awaited
**What goes wrong:** `tokio::task::spawn_blocking(move || { f.zero_force().ok(); })` returns a JoinHandle that is dropped -- the FFB zero races with `game.stop()`. On fast machines, the game process dies before the HID write reaches the wheelbase.
**Why it happens:** The current code pattern treats FFB zero as "fire and forget" alongside `enforce_safe_state()`.
**How to avoid:** `.await.ok()` on the spawn_blocking call. The HID write takes <200ms (confirmed in archive pitfall P-19). The 500ms delay after gives buffer.
**Warning signs:** Wheel still has force after session end; force releases only when rc-agent restarts (startup zero).

### Pitfall 2: ConspitLink Undoing FFB Zero
**What goes wrong:** ConspitLink2.0 maintains its own HID handle to the wheelbase and may send a keep-alive or re-enable command that overrides the FFB zero (documented in archive pitfall P-20).
**Why it happens:** ConspitLink periodically writes to the OpenFFBoard device for its telemetry display and configuration.
**How to avoid:** The current FfbController uses the OpenFFBoard VENDOR interface (usage page 0xFF00, report ID 0xA1) which is separate from the DirectInput game FFB interface. The CMD_ESTOP command (0x0A) is an emergency stop that disables the motor at the firmware level -- ConspitLink's game FFB commands cannot override it because the motor is disabled at a lower priority level. This is confirmed by the existing code comment and the fact that startup FFB zero works reliably.
**Warning signs:** FFB zero log says "sent" but wheelbase retains force. If observed, ConspitLink interaction is the likely cause.

### Pitfall 3: Missing Grip Enforcement in Server Config
**What goes wrong:** Single-player race.ini has `DYNAMIC_TRACK.SESSION_START=100` (already enforced), but the multiplayer server config's DYNAMIC_TRACK section uses values from `AcLanSessionConfig` which defaults to different values.
**Why it happens:** `AcLanSessionConfig::default()` uses `session_start: 100` but the dynamic_track struct could be overridden by staff/presets.
**How to avoid:** In `generate_server_cfg_ini()`, always override `dt.session_start` to 100 and `config.damage_multiplier` to 0, regardless of what the config says.
**Warning signs:** Multiplayer sessions have reduced grip or damage enabled.

### Pitfall 4: Assists.ini DAMAGE Not Hardcoded
**What goes wrong:** AC and CSP may read assists from `assists.ini` instead of race.ini's [ASSISTS] section. If only race.ini is hardcoded to DAMAGE=0 but assists.ini still uses user params, damage could be non-zero.
**Why it happens:** `write_assists_ini()` at line 769 reads `params.conditions.damage` -- same unsafe pattern as race.ini.
**How to avoid:** Hardcode DAMAGE=0 in BOTH `write_assists_section()` (for race.ini) AND `write_assists_ini()` (for assists.ini).
**Warning signs:** Single-player sessions show damage effects despite race.ini having DAMAGE=0.

### Pitfall 5: StopGame Handler Missing FFB Zero
**What goes wrong:** The `StopGame` handler in main.rs (line 1308-1338) does NOT call `ffb.zero_force()` at all. It directly calls `game.stop()` without any FFB safety.
**Why it happens:** StopGame was implemented before FFB safety was a requirement. Other session-end paths (BillingStopped, SessionEnded) do call FFB zero, but StopGame was missed.
**How to avoid:** Add FFB zero + 500ms delay to the StopGame handler, following the same pattern as the other session-end paths.
**Warning signs:** Manual "Stop Game" from dashboard leaves wheel with force applied.

### Pitfall 6: Crash Detection FFB Zero Not Immediate
**What goes wrong:** When a game crashes during active billing (line 769), the code arms a 30s crash recovery timer but does NOT zero FFB immediately. The FFB stays at whatever force level the game last commanded, potentially for 30 seconds.
**Why it happens:** The billing-active branch only arms the timer and does not call enforce_safe_state() (which includes FFB zero).
**How to avoid:** Zero FFB immediately when crash is detected during billing, BEFORE arming the 30s timer.
**Warning signs:** Wheel has sustained force for 30 seconds after game crash before recovery timer fires.

## Code Examples

### Example 1: Hardcoded DAMAGE=0 in write_assists_section()
```rust
// Source: ac_launcher.rs:484-502
fn write_assists_section(ini: &mut String, params: &AcLaunchParams) {
    let aids = params.aids.clone().unwrap_or_default();
    // REMOVED: let damage = params.conditions.as_ref().map(|c| c.damage).unwrap_or(0);
    let auto_shifter = if params.transmission == "auto" || params.transmission == "automatic" { 1 } else { 0 };

    let _ = writeln!(ini, "[ASSISTS]");
    let _ = writeln!(ini, "ABS={}", aids.abs);
    let _ = writeln!(ini, "AUTO_CLUTCH={}", aids.autoclutch);
    let _ = writeln!(ini, "AUTO_SHIFTER={}", auto_shifter);
    let _ = writeln!(ini, "DAMAGE=0");           // SAFETY: always 0, never from params
    let _ = writeln!(ini, "IDEAL_LINE={}", aids.ideal_line);
    let _ = writeln!(ini, "STABILITY={}", aids.stability);
    let _ = writeln!(ini, "TRACTION_CONTROL={}", aids.tc);
    let _ = writeln!(ini, "VISUAL_DAMAGE=0");
    let _ = writeln!(ini, "SLIPSTREAM=1");
    let _ = writeln!(ini, "TYRE_BLANKETS=1");
    let _ = writeln!(ini, "AUTO_BLIP=1");
    let _ = writeln!(ini, "FUEL_RATE=1");
}
```

### Example 2: Server Config Safety Override
```rust
// Source: rc-core/src/ac_server.rs:190, in generate_server_cfg_ini()
// Override safety-critical values regardless of config
let safe_damage = 0;  // SAFETY: never allow damage in multiplayer
let safe_session_start = 100;  // SAFETY: always 100% grip

// In the format string, use safe_damage instead of config.damage_multiplier
// In the DYNAMIC_TRACK section, override dt.session_start with safe_session_start
```

### Example 3: New WebSocket Message Variant
```rust
// Source: rc-common/src/protocol.rs -- add to AgentMessage enum
/// Agent reports FFB safety action completed
FfbZeroed { pod_id: String },

/// Agent reports game crash detected (process disappeared while billing active)
GameCrashed {
    pod_id: String,
    sim_type: SimType,
    exit_code: Option<i32>,
    billing_active: bool,
},
```

### Example 4: Post-Write Verification
```rust
// New function in ac_launcher.rs
fn verify_safety_settings() -> Result<()> {
    let race_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("race.ini");

    let content = std::fs::read_to_string(&race_ini_path)
        .map_err(|e| anyhow::anyhow!("Cannot read race.ini for safety verification: {}", e))?;

    // Verify DAMAGE=0 (not DAMAGE=1, DAMAGE=50, etc.)
    let has_safe_damage = content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "DAMAGE=0"
    });
    if !has_safe_damage {
        anyhow::bail!("SAFETY VIOLATION: race.ini DAMAGE is not 0 -- refusing to launch AC");
    }

    // Verify SESSION_START=100
    let has_safe_grip = content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "SESSION_START=100"
    });
    if !has_safe_grip {
        anyhow::bail!("SAFETY VIOLATION: race.ini SESSION_START is not 100 -- refusing to launch AC");
    }

    tracing::info!("[safety] Post-write verification passed: DAMAGE=0, SESSION_START=100");
    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| FFB zero after game kill | FFB zero BEFORE game kill with 500ms delay | This phase | Safety-critical ordering fix |
| DAMAGE from user params | DAMAGE always hardcoded 0 | This phase | Prevents equipment damage |
| No post-write verification | Verify race.ini before launch | This phase | Defense-in-depth against bugs |
| Crash detection without FFB zero | Immediate FFB zero on crash detection | This phase | Prevents sustained force on crash |

**Deprecated/outdated:**
- `cleanup_after_session()` function is marked as dead code (compiler warning) -- it is superseded by `enforce_safe_state()` but still referenced in CONTEXT.md. Both need FFB ordering fix.

## Conspit Link Preset Automation -- Investigation Results

**Finding: NO CLI/API available.** Conspit Link 2.0 is a WPF GUI application with no documented command-line interface, configuration file API, or inter-process communication protocol. Research checked:
1. Official Conspit documentation (PW1 tutorial, H.AO function guide, 300 GT guide) -- GUI-only config management
2. Install directory on pods (`C:\Program Files (x86)\Conspit Link 2.0\`) -- no CLI flags documented
3. Web search for automation/config file/preset switching -- no results
4. Config system: GUI-based "Config" section at bottom-left of each settings page; supports save/rename/export/import but only through the GUI

**Recommendation:** Defer Conspit Link preset auto-selection to a future phase. Current workaround is adequate: Conspit Link auto-detects the running game via shared memory (it reads AC's telemetry independently). The steering wheel display shows telemetry data when AC is running, regardless of which "game preset" is selected in Conspit Link. The preset primarily affects LCD layout, not safety or functionality.

**Confidence:** MEDIUM -- Conspit Link may have undocumented config files in AppData or registry. A future investigation could scan `%AppData%\Conspit` or `HKCU\Software\Conspit` on a pod. But for this phase, it is out of scope per the deferred items list.

## Open Questions

1. **Conspit Link config file location**
   - What we know: WPF app, install at `C:\Program Files (x86)\Conspit Link 2.0\`, no documented CLI
   - What's unclear: Whether it stores presets in AppData, registry, or install directory
   - Recommendation: Defer to future phase. Not blocking for safety enforcement.

2. **GameCrashed vs GameStateUpdate message**
   - What we know: Current crash detection sends `AgentMessage::GameStateUpdate` with `GameState::Error`
   - What's unclear: Whether core already handles this for billing pause, or if a new `GameCrashed` variant is needed
   - Recommendation: Claude's discretion. Reusing existing `GameStateUpdate(GameState::Error)` is simpler; adding `GameCrashed` is more explicit. Either works for billing pause since core already watches for GameState::Error.

3. **DYNAMIC_TRACK in server_cfg.ini override scope**
   - What we know: `AcLanSessionConfig` has a `dynamic_track: AcDynamicTrackConfig` field; default session_start is unclear in the struct
   - What's unclear: Whether the default AcDynamicTrackConfig already sets session_start=100
   - Recommendation: Hardcode session_start=100 in `generate_server_cfg_ini()` regardless of config value. Belt-and-suspenders.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in, stable toolchain 1.93.1) |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p rc-agent -- --test-threads=1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BILL-03 | DYNAMIC_TRACK SESSION_START=100 in race.ini | unit | `cargo test -p rc-agent -- test_write_race_ini --test-threads=1` | Existing tests check sections but NOT SESSION_START value -- Wave 0 |
| BILL-03 | Grip=100 in server_cfg.ini DYNAMIC_TRACK | unit | `cargo test -p rc-core -- test_generate_server_cfg --test-threads=1` | Existing test checks output format -- needs safety assertion -- Wave 0 |
| BILL-04 | DAMAGE=0 in race.ini [ASSISTS] | unit | `cargo test -p rc-agent -- test_write_race_ini_damage --test-threads=1` | Existing `test_write_race_ini_practice_with_aids` checks DAMAGE from aids but does NOT verify hardcoded 0 -- Wave 0 |
| BILL-04 | DAMAGE=0 in assists.ini | unit | `cargo test -p rc-agent -- test_write_assists_ini --test-threads=1` | No existing test -- Wave 0 |
| BILL-04 | DAMAGE_MULTIPLIER=0 in server_cfg.ini | unit | `cargo test -p rc-core -- test_server_cfg_damage --test-threads=1` | No existing test for safety override -- Wave 0 |
| BILL-05 | Post-write verification rejects bad values | unit | `cargo test -p rc-agent -- test_verify_safety --test-threads=1` | No existing test -- Wave 0 |
| BILL-05 | FfbController::zero_force() command format | unit | `cargo test -p rc-agent -- test_vendor_cmd_buffer --test-threads=1` | Existing |
| BILL-05 | New WebSocket message roundtrip | unit | `cargo test -p rc-common -- test_ffb_zeroed_roundtrip --test-threads=1` | No existing test -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent && cargo test -p rc-common`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `ac_launcher::tests::test_write_race_ini_damage_always_zero` -- verify DAMAGE=0 regardless of params.conditions.damage value
- [ ] `ac_launcher::tests::test_write_race_ini_grip_always_100` -- verify SESSION_START=100 in DYNAMIC_TRACK
- [ ] `ac_launcher::tests::test_verify_safety_settings_passes` -- verify_safety_settings() accepts correct file
- [ ] `ac_launcher::tests::test_verify_safety_settings_rejects_damage` -- verify_safety_settings() rejects DAMAGE != 0
- [ ] `ac_launcher::tests::test_verify_safety_settings_rejects_grip` -- verify_safety_settings() rejects SESSION_START != 100
- [ ] `ac_launcher::tests::test_write_assists_ini_damage_always_zero` -- assists.ini also hardcodes DAMAGE=0
- [ ] `protocol::tests::test_ffb_zeroed_roundtrip` -- new FfbZeroed message serde roundtrip
- [ ] `protocol::tests::test_game_crashed_roundtrip` -- new GameCrashed message serde roundtrip (if added)
- [ ] `ac_server::tests::test_server_cfg_damage_always_zero` -- verify DAMAGE_MULTIPLIER=0 in generated server config
- [ ] `ac_server::tests::test_server_cfg_grip_always_100` -- verify SESSION_START=100 in generated server config DYNAMIC_TRACK

## Sources

### Primary (HIGH confidence)
- **Codebase inspection** -- all source files in crates/rc-agent/src/ and crates/rc-common/src/ and crates/rc-core/src/
- **Archive research** -- `.planning/archive/hud-safety/research/PITFALLS.md` (P-19, P-20, P-21) for FFB safety edge cases
- **AC DYNAMIC_TRACK docs** -- [FreakHosting guide](https://help.freakhosting.com/games/assetto-corsa/configuring-dynamic-track-rubbering-in) confirms SESSION_START=100 means 100% grip
- **Steam community** -- [Dynamic track zero grip](https://steamcommunity.com/app/244210/discussions/0/618453594740179518/) confirms SESSION_START controls grip percentage
- **Kunos forum** -- [AC Dedicated Server Manual](https://www.assettocorsa.net/forum/index.php?faq/assetto-corsa-dedicated-server-manual.28/) for server_cfg.ini format

### Secondary (MEDIUM confidence)
- **Conspit documentation** -- [PW1 tutorial](https://oss.conspit.com/file/4/b/1c/PW1%E9%A9%B1%E5%8A%A8%E6%95%99%E7%A8%8BEN%20V1.1.pdf) and [300 GT guide](https://oss.conspit.com/file/1/7/96/CONSPIT_300%20GT_FunctionGuide_EN%20v1.0.pdf) for Conspit Link 2.0 features (GUI-only config management)

### Tertiary (LOW confidence)
- **Conspit Link automation** -- No evidence found for CLI/API; conclusion based on absence of documentation. May have undocumented features.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies
- Architecture: HIGH -- codebase fully inspected, patterns established, changes are surgical
- Pitfalls: HIGH -- prior archive research (P-19 through P-21) covers FFB edge cases; INI format confirmed via community docs
- Conspit Link automation: LOW -- based on absence of documentation, not positive confirmation

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable domain -- AC INI format and OpenFFBoard protocol are not changing)
