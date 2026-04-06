# Racing Point — Error Catalog

Indexed reference of known errors, root causes, and fixes. Organized by system layer.

---

## Windows / OS Errors

### `0xC0000005` — Access Violation (ntdll.dll)

- **Where:** Pod crash-seh.log
- **Symptom:** rc-agent crash loop, consistent offset
- **Root Cause:** Corrupted WMI/COM state, marginal RAM, or driver crash
- **Fix:** Reboot pod. If persists: `sfc /scannow`, `winmgmt /verifyrepository`, test RAM with `mdsched`
- **History:** Pod 6 crash-looped for 5 hours (2026-03-26). Same binary stable on 7 other pods. Reboot fixed.

### `os error 10048` — Address Already In Use

- **Where:** rc-agent startup, ports 8090/18923/18924
- **Symptom:** Port binding fails, service won't start
- **Root Cause:** Stale process holding port (zombie rc-agent, orphan PowerShell)
- **Fix:** `netstat -ano | findstr <port>` -> `taskkill /F /PID <pid>`. SO_REUSEADDR (305638b) reduces frequency.
- **Prevention:** Named mutex `Global\RacingPoint_RCAgent_SingleInstance`

### `CLOSE_WAIT` Socket Accumulation

- **Where:** rc-agent :8090 (health poll connections not closing)
- **Symptom:** Port exhaustion, rc-agent stops accepting connections
- **Root Cause:** HTTP clients not sending Connection: close, TCP TIME_WAIT buildup
- **Fix:** `Connection: close` header middleware in remote_ops. Self-monitor detects >20 and reboots (night_ops).
- **Diagnosis:** `netstat -ano | findstr 8090 | findstr CLOSE_WAIT | find /c /v ""`

### `The process cannot access the file because it is being used by another process`

- **Where:** start-rcagent.bat via schtasks
- **Symptom:** rc-agent fails to start, schtask returns exit code 1
- **Root Cause:** `start "" /D dir prog.exe 2>> file.log` — file redirect on `start` command fails in non-interactive context
- **Fix:** Remove file redirects from `start` commands in .bat files. Stderr capture in binary itself.
- **History:** Misdiagnosed as Pod 6-specific for weeks; actually fleet-wide (2026-04-03).

---

## Billing Errors

### Double-End / Race Condition

- **Where:** billing.rs `authoritative_end_session()`
- **Symptom:** Billing ended twice (refund + charge)
- **Protection:** CAS (Compare-And-Swap) on billing state prevents double-end
- **Status:** FIXED (billing_fsm.rs, 18 unit tests)

### F-05: Refund Calculation Bug

- **Where:** `end_billing_session()` line 2213/2255
- **Symptom:** Customer loses money on early-end (overwritten wallet_debit_paise)
- **Root Cause:** UPDATE overwrites column before SELECT reads it for refund calc
- **Fix:** `5d1ea000` — read original value before UPDATE
- **Status:** RESOLVED, deployed in `ccbabd15`

### Billing Session Survives Pod Restart

- **Where:** rc-agent INTERRUPTED_SESSION sentinel
- **Symptom:** Pod restarts mid-billing, customer's time lost
- **Fix:** rc-agent writes INTERRUPTED_SESSION_{id}.json on graceful shutdown; on next boot, sends recovery notification to server for partial refund
- **Status:** IMPLEMENTED (DEPLOY-02)

---

## WebSocket Errors

### WS Churn (>10 connects/min)

- **Where:** /fleet/health `dashboard_ws_churn` field
- **Symptom:** Dashboard enters connect/disconnect loop
- **Root Cause:** Stale frontend JS can't parse new server WS messages
- **Fix:** Rebuild ALL frontends after server binary deploy. Check `connects_per_min` field.
- **History:** 2026-04-03 — admin dashboard with 4-day-old build. 800+ events/min for hours.

### Pod WS Drops Every N Seconds

- **Where:** Server WS handler
- **Symptom:** Game launches return ok:true but never reach agent
- **Root Cause:** Network instability, WS auth failure lockout (5 failures = 300s ban)
- **Fix:** Check server logs for "Pod X registered" frequency. Clear auth failure counter.

### GameTracker Stuck in "Launching"

- **Where:** game_launcher.rs
- **Symptom:** "Already has a game active" error on launch
- **Root Cause:** WS dropped between queue and delivery, agent never ACK'd
- **Fix:** Dynamic timeout: 120s (AC), 90s (others), 180s cap. Auto-transitions to Error on timeout.
- **Manual:** `POST /api/v1/games/stop` to clear stuck state

---

## Game Launch Errors

### Game Process Runs But Not Visible

- **Where:** Pod rc-agent
- **Symptom:** acs.exe running in tasklist but no window on screen
- **Root Cause:** rc-agent in Session 0 (services context, no GUI)
- **Diagnosis:** `tasklist /V /FO CSV | findstr rc-agent` — Session must show "Console"
- **Fix:** Kill rc-agent, let RCWatchdog restart in Session 1

### AI Opponents Missing / Wrong Difficulty

- **Where:** Kiosk -> AcLaunchParams -> race.ini
- **Symptom:** Game launches with zero AI or wrong difficulty
- **Root Cause:** Serde silently drops unknown JSON fields — field name mismatch between kiosk and Rust struct
- **Diagnosis:** Check kiosk `buildLaunchArgs()` field names vs `AcLaunchParams` struct
- **Fix:** Audit Protocol Phase 62. Verify: read back race.ini from pod after launch.
- **History:** 2026-03-26 — `ai_difficulty: "easy"` vs `ai_level: u32`, `ai_count: 5` vs `ai_cars: Vec<AiCarSlot>`

### Content Manager Launch Failure

- **Where:** ac_launcher.rs
- **Symptom:** Content Manager fails to generate race configs
- **Root Cause:** Missing track/car mods, corrupt CM cache
- **Diagnosis:** Check `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\content\`
- **Fix:** Verify game content exists, clear CM cache

---

## Lock Screen Errors

### Blank Screen Stays Active During Session

- **Where:** rc-agent lock_screen.rs
- **Symptom:** Pod screen stays black instead of timer
- **Root Causes (in order):**
  1. rc-agent restarted during active session (server re-sends BillingStarted on Register since 273db1c)
  2. Stale rc-agent holding port 18923 (mutex since 305638b)
  3. Browser not relaunching on state change (always relaunch since 05ef1d6)
  4. WebSocket not connected yet
- **Debug:** See rc-agent Debug Checklist in SERVICE-REFERENCE.md

### `edge_process_count: 0` with `lock_screen_state: screen_blanked`

- **Where:** debug_server :18924/status
- **Symptom:** Lock screen thinks it's blanked but Edge browser never launched
- **Root Cause:** `show_blank_screen()` sets state before `launch_browser()` — silent browser launch failure
- **Diagnosis:** `curl -s http://<pod>:18924/status | jq .edge_process_count`
- **Fix:** Trigger blank screen and verify edge_process_count > 0 within 12s

---

## Deploy Errors

### Build ID Mismatch After Deploy

- **Where:** /health `build_id` field
- **Symptom:** Deployed binary shows old build_id
- **Root Cause:** Cargo cached binary (GIT_HASH not refreshed)
- **Fix:** `touch crates/<crate>/build.rs` before `cargo build --release`

### Binary Size Mismatch (Downloaded HTML Instead)

- **Where:** HTTP staging server
- **Symptom:** Pod downloads 335-byte file instead of 15MB binary
- **Root Cause:** Staging server serving from wrong directory
- **Fix:** `python -m http.server 18889 --directory /path/to/deploy-staging`. Verify file size > 1MB.

### Watchdog Fights Deploy

- **Where:** Server during binary swap
- **Symptom:** Old binary restarts before swap completes
- **Root Cause:** Watchdog not disabled before kill
- **Fix:** `deploy-server.sh` v3.0 disables schtasks + kills watchdog + sets DEPLOY_IN_PROGRESS sentinel

### Static Files 404 After Frontend Deploy

- **Where:** Next.js apps (kiosk, web, admin)
- **Symptom:** Pages load (SSR works) but CSS/JS return 404
- **Root Cause:** `required-server-files.json` has build-machine paths, not deploy-target paths
- **Fix:** Set `outputFileTracingRoot: path.join(__dirname)` in all `next.config.ts`. Verify: curl one `_next/static/` URL.

---

## Authentication Errors

### 401 on rc-agent Allowlist Fetch

- **Where:** GET /api/v1/config/kiosk-allowlist or /guard/whitelist/pod-{N}
- **Symptom:** rc-agent falls back to empty allowlist, everything flagged
- **Root Cause:** GET endpoints were behind auth middleware
- **Fix:** Moved to public_routes (GET is public, POST/DELETE require staff JWT)

### Staff Can't Login (Kiosk)

- **Where:** /kiosk/staff
- **Symptom:** Staff page shows customer page instead
- **Root Cause:** Login page added to middleware auth gate (chicken-and-egg)
- **Fix:** Never middleware-protect a login page. Use client-side auth gate.
- **History:** 2026-04-04 — SEC-P2-9 added /staff to STAFF_ROUTES middleware

### 401 on Service Endpoints (Pod -> Server)

- **Where:** rc-sentry /exec, pod health checks
- **Symptom:** "unauthorized" on all pod commands
- **Root Cause:** Service key mismatch between server and pods
- **Diagnosis:** Compare server racecontrol.toml key vs pod rc-sentry.toml key
- **Fix:** `bash scripts/deploy-preflight.sh <hash>` validates key parity

---

## Recovery System Errors

### MAINTENANCE_MODE Won't Clear

- **Where:** `C:\RacingPoint\MAINTENANCE_MODE`
- **Symptom:** rc-agent permanently blocked, no auto-recovery
- **Root Cause:** 3+ restarts in 10min triggered sentinel
- **Fix (Manual):** `del C:\RacingPoint\MAINTENANCE_MODE` via SSH or rc-sentry exec
- **Auto-clear:** rc-watchdog clears after 30 minutes (SW-07), but rc-agent's startup_cleanup only clears stale OTA sentinels, NOT MAINTENANCE_MODE
- **NOTE:** CLAUDE.md documents this as having no timeout; rc-watchdog adds a 30-min auto-clear

### Multiple Recovery Systems Fighting

- **Where:** Self_monitor + RCWatchdog + rc-sentry + pod_monitor
- **Symptom:** Infinite restart loop, rapid cycling
- **Root Cause:** Independent recovery systems with no coordination
- **Fix:** MAINTENANCE_MODE stops the cycle. Recovery authority registry (rc-common/recovery.rs) prevents concurrent ownership.

### WoL Revives Pod in MAINTENANCE_MODE

- **Where:** Server pod_monitor WoL
- **Symptom:** Pod goes down, WoL wakes it, crashes, WoL again
- **Root Cause:** WoL doesn't check MAINTENANCE_MODE
- **Fix:** Check pod state before WoL (graduated_recovery cascade guard)

---

## Database Errors

### ALTER TABLE Failure (Suppressed)

- **Where:** db/mod.rs migrations
- **Symptom:** Silent — `let _ = sqlx::query("ALTER TABLE ...")` ignores errors
- **Root Cause:** Migration runs ALTER on table that already has the column
- **Impact:** Functional (column already exists) but fragile
- **Better:** Use `ALTER TABLE ... ADD COLUMN IF NOT EXISTS` (SQLite 3.35.0+)

### Cloud Sync Schema Divergence

- **Where:** cloud_sync.rs
- **Symptom:** Sync fails silently, cloud and venue have different columns
- **Root Cause:** Server deployed with new migrations but cloud DB on old binary
- **Fix:** DEPLOY PARITY — always deploy to both. Check column existence before sync.

---

## HMAC / Security TODOs

### Payment Gateway Webhook Unverified

- **Where:** routes.rs lines 8739, 10587, 11037
- **Symptom:** `TODO: Verify HMAC signature from gateway (Razorpay/Cashfree)`
- **Impact:** Payment webhooks accepted without signature verification
- **Status:** TODO — waiting for Bono to deploy matching HMAC key
- **Risk:** Medium — endpoint is rate-limited and logs all calls, but could accept forged webhooks

---

## Process Guard Errors

### 28,749 False Violations/Day

- **Where:** All pods, process-guard.log
- **Symptom:** Everything flagged as violation
- **Root Cause:** Empty allowlist (server was down at boot)
- **Fix:** 5-min periodic re-fetch (821c3031). Safety valve: >80% violations = force report_only.
- **Diagnosis:** Check `violation_count_24h` in fleet health. If 100+ on all pods = empty allowlist.

### Critical System Process Killed

- **Where:** Process guard enforcement
- **Protection:** NEVER_KILL list: csrss.exe, smss.exe, wininit.exe, services.exe, svchost.exe
- **Additional:** Safety valve trips at >80% violation rate
