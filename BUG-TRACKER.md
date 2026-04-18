# Bug Tracker — RaceControl

> **Single source of truth for all known bugs.**
> Updated: 2026-04-17 04:00 IST | HEAD: `3bb882cc` | Server: `3bb882cc` | Pods 1-8: `3bb882cc` | POS: `408b04c8` (SAC blocked) | VPS: `8a0f82a1`

## How to Use

- **Every session:** Read this file first. Update status after fixing/deploying/verifying.
- **Status flow:** `OPEN` -> `CODE_FIXED` -> `DEPLOYED` -> `VERIFIED` -> `CLOSED`
- **Don't close without evidence.** Paste the verify command + output in the Notes column or link a handoff.

---

## Wave 1 — Maximum Impact (fix root causes, collapse dependent bugs)

### Chain A: Zero Laps (ROOT CAUSE — resolves 6+ dependent bugs)

| ID | Bug | Severity | Status | Fix Commit | Deployed To | Depends On | Notes |
|----|-----|----------|--------|------------|-------------|------------|-------|
| **ZL-1** | Apps preset missing `[RACECONTROL]` plugin — AC never loads RC plugin, zero laps ever recorded | **P0** | DEPLOYED | `d4b6247d` (apps-default.ini preset add) + `4bc9dce8` (SHM name `Local\` prefix + static offsets + `sim-ac` tracing filter) | Server: `3bb882cc`. Pods 1-8: `3bb882cc`. POS: blocked (SAC). VPS: `8a0f82a1` | — | **Deployed 2026-04-17 03:00 IST.** Needs runtime verification: launch AC, drive laps, check DB. Prior attribution `e86ff0c0 tree` was build_id containing the fix, not the fix commit — corrected 2026-04-18. |
| ZL-2 | python.ini missing `[RACECONTROL]` section on pods | P0 | DEPLOYED | `ac0b215e` + `cfc73811` | Server: `3bb882cc`. Pods 1-8: `3bb882cc` (includes both). POS: blocked (SAC) | ZL-1 | **Deployed 2026-04-17 03:00 IST.** Both `ac0b215e` and `cfc73811` now on all pods. |
| ZL-3 | 0 laps in DB (DB-1) | P0 | OPEN | — | — | ZL-1 + ZL-2 | Symptom, not a bug. Resolves when ZL-1+ZL-2 deployed+verified. |
| ZL-4 | 222/342 sessions 0 driving_seconds (DB-4) | P1 | OPEN | — | — | ZL-1 | Same root cause — no telemetry SHM without plugin. |
| ZL-5 | Only 7% sessions "completed" (DB-7) | P1 | OPEN | — | — | ZL-1 | No completion detection without lap data. |
| ZL-6 | Sessions "ended" with 20s avg driving — false idle (DB-8) | P1 | OPEN | — | — | ZL-1 | Idle detection triggers without telemetry feedback. |
| ZL-7 | Snap billing 30min boundary can't be verified (Pending-4) | P2 | BLOCKED | `290f16ca` | All targets | ZL-1 | Code is deployed. Can't test until laps work. |
| ZL-8 | Kiosk visual verification blocked | P2 | BLOCKED | — | — | ZL-1 | Can't verify kiosk tier display without working sessions. |

**Wave 1A action:** ~~Rebuild `rc-agent`, deploy to all 8 pods + POS.~~ **DONE 2026-04-17 03:00 IST.** All 8 pods on `3bb882cc`. POS blocked by SAC (S-4). Needs runtime verification at next venue visit.

---

### Chain B: Game Launch Config (config-only, no rebuild needed)

| ID | Bug | Severity | Status | Fix | Deployed To | Depends On | Notes |
|----|-----|----------|--------|-----|-------------|------------|-------|
| GLC-1 | iRacing listed in TOML but not installed — 9 launch crashes on Pods 3+4 (GL-5) | P2 | NEEDS_INVESTIGATION | — | — | — | **Misdiagnosed:** iRacing IS installed on Pods 3+4, exe exists at configured path. Crashes are runtime, not config. Reclassified to Wave 2. |
| GLC-2 | No exe_path/steam_app_id for Forza — 3 crashes on Pods 3+8 (GL-10) | P2 | OPEN | Remove `forza = 5300` UDP line from TOML, or add `[games.forza]` if Forza gets installed | — | — | **Confirmed:** Forza NOT installed on Pods 3+8. No `[games.forza]` section exists. UDP port line is harmless but misleading. |
| GLC-3 | Path not found — 1 crash on Pod 8 (GL-12) | P3 | OPEN | — | — | GLC-2 | Likely resolves with GLC-2 TOML fix. Single occurrence. |
| GLC-4 | **Structural:** inventory_rescan adds games without launch config | P2 | OPEN | Validate exe_path exists at scan time, reject unconfigured games | — | — | Prevents GLC-1/2/3 from recurring. Code change in `game_inventory.rs`. |

**Wave 1B action:** SSH to pods, audit TOMLs, fix configs. Then fix `game_inventory.rs` structurally.

---

### Chain C: Already Fixed + Deployed — Need Runtime Verification

| ID | Bug | Severity | Status | Fix Commit | Deployed To | Verify How |
|----|-----|----------|--------|------------|-------------|------------|
| V-1 | Steam dialog blocking game launch — 13 crashes (GL-8) | P1 | DEPLOYED | `40968ddc` | Server + all pods | Launch a Steam game, check no vguiPopupWindow blocks. Check `game_launch_events` for new GL-8 crashes after deploy date. |
| V-2 | F1 25 orphan process — 10 crashes (GL-9) | P1 | DEPLOYED | `bf8a30e4` | Server + all pods | Launch F1 25 on Pod 4, exit, relaunch. Check no "orphan process" error in agent log. |
| V-3 | car/track/sim_type always NULL in billing_sessions (DB-2) | P1 | DEPLOYED | `40968ddc` | Server + all pods | Start a session, check `SELECT car, track, sim_type FROM billing_sessions ORDER BY started_at DESC LIMIT 1`. |
| V-4 | Watchdog MAINTENANCE_MODE never auto-clears (TTL bug) | P1 | DEPLOYED | `f55134f3` | Server | Wait for a crash+recovery cycle, verify sentinel is cleared within 30 min. Or read watchdog log for "Clearing stale MAINTENANCE_MODE". |
| V-5 | Bat quoting — `set HOSTNAME=0.0.0.0` trailing space | P1 | DEPLOYED | Manual fix on server | Server | Reboot server, verify :3200 and :3201 start automatically via schtask. |
| V-6 | Session never ends — driver_name column missing + StopGame not sent | P0 | DEPLOYED | `7fb716e2` | Server: `3bb882cc`. Pods 1-8: `3bb882cc`. VPS: `8a0f82a1` | **Deployed 2026-04-17.** Verify: end a session from kiosk, confirm game stops. |
| V-7 | Wallet balance shows 0 cr — field name mismatch | P1 | DEPLOYED | `e86ff0c0` | Server: `3bb882cc`. Pods 1-8: `3bb882cc`. VPS: `8a0f82a1` | **Deployed 2026-04-17.** Verify: check kiosk wallet display shows correct balance. |
| V-8 | Per-minute debit used invalid txn_type — sessions auto-ending at 120s | P0 | DEPLOYED | `408b04c8` | Server: `3bb882cc`. Pods 1-8: `3bb882cc`. VPS: `8a0f82a1` | **Deployed 2026-04-17.** Verify: run per-minute session >2min, confirm it doesn't auto-end. |

**Wave 1C action:** V-1 through V-5 just need testing at next venue visit. ~~V-6/V-7/V-8 need deploy first~~ **V-6/V-7/V-8 now DEPLOYED** (included in Wave 1A `3bb882cc` deploy). All 8 items need runtime verification at venue.

---

## Wave 2 — Investigation (281 failure events, reclassified)

> **Wave 1 deployed 2026-04-17 03:00 IST.** Post-deploy: 1 transient crash (Pod 6, immediate recovery). Full reclassification below.

### 2A: Active Investigation — Root Cause Unknown

| ID | Bug | Count | Severity | Pods (top 3) | Status | Analysis | Next Step |
|----|-----|-------|----------|--------------|--------|----------|-----------|
| INV-1 | Generic "Game process exited unexpectedly" — no exit code captured | 111 | P2 | Pod 8 (23), Pod 3 (22), Pod 4 (22) | CODE_FIXED | `event_loop.rs:1065` now passes `game.last_exit_code` through to the crash event. `game_process.rs` PID-only path (Steam URL launches) now captures exit code via `GetExitCodeProcess` Win32 API. Future crashes will report exit code instead of "no exit code". | **Venue:** After deploy, check DB for new crashes — they should now have exit codes for classification. Check `%USERPROFILE%\Documents\Assetto Corsa\logs\log.txt` on Pods 3/4/8. |
| INV-2 | Launch timeout 120s — AC never reaches "Running" state | 26 | P2 | Pod 8 (15), Pod 4 (6), Pod 6 (6) | EXPECTED_SELF_RESOLVE | All Assetto Corsa. Pod 8 dominates (58%). Root cause: AC SHM never populated because RC plugin wasn't loaded (ZL-1/ZL-2). ZL-1+ZL-2 deployed `3bb882cc` 2026-04-17 03:00 IST. Server timeout at 120s for AC (game_launcher_support.rs:100). | **Re-measure after 24h venue operation.** If still occurring post-ZL-1: SSH to Pod 8 during AC launch, verify plugin loads in AC log.txt. |
| INV-3 | Exit code 1 — process exits with known error code | 13 | P2 | Pod 4 (9), Pod 3 (2), Pod 8 (2) | NEEDS_VENUE_VERIFY | Pod 4: 9/13 are F1 25 orphan process blocking AC launch (pairs with V-2 orphan cleanup `bf8a30e4`). Pod 3: 2 are AC Evo. INV-1 fix now captures exit codes for all future crashes. V-2 orphan cleanup deployed — needs runtime verify. | **Venue:** Verify V-2 orphan cleanup works on Pod 4 (launch F1 25, exit, launch AC). Check Pod 3 AC Evo logs. |
| INV-4 | "Launch timed out (30s)" — stale Stopping state cleanup | 8 | P3 | Pod 6 (2), Pod 3 (2), Pod 7 (2) | BY_DESIGN | **Misclassified in initial triage.** These are NOT launch timeouts — they're stale Stopping state cleanups from `game_launcher_support.rs:131-132`. When the server restarts and finds a game in Stopping state for 30-90s, it force-errors it. This is intentional cleanup, not a bug. All 8 events correlate with known server restart times. | No action needed. |

### 2B: Already Fixed — Historical Events Only

| ID | Bug | Count | Fixed By | Last Event | Notes |
|----|-----|-------|----------|------------|-------|
| INV-5 | `session_type="pitlane"` rejected | 23 | Pod 8 incident fix + test (`ac_launcher.rs:3582`) | 2026-04-08 | 20/23 on Pod 8. Kiosk `start_type` leaked into `session_type`. Fixed in kiosk wizard. |
| INV-6 | `ERROR_INVALID_HANDLE (os error 6)` on `acs.exe` spawn | 9 | `spawn_safe.rs` + `ac_launcher.rs:738` Stdio::null() | 2026-04-07 | Pod 3 (6), Pod 4 (3). FreeConsole() invalidated handles. All pre-fix. |
| INV-7 | `os error 50` "request not supported" on `acs.exe` spawn | 2 | Same `spawn_safe` fix | 2026-04-07 | Pod 1 (1), Pod 4 (1). Related to console handle state. |
| INV-8 | Orphan F1_25.exe blocking AC launch | 8 | `bf8a30e4` orphan cleanup (V-2) | 2026-04-16 | ALL Pod 4. F1 25 process resists `taskkill`. Deployed — needs runtime verify (V-2). |

### 2C: New Pattern — Steam Dialog Blocking

| ID | Bug | Count | Severity | Pods | Games | Status | Next Step |
|----|-----|-------|----------|------|-------|--------|-----------|
| INV-9 | Steam dialog visible after 60s — game never launches | 15 | P2 | 7 pods (all except Pod 8) | Forza Horizon 5 (5), EA SPORTS WRC (5), Le Mans Ultimate (3), AC Evo (1), F1 25 (1) | CODE_FIXED | Root cause: `steam_checks.rs` dismissed dialog only ONCE (`steam_dialog_dismissed` flag). Steam shows sequential dialogs (update→EULA→DRM) — after dismissing the first, the flag prevented retrying. **Fix:** Removed single-dismiss limit. Now retries every 2s poll cycle with counter logging. | **Venue:** After deploy, test Steam game launch. If still occurring, check Steam auto-login config and disable update prompts in Steam settings. |

### Summary: 281 failure events reclassified — Wave 2 COMPLETE

| Category | Events | Bugs | Status |
|----------|--------|------|--------|
| **CODE_FIXED (this session)** | 126 | INV-1, INV-9 | Exit code capture + Steam dialog retry. Needs deploy + venue verify. |
| **EXPECTED_SELF_RESOLVE** | 26 | INV-2 | ZL-1 plugin fix deployed `3bb882cc`. Re-measure after venue hours. |
| **NEEDS_VENUE_VERIFY** | 13 | INV-3 | F1 25 orphan on Pod 4. V-2 deployed. |
| **BY_DESIGN** | 8 | INV-4 | Stale Stopping state cleanup, not a bug. |
| **ALREADY_FIXED (prior sessions)** | 42 | INV-5/6/7/8 | Historical only. |
| Wave 1 (GLC-1/2/3 config) | 13 | 3 bugs | Tracked in Wave 1. |
| Already tracked (V-1 to V-8) | 53 | 8 bugs | Deployed, need runtime verify. |

**Post-deploy crash rate:** 1 transient event in ~20 min. Need venue hours for real rate.

**Wave 2 action plan:**
1. ~~Query post-deploy crash rate~~ **DONE** — 1 transient event.
2. ~~Code fix INV-1~~ **DONE** — exit code capture for both child process and PID-only paths.
3. ~~Code fix INV-9~~ **DONE** — Steam dialog dismissal retries every poll cycle.
4. ~~Investigate INV-4~~ **DONE** — BY_DESIGN (stale Stopping cleanup).
5. **BUILD + DEPLOY** — `cargo build --release --bin rc-agent` with INV-1/INV-9 fixes.
6. **Venue visit:** Verify V-2 orphan cleanup (Pod 4), check AC Evo logs (Pod 3), test Steam game launch (INV-9).
7. **Re-measure after 24h** — INV-2 timeouts should self-resolve with ZL-1 plugin.

---

## Wave 3 — Structural + Low Priority

| ID | Bug | Severity | Status | Notes |
|----|-----|----------|--------|-------|
| S-1 | AI debugger: 5/12 safety gaps remaining (dead code, pattern coarseness, dual-system coordination) | P2 | OPEN | See `session_handoff_20260416_ai_debugger_structural_fix.md`. 7/12 done in `f4de983d`. |
| S-2 | DB-3: `refunds` table empty — may be dead table | P3 | NEEDS_INVESTIGATION | Check if Phase 363 migration created it. May be unused. |
| S-3 | DB-6: `experience_id` always NULL in billing_sessions | P3 | OPEN | Kiosk may not send it, or billing endpoint ignores it. Code fix needed. |
| S-4 | POS agent not running — SAC blocks unsigned exe | P1 | BLOCKED | Smart App Control (SAC=On) on POS PC. Needs SAC disabled by Uday or code signing. |
| S-5 | Server -dirty builds from deploy hygiene | P3 | CLOSED | Fixed in `f55134f3` — `incremental = false` in release profile. Clean builds going forward. |
| S-6 | Frontend watchdog not auto-restarting on its own | P2 | DEPLOYED | `schtasks /Run` done manually. Monitor if it self-starts after next reboot. |
| S-7 | VPS/Server build parity gap | P2 | OPEN | Server: `f55134f3`, VPS: `453ae086`. VPS is behind. Deploy to VPS after Wave 1. |
| S-8 | `mesh_kb.db` corrupted on Pod 8 | P3 | OPEN | `"database disk image is malformed"` every 5 min. Delete → rc-agent recreates. |
| S-9 | EXEC BLOCKED spam every 2 min on Pod 8 | P3 | OPEN | `netstat -ano \| findstr ...` blocked by metacharacter sanitizer. Unknown caller. |
| S-10 | Wheelbase USB disconnected on Pod 8 | P2 | OPEN | `RESIL-04` at boot. Physical check needed. May affect FFB. |
| S-11 | WS round-trip warnings 935-1628ms on Pods 3-8 + clock drift 9-14s | P2 | OPEN | May contribute to launch timeouts (INV-2, INV-4). Network investigation needed. |
| S-12 | Invalid Date bugs across 3 apps (9 occurrences) | P2 | CODE_FIXED | `f41c8478` + `35ce755`. Deployed to server+VPS in previous session. Needs visual verify. |
| S-13 | Status page stops updating — `.finally()` on fetching flag | P2 | CODE_FIXED | `bdec5170`. Deployed to server. Needs verify during next venue visit. |

---

## Deploy Dependency Map

```
HEAD (3bb882cc) ──── contains all fixes
  │
  ├── Server (.23): 3bb882cc ─── AT HEAD ✓ (deployed 2026-04-17 02:42 IST)
  │
  ├── Pods 1-8: 3bb882cc ─── AT HEAD ✓ (deployed 2026-04-17 02:43-03:05 IST)
  │
  ├── POS (.130): 408b04c8 ─── BLOCKED by SAC (S-4)
  │     ACTION: Disable SAC, then run deploy-pod-agent.sh 9 3bb882cc
  │
  ├── VPS: 8a0f82a1 ─── AT HEAD ✓ (rebuilt 2026-04-17 03:10 IST)
  │
  ├── Kiosk frontends (server): REBUILT ✓ (deployed 2026-04-17 03:01 IST)
  │     Kiosk :3300 — API proxy verified, deep health passes
  │     Web :3200 — page loads, API proxy not tested (client-side API calls)
  │     Admin :3201 — page loads, auth redirect working
  │
  └── Kiosk frontends (VPS): REBUILT ✓ (deployed 2026-04-17 03:12 IST)
```

---

## Execution Checklist — Wave 1

```
[x] 1. cargo build --release --bin racecontrol (James) — 58MB, 3m42s
[x] 2. cargo build --release --bin rc-agent (James) — 26MB, 1m05s
[x] 3. Deploy racecontrol to server .23 — build_id=3bb882cc verified
[x] 4. Deploy rc-agent to Pod 8 (canary) — build_id=3bb882cc, stable 200s+
[ ] 5. Launch AC on Pod 8, drive 2-3 laps — NEEDS VENUE VISIT
[ ] 6. Check: SELECT * FROM laps ORDER BY created_at DESC LIMIT 5 — NEEDS VENUE VISIT
[x] 7. Deploy rc-agent to all pods — Pods 1-8 all 3bb882cc (deploy-pod-agent.sh)
[ ] 8. Verify V-1 through V-5 (runtime checks) — NEEDS VENUE VISIT
[x] 9. Audit GLC-1/GLC-2 TOML configs — iRacing installed (misdiagnosis), Forza not installed
[x] 10. Rebuild kiosk + web + admin frontends on server — API proxy verified
[x] 11. Deploy to VPS (git pull + rebuild + verify) — build_id=8a0f82a1
[ ] 12. Re-measure crash rate post-deploy — NEEDS VENUE HOURS
```

---

## Session Log

| Date | Session | Bugs Closed | Bugs Opened | Notes |
|------|---------|-------------|-------------|-------|
| 2026-04-17 | Created tracker | 0 | 30 | Consolidated from 8 handoff files |
| 2026-04-17 | Wave 1 deploy | 0 | 0 | Server+Pods 1-8+VPS all on 3bb882cc. ZL-1/ZL-2 DEPLOYED. V-6/V-7/V-8 DEPLOYED. Frontends rebuilt. GLC-1 reclassified (iRacing installed). 4 items need venue visit: laps verification, V-1 to V-5 runtime checks, crash rate re-measure. |
| 2026-04-17 | Wave 2 investigation + fixes | 0 | 1 | 281 events → 9 bugs. **CODE_FIXED:** INV-1 (exit code capture, 3 files), INV-9 (Steam dialog retry). **BY_DESIGN:** INV-4 (stale Stopping cleanup). **SELF_RESOLVE:** INV-2 (ZL-1 plugin). **VENUE_ONLY:** INV-3 (Pod 4 orphan). 42 historical (INV-5/6/7/8). All tests pass: rc-agent 799, racecontrol 948, rc-common all. |
