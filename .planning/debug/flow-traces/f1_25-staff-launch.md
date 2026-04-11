# F1 25 Staff-Launch Flow Trace

**Created:** 2026-04-11
**Scenario owner:** Racing Point venue, staff-initiated launches (no customer self-service)
**Method:** Flow-Trace Debugging (8 phases: MAP → BASELINE → INSTRUMENT → TRACE → FIX ONE → RETRACE → CODIFY → GUARD)
**Status:** Phase 1 draft. No code changes permitted until Phase 5.

---

## 1. Scenario (frozen before any work)

**Exact scenario under test — one sentence:**
> A staff member opens the kiosk staff page on server .23 (`http://192.168.31.23:3300/kiosk/staff`), selects Pod 4, picks a registered test driver and the 30-minute pricing tier, clicks "Launch F1 25", the customer then drives for 30 minutes, the session ends, and Pod 4 returns to its idle state — all without any PIN entry by the customer.

**Scope boundaries:**
- One game: **F1 25** (SimType::F125, Steam App ID 3059520)
- One pod: **Pod 4** (LAN `192.168.31.88`, Tailscale `100.75.45.10`)
- One staff interface: **kiosk staff** at `:3300/kiosk/staff` — NOT POS billing, NOT admin dashboard
- One auth mode: **staff-initiated** — billing starts directly, no `create_auth_token` / no customer PIN
- One session shape: **30-minute single split** (no sub-sessions, no multiplayer)

**What this scenario does NOT cover:**
- Multiplayer / AC server launches
- Customer self-service PIN/QR flow (venue not using — `customer_self_service_mode = false`)
- Between-session continuation, refunds, disputes
- Other games (iRacing, LMU, ACE, ACR, FH5) — adapt via §7

---

## 2. Prerequisites (must be true before trace starts)

| # | Prereq | Verify |
|---|---|---|
| P1 | Pod 4 powered on + rc-agent reachable | `curl http://192.168.31.88:8090/health` → `status: ok` |
| P2 | Pod 4 on current binary `b1fc9484` (IDLE-01 + LAUNCH-FIX-3/4) | `/health` → `build_id: b1fc9484` |
| P3 | Pod 4 rc-agent is in Session 1 (Console), not Session 0 | `tasklist /V /FI "IMAGENAME eq rc-agent.exe" /FO CSV` — `Session#` = 1 |
| P4 | Server racecontrol reachable | `curl http://192.168.31.23:8080/api/v1/health` → 200 |
| P5 | Kiosk staff frontend reachable | `curl http://192.168.31.23:3300/kiosk/staff` → HTML |
| P6 | No active billing session on Pod 4 | `/fleet/health` pod_4 row → no `game_state` field, no `active_sentinels` showing a session |
| P7 | No `MAINTENANCE_MODE` / `OTA_DEPLOYING` sentinels on Pod 4 | `/exec` `dir C:\RacingPoint\MAINTENANCE_MODE` → not found |
| P8 | No orphan F1 25 / Steam process from a previous run | tasklist filter — no `F1_25.exe`, Steam state documented |
| P9 | F1 25 actually installed on Pod 4 (Steam appmanifest) | `/exec` `dir "C:\Program Files (x86)\Steam\steamapps\appmanifest_3059520.acf"` — exists |
| P10 | Pod 4 has F1 25 in its `rc-agent.toml` games registry | `/exec` `type C:\RacingPoint\rc-agent.toml` + grep `f1_25` |
| P11 | Steam is logged in on Pod 4 (offline mode OK), no unresolved dialog | `tasklist /V` — `steam.exe` with clean window title, not "Steam Login" |
| P12 | Test driver `3c6e7dc2` exists + has ≥ ₹900 wallet balance | `curl /api/v1/drivers/3c6e7dc2` → balance_paise ≥ 90000 |
| P13 | Staff JWT available in the browser session (user logged into kiosk staff) | Browser DevTools → localStorage `staff_jwt` present |
| P14 | ConspitLink running (wheelbase needed for FFB) | tasklist — `ConspitLink2.0.exe` ≥ 1 instance; ideally exactly 1 (no leak) |
| P15 | Wheelbase (Conspit Ares) reachable | `VSD Craft.exe` running or `/health` shows FFB state OK |

**If any prereq fails:** stop and fix the prereq first. Do not start the trace with a broken baseline.

---

## 3. Known bugs overlay (as of 2026-04-11)

State of the system at the moment this MAP was drafted, so future trace runs can reason about what's known vs. new:

| Tag | Description | Status | Relevant hops |
|---|---|---|---|
| **IDLE-01** | Pod idle state was showing empty customer PIN pad (`show_idle_pin_entry`) in a venue that doesn't use customer self-service | **Fix deployed** `b1fc9484` — `customer_self_service_mode=false` routes idle to `show_blank_screen`. Pod 4 eyeball-verified; Pods 1-3,5-8 endpoint-verified only. | Hop 0, Hop 50-51 |
| **LAUNCH-FIX-3** | Non-AC launch branch (Steam URL path) never called `close_browser()` after game confirmed, leaving splash up forever | **Fix deployed** `66b87bda` — mpsc channel in event_loop. **NOT verified** against a real F1 25 launch (blocked by Bug 2 below). | Hops 33-35 |
| **LAUNCH-FIX-4** | `show_native_window()` after `SW_HIDE` only called `request_repaint()` — repaint on hidden window is a no-op, state said blanked but screen showed desktop | **Fix deployed** `1136fd1a` — now calls `request_show()` (`SW_SHOW + SetForegroundWindow`) before repaint | Hops 14, 17, 25, 50 |
| **Bug 2** (tentative) | F1 25 `steam://rungameid/3059520` fires, Steam activates (PID 9068, 8 helper processes), but `F1_25.exe` never appears in tasklist. Server's `game_state` was falsely set to `running` before Bug 1 deploy restart cleared it. | **Unknown root cause.** This MAP is the tool to find it. | Hops 28-33 |
| **debug_server hardcoded** | `/debug` endpoint at [debug_server.rs:157](../../../crates/rc-agent/src/debug_server.rs#L157) hardcodes `native_window_alive = 1` — the probe lies | Outstanding observability gap. Affects how much we trust `/debug` output in this trace. | Any hop using `/debug` |
| **ConspitLink leak** | 3-4 instances of `ConspitLink2.0.exe` accumulating on Pod 4 (should be 1), ~730 MB wasted | Outstanding, not launch-blocking | Prereq P14 |
| **Adapter "launch_verifier" dead code** | 5-stage launch verifier in [launch_verifier.rs](../../../crates/rc-agent/src/launch_verifier.rs) exists + has tests but is never called from production | Outstanding — production launch path uses a simpler `is_running()` polling approach | Hops 26-32 |
| **"game_state: running" with no process** | Server's `LaunchStateMachine` (Phase 368) reported Pod 4 `game_state: running` while the game process didn't exist. Unclear if this is a server state-machine bug or a delayed-write from a previous session | Unknown — part of Bug 2 investigation | Hops 22-33 |

**Rule:** when a trace run hits a divergence at a hop with a known bug, note the overlap. If the known bug explains the divergence fully, we fix that one. If there's a second divergence, that's a new bug.

---

## 4. Phase 1 — MAP: the 51 hops

**Legend:**
- **Actor/Layer:** who or what performs the action
- **Expected observable:** the exact value a probe should return if the hop succeeded
- **Probe:** the command or inspection to run to verify
- **Known risk:** bug tag from §3 or a historical risk documented elsewhere

| # | Actor / Layer | Action | Expected observable | Probe | Known risk |
|---|---|---|---|---|---|
| **0** | Pod 4 / rc-agent | Idle state before trace starts | `lock_screen_state=screen_blanked`, `native_window_alive=1`, Racing Point blank animation visible on 3 monitors | `curl http://192.168.31.88:18924/debug` + user eye | IDLE-01 (fixed, post-fix default) |
| **1** | Staff / browser | Opens `http://192.168.31.23:3300/kiosk/staff` in a Chromium-based browser | HTTP 200, page HTML contains the staff grid, no auth redirect (if logged in) | Browser DevTools Network tab + HAR capture | Middleware auth gates — memory warns against blocking login page |
| **2** | Kiosk staff page / Next.js | Page mounts, `useKioskSocket` hook connects to server WS at `ws://192.168.31.23:8080/ws` | Browser console: WS connected, no retry storm | DevTools Console + Network WS frame | Stale JS build → WS churn storm (memory: dashboard_ws_churn) |
| **3** | Kiosk page → Server | Page fetches `/api/v1/pods` or similar fleet list | HTTP 200, JSON with all 8 pods | DevTools Network | — |
| **4** | Staff / browser | Clicks Pod 4 tile | Staff UI opens the pod-details panel for Pod 4 | Screen observation + DevTools React state | — |
| **5** | Staff / browser | Selects test driver `3c6e7dc2` from driver picker | Driver name appears in wizard state, wallet balance shown | Screen + DevTools | Driver must be registered + waiver signed |
| **6** | Staff / browser | Selects 30-minute pricing tier (`₹700` or `₹900` per memory) | Tier + price shown in wizard summary | Screen + DevTools | Pricing tier IDs must match server `/pricing` |
| **7** | Staff / browser | Opens game picker (non-AC direct-launch panel) | Game picker shows F1 25 if installed on Pod 4 (per Phase 361 preset filtering) | Screen + `/api/v1/pods/4/inventory` | Phase 361 filter: pod inventory must list f1_25 |
| **8** | Staff / browser | Selects F1 25 | F1 25 highlighted in picker | Screen | — |
| **9** | Staff / browser | Clicks "Launch" button | `handleGameLaunch("f1_25", ...)` fires at [kiosk/src/app/staff/page.tsx:194](../../../kiosk/src/app/staff/page.tsx#L194) | DevTools Network tab — see next two API calls queue up | — |
| **10** | Kiosk page → Server | `POST http://192.168.31.23:8080/api/v1/billing/start` with `{pod_id: "pod_4", driver_id: "3c6e7dc2", pricing_tier_id, staff_id}` | HTTP 200, `{ok: true, billing_session_id: "<uuid>", nonce: "..."}` | DevTools Network → Response body + server access log | FSM-03 rejects if pod already has active billing |
| **11** | Server / billing_fsm | Validates: pod exists, driver wallet ≥ tier price, no existing billing | Server log: `Billing session started for pod_4 driver=3c6e7dc2 tier=...` | `tail racecontrol.jsonl \| grep billing` on server .23 | — |
| **12** | Server / racecontrol | Persists billing row to DB (`billing_sessions` table) | Row exists with `status='active'`, `started_at=now` | `sqlite3 C:/RacingPoint/racecontrol.db "SELECT ..."` | Cloud sync: venue = authoritative for billing |
| **13** | Server → Pod 4 | Sends WS message `CoreToAgentMessage::BillingStarted { billing_session_id, driver_name, allocated_seconds: 1800, ... }` | Server ws_senders log: `sent BillingStarted to pod_4` | Server log grep | DEPLOY-05 dedup by command_id — replays silently acked |
| **14** | Pod 4 / rc-agent ws_handler | Receives BillingStarted at [ws_handler.rs:222](../../../crates/rc-agent/src/ws_handler.rs#L222) | Pod log: `Billing started: <id> for <name> (1800s)` | `findstr "Billing started" C:\RacingPoint\rc-agent-.2026-04-11.jsonl` via `/exec` | — |
| **15** | Pod 4 / rc-agent | Runs pre_flight::run (PF-01) — FFB, disk, Steam state checks | Pod log: `Pre-flight passed, proceeding with session` | Log grep | PF returns MaintenanceRequired → show_maintenance_required, billing aborts |
| **16** | Pod 4 / rc-agent | Sets `billing_active=true` on heartbeat_status + remote_ops::BILLING_ACTIVE + failure_monitor | `/debug` → billing_active flag | `/debug` or memory inspection | MMA-Iter2: both flags must set |
| **17** | Pod 4 / rc-agent | Calls `state.lock_screen.show_active_session(driver, 1800, 1800)` → `hide_native_window()` | `lock_screen_state` transitions Hidden; `native_window` hidden via `nw.hide()` | `curl :18924/debug` → `lock_screen_state=hidden` (or active_session) | LAUNCH-FIX-4: SW_HIDE'd window must re-show cleanly on next state change |
| **18** | Pod 4 / rc-agent | Activates overlay HUD — `state.overlay.activate(driver, 1800)` | Overlay HUD visible on pod display (time remaining, driver name) | User eye + `/debug` overlay state | Overlay vs game z-order — game must take foreground |
| **19** | Pod 4 / rc-agent | Spawns `ac_launcher::minimize_background_windows()` in blocking task | Background apps (Explorer, Discord, etc) minimized | `tasklist /V` — check foreground process | Memory: can disrupt NVIDIA Surround if explorer restarted |
| **20** | Pod 4 / rc-agent | Sends `AgentMessage::LaunchTimelineReport` back to server (LAUNCH-05) | Server dashboard receives event, timeline begins | Server dashboard / timeline API | Fire-and-forget — silent failure on channel full |
| **21** | Kiosk page / Next.js | Staff page proceeds to `api.launchGame("pod_4", "f1_25", launch_args)` → `POST /api/v1/games/launch` | HTTP 200, `{ok: true}` | DevTools Network Response | Concurrent session guard (409) |
| **22** | Server / racecontrol | Validates: pod has game in inventory, pod has active billing, feature flag on, not already launching | Server log: `LaunchGame accepted for pod_4 sim=F125` | Server log grep | `game_launch` flag off → silent 200 with error body |
| **23** | Server / racecontrol | Mints `launch_id` UUID v4 for Phase 318 LAUNCH-05 timeline | Server log includes launch_id | Log | Split-deploy fallback if rc-agent on old build |
| **24** | Server → Pod 4 | Sends WS `CoreToAgentMessage::LaunchGame { sim_type: F125, launch_args, force_clean: false, duration_minutes: 30, launch_id }` | Server ws log: `sent LaunchGame to pod_4 sim=F125 launch_id=<uuid>` | Log grep | Protocol version mismatch if rc-agent on a binary that can't deserialize new fields |
| **25** | Pod 4 / rc-agent ws_handler | Receives LaunchGame at [ws_handler.rs:449](../../../crates/rc-agent/src/ws_handler.rs#L449) — writes to termination log file for crash forensics | Pod log: `LaunchGame received: sim=F125 force_clean=false ts=...` | `findstr "LaunchGame received" rc-agent-.2026-04-11.jsonl` | SEC-10 mutex serializes concurrent launches |
| **26** | Pod 4 / rc-agent | Acquires SEC-10 mutex, checks `game_launch` feature flag | Mutex acquired, flag = true | Log grep for flag check | Flag off → silent ignore |
| **27** | Pod 4 / rc-agent | Runs `pre_launch_checks()` at [game_process.rs:73](../../../crates/rc-agent/src/game_process.rs#L73) in `spawn_blocking` | Log: `LAUNCH-FIX-3: no orphan processes` + `disk ok` | Log grep | Orphan F1_25.exe or F1_2025.exe → auto-cleanup attempt; permanent orphan → launch fails |
| **28** | Pod 4 / rc-agent | Sets `current_sim_type = F125` on event_loop state | State visible via memory or `/debug` | `/debug` extension | — |
| **29** | Pod 4 / rc-agent | Non-AC branch at [ws_handler.rs:1009](../../../crates/rc-agent/src/ws_handler.rs#L1009) calls `state.lock_screen.show_launch_splash(driver_name)` | `lock_screen_state=launch_splash`, native window re-shown with "Preparing your session..." text | `/debug` within ~500ms of LaunchGame receipt | LAUNCH-FIX-4: window must re-show via SW_SHOW, not just repaint |
| **30** | Pod 4 / rc-agent | Calls `GameProcess::launch(&config, SimType::F125)` at [game_process.rs:284](../../../crates/rc-agent/src/game_process.rs#L284) | Returns `Ok(GameProcess { state: Launching, child: None, pid: None })` | Log: `Launching via URL scheme: steam://rungameid/3059520` | `config.use_steam` must be true, `steam_app_id = 3059520` from TOML |
| **31** | Pod 4 / Win32 | Dispatches `cmd /C start "" steam://rungameid/3059520` via `spawn_safe` | cmd.exe child exits 0 (Steam URI handler registered) | Log for spawn_safe result | URI handler not registered = silent failure |
| **32** | Pod 4 / Windows OS | Windows routes `steam://` URI to `%ProgramFiles(x86)%/Steam/Steam.exe` | Steam brought to foreground OR spawned if not running | `tasklist /FI "IMAGENAME eq steam.exe"` + window title | **Bug 2 suspect layer** |
| **33** | Pod 4 / Steam | Steam resolves `rungameid/3059520`, checks install, readiness, auth | Steam's own loading spinner / library UI on screen | User eye | **Bug 2 suspect layer** — Steam login dialog, offline mode, missing install, corrupted manifest, EA account not linked |
| **34** | Pod 4 / Steam | Launches F1 25 via EA Javelin kernel anti-cheat initialization | EA anti-cheat services start (`eaanticheat.gameservicelauncher.exe`, `eadesktop.exe`, `ealink.exe`) | `tasklist` filter for EA processes | Anti-cheat allowlist must include these 22 entries (per memory commit `693214bb`) |
| **35** | Pod 4 / F1 25 | `F1_25.exe` (or `F1_2025.exe`) process spawns | Process appears in `tasklist /FI "IMAGENAME eq F1_25.exe"` within 30s of Step 31 | tasklist loop every 2s | **Bug 2 primary failure point** — process never appears |
| **36** | Pod 4 / rc-agent | Spawned GAME-07 tokio task (introduced in LAUNCH-FIX-3) polls for game window with 60s timeout | Task log: `GAME-07 waiting for F125 window` → `detected` OR `timeout` | rc-agent log grep | 60s timeout with no detection → splash stays up |
| **37** | Pod 4 / rc-agent | GAME-07 task detects F1 25 window via `find_game_pid(SimType::F125)` matching `F1_25.exe`/`F1_2025.exe` | Task returns pid; log: `LAUNCH-FIX-3: lock_screen_hide signal sent for F125 (GAME-07 path, pid=<N>)` | Log grep | LAUNCH-FIX-3 NEVER VERIFIED against a real launch |
| **38** | Pod 4 / rc-agent | Task sends `()` on mpsc channel `lock_screen_hide_tx` | Channel recv in event_loop select arm | Log grep for: `LAUNCH-FIX-3: Lock screen hidden via GAME-07 async signal` | Channel full or closed → silent miss |
| **39** | Pod 4 / rc-agent event_loop | Select arm receives signal, calls `state.lock_screen.close_browser()` → `hide_native_window()` | `lock_screen_state=hidden`, `native_window` SW_HIDE'd | `/debug` | — |
| **40** | Pod 4 / Win32 | F1 25 takes foreground (fullscreen exclusive or borderless fullscreen) | `tasklist /V` → F1 25 window title visible, foreground process = F1_25.exe | tasklist /V + `GetForegroundWindow` check | Memory: NVIDIA Surround triple-monitor must still be working |
| **41** | Customer / physical | Sees F1 25 main menu on the 3 monitors | Visual confirmation | User eye | Visible window titles overlay story from CLAUDE.md |
| **42** | Customer / F1 25 | Navigates menus → starts a session (Time Trial / Grand Prix / whatever) | Customer is on track in F1 25 | User eye | — |
| **43** | F1 25 → Pod 4 rc-agent | F1 25 sends UDP telemetry packets to `127.0.0.1:20777` (must be enabled in F1 25 settings + IP:port configured) | UDP packets arrive at F1 25 adapter `UdpSocket` bound on `0.0.0.0:20777` | `netstat -an \| findstr 20777` + rc-agent log `F1 25 adapter listening` | F1 25 UDP telemetry NOT auto-configured — manual setup per pod or setup script |
| **44** | Pod 4 / rc-agent F1 25 adapter | Processes packets, gates on `speed_kmh > 0` before firing `DetectorSignal::UdpActive` | Adapter log: `UdpActive` signal fired when speed > 0 | rc-agent log grep | Button-event packets in menu → don't start billing (per `sims/f1_25.rs:506-514`) |
| **45** | Pod 4 / rc-agent | TIMER-SYNC: Creates session enforcer on `AcStatus::Live` (or equivalent) | Enforcer running, countdown started | `/debug` or log | TIMER-SYNC applies to AC but may have different signal for F125 |
| **46** | Pod 4 / rc-agent | Overlay HUD visibly updates every second (countdown timer) | Overlay shows `29:59` decrementing | User eye | Overlay visible ≠ game visible — check z-order |
| **47** | Customer / F1 25 | Drives laps | Telemetry → rc-agent `read_telemetry()` returns `TelemetryFrame` with lap_time_ms, sector, speed | `/debug` or telemetry forward to server | — |
| **48** | Pod 4 / rc-agent F1 25 adapter | Detects lap completion (via `last_completed_lap.take()` at [sims/f1_25.rs:561](../../../crates/rc-agent/src/sims/f1_25.rs#L561)) | `poll_lap_completed` returns `Some(LapData)` | rc-agent log: `lap_completed` event | — |
| **49** | Pod 4 → Server | Sends `AgentMessage::LapCompleted` over WS | Server logs, `laps` table insert | Server log + DB | — |
| **50** | Pod 4 / rc-agent | 30-min timer expires → inactivity monitor or session_enforcer fires → sends `EndBilling` back to server | Server receives end, writes `billing_sessions.ended_at`, computes refund/charges | Server log + DB | — |
| **51** | Server → Pod 4 | Sends `SubSessionEnded { ... }` WS → rc-agent shows session summary via `show_session_summary()` for ~N seconds → falls through to `show_idle_state()` → returns to `screen_blanked` (post IDLE-01) | `lock_screen_state` transitions Summary → screen_blanked; user sees RP blank animation again | `/debug` + user eye + server log | IDLE-01 verified on fresh idle return only via Pod 4 canary; not yet tested via full billing cycle |

---

## 5. Phase 2 — BASELINE: the AC control case

**Why it matters:** Assetto Corsa launch is known-working at this venue. Running the same flow for AC lets us compare which hops succeed/fail and isolate what's specific to F1 25 / Steam URI launches.

**Differences AC vs F1 25 (mapped to the hops above):**

| Hop | AC flavor | F1 25 flavor | Implication |
|---|---|---|---|
| 30 | `ac_launcher::launch_ac()` → custom INI generation → direct `acs.exe` spawn with `DETACHED_PROCESS` | `GameProcess::launch()` → `cmd /C start "" steam://rungameid/3059520` | **Completely different spawn path.** AC bypasses Steam. |
| 30a | AC has `ac_launcher.rs` (3825 lines) handling asset pre-flight, race.ini generation, Content Manager URIs for MP | F1 25 has zero rc-agent-side pre-flight — relies entirely on Steam | F1 25 has a much smaller attack surface for launch bugs |
| 31-34 | Steam not involved (SP). For MP, uses `acmanager://` Content Manager URI | Steam is the only launcher, Steam state matters | Steam-related failure modes are F1 25 exclusive |
| 36 | AC sets `lock_screen_state=launch_splash`, then AC path hits `close_browser()` synchronously at [ws_handler.rs:767](../../../crates/rc-agent/src/ws_handler.rs#L767) | F1 25 path uses the GAME-07 async mpsc channel | Different signal mechanism; LAUNCH-FIX-3 only matters for F1 25 / other non-AC |
| 43 | AC telemetry via shared memory (3 Kunos SHM blocks) | F1 25 telemetry via UDP 20777 | Different adapter entirely — `sims/assetto_corsa.rs` vs `sims/f1_25.rs` |
| 45 | "Live" signal from AC status field (2 = LIVE) | "Live" signal from `speed_kmh > 0` UDP packet | Different "playable" criterion |

**Phase 2 deliverable:** Run hops 0-20 (pre-launch through billing) for an AC launch on Pod 4 and record the observables. The first 20 hops should be IDENTICAL between AC and F1 25 — if any differ, that's a pre-launch divergence. The later hops (21-51) differ by design and are documented above.

**When to run Phase 2:** before any F1 25 trace, to confirm the "shared" hops work at all on Pod 4 today.

---

## 6. Phase 3 — INSTRUMENTATION: continuous observability

**Goal:** capture every hop's observable automatically during a trace run so we don't rely on manual probing that misses transient state.

**Setup (one-time per trace run):**

1. **Pod 4 state capture loop** — run from James `.27` via Bash in the background:
   ```bash
   # tail-state.sh — snapshots Pod 4 every 2s to a timestamped file
   while true; do
     TS=$(python3 -c "from datetime import datetime,timedelta;print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%H:%M:%S'))")
     HEALTH=$(curl -sS -m 3 http://192.168.31.88:8090/health 2>/dev/null)
     DEBUG=$(curl -sS -m 3 http://192.168.31.88:18924/debug 2>/dev/null)
     echo "$TS | $HEALTH | $DEBUG" >> /tmp/pod4-state-$(date +%Y%m%d-%H%M).log
     sleep 2
   done
   ```

2. **Server state capture loop** — snapshots server fleet health for Pod 4 every 2s:
   ```bash
   while true; do
     TS=$(...); P4=$(curl -sS http://192.168.31.23:8080/api/v1/fleet/health | python3 -c "import json,sys;d=json.load(sys.stdin);p=[x for x in d['pods'] if x['pod_number']==4][0];print(json.dumps({k:v for k,v in p.items() if k in ['build_id','game_state','screen_blanked','ws_connected','experience_status','active_sentinels']}))")
     echo "$TS | $P4" >> /tmp/server-pod4-state-$(date +%Y%m%d-%H%M).log
     sleep 2
   done
   ```

3. **Pod 4 process watch** — snapshots game-related process state every 2s:
   ```bash
   # Uses C:\Users\bono\tmp\exec_tasklist.json payload
   while true; do
     TS=$(...); OUT=$(curl -sS -m 5 -X POST http://192.168.31.88:8090/exec -d @C:/Users/bono/tmp/exec_tasklist.json | python3 -c "import json,sys;r=json.load(sys.stdin);print(';'.join(l.split(',')[0].strip('\"')+':'+l.split(',')[1].strip('\"') for l in r.get('stdout','').splitlines() if any(k in l.lower() for k in ['f1_25','f1_2025','steam','rc-agent','acs.exe'])))")
     echo "$TS | $OUT" >> /tmp/pod4-procs-$(date +%Y%m%d-%H%M).log
     sleep 2
   done
   ```

4. **Pod 4 rc-agent log tail** — streams new log lines as they're written:
   ```bash
   # Poll the JSONL log and diff it every 2s (cmd.exe has no tail -f)
   # Alternative: use PowerShell Get-Content -Wait via exec
   ```
   (Implementation detail — add to instrumentation-scripts/ when we run trace.)

5. **Server racecontrol log tail** — SSH to server .23 and tail its log file:
   ```bash
   ssh server "powershell -Command \"Get-Content C:/RacingPoint/racecontrol-*.jsonl -Tail 0 -Wait\""
   ```

6. **User eye log** — a simple numbered log the user fills in during the trace:
   ```
   T+0   : clicked launch button
   T+3s  : Steam loading spinner on center monitor
   T+8s  : spinner still there
   T+15s : EA anti-cheat popup
   ...
   ```

**Trace run artifact layout:**
```
.planning/debug/flow-traces/runs/
  2026-04-11-2145-f1_25-attempt-01/
    pod4-state.log              # from capture loop 1
    server-pod4-state.log       # from capture loop 2
    pod4-procs.log              # from capture loop 3
    rc-agent-tail.log           # from tail 4
    racecontrol-tail.log        # from tail 5
    user-eye.log                # from manual note-taking
    trace-analysis.md           # post-run analysis — which hops diverged
```

**Clock sync:** all capture loops compute IST via manual UTC+5:30 (Git Bash `TZ=Asia/Kolkata` silently fails per memory). Every line is timestamped so they can be merged into a single timeline in analysis.

---

## 7. Phase 4 — TRACE: running the scenario

**Procedure:**

1. **Kill any orphan capture loops from prior runs** (tmp log files get big fast)
2. **Verify prereqs (§2)** — all 15 must pass. If not, fix first.
3. **Start the 5 instrumentation loops (§6)** — verify each writes output
4. **Mark T+0** — note the IST time the trace starts
5. **User runs the scenario manually** following hops 1-9 (browser actions)
6. **User fills in user-eye.log** with timestamps and observations
7. **Let the trace run to completion OR to the point where it clearly breaks**
8. **Stop the capture loops**
9. **Merge all 5 log files into a single timeline** sorted by timestamp
10. **Walk the MAP** row by row. For each hop, find the corresponding evidence in the timeline. Mark:
    - ✅ observed as expected
    - ⚠️ observed but different value (note the divergence)
    - ❌ no evidence of this hop happening at all
11. **First ❌ or first ⚠️ = divergence point** = bug layer
12. **Write the trace-analysis.md** with:
    - Trace run ID
    - First divergence hop + observed vs expected
    - All evidence citations (log line, timestamp)
    - Hypothesis list for that layer (cheapest test first)

**Rule: do NOT fix anything during the trace.** The trace is read-only. Fix goes into Phase 5.

---

## 8. Phase 5 — FIX ONE: applying a change

**Preconditions:**
- Trace completed
- First divergence point identified with evidence
- Hypothesis list for that layer written down

**Procedure:**

1. **Pick the cheapest hypothesis to test**
2. **Test it** (grep, read, curl, tasklist — no code changes)
3. **If confirmed:** write the smallest possible fix at the identified layer
4. **One file, one intent, one commit.** No bundling.
5. **Commit message includes:** bug tag, divergence hop, fix summary, trace run ID for evidence
6. **LOGBOOK.md update** per standing rule
7. **Git push**
8. **Deploy only to Pod 4 (canary)** using the existing deploy sequence
9. **No other pods until Phase 6 retrace passes on Pod 4**

**Forbidden:**
- Fixing a second bug "while we're in there"
- Refactoring code in the vicinity of the fix
- Pre-fixing hops downstream of the current divergence
- Assumption-based fixes without a confirmed hypothesis

---

## 9. Phase 6 — RETRACE: running again

**Procedure:**

1. **Start new run directory** `runs/<timestamp>-f1_25-attempt-NN/`
2. **Re-run the trace (§7)**
3. **Compare new timeline against the previous divergence point:**
   - If the hop now passes: fix worked. New divergence point = next bug OR flow completes.
   - If the hop still fails: fix didn't work. Revert? Adjust?
4. **Repeat until the flow completes or a hard blocker stops us**

**Convergence rule:** each retrace should push the first-divergence point LATER in the flow. If it regresses EARLIER, we introduced a new bug — revert immediately.

---

## 10. Phase 7 — CODIFY: automating the trace

Once a complete trace passes end-to-end manually, convert it into an automated test:

1. **Playwright script** simulates hops 1-9 (browser actions)
2. **Bash/Node harness** runs probes for hops 10-51 automatically
3. **Assertions** map 1:1 to the "Expected observable" column in the MAP
4. **Placed under `tests/e2e/playwright/kiosk/f1_25-staff-launch-trace.spec.ts`**
5. **Test runs against Pod 4** as the canary (not against James-local, not against a mock)
6. **Test fixtures:**
   - Test driver `3c6e7dc2` must have ≥ ₹10K wallet
   - Pre-test cleanup: kill any F1_25.exe, clear MAINTENANCE_MODE, etc.
   - Post-test cleanup: end billing, return pod to idle

---

## 11. Phase 8 — GUARD: CI integration

1. **Add the Playwright trace test to the existing e2e suite** (`tests/e2e/playwright/`)
2. **Gate merges on the trace test passing** (so a regression re-introducing any of the bugs in §3 blocks the PR)
3. **Add a nightly cron** that runs the trace against the venue at low-traffic hours
4. **Fail loud:** any trace failure fires a WhatsApp alert to staff + a comms-link message to Bono

---

## 12. Adapting this MAP for other games

This trace was written for F1 25. To adapt for another game, change these hops:

| Hop | Game-dependent | Notes |
|---|---|---|
| 7 | Game picker inclusion | Verify game listed in pod inventory per Phase 361 preset filter |
| 22 | Server validates pod has game | Must be in pod's `installed_games` array |
| 24 | WS message `sim_type` | F125 / IRacing / LeMansUltimate / AssettoCorsaEvo / AssettoCorsaRally / ForzaHorizon5 |
| 27 | Pre-launch checks orphan list | Include the game's .exe names in `all_game_process_names()` |
| 29 | Launch path branch | AC uses `ac_launcher::launch_ac()` (direct exe), others use `GameProcess::launch` (Steam URI) |
| 31 | Steam App ID | 3058630 (ACE), 3917090 (ACR), 3059520 (F1 25), 266410 (iRacing), 2399420 (LMU), 1551360 (FH5) |
| 34 | Anti-cheat | EA Javelin (F1 25), EAC/EOS (iRacing, LMU), none (AC, ACE, ACR), custom (FH5) — each has a different service chain |
| 35 | Game exe name | `acs.exe`, `AssettoCorsaEVO.exe`, `acr.exe`, `iRacingSim64DX11.exe`, `LMU.exe`, `F1_25.exe`, `ForzaHorizon5.exe` |
| 43 | Telemetry transport | AC/ACE/iRacing/LMU use shared memory, F1 25 uses UDP 20777, Forza uses UDP 5300 |
| 45 | "Playable" signal | `AC_STATUS=2` (AC), `IsOnTrack` var (iRacing), Scoring SHM game phase (LMU), `speed_kmh>0` (F1 25) |
| 48 | Lap completion detection | Counter increment (iRacing), UDP packet field (F1 25), SHM reading (AC/ACE), Scoring SHM (LMU) |

**Reusable sections (unchanged per game):**
- Phase 0 idle state (same for all games post IDLE-01)
- Hops 1-20 (kiosk staff → billing start → BillingStarted → lock screen hidden)
- Hops 50-51 (session end → idle return)
- Phases 2 (baseline compare), 3 (instrumentation), 4-8 (procedure)

**One-and-done promise:** if we run this trace once for F1 25, fix every divergence we find, and converge to a passing run — we have a proven template. Adapting for iRacing/LMU/ACE/ACR is then a mechanical substitution of the table above + re-running the trace.

---

## 13. Open questions for user before Phase 2 starts

1. **Trace run location:** OK to write `runs/` subdirectories under `.planning/debug/flow-traces/`? (Yes by default — tell me if you want them elsewhere.)
2. **Instrumentation loops:** OK to run 5 background Bash loops on James for the duration of a trace? They're read-only and cheap. (Yes by default — tell me if you want fewer or different probes.)
3. **User-eye log format:** prefer (a) a markdown file you type into, (b) voice-to-text dictation while walking the venue, (c) me calling out timestamps and you shouting observations back?
4. **AC baseline — who runs it and when?** Staff needs to fire an AC launch with no customer to capture the baseline. This is 5-10 min of venue time. Fit this in now, after hours, or skip it and go straight to F1 25 trace?
5. **Kill the parallel F-01/F-02 session resolution before we start tracing, or live with it?** Their session still has an uncommitted patch waiting for "continue". If they commit and deploy server changes mid-trace, our evidence gets contaminated. Recommend: tell them to hold until we finish one F1 25 trace cycle.
6. **Verification target for the trace run:** Pod 4 physical display, OR we also want go2rtc camera snapshots at key hops? The NVR has Pod 4 coverage — we could auto-snapshot.
