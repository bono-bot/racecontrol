# Racing Point — Diagnostic Playbook

**Purpose:** Single entry point for diagnosing ANY problem. Follow the steps in order. Do not skip ahead.

**Rule:** Read the relevant section THOROUGHLY before acting. No skimming. One problem at a time.

---

## Reference Docs (read thoroughly, not skim)

Before debugging, read the relevant docs from `docs/`:

| Doc | When to read | Lines |
|-----|-------------|-------|
| `ARCHITECTURE.md` | EVERY session — system overview, crate map, topology, WS protocol, recovery tiers, network map | 434 |
| `ERROR-CATALOG.md` | When you see an error — known root causes + fixes indexed by symptom | ~320 |
| `SERVICE-REFERENCE.md` | When debugging a specific binary — modules, config, ports, common failures | ~420 |
| `DATA-FLOW-DIAGRAMS.md` | When tracing a data flow bug — 9 diagrams showing where data breaks | ~530 |
| `LOG-LOCATIONS.md` | When you need to find evidence — every log file on every machine | ~200 |
| `API.md` | When debugging an endpoint — all ~403 routes across 7 auth tiers | varies |

**Order:** ARCHITECTURE (understand the system) → ERROR-CATALOG (known issue?) → LOG-LOCATIONS (find evidence) → SERVICE-REFERENCE (understand the binary) → DATA-FLOW-DIAGRAMS (trace the flow).

---

## Step 0 — Has This Been Solved Before? (MANDATORY — do this FIRST)

Before investigating ANYTHING:

```bash
# 1. Search memory files for keywords
ls ~/.claude/projects/C--Users-bono/memory/ | grep -i "<keyword>"
# READ the matching files — don't just list them

# 2. Search LOGBOOK for past fixes
grep -i "<keyword>" LOGBOOK.md | head -20

# 3. Search git history
git log --oneline --grep="<keyword>" | head -20

# 4. Search ERROR-CATALOG for known errors
grep -i "<keyword>" docs/ERROR-CATALOG.md
```

**If a fix commit exists:** Read that commit (`git show <hash>`), verify it's deployed (check build_id on target), DONE. Do NOT re-investigate.

---

## Step 1 — Identify the Domain

Which system layer is the problem in? This determines which section to read.

| Symptom | Domain | Go to Section |
|---------|--------|---------------|
| Customer can't start session | **Billing** | Section A |
| Game won't launch / crashes | **Game Launch** | Section B |
| Screen wrong (blank, timer, desktop visible) | **Lock Screen** | Section C |
| Pod offline / not responding | **Pod Health** | Section D |
| Dashboard stale / not updating | **WebSocket** | Section E |
| Deploy failed / wrong build | **Deploy** | Section F |
| Cloud out of sync | **Cloud Sync** | Section G |
| Staff can't login | **Auth** | Section H |
| AI healing not working | **Meshed Intelligence** | Section I |
| Process being killed | **Process Guard** | Section J |

---

## Section A — Billing Problems

### The Customer Journey (verified by Uday, 2026-04-03)

```
1. Customer registers on PWA (app.racingpoint.cloud) or venue kiosk (:3300/register)
2. Staff (Vishal) tops up wallet at POS (:3200/billing) → "Top Up Wallet"
3. Staff clicks idle pod → BillingStartModal → selects driver + tier → Start
4. Wallet debited upfront (FATM-01), session enters WaitingForGame
5. Staff goes to kiosk /staff on server (:3300/kiosk/staff) → configures + launches game
6. Timer starts when game reaches AcStatus::Live (deferred billing, BILL-13)
7. Session ends: timer expiry, manual stop, or 10min inactivity
8. Refund calculated from original wallet_debit_paise (F-05 fix: read before UPDATE)
```

### Key files
- `crates/racecontrol/src/billing.rs` — BillingManager, start/end session
- `crates/racecontrol/src/billing_fsm.rs` — 11-state FSM with CAS protection
- `crates/racecontrol/src/wallet.rs` — Wallet operations, resolve_wallet_owner()
- `crates/racecontrol/src/billing_replay.rs` — Nonce replay protection

### Billing FSM states
```
Pending → WaitingForGame → Active
Active → PausedGamePause | PausedDisconnect | PausedManual | PausedCrashRecovery
Any → Completed | EndedEarly | Cancelled | CancelledNoPlayable
```

### Known billing bugs (RESOLVED)
- **F-05 refund calc** — FIXED `5d1ea000`: reads original value before UPDATE
- **Double-end race** — FIXED: CAS in authoritative_end_session()
- **Stale auto-cancel losing money** — FIXED `8184d4f3`: refund wallet on stale cancel
- **PausedDisconnect killing session on reconnect** — Found in ecosystem audit, status: IN CODE

### Billing debug commands
```bash
# Active billing sessions
curl -s -H "Authorization: Bearer <jwt>" http://192.168.31.23:8080/api/v1/billing/active

# Billing session detail
curl -s -H "Authorization: Bearer <jwt>" http://192.168.31.23:8080/api/v1/billing/<session_id>

# Wallet balance
curl -s -H "Authorization: Bearer <jwt>" http://192.168.31.23:8080/api/v1/wallets/<driver_id>
```

---

## Section B — Game Launch Problems

### The Launch Chain

```
1. API: POST /games/launch { pod_id, sim_type, launch_args }
2. Server: game_launcher.rs validates → creates GameTracker (state: Launching)
3. Server: WS → CoreToAgentMessage::LaunchGame to pod
4. Pod: ac_launcher.rs receives → generates race.ini + assists.ini
5. Pod: Spawns launch-ac.bat (SP: direct acs.exe, MP: Content Manager URI)
6. Pod: game_process monitors PID + shared memory
7. Pod: WS → GameStateUpdate(Launching) [ACK to server]
8. Pod: launch_verifier checks window exists
9. Pod: WS → GameStateUpdate(Running)
10. Server: GameTracker Launching → Running
```

### Key files
- `crates/rc-agent/src/ac_launcher.rs` — AC launch, ini generation, CM/direct modes
- `crates/rc-agent/src/game_process.rs` — Game process monitoring
- `crates/rc-agent/src/launch_verifier.rs` — 4-stage verification
- `crates/rc-agent/src/event_loop.rs` — Main loop, WaitingForLive state
- `crates/rc-agent/src/ws_handler.rs` — WS handler, GAME_LAUNCHING sentinel
- `crates/racecontrol/src/game_launcher.rs` — Server-side launch + GameTracker
- `deploy/launch-ac.bat` — Bat file that spawns the game

### Launch timeouts
| Game | Default | Hard Cap |
|------|---------|----------|
| Assetto Corsa | 120s | 180s |
| F1 25 | 90s | 180s |
| iRacing | 90s | 180s |
| Others | 90s | 180s |

### Common launch failures

| Symptom | Root Cause | Fix | Status |
|---------|-----------|-----|--------|
| Agent dies during launch_ac() | SHM access violation | Fixed in code | DEPLOYED all pods |
| Game launches but wrong config | Serde field name mismatch | Audit kiosk buildLaunchArgs() vs AcLaunchParams | STANDING RULE |
| GameTracker stuck in "Launching" | WS dropped, no ACK | Dynamic timeout auto-errors | DEPLOYED |
| Agent in Session 0, can't launch | Schtask/service context | Kill agent, let RCWatchdog restart | STANDING RULE |
| SP launch via CM fails | CM --race flag doesn't exist | SP now uses direct acs.exe | DEPLOYED `24d181d4` |
| WaitingForLive deadlock | 180s timeout was too long | Reduced to 60s + dead game detection | DEPLOYED `e3d1ae76` |
| controls.ini destroyed | AC overwrites on launch | controls.ini now preserved | DEPLOYED `d3df2992` |

### Launch debug commands
```bash
# Pod game state
curl -s http://<pod_ip>:8090/health | jq '{game_state, active_game, build_id}'

# Debug endpoint (lock screen state, edge count)
curl -s http://<pod_ip>:18924/status

# Server game state for pod
curl -s http://192.168.31.23:8080/api/v1/games/status/<pod_id>

# Force stop stuck game
curl -s -X POST -H "Authorization: Bearer <jwt>" http://192.168.31.23:8080/api/v1/games/stop -d '{"pod_id":"<id>"}'

# Check race.ini on pod (verify config correctness)
ssh User@<pod_tailscale_ip> "type \"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\cfg\race.ini\""
```

### STILL OPEN — Game launch issues needing venue testing
1. **E2E test: SP launch with billing** — needs venue open, full customer flow
2. **Pod 2 error dialog** — different error from other pods during AC launch
3. **Game exits after load** — observed on some pods, needs physical investigation
4. **venue_id column WARN** — server logs: `table pods has no column named venue_id`

---

## Section C — Lock Screen Problems

### Lock screen states
```
Hidden → ScreenBlanked → PinEntry → ActiveSession → SessionSummary → BetweenSessions
```

### Key files
- `crates/rc-agent/src/lock_screen.rs` — State machine, Edge browser management
- Served from `http://127.0.0.1:18923/` (local to pod)

### Common failures

| Symptom | Debug | Fix |
|---------|-------|-----|
| Blank screen during active session | Check if rc-agent restarted (server re-sends BillingStarted on Register) | Automatic since `273db1c` |
| edge_process_count: 0 but state: blanked | `curl http://<pod>:18924/status` | Blanking state now gated on browser launch (e3d1ae76) |
| Customer sees desktop | Agent in Session 0, or Edge failed to launch | Verify `tasklist /V` shows Console session |
| Wrong page displayed | `curl http://<pod>:18923/` to see what's served | Check lock_screen_state in health |

### Debug commands
```bash
# Is Edge running?
curl -s http://<pod_ip>:18924/status | jq '{lock_screen_state, edge_process_count}'

# What page is displayed?
curl -s http://<pod_ip>:18923/

# Is agent in Session 1?
ssh User@<pod_tailscale_ip> "tasklist /V /FO CSV | findstr rc-agent"
# Session column MUST show "Console", NOT "Services"

# Trigger blank screen
curl -s -X POST http://<pod_ip>:8090/exec -H "X-Service-Key: <key>" -d '{"cmd":"echo blank"}'
# Then verify: edge_process_count > 0 within 12s
```

---

## Section D — Pod Health Problems

### Quick fleet check (ALWAYS START HERE)
```bash
curl -s http://192.168.31.23:8080/api/v1/fleet/health | python3 -m json.tool
```

Fields to check per pod: `ws_connected`, `http_reachable`, `build_id`, `uptime_secs`, `crash_loop`

### Pod IPs
| Pod | LAN IP | Tailscale | SSH |
|-----|--------|-----------|-----|
| 1 | 192.168.31.89 | 100.92.122.89 | `ssh pod1` |
| 2 | 192.168.31.33 | 100.105.93.108 | `ssh pod2` |
| 3 | 192.168.31.28 | 100.69.231.26 | `ssh pod3` |
| 4 | 192.168.31.88 | 100.75.45.10 | `ssh pod4` |
| 5 | 192.168.31.86 | 100.110.133.87 | `ssh pod5` |
| 6 | 192.168.31.87 | 100.127.149.17 | `ssh pod6` |
| 7 | 192.168.31.38 | 100.82.196.28 | `ssh pod7` |
| 8 | 192.168.31.91 | 100.98.67.67 | `ssh pod8` |
| POS | 192.168.31.20 | 100.95.211.1 | `ssh pos` |

### Decision tree: Pod not responding

```
Fleet health shows ws_connected: false?
    |
    ├── http_reachable: true → WS auth issue. Check server logs for "Pod N registered" frequency
    |
    ├── http_reachable: false → Pod may be off or agent dead
    |       |
    |       ├── Can you ping it? (use check-alive.sh, NOT single ping)
    |       |       |
    |       |       ├── YES → Agent is dead. Check:
    |       |       |       1. MAINTENANCE_MODE sentinel
    |       |       |       2. rc-sentry health: curl http://<pod>:8091/health
    |       |       |       3. Recovery: kill agent → RCWatchdog auto-restarts in Session 1
    |       |       |
    |       |       └── NO → Pod is powered off or network issue
    |       |               1. Try Tailscale IP
    |       |               2. If both fail: WoL or physical check
    |       |
    |       └── Don't use single ping to conclude "powered off" — use wait-for-pods.sh
    |
    └── crash_loop: true → 3+ restarts in 5min
            1. Reboot pod FIRST (clears corrupted OS state)
            2. If persists after reboot: check crash-seh.log, Event Viewer
```

### Sentinel files (check these FIRST when agent won't start)
```bash
ssh User@<pod_tailscale_ip> "dir C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\OTA_DEPLOYING C:\RacingPoint\GAME_LAUNCHING 2>nul"
```

### 5 restart mechanisms (know which one is active)
1. **HKLM Run** → `start-rcagent.bat` (boot/login) — kills, cleans, swaps hash binary, starts
2. **RCWatchdog service** → `spawn_in_session1()` (agent dead >5s) — starts exe directly
3. **rc-sentry** → delegates to watchdog
4. **StartRCAgent schtask** → runs bat (WARNING: may start in Session 0)
5. **Self-restart** → GRACEFUL_RELAUNCH sentinel → exit → watchdog picks up

---

## Section E — WebSocket / Dashboard Problems

### WS Architecture
- Pod agents connect to: `wss://server:8080/ws/agent`
- Dashboards connect to: `ws://server:8080/ws/dashboard`
- Mesh Intelligence: `ws://server:8080/ws/ai-channel`

### Common WS failures

| Symptom | Root Cause | Debug |
|---------|-----------|-------|
| Dashboard shows stale data | WS churn from stale frontend JS | Check `dashboard_ws_churn.connects_per_min` — >10 = stale |
| Dashboard "Connecting..." | WS auth failure or server down | Check server health + browser console |
| Pod data not updating | Pod WS disconnected | Fleet health → ws_connected for that pod |
| Game launch returns ok but nothing happens | WS dropped between queue and delivery | GameTracker timeout will catch it |

### Fix for WS churn
```bash
# Check churn rate
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.[0].dashboard_ws_churn'

# If connects_per_min > 10: rebuild ALL frontends
# kiosk, web, admin — then redeploy
```

---

## Section F — Deploy Problems

### Deploy order (NEVER skip steps)
1. `touch crates/<crate>/build.rs` (refresh GIT_HASH)
2. `cargo build --release --bin <binary>`
3. Security gate: `bash gate-check.sh`
4. Canary: Pod 8 first → verify build_id → verify specific fix
5. Fleet: Pods 1-7
6. Server: `deploy-server.sh` v3.0 (8-step, auto-rollback)
7. Frontends: rebuild ALL 3 (kiosk, web, admin)
8. Cloud: git push → Bono relay `git_pull` → rebuild → verify BOTH

### Key deploy files
- `deploy-staging/deploy-server.sh` — Server deploy script (MMA-hardened)
- `scripts/deploy-pod-agent.sh` — Pod agent deploy
- `deploy-staging/` — Build staging area + HTTP server

### Common deploy failures — see ERROR-CATALOG.md Section "Deploy Errors"

---

## Section G — Cloud Sync Problems

### Architecture
- Venue = LOCAL authority (billing, laps, game state)
- Cloud = CLOUD authority (drivers, pricing, catalog)
- Sync: relay mode (2s) + HTTP fallback (30s)
- 15 tables synced

### Debug commands
```bash
# Venue health
curl -s http://192.168.31.23:8080/api/v1/health | jq '{status, build_id}'

# Cloud health
curl -s http://srv1422716.hstgr.cloud:8080/api/v1/health | jq '{status, build_id}'

# Build parity check
echo "Venue:" && curl -s http://192.168.31.23:8080/api/v1/health | jq .build_id
echo "Cloud:" && curl -s http://srv1422716.hstgr.cloud:8080/api/v1/health | jq .build_id

# Sync status in server logs
grep "cloud_sync" racecontrol-*.jsonl | tail -5
```

---

## Section H — Auth Problems

### Auth endpoints
| Role | Login Endpoint | Token |
|------|---------------|-------|
| Superadmin | `/api/v1/auth/admin-login` (Argon2 PIN hash) | JWT (superadmin) |
| Staff | `/api/v1/staff/validate-pin` (4-digit daily rotate) | JWT (cashier/manager) |
| Customer | PWA registration + WhatsApp OTP | JWT (customer) |
| Pod-to-server | PSK in WS query param | WS session |
| Service-to-service | `X-Service-Key` header | Per-request |

### Common auth failures — see ERROR-CATALOG.md Section "Authentication Errors"

---

## Section I — Meshed Intelligence Problems

### MI Status (from ecosystem audit 2026-04-04)
- **Tier 0 (Audit KB):** IMPLEMENTED — `audit_known_issues` table
- **Tier 1 (Deterministic):** FULLY IMPLEMENTED — self_heal.rs
- **Tier 2 (Knowledge Base):** FULLY IMPLEMENTED — knowledge_base.rs + SQLite KB
- **Tier 3 (Local Ollama):** FULLY IMPLEMENTED — but Ollama URL may be wrong on deployed pods
- **Tier 4 (Cloud AI):** STUB — OpenRouter wrapper only in rc-agent
- **Tier 5 (Escalation):** STUB — notification skeleton only
- **Mesh gossip:** INCOMPLETE — skeleton, no fleet consensus

### Known MI issues
- Ollama URL wrong on deployed pods (ecosystem audit finding)
- No OpenRouter key on pods = zero Tier 4
- Knowledge base may be empty on most pods

---

## Section J — Process Guard Problems

### Quick check
```bash
# Violation count on all pods
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.[] | {pod_number, violation_count_24h}'
```

If violation_count_24h > 100 on all pods = empty allowlist (server was down at boot)
→ Wait 5 min for re-fetch, or restart rc-agent

---

## Current System Status (UPDATE THIS SECTION after every session)

**Last updated:** 2026-04-06 18:34 IST

### Deployed Builds
| Target | Build | Date |
|--------|-------|------|
| Server .23 | `70626c9c` | 2026-04-06 |
| Pods 1-8 | `f05e324e` | 2026-04-06 |
| POS .20 | `c31997c0` | unknown |
| Cloud (Bono VPS) | `8a94395e` | 2026-04-06 |

### Open Issues (actively broken or unverified)

| # | Issue | Domain | Priority | Status | Memory File |
|---|-------|--------|----------|--------|-------------|
| 1 | E2E SP launch with billing not tested | Game Launch | P0 | NEEDS VENUE | project_game_launch_testing.md |
| 2 | Pod 2 different AC error dialog | Game Launch | P1 | UNVERIFIED | project_game_launch_testing.md |
| 3 | Game exits after load on some pods | Game Launch | P1 | UNVERIFIED | project_game_launch_testing.md |
| 4 | venue_id column WARN in server logs | Server | P2 | OPEN | — |
| 5 | 23+ lock-across-await remain in codebase | Code Quality | P2 | KNOWN | project_launch_resilience_gaps.md |
| 6 | MI Tier 3 Ollama URL wrong on pods | MI | P2 | FOUND IN AUDIT | project_ecosystem_audit_20260404.md |
| 7 | MI Tier 4/5 mostly stub | MI | P3 | BY DESIGN | — |
| 8 | Mesh gossip incomplete | MI | P3 | BY DESIGN | — |
| 9 | PausedDisconnect kills session on reconnect | Billing | P1 | IN CODE | project_ecosystem_audit_20260404.md |
| 10 | rc-sentry, rc-sentry-ai not audited | Audit | P2 | GAP | project_ecosystem_audit_20260404.md |

### Resolved Issues (recently closed — reference only)

| Issue | Resolution | Commit | Date |
|-------|-----------|--------|------|
| Agent dies during AC launch | SHM access violation fix | multiple | 2026-04-06 |
| Watchdog Session 0 broken | CreateProcessAsUser fix | multiple | 2026-04-06 |
| WaitingForLive 180s deadlock | 60s timeout + dead game detection | `e3d1ae76` | 2026-04-06 |
| Blanking state race condition | Browser-gated state change | `e3d1ae76` | 2026-04-06 |
| SP launch via CM failing | Direct acs.exe for SP mode | `24d181d4` | 2026-04-06 |
| F-05 refund calculation bug | Read before UPDATE | `5d1ea000` | 2026-03-28 |
| Pod 6 NIC power management | Realtek fix all 8 pods | — | 2026-04-03 |
| start-rcagent.bat redirect bug | Removed 2>> redirect | — | 2026-04-03 |

---

## GSD Milestone Status (from `.planning/ROADMAP.md`)

**Check ROADMAP.md at session start** — incomplete GSD phases are unfinished features that may explain bugs.

### Recently Completed: v43.0 Self-Audit & Visual Regression (2026-04-06)

All 4 phases (325-328) DONE: page crawler, visual regression tests, enforcement hooks, AI self-audit.

### Completed: v40.0 Game Launch Reliability (4/4 phases done — SHIPPED)

| Phase | Status | What's left |
|-------|--------|-------------|
| 311: Launch-Billing Guard | DONE | — |
| 312: WS ACK Protocol | DONE | — |
| 313: GameState Resilience | DONE | All 4 success criteria verified against deployed code. ROADMAP + plan checkboxes closed 2026-04-06. |
| 314: Billing Atomicity | DONE | — |

### Active: v41.0 Game Intelligence System (4/6 phases done)

| Phase | Status | What's left |
|-------|--------|-------------|
| 315: Shared Types | DONE | — |
| 316: Agent Content Scanner | DONE | — |
| **317: Server Inventory** | **PARTIAL** | Plan 317-01: `pod_game_inventory` table, `fleet_validity` field, WS handlers for GameInventoryUpdate + ComboValidationReport |
| 318: Launch Intelligence | DONE | — |
| **319: Reliability Dashboard** | **PARTIAL** | Plan 319-01: fleet game matrix page (`/games/reliability`), combo reliability table with red highlight < 70% |
| 320: Kiosk Game Filtering | DONE | — |

### Queued (not started)

| Milestone | Phases | Depends on |
|-----------|--------|------------|
| v42.0 Meshed Intelligence Migration | 321-324 | v41.0 |

### Also incomplete

- v39.0 Phase 310: Plan 310-02 (dashboard trace endpoint) — DEFERRED

### How GSD connects to debugging

Incomplete GSD phases can **cause** bugs in the open issues list:
- Phase 317 incomplete → kiosk may show games that aren't installed → silent launch failure (D2)
- Phase 313 unverified → GameTracker may still get stuck in edge cases (D2)
- Phase 319 incomplete → no visibility into which combos are reliable (makes D2 debugging harder)

**Rule:** Fix D2 bugs first, then complete the GSD phases that prevent those bugs from recurring.

---

## The Rules for Using This Playbook

1. **Step 0 is not optional.** Check memory/LOGBOOK/git BEFORE investigating.
2. **Read the whole section.** Don't skim for the answer you expect to find.
3. **One problem at a time.** Finish diagnosing + fixing + verifying issue A before starting issue B.
4. **Update the Open Issues table** after every fix. If you fixed it, move it to Resolved.
5. **Verify the exact behavior, not proxies.** Health 200 ≠ fixed. Build_id match ≠ fixed.
6. **If the fix exists in git but isn't deployed, that's NOT fixed.** Deploy + verify = fixed.
