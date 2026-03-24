# Phase 59: Auto-Switch Configuration - Research

**Researched:** 2026-03-24
**Domain:** ConspitLink config file placement + rc-agent startup self-healing (Rust)
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- rc-agent ensures `Global.json` exists at `C:\RacingPoint\Global.json` at startup (self-healing)
- Copy from install dir (`C:\Program Files (x86)\Conspit Link 2.0\Global.json`) to `C:\RacingPoint\Global.json`
- Ensure `AresAutoChangeConfig` is set to `"open"` in the placed file
- If `C:\RacingPoint\` directory doesn't exist, create it
- This runs as part of rc-agent startup, before ConspitLink watchdog kicks in
- Use ConspitLink's shipped default .Base presets for all 4 games (Phase 61 handles tuning)
- Verify existing GameToBaseConfig.json has entries for all 4 venue games (AC, F1 25, ACC/AC EVO, AC Rally)
- If mappings are missing or point to non-existent files, fix them
- After writing/updating config files, restart ConspitLink using `restart_conspit_link_hardened(false)` (not crash recovery)
- ConspitLink caches config at startup — file writes without restart are ineffective (ARCHITECTURE.md Anti-Pattern 4)
- Only restart if config actually changed (compare content before/after)
- Manual game launch test on canary pod (Pod 8) — Claude builds and deploys, human tests on hardware

### Claude's Discretion
- Exact startup timing (when in rc-agent init sequence to place config)
- Whether to use file copy or atomic write (temp + rename) for Global.json placement
- JSON manipulation approach (serde_json parse + modify + write vs string replace)
- Error handling for edge cases (locked files, permission denied)

### Deferred Ideas (OUT OF SCOPE)
- Custom venue-tuned .Base presets — Phase 61 (FFB Preset Tuning)
- rc-agent pre-loading presets before game launch — Phase 60 (Pre-Launch Profile Loading)
- Fleet-wide config push from racecontrol — Phase 62 (superseded by v22.0)
- Config hash reporting in heartbeats — Phase 63 (Fleet Monitoring)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PROF-01 | Global.json exists at `C:\RacingPoint\Global.json` on every pod (runtime read path ConspitLink actually uses) | Root cause verified from CL log: CL reads this path, not install dir. rc-agent startup self-heal pattern (self_heal.rs) is the correct location for this fix. |
| PROF-02 | GameToBaseConfig.json mappings point to correct presets for all 4 active games | File structure verified from STACK.md. Game keys: ASSETTO_CORSA, F1_25, and ACC/AC EVO keys. Verify-and-fix function pattern in ffb_controller.rs is reusable. |
| PROF-04 | Launching AC, F1 25, ACC/AC EVO, or AC Rally causes ConspitLink to auto-load the matching preset | Depends on PROF-01 (correct Global.json path) + PROF-02 (correct mappings) + ConspitLink restart after any changes. All prerequisite APIs exist. |
</phase_requirements>

---

## Summary

Phase 59 is a targeted config placement fix, not a major code feature. The root cause is well-understood: ConspitLink reads `Global.json` from `C:\RacingPoint\Global.json` at runtime (confirmed from CL log), but that path does not exist on pods — only the install-dir copy exists. This breaks `AresAutoChangeConfig` silently: the setting is there, CL just never reads it.

The fix has three parts: (1) rc-agent places `Global.json` at `C:\RacingPoint\` on startup and ensures `AresAutoChangeConfig` is `"open"`, (2) rc-agent verifies `GameToBaseConfig.json` has correct entries for all 4 venue games pointing to existing `.Base` files, and (3) if any config changed, rc-agent restarts ConspitLink so it picks up the new config. All the infrastructure to do this already exists in Phase 58's `restart_conspit_link_hardened()`, `backup_conspit_configs()`, and the `_impl(Option<&Path>)` testable pattern.

The primary work is writing a new `ensure_auto_switch_config()` function in `ffb_controller.rs` (or a new `conspit_config.rs` module) that runs early in the startup sequence, before `enforce_safe_state()` calls `ensure_conspit_link_running()`.

**Primary recommendation:** Add `ensure_auto_switch_config()` to the startup sequence in `main.rs` between self-heal and `enforce_safe_state()`. Use atomic write (temp + rename) for Global.json. Use `serde_json` to parse+modify+write (not string replace) to avoid corrupting the JSON structure.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde_json` | workspace | JSON parse, modify, write for Global.json + GameToBaseConfig.json | Already in rc-agent deps. Only correct way to modify JSON without corruption risk. |
| `std::fs` | stdlib | File copy, directory creation, atomic rename | No new crate needed for file operations on Windows. |
| `winapi` | 0.3 (workspace) | Windows-specific path operations (already used throughout rc-agent) | Already in deps. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tracing` | workspace | Structured logging for config placement events | Every branch of the config-ensure path should log at appropriate level |
| `std::io::Write` | stdlib | Write temp file before atomic rename | Used in self_heal.rs and other places already |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde_json parse+modify | String replace / regex | String replace can corrupt JSON if value contains quotes or special chars. serde_json is safe. |
| Atomic write (temp + rename) | Direct `fs::write()` | Direct write creates a window where CL reads a partial file. Atomic is safer. NTFS rename is atomic. |
| New `conspit_config.rs` module | Add to `ffb_controller.rs` | ffb_controller.rs already has all config-adjacent code (CONSPIT_CONFIG_FILES, RUNTIME_GLOBAL_JSON, backup/verify). Keeping it there avoids cross-module coupling. |

**Installation:** No new crates needed — all dependencies already in workspace.

---

## Architecture Patterns

### Recommended Project Structure

The new function lives in `ffb_controller.rs` alongside existing config functions:

```
crates/rc-agent/src/
├── ffb_controller.rs    # ADD: ensure_auto_switch_config() + ensure_auto_switch_config_impl()
│                        # EXISTING: backup_conspit_configs(), verify_conspit_configs(),
│                        #           restart_conspit_link_hardened(), CONSPIT_CONFIG_FILES,
│                        #           RUNTIME_GLOBAL_JSON
├── main.rs              # WIRE: call ensure_auto_switch_config() in startup sequence
└── self_heal.rs         # Reference: existing startup self-heal pattern to follow
```

### Pattern 1: Startup Self-Heal (established in self_heal.rs)

**What:** Run at startup, check if something is wrong, fix it, log what was repaired, continue regardless of failure.

**When to use:** Any idempotent repair that should run every startup before dependent systems start.

**Example (from self_heal.rs):**
```rust
// Non-fatal: if repair fails, log a warning and continue. Never panic.
pub fn run(exe_dir: &Path) -> SelfHealResult {
    let mut result = SelfHealResult { ... };
    let config_path = exe_dir.join("rc-agent.toml");
    if !config_path.exists() {
        tracing::warn!(target: LOG_TARGET, "rc-agent.toml missing -- attempting repair");
        match repair_config(&config_path) {
            Ok(()) => { result.config_repaired = true; }
            Err(e) => { result.errors.push(format!("config: {}", e)); }
        }
    }
    result
}
```

Apply the same pattern to `ensure_auto_switch_config()`: non-fatal, returns a struct indicating what was done.

### Pattern 2: Testable _impl() with Optional Base Dir (established in ffb_controller.rs)

**What:** Production function calls `_impl(None)`, test function calls `_impl(Some(test_dir))`. Allows filesystem-dependent code to be unit-tested without touching production paths.

**When to use:** Every filesystem function in rc-agent. Already established in Phase 58.

**Example (from ffb_controller.rs):**
```rust
// Source: crates/rc-agent/src/ffb_controller.rs (Phase 58)
pub fn backup_conspit_configs() {
    backup_conspit_configs_impl(None);
}

fn backup_conspit_configs_impl(base_dir: Option<&std::path::Path>) {
    let config_entries: Vec<(String, &str)> = if let Some(dir) = base_dir {
        // Test mode: relative paths inside test dir
        CONSPIT_CONFIG_FILES.iter()
            .map(|(_path, name)| (dir.join(name).to_string_lossy().into_owned(), *name))
            .collect()
    } else {
        // Production: hardcoded absolute paths
        CONSPIT_CONFIG_FILES.iter()
            .map(|(path, name)| (path.to_string(), *name))
            .collect()
    };
    // ...
}

#[cfg(test)]
pub(crate) fn backup_conspit_configs_in_dir(dir: &std::path::Path) {
    backup_conspit_configs_impl(Some(dir));
}
```

`ensure_auto_switch_config()` MUST follow this pattern. Production call uses hardcoded paths; tests use tmpdir.

### Pattern 3: Compare-Before-Write (to avoid unnecessary CL restarts)

**What:** Read the current file content, compare to what would be written. Only write (and then restart CL) if content actually changed.

**When to use:** Any config file placement that triggers a ConspitLink restart. Unnecessary restarts waste ~8s startup time and create noise in logs.

```rust
// Pseudocode for compare-before-write
fn place_global_json(target: &Path, source: &Path) -> Result<bool, Error> {
    let source_content = fs::read_to_string(source)?;
    let mut json: Value = serde_json::from_str(&source_content)?;
    // Ensure AresAutoChangeConfig is "open"
    json["AresAutoChangeConfig"] = json!("open");
    let new_content = serde_json::to_string_pretty(&json)?;

    // Compare with existing (if any)
    if target.exists() {
        if let Ok(existing) = fs::read_to_string(target) {
            if existing == new_content {
                return Ok(false); // No change needed
            }
        }
    }

    // Atomic write: temp file then rename
    let tmp = target.with_extension("json.tmp");
    fs::write(&tmp, &new_content)?;
    fs::rename(&tmp, target)?;
    Ok(true) // Changed
}
```

### Pattern 4: Startup Sequence Placement

**What:** Where in `main.rs` to call `ensure_auto_switch_config()`.

**Constraint from CONTEXT.md:** "This runs as part of rc-agent startup, before ConspitLink watchdog kicks in."

The startup sequence in `main.rs` is:
1. `detect_crash_recovery()` + `write_phase("init")`
2. `self_heal::run()` — config/script/registry repair
3. `load_config()` — fail-fast if config broken
4. Tracing init
5. `game_process::cleanup_orphaned_games()`
6. `firewall::configure()`
7. `remote_ops::start_checked(8090)`
8. FFB controller init
9. `ffb.zero_force_with_retry(3, 100)` — FFB safety on startup
10. Lock screen start + `show_startup_connecting()`
11. **[INSERT: `ensure_auto_switch_config()` HERE]** — before ConspitLink watchdog
12. `enforce_safe_state()` (delayed, calls `ensure_conspit_link_running()`)
13. WebSocket connect loop

**Correct insertion point:** After step 10 (lock screen) and before step 12 (enforce_safe_state). The ConspitLink watchdog is inside `enforce_safe_state()` → `ensure_conspit_link_running()`, so any config placement must precede that call. The function should NOT block startup — run it in a `spawn_blocking` task same as `enforce_safe_state()`.

### Anti-Patterns to Avoid

- **Direct file write without JSON parse:** Writing Global.json by string manipulation can produce invalid JSON if the existing file has unexpected structure. Always parse → modify → serialize.
- **Conditional CL restart without content comparison:** Always restarting CL on startup is wasteful (8s delay). Compare content hash or string before deciding to restart.
- **Hardcoded game key strings outside constants:** Game keys (`ASSETTO_CORSA`, `F1_25`, etc.) should be constants, not inline string literals spread across logic.
- **Skipping the `_impl()` pattern for new filesystem code:** Tests cannot cover hardcoded production paths on the dev machine. `_impl(Option<&Path>)` is mandatory per Phase 58 pattern.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON modification | String replace / regex substitution | `serde_json::from_str` + field set + `serde_json::to_string_pretty` | JSON can have whitespace, escaping, ordering variations — string replace is fragile |
| Atomic file write | `fs::write()` directly | Write to `.json.tmp` then `fs::rename()` | NTFS rename is atomic; direct write creates partial-file read window for CL |
| ConspitLink restart after config | Custom process kill+start | `restart_conspit_link_hardened(false)` | Already implemented in Phase 58 with minimize retry, backup, verify |
| Config backup before placement | Ad-hoc copy | `backup_conspit_configs()` | Already validates JSON before overwriting .bak — prevents corrupt backup chain |
| Process detection | `tasklist` exec + parse | `ac_launcher::is_process_running("ConspitLink2.0.exe")` | Already implemented, used throughout ac_launcher.rs |

**Key insight:** Almost all required infrastructure exists. This phase is primarily wiring existing functions together with a new `ensure_auto_switch_config()` function.

---

## Common Pitfalls

### Pitfall 1: CL Restarts on Every Startup
**What goes wrong:** `ensure_auto_switch_config()` always detects "change needed" because it doesn't compare existing file content. Every pod startup triggers a 8-second CL close+restart+minimize cycle.
**Why it happens:** Not reading the current target file before writing.
**How to avoid:** Read `C:\RacingPoint\Global.json` before writing. If it already has `AresAutoChangeConfig: "open"` and matches the source content (minus that field), skip write and skip CL restart.
**Warning signs:** Startup log shows "ConspitLink restarted" on every boot even when nothing changed.

### Pitfall 2: Installing-Dir Global.json May Be Missing AresAutoChangeConfig
**What goes wrong:** The install-dir `Global.json` has `AresAutoChangeConfig: "close"` (or the field is missing) so just copying the file doesn't fix the problem.
**Why it happens:** ConspitLink defaults to `"close"` on fresh install. The field needs to be explicitly set to `"open"`.
**How to avoid:** After reading the source `Global.json`, always force `json["AresAutoChangeConfig"] = json!("open")` before writing to target. Do not just copy the file.
**Warning signs:** Auto-switch still doesn't work even after placing the file.

### Pitfall 3: GameToBaseConfig.json Points to Non-Existent .Base Files
**What goes wrong:** `GameToBaseConfig.json` has entries for games but the paths point to `.Base` files that don't exist on the pod (deleted, renamed, or wrong path).
**Why it happens:** CL shipped with some game paths using Chinese directory names (`官方预设\`) that may have been reorganized.
**How to avoid:** After verifying each game key exists in `GameToBaseConfig.json`, also verify the mapped `.Base` file path exists with `Path::new(path).exists()`. If the file doesn't exist, find the actual default preset and update the mapping.
**Warning signs:** CL has the correct game key in GameToBaseConfig but still loads a wrong/default preset.

### Pitfall 4: CL Restart Race with Watchdog
**What goes wrong:** `ensure_auto_switch_config()` decides to restart CL. Meanwhile the ConspitLink watchdog (in `enforce_safe_state()`) fires and tries to restart CL too. Double-restart sequence confuses the startup state.
**Why it happens:** Both paths can run within the same startup window.
**How to avoid:** `ensure_auto_switch_config()` should check `SESSION_END_IN_PROGRESS` before restarting (same guard used by watchdog). Alternatively, placement timing: if `ensure_auto_switch_config()` runs synchronously and completes before `enforce_safe_state()` is spawned, there is no race.
**Warning signs:** CL started, closed, started again within 10s of startup.

### Pitfall 5: C:\RacingPoint\ Directory May Not Exist
**What goes wrong:** `fs::write()` or `fs::rename()` fails with "The system cannot find the path specified" if `C:\RacingPoint\` doesn't exist.
**Why it happens:** Fresh pod installs may not have this directory until rc-agent creates it.
**How to avoid:** Check `fs::create_dir_all(r"C:\RacingPoint")` before any write to that path. Already the install location for `rc-agent.exe`, so it should exist on active pods, but must not assume.
**Warning signs:** Error log shows "cannot find path" when placing Global.json.

### Pitfall 6: AC EVO / AC Rally Game Keys May Differ from Expected
**What goes wrong:** Phase uses AC EVO and AC Rally game keys that don't match what CL ships in GameToBaseConfig.json.
**Why it happens:** CL may use `ASSETTO_CORSA_EVO` or `ASSETTOCORSAEVO` — exact key is only known from the actual file on pod hardware.
**How to avoid:** Research task must inspect the actual `GameToBaseConfig.json` on a pod (via fleet exec or Pod 8 direct access) to confirm exact game keys before writing fix logic.
**Warning signs:** AC EVO and AC Rally don't auto-switch even after the fix.

---

## Code Examples

### Ensure Global.json at Runtime Path

```rust
// ffb_controller.rs — new function following established _impl() pattern
const LOG_TARGET_CFG: &str = "conspit-cfg";

pub struct AutoSwitchConfigResult {
    pub global_json_placed: bool,
    pub global_json_changed: bool,
    pub game_to_base_fixed: bool,
    pub conspit_restarted: bool,
    pub errors: Vec<String>,
}

pub fn ensure_auto_switch_config() -> AutoSwitchConfigResult {
    ensure_auto_switch_config_impl(None, None)
}

fn ensure_auto_switch_config_impl(
    install_dir: Option<&std::path::Path>,
    runtime_dir: Option<&std::path::Path>,
) -> AutoSwitchConfigResult {
    let install_base = install_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Program Files (x86)\Conspit Link 2.0"));
    let runtime_base = runtime_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\RacingPoint"));

    let mut result = AutoSwitchConfigResult {
        global_json_placed: false,
        global_json_changed: false,
        game_to_base_fixed: false,
        conspit_restarted: false,
        errors: Vec::new(),
    };

    // Ensure C:\RacingPoint\ exists
    if let Err(e) = std::fs::create_dir_all(&runtime_base) {
        result.errors.push(format!("create_dir_all failed: {}", e));
        return result;
    }

    // 1. Place Global.json with AresAutoChangeConfig forced to "open"
    let source = install_base.join("Global.json");
    let target = runtime_base.join("Global.json");
    match place_global_json(&source, &target) {
        Ok(changed) => {
            result.global_json_placed = true;
            result.global_json_changed = changed;
            if changed {
                tracing::info!(target: LOG_TARGET_CFG, "Global.json placed at runtime path (changed=true)");
            } else {
                tracing::debug!(target: LOG_TARGET_CFG, "Global.json already correct at runtime path");
            }
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET_CFG, "Failed to place Global.json: {}", e);
            result.errors.push(format!("Global.json: {}", e));
        }
    }

    // 2. Verify GameToBaseConfig.json (only if source exists)
    let gtb_path = install_base.join("JsonConfigure").join("GameToBaseConfig.json");
    if gtb_path.exists() {
        match verify_game_to_base_config(&gtb_path, &install_base) {
            Ok(fixed) => {
                result.game_to_base_fixed = fixed;
                if fixed {
                    tracing::info!(target: LOG_TARGET_CFG, "GameToBaseConfig.json mappings fixed");
                }
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET_CFG, "GameToBaseConfig.json verify failed: {}", e);
                result.errors.push(format!("GameToBaseConfig.json: {}", e));
            }
        }
    }

    // 3. Restart CL only if something changed
    if result.global_json_changed || result.game_to_base_fixed {
        tracing::info!(target: LOG_TARGET_CFG, "Config changed — restarting ConspitLink to pick up new config");
        restart_conspit_link_hardened(false);
        result.conspit_restarted = true;
    }

    result
}
```

### Place Global.json (Atomic, AresAutoChangeConfig Forced)

```rust
fn place_global_json(source: &std::path::Path, target: &std::path::Path) -> Result<bool, String> {
    if !source.exists() {
        return Err(format!("Source Global.json not found: {}", source.display()));
    }

    // Parse source JSON
    let content = std::fs::read_to_string(source)
        .map_err(|e| format!("read source: {}", e))?;
    let mut json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("parse source JSON: {}", e))?;

    // Force AresAutoChangeConfig to "open"
    json["AresAutoChangeConfig"] = serde_json::json!("open");

    let new_content = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("serialize: {}", e))?;

    // Compare with existing target (skip write if identical)
    if target.exists() {
        if let Ok(existing) = std::fs::read_to_string(target) {
            if existing == new_content {
                return Ok(false); // No change needed
            }
        }
    }

    // Atomic write: .json.tmp then rename
    let tmp = target.with_extension("json.tmp");
    std::fs::write(&tmp, &new_content)
        .map_err(|e| format!("write tmp: {}", e))?;
    std::fs::rename(&tmp, target)
        .map_err(|e| format!("rename tmp->target: {}", e))?;

    Ok(true) // Changed
}
```

### Wire into main.rs Startup Sequence

```rust
// main.rs — after lock screen start, before delayed enforce_safe_state
// Source: crates/rc-agent/src/main.rs startup sequence (line ~556 region)

// AUTO-SWITCH-01: Ensure ConspitLink auto game detection config is in place
// Runs before ConspitLink watchdog so CL starts with correct config.
tokio::task::spawn_blocking(|| {
    let result = ffb_controller::ensure_auto_switch_config();
    if !result.errors.is_empty() {
        tracing::warn!(
            target: LOG_TARGET,
            "Auto-switch config errors: {:?}",
            result.errors
        );
    }
    tracing::info!(
        target: LOG_TARGET,
        "Auto-switch config: placed={} changed={} game_to_base_fixed={} restarted={}",
        result.global_json_placed,
        result.global_json_changed,
        result.game_to_base_fixed,
        result.conspit_restarted
    );
});
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual Global.json placement (staff task) | rc-agent self-heals on every startup | Phase 59 | No staff action needed; survives pod reimages and installs |
| Assumed CL reads from install dir | Write to BOTH install dir AND `C:\RacingPoint\` | Discovered in ARCHITECTURE.md research | AresAutoChangeConfig actually works |
| Generic `restart_conspit_link()` | Hardened `restart_conspit_link_hardened(bool)` with backup/verify/crash-count | Phase 58 | Safe to call from any context |

**Deprecated/outdated:**
- Any code that only writes to install dir for `Global.json` — must write to BOTH locations (Anti-Pattern 3 from ARCHITECTURE.md)

---

## Open Questions

1. **Exact game keys in GameToBaseConfig.json for AC EVO and AC Rally**
   - What we know: Game keys exist for `ASSETTO_CORSA` and `F1_25` (confirmed in STACK.md file tree). AC EVO and AC Rally keys are not confirmed.
   - What's unclear: Are they `ASSETTO_CORSA_EVO`, `AC_EVO`, `ASSETTO_CORSA_EVOLUTION`? Is AC Rally `AC_RALLY` or `ASSETTO_CORSA_RALLY`?
   - Recommendation: Plan 01 should include a task to inspect the actual `GameToBaseConfig.json` on Pod 8 via fleet exec (`cat "C:\Program Files (x86)\Conspit Link 2.0\JsonConfigure\GameToBaseConfig.json"`) before writing the fix logic. The exact keys must be confirmed from hardware.

2. **Whether Global.json exists in install dir after fresh CL install**
   - What we know: `C:\Program Files (x86)\Conspit Link 2.0\Global.json` was found on pods (STACK.md confirms). CL writes this on first run.
   - What's unclear: If CL was never launched after reinstall, the file may not exist yet.
   - Recommendation: Handle the case where source `Global.json` doesn't exist — either skip placement (CL hasn't run yet) or create a minimal Global.json with only `AresAutoChangeConfig: "open"`. Skipping is safer.

3. **Default .Base paths currently in GameToBaseConfig.json**
   - What we know: CL ships with defaults in `官方预设\` (Chinese directory names) for each game.
   - What's unclear: Do the current mappings point to existing files on pods or are they stale?
   - Recommendation: Phase 01 inspection task should also dump the current mappings and verify each `.Base` path exists. Plan the fix based on actual pod state, not assumed state.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`#[test]`) + `cargo test` |
| Config file | `Cargo.toml` (test in same crate, no separate config) |
| Quick run command | `cargo test -p rc-agent ffb_controller` |
| Full suite command | `cargo test -p rc-agent && cargo test -p rc-common && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROF-01 | `ensure_auto_switch_config()` places Global.json at runtime path | unit | `cargo test -p rc-agent test_ensure_auto_switch_global_json` | ❌ Wave 0 |
| PROF-01 | `ensure_auto_switch_config()` forces `AresAutoChangeConfig: "open"` even if source has `"close"` | unit | `cargo test -p rc-agent test_auto_switch_forces_open` | ❌ Wave 0 |
| PROF-01 | No-op when Global.json already correct (no CL restart) | unit | `cargo test -p rc-agent test_auto_switch_no_change_no_restart` | ❌ Wave 0 |
| PROF-01 | Creates `C:\RacingPoint\` directory if missing | unit | `cargo test -p rc-agent test_auto_switch_creates_dir` | ❌ Wave 0 |
| PROF-02 | `verify_game_to_base_config()` detects missing game key | unit | `cargo test -p rc-agent test_game_to_base_missing_key` | ❌ Wave 0 |
| PROF-02 | `verify_game_to_base_config()` detects mapping to non-existent .Base file | unit | `cargo test -p rc-agent test_game_to_base_missing_file` | ❌ Wave 0 |
| PROF-04 | CL restarted after config change | unit | `cargo test -p rc-agent test_auto_switch_restarts_on_change` | ❌ Wave 0 |
| PROF-04 | Manual: launch each of 4 venue games on Pod 8, verify preset loads | manual-only | N/A — human verifies ConspitLink preset change on hardware | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent ffb_controller`
- **Per wave merge:** `cargo test -p rc-agent && cargo test -p rc-common`
- **Phase gate:** Full suite green + Pod 8 manual verification before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Unit tests for `ensure_auto_switch_config_impl()` in `ffb_controller.rs` — covers PROF-01, PROF-02, PROF-04
- [ ] Tests should use `tempdir` crate or `std::env::temp_dir()` as base_dir override (same pattern as `backup_conspit_configs_in_dir`)
- [ ] No framework install needed — `cargo test` already works

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/ffb_controller.rs` — CONSPIT_CONFIG_FILES (line 23), RUNTIME_GLOBAL_JSON (line 30), backup_conspit_configs_impl() pattern, restart_conspit_link_hardened(), _impl(Option<&Path>) testable pattern
- `crates/rc-agent/src/main.rs` — startup sequence, where enforce_safe_state() is called (line 563), spawn_blocking pattern for startup tasks
- `crates/rc-agent/src/self_heal.rs` — SelfHealResult pattern, non-fatal repair pattern
- `crates/rc-agent/src/ac_launcher.rs` — ensure_conspit_link_running() (line 1390), enforce_safe_state() (line 1585), SESSION_END_IN_PROGRESS guard
- `.planning/research/conspit-link/ARCHITECTURE.md` — CRITICAL DISCOVERY section (line 83), Anti-Pattern 4 (config cache), Pattern 1 (config-then-restart)
- `.planning/research/conspit-link/STACK.md` — File locations (line 111), .Base preset format, GameToBaseConfig.json role
- `.planning/phases/59-auto-switch-configuration/59-CONTEXT.md` — All locked decisions

### Secondary (MEDIUM confidence)
- `.planning/research/conspit-link/PITFALLS.md` — Pitfall 11 (concurrent writers), Pitfall 4 (force-kill corruption), multiple write patterns
- `.planning/phases/58-conspitlink-process-hardening/58-01-SUMMARY.md` — What Phase 58 delivered, patterns established

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all required crates already in deps, no new dependencies
- Architecture: HIGH — all patterns exist in codebase (Phase 58), just need new function
- Pitfalls: HIGH — root cause confirmed from CL log, game key uncertainty is LOW (needs pod inspection)
- Test approach: HIGH — same _impl() pattern as Phase 58, testable with tmpdir

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (config file format is stable; ConspitLink version unchanged)
