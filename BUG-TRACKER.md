# Bug Tracker — RaceControl

> **Single source of truth for all known bugs.**
> Updated: 2026-04-17 03:15 IST | HEAD: `3bb882cc` | Server: `3bb882cc` | Pods 1-8: `3bb882cc` | POS: `408b04c8` (SAC blocked) | VPS: `8a0f82a1`

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

## Wave 2 — Investigation (150 crash events, unknown root causes)

> **Do this AFTER Wave 1 deploy.** Crash rates will likely drop once configs are fixed and telemetry works. Re-measure first.

| ID | Bug | Count | Severity | Status | Next Step |
|----|-----|-------|----------|--------|-----------|
| INV-1 | Generic "exited unexpectedly" (GL-1) | 111 | P2 | NEEDS_INVESTIGATION | Pick 5 most recent, SSH to pod, check Windows Event Viewer + AC log.txt. Classify: GPU crash / content error / memory / unknown. |
| INV-2 | Launch timeout 120s (GL-2) | 26 | P2 | NEEDS_INVESTIGATION | May be WS latency (935-1628ms on Pods 3-8). Check if timeout events correlate with high round-trip pods. |
| INV-3 | Exit code 1 (GL-4) | 13 | P2 | NEEDS_INVESTIGATION | AC exit code 1 = content error? Check AC log.txt on Pod 4 (9 of 13 events). |
| INV-4 | Launch timeout 30s (GL-7) | 8 | P3 | NEEDS_INVESTIGATION | Config: increase timeout? Or fix underlying slow launch. Check if cold-start vs warm-start matters. |

**Wave 2 action:** After Wave 1 deploy, query `game_launch_events WHERE created_at > '<wave1_deploy_time>'` to get fresh crash rate. Then investigate remaining failures.

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
