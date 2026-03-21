# Domain Pitfalls: AC Launch & Session Management

**Domain:** Assetto Corsa programmatic launch, session management, and multiplayer orchestration for a commercial sim racing venue
**Researched:** 2026-03-13
**Confidence:** HIGH for pitfalls derived from codebase + debugging playbook; MEDIUM for web-sourced AC community patterns

---

## Critical Pitfalls

Mistakes that cause customer-facing failures, billing inaccuracies, or safety incidents. These demand prevention, not just detection.

---

### Pitfall 1: Billing Starts Before Customer is Actually Driving

**What goes wrong:**
The billing timer starts when `launch_ac()` returns (PID detected), but the customer is still staring at a loading screen. AC's DirectX initialization, shader compilation, and asset loading can take 5-30 seconds depending on track/car complexity and CSP features. On cold starts (first launch after boot), shader caching alone can add 15-30 seconds. The customer pays for time spent watching a progress bar.

**Why it happens:**
The current code in `ac_launcher.rs` returns a `LaunchResult` with a PID as soon as `acs.exe` appears in the process list (`wait_for_ac_process(15)`). Process existence is not the same as "game is playable." AC creates shared memory files (`acpmf_graphics`) only after DirectX init completes, and the `STATUS` field transitions from `OFF(0)` to `LIVE(2)` only when the player is on track.

**Consequences:**
- Customer charged for 15-30 seconds of loading on every session
- At 30-min sessions (Rs 700), that is ~3-5% overbilling per session
- Customers notice and complain, especially on complex tracks (Nordschleife, Spa with rain/CSP weather)
- Billing discrepancy amplified in multiplayer where join handshake adds latency

**Prevention:**
Use the AC shared memory `acpmf_graphics` STATUS field as the billing trigger, not process existence. The state machine should be:

1. `launch_ac()` returns PID -- state: LAUNCHING
2. Shared memory `acpmf_graphics` opens successfully -- state: LOADING
3. `STATUS` field reads `2` (LIVE) -- state: DRIVING -- **start billing here**
4. `STATUS` reads `0` (OFF) or process exits -- state: ENDED -- stop billing

Poll the STATUS field every 500ms after PID is detected. Add a safety timeout (90 seconds) -- if STATUS never reaches LIVE, mark as a launch failure and do NOT bill.

**Detection (warning signs):**
- Billing start timestamps consistently precede first telemetry packet by 10+ seconds
- Customer complaints about "charged before I could drive"
- Shared memory STATUS reads 0 or 1 during the first 10-30 seconds after PID appears

**Phase to address:** Phase 1 -- core billing sync mechanism

---

### Pitfall 2: Shared Memory Stale Data After Crash or Unclean Exit

**What goes wrong:**
When AC crashes (acs.exe killed by taskkill, WerFault dialog, or GPU driver crash), the shared memory mapped files (`acpmf_physics`, `acpmf_graphics`, `acpmf_static`) remain in memory with their last-written values. The memory-mapped sections are OS-level kernel objects that persist until all handles are closed. If rc-agent still has open handles via `OpenFileMappingW`, the data stays readable but frozen at the crash-time values. The telemetry reader sees "valid" data: speed 230 km/h, throttle 1.0, lap count 15 -- all from the moment of the crash. The driving detector thinks the customer is still driving. Billing continues.

**Why it happens:**
Windows memory-mapped files persist as long as any process holds an open handle. The current `AssettoCorsaAdapter` in `sims/assetto_corsa.rs` opens handles in `connect()` and only closes them in `disconnect()`. If the game crashes without a clean `disconnect()` call, the adapter reads frozen data indefinitely.

**Consequences:**
- Billing continues after game crash (customer charged for staring at WerFault dialog)
- Driving detector reports `Active` because frozen telemetry shows non-zero speed/throttle
- Lap data corruption: if `completedLaps` counter was mid-increment at crash time, a phantom lap may be recorded
- The `initial_laps` snapshot taken in `connect()` only protects against stale data from a PREVIOUS session's memory, not from a crash DURING the current session

**Prevention:**
Three-layer detection:

1. **Process liveness check:** Poll `is_process_alive(pid)` every 2 seconds alongside telemetry reads. If acs.exe PID is dead but shared memory still reads data, immediately flag stale and stop billing.
2. **Heartbeat validation:** Check `acpmf_graphics` `packetId` field (offset 0). This counter increments with every graphics update (~10Hz). If `packetId` stops incrementing for 3 consecutive reads (3 seconds), the data is stale regardless of PID status.
3. **STATUS field monitoring:** If `STATUS` drops from `LIVE(2)` to `OFF(0)`, the session has ended even if the process is still technically alive (e.g., stuck in shutdown).

**Detection (warning signs):**
- Telemetry stream shows identical `packetId` values for consecutive reads
- Driving state remains `Active` after game process is gone from tasklist
- Billing session outlasts the actual gameplay session by minutes

**Phase to address:** Phase 1 -- billing sync; Phase 2 -- telemetry reliability

---

### Pitfall 3: Content Manager `acmanager://` URI Silently Fails with No Error

**What goes wrong:**
The `acmanager://race/online?ip=...&httpPort=...` URI opens Content Manager, but CM shows a GUI dialog (error popup, booking screen, "Cannot connect" message) instead of launching the race. No stdout/stderr is produced. The rc-agent code in `launch_via_cm()` uses `cmd /c start "" <uri>` which returns immediately with exit code 0 regardless of what CM does. The `wait_for_ac_process(15)` times out, the fallback to direct `acs.exe` fires, but direct launch with `[REMOTE] ACTIVE=1` in race.ini may also fail because the server handshake requires CM's join protocol.

**Why it happens:**
Content Manager's `acmanager://` protocol handler is a Windows URI scheme registered by CM during installation. The `start` command opens the URI handler and returns -- it does not wait for CM to finish its work. CM's error handling is GUI-only (modal dialogs) because it is designed as an interactive desktop app, not a headless automation target. Common CM failure modes:
- "Settings are not specified" -- CM's internal Quick Drive preset was never configured on this pod
- "Server is not available" -- acServer.exe is not running or port mismatch
- "Booking is not available" -- server in booking mode but no slot reserved for this GUID
- "Request Cannot be processed" -- CM crashed internally, WerFault spawned

**Consequences:**
- Multiplayer launch fails silently; customer sees CM error dialog they cannot dismiss (kiosk mode blocks Alt+F4)
- Fallback to direct acs.exe may put the client in a broken multiplayer state (connected but stuck in lobby)
- rc-agent reports launch "success" because the PID appeared (acs.exe ran briefly then exited)
- Staff sees the pod as "in session" when the customer is stuck

**Prevention:**
1. **Prefer direct `acs.exe` launch for all modes** when possible. For single-player, this already works (the code does it). For multiplayer, write the `[REMOTE]` section in `race.ini` with correct server IP/port/password -- AC's native multiplayer join via race.ini works without CM for pickup-mode servers.
2. **If CM is required** (e.g., for booking-mode servers), add post-launch validation: after `wait_for_ac_process()`, verify that the STATUS field in shared memory reaches LIVE within 30 seconds. If not, kill CM, kill acs.exe, log the CM diagnostics (the `diagnose_cm_failure()` function already exists), and report failure to the customer.
3. **Kill CM error dialogs proactively:** After a failed CM launch, check for `WerFault.exe` and CM processes with active windows, kill them, and show a "Launch failed, retrying" message on the lock screen.

**Detection (warning signs):**
- CM process is alive but acs.exe never appeared after 15 seconds
- `diagnose_cm_failure()` returns "CM process alive but acs.exe not spawned"
- Customer reports seeing a Content Manager window instead of the game

**Phase to address:** Phase 2 -- multiplayer launch; fallback logic already partially exists

---

### Pitfall 4: Server Config / Client Config Mismatch Prevents Multiplayer Join

**What goes wrong:**
The AC dedicated server (`acServer.exe`) on .51 has its own `server_cfg.ini` and `entry_list.ini`. The client's `race.ini` must specify a car that exists in the server's entry list, with EXACT name matching (case-sensitive). If rc-agent writes `race.ini` with `MODEL=ks_ferrari_488_gt3` but the server's entry list has `MODEL=ks_ferrari_488_GT3`, the join fails with "no available slots." The customer sees a loading screen that goes nowhere.

**Why it happens:**
Three independent config sources must agree: (1) server `server_cfg.ini` CARS= list, (2) server `entry_list.ini` [CAR_N] MODEL= entries, (3) client `race.ini` [RACE] MODEL= and [CAR_0] MODEL=. Any mismatch -- typo, case difference, missing car -- means the server rejects the client silently. AC does not provide a helpful error message; it just shows "no available slots" or hangs at the join screen.

Additionally:
- `entry_list.ini` must have at least `MAX_CLIENTS` car entries
- PICKUP_MODE must be enabled (PICKUP_MODE_ENABLED=1) for the venue's open-join model
- If the server uses GUID-locked entries (booking mode), each pod needs a registered Steam GUID

**Consequences:**
- Multiplayer session launch fails for one or more pods
- Customer waits at loading screen, billing may have started
- Difficult to debug because the failure is silent -- no error logs on the client side
- If only some cars are misconfigured, some pods join while others fail, causing confusion

**Prevention:**
1. **Single source of truth:** Build the server config and client race.ini from the same car/track catalog in rc-core. Never manually edit server config files. The AC catalog (36 tracks, 325 cars) in the existing custom experience booking should be the authoritative list.
2. **Validation at session creation time:** Before writing race.ini, verify the chosen car exists in the server's entry list by querying the AC server's HTTP API (`GET /INFO` on the server's HTTP port returns current config including car list).
3. **Case-sensitive matching:** Store car folder names exactly as they appear on disk. AC is case-sensitive on the server side (Linux-style matching even on Windows).
4. **Use PICKUP_MODE for the venue:** PICKUP_MODE_ENABLED=1 with enough entry_list slots for 8 pods. Do not use booking mode -- it requires GUID reservation which adds complexity and a point of failure.

**Detection (warning signs):**
- Pod shows "Launching" but shared memory STATUS never reaches LIVE
- Server logs show "car is illegal" or "entry list" errors
- Some pods join the server while others fail on the same session

**Phase to address:** Phase 2 -- multiplayer orchestration

---

### Pitfall 5: FFB Safety Gap Between Game Kill and Wheelbase Zero

**What goes wrong:**
When a session ends, the current code in `cleanup_after_session()` kills acs.exe FIRST, then (in a separate flow) calls `ffb_controller.zero_force()`. Between the kill and the zero command, there is a window (100ms-2s) where the OpenFFBoard wheelbase may hold its last FFB position. If the game was generating strong corner force at the moment of kill, the wheel can snap to center violently or hold a sustained torque until the zero command arrives. With the Conspit Ares 8Nm wheelbase, this is enough force to injure a customer's wrist or fingers.

**Why it happens:**
DirectInput FFB effects are destroyed when the game process exits, but the OpenFFBoard firmware may hold the last commanded torque for a brief period before its internal watchdog zeros the motor. The firmware watchdog timeout varies by configuration (typically 500ms-2s). During this window, the wheel is uncontrolled.

The current `ac_launcher.rs::enforce_safe_state()` calls `ensure_conspit_link_running()` which checks if ConspitLink is alive, but does NOT zero the wheelbase torque. The `ffb_controller.rs::zero_force()` sends an estop command via the vendor HID interface -- this is independent of the game's DirectInput FFB path. The sequence matters: zero torque BEFORE killing the game, not after.

**Consequences:**
- Customer wrist/hand injury from sudden wheel snap (liability risk)
- Wheelbase motor holds torque during crash dialog (WerFault) -- customer cannot release wheel
- Repeated snaps damage the wheelbase belt/motor over time (shortened hardware lifespan)

**Prevention:**
Enforce a strict sequence in all session-end and error-recovery paths:
1. `ffb_controller.zero_force()` -- zero the motor via OpenFFBoard vendor HID
2. Wait 200ms for the command to take effect
3. Kill acs.exe
4. Kill Content Manager
5. Verify the game process is gone

This sequence already exists partially (commit 93b9b59 added FFB zero on session end and startup). Verify it is applied consistently in ALL exit paths: normal session end, billing expiry, manual stop, game crash detection, and `enforce_safe_state()`.

**Detection (warning signs):**
- Customer reports wheel "jerking" or "snapping" when session ends
- `ffb_controller.zero_force()` returning `Ok(false)` (device not found) in logs -- means HID device was busy or ConspitLink has exclusive access
- Ordering in logs: "Killed AC" appears BEFORE "FFB: emergency stop sent"

**Phase to address:** Phase 1 -- must be correct from day one (safety-critical)

---

### Pitfall 6: assists.ini and race.ini Desynchronization

**What goes wrong:**
AC reads assist settings from multiple files: `race.ini` [ASSISTS] section, `assists.ini`, and Content Manager's internal settings cache. When rc-agent writes `race.ini` with `DAMAGE=0` (safety enforcement), but an older `assists.ini` has `DAMAGE=100`, or CM's cached settings override both, the in-game damage setting may not be what was intended. A customer crashes at 200 km/h and the car model visually shatters, or worse, the physics model applies damage that ends their session early.

**Why it happens:**
AC's config loading priority is undocumented and inconsistent between versions and CSP patches:
- Base AC reads `race.ini` [ASSISTS] at session start
- CSP may override from `assists.ini` if it exists
- Content Manager writes its own cached values to `assists.ini` when launching via `acmanager://`
- The `set_transmission()` function in `ac_launcher.rs` already accounts for this by writing BOTH files, but other assist values (DAMAGE, STABILITY, ABS, TC) are only written to assists.ini in `write_assists_ini()`

The existing code already writes both `race.ini` and `assists.ini` (good), but if a third party (CSP, CM, or a manual edit) modifies either file after rc-agent writes them and before AC reads them, the safety settings can be overridden.

**Consequences:**
- DAMAGE=0 (safety) overridden by stale assists.ini -- customer car takes physics damage
- STABILITY=0 overridden -- novice customer spins uncontrollably (safety issue on high-power cars)
- Traction control / ABS settings not what customer selected -- poor experience

**Prevention:**
1. **Write BOTH files atomically:** The current `write_race_ini()` + `write_assists_ini()` pattern is correct. Keep it.
2. **Write AFTER killing the old AC process:** The current sequence (kill AC, wait 2s, write configs, launch) is correct. Verify no race condition exists where AC reads config before rc-agent finishes writing.
3. **Post-launch validation:** After shared memory STATUS reaches LIVE, read the current assist values from shared memory or from the config files to verify DAMAGE=0 was applied. Log a warning if mismatch detected.
4. **Never use CM for single-player launches:** CM may overwrite assists.ini with its own cached values. The code already avoids CM for single-player (good). Keep this pattern.
5. **Lock the files:** Consider setting `assists.ini` and `race.ini` as read-only after writing, then removing read-only before next write. This prevents CM or CSP from modifying them mid-session.

**Detection (warning signs):**
- In-game damage visible on customer's car despite DAMAGE=0 in race.ini
- Log shows assists.ini DAMAGE value differs from what was written
- Customer reports assist settings feel "wrong" compared to what they selected

**Phase to address:** Phase 1 -- safety preset enforcement

---

## Moderate Pitfalls

Issues that degrade experience or complicate operations but do not cause safety incidents or billing errors.

---

### Pitfall 7: ConspitLink Crash Leaves Wheelbase Uninitialized

**What goes wrong:**
ConspitLink (the Conspit Ares management app) crashes or is killed during session. The next AC launch finds the wheelbase in an uninitialized state -- FFB may not work, or the wheel may have no center spring. The customer turns the wheel and gets no resistance, or the wheel drifts to one side. The `ensure_conspit_link_running()` watchdog in `ac_launcher.rs` restarts ConspitLink, but the restart takes 3-5 seconds and the game may have already initialized DirectInput FFB with a "no device" error.

**Prevention:**
1. Ensure ConspitLink is running and healthy BEFORE launching AC (the current code does kill+launch+wait+minimize). Add a check: after ConspitLink restarts, wait until the OpenFFBoard HID device appears in `hidapi::HidApi::new().device_list()` before launching the game.
2. If FFB fails mid-session (customer reports "no feedback"), have a "restart FFB" button in the PWA that restarts ConspitLink and sends a `set_ffb()` command without restarting the game.

**Detection:** Customer reports "wheel feels dead" or "no resistance." `ffb_controller.zero_force()` returns `Ok(false)` (device not found) during session.

**Phase to address:** Phase 3 -- mid-session controls

---

### Pitfall 8: AC Server Port Conflicts on Restart

**What goes wrong:**
The AC dedicated server on .51 uses three ports: UDP (default 9456), TCP (default 9457), and HTTP (default 8098). When the server is restarted for a config change (new track/car list for a multiplayer session), the old process may not release ports immediately. The new acServer.exe fails to bind and exits silently. All multiplayer joins fail until the port is free.

**Prevention:**
1. Kill acServer.exe, wait 3 seconds for port release, THEN start the new instance.
2. Use `netstat` to verify the port is free before starting.
3. Alternatively, keep the server running and use the server's HTTP API to rotate tracks/cars without restarting (if using AssettoServer or CM Server Manager wrapper that supports hot reload).

**Detection:** Server process starts but clients cannot connect. HTTP port query (`GET /INFO`) returns connection refused. Server log shows "address already in use."

**Phase to address:** Phase 2 -- multiplayer server management

---

### Pitfall 9: Track/Car Content Not Installed on All Pods

**What goes wrong:**
The AC content catalog (36 tracks, 325 cars) assumes all content is installed identically on all 8 pods. If one pod is missing a DLC track or a mod car, the game crashes silently on launch (no error dialog with FORCE_START=1, just immediate exit). The customer's session fails but other pods work fine, making it hard to diagnose.

**Prevention:**
1. **Content inventory scan at startup:** On rc-agent startup, scan the AC installation directory for installed cars (`content/cars/*/`) and tracks (`content/tracks/*/`). Report the list to rc-core. Compare across all 8 pods.
2. **Filter unavailable content per pod:** When generating car/track options for the PWA/kiosk, intersect the pod's installed content with the catalog. Never offer a car/track that is not installed on the target pod.
3. **Use the same disk image or deploy script** for all 8 pods to ensure content parity.

**Detection:** Single pod fails to launch a track/car that works on all other pods. `acs.exe` exits with code 0 within 2 seconds of launch (too fast for normal loading). No shared memory files appear.

**Phase to address:** Phase 1 -- content validation

---

### Pitfall 10: DirectX Initialization Failure on First Launch After Boot

**What goes wrong:**
The first AC launch after a Windows boot (cold start) takes significantly longer because the DirectX shader cache is empty. AC (and CSP especially) compiles hundreds of shaders on first load. This can take 30-60 seconds on complex tracks with CSP weather effects. If the billing timeout or health check timeout is shorter than this, the system may declare a launch failure and kill the game before it finishes loading.

Additionally, some GPU driver states (especially after Windows Update or driver update) cause DirectX initialization to fail entirely on first attempt but succeed on retry.

**Prevention:**
1. **Generous timeout for first launch:** Use a 90-second timeout for shared memory STATUS to reach LIVE, not 15 seconds. The current `wait_for_ac_process(15)` only waits for the process, not for DirectX init.
2. **Warm-up launch at boot time:** Automatically launch AC with a lightweight track (e.g., `magione` or `ks_vallelunga`) for 10 seconds at pod boot to prime the shader cache, then kill it. This ensures subsequent customer launches are fast.
3. **Retry on DirectX failure:** If acs.exe exits within 5 seconds of launch (abnormal exit), automatically retry once with a 5-second delay. Many DirectX failures are transient.

**Detection:** `acs.exe` exits with non-zero exit code within 5 seconds. Shared memory files never appear. `diagnose_cm_failure()` finds WerFault.exe or a DirectX error in CM logs.

**Phase to address:** Phase 1 -- launch reliability

---

### Pitfall 11: Multiplayer Session Timing Across Pods

**What goes wrong:**
In a multi-pod multiplayer session, different pods launch at slightly different times (network latency in the launch command, varying DirectX init times). Pod 1 might be on track and driving 20 seconds before Pod 8 finishes loading. If billing starts per-pod when each pod's STATUS reaches LIVE, customers pay different amounts for the same "shared session." If billing starts globally when the first pod is ready, early pods wait while late pods load.

**Prevention:**
1. **Launch all pods, wait for all to reach LIVE, then signal "session active":** rc-core orchestrates the multiplayer launch. Each pod reports its STATUS transition. Billing for all pods in a multiplayer session starts when the LAST pod reaches LIVE (or after a 60-second timeout, whichever comes first). Pods that loaded early see the track but billing has not started yet.
2. **Alternatively, use AC's session timer:** In practice mode, AC has its own session timer (`DURATION_MINUTES`). Sync billing to the server's session timer rather than individual pod detection. The server's session starts when acServer.exe begins the practice phase, which is the same for all connected clients.

**Detection:** Multiplayer session billing amounts differ across pods despite the "same" session. Customers compare receipts and notice discrepancy.

**Phase to address:** Phase 2 -- multiplayer orchestration

---

### Pitfall 12: Window Focus and Foreground Race Condition

**What goes wrong:**
After launching AC, rc-agent runs `minimize_background_windows()` and `bring_game_to_foreground()`. But AC creates its window asynchronously during DirectX init. If the foreground commands fire before AC's window exists, they do nothing. Then ConspitLink, Steam overlay, or Windows notification pops up ON TOP of the game, stealing focus. The customer cannot interact with AC because another window has keyboard/mouse focus.

The existing code has a hardcoded `sleep(8s)` + `sleep(2s)` delay to wait for AC to load, but this is unreliable -- some tracks load faster (2s), some slower (30s with CSP).

**Prevention:**
1. **Event-driven foreground management:** Instead of sleeping a fixed duration, poll for AC's window handle (`FindWindowW("Assetto Corsa")`) until it appears, then bring it to foreground.
2. **Re-focus on STATUS transition:** When shared memory STATUS reaches LIVE, immediately bring the AC window to foreground and minimize everything else. This is the moment the game is actually ready for input.
3. **Periodic focus enforcement during session:** Every 10 seconds, check if AC is the foreground window. If not (Steam overlay popup, Windows notification, ConspitLink), re-focus AC. Be careful not to fight with intentional overlays (the RC billing overlay runs in Edge, which IS in the allow list).

**Detection:** Customer reports "game behind another window" or "can't click on the game." AC window exists but is not the foreground process.

**Phase to address:** Phase 1 -- launch sequence polish

---

## Minor Pitfalls

Annoyances or edge cases that should be handled but are not session-breaking.

---

### Pitfall 13: Steam Overlay Interferes with Kiosk

**What goes wrong:**
AC is a Steam game. The Steam overlay (Shift+Tab) can be activated by customers, giving them access to the Steam store, browser, chat, and other non-game features. This breaks the kiosk security model.

**Prevention:** Disable Steam overlay per-game: In Steam library, right-click AC > Properties > uncheck "Enable the Steam overlay while in-game." Alternatively, launch acs.exe directly (bypassing Steam) -- the game runs without overlay when not launched through Steam. The current direct-launch path already does this for single-player.

**Phase to address:** Phase 1 -- pod hardening

---

### Pitfall 14: AC Replay Mode Triggers False "Active" Detection

**What goes wrong:**
If a customer watches an in-game replay, the shared memory STATUS reads `1` (REPLAY), not `2` (LIVE). The driving detector sees STATUS != LIVE and may interpret this as "not driving" and trigger idle detection, even though the customer is actively engaged (watching their replay).

**Prevention:** Treat STATUS=1 (REPLAY) as an "active" state for billing purposes. The customer chose to watch the replay -- they should not be timed out. Only STATUS=0 (OFF) and STATUS=3 (PAUSE for extended duration) should count as potentially idle.

**Phase to address:** Phase 2 -- billing edge cases

---

### Pitfall 15: Race.ini Track Config Field Causes Wrong Track Layout

**What goes wrong:**
Some AC tracks have multiple layouts (e.g., `ks_nurburgring` has `gp`, `sprint`, `endurance`, etc.). The `CONFIG_TRACK` field in race.ini selects the layout. If this field is empty when a track has multiple layouts, AC loads the default layout which may not match what the customer selected. If the field contains a non-existent layout name, AC may crash or load an empty track.

**Prevention:**
1. The AC catalog must store valid layout names alongside track IDs.
2. Validate the `track_config` parameter against the track's actual layout folders on disk (`content/tracks/<track>/<layout>/`).
3. For tracks with only one layout, leave CONFIG_TRACK empty.
4. For tracks with multiple layouts, REQUIRE the layout to be specified in the PWA/kiosk selection.

**Phase to address:** Phase 1 -- content validation and option filtering

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Billing sync to gameplay | Stale shared memory after crash (Pitfall 2) | Monitor packetId counter + process liveness; stop billing when either indicates stale |
| Billing sync to gameplay | Loading screen billing (Pitfall 1) | Use STATUS=LIVE as billing trigger, not PID existence |
| Safety preset enforcement | assists.ini desync (Pitfall 6) | Write both race.ini and assists.ini; post-launch verify DAMAGE=0 |
| Safety preset enforcement | FFB safety gap (Pitfall 5) | Zero torque BEFORE killing game, in ALL exit paths |
| Multiplayer orchestration | Server/client config mismatch (Pitfall 4) | Single source of truth for car/track names; validate against server HTTP API |
| Multiplayer orchestration | Session timing across pods (Pitfall 11) | Wait for all pods to reach LIVE before starting shared billing |
| Content Manager integration | Silent CM failure (Pitfall 3) | Prefer direct acs.exe launch; add post-launch STATUS validation |
| Launch reliability | DirectX cold start (Pitfall 10) | 90-second timeout; warm-up launch at boot; retry once on fast exit |
| Window management | Focus race condition (Pitfall 12) | Event-driven foreground on STATUS=LIVE, not fixed sleep |
| Mid-session controls | ConspitLink crash (Pitfall 7) | Verify HID device presence before launching game |

---

## "Looks Done But Isn't" Checklist

- [ ] **Launch "works":** acs.exe PID appears does NOT mean the game is playable. Must wait for shared memory STATUS=LIVE.
- [ ] **Billing "synced":** Starting billing on PID detection is NOT synced to gameplay. Must use STATUS=LIVE transition.
- [ ] **Safety "enforced":** Writing DAMAGE=0 to race.ini does NOT guarantee it takes effect. Must also write assists.ini and verify no override from CM/CSP.
- [ ] **Multiplayer "connected":** Client process running does NOT mean joined to server. Must verify via server HTTP API or shared memory multiplayer fields.
- [ ] **FFB "zeroed":** Killing the game does NOT zero the wheelbase. Must send explicit estop via OpenFFBoard vendor HID BEFORE killing the game.
- [ ] **Config "correct":** Car name in race.ini matching your catalog does NOT mean it matches the server entry_list. Must be case-sensitive exact match.
- [ ] **Content "available":** 325 cars in the catalog does NOT mean 325 cars on every pod. Must validate installed content per-pod at startup.
- [ ] **Window "visible":** Launching acs.exe does NOT mean it is the foreground window. Must explicitly bring to foreground after DirectX init completes.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Billing during loading (1) | LOW | Retroactive credit: calculate time between PID detection and STATUS=LIVE, subtract from billed amount |
| Stale shared memory (2) | LOW | Close all SHM handles, disconnect adapter, stop billing. Auto-detectable via packetId staleness |
| CM silent failure (3) | MEDIUM | Kill CM + acs.exe, show error on lock screen, offer retry. If persistent, fall back to single-player |
| Config mismatch (4) | MEDIUM | Fix server config to match catalog, restart server. Requires staff intervention |
| FFB safety gap (5) | HIGH | If customer injured: incident report, hardware inspection. Prevention is the only acceptable strategy |
| Assists desync (6) | LOW | Relaunch AC with correct configs. Verify post-launch. Quick recovery |
| ConspitLink crash (7) | LOW | Restart ConspitLink, wait for HID device, send set_ffb(). 5-second recovery |
| Port conflict (8) | LOW | Kill old server, wait 3s, restart. Verify ports free via netstat |
| Missing content (9) | MEDIUM | Install missing content on the pod. Requires physical access or remote deploy |
| DirectX cold start (10) | LOW | Retry launch automatically. If persistent, reboot pod |
| Multiplayer timing (11) | LOW | Adjust billing start to align with slowest pod or server session timer |
| Window focus (12) | LOW | Re-run foreground enforcement. Auto-recoverable in 10-second periodic check |

---

## Sources

- **Codebase inspection (HIGH):** `ac_launcher.rs` -- full launch sequence, race.ini writing, CM integration, foreground management. `sims/assetto_corsa.rs` -- shared memory telemetry, STATUS field, packetId. `ffb_controller.rs` -- OpenFFBoard estop, HID interface. `game_process.rs` -- PID management, orphan cleanup. `driving_detector.rs` -- hysteresis detection, HID input parsing.
- **Debugging playbook (HIGH):** `debugging-playbook.md` -- Session 0 issues, Edge stacking, file locks, zombie processes. Direct venue experience.
- **MEMORY.md (HIGH):** ConspitLink management ("Don't kill, just minimize"), FFB safety (commit 93b9b59), CSP gui.ini FORCE_START, RP_OPTIMAL preset.
- **[OpenFFBoard Games Setup Wiki](https://github.com/Ultrawipf/OpenFFBoard/wiki/Games-setup) (MEDIUM):** AC is "fully working" with OpenFFBoard. No game-specific config needed. DirectInput compatible.
- **[AC Dedicated Server Manual -- Kunos Forum](https://www.assettocorsa.net/forum/index.php?faq/assetto-corsa-dedicated-server-manual.28/) (MEDIUM):** server_cfg.ini, entry_list.ini, PICKUP_MODE, MAX_CLIENTS, port configuration.
- **[Elite Game Servers -- "Car is illegal" error](https://www.elitegameservers.net/clientarea/knowledgebase/100/Error-entry-list-CAR_-car-is-illegal.html) (MEDIUM):** Car name must match in both server_cfg.ini CARS= and entry_list.ini MODEL=.
- **[AC Server Pickup Mode -- Elite Game Servers](https://www.elitegameservers.net/clientarea/knowledgebase/35/How-to-set-server-in-non-booking-mode.html) (MEDIUM):** PICKUP_MODE_ENABLED=1, entry_list must have >= MAX_CLIENTS entries.
- **[Assetto Corsa Mods -- "No available slots"](https://assettocorsamods.net/threads/cars-mod-on-dedicated-cannot-join-server-no-available-slots.695/) (MEDIUM):** Case-sensitive car name matching causes slot mismatch.
- **[CM ArgumentsHandler source](https://github.com/gro-ove/actools/blob/master/AcManager/Tools/ArgumentsHandler.Commands.cs) (MEDIUM):** acmanager:// URI protocol handler. Shows race/online and race/config paths.
- **[Steam Community -- AC DirectX errors](https://steamcommunity.com/app/244210/discussions/0/648814841411662567/) (LOW):** DirectX init failures, xinput1_3.dll, GPU scheduling issues.
- **[OverTake.gg -- AC DirectX error](https://www.overtake.gg/threads/asetto-corsa-directx-error.246681/) (LOW):** Antivirus blocking, driver issues, CommonRedist reinstallation.
- **[Steam Community -- ACS.EXE Not Responding](https://steamcommunity.com/app/244210/discussions/0/3128289322263531427/) (LOW):** Game hangs on exit, requires taskkill -- relevant to cleanup_after_session.

---
*Pitfalls research for: AC Launcher -- Assetto Corsa Launch & Session Management*
*Researched: 2026-03-13*
