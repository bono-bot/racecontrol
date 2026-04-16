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
| **ZL-1** | Apps preset missing `[RACECONTROL]` plugin — AC never loads RC plugin, zero laps ever recorded | **P0** | DEPLOYED | `e86ff0c0` tree (ac_launcher.rs +7 lines) | Server: `3bb882cc`. Pods 1-8: `3bb882cc`. POS: blocked (SAC). VPS: `8a0f82a1` | — | **Deployed 2026-04-17 03:00 IST.** Needs runtime verification: launch AC, drive laps, check DB. |
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
| INV-1 | Generic "Game process exited unexpectedly" — no exit code captured | 111 | P2 | Pod 8 (23), Pod 3 (22), Pod 4 (22) | NEEDS_INVESTIGATION | Heartbeat poll path (`event_loop.rs:1065`) detects dead process but doesn't capture exit code (`exit_code: None`). ALL AC except 5 F1 25 on Pod 4. Spike days: Mar 18 (26), Apr 8 (23), Apr 11 (21). **1 post-deploy event** (Pod 6, transient). | **Code fix:** Add `try_wait()` before declaring dead to capture exit code. **Venue:** Check Windows Event Viewer `Application` log for AC crash dumps on Pod 8. Check `%USERPROFILE%\Documents\Assetto Corsa\logs\log.txt` on top 3 pods. |
| INV-2 | Launch timeout 120s — AC never reaches "Running" state | 26 | P2 | Pod 8 (15), Pod 4 (6), Pod 6 (6) | NEEDS_INVESTIGATION | All Assetto Corsa. Pod 8 dominates (58%). Combined with `timeout` event_type (47 total — same pattern, empty error_message). **Hypothesis:** AC SHM never populates (pre-ZL-1 fix, no plugin loaded). Post-ZL-1 deploy should reduce this — re-measure after venue hours. | **Wait for post-deploy data.** If still occurring: SSH to Pod 8 during AC launch, watch `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\apps\python\racecontrol\` exists + AC log.txt for plugin load. |
| INV-3 | Exit code 1 — process exits with known error code | 13 | P2 | Pod 4 (9), Pod 3 (2), Pod 8 (2) | NEEDS_INVESTIGATION | Pod 4: 9/13 are F1 25 (often paired with orphan F1_25.exe crash — F1 25 can't be killed cleanly). Pod 3: 2 are AC Evo (most recent: 2026-04-16). Exit code 1 in AC = content load error (missing track/car/mod). | **Venue:** SSH to Pod 4, check `%LOCALAPPDATA%\F1_25\` for crash logs. For Pod 3 AC Evo: check `%USERPROFILE%\Documents\Assetto Corsa Competizione\logs\` (AC Evo may use ACC paths). |
| INV-4 | Launch timeout 30s — process starts but never detected as running | 8 | P3 | Pod 6 (2), Pod 3 (2), Pod 7 (2) | NEEDS_INVESTIGATION | AC (6) + EA SPORTS WRC (2). Different from INV-2: shorter timeout suggests process detection failure, not SHM issue. May be cold-start (first launch after boot) vs warm (Steam already running). | **Venue:** Test cold-start launch on Pod 6. If reproducible, check if Steam overlay initialization adds delay. Consider increasing 30s timeout to 60s as config change. |

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
| INV-9 | Steam dialog visible after 60s — game never launches | 15 | P2 | 7 pods (all except Pod 8) | Forza Horizon 5 (5), EA SPORTS WRC (5), Le Mans Ultimate (3), AC Evo (1), F1 25 (1) | NEEDS_INVESTIGATION | Steam shows DRM check, update prompt, or login dialog instead of launching game. Already handled by GL-8 fix (`40968ddc`) for the vguiPopupWindow dismiss — but these 15 events are AFTER GL-8 deploy on some pods. **Venue:** Check Steam auto-login config, disable "Check for game updates" in Steam settings per pod. May need `steam://rungameid/` URI with `-silent` flag. |

### Summary: 281 failure events reclassified

| Category | Events | Status |
|----------|--------|--------|
| 2A: Active investigation (INV-1 to INV-4) | 158 | 4 bugs open |
| 2B: Already fixed (INV-5 to INV-8) | 42 | Historical only |
| 2C: Steam dialog (INV-9) | 15 | New — investigate |
| Wave 1 (iRacing config, no exe_path, etc.) | 13 | Tracked in GLC-1/2/3 |
| Already tracked (V-1 through V-8) | 53 | Deployed, need verify |

**Post-deploy crash rate:** 1 event in ~20 minutes of testing (Pod 6 transient). **Need venue-hours data to measure real post-deploy rate.** Most Wave 2 events are pre-deploy — ZL-1/ZL-2 fixes (telemetry plugin) likely eliminate INV-2 timeouts caused by missing SHM data.

**Wave 2 action plan:**
1. ~~Query post-deploy crash rate~~ **DONE** — 1 transient event. Need venue hours for meaningful sample.
2. **Code fix (INV-1):** Add `try_wait()` exit code capture to heartbeat poll path — eliminates the "unknown crash" black hole.
3. **Venue visit:** Check Event Viewer + AC logs on Pods 3/4/8 for INV-1/INV-3. Check Steam config for INV-9.
4. **Re-measure after 24h of venue operation** — INV-2 timeouts may self-resolve with ZL-1 plugin fix.

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
| 2026-04-17 | Wave 2 investigation | 0 | 1 | 281 failure events reclassified into 9 bugs (INV-1 to INV-9). 42 events already fixed (INV-5 to INV-8). New: INV-9 Steam dialog blocking (15 events, 5 games, 7 pods). INV-1 needs code fix (exit code capture). INV-2 likely self-resolves with ZL-1 plugin. Post-deploy: 1 transient crash only. |
