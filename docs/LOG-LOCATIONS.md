# Racing Point — Log Locations

Where to find logs on each machine. Critical for debugging.

**WARNING:** All Rust tracing logs use UTC timestamps. Operations are in IST (UTC+5:30). Always convert before correlating events.

---

## Server (.23)

| File | Service | Content | Format | Rotation |
|------|---------|---------|--------|----------|
| `C:\RacingPoint\racecontrol-{date}.jsonl` | racecontrol | Main server logs | JSONL (structured) | 100MB/24h, 30-day |
| Console stdout | racecontrol | Live output (different filter!) | Plain text | None |
| `C:\RacingPoint\data\racecontrol.db` | racecontrol | SQLite DB (WAL mode) | Binary | Backup pipeline |
| `C:\RacingPoint\recovery-log.jsonl` | racecontrol | Recovery decisions | JSONL | Append |
| Scheduled Task logs | kiosk/web/admin | Next.js app output | Varies | Per schtask |
| Windows Event Viewer | OS | System/Application events | XML | 20MB default |

### Server Console vs JSONL

The console and JSONL file use **different tracing filters**. Process guard violations flood the console but may NOT appear in JSONL. When checking for WARNs, check BOTH:
```bash
# JSONL (may miss some targets)
findstr WARN C:\RacingPoint\racecontrol-*.jsonl

# Console (if captured — check PowerShell watchdog output)
# The watchdog captures stdout but may truncate
```

---

## Pods 1-8

| File | Service | Content | Format | Rotation |
|------|---------|---------|--------|----------|
| `C:\RacingPoint\rc-agent-{date}.jsonl` | rc-agent | Main pod agent logs | JSONL | 100MB/24h, 30-day |
| `C:\RacingPoint\rc-bot-events.log` | rc-agent | Panic events (sync write) | Plain text | None |
| `C:\RacingPoint\crash-seh.log` | rc-agent | Windows SEH exceptions | Plain text | None |
| `C:\RacingPoint\termination.log` | rc-agent | Process termination events | Plain text | None |
| `C:\RacingPoint\process-guard.log` | rc-agent | Process guard violations | Plain text | 512KB |
| `C:\RacingPoint\startup.log` | rc-agent | Boot phase tracking | Plain text | Per-boot |
| `C:\RacingPoint\watchdog.log` | rc-sentry | Watchdog/sentry logs | Plain text | Rolling daily |
| `C:\RacingPoint\recovery-pod.jsonl` | rc-sentry | All restart decisions | JSONL | Append |
| `C:\RacingPoint\flags-cache.json` | rc-agent | Feature flag cache | JSON | Atomic write |
| `C:\RacingPoint\sentry-flags.json` | rc-agent | Flags for rc-sentry | JSON | Atomic write |
| `C:\RacingPoint\knowledge-base.db` | rc-agent | Tier 2 KB solutions | SQLite | Persistent |
| `C:\RacingPoint\MAINTENANCE_MODE` | rc-agent | Sentinel (crash loop) | JSON | Manual clear |
| Windows Event Viewer | OS | ntdll crashes, service failures | XML | 20MB |

### Pod Debug Endpoints (Live)

| Endpoint | Content |
|----------|---------|
| `http://<pod>:8090/health` | Uptime, build_id, WS status, exec slots |
| `http://<pod>:18924/status` | Lock screen state, edge_process_count, last_launch_error |
| `http://<pod>:18924/page` | Current HTML being displayed |
| `http://<pod>:18924/screenshot` | PNG screenshot |
| `http://<pod>:8090/screenshot?method=dxgi` | D3D game screenshot (during gameplay) |

---

## James (.27)

| File | Service | Content | Format | Rotation |
|------|---------|---------|--------|----------|
| `C:\Users\bono\.claude\rc-watchdog.log` | rc-watchdog | Fleet healer logs | Plain text | None |
| `C:\RacingPoint\mma-diagnosis.json` | rc-watchdog | MMA diagnosis results | JSON | Overwrite |
| `C:\RacingPoint\MMA_DIAGNOSING` | rc-watchdog | MMA in-progress sentinel | JSON | 120s TTL |
| `C:\Users\bono\racingpoint\process-guard-james.log` | rc-process-guard | James process violations | Plain text | 512KB |
| `C:\Users\bono\racingpoint\recovery-log.jsonl` | rc-watchdog | Recovery decisions | JSONL | Append |
| comms-link stdout | comms-link | Relay messages | Plain text | None |
| Ollama logs | Ollama | LLM inference | Varies | Via Ollama |
| go2rtc logs | go2rtc | Camera stream status | stdout | None |

---

## POS PC (.20)

| File | Service | Content | Format |
|------|---------|---------|--------|
| `C:\RacingPoint\rc-agent-{date}.jsonl` | rc-agent | Same as pods | JSONL |
| `C:\RacingPoint\watchdog.log` | rc-sentry | Same as pods | Plain text |
| `C:\RacingPoint\recovery-pod.jsonl` | rc-sentry | Restart decisions | JSONL |

---

## Bono VPS (Cloud)

| Location | Service | Content |
|----------|---------|---------|
| `pm2 logs racecontrol` | racecontrol | Server logs (stdout + stderr) |
| `pm2 logs comms-link` | comms-link | Relay tunnel logs |
| `pm2 logs kiosk` | kiosk | Kiosk app logs |
| `pm2 logs web` | web | Dashboard logs |
| `pm2 logs admin` | admin | Admin app logs |
| `/var/log/nginx/access.log` | nginx | HTTP access logs |
| `/var/log/nginx/error.log` | nginx | Proxy errors |

**Access:** `ssh root@100.70.177.44` or via comms-link relay: `curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"pm2_logs","args":"racecontrol --lines 50"}'`

---

## Quick Debugging Commands

### "What happened on Pod N?"

```bash
# 1. Check if rc-agent is running and in Session 1
ssh User@<pod_tailscale_ip> "tasklist /V /FO CSV | findstr rc-agent"

# 2. Recent logs (last 50 lines)
ssh User@<pod_tailscale_ip> "type C:\RacingPoint\rc-agent-2026-04-06.jsonl | findstr /C:\"WARN\" /C:\"ERROR\""

# 3. Crash logs
ssh User@<pod_tailscale_ip> "type C:\RacingPoint\crash-seh.log"

# 4. Restart history
ssh User@<pod_tailscale_ip> "type C:\RacingPoint\recovery-pod.jsonl"

# 5. Sentinel files
ssh User@<pod_tailscale_ip> "dir C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\OTA_DEPLOYING 2>nul"
```

### "Why is the server slow?"

```bash
# 1. Health check
curl -s http://192.168.31.23:8080/api/v1/health | jq .

# 2. Fleet status
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.[] | {pod_number, ws_connected, http_reachable, build_id}'

# 3. Active billing sessions (check for stuck FSM)
curl -s -H "Authorization: Bearer <jwt>" http://192.168.31.23:8080/api/v1/billing/active

# 4. DB stats
curl -s -H "Authorization: Bearer <jwt>" http://192.168.31.23:8080/api/v1/debug/db-stats

# 5. WS churn (>10/min = stale frontend)
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.[0].dashboard_ws_churn'
```

### "Is the cloud in sync?"

```bash
# 1. Cloud health
curl -s http://srv1422716.hstgr.cloud:8080/api/v1/health

# 2. Sync status (from venue server logs)
findstr "cloud_sync" C:\RacingPoint\racecontrol-*.jsonl | tail -5

# 3. Build parity
echo "Venue:" && curl -s http://192.168.31.23:8080/api/v1/health | jq .build_id
echo "Cloud:" && curl -s http://srv1422716.hstgr.cloud:8080/api/v1/health | jq .build_id
```

### "What's rc-watchdog doing?"

```bash
# James (.27) — read last 20 lines
tail -20 "C:\Users\bono\.claude\rc-watchdog.log"

# Check all 9 services it monitors
curl -s http://192.168.31.27:11434/api/tags | head -1    # Ollama
curl -s http://localhost:8766/relay/health                 # comms-link
curl -s http://192.168.31.23:8080/api/v1/health           # server
curl -s http://192.168.31.23:3300/kiosk/api/health        # kiosk
curl -s http://192.168.31.23:3200/api/health              # dashboard
curl -s http://192.168.31.27:1984/api/frame.jpeg | wc -c  # go2rtc (>1KB = ok)
```

---

## Timestamp Conversion Reference

**CRITICAL: Rust tracing logs are UTC. All operations are IST.**

```
UTC 00:00 = IST 05:30
UTC 06:00 = IST 11:30
UTC 12:00 = IST 17:30
UTC 18:00 = IST 23:30
```

**Never use `TZ=Asia/Kolkata date` in Git Bash — it silently returns UTC!**

```bash
# Correct IST time
bash scripts/ist-now.sh

# Or manually
python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%H:%M IST'))"
```

**Before counting events in logs:** Convert every timestamp to IST and exclude your own actions (deploys, restarts, test kills). An audit that reports its own deploys as "unexplained restarts" wastes investigation time.
