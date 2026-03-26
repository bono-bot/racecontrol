# Phase 197: Launch Resilience & AC Hardening - Research

**Researched:** 2026-03-26
**Domain:** Game launcher resilience (Rust/Axum server + rc-agent), AC-specific reliability
**Confidence:** HIGH — all findings from direct source code inspection

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all locked to CONTEXT.md specifics below.

### Claude's Discretion
All implementation choices are at Claude's discretion — resilience/infrastructure phase. Key decisions:
- Dynamic timeout calculation strategy (median + 2*stdev from launch_events)
- Pre-launch health check implementation on rc-agent side
- Error taxonomy enum design
- Auto-retry mechanism (Race Engineer extension vs new system)
- Clean state reset: which processes to kill, what files to clean
- AC polling implementation (interval, max wait, backoff)
- WhatsApp alert format and triggering logic
- Whether to split into per-game launcher files or keep in game_launcher.rs

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LAUNCH-08 | Dynamic timeout: median(last 10 durations) + 2*stdev, per car/track/sim combo | launch_events table has duration_to_playable_ms; no query function exists yet — must create |
| LAUNCH-09 | Default timeouts: AC=120s, F1=90s, iRacing=90s when no history | check_game_health() currently hardcodes AC=120s, others=60s — needs update |
| LAUNCH-10 | Pre-launch health checks on rc-agent: no orphan game exe, disk > 1GB, no MAINTENANCE_MODE, no OTA_DEPLOYING | pre_flight.rs exists but handles HID/ConspitLink/orphan only; need disk + sentinel checks added |
| LAUNCH-11 | Clean state reset: kill all 13 game exe names, delete game.pid, clear shared memory adapter | game_process.rs has cleanup_orphaned_games() and all_game_process_names() (13 names) — reuse |
| LAUNCH-12 | Auto-retry: 2 attempts max, same launch_args, then WhatsApp alert | Race Engineer already does 2 retries but has atomicity bug — needs write lock fix |
| LAUNCH-13 | Error taxonomy: ProcessCrash(exit_code) must classify real exit codes not just string matching | ErrorTaxonomy enum in metrics.rs has ProcessCrash { exit_code: i64 } but classify_error_taxonomy() uses string matching only |
| LAUNCH-14 | Game crash counter separate from pod health counter — no MAINTENANCE_MODE from game crashes | MAINTENANCE_MODE sentinel_watcher.rs monitors it; Race Engineer must NOT write MAINTENANCE_MODE |
| LAUNCH-15 | Staff WhatsApp alert after 2 failed auto-retries with structured content | billing.rs has Evolution API pattern; no staff alert in Race Engineer yet |
| LAUNCH-16 | Relaunch fix: tracker with launch_args=None rejected with clear message | relaunch_game() already checks but doesn't guard null args before attempting |
| LAUNCH-17 | Race Engineer atomic: single write lock for counter increment + relaunch spawn | Current code does read lock then write lock separately — TOCTOU race condition |
| LAUNCH-18 | Timeout fires Race Engineer relaunch (not just Error state) | check_game_health() transitions to Error but does NOT trigger Race Engineer auto-retry |
| LAUNCH-19 | stop_game() logs sim_type (not empty string) | stop_game() logs empty string "" to activity log — bug confirmed at line 449 |
| AC-01 | AC post-kill wait polls for acs.exe absence (max 5s) not hardcoded 2s sleep | ac_launcher.rs line 293: `std::thread::sleep(2s)` — must replace with polling loop |
| AC-02 | AC load wait polls for AC window handle (max 30s) not hardcoded 8s sleep | ac_launcher.rs line 358: `std::thread::sleep(8s)` — must replace with window polling |
| AC-03 | CM timeout increased to 30s with 5s progress logging | wait_for_ac_process() called with timeout_secs=15 — must increase to 30 with progress logs |
| AC-04 | CM fallback: fresh PID via find_game_pid() not stale CM PID | Fallback at line 340 uses child.id() which is CM's child PID — must call find_acs_pid() |
</phase_requirements>

---

## Summary

Phase 197 adds resilience and reliability to a game launcher that already has solid structural foundations (from Phase 196) but lacks dynamic intelligence. The server-side (`game_launcher.rs`) handles launch orchestration, timeout detection, Race Engineer auto-retry, and metrics recording. The agent-side (`ac_launcher.rs`) handles the actual AC process lifecycle including Content Manager integration.

The research confirms 16 concrete gaps across both crates. The most systemic gaps are: (1) check_game_health() sets Error state but does NOT trigger Race Engineer retry on timeout — the retry path only fires from handle_game_state_update(); (2) Race Engineer has a read-then-write lock pattern that allows duplicate relaunches under rapid error events; (3) AC launch has three hardcoded sleeps that need polling replacements; (4) the pre-launch health check system (pre_flight.rs) exists but only checks HID/ConspitLink/orphan-game — disk space and sentinel file checks must be added.

**Primary recommendation:** Two plans — one for server-side (dynamic timeout + Race Engineer fixes + stop_game bug + WhatsApp alert + taxonomy fix) and one for agent-side (pre-launch checks + clean state reset + AC polling + CM timeout). Test each plan via cargo test before moving to the next.

---

## Standard Stack

### Core (already in use — no new dependencies)
| Library | Purpose | Location |
|---------|---------|----------|
| `sqlx` | SQLite queries for dynamic timeout (SELECT duration_to_playable_ms FROM launch_events) | racecontrol/Cargo.toml |
| `tokio` | Async spawn for Race Engineer relaunch delay | all crates |
| `tracing` | Structured logging for timeout progress | all crates |
| `serde_json` | launch_args parsing, WhatsApp payload construction | all crates |
| `reqwest` | Evolution API HTTP call for staff WhatsApp | racecontrol (billing.rs pattern) |
| `sysinfo` | Process scanning in agent (already used in cleanup_orphaned_games) | rc-agent |

### No new Cargo dependencies needed
All required functionality is achievable with existing crate dependencies. The dynamic timeout SQL query uses sqlx already imported. The staff WhatsApp alert uses the reqwest + Evolution API pattern already implemented in billing.rs.

---

## Architecture Patterns

### Pattern 1: Dynamic Timeout Query (server-side)
**What:** Query launch_events for the last 10 successful launches of a specific sim+car+track combo, compute median + 2*stdev.
**When to use:** At game launch time, before creating the GameTracker.
**Location:** New function in `metrics.rs` — `query_dynamic_timeout(db, sim_type, car, track) -> u64`

```rust
// In metrics.rs — query last 10 successful durations for a combo
pub async fn query_dynamic_timeout(
    db: &SqlitePool,
    sim_type: &str,
    car: Option<&str>,
    track: Option<&str>,
    default_secs: u64,
) -> u64 {
    let rows: Vec<(i64,)> = sqlx::query_as(
        "SELECT duration_to_playable_ms FROM launch_events
         WHERE sim_type = ? AND (car = ? OR ? IS NULL) AND (track = ? OR ? IS NULL)
           AND outcome = '\"Success\"'
           AND duration_to_playable_ms IS NOT NULL
         ORDER BY created_at DESC LIMIT 10"
    )
    .bind(sim_type).bind(car).bind(car).bind(track).bind(track)
    .fetch_all(db).await.unwrap_or_default();

    if rows.len() < 3 {
        tracing::info!("default timeout {}s for {}/{:?}/{:?} (insufficient history: {} samples)",
            default_secs, sim_type, car, track, rows.len());
        return default_secs;
    }

    // Compute median + 2*stdev in Rust (SQLite lacks these functions)
    let mut durations_ms: Vec<f64> = rows.iter().map(|(d,)| *d as f64).collect();
    durations_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = durations_ms[durations_ms.len() / 2];
    let mean = durations_ms.iter().sum::<f64>() / durations_ms.len() as f64;
    let variance = durations_ms.iter().map(|d| (d - mean).powi(2)).sum::<f64>()
        / durations_ms.len() as f64;
    let stdev = variance.sqrt();
    let timeout_ms = median + 2.0 * stdev;
    let timeout_secs = (timeout_ms / 1000.0).ceil() as u64;

    tracing::info!("dynamic timeout {}s for {}/{:?}/{:?} (median={:.0}ms stdev={:.0}ms samples={})",
        timeout_secs, sim_type, car, track, median, stdev, rows.len());
    timeout_secs.max(30) // floor at 30s regardless of history
}
```

**Key insight:** SQLite does not have PERCENTILE or NTILE window functions. Median + stdev must be computed in Rust after fetching the sorted rows. This is the same approach used in Phase 195-03 for P95 computation.

### Pattern 2: Race Engineer Atomic Fix
**What:** Single write lock for counter increment + spawn — prevents duplicate relaunches from rapid duplicate Error events.
**Current bug:** Two separate lock acquisitions (read lock for counter, write lock for increment) creates TOCTOU window.

```rust
// BEFORE (buggy — two lock acquisitions):
let relaunch_count = {
    let games = state.game_launcher.active_games.read().await;  // lock 1
    games.get(pod_id).map(|t| t.auto_relaunch_count).unwrap_or(999)
};
if relaunch_count < 2 {
    let mut games = state.game_launcher.active_games.write().await; // lock 2 — gap here!
    if let Some(tracker) = games.get_mut(pod_id) {
        tracker.auto_relaunch_count += 1;
    }
    // spawn relaunch
}

// AFTER (atomic — single write lock for check+increment+decision):
let should_relaunch = {
    let mut games = state.game_launcher.active_games.write().await;
    if let Some(tracker) = games.get_mut(pod_id) {
        if tracker.auto_relaunch_count < 2 {
            tracker.auto_relaunch_count += 1;
            Some(tracker.auto_relaunch_count) // return new count
        } else {
            None // exhausted
        }
    } else {
        None
    }
};
if let Some(attempt) = should_relaunch {
    tokio::spawn(async move { /* relaunch */ });
}
```

### Pattern 3: Pre-Launch Health Check Message (server → agent)
**What:** Server sends a new CoreToAgentMessage::PreLaunchCheck before CoreToAgentMessage::LaunchGame. Agent responds with AgentMessage::PreLaunchCheckResult.
**Alternative (simpler):** Pre-launch checks run on agent in response to LaunchGame itself — rejected with GameStateUpdate { state: Error, message: "MAINTENANCE_MODE active" }. No new message type needed.

**Recommendation:** Keep in existing LaunchGame handler on agent side. When agent receives LaunchGame, run pre-checks synchronously before spawning launch thread. If any check fails, send GameStateUpdate { state: Error, ... } immediately. This avoids protocol changes to rc-common/protocol.rs.

### Pattern 4: AC Polling Replacement (agent-side)
**What:** Replace hardcoded sleep() calls with polling loops using sysinfo/tasklist.

```rust
// BEFORE: std::thread::sleep(Duration::from_secs(2));  // after killing acs.exe

// AFTER: poll for acs.exe absence
fn wait_for_acs_exit(max_wait_secs: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(max_wait_secs);
    while std::time::Instant::now() < deadline {
        if find_acs_pid().is_none() {
            return true; // clean exit confirmed
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false // timed out
}

// BEFORE: std::thread::sleep(Duration::from_secs(8));  // wait for AC window

// AFTER: poll for AC window handle
fn wait_for_ac_window(max_wait_secs: u64) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(max_wait_secs);
    while std::time::Instant::now() < deadline {
        if find_ac_window_hwnd().is_some() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    false
}
```

`find_ac_window_hwnd()` uses `winapi::um::winuser::FindWindowW` with lpClassName=None, lpWindowName matching "Assetto Corsa" or NULL for first acs.exe window. The `winapi` crate is already a dependency for SendInput in mid_session module.

### Pattern 5: WhatsApp Staff Alert (server-side)
**What:** After 2 failed Race Engineer relaunches, send WhatsApp to Uday's number with structured content.
**Pattern:** Mirror billing.rs `send_whatsapp_receipt` — reqwest::Client, 5s timeout, Evolution API POST.

```rust
// Staff WhatsApp alert (Uday's number from config or hardcoded)
async fn send_staff_launch_alert(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type: &str,
    error_taxonomy: &str,
    attempt_codes: Vec<Option<i32>>,
) {
    let msg = format!(
        "🔧 Launch Failure — Pod {}\nGame: {}\nError: {}\nAttempts: {}\nAction: Check pod + try different car/track",
        pod_id, sim_type, error_taxonomy,
        attempt_codes.iter().map(|c| c.map_or("unknown".into(), |x| format!("{:#010x}", x))).collect::<Vec<_>>().join(", ")
    );
    // POST to Evolution API /message/sendText/{instance}
    // Staff phone: 7075778180 (from project_whatsapp_phone_mapping.md)
}
```

**Note:** Staff phone 7075778180 (from MEMORY.md `project_whatsapp_phone_mapping.md`). Use `state.config.auth.evolution_url/key/instance` pattern from billing.rs line 2381-2384.

### Pattern 6: ErrorTaxonomy ProcessCrash Fix
**What:** classify_error_taxonomy() currently does string matching only. For ProcessCrash exit codes, agent must send the numeric exit code in the error_message.

**Current flow:** Agent calls `GameState::Error` with `error_message = Some("Process exited with code 0xC0000005")`. Server's classify_error_taxonomy() receives this string and can only match by text.

**Fix:** Two-part:
1. Agent: when game process exits with known code, format as `"exit_code:3221225477"` prefix in error_message
2. Server classify_error_taxonomy(): detect `"exit_code:"` prefix, extract u64, return `ErrorTaxonomy::ProcessCrash { exit_code }`

Alternatively: Add `exit_code: Option<i64>` to `GameLaunchInfo` (rc-common/types.rs) so it flows typed through the protocol. This is cleaner but requires rc-common changes and recompile of both crates.

**Recommendation:** Add `exit_code: Option<i64>` to GameLaunchInfo in rc-common. Agent populates it from `child.try_wait()` result. Server uses it directly in handle_game_state_update() to build ErrorTaxonomy::ProcessCrash. This avoids string parsing hacks.

---

## Bug Inventory (confirmed from code)

### Bug 1: stop_game() logs empty sim_type (LAUNCH-19)
**File:** `crates/racecontrol/src/game_launcher.rs` line 449
**Code:** `log_pod_activity(state, pod_id, "game", "Game Stopping", "", "core");`
**Fix:** Capture `sim_type` from tracker before setting to Stopping, pass to log_pod_activity.
```rust
// In stop_game(), after capturing tracker info:
let sim_type_str = info.sim_type.to_string();
log_pod_activity(state, pod_id, "game", "Game Stopping", &sim_type_str, "core");
```
This is a one-line fix — sim_type is already in `info` at that point.

### Bug 2: AC kill wait is hardcoded 2s sleep (AC-01)
**File:** `crates/rc-agent/src/ac_launcher.rs` line 293
**Code:** `std::thread::sleep(std::time::Duration::from_secs(2));`
**Context:** After taskkill /F /IM acs.exe — 2s may not be enough if AC is mid-cleanup.
**Fix:** Replace with poll loop calling `find_acs_pid()` every 500ms, max 5s. Already have `find_acs_pid()` at line 1136.

### Bug 3: AC load wait is hardcoded 8s sleep (AC-02)
**File:** `crates/rc-agent/src/ac_launcher.rs` line 358
**Code:** `std::thread::sleep(std::time::Duration::from_secs(8));`
**Context:** After launching acs.exe — waiting 8s before minimizing Conspit Link. This is not enough on CSP-heavy configs, and too much on fast machines.
**Fix:** Poll for AC window handle using `FindWindowW`. Max wait 30s. Fallback: if window not found after 30s, proceed anyway (game may still be loading).

### Bug 4: CM wait_for_ac_process timeout is 15s (AC-03)
**File:** `crates/rc-agent/src/ac_launcher.rs` line 319
**Code:** `match wait_for_ac_process(15) {`
**Fix:** Increase to 30s. Add progress logging at 5s intervals inside `wait_for_ac_process()`.

### Bug 5: CM fallback uses stale PID (AC-04)
**File:** `crates/rc-agent/src/ac_launcher.rs` lines 337-341
**Code:** `let child = Command::new(ac_dir.join("acs.exe")).spawn()?; child.id()`
**Context:** When CM fails, falls back to direct acs.exe spawn. Returns `child.id()` from the spawn — but this is the correct pid for direct launch. However, the CM path before this spawned acs.exe as a CM child — its PID was from `wait_for_ac_process(15)` which correctly called `find_acs_pid()`. The *direct launch* case correctly uses spawn's PID.
**Actual issue:** When direct fallback launch happens, `persist_pid(pid)` is NOT called for the directly-spawned child. The `persist_pid()` call only happens in game_process.rs `launch()`, not in ac_launcher.rs. **Fix:** Call `game_process::persist_pid(pid)` after both launch paths in ac_launcher.

### Bug 6: Race Engineer TOCTOU on auto_relaunch_count (LAUNCH-17)
**File:** `crates/racecontrol/src/game_launcher.rs` lines 661-677
**Code:** Read lock to get count, then separate write lock to increment — two atomic operations not protected as one.
**Fix:** Combine into single write lock that reads, checks, and increments atomically (Pattern 2 above).

### Bug 7: check_game_health() timeout does NOT trigger Race Engineer (LAUNCH-18)
**File:** `crates/racecontrol/src/game_launcher.rs` lines 832-889
**Code:** Sets `tracker.game_state = GameState::Error` but then does NOT call the Race Engineer path.
**Fix:** After setting Error state in check_game_health(), synthetically call `handle_game_state_update()` with the error info, OR inline the Race Engineer logic (preferred — avoids circular dependency).

### Bug 8: split_whitespace on exe args breaks paths with spaces (LAUNCH-19 / arg parsing)
**File:** `crates/rc-agent/src/game_process.rs` line 191-193
**Code:** `for arg in args.split_whitespace() { cmd.arg(arg); }`
**Context:** `GameExeConfig.args = Some("C:\\Program Files\\Steam\\... F1_25.exe")` splits on spaces — breaks any path with spaces.
**Fix:** For F1Launcher and IRacingLauncher in rc-agent's event_loop.rs, pass args as a proper JSON array or use shell-quote parsing. Since ac_launcher.rs handles AC separately, this primarily affects the F1/iRacing path in game_process.rs.
**Recommendation:** Accept args as `Vec<String>` in GameExeConfig (or parse JSON array from launch_args). Alternatively, the LaunchGame message already carries `launch_args: Option<String>` as JSON — each sim's agent handler should parse its own JSON and construct args properly, not pass through GameExeConfig.args string.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead |
|---------|-------------|-------------|
| Process scanning on agent | Custom /tasklist parsing | `sysinfo::System::refresh_processes()` — already used in cleanup_orphaned_games() |
| Median/stdev calculation | Database aggregate SQL | Rust in-memory computation after fetching rows (SQLite has no PERCENTILE) |
| WhatsApp alert HTTP client | Custom HTTP wrapper | reqwest::Client with 5s timeout — same pattern as billing.rs lines 2388-2397 |
| AC window detection | Polling tasklist for window | `FindWindowW` from winapi crate (already imported for SendInput) |
| Disk space check | Custom wmic parser | Existing pod_healer.rs `check_disk_free_pct()` pattern — reuse in agent's pre-launch check |

---

## Common Pitfalls

### Pitfall 1: check_game_health() and handle_game_state_update() both set Error
**What goes wrong:** If check_game_health() fires and sets Error, then shortly after an agent message arrives also setting Error, Race Engineer fires twice — once from check_game_health() (if we add RE there) and once from handle_game_state_update(). Double relaunch.
**How to avoid:** Track whether timeout already triggered Race Engineer. Add `timeout_relaunch_triggered: bool` to GameTracker, or use the `auto_relaunch_count` — if count is already >= 1 when handle_game_state_update() fires, the second trigger skips. The atomic fix (Pattern 2) naturally handles this if both paths share the same write lock.

### Pitfall 2: Pre-launch checks on blocking thread
**What goes wrong:** Pre-launch checks in agent (disk check via wmic, tasklist scan) are blocking operations. Running them in the async tokio task blocks the event loop.
**How to avoid:** Use `tokio::task::spawn_blocking()` for disk check and tasklist scan — same pattern as pre_flight.rs line 17 `use tokio::task::spawn_blocking`.

### Pitfall 3: MAINTENANCE_MODE written by game crash recovery
**What goes wrong:** If clean state reset kills processes aggressively, rc-agent's self_monitor.rs may interpret rapid restarts as crash storm and write MAINTENANCE_MODE. This permanently blocks all future restarts.
**How to avoid:** LAUNCH-14 requirement — Race Engineer's clean state reset must NOT trigger MAINTENANCE_MODE. The clean state reset happens server-side (via agent message), not via self_monitor restart counting. Verify that killing game processes does NOT increment rc-agent's internal restart counter. The restart counter in self_monitor.rs only counts rc-agent restarts, not game process kills — confirmed safe.

### Pitfall 4: AC polling blocks the launch thread (which is already blocking)
**What goes wrong:** `launch_ac()` in ac_launcher.rs is a synchronous blocking function. Adding more polling loops extends blocking time. In event_loop.rs, this is called via `spawn_blocking`.
**How to avoid:** Polling inside `launch_ac()` is fine since the function is already expected to block. The caller (event_loop.rs) uses spawn_blocking. Keep polling intervals at 500ms to prevent tight CPU spin.

### Pitfall 5: Dynamic timeout stored per-combo but combo has no-car/no-track launches
**What goes wrong:** Some launches may have car=NULL, track=NULL (externally tracked, non-AC games). The SQL query must handle NULL safely.
**How to avoid:** Use `(car = ? OR ? IS NULL)` SQL pattern (shown in Pattern 1 above). When car/track is None in Rust, the query matches all launches for that sim_type.

### Pitfall 6: Race Engineer fires after billing session ended
**What goes wrong:** 5s delay before relaunch. If billing ends during those 5 seconds, the re-check `still_billing` returns false — this is correct and already implemented. No change needed here.

### Pitfall 7: CM 30s timeout extends launch time on CM-missing pods
**What goes wrong:** If `find_cm_exe()` fails to find CM (most single-player pods), the CM path is skipped entirely. But if CM IS present and fails, 30s timeout makes the total AC launch time 30s + direct fallback + 30s window wait = 60+ seconds before returning error. This is within the dynamic timeout window for AC (120s+).
**How to avoid:** The 30s CM timeout applies only to multiplayer mode (`params.game_mode == "multi"`). Single-player bypasses CM entirely — no impact.

---

## Code Examples

### Disk Check (Pre-launch, reuse pod_healer pattern)
```rust
// In ac_launcher.rs (agent-side) — reuse the wmic query pattern from pod_healer.rs
fn check_disk_space_gb() -> f64 {
    let output = hidden_cmd("cmd")
        .args(["/C", "wmic logicaldisk where \"DeviceID='C:'\" get freespace /format:csv"])
        .output();
    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                let parts: Vec<&str> = line.trim().split(',').collect();
                if parts.len() >= 2 {
                    if let Ok(bytes) = parts.last().unwrap_or(&"0").trim().parse::<u64>() {
                        return bytes as f64 / (1024.0 * 1024.0 * 1024.0);
                    }
                }
            }
            0.0
        }
        Err(_) => 0.0,
    }
}
```

### Sentinel File Check (Pre-launch)
```rust
// Check MAINTENANCE_MODE or OTA_DEPLOYING before launch
fn check_sentinel_absent(sentinel_name: &str) -> Result<(), String> {
    let path = std::path::Path::new(r"C:\RacingPoint").join(sentinel_name);
    if path.exists() {
        Err(format!("{} active — launch rejected", sentinel_name))
    } else {
        Ok(())
    }
}
```

### Stop Logging Fix (one-liner)
```rust
// game_launcher.rs stop_game() — BEFORE line that was:
// log_pod_activity(state, pod_id, "game", "Game Stopping", "", "core");
// AFTER:
log_pod_activity(state, pod_id, "game", "Game Stopping", &info.sim_type.to_string(), "core");
```

### CM Progress Logging
```rust
fn wait_for_ac_process(timeout_secs: u64) -> Result<u32> {
    let poll_interval = std::time::Duration::from_millis(500);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    let start = std::time::Instant::now();
    let mut last_log_secs = 0u64;

    while std::time::Instant::now() < deadline {
        if let Some(pid) = find_acs_pid() {
            tracing::info!(target: LOG_TARGET, "Found acs.exe with PID {} after {:.1}s", pid, start.elapsed().as_secs_f32());
            return Ok(pid);
        }
        let elapsed_secs = start.elapsed().as_secs();
        if elapsed_secs / 5 > last_log_secs / 5 && elapsed_secs >= 5 {
            tracing::info!(target: LOG_TARGET, "CM progress: checking acs.exe... ({}s elapsed)", elapsed_secs);
            last_log_secs = elapsed_secs;
        }
        std::thread::sleep(poll_interval);
    }
    anyhow::bail!("acs.exe did not appear within {}s after CM launch", timeout_secs)
}
```

---

## State of the Art

| Old Approach | Current Approach | Phase 197 Change |
|--------------|------------------|-----------------|
| Hardcoded 120s AC timeout | Hardcoded per-game timeout in check_game_health() | Dynamic: median + 2*stdev from launch_events (LAUNCH-08) |
| No pre-launch checks on agent | Pre-flight (HID/Conspit/orphan) on billing start | Add disk + MAINTENANCE_MODE + OTA_DEPLOYING to launch path (LAUNCH-10) |
| Race Engineer fires from Error event only | Race Engineer fires from handle_game_state_update() only | Also fire from timeout path in check_game_health() (LAUNCH-18) |
| Read-then-write lock on relaunch count | Two separate lock acquisitions (TOCTOU) | Single write lock: check + increment + spawn atomically (LAUNCH-17) |
| 2s sleep after kill, 8s sleep after launch | Hardcoded sleeps in ac_launcher.rs | Poll for acs.exe absence (max 5s), poll for AC window (max 30s) (AC-01, AC-02) |
| CM timeout = 15s, no progress logs | wait_for_ac_process(15) | CM timeout = 30s, progress logs at 5s intervals (AC-03) |

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + tokio::test |
| Config file | none (in-process) |
| Quick run command | `cargo test -p racecontrol game_launcher -- --nocapture` |
| Full suite command | `cargo test -p racecontrol && cargo test -p rc-agent && cargo test -p rc-common` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LAUNCH-08 | Dynamic timeout: insert 10 events, verify computed timeout | unit | `cargo test -p racecontrol metrics::tests -- --nocapture` | ❌ Wave 0 |
| LAUNCH-09 | Default timeout: no history → AC=120s, F1=90s | unit | `cargo test -p racecontrol game_launcher::tests::test_default_timeout` | ❌ Wave 0 |
| LAUNCH-10 | Pre-launch rejection: MAINTENANCE_MODE file present | unit | `cargo test -p racecontrol game_launcher::tests::test_prelaunch_maintenance` | ❌ Wave 0 |
| LAUNCH-11 | Clean state reset: all 13 names killed | unit | `cargo test -p rc-agent game_process::tests::test_cleanup_names` | ✅ (partial) |
| LAUNCH-12 | Auto-retry: 2 attempts then WhatsApp alert | unit | `cargo test -p racecontrol game_launcher::tests::test_race_engineer_retry` | ❌ Wave 0 |
| LAUNCH-13 | ProcessCrash(exit_code) classification from typed field | unit | `cargo test -p racecontrol game_launcher::tests::test_error_taxonomy` | ✅ (partial — string-based) |
| LAUNCH-14 | No MAINTENANCE_MODE from game crash counter | unit | verify no sentinel write in race engineer code path | manual verify |
| LAUNCH-15 | WhatsApp staff alert content and trigger | manual | Trigger 2 failed relaunches on Pod 8, verify WhatsApp received | manual-only |
| LAUNCH-16 | Null args rejection with clear message | unit | `cargo test -p racecontrol game_launcher::tests::test_relaunch_null_args` | ❌ Wave 0 |
| LAUNCH-17 | Atomic counter: 2 rapid Error events → 1 relaunch | unit | `cargo test -p racecontrol game_launcher::tests::test_atomic_relaunch` | ❌ Wave 0 |
| LAUNCH-18 | Timeout → Race Engineer fires (not just Error state) | unit | `cargo test -p racecontrol game_launcher::tests::test_timeout_triggers_relaunch` | ❌ Wave 0 |
| LAUNCH-19 | stop_game logs non-empty sim_type | unit | `cargo test -p racecontrol game_launcher::tests::test_stop_game_logs_sim_type` | ❌ Wave 0 |
| AC-01 | Post-kill polling: no hardcoded 2s sleep | code review | `grep "sleep.*from_secs(2)" crates/rc-agent/src/ac_launcher.rs` returns zero | ✅ (post-fix) |
| AC-02 | Load wait polling: no hardcoded 8s sleep | code review | `grep "sleep.*from_secs(8)" crates/rc-agent/src/ac_launcher.rs` returns zero | ✅ (post-fix) |
| AC-03 | CM timeout 30s with 5s progress logs | code review + manual | `grep "wait_for_ac_process(30)" ac_launcher.rs` | ✅ (post-fix) |
| AC-04 | Fresh PID after direct fallback | code review | `grep "persist_pid" ac_launcher.rs` shows call after direct spawn | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol 2>&1 | tail -5`
- **Per wave merge:** `cargo test -p racecontrol && cargo test -p rc-agent && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/game_launcher.rs` — add test stubs: `test_default_timeout`, `test_prelaunch_maintenance`, `test_race_engineer_retry`, `test_relaunch_null_args`, `test_atomic_relaunch`, `test_timeout_triggers_relaunch`, `test_stop_game_logs_sim_type`
- [ ] `crates/racecontrol/src/metrics.rs` — add `test_dynamic_timeout_query` using in-memory SQLite
- [ ] Framework install: none needed — `#[test]` built-in

*(Existing test infrastructure in game_launcher.rs `#[cfg(test)] mod tests` at line 994 covers make_state() setup — new tests extend this module)*

---

## Plan Structure Recommendation

**Two plans for this phase:**

**Plan 197-01: Server-Side Resilience** (racecontrol crate)
- LAUNCH-08: Dynamic timeout query function in metrics.rs
- LAUNCH-09: Default timeouts update in check_game_health() (AC=120, F1=90, iRacing=90)
- LAUNCH-12: Race Engineer atomic fix (Pattern 2) + null args guard (LAUNCH-16)
- LAUNCH-13: ErrorTaxonomy ProcessCrash via typed exit_code field in rc-common GameLaunchInfo
- LAUNCH-14: Verify no MAINTENANCE_MODE writes in Race Engineer path
- LAUNCH-15: WhatsApp staff alert after 2 failed attempts
- LAUNCH-17: Single write lock atomic counter
- LAUNCH-18: check_game_health() triggers Race Engineer (not just Error state)
- LAUNCH-19: stop_game() logs correct sim_type

**Plan 197-02: Agent-Side AC Hardening** (rc-agent crate)
- LAUNCH-10: Pre-launch checks in LaunchGame handler (disk, MAINTENANCE_MODE, OTA_DEPLOYING, orphan exe)
- LAUNCH-11: Clean state reset function (all 13 names + game.pid + adapter reset)
- AC-01: Post-kill polling (2s sleep → poll max 5s)
- AC-02: Load wait polling (8s sleep → poll max 30s for AC window)
- AC-03: CM timeout 15s → 30s with progress logging
- AC-04: persist_pid() after direct fallback spawn

**Compile dependency:** rc-common change (exit_code on GameLaunchInfo) must be in 197-01 first. 197-02 then picks it up. Both compile independently except for the rc-common shared type — sequence 197-01 then 197-02.

---

## Open Questions

1. **AC Window Detection API**
   - What we know: `winapi` crate is already imported (mid_session module uses SendInput). `FindWindowW` exists.
   - What's unclear: The AC window title string on pods. May be "Assetto Corsa" or may use a different window class. Need to verify on a live pod.
   - Recommendation: Use `EnumWindows` with `GetWindowThreadProcessId` to find a window belonging to the acs.exe PID — avoids window title dependency entirely.

2. **Staff WhatsApp phone number hardcoding**
   - What we know: Staff phone is 7075778180 per MEMORY.md project_whatsapp_phone_mapping.md
   - What's unclear: Should this be in racecontrol.toml config or hardcoded?
   - Recommendation: Add `staff_alert_phone` field to `[auth]` section in Config. Default to empty (no alert if not configured). Matches the pattern of optional Evolution API config.

3. **Pre-launch check location: server vs agent**
   - What we know: CONTEXT.md says "pre-launch checks on rc-agent side"
   - What's unclear: If agent is disconnected at launch time, server already rejects immediately. Pre-launch checks on agent add latency before agent confirms launch received.
   - Recommendation: Agent checks run synchronously at the top of the LaunchGame handler, before spawning the launch thread. Return GameStateUpdate { Error } if any check fails. This keeps the check at the right abstraction layer (agent knows local disk/sentinel state) without adding a round-trip.

---

## Sources

### Primary (HIGH confidence)
- Direct inspection of `crates/racecontrol/src/game_launcher.rs` — full code read, all bug locations confirmed with line numbers
- Direct inspection of `crates/rc-agent/src/ac_launcher.rs` — hardcoded sleeps confirmed at lines 293, 358; CM timeout confirmed at line 319
- Direct inspection of `crates/racecontrol/src/metrics.rs` — ErrorTaxonomy enum, LaunchEvent schema
- Direct inspection of `crates/rc-agent/src/game_process.rs` — all_game_process_names() confirmed 13 entries, split_whitespace bug at line 191
- Direct inspection of `crates/rc-agent/src/event_loop.rs` — CrashRecoveryState, ConnectionState, existing launch state machine
- Direct inspection of `crates/rc-agent/src/pre_flight.rs` — existing check pattern (HID/ConspitLink/orphan)
- Direct inspection of `crates/racecontrol/src/billing.rs` — Evolution API WhatsApp pattern (lines 2381-2450)
- Direct inspection of `crates/racecontrol/src/db/mod.rs` — launch_events table schema (lines 319-337)

### Secondary (MEDIUM confidence)
- CONTEXT.md specifics — phase boundary, requirement details, clean state spec (13 game exe names)
- ROADMAP.md Phase 197 success criteria — 16 verification conditions

---

## Metadata

**Confidence breakdown:**
- Bug locations: HIGH — line-number verified in source
- Fix patterns: HIGH — consistent with existing codebase patterns
- Dynamic timeout SQL: HIGH — SQLite limitations confirmed (no PERCENTILE)
- AC window API: MEDIUM — winapi crate present but window title unverified without live pod
- WhatsApp staff alert: HIGH — same pattern as billing.rs, phone number in MEMORY.md

**Research date:** 2026-03-26 IST
**Valid until:** 2026-04-25 (stable codebase, no external dependencies)
