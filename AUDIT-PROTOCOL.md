# Racing Point Operations Audit Protocol — 20 Phases

**Version:** 1.0 | **Created:** 2026-03-23 | **Author:** James Vowles
**Standing Rule:** Run this audit before shipping any milestone, after major incidents, or weekly during operations.

**Prerequisites:**
- Convert ALL timestamps: racecontrol logs are UTC, operations are IST (UTC+5:30)
- Exclude your own actions from event counts (deploys, test kills)
- Fix during audit, don't just catalog — apply smallest reversible fix immediately

---

## Phase 1: Fleet Inventory
**What:** Every binary, build_id, uptime, process count across all machines.
**Verify:** All builds match expected HEAD. If stale, `touch build.rs` and rebuild.

```bash
# Server .23
curl -s http://192.168.31.23:8080/api/v1/health
curl -s http://192.168.31.23:8090/health

# All 8 pods — rc-agent :8090 + rc-sentry :8091
for IP in 192.168.31.89 .33 .28 .88 .86 .87 .38 .91; do
  curl -s http://$IP:8090/health
  curl -s http://$IP:8091/health
done

# James .27
node --version
curl -s http://localhost:11434/api/tags        # Ollama
curl -s http://localhost:8766/relay/health      # comms-link
curl -s http://localhost:1984/api              # go2rtc

# Bono VPS
curl -s http://100.70.177.44:8080/api/v1/health
```

**Fix loop trigger:** Any build_id mismatch, any service DOWN.

---

## Phase 2: API Data Integrity
**What:** Every API endpoint returns correct DATA, not just HTTP 200.
**Standing rule:** "Verify the EXACT behavior path, not proxies."

```bash
# Auth
SESSION=$(curl -s -X POST http://192.168.31.23:8080/api/v1/terminal/auth \
  -H "Content-Type: application/json" -d '{"pin":"261121"}' | jq -r '.session')

# Test with auth — check response has actual data
curl -s http://192.168.31.23:8080/api/v1/fleet/health  # pods array, ws/http status
curl -s http://192.168.31.23:8080/api/v1/logs?lines=1  # must be .jsonl file, not .log
curl -s http://192.168.31.23:8080/api/v1/logs?level=error&lines=3  # check for active errors
```

**Fix loop trigger:** Empty responses, stale log file name, active errors.

---

## Phase 3: WebSocket Flows
**What:** Dashboard WS, agent WS connections are alive and flowing data.

```bash
# WS endpoint exists (returns 400 = upgrade required)
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/ws/dashboard
curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/ws/agent

# Fleet health — ws_connected must be true for all online pods
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.pods[] | {pod_number, ws_connected}'

# Check for WS latency warnings in logs
curl -s "http://192.168.31.23:8080/api/v1/logs?level=warn&lines=50" | grep "round-trip slow"
```

**Fix loop trigger:** Any pod ws_connected=false, excessive latency warnings.

---

## Phase 4: Cross-Process Data Paths
**What:** Billing → pods, kiosk → racecontrol, cloud sync round-trip.

```bash
# Cloud sync bidirectional
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep -i "sync push\|sync pull\|upserted"

# Both racecontrol builds must match
LOCAL=$(curl -s http://192.168.31.23:8080/api/v1/health | jq -r '.build_id')
CLOUD=$(curl -s http://100.70.177.44:8080/api/v1/health | jq -r '.build_id')
[ "$LOCAL" = "$CLOUD" ] && echo "MATCH" || echo "MISMATCH — redeploy"
```

**Fix loop trigger:** Sync errors, build mismatch, no recent sync activity.

---

## Phase 5: rc-agent Exec Capability
**What:** Every pod can execute commands via rc-agent :8090.

```bash
for IP in 192.168.31.89 .33 .28 .88 .86 .87 .38 .91; do
  curl -s -X POST http://$IP:8090/exec \
    -H "Content-Type: application/json" -d '{"cmd":"hostname"}' | jq '.stdout'
done
```

**Fix loop trigger:** Any pod returns exit_code != 0 or empty stdout.

---

## Phase 6: rc-sentry Crash Detection
**What:** Every pod's rc-sentry is running and can detect + restart rc-agent.
**Standing rule:** "MAINTENANCE_MODE sentinel silently kills all restarts."

```bash
# Check sentry health on all pods
for IP in ...; do curl -s http://$IP:8091/health; done

# Check for stale MAINTENANCE_MODE on all pods
for IP in ...; do
  curl -s -X POST http://$IP:8091/exec \
    -d '{"cmd":"type C:\\RacingPoint\\MAINTENANCE_MODE 2>nul || echo CLEAN"}'
done
```

**Fix loop trigger:** Any sentry DOWN, any MAINTENANCE_MODE file present.

---

## Phase 7: Camera Pipeline
**What:** go2rtc streams → rc-sentry-ai face detection → audit log.
**Standing rule:** "Verify monitoring targets against running system, not docs."

```bash
# go2rtc on James :1984 (NOT 8096)
curl -s http://localhost:1984/api/streams | jq 'length'

# rc-sentry-ai process
tasklist | grep rc-sentry-ai

# Face audit log
wc -l C:/RacingPoint/logs/face-audit.jsonl
tail -1 C:/RacingPoint/logs/face-audit.jsonl
```

**Fix loop trigger:** Stream count mismatch, no face detections, sentry-ai dead.

---

## Phase 8: Process Guard
**What:** Guard scanning, violation count, allowlist coverage.

```bash
# Server-guard violations (should be 0 after allowlist)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=warn&lines=30" | grep "server-guard"

# Pod violations (24h counter)
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.pods[] | {pod_number, violation_count_24h}'

# Verify Variable_dump.exe is NOT in allowlist
grep -i variable_dump racecontrol.toml  # must return nothing
```

**Fix loop trigger:** Any server-guard violations, violation_count > 0 on freshly scanned pods.

---

## Phase 9: AI Healer
**What:** rc-watchdog monitoring all 10 services, failure state clean.

```bash
# Failure state
cat C:/Users/bono/.claude/watchdog-state.json

# Recent healer runs
grep -E "INFO|WARN" C:/Users/bono/.claude/rc-watchdog.log.$(date +%Y-%m-%d) | tail -10

# Verify 10 services checked (not 5)
grep "starting check run" C:/Users/bono/.claude/rc-watchdog.log.$(date +%Y-%m-%d) | tail -1
```

**Fix loop trigger:** Any service in failure state, less than 10 services checked.

---

## Phase 10: Blanking Screen & Lock Screen
**What:** Every pod shows the correct lock/blanking screen with no overlays.
**Standing rule:** "Audit what the CUSTOMER sees, not what the API returns."

```bash
# Check for overlay processes on each pod (M365 Copilot, NVIDIA Overlay, etc.)
for IP in ...; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /V /FO CSV /NH | findstr /C:\"Copilot\" /C:\"NVIDIA Overlay\" /C:\"AMD DVR\" /C:\"WindowsTerminal\" /C:\"OneDrive\" /C:\"Widgets\""}'
done

# Check Edge stacking (>5 msedge.exe = stacking bug)
for IP in ...; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /NH | find /C \"msedge.exe\""}'
done

# Check if lock screen is the foreground window
for IP in ...; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"tasklist /V /FO CSV /NH | findstr /C:\"kiosk\" /C:\"Edge\" /C:\"chrome\""}'
done
```

**Fix loop trigger:** Any overlay process found, Edge count > 5, no kiosk/Edge as foreground.

---

## Phase 11: Preflight Checks
**What:** Verify all rc-agent preflight checks pass on every pod.
**These run before each billing session — audit verifies they're working.**

```bash
# Trigger preflight check report on each pod
for IP in ...; do
  curl -s -X POST http://$IP:8090/exec \
    -d '{"cmd":"curl.exe -s http://127.0.0.1:8090/health"}' | jq '.stdout'
done

# Check preflight results in rc-agent logs
for IP in ...; do
  curl -s -X POST http://$IP:8091/exec \
    -d '{"cmd":"findstr /C:\"preflight\" /C:\"DISP\" /C:\"NET\" /C:\"HW\" C:\\RacingPoint\\rc-agent-.*.jsonl | findstr /C:\"FAIL\""}'
done
```

**Preflight checks verified:**
- DISP-01: Monitor resolution (1920x1080)
- DISP-02: Lock screen/blanking active
- DISP-03: Popup overlay processes killed
- NET-01: Server .23 reachable
- NET-02: WebSocket connected
- HW-01: Wheelbase USB detected (VID:1209 PID:FFB0)
- HW-02: Pedal input responsive
- PROC-01: No duplicate rc-agent instances
- PROC-02: Game process not stale from previous session
- PROC-03: Popup window pre-flight (kill overlay processes)

**Fix loop trigger:** Any FAIL in preflight results.

---

## Phase 12: Sentinel Files & Stale State
**What:** No stale MAINTENANCE_MODE, GRACEFUL_RELAUNCH, or restart sentinels on any pod.

```bash
for IP in ...; do
  curl -s -X POST http://$IP:8091/exec \
    -d '{"cmd":"dir C:\\RacingPoint\\MAINTENANCE_MODE C:\\RacingPoint\\GRACEFUL_RELAUNCH C:\\RacingPoint\\rcagent-restart-sentinel.txt 2>nul || echo CLEAN"}'
done
```

**Fix loop trigger:** Any sentinel file present — delete immediately.

---

## Phase 13: Tailscale & Network
**What:** Tailscale connected on .23, James, all pods. Server can reach Bono VPS.

```bash
# James Tailscale
tailscale status | grep "racing-point-server\|srv1422716"

# Server .23 → Bono VPS
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"curl.exe -s -m 5 http://100.70.177.44:8080/api/v1/health"}'

# Pod-to-server latency
for IP in ...; do
  curl -s -X POST http://$IP:8090/exec -d '{"cmd":"ping -n 1 192.168.31.23"}'
done
```

**Fix loop trigger:** Tailscale offline, server can't reach VPS, high pod latency.

---

## Phase 14: Comms-Link Relay E2E
**What:** Single exec, chain, health — all pass per Ultimate Rule.

```bash
curl -s -X POST http://localhost:8766/relay/exec/run \
  -d '{"command":"node_version","reason":"audit"}'
curl -s -X POST http://localhost:8766/relay/chain/run \
  -d '{"steps":[{"command":"node_version"},{"command":"uptime"}]}'
curl -s http://localhost:8766/relay/health
```

**Fix loop trigger:** Any non-zero exitCode, connectionMode != REALTIME.

---

## Phase 15: Cloud Sync Bidirectional
**What:** Push AND pull verified with actual data counts.

```bash
# Recent sync in venue logs
curl -s "http://192.168.31.23:8080/api/v1/logs?lines=50" | grep "sync push\|sync pull"

# Recent sync errors (should be 0 or old timestamps only)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=error&lines=5"

# Bono side sync
ssh root@100.70.177.44 "pm2 logs racecontrol --lines 10 --nostream 2>&1 | grep sync"
```

**Fix loop trigger:** No recent sync, active sync errors, sync only one direction.

---

## Phase 16: Standing Rules Compliance
**What:** Auto-push clean, Bono notified, rules synced across all 3 files.

```bash
# Unpushed commits
cd racecontrol && git status -sb
cd comms-link && git status -sb

# Rules sync check (cascade update rule in all 3)
grep "data formats" racecontrol/CLAUDE.md comms-link/CLAUDE.md memory/standing-rules.md
```

**Fix loop trigger:** Unpushed commits, rules out of sync.

---

## Phase 17: Log Health
**What:** Log files not bloated, rotation working, no flooding.

```bash
# Server log size today
curl -s -X POST http://192.168.31.23:8090/exec \
  -d '{"cmd":"dir C:\\RacingPoint\\logs\\racecontrol-*.jsonl"}'

# Error rate (should be < 10/hour after fixes)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=error&lines=1" | jq '.filtered'

# WARN rate (should be < 100/hour after allowlist)
curl -s "http://192.168.31.23:8080/api/v1/logs?level=warn&lines=1" | jq '.filtered'
```

**Fix loop trigger:** Log > 50MB/day, error rate > 10/hour, warn rate > 100/hour.

---

## Phase 18: Database Integrity
**What:** All sync tables have required columns, no schema drift.

```bash
# Cloud DB schema check
ssh root@100.70.177.44 "for t in drivers wallets billing_sessions pricing_tiers \
  kiosk_experiences reservations debit_intents kiosk_settings cafe_orders; do \
  echo -n \"\$t: \"; sqlite3 /root/racecontrol/data/racecontrol.db \
  \"PRAGMA table_info(\$t)\" | grep -c updated_at; done"
```

**Fix loop trigger:** Any table missing updated_at.

---

## Phase 19: Billing & Game Launch Readiness
**What:** Pricing tiers loaded, games launchable, billing can start/end.

```bash
# Pricing tiers (cloud-authoritative)
curl -s http://192.168.31.23:8080/api/v1/pricing -H "x-terminal-session: $SESSION"

# Active sessions
curl -s http://192.168.31.23:8080/api/v1/billing/sessions/active -H "x-terminal-session: $SESSION"

# Game list available
curl -s http://192.168.31.23:8080/api/v1/games -H "x-terminal-session: $SESSION"
```

**Fix loop trigger:** No pricing tiers, games endpoint empty.

---

## Phase 20: Full System E2E (Customer Path)
**What:** Walk the complete customer journey: kiosk landing → pod select → PIN → game → telemetry → billing end.
**Standing rule:** "Shipped Means Works For The User."

**Manual steps (requires physical or remote verification):**
1. Open kiosk at `:3300/kiosk` — 8 pod grid visible?
2. Click available pod — PIN modal opens?
3. Open dashboard at `:3200` from POS machine — pods visible with status?
4. Start billing session on a pod — timer starts?
5. Launch game — pod status changes to "launching" → "running"?
6. Telemetry flows — speed/RPM visible on dashboard?
7. End session — pod returns to idle, billing record created?

**Automated smoke test:**
```bash
# Kiosk HTML loads with Next.js markers
curl -s http://192.168.31.23:3300/kiosk | grep -c "__NEXT"

# Dashboard HTML loads
curl -s http://192.168.31.23:3200 | grep -c "__NEXT"

# Both > 0 = frontend is serving
```

**Fix loop trigger:** Any step fails, any page doesn't load, no telemetry flow.

---

## Audit Summary Template

```
AUDIT DATE: _______________
AUDITOR: _______________

| Phase | Status | Notes |
|-------|--------|-------|
| 1. Fleet Inventory | | |
| 2. API Data | | |
| 3. WebSocket Flows | | |
| 4. Cross-Process | | |
| 5. rc-agent Exec | | |
| 6. rc-sentry | | |
| 7. Camera Pipeline | | |
| 8. Process Guard | | |
| 9. AI Healer | | |
| 10. Blanking Screen | | |
| 11. Preflight Checks | | |
| 12. Sentinel Files | | |
| 13. Tailscale/Network | | |
| 14. Comms-Link E2E | | |
| 15. Cloud Sync | | |
| 16. Standing Rules | | |
| 17. Log Health | | |
| 18. DB Integrity | | |
| 19. Billing/Games | | |
| 20. Full E2E | | |

OVERALL: PASS / FAIL / PARTIAL
ISSUES FOUND: ___
FIXED DURING AUDIT: ___
DEFERRED: ___
```
