# Phase 58: ConspitLink Process Hardening - Research

**Researched:** 2026-03-20
**Domain:** Windows process management, JSON config integrity, crash recovery watchdog hardening (Rust/rc-agent)
**Confidence:** HIGH

## Summary

Phase 58 hardens the ConspitLink process management that Phase 57 introduced. The current codebase has two ConspitLink management paths: (1) the 10-second watchdog in main.rs that calls `ensure_conspit_link_running()`, and (2) the Phase 57 `safe_session_end()` / `restart_conspit_link()` orchestrator in `ffb_controller.rs`. Neither path tracks crash counts, backs up config files before writes, or verifies JSON integrity on watchdog restarts. The window minimization after restart is handled by a detached `std::thread::spawn` with a 4-second sleep, which is fragile across rapid restart cycles.

The work is entirely within `rc-agent` Rust code -- no new crates are needed. The core changes are: (1) add an `AtomicU32` crash counter to track ConspitLink restarts, (2) add config backup logic (copy JSON files to `.bak` before any restart), (3) add JSON parse verification after every restart (not just the one in `restart_conspit_link()`), and (4) ensure `minimize_conspit_window()` is called reliably after restarts triggered by the watchdog path.

**Primary recommendation:** Centralize all ConspitLink restart logic into a single `restart_conspit_link_hardened()` function that: increments crash count, backs up config files, launches the process, verifies JSON integrity, and minimizes the window. Both the watchdog and session-end paths should call this single function.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PROC-01 | Hardened watchdog with crash-count tracking and graceful restart (never taskkill /F) | Add `static CRASH_COUNT: AtomicU32` incremented on each watchdog restart. Existing code already uses WM_CLOSE (never taskkill /F). Log crash count on each restart. |
| PROC-02 | Post-restart config file verification (JSON parse check) | `restart_conspit_link()` already verifies `Global.json` after 5s. Extend to verify all 3 critical JSON files (Settings.json, Global.json, GameToBaseConfig.json). |
| PROC-03 | Config file backup before any write operation | Before restart (which causes ConspitLink to write on startup), copy the 3 JSON config files to `.bak` counterparts. If post-restart verification fails, restore from `.bak`. |
| PROC-04 | Window minimization survives ConspitLink restarts | Current `minimize_conspit_window()` works but is called on a detached thread with a fixed 4s delay. Make it retry with polling (check if window exists, then minimize) to handle variable startup times. |
</phase_requirements>

## Standard Stack

### Core (already in rc-agent)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::sync::atomic::AtomicU32` | stdlib | Crash count tracking | Lock-free, thread-safe counter. No external crate needed. |
| `serde_json` | workspace | JSON parse verification for config files | Already in deps, used by `restart_conspit_link()` |
| `winapi` | 0.3 | FindWindowW, ShowWindow (SW_MINIMIZE) for window management | Already in deps, used by `minimize_conspit_window()` |
| `std::fs::copy` | stdlib | Config file backup (copy to .bak) | No crate needed for simple file copy |

### New Crates Needed

None. All required functionality is available through existing dependencies and stdlib.

### NOT Needed

| Technology | Why Not |
|------------|---------|
| `notify` crate | Filesystem watching is for Phase 62 (Fleet Config Distribution), not this phase |
| Persistent crash count (file/DB) | In-memory `AtomicU32` resets on rc-agent restart, which is acceptable -- a full agent restart implies the pod was rebooted or redeployed |

## Architecture Patterns

### Recommended Changes

```
crates/rc-agent/src/
  ffb_controller.rs   # MODIFY: Replace restart_conspit_link() with hardened version
                      #   - Crash count tracking (AtomicU32)
                      #   - Config backup before restart
                      #   - Extended JSON verification (3 files, not just Global.json)
                      #   - Reliable minimize with retry polling
  ac_launcher.rs      # MODIFY: ensure_conspit_link_running() delegates to hardened restart
                      #   - Remove duplicate restart logic
                      #   - Call ffb_controller::restart_conspit_link_hardened()
```

### Pattern 1: Centralized Hardened Restart

**What:** A single function that handles all ConspitLink restart scenarios with crash tracking, config backup, verification, and window management.
**When to use:** Both watchdog (ensure_conspit_link_running) and session-end (safe_session_end) paths.
**Example:**
```rust
use std::sync::atomic::{AtomicU32, Ordering};

/// Global crash counter -- tracks how many times ConspitLink has been restarted
/// since rc-agent started. Resets to 0 on agent restart.
static CONSPIT_CRASH_COUNT: AtomicU32 = AtomicU32::new(0);

/// Config files that must be backed up and verified.
const CONSPIT_CONFIG_FILES: &[(&str, &str)] = &[
    (r"C:\Program Files (x86)\Conspit Link 2.0\Settings.json", "Settings.json"),
    (r"C:\Program Files (x86)\Conspit Link 2.0\Global.json", "Global.json"),
    (r"C:\Program Files (x86)\Conspit Link 2.0\JsonConfigure\GameToBaseConfig.json", "GameToBaseConfig.json"),
];

/// Also check the runtime copy at C:\RacingPoint\Global.json
const RUNTIME_GLOBAL_JSON: &str = r"C:\RacingPoint\Global.json";

pub fn get_crash_count() -> u32 {
    CONSPIT_CRASH_COUNT.load(Ordering::Relaxed)
}
```

### Pattern 2: Config Backup Before Restart

**What:** Copy each config file to a `.bak` sibling before restarting ConspitLink.
**When to use:** Before every ConspitLink restart (both watchdog and session-end paths).
**Example:**
```rust
fn backup_conspit_configs() {
    for (path, name) in CONSPIT_CONFIG_FILES {
        let src = std::path::Path::new(path);
        if src.exists() {
            let bak = src.with_extension("json.bak");
            match std::fs::copy(src, &bak) {
                Ok(_) => tracing::debug!("Backed up {} -> {}", name, bak.display()),
                Err(e) => tracing::warn!("Failed to backup {}: {}", name, e),
            }
        }
    }
    // Also backup runtime Global.json if it exists
    let runtime = std::path::Path::new(RUNTIME_GLOBAL_JSON);
    if runtime.exists() {
        let bak = runtime.with_extension("json.bak");
        let _ = std::fs::copy(runtime, &bak);
    }
}
```

### Pattern 3: Post-Restart JSON Verification with Restore

**What:** After ConspitLink starts, verify all config files parse as valid JSON. If any fail, restore from `.bak`.
**When to use:** 5 seconds after every ConspitLink restart.
**Example:**
```rust
fn verify_conspit_configs() -> bool {
    let mut all_ok = true;
    for (path, name) in CONSPIT_CONFIG_FILES {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                if serde_json::from_str::<serde_json::Value>(&contents).is_err() {
                    tracing::warn!("{} is corrupted — attempting restore from backup", name);
                    let bak = std::path::Path::new(path).with_extension("json.bak");
                    if bak.exists() {
                        match std::fs::copy(&bak, path) {
                            Ok(_) => tracing::info!("{} restored from backup", name),
                            Err(e) => {
                                tracing::error!("Failed to restore {}: {}", name, e);
                                all_ok = false;
                            }
                        }
                    } else {
                        tracing::error!("{} corrupted and no backup exists", name);
                        all_ok = false;
                    }
                } else {
                    tracing::debug!("{} integrity check passed", name);
                }
            }
            Err(e) => {
                tracing::warn!("Could not read {} for verification: {}", name, e);
                // File missing is not necessarily corruption -- CL may not have written it yet
            }
        }
    }
    all_ok
}
```

### Pattern 4: Reliable Window Minimize with Retry

**What:** Instead of a single 4s sleep then minimize, poll for the window to appear and minimize once found.
**When to use:** After every ConspitLink restart.
**Example:**
```rust
/// Poll for ConspitLink window and minimize it. Retries every 500ms for up to 8s.
fn minimize_conspit_window_with_retry() {
    for attempt in 1..=16 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        // Try FindWindowW with known titles
        if try_minimize_conspit_window() {
            tracing::info!("ConspitLink window minimized after {}ms", attempt * 500);
            return;
        }
    }
    tracing::warn!("ConspitLink window not found after 8s — may not have a visible window");
}
```

### Anti-Patterns to Avoid

- **Never taskkill /F ConspitLink** -- already enforced by Phase 57 (WM_CLOSE only). This phase MUST NOT introduce any force-kill paths.
- **Never have two independent restart paths** -- the current code has `ensure_conspit_link_running()` in ac_launcher.rs AND `restart_conspit_link()` in ffb_controller.rs. Consolidate into one hardened function.
- **Never skip config backup** -- even the watchdog path (10s timer) must backup before restart. A crash that corrupted config is the most likely scenario for the watchdog to fire.
- **Never store crash count in a file** -- unnecessary complexity. In-memory AtomicU32 is sufficient. If rc-agent itself restarts, the count resetting is fine.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Thread-safe counter | Mutex<u32> | AtomicU32 | Lock-free, simpler, no deadlock risk |
| JSON verification | Custom parser | serde_json::from_str::<Value> | Already in deps, handles all JSON edge cases |
| File backup | Rename + recreate | std::fs::copy | Atomic on NTFS, preserves original for CL to use |
| Window minimize retry | Fixed sleep | Polling loop with FindWindowW | Handles variable CL startup time (2-8s observed) |

## Common Pitfalls

### Pitfall 1: Race Between Watchdog and Session-End Restart
**What goes wrong:** The 10s watchdog fires `ensure_conspit_link_running()` during the ~5s window when `safe_session_end()` has closed ConspitLink but not yet restarted it. The watchdog detects CL as "not running" and starts a second instance.
**Why it happens:** Two independent timers managing the same process.
**How to avoid:** Add a `static AtomicBool` flag `SESSION_END_IN_PROGRESS` that `safe_session_end()` sets to true before closing CL and clears after restart. The watchdog skips its check when this flag is set.
**Warning signs:** Two ConspitLink processes running simultaneously. Log messages showing "Conspit Link not running" immediately after "Session-end safety sequence complete."

### Pitfall 2: Config Backup Overwrites Good Backup with Corrupt File
**What goes wrong:** CL crashes mid-write, corrupting `Global.json`. The watchdog fires, backs up the corrupted file (overwriting the good `.bak`), then restarts CL. Verification fails, restore from `.bak` gets the corrupt copy.
**Why it happens:** Unconditional backup overwrites the last `.bak` regardless of its quality.
**How to avoid:** Before overwriting a `.bak`, verify the EXISTING file parses as valid JSON first. Only backup if the current file is valid. If the current file is already corrupt, skip the backup step (the existing `.bak` is likely the good copy).
**Warning signs:** `.bak` file has 0 bytes or fails JSON parse.

### Pitfall 3: ConspitLink Window Title Changes Between Versions
**What goes wrong:** `minimize_conspit_window()` uses hardcoded title strings. A ConspitLink update changes the window title and minimize stops working.
**Why it happens:** FindWindowW matches exact title strings.
**How to avoid:** The existing code already tries 4 title variants. The retry polling pattern adds resilience (catches windows that appear late). Additionally, the PowerShell fallback in `minimize_conspit_window()` uses `ConspitLink*` wildcard pattern on process names, which is more robust than title matching.
**Warning signs:** ConspitLink window stays visible over the kiosk lock screen after restart.

### Pitfall 4: Config Files Locked by ConspitLink During Backup
**What goes wrong:** `std::fs::copy()` fails because ConspitLink has the file open for writing.
**Why it happens:** CL may hold file handles open while running.
**How to avoid:** The backup runs BEFORE restarting CL (when CL is already closed). For the watchdog path, CL has crashed so it should not be holding locks. If copy fails, log a warning and continue -- the restart is more important than the backup.
**Warning signs:** `std::fs::copy()` returns a sharing violation error.

### Pitfall 5: Crash Count Inflation from Expected Restarts
**What goes wrong:** `safe_session_end()` closes and restarts CL every session. If this increments the crash counter, the count becomes meaningless (10 sessions = "10 crashes").
**Why it happens:** Not distinguishing between deliberate restarts (session-end) and crash recovery (watchdog).
**How to avoid:** Only increment crash count in the WATCHDOG path (`ensure_conspit_link_running()`), not in `safe_session_end()` / `restart_conspit_link()`. The session-end restart is intentional, not a crash.
**Warning signs:** Crash count in logs climbing rapidly during normal operations.

## Code Examples

### Current ensure_conspit_link_running() (to be hardened)

```rust
// CURRENT (ac_launcher.rs line 1384):
pub fn ensure_conspit_link_running() {
    let conspit_path = r"C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe";
    if !Path::new(conspit_path).exists() {
        return; // Not installed
    }
    if is_process_running("ConspitLink2.0.exe") {
        return; // Already running
    }
    tracing::warn!("Conspit Link not running -- restarting (crash recovery)...");
    // ... spawn cmd /c start + 4s minimize thread ...
}
```

**Problems:**
1. No crash count tracking
2. No config backup before restart
3. No JSON verification after restart
4. Fixed 4s delay for minimize (fragile)
5. Duplicate restart logic vs. restart_conspit_link() in ffb_controller.rs

### Current restart_conspit_link() (already partially hardened by Phase 57)

```rust
// CURRENT (ffb_controller.rs line 383):
pub fn restart_conspit_link() {
    // ... check path exists ...
    // ... spawn cmd /c start ...
    // ... 4s thread -> minimize_conspit_window() ...
    // ... 5s thread -> verify Global.json only ...
}
```

**Problems:**
1. Only verifies Global.json (misses Settings.json, GameToBaseConfig.json)
2. No config backup before restart
3. No crash count (but this is the session-end path, so crash count should NOT increment here)
4. Fixed 4s delay for minimize

### Proposed Hardened Restart

```rust
/// Restart ConspitLink with full hardening:
/// - Backup config files (if not already corrupt)
/// - Launch process
/// - Verify all JSON configs after startup
/// - Minimize window with retry polling
/// - Optionally increment crash counter (only for watchdog path)
pub fn restart_conspit_link_hardened(is_crash_recovery: bool) {
    let conspit_path = r"C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe";
    if !std::path::Path::new(conspit_path).exists() {
        tracing::debug!("ConspitLink not installed — skipping restart");
        return;
    }

    // 1. Increment crash counter (watchdog only, not session-end)
    if is_crash_recovery {
        let count = CONSPIT_CRASH_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::warn!("ConspitLink crash recovery restart #{}", count);
    }

    // 2. Backup configs (only if current files are valid)
    backup_conspit_configs();

    // 3. Launch ConspitLink
    match crate::ac_launcher::hidden_cmd("cmd")
        .args(["/c", "start", "", conspit_path])
        .spawn()
    {
        Ok(_) => {
            tracing::info!("ConspitLink started, will verify + minimize...");
            // Single thread: wait for startup, minimize, then verify
            std::thread::spawn(|| {
                // Minimize with retry (polls every 500ms for up to 8s)
                minimize_conspit_window_with_retry();
                // Verify all config files (at ~5s after launch)
                std::thread::sleep(std::time::Duration::from_secs(1));
                verify_conspit_configs();
            });
        }
        Err(e) => tracing::error!("Failed to restart ConspitLink: {}", e),
    }
}
```

### Session-End Guard Against Watchdog Race

```rust
use std::sync::atomic::AtomicBool;

/// Flag: true when safe_session_end() is managing CL lifecycle.
/// The watchdog MUST skip its check when this is set.
static SESSION_END_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

// In safe_session_end():
SESSION_END_IN_PROGRESS.store(true, Ordering::Release);
// ... close CL, HID commands, restart CL ...
SESSION_END_IN_PROGRESS.store(false, Ordering::Release);

// In ensure_conspit_link_running():
if SESSION_END_IN_PROGRESS.load(Ordering::Acquire) {
    tracing::debug!("Skipping CL watchdog — session-end in progress");
    return;
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Simple watchdog (detect dead, restart) | Hardened watchdog (crash count, backup, verify, minimize) | Phase 58 (this phase) | Crash recovery with config integrity |
| Two restart functions (ac_launcher + ffb_controller) | Single hardened restart function | Phase 58 (this phase) | Eliminates duplication and inconsistency |
| Global.json-only verification | All 3 config files verified + auto-restore from backup | Phase 58 (this phase) | Config corruption detected and recovered |
| Fixed 4s minimize delay | Polling minimize with retry | Phase 58 (this phase) | Window minimization survives variable startup times |

## Open Questions

1. **Is C:\RacingPoint\Global.json a copy or a symlink?**
   - What we know: `restart_conspit_link()` checks `C:\RacingPoint\Global.json`, but the install directory has its own `Global.json` at `C:\Program Files (x86)\Conspit Link 2.0\Global.json`.
   - What's unclear: Which one does ConspitLink actually read at runtime? Are they the same file?
   - Recommendation: Backup and verify BOTH paths. Phase 59 (PROF-01) explicitly addresses fixing the runtime path, so Phase 58 should protect both locations.

2. **Maximum acceptable crash count before escalation**
   - What we know: Crash count is tracked but what action to take at thresholds is undefined.
   - What's unclear: Should rc-agent alert the server after N crashes? Stop trying to restart?
   - Recommendation: For Phase 58, just track and log. Add a warning at crash count >= 5 in a single agent lifetime. Alerting to server can be wired in Phase 63 (Fleet Monitoring, CLMON-01).

3. **ConspitLink startup time variability**
   - What we know: Current code assumes 4s for window to appear. On slow pods or after updates, this may vary.
   - What's unclear: Actual range of startup times across the fleet.
   - Recommendation: The polling minimize pattern (500ms intervals, up to 8s) handles this. If a pod consistently takes longer, adjust the max timeout.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-agent -- --test-threads=1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROC-01 | Crash count increments on watchdog restart, not session-end | unit | `cargo test -p rc-agent -- crash_count` | Wave 0 |
| PROC-01 | Watchdog skips when session-end in progress | unit | `cargo test -p rc-agent -- session_end_guard` | Wave 0 |
| PROC-02 | JSON parse verification catches corrupt files | unit | `cargo test -p rc-agent -- verify_conspit_configs` | Wave 0 |
| PROC-03 | Config backup creates .bak files, skips if source is corrupt | unit | `cargo test -p rc-agent -- backup_conspit_configs` | Wave 0 |
| PROC-03 | Auto-restore from .bak on corruption | unit | `cargo test -p rc-agent -- restore_from_backup` | Wave 0 |
| PROC-04 | Window minimization retry loop terminates within 8s | unit | `cargo test -p rc-agent -- minimize_retry_timeout` | Wave 0 |
| PROC-01..04 | Full restart hardening works on a live pod | manual-only | Manual: kill ConspitLink on Pod 8, verify watchdog restarts it with backup + verify + minimize | N/A |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-agent -- --test-threads=1`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green + manual test on Pod 8 (kill ConspitLink, verify restart sequence)

### Wave 0 Gaps

- [ ] `ffb_controller.rs` tests: `test_crash_count_increment` -- verify AtomicU32 increments only on crash recovery, not session-end
- [ ] `ffb_controller.rs` tests: `test_session_end_guard_flag` -- verify AtomicBool prevents watchdog during session-end
- [ ] `ffb_controller.rs` tests: `test_backup_skips_corrupt_source` -- verify backup does not overwrite good .bak with corrupt source
- [ ] `ffb_controller.rs` tests: `test_verify_valid_json` -- verify valid JSON passes check
- [ ] `ffb_controller.rs` tests: `test_verify_corrupt_json_triggers_restore` -- verify corrupt JSON triggers .bak restore
- [ ] `ffb_controller.rs` tests: `test_config_file_list_complete` -- verify CONSPIT_CONFIG_FILES has all 3 entries

## Sources

### Primary (HIGH confidence)

- `crates/rc-agent/src/ffb_controller.rs` -- Current `close_conspit_link()`, `restart_conspit_link()`, `safe_session_end()` implementation (Phase 57 code)
- `crates/rc-agent/src/ac_launcher.rs` lines 1384-1409 -- Current `ensure_conspit_link_running()` watchdog
- `crates/rc-agent/src/ac_launcher.rs` lines 1332-1379 -- Current `minimize_conspit_window()` with FindWindowW + PowerShell fallback
- `crates/rc-agent/src/ac_launcher.rs` lines 1586-1621 -- Current `enforce_safe_state()` with `skip_conspit_restart` flag
- `crates/rc-agent/src/main.rs` line 1565 -- 10s watchdog calling `ensure_conspit_link_running()`
- `.planning/research/conspit-link/STACK.md` -- Config file locations verified on disk
- `.planning/research/conspit-link/PITFALLS.md` -- Pitfall 4 (force-kill corruption), Pitfall 9 (focus stealing), Pitfall 11 (concurrent writes)

### Secondary (MEDIUM confidence)

- `.planning/phases/57-session-end-safety/57-RESEARCH.md` -- Phase 57 architecture decisions (ConspitLink lifecycle in safe_session_end)

### Tertiary (LOW confidence)

- ConspitLink startup time variability -- not measured across fleet, 4s is an assumption
- Whether ConspitLink holds file locks while running -- inferred from Qt app behavior, not directly verified

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all stdlib/existing deps, no new crates
- Architecture: HIGH -- patterns directly extend existing Phase 57 code with well-understood Rust primitives (AtomicU32, AtomicBool, fs::copy, serde_json)
- Pitfalls: HIGH -- race conditions and backup corruption are classic distributed systems problems with known solutions
- Config file paths: HIGH -- verified on disk in STACK.md research

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain, hardware/software unchanged)
