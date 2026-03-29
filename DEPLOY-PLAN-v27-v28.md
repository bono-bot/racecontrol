# Coordinated Deploy Plan — v27.0 + v28.0 + 27 Security Fixes

**Created:** 2026-03-29 19:30 IST (Sunday)
**Deploy Window:** Monday 2026-03-30 06:00-10:00 IST (before venue opens ~12:00)
**Git HEAD:** `76977b6e` (dd05b016 after ROADMAP fix)
**Gap:** 155 commits behind production, 62 server code changes, 10 DB migrations, 6 protocol changes

---

## Phase 0: Build & Stage (Sunday Night — NO deploy lock constraint)

### 0.1 Clean Build
```bash
cd C:/Users/bono/racingpoint/racecontrol
git stash  # ensure clean tree
git log -1 --oneline  # confirm HEAD = 76977b6e

# Force fresh GIT_HASH embed
touch crates/racecontrol/build.rs
touch crates/rc-agent/build.rs

# Build with locked deps
export PATH="$PATH:/c/Users/bono/.cargo/bin"
cargo build --release --bin racecontrol
cargo build --release --bin rc-agent
```

### 0.2 Stage Binaries with Hash Names
```bash
HASH=$(git rev-parse --short HEAD)
cp target/release/racecontrol.exe deploy-staging/racecontrol-${HASH}.exe
cp target/release/rc-agent.exe deploy-staging/rc-agent-${HASH}.exe

# Verify build_id embedded
strings deploy-staging/racecontrol-${HASH}.exe | grep "$HASH"
strings deploy-staging/rc-agent-${HASH}.exe | grep "$HASH"

# SHA256 for integrity
sha256sum deploy-staging/racecontrol-${HASH}.exe > deploy-staging/racecontrol-${HASH}.sha256
sha256sum deploy-staging/rc-agent-${HASH}.exe > deploy-staging/rc-agent-${HASH}.sha256
```

### 0.3 Check Frontend Changes
```bash
# Kiosk changes since last build (Mar 28)
git log --oneline --since="2026-03-28" -- kiosk/
# Web changes since last build (Mar 29 15:54)
git log --oneline --since="2026-03-29T10:00:00" -- web/
# If ANY changes: rebuild
# cd kiosk && npm run build
# cd web && npm run build
```

### 0.4 Verify Tests Pass
```bash
cargo test -p rc-common --lib
cargo test -p racecontrol-crate --lib
# Must be 854+ tests, 0 failures
```

### 0.5 Start HTTP Server for Pod Downloads
```bash
# Kill any existing staging server
pkill -f 'http.server 18889' 2>/dev/null || true
cd deploy-staging && python -m http.server 18889 &
```

---

## Phase 0.9: Deploy Lock & Monitoring Setup (Sunday Night)

### 0.9.1 Acquire Deploy Lock
```bash
# Use comms-link deploy-lock (ESM, atomic fs.openSync 'wx')
cd /c/Users/bono/racingpoint/comms-link
node shared/deploy-lock.js acquire "v27+v28 coordinated deploy" james
# If LOCKED by another actor: ABORT. Do not proceed.
```

### 0.9.2 Start Monitoring Tail
```bash
# On Monday morning, before Phase 1:
# Terminal 1: Server logs
ssh ADMIN@100.125.108.37 "cd /d C:\RacingPoint && type racecontrol.jsonl | find /c /v \"\""
# Terminal 2: Fleet health poll (every 30s)
watch -n 30 'curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq "[.[] | {pod: .pod_number, ws: .ws_connected, build: .build_id}]"'
# Terminal 3: Error monitor
ssh ADMIN@100.125.108.37 "cd /d C:\RacingPoint && findstr /C:ERROR racecontrol.jsonl | tail -5"
```

### 0.9.3 Release Deploy Lock (after Phase 10 Go decision)
```bash
node shared/deploy-lock.js release
```

---

## Phase 1: Monday Morning Pre-Flight (06:00 IST)

### 1.1 Fresh DB Backup (CRITICAL — taken MORNING, not night before)
```bash
# WAL checkpoint first to flush pending writes
ssh ADMIN@100.125.108.37 "cd /d C:\RacingPoint && sqlite3 racecontrol.db 'PRAGMA wal_checkpoint(TRUNCATE);'"

# Copy DB + WAL to James
scp ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.db deploy-staging/backup-pre-deploy-racecontrol.db
scp ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.db-wal deploy-staging/backup-pre-deploy-racecontrol.db-wal 2>/dev/null || true

# Verify backup integrity
sqlite3 deploy-staging/backup-pre-deploy-racecontrol.db "PRAGMA integrity_check;"
# Must output: ok
```

### 1.2 Verify System Health Before Changes
```bash
# Server health
curl -s http://192.168.31.23:8080/api/v1/health | jq '.build_id'
# Expected: ccbabd15

# Fleet health — all 8 pods + POS
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.[].ws_connected'

# Cloud VPS
curl -s http://localhost:8766/relay/health
```

### 1.3 Notify Staff
```bash
# WhatsApp notification
cd /c/Users/bono/racingpoint/comms-link
COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" \
COMMS_URL="ws://srv1422716.hstgr.cloud:8765" \
node send-message.js "MAINTENANCE: System update starting. Venue will be ready before opening."
```

---

## Phase 2: Session Drain (06:15 IST)

### 2.1 Enable Maintenance Mode
```bash
# Check active sessions
curl -s http://192.168.31.23:8080/api/v1/billing/active | jq '.count'

# If count > 0: wait. At 06:00 IST there should be 0 sessions (venue closed since 23:00).
# If somehow active: DO NOT PROCEED until count = 0.
```

### GATE: Active sessions = 0. If not 0 after 15 min, ABORT and reschedule.

---

## Phase 3: DB Migrations (06:20 IST)

### 3.1 Migrations Run Automatically
SQLite migrations in racecontrol use `CREATE TABLE IF NOT EXISTS` + `ALTER TABLE ADD COLUMN` wrapped in error handling. They run at binary startup. No separate migration step needed — the new binary will run them on first start.

**However:** To be safe, verify the old binary is STOPPED before new binary starts (standing rule: confirmed kill before swap).

### GATE: Old server process confirmed dead (tasklist + port 8080 free).

---

## Phase 4: Server Deploy (06:25 IST)

### 4.1 Execute deploy-server.sh
```bash
bash scripts/deploy-server.sh
```
This script handles: connectivity check → download → confirmed kill → atomic swap → start → build_id verify → smoke test → auto-rollback on failure.

### 4.2 Post-Server Verification
```bash
# Build ID must match HEAD
curl -s http://192.168.31.23:8080/api/v1/health | jq '.build_id'
# Expected: 76977b6e (or current HEAD short hash)

# New modules initialized
# Check logs for: "driver-rating worker started", "TelemetryWriter started", etc.

# DB migrations ran
# Check logs for: "Mesh intelligence tables initialized", new table creation
```

### 4.3 Credential Restart (Server)
```bash
# Restart server to pick up rotated JWT/relay/sentry keys
# deploy-server.sh already restarts — just verify keys are active
curl -s http://192.168.31.23:8080/api/v1/health
```

### GATE: build_id matches, health OK, no errors in log. If FAIL: auto-rollback in deploy-server.sh.

---

## Phase 5: Cloud VPS Deploy (06:35 IST)

### 5.1 Build on VPS
```bash
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"command":"git_pull","reason":"v27+v28 deploy"}'

# Build on VPS (Linux)
ssh root@100.70.177.44 "cd /root/racecontrol && git pull && touch crates/racecontrol/build.rs && cargo build --release 2>&1 | tail -5"

# Restart
ssh root@100.70.177.44 "pm2 restart racecontrol"

# Verify
curl -s http://100.70.177.44:8080/api/v1/health | jq '.build_id'
```

### 5.2 Restart comms-link + WhatsApp bot on VPS
```bash
ssh root@100.70.177.44 "pm2 restart comms-link && pm2 restart whatsapp-bot"
```

### GATE: VPS build_id matches, comms-link healthy, WhatsApp bot responding.

---

## Phase 6: Rolling Pod Update (06:50 IST)

### 6.1 Wave A — Pods 1-2 (Canary)
```bash
# Download binary on each pod
for pod_ip in 192.168.31.89 192.168.31.33; do
  # Download via rc-sentry
  curl -s -X POST "http://${pod_ip}:8091/exec" \
    -H "Content-Type: application/json" \
    -H "X-Service-Key: <sentry_key>" \
    -d "{\"cmd\":\"curl.exe -s -o C:\\\\RacingPoint\\\\rc-agent-${HASH}.exe http://192.168.31.27:18889/rc-agent-${HASH}.exe\",\"timeout_ms\":30000}"
done

# Kill rc-agent on each pod (RCWatchdog auto-restarts with new binary)
for pod_ip in 192.168.31.89 192.168.31.33; do
  curl -s -X POST "http://${pod_ip}:8091/exec" \
    -H "Content-Type: application/json" \
    -H "X-Service-Key: <sentry_key>" \
    -d "{\"cmd\":\"taskkill /F /IM rc-agent.exe\",\"timeout_ms\":10000}"
done

# Wait 30s for RCWatchdog restart
sleep 30

# Verify Wave A
for pod_ip in 192.168.31.89 192.168.31.33; do
  echo "Pod $pod_ip:"
  curl -s "http://${pod_ip}:8090/health" | jq '.build_id'
done
```

### HEALTH GATE: Both pods show correct build_id + WS connected. If FAIL: revert these 2 pods, stop rolling update.

### 6.2 Wave B — Pods 3-4
(Same procedure as Wave A with IPs .28, .88)

### 6.3 Wave C — Pods 5-6
(Same with IPs .86, .87)

### 6.4 Wave D — Pods 7-8
(Same with IPs .38, .91)

### GATE: All 8 pods show correct build_id. fleet/health shows 8/8 ws_connected.

---

## Phase 7: POS Update (07:20 IST)

```bash
# POS uses rc-pos-agent (same rc-agent binary, different config)
scp deploy-staging/rc-agent-${HASH}.exe POS@192.168.31.20:C:/RacingPoint/rc-agent-${HASH}.exe

# SSH to POS and swap
ssh POS@192.168.31.20 "cd /d C:\RacingPoint && taskkill /F /IM rc-agent.exe & ping -n 4 127.0.0.1 >nul & del rc-agent-prev.exe & ren rc-agent.exe rc-agent-prev.exe & ren rc-agent-${HASH}.exe rc-agent.exe"

# Restart via schtasks
ssh POS@192.168.31.20 "schtasks /Run /TN StartRCAgent"

# Verify
curl -s http://192.168.31.20:8090/health | jq '.build_id'
```

### GATE: POS build_id matches, kiosk billing page loads.

---

## Phase 8: Firewall Deploy (07:30 IST)

```bash
# Copy firewall script to all pods and execute
for pod_ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do
  # Download
  curl -s -X POST "http://${pod_ip}:8091/exec" -H "Content-Type: application/json" -H "X-Service-Key: <sentry_key>" \
    -d "{\"cmd\":\"curl.exe -s -o C:\\\\RacingPoint\\\\pod-firewall-rules.ps1 http://192.168.31.27:18889/pod-firewall-rules.ps1\",\"timeout_ms\":15000}"
  # Apply
  curl -s -X POST "http://${pod_ip}:8091/exec" -H "Content-Type: application/json" -H "X-Service-Key: <sentry_key>" \
    -d "{\"cmd\":\"powershell -ExecutionPolicy Bypass -File C:\\\\RacingPoint\\\\pod-firewall-rules.ps1\",\"timeout_ms\":30000}"
done

# James machine firewall
powershell -ExecutionPolicy Bypass -File scripts/james-firewall-rules.ps1
```

---

## Phase 9: E2E Smoke Test (07:40 IST)

### 9.1 Critical Path Tests
| Test | Command | Expected |
|------|---------|----------|
| Health | `curl -s http://192.168.31.23:8080/api/v1/health` | build_id = HEAD hash |
| Fleet | `curl -s http://192.168.31.23:8080/api/v1/fleet/health \| jq '.[].ws_connected'` | 8x true |
| Billing start | Create test session via admin dashboard | Session created, timer starts |
| Billing end | End test session | Refund calculated, wallet updated |
| Coupon | Reserve → redeem → cancel flow | FSM transitions correct |
| Leaderboard | `curl -s http://192.168.31.23:8080/api/v1/leaderboard/records` | Returns track records |
| Telemetry | Check telemetry.db has recent entries | Writes flowing |
| Input validation | Submit bad phone number | 400 rejection |
| Exec blocklist | Send `FOR /F` command | Blocked |
| WhatsApp | Send test message | Delivered |

### 9.2 New Module Verification
| Module | Check |
|--------|-------|
| driver_rating | Exists in DB, worker started (log) |
| telemetry_store | telemetry.db created, writer running |
| input_validation | Bad input rejected on register |
| notification_outbox | Table exists, drain task running |

### GATE: All tests pass. If ANY critical test fails: rollback.

---

## Phase 10: Go/No-Go (08:00 IST)

- [ ] All build_ids match across 11 targets (server, 8 pods, POS, VPS)
- [ ] 0 errors in server logs (last 10 min)
- [ ] WS connected: 8/8 pods
- [ ] E2E billing flow works
- [ ] Security fixes active (spot check auth, blocklist, CAS)
- [ ] Firewall rules persistent (schtasks registered on pods)
- [ ] Credentials rotated and services restarted

**GO:** Enable venue operations
**NO-GO:** Execute full rollback (Phase R below)

---

## Phase R: Emergency Rollback

### R.1 Revert Server
```bash
ssh ADMIN@100.125.108.37 "cd /d C:\RacingPoint && taskkill /F /IM racecontrol.exe & ping -n 4 127.0.0.1 >nul & ren racecontrol.exe racecontrol-failed.exe & ren racecontrol-prev.exe racecontrol.exe & schtasks /Run /TN StartRCDirect"
```

### R.2 Revert DB (if schema incompatible with old binary)
```bash
scp deploy-staging/backup-pre-deploy-racecontrol.db ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.db
# Restart server again after DB restore
```

### R.3 Revert Pods (only if updated)
```bash
# Each pod has rc-agent-prev.exe — RCWatchdog uses start-rcagent.bat which swaps
# Kill current agent → watchdog starts prev
```

### R.4 Revert VPS
```bash
ssh root@100.70.177.44 "cd /root/racecontrol && git checkout ccbabd15 && cargo build --release && pm2 restart racecontrol"
```

---

## Timing Summary

| Phase | Start IST | Duration | Cumulative |
|-------|-----------|----------|------------|
| 1. Pre-flight | 06:00 | 15 min | 06:15 |
| 2. Session drain | 06:15 | 5 min | 06:20 |
| 3. DB migrations | 06:20 | auto | 06:20 |
| 4. Server deploy | 06:25 | 10 min | 06:35 |
| 5. VPS deploy | 06:35 | 15 min | 06:50 |
| 6. Pod rolling (4 waves) | 06:50 | 30 min | 07:20 |
| 7. POS update | 07:20 | 10 min | 07:30 |
| 8. Firewall | 07:30 | 10 min | 07:40 |
| 9. E2E smoke | 07:40 | 20 min | 08:00 |
| 10. Go/No-Go | 08:00 | — | — |
| **Buffer before venue** | 08:00-12:00 | **4 hours** | — |
