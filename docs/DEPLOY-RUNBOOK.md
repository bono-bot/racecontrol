# Racing Point — Deployment Runbook

Last updated: 2026-03-23
Standing rule: Always run `bash deploy-staging/check-health.sh` before marking any deploy complete.

---

## Quick Reference

| Service | Deploy Command | Health Endpoint |
|---------|----------------|-----------------|
| racecontrol | `bash deploy-staging/deploy.sh racecontrol` | `http://192.168.31.23:8080/api/v1/health` |
| kiosk | `bash deploy-staging/deploy.sh kiosk` | `http://192.168.31.23:3300/kiosk/api/health` |
| web dashboard | `bash deploy-staging/deploy.sh web` | `http://192.168.31.23:3200/api/health` |
| rc-sentry | `bash deploy-staging/deploy.sh rc-sentry` | `http://192.168.31.23:8096/health` |
| comms-link | `bash deploy-staging/deploy.sh comms-link` | `http://localhost:8766/health` |
| rc-agent (pods) | Manual — see rc-agent section below | `http://<pod-ip>:8090/health` |

---

## Pre-Deploy Checklist (ALL services)

- [ ] `cargo test` / `npm test` passes locally before uploading anything
- [ ] Binary size check — compare to previous build (unexpectedly large = problem)
- [ ] Rollback plan prepared — know the exact command to undo before starting
- [ ] After deploy: `bash deploy-staging/check-health.sh` — all 5 services PASS

---

## racecontrol (Server .23, port 8080)

### Deploy

```bash
bash deploy-staging/deploy.sh racecontrol
```

This script: builds release binary (`cargo build --release --bin racecontrol`), SCPs to server,
runs `taskkill /F /IM racecontrol.exe`, moves binary into place, then runs `schtasks /Run /TN StartRCTemp`.
The scheduled task survives SSH disconnect — never use `ssh ... "start ..."` directly.

### Manual Steps (if deploy.sh is unavailable)

```bash
# 1. Build
cargo build --release --bin racecontrol

# 2. Back up current binary on server before overwriting
ssh ADMIN@192.168.31.23 "copy C:\RacingPoint\racecontrol.exe C:\RacingPoint\racecontrol-prev.exe"

# 3. Upload new binary
scp target/release/racecontrol.exe ADMIN@192.168.31.23:C:/RacingPoint/racecontrol-new.exe

# 4. Swap and restart via scheduled task (SSH-disconnect-safe)
ssh ADMIN@192.168.31.23 "taskkill /F /IM racecontrol.exe & move /Y C:\RacingPoint\racecontrol-new.exe C:\RacingPoint\racecontrol.exe & schtasks /Run /TN StartRCTemp"

# 5. Wait and verify
sleep 5 && curl http://192.168.31.23:8080/api/v1/health
```

### Rollback

```bash
ssh ADMIN@192.168.31.23 "taskkill /F /IM racecontrol.exe & copy /Y C:\RacingPoint\racecontrol-prev.exe C:\RacingPoint\racecontrol.exe & schtasks /Run /TN StartRCTemp"
```

Keep `racecontrol-prev.exe` on the server before every deploy. The manual step 2 above does this.

---

## kiosk (Server .23, port 3300)

### Deploy

```bash
bash deploy-staging/deploy.sh kiosk
```

Note: kiosk is a Next.js app. The build bakes `NEXT_PUBLIC_API_URL` at compile time.
Ensure correct LAN IP is set before building. Standalone deploy requires `.next/static` copied into `.next/standalone/`.

### Verify

```bash
curl http://192.168.31.23:3300/kiosk/api/health
# Expected: {"status":"ok","service":"kiosk","version":"0.1.0"}
```

### Rollback

```bash
ssh ADMIN@192.168.31.23 "cd C:\RacingPoint\kiosk && git revert HEAD --no-edit && schtasks /Run /TN StartKiosk"
```

---

## web dashboard (Server .23, port 3200)

### Deploy

```bash
bash deploy-staging/deploy.sh web
```

Same Next.js standalone deploy rules apply as kiosk (baked env vars, .next/static copy).

### Verify

```bash
curl http://192.168.31.23:3200/api/health
# Expected: {"status":"ok","service":"web-dashboard","version":"0.1.0"}
```

### Rollback

```bash
ssh ADMIN@192.168.31.23 "cd C:\RacingPoint\web && git revert HEAD --no-edit && schtasks /Run /TN StartWebDashboard"
```

---

## rc-sentry (Server .23, port 8096)

### Deploy

```bash
bash deploy-staging/deploy.sh rc-sentry
```

rc-sentry is the fleet watchdog and camera AI service. Same SCP + schtasks pattern as racecontrol.

### Verify

```bash
curl http://192.168.31.23:8096/health
# Expected: {"status":"ok",...}
```

### Rollback

```bash
ssh ADMIN@192.168.31.23 "taskkill /F /IM rc-sentry.exe & copy /Y C:\RacingPoint\rc-sentry-prev.exe C:\RacingPoint\rc-sentry.exe & schtasks /Run /TN StartRCSentry"
```

---

## comms-link relay (James .27, port 8766)

### Deploy

```bash
bash deploy-staging/deploy.sh comms-link
```

comms-link runs on James's machine (192.168.31.27). Managed by pm2.

### Verify

```bash
curl http://localhost:8766/health
# Expected: {"status":"ok","service":"comms-link","version":"1.0.0","connected":true,...}
```

### Rollback

```bash
cd C:/Users/bono/racingpoint/comms-link && git revert HEAD --no-edit && pm2 restart comms-link-bono
```

---

## rc-agent (All 8 pods, port 8090)

rc-agent uses the `RCAGENT_SELF_RESTART` sentinel for live updates — **NEVER use `taskkill /F /IM rc-agent.exe`**.
Taskkill kills the exec handler before the restart command runs, taking the pod offline.

### Deploy sequence (RCAGENT_SELF_RESTART — standing rule)

```bash
# 1. Build
cargo build --release --bin rc-agent

# 2. Copy to staging
cp target/release/rc-agent.exe deploy-staging/rc-agent.exe

# 3. Start HTTP server on :9998 (from James .27)
python -m http.server 9998 --directory deploy-staging &
HTTP_PID=$!

# 4. CANARY FIRST — Pod 8 before fleet
# Download new binary on Pod 8 via relay exec:
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"pod":8,"command":"curl.exe -s -o C:\\RacingPoint\\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe"}'

# 5. Send RCAGENT_SELF_RESTART sentinel — rc-agent calls relaunch_self(), swaps binary, restarts
curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"pod":8,"command":"RCAGENT_SELF_RESTART"}'

# 6. Verify Pod 8 build_id matches new build
sleep 10 && curl http://192.168.31.91:8090/health

# 7. If Pod 8 OK, repeat steps 4-6 for pods 1-7
# Repeat: change pod number, check pod IPs from CLAUDE.md network map

# 8. Stop HTTP server
kill $HTTP_PID
```

### Pod IP reference

| Pod | LAN IP | Tailscale |
|-----|--------|-----------|
| Pod 1 | 192.168.31.89 | sim1-1 / 100.92.122.89 |
| Pod 2 | 192.168.31.33 | sim2 / 100.105.93.108 |
| Pod 3 | 192.168.31.28 | sim3 / 100.69.231.26 |
| Pod 4 | 192.168.31.88 | sim4 / 100.75.45.10 |
| Pod 5 | 192.168.31.86 | sim5 / 100.110.133.87 |
| Pod 6 | 192.168.31.87 | sim6 / 100.127.149.17 |
| Pod 7 | 192.168.31.38 | sim7 / 100.82.196.28 |
| Pod 8 | 192.168.31.91 | sim8 / 100.98.67.67 |

### Rollback

```bash
# SCP old binary back to pod via Tailscale SSH
ssh -o StrictHostKeyChecking=no User@<tailscale_ip> "copy /Y C:\RacingPoint\rc-agent-prev.exe C:\RacingPoint\rc-agent.exe & schtasks /Run /TN StartRCAgent"
```

Alternatively, if Tailscale unreachable: use pendrive deploy kit.

```bash
# Pendrive fallback (run as admin on the pod)
D:\pod-deploy\install.bat <pod_number>
```

---

## Health Check Script

```bash
bash deploy-staging/check-health.sh
```

Polls all 5 services (racecontrol, kiosk, web-dashboard, comms-link, rc-sentry).
Prints `PASS` or `FAIL` per service. Exits non-zero if any service is down.

Run after EVERY deploy. Required before marking any milestone shipped.

Sample output when healthy:

```
=== Racing Point Health Check 2026-03-23 10:30 IST ===

  PASS  racecontrol   :8080 (http://192.168.31.23:8080/api/v1/health)
  PASS  kiosk         :3300 (http://192.168.31.23:3300/kiosk/api/health)
  PASS  web-dashboard :3200 (http://192.168.31.23:3200/api/health)
  PASS  comms-link    :8766 (http://localhost:8766/health)
  PASS  rc-sentry     :8096 (http://192.168.31.23:8096/health)

Results: 5 passed, 0 failed
HEALTH CHECK PASSED — all services healthy
```

---

## Recovery: Pod with dead rc-agent

When rc-agent is dead and LAN exec is unavailable:

```bash
# Tailscale SSH fallback
ssh -o StrictHostKeyChecking=no User@<tailscale_ip>

# On the pod — restart via scheduled task
schtasks /Run /TN StartRCAgent
```

Run `tailscale status` to find pod Tailscale IPs (sim1–sim8).
If Tailscale is also down, use pendrive: `D:\pod-deploy\install.bat <pod_number>` run as admin on the pod.

---

## Related Documents

- `docs/API-BOUNDARIES.md` — API contract between services
- `docs/openapi.yaml` — Full API spec
- `docs/debugging-playbook.md` — Incident debugging procedures
- `deploy-staging/check-health.sh` — Health check automation
- `LOGBOOK.md` — Commit and incident log
