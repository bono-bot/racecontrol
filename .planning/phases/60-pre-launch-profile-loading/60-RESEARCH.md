# Phase 60: Pre-Launch Profile Loading - Research

**Researched:** 2026-03-24
**Domain:** rc-agent FFB preset loading, ConspitLink config control, tokio async patterns in Rust
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Pre-Launch Hook Location**
- Insert in `ws_handler.rs` LaunchGame handler, after safe mode entry but before game process spawn
- New function `pre_load_game_preset(sim_type: SimType)` in `ffb_controller.rs` — called from LaunchGame handler
- Lookup table maps `SimType` → ConspitLink game key string (reuses VENUE_GAME_KEYS constants from Phase 59)
- Brief block (2-3s max) before game spawn to ensure preset is loaded — FFB must be correct from first input

**Preset Loading Mechanism**
- Wait 3s for ConspitLink auto-detect (Phase 59) to switch preset first
- If CL auto-detect doesn't switch within 3s timeout, escalate: force preset via Global.json `LastUsedPreset` field write + CL restart
- Only restart CL if auto-detect failed — avoid unnecessary restarts
- If CL is not running, use `ensure_conspit_link_running()` from existing watchdog before attempting preset load

**Safe Fallback for Unrecognized Games**
- "Unrecognized" = SimType variant not in the SimType→key lookup table (e.g., Forza, ForzaHorizon5)
- Safe default: 50% power cap via HID `axis.power` command + centered spring via `axis.idlespring` — reuses Phase 57 HID commands
- Log `tracing::warn` with game name — visible in pod logs, no WhatsApp (not critical)
- Restore normal power cap (80%) on session end — `safe_session_end()` already handles this

### Claude's Discretion
- Exact 3s timeout implementation (tokio::time::timeout vs polling loop)
- Whether to check ConspitLink's current preset before waiting (skip wait if already correct)
- Error handling for HID command failures during fallback
- Test structure and naming

### Deferred Ideas (OUT OF SCOPE)
- Custom venue-tuned .Base presets per game — Phase 61 (FFB Preset Tuning)
- Fleet-wide config push — Phase 62 (superseded by v22.0)
- Reading ConspitLink's current active preset programmatically — no API exists, would need log parsing
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PROF-03 | rc-agent loads the correct game preset before launching the game process (not relying solely on ConspitLink auto-detect) | Covered by pre-launch hook in LaunchGame handler + `pre_load_game_preset()` function with 3s wait + escalation path |
| PROF-05 | If an unrecognized game launches, a safe default preset is applied (conservative force, centered spring) | Covered by fallback branch: 50% `axis.power` + `axis.idlespring` when SimType not in lookup table |
</phase_requirements>

---

## Summary

Phase 60 adds a pre-launch FFB safety net inside `ws_handler.rs`'s `LaunchGame` handler. The inserted hook calls `pre_load_game_preset(sim_type)` in `ffb_controller.rs` AFTER safe mode entry but BEFORE the game process is spawned. This ensures the wheelbase has the correct FFB profile from the first customer input, rather than relying on ConspitLink's auto-detect (which is inherently racy — it polls on an interval while the game's DirectInput init happens immediately on launch).

The design follows a "trust but verify" strategy: Phase 59's AresAutoChangeConfig is the happy path. Phase 60 is the explicit guarantee. For recognized games (AC, F1 25, AC EVO, AC Rally), the hook waits up to 3 seconds for CL auto-detect to switch, then escalates to a forced `LastUsedPreset` write + CL restart only if needed. For unrecognized games (Forza, ForzaHorizon5), it immediately applies a conservative 50% power cap and idlespring centering — prioritizing customer safety over FFB immersion for untested games.

The entire hook runs in a `tokio::task::spawn_blocking` call (the established pattern for synchronous HID and filesystem operations in this async handler) and must complete within 2-3 seconds before game spawn proceeds. No new crates or external dependencies are needed — all reusable assets exist in Phase 57-59 code.

**Primary recommendation:** Implement `pre_load_game_preset()` as a synchronous function in `ffb_controller.rs` with the `_impl(Option<&Path>)` testable pattern. Call it via `spawn_blocking` in the `LaunchGame` handler between safe mode entry and game process spawn. Keep the 3s timeout as `tokio::time::timeout` wrapping a polling loop that checks `is_process_running("ConspitLink2.0.exe")` plus a 100ms sleep interval.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio` | existing in Cargo.toml | async runtime, `time::timeout`, `task::spawn_blocking` | Already the project's async runtime — `spawn_blocking` is the established pattern for sync HID/filesystem ops in async handlers |
| `serde_json` | existing | Read/write `Global.json` for `LastUsedPreset` escalation path | Already used throughout ffb_controller.rs for JSON config manipulation |
| `tracing` | existing | `warn!` logging for unrecognized game fallback | Already the project's logging framework |
| `hidapi` | existing | `set_gain()` and `set_idle_spring()` for fallback HID commands | Already the HID interface in `FfbController` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `std::thread::sleep` | std | 100ms polling interval inside `spawn_blocking` | Inside the sync context of spawn_blocking — cannot use `tokio::time::sleep` here |
| `std::path::Path` | std | Testable `_impl(Option<&Path>)` pattern for Global.json path injection | All filesystem-dependent functions follow this pattern per Phase 58 convention |

**No new dependencies required.** All needed crates already exist in the project's Cargo.toml.

---

## Architecture Patterns

### Recommended Project Structure

No new files are required. All changes land in existing files:

```
crates/rc-agent/src/
├── ws_handler.rs       # Insert pre_load_game_preset() call in LaunchGame handler (lines 283-312)
├── ffb_controller.rs   # New pub fn pre_load_game_preset() + helper + tests
└── (no new modules)
```

### Pattern 1: Pre-Launch Hook Insertion Point

**What:** Insert a `spawn_blocking` call in the `LaunchGame` handler between safe mode entry and game process spawn.

**When to use:** Every `LaunchGame` message received from server — no exceptions.

**Exact insertion point** in `ws_handler.rs`: After the safe mode block (line ~309) and before the `AssettoCorsa` branch (line ~311). The hook is game-agnostic — it handles all `SimType` variants uniformly before branching.

```rust
// Source: crates/rc-agent/src/ws_handler.rs (insertion after line ~309)
// After safe_mode.enter() block, before game-specific spawn branches:

let pre_load_sim = launch_sim; // copy before move into closure
let pre_load_result = tokio::task::spawn_blocking(move || {
    crate::ffb_controller::pre_load_game_preset(pre_load_sim, None)
}).await;

match pre_load_result {
    Ok(Err(e)) => tracing::warn!(target: LOG_TARGET, "pre_load_game_preset failed: {} -- continuing with game launch", e),
    Err(e) => tracing::warn!(target: LOG_TARGET, "pre_load_game_preset panicked: {} -- continuing", e),
    Ok(Ok(())) => {}
}
// game spawn proceeds regardless — pre-load failure is non-fatal
```

**Key design rule:** Pre-load failure MUST NOT block game launch. A failed preset load is far less bad than a failed game launch. Log the error, continue.

### Pattern 2: `pre_load_game_preset` Function Structure

**What:** New public function in `ffb_controller.rs` following the established `_impl(Option<&Path>)` testable pattern.

```rust
// Source: ffb_controller.rs established patterns (Phase 58)
pub fn pre_load_game_preset(sim_type: SimType, runtime_dir: Option<&std::path::Path>) -> Result<(), String> {
    pre_load_game_preset_impl(sim_type, runtime_dir)
}

fn pre_load_game_preset_impl(sim_type: SimType, runtime_dir: Option<&std::path::Path>) -> Result<(), String> {
    let game_key = sim_type_to_game_key(sim_type);

    match game_key {
        Some(key) => {
            // Recognized game: wait up to 3s for CL auto-detect, then escalate
            wait_for_cl_or_force_preset(key, runtime_dir)
        }
        None => {
            // Unrecognized game: apply safe fallback immediately
            apply_unrecognized_game_fallback(sim_type)
        }
    }
}
```

### Pattern 3: SimType-to-Game-Key Lookup Table

**What:** Mapping from `SimType` enum variants to ConspitLink game key strings. Reuses the `VENUE_GAME_KEYS` constant naming convention from Phase 59.

**Known mappings (all 4 confirmed from Pod 8 hardware inspection, 2026-03-24):**
```rust
// Source: crates/rc-agent/src/ffb_controller.rs VENUE_GAME_KEYS comment
fn sim_type_to_game_key(sim_type: SimType) -> Option<&'static str> {
    match sim_type {
        SimType::AssettoCorsa         => Some("Assetto Corsa"),
        SimType::AssettoCorsaRally    => Some("Assetto Corsa"),      // uses AC preset (no specific one)
        SimType::F125                 => Some("F1 25"),
        SimType::AssettoCorsaEvo      => Some("ASSETTO_CORSA_EVO"), // uppercase-underscore style confirmed
        SimType::IRacing              => None, // not in VENUE_GAME_KEYS — treat as unrecognized
        SimType::LeMansUltimate       => None, // not in VENUE_GAME_KEYS — treat as unrecognized
        SimType::Forza                => None, // unrecognized — safe fallback
        SimType::ForzaHorizon5        => None, // unrecognized — safe fallback
    }
}
```

**Important:** AssettoCorsaRally is not explicitly listed in `VENUE_GAME_KEYS` — the planner must decide whether it falls under "Assetto Corsa" key or is treated as unrecognized. The CONTEXT.md's locked decisions show the lookup table maps `SimType` → ConspitLink key and that Forza/ForzaHorizon5 are examples of unrecognized. AssettoCorsaRally is a judgment call for the planner. The conservative default is to treat it as unrecognized (safe fallback) unless VENUE_GAME_KEYS is expanded.

### Pattern 4: 3-Second Wait Strategy

**What:** Poll for CL preset switch with 100ms sleep intervals, up to 30 iterations (3 seconds total), then escalate.

**When to use:** For recognized games only — when CL auto-detect is expected to fire.

```rust
// Source: established pattern from ffb_controller.rs ensure_auto_switch_config polling
fn wait_for_cl_or_force_preset(
    game_key: &str,
    runtime_dir: Option<&std::path::Path>,
) -> Result<(), String> {
    // Step 1: Ensure CL is running before waiting
    if !crate::ac_launcher::is_process_running("ConspitLink2.0.exe") {
        crate::ac_launcher::ensure_conspit_link_running();
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Step 2: Wait up to 3s for auto-detect to fire
    for _ in 0..30 {
        // Check: if we had a way to verify CL switched, we'd do it here.
        // Since we can't read CL's active preset (no API, deferred), just wait.
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Step 3: Check if we should escalate (verify auto-detect worked)
    // Since we cannot read CL's active preset programmatically (deferred per CONTEXT.md),
    // check is: did CL just restart? If not, force-write LastUsedPreset + restart.
    // Claude's discretion: the planner may choose "always escalate after 3s" to guarantee
    // correctness, since we can't verify CL switched.

    Ok(())
}
```

**Critical limitation:** There is no API to read ConspitLink's currently loaded preset (noted in CONTEXT.md deferred section). This means the 3s wait cannot be a "verified success" — it's a time budget. The planner should decide whether to:
- Option A: Always escalate after 3s (guaranteed correctness, adds ~4-8s to game launch in all cases)
- Option B: Only escalate if CL was NOT running at hook entry (faster, relies on Phase 59 auto-detect for running-CL case)

Recommendation: Option B for running-CL, Option A for non-running-CL (which must restart anyway).

### Pattern 5: Escalation via Global.json LastUsedPreset

**What:** When CL auto-detect cannot be confirmed to have fired, force the correct preset by writing the `LastUsedPreset` device entry in `Global.json` and restarting CL.

**How `LastUsedPreset` works:** The `Global.json` file has per-device preset tracking. ConspitLink reads this on startup to restore the last-used preset. By writing the correct game key here before restarting CL, we ensure it starts with the right preset loaded.

```json
// Global.json structure (from ARCHITECTURE.md)
{
  "AresAutoChangeConfig": "open",
  "LastUsedPreset": "Assetto Corsa",   // <-- this field controls startup preset
  ...
}
```

**Implementation:** Use `serde_json` parse-modify-write (the established pattern from `ensure_auto_switch_config_impl`). Write to the runtime path `C:\RacingPoint\Global.json` (not install dir — the critical runtime read path confirmed by CL log analysis).

```rust
// Source: ffb_controller.rs ensure_auto_switch_config_impl pattern
fn force_preset_via_global_json(
    game_key: &str,
    runtime_dir: Option<&std::path::Path>,
) -> Result<(), String> {
    let runtime_path = runtime_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(RUNTIME_DIR));
    let global_json_path = runtime_path.join("Global.json");

    let content = std::fs::read_to_string(&global_json_path)
        .map_err(|e| format!("Failed to read Global.json: {}", e))?;
    let mut json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse Global.json: {}", e))?;

    if let Some(obj) = json.as_object_mut() {
        obj.insert("LastUsedPreset".to_string(), serde_json::Value::String(game_key.to_string()));
    }

    let updated = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("Failed to serialize Global.json: {}", e))?;
    std::fs::write(&global_json_path, updated)
        .map_err(|e| format!("Failed to write Global.json: {}", e))?;

    // Restart CL for preset to take effect
    restart_conspit_link_hardened(false);
    Ok(())
}
```

### Pattern 6: Unrecognized Game Fallback

**What:** Apply 50% power cap + low idlespring immediately (no waiting), log warn, continue.

```rust
// Source: crates/rc-agent/src/ffb_controller.rs set_gain() and set_idle_spring() (Phase 57)
fn apply_unrecognized_game_fallback(sim_type: SimType) -> Result<(), String> {
    tracing::warn!(
        target: LOG_TARGET,
        "Unrecognized game {:?} — applying safe fallback: 50% power cap + idlespring centering",
        sim_type
    );

    let ffb = FfbController::new(VID, PID); // VID=0x1209, PID=0xFFB0
    // 50% power cap: value = (50 * 65535) / 100 = 32767
    let _ = ffb.set_gain(50); // non-fatal if device not found
    // Low centering spring (conservative — not full ramp like session end)
    let _ = ffb.set_idle_spring(1000); // ~500 raw units = gentle centering
    Ok(())
}
```

**Fallback recovery:** `safe_session_end()` already restores normal power cap (80%) and proper idlespring ramp on session end. No new restore logic needed.

### Anti-Patterns to Avoid

- **Do NOT block game launch on preset load failure.** The hook is a safety enhancement, not a gate. If `pre_load_game_preset` fails for any reason, log and continue. A crashed game launch is worse than a slightly wrong FFB profile.
- **Do NOT write Global.json without checking if CL is running.** If CL is mid-read of the file, a write creates a race. Always stop CL first (or check SESSION_END_IN_PROGRESS sentinel) before writing. The existing `restart_conspit_link_hardened()` handles the full stop-write-restart sequence correctly.
- **Do NOT call `restart_conspit_link_hardened()` for recognized games when CL is already running and auto-detect likely fired.** This adds 4-8 seconds to every game launch unnecessarily. Only restart CL when the escalation path is triggered (auto-detect unconfirmable + CL wasn't running at hook entry).
- **Do NOT use `tokio::time::sleep` inside `spawn_blocking`.** The polling loop runs in a blocking thread — use `std::thread::sleep`. Using `tokio::time::sleep` in a blocking context can cause runtime issues.
- **Do NOT fight ConspitLink for HID writes** during the 3s wait window. The wait is passive — let CL do its work. Only use HID commands for the unrecognized-game fallback path, which does not involve CL preset loading.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Polling CL process state | Custom process polling loop | `crate::ac_launcher::is_process_running("ConspitLink2.0.exe")` | Already implemented, tested, and used in watchdog |
| Starting CL if not running | Direct process spawn | `crate::ac_launcher::ensure_conspit_link_running()` | Handles install path check, delegates to hardened restart, respects SESSION_END_IN_PROGRESS |
| Restarting CL | Direct WM_CLOSE + spawn | `crate::ffb_controller::restart_conspit_link_hardened(false)` | Phase 58 hardened restart — config backup/verify, minimize, graceful close sequence |
| HID power cap | Custom HID report | `FfbController::set_gain(percent)` | Already implemented with CLASS_AXIS encoding |
| HID centering spring | Custom HID report | `FfbController::set_idle_spring(value)` | Already implemented with CLASS_AXIS + CMD_IDLESPRING encoding |
| JSON config modification | String manipulation | `serde_json` parse + modify + write (existing pattern) | Existing pattern in ensure_auto_switch_config_impl — safe, tested, handles nested structure |
| Async timeout | Manual timer loop | `tokio::time::timeout(Duration::from_secs(3), async { ... })` | Built-in tokio primitive — but NOTE: the inner work must also be async for this to work. In spawn_blocking context, use poll loop with std::thread::sleep instead |

---

## Common Pitfalls

### Pitfall 1: Race Between Pre-Load and CL Auto-Detect
**What goes wrong:** rc-agent calls `force_preset_via_global_json` + CL restart while CL auto-detect fires simultaneously (triggered by game process being detectable before full launch). Two writes to the wheelbase from different sources, unknown winner.
**Why it happens:** The 3s wait ends and escalation starts at the same moment CL detects the game.
**How to avoid:** Only escalate when CL was NOT running at hook entry (CL cannot auto-detect if not running). If CL was running throughout the 3s wait, trust auto-detect and skip escalation. This is the safest strategy given the no-read-current-preset limitation.
**Warning signs:** Game launches with correct preset but then briefly resets (visible wheel jerk) = both paths fired.

### Pitfall 2: spawn_blocking Adds to Total Launch Latency
**What goes wrong:** The 3s wait + potential CL restart (~4-8s) adds visibly to game launch time. Customer sees the lock screen for 7-11 seconds instead of 2-3 seconds.
**Why it happens:** The blocking call must complete before game spawn proceeds.
**How to avoid:** Structure the wait to be "3s or less" — if CL is already running and auto-detect is reliable (Phase 59 verified on Pod 8), skip the wait for known-good pods. The happy path should be: CL running + recognized game = minimal delay (check CL running, do 100ms grace period, proceed).
**Warning signs:** User feedback that game launches "feel slow."

### Pitfall 3: SESSION_END_IN_PROGRESS Race
**What goes wrong:** `pre_load_game_preset` triggers a CL restart in the escalation path while `safe_session_end()` is still running its own CL lifecycle management.
**Why it happens:** A new game is launched very quickly after a session ends (staff re-launching before session-end sequence completes).
**How to avoid:** Check `SESSION_END_IN_PROGRESS` before writing Global.json or calling restart. If set, wait for it to clear (add a short retry loop). The existing `ensure_conspit_link_running()` already checks this sentinel — use the same guard.
**Warning signs:** CL restart in escalation path followed immediately by another restart from session-end = double restart, 8-16s dead time.

### Pitfall 4: Global.json Write Without Running CL
**What goes wrong:** `LastUsedPreset` field is written to `C:\RacingPoint\Global.json` but CL is not restarted, so it never reads the updated file.
**Why it happens:** Code writes the field but skips the restart (trying to avoid unnecessary restarts).
**How to avoid:** `LastUsedPreset` is ONLY read on CL startup. Writing it without a restart has zero effect. The escalation path MUST include `restart_conspit_link_hardened(false)` after the write. This is not optional.
**Warning signs:** Written Global.json shows correct LastUsedPreset but wheelbase has wrong FFB profile.

### Pitfall 5: Fallback idlespring Value Too High
**What goes wrong:** `set_idle_spring(value)` for the unrecognized-game fallback uses a value that snaps the wheel sharply to center. Customer's hands are on the wheel, wrist injury risk.
**Why it happens:** Using the same target idlespring value (2000) as session-end without the ramp-up sequence.
**How to avoid:** Use a LOW idlespring value for pre-launch fallback (e.g., 500-1000, gentle centering). The session-end sequence ramps from 0 to 2000 over 500ms precisely to avoid snap-back. Pre-launch fallback is setting a baseline, not centering from stuck position.
**Warning signs:** Audible "thunk" from wheelbase when unrecognized game pre-load fires.

---

## Code Examples

### LaunchGame Handler Insertion (ws_handler.rs)

```rust
// Source: crates/rc-agent/src/ws_handler.rs, after line ~309 (safe mode entry block)
// Insert BEFORE the `if launch_sim == SimType::AssettoCorsa` branch:

{
    let pre_load_sim = launch_sim;
    let pre_load_result = tokio::task::spawn_blocking(move || {
        crate::ffb_controller::pre_load_game_preset(pre_load_sim, None)
    }).await;
    match pre_load_result {
        Ok(Ok(())) => tracing::info!(target: LOG_TARGET, "pre_load_game_preset: ok"),
        Ok(Err(e)) => tracing::warn!(target: LOG_TARGET, "pre_load_game_preset failed (non-fatal): {}", e),
        Err(e) => tracing::warn!(target: LOG_TARGET, "pre_load_game_preset panicked (non-fatal): {}", e),
    }
}
// Game spawn proceeds regardless
```

### Testable Function Signature (ffb_controller.rs)

```rust
// Source: Phase 58 testable _impl pattern (established in ffb_controller.rs)
/// Pre-load the correct FFB preset for the given game BEFORE game spawn.
/// Called from ws_handler.rs LaunchGame handler via spawn_blocking.
/// Non-fatal — errors are returned, not panicked.
///
/// Production path: runtime_dir = None (uses C:\RacingPoint\)
/// Test path: runtime_dir = Some(&tempdir) (injects test filesystem)
pub fn pre_load_game_preset(
    sim_type: SimType,
    runtime_dir: Option<&std::path::Path>,
) -> Result<(), String> {
    pre_load_game_preset_impl(sim_type, runtime_dir)
}
```

### Unit Test Pattern

```rust
// Source: ffb_controller.rs mod tests pattern (existing at line 991)
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_pre_load_recognized_game_writes_last_used_preset() {
        let dir = std::env::temp_dir().join("pre_load_test_recognized");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Seed Global.json with a different LastUsedPreset
        let initial = serde_json::json!({
            "AresAutoChangeConfig": "open",
            "LastUsedPreset": "F1 25"
        });
        fs::write(dir.join("Global.json"),
            serde_json::to_string_pretty(&initial).unwrap()).unwrap();

        // Run pre-load for AC — should update LastUsedPreset to "Assetto Corsa"
        // (No CL restart in test — production path is guarded by install_dir.is_none())
        let result = pre_load_game_preset(SimType::AssettoCorsa, Some(&dir));
        // Result may be Ok or Err depending on whether test calls restart —
        // test verifies file write behavior, not restart behavior
        let content = fs::read_to_string(dir.join("Global.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["LastUsedPreset"], "Assetto Corsa");
    }

    #[test]
    fn test_pre_load_unrecognized_game_does_not_write_global_json() {
        let dir = std::env::temp_dir().join("pre_load_test_unrecognized");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let initial = serde_json::json!({ "AresAutoChangeConfig": "open" });
        fs::write(dir.join("Global.json"),
            serde_json::to_string_pretty(&initial).unwrap()).unwrap();

        // Forza is unrecognized — should apply HID fallback, not touch Global.json
        let _ = pre_load_game_preset(SimType::Forza, Some(&dir));
        // Global.json should be unchanged
        let content = fs::read_to_string(dir.join("Global.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(json.get("LastUsedPreset").is_none());
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Rely solely on CL auto-detect for preset loading | rc-agent explicitly pre-loads + escalates if auto-detect races | Phase 60 (this phase) | Guaranteed correct FFB from first customer input, not dependent on CL timing |
| No handling for unrecognized games | 50% power cap + idlespring for any unrecognized SimType | Phase 60 (this phase) | Safety baseline for games not in VENUE_GAME_KEYS |
| ConspitLink auto-detect as primary (Phase 59) | CL auto-detect as primary, pre-launch hook as safety net | Phase 59-60 | Two-layer guarantee: auto-detect handles the common path, hook guarantees correctness |

**Still applicable (not superseded):**
- Phase 57 HID commands (`set_gain`, `set_idle_spring`) — directly reused in fallback
- Phase 58 hardened restart — directly reused in escalation path
- Phase 59 VENUE_GAME_KEYS and auto-switch config — the happy path that pre-launch hook complements

---

## Open Questions

1. **Should AssettoCorsaRally use "Assetto Corsa" preset key or be treated as unrecognized?**
   - What we know: `VENUE_GAME_KEYS` has 4 entries — "Assetto Corsa", "F1 25", "Assetto Corsa Competizione", "ASSETTO_CORSA_EVO". AC Rally is not listed.
   - What's unclear: Does ConspitLink's GameToBaseConfig.json have an "Assetto Corsa Rally" entry? If so, what is the key string?
   - Recommendation: Default to treating AC Rally as unrecognized (safe fallback) until the key is confirmed from Pod 8 hardware inspection. Alternatively, map it to "Assetto Corsa" since they share the same physics engine and FFB feel is similar.

2. **How long does `restart_conspit_link_hardened(false)` take in practice?**
   - What we know: The function waits 8 seconds for window minimize (16 attempts x 500ms per `minimize_conspit_window` loop in ffb_controller.rs lines 861-869). Total restart time is roughly 8-12 seconds.
   - What's unclear: This would push total pre-launch time to 3s wait + 8-12s restart = 11-15s for the escalation path. Is this acceptable for game launch UX?
   - Recommendation: Only trigger the escalation path when CL was not running at hook entry. If CL was running, trust auto-detect (Phase 59 verified this works). This keeps the common-path latency to ~3s.

3. **Is `tokio::time::timeout` usable to wrap the spawn_blocking call?**
   - What we know: `tokio::time::timeout` can wrap any future including `spawn_blocking(...).await`.
   - What's unclear: If timeout fires while spawn_blocking is in the middle of a CL restart, the blocking thread continues running even though the future was dropped. This could cause a CL restart to run "in the background" after game launch has already started.
   - Recommendation: Use a simple `std::thread::sleep` loop inside `spawn_blocking` with a counter (30 x 100ms = 3s), rather than wrapping the entire spawn with `tokio::time::timeout`. This gives cleaner semantics — the blocking thread always runs to completion before the handler proceeds.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | `Cargo.toml` (existing) |
| Quick run command | `cargo test -p rc-agent pre_load` |
| Full suite command | `cargo test -p rc-agent && cargo test -p rc-common && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROF-03 | `pre_load_game_preset(AssettoCorsa, Some(&dir))` writes "Assetto Corsa" to `LastUsedPreset` in Global.json | unit | `cargo test -p rc-agent test_pre_load_recognized_game` | ❌ Wave 0 |
| PROF-03 | `pre_load_game_preset(F125, Some(&dir))` writes "F1 25" to `LastUsedPreset` | unit | `cargo test -p rc-agent test_pre_load_f125` | ❌ Wave 0 |
| PROF-03 | `pre_load_game_preset(AssettoCorsaEvo, Some(&dir))` writes "ASSETTO_CORSA_EVO" | unit | `cargo test -p rc-agent test_pre_load_ac_evo` | ❌ Wave 0 |
| PROF-05 | `pre_load_game_preset(Forza, Some(&dir))` does NOT modify Global.json | unit | `cargo test -p rc-agent test_pre_load_unrecognized_no_global_write` | ❌ Wave 0 |
| PROF-05 | `sim_type_to_game_key(Forza)` returns `None`, `sim_type_to_game_key(ForzaHorizon5)` returns `None` | unit | `cargo test -p rc-agent test_sim_type_to_game_key_unrecognized` | ❌ Wave 0 |
| PROF-05 | `sim_type_to_game_key(AssettoCorsa)` returns `Some("Assetto Corsa")` | unit | `cargo test -p rc-agent test_sim_type_to_game_key_recognized` | ❌ Wave 0 |
| PROF-03+05 | HID fallback for unrecognized game: `apply_unrecognized_game_fallback` runs without panic on no-device machine | unit | `cargo test -p rc-agent test_unrecognized_fallback_no_device` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent pre_load`
- **Per wave merge:** `cargo test -p rc-agent && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] New unit tests in `crates/rc-agent/src/ffb_controller.rs` `mod tests` block — covers PROF-03 and PROF-05
- [ ] No new test files needed — all tests live inline in `ffb_controller.rs` following established pattern

*(All tests are new — no existing test covers pre_load_game_preset behavior since the function does not exist yet.)*

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/ffb_controller.rs` (local) — VENUE_GAME_KEYS, restart_conspit_link_hardened, set_gain, set_idle_spring, SESSION_END_IN_PROGRESS sentinel, existing test patterns
- `crates/rc-agent/src/ws_handler.rs` lines 283-312 (local) — exact insertion point for LaunchGame hook
- `crates/rc-common/src/types.rs` lines 8-20 (local) — SimType enum, all 8 variants
- `crates/rc-agent/src/ac_launcher.rs` lines 1388-1408 (local) — ensure_conspit_link_running, is_process_running
- `.planning/phases/60-pre-launch-profile-loading/60-CONTEXT.md` (local) — locked decisions

### Secondary (MEDIUM confidence)
- `.planning/research/conspit-link/ARCHITECTURE.md` — Global.json LastUsedPreset field structure, CL runtime read path (`C:\RacingPoint\`), anti-patterns
- `.planning/research/conspit-link/PITFALLS.md` — Pitfall 5 (auto game detection race condition), Pitfall 2 (P-20 HID contention), Pitfall 3 (snap-back torque)
- `.planning/research/conspit-link/STACK.md` — HID command table, idlespring (Axis 0xA01 cmd 0x05), power (Axis 0xA01 cmd 0x00)
- `.planning/phases/57-session-end-safety/57-CONTEXT.md` — HID commands, power cap, session-end recovery sequence
- `.planning/phases/59-auto-switch-configuration/59-CONTEXT.md` — Phase 59 prerequisite details

### Tertiary (LOW confidence)
- None — all key claims are verified against project source code and local research files.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries exist in current Cargo.toml, no new dependencies
- Architecture: HIGH — insertion point precisely identified, function signatures follow established patterns
- Pitfalls: HIGH — based on live code review (SESSION_END_IN_PROGRESS, CL restart timing, HID ramp-up rules)
- Test map: HIGH — test framework is `cargo test`, all test functions are new (no existing coverage to verify)

**Research date:** 2026-03-24 IST
**Valid until:** 2026-04-24 (stable domain — ConspitLink version locked at 1.1.2, OpenFFBoard HID commands stable)
