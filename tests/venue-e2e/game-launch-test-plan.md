# Venue E2E Game Launch Test Plan

**Purpose:** Test Cluster A open issues (#1 SP+billing, #2 Pod 2 error, #3 game exits after load)
**When:** Next venue session (pods powered on, AC installed, staff available)
**Pre-requisites:** Server on `70626c9c`, pods on `f05e324e` or later, billing test driver with wallet balance

---

## Pre-Test Checks (5 min)

```bash
# 1. Fleet health — all 8 pods connected
curl -s http://192.168.31.23:8080/api/v1/fleet/health | python3 -c "
import json,sys
d=json.load(sys.stdin)
for p in sorted(d, key=lambda x: x.get('pod_number',0)):
  print(f'Pod {p[\"pod_number\"]}: ws={p[\"ws_connected\"]} http={p[\"http_reachable\"]} build={p.get(\"build_id\",\"?\")[:8]}')
"

# 2. No MAINTENANCE_MODE on any pod
for i in 1 2 3 4 5 6 7 8; do
  echo -n "Pod $i: "
  ssh pod$i "if exist C:\RacingPoint\MAINTENANCE_MODE (echo BLOCKED) else (echo ok)" 2>/dev/null || echo "SSH_FAIL"
done

# 3. All agents in Session 1
for i in 1 2 3 4 5 6 7 8; do
  echo -n "Pod $i session: "
  ssh pod$i "tasklist /V /FO CSV | findstr rc-agent" 2>/dev/null | grep -o "Console\|Services"
done

# 4. Server venue_id WARN check (should be fixed after deploy)
ssh server "type C:\RacingPoint\logs\racecontrol-*.jsonl 2>nul | findstr venue_id | findstr WARN" 2>/dev/null | tail -3
```

---

## Test 1: SP Launch with Billing (Issue #1 — P0)

**Goal:** Full customer journey: wallet debit → game launch → timer running → early stop → refund

### Steps

1. **Create test driver** (or use existing with wallet balance ≥ ₹700)
2. **Top up wallet** from POS (:3200/billing) → "Top Up Wallet" → ₹1000 cash
3. **Start billing session:**
   - Click idle pod (Pod 8 canary) on POS billing page
   - Select test driver + 30min tier (₹700)
   - Click Start
   - **VERIFY:** Wallet debited by ₹700, session enters `WaitingForGame`
4. **Launch game from kiosk /staff:**
   - Go to server :3300/kiosk/staff (PIN protected)
   - Select Pod 8 → Assetto Corsa → Spa → ks_ferrari_sf15t → Rookie difficulty
   - Click Launch
   - **VERIFY on Pod 8:** 
     ```bash
     # Agent health shows game launching
     curl -s http://192.168.31.91:8090/health | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'game={d.get(\"game_state\")} sim={d.get(\"active_game\")}')"
     
     # Lock screen shows timer (not blank, not idle)
     curl -s http://192.168.31.91:18924/status | python3 -c "import json,sys; d=json.load(sys.stdin); print(f'lock={d.get(\"lock_screen_state\")} edge={d.get(\"edge_process_count\")}')"
     
     # Race.ini has correct config
     ssh pod8 "type \"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\cfg\race.ini\"" 2>/dev/null | head -20
     ```
   - **VERIFY:** AC game window visible on Pod 8 screen (PHYSICAL CHECK)
   - **VERIFY:** Server billing state = `Active` (timer running)
     ```bash
     curl -s -H "Authorization: Bearer <jwt>" http://192.168.31.23:8080/api/v1/billing/active | python3 -c "import json,sys; d=json.load(sys.stdin); [print(f'pod={s[\"pod_id\"]} state={s[\"state\"]} elapsed={s.get(\"elapsed_seconds\",0)}s') for s in d]"
     ```
5. **End session early** (after ~2 min):
   - From POS billing page → click active pod → Stop Session
   - **VERIFY:** Refund calculated correctly (700 - (2min * rate) = expected refund)
   - **VERIFY:** Wallet balance = 1000 - 700 + refund
   - **VERIFY:** Pod returns to blank/idle screen

### Pass Criteria
- Game launches on first attempt
- Billing timer starts when AC reaches `Running` state (not at launch time)
- Refund matches per-minute billing formula
- Pod screen returns to blank after session end

---

## Test 2: Pod 2 Error Dialog Investigation (Issue #2 — P1)

**Goal:** Reproduce and document the different AC error dialog on Pod 2

### Steps

1. **Launch AC on Pod 2** via same kiosk /staff flow (no billing needed — just game launch)
2. **PHYSICAL OBSERVATION:** Watch Pod 2 screen during launch
   - Does AC start loading?
   - Does an error dialog appear? If so: **photograph the exact error text**
   - Does the game window appear at all?
3. **Compare with Pod 8** (known working):
   ```bash
   # AC content comparison
   ssh pod2 "dir \"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\content\cars\ks_ferrari_sf15t\"" 2>/dev/null
   ssh pod8 "dir \"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\content\cars\ks_ferrari_sf15t\"" 2>/dev/null
   
   # CSP version comparison
   ssh pod2 "type \"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\extension\config.ini\"" 2>/dev/null | head -5
   ssh pod8 "type \"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\extension\config.ini\"" 2>/dev/null | head -5
   
   # Content Manager version
   ssh pod2 "dir \"C:\Users\User\AppData\Local\AcTools Content Manager\"" 2>/dev/null | head -5
   ssh pod8 "dir \"C:\Users\User\AppData\Local\AcTools Content Manager\"" 2>/dev/null | head -5
   ```
4. **Record outcome:** Error text, which software versions differ, hypothesis

### Pass Criteria
- Error reproduced and documented with photo
- Root cause identified (or narrowed to AC content / CSP version / CM version / hardware)

---

## Test 3: Game Exits After Load (Issue #3 — P1)

**Goal:** Reproduce and diagnose game quitting shortly after loading

### Steps

1. **Launch AC on Pods 1-8 sequentially** (or the pods where this was observed)
2. For each pod, after launch:
   ```bash
   # Monitor game PID for 60 seconds
   for i in 1 2 3 4 5 6 7 8 9 10 11 12; do
     ssh pod$N "tasklist /FI \"IMAGENAME eq acs.exe\" /FO CSV /NH" 2>/dev/null
     sleep 5
   done
   ```
3. **If game exits:** Check agent logs for exit code
   ```bash
   ssh pod$N "type C:\RacingPoint\logs\rc-agent-*.jsonl 2>nul | findstr -i \"game.*exit\|acs.*exit\|process.*died\|game.*crash\"" 2>/dev/null | tail -5
   ```
4. **Check Event Viewer for AC crashes:**
   ```bash
   ssh pod$N "wevtutil qe Application /q:\"*[System[Provider[@Name='Application Error'] and TimeCreated[timediff(@SystemTime) <= 3600000]]]\" /c:5 /f:text" 2>/dev/null
   ```
5. **Hypothesis tree:**
   - AC config issue → check race.ini generated correctly
   - CSP incompatibility → check CSP version + FORCE_START=1
   - Anti-cheat conflict → check if any anti-cheat processes are running
   - GPU/driver issue → compare GPU driver versions across pods
   - Shared memory conflict → check if rcpmf_telemetry opens correctly

### Pass Criteria
- Reproduced on specific pods (or confirmed not reproducible)
- Exit code captured and mapped to ERROR-CATALOG
- Root cause narrowed to one of the hypothesis tree branches

---

## Post-Test Actions

1. Update DIAGNOSTIC-PLAYBOOK.md open issues table with results
2. Update project_game_launch_testing.md memory file
3. If bugs found: fix → deploy → re-verify (separate session)
4. If all 3 tests pass: close Cluster A issues
