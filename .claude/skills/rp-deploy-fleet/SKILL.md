---
name: rp-deploy-fleet
description: Canary-first fleet deploy — Pod 8 first, verify, approve, then pods 1-7
disable-model-invocation: true
---

# /rp:deploy-fleet — Canary-First Fleet Deploy

## When to Use

James explicitly runs `/rp:deploy-fleet` to push a staged rc-agent binary to all 8 pods. Always run `/rp:deploy` first to build and stage the binary. Never auto-triggered.

This skill enforces the canary gate: Pod 8 is deployed, verified, and approved by James before any other pod is touched.

## Prerequisites

### Check 1: Staging HTTP server running on :9998

```bash
curl -sf http://192.168.31.27:9998/rc-agent.exe -o /dev/null -w "%{http_code} %{size_download} bytes\n"
```

If this fails (not 200) or returns 0 bytes: **STOP.** Tell James:
- Start the staging HTTP server: run `schtasks /run /tn "RacingPoint-StagingHTTP"` on this machine, OR
- Manually: `python C:/Users/bono/racingpoint/deploy-staging/http_server.py`
- Verify: `curl -I http://192.168.31.27:9998/rc-agent.exe`

### Check 2: rc-agent.exe exists in deploy-staging

```bash
ls -la /c/Users/bono/racingpoint/deploy-staging/rc-agent.exe
```

Show file size. If missing or < 8,000,000 bytes: **STOP.** Tell James to run `/rp:deploy` first to build and stage the binary.

---

## Step 1: Deploy to Pod 8 (canary)

```bash
cd /c/Users/bono/racingpoint/deploy-staging
python3 deploy_pod.py 8
```

Show full deploy_pod.py output. Note: a timeout on the "start rc-agent" step is expected — rc-agent runs indefinitely so the start command never returns.

---

## Step 2: Wait for rc-agent restart

```bash
sleep 10
```

10 seconds allows RCAGENT_SELF_RESTART to complete and rc-agent to reconnect to the WebSocket server.

---

## Step 3: Run verify.sh against Pod 8

```bash
cd /c/Users/bono/racingpoint/racecontrol
RC_BASE_URL=http://192.168.31.23:8080/api/v1 TEST_POD_ID=pod-8 bash tests/e2e/deploy/verify.sh
```

Show the full verify.sh output to James — every gate result and any failures.

**Check exit code:**

If exit code != 0: **STOP.** Print:

```
CANARY FAILED — {N} gate(s) failed on Pod 8.
Fix the issues shown above before fleet rollout.
Run /rp:deploy-fleet again when ready.
```

Do NOT proceed to Step 4.

---

## Step 4: Ask James for approval

Print:

```
Pod 8 canary PASSED all verification gates.
Deploy to remaining 7 pods (1-7)? [y/N]
```

Wait for James's response. Only proceed if James responds with: `y`, `yes`, `go`, or `proceed` (case-insensitive).

If James says no, cancel, wait, or anything other than explicit confirmation: **STOP.** Print:

```
Fleet rollout cancelled. Run /rp:deploy-fleet again when ready.
Pod 8 is already updated and running the new binary.
```

---

## Step 5: Deploy pods 1-7 sequentially

For each pod in order 1, 2, 3, 4, 5, 6, 7:

```bash
cd /c/Users/bono/racingpoint/deploy-staging
echo "--- Deploying Pod 1 ---"
python3 deploy_pod.py 1
```

```bash
cd /c/Users/bono/racingpoint/deploy-staging
echo "--- Deploying Pod 2 ---"
python3 deploy_pod.py 2
```

```bash
cd /c/Users/bono/racingpoint/deploy-staging
echo "--- Deploying Pod 3 ---"
python3 deploy_pod.py 3
```

```bash
cd /c/Users/bono/racingpoint/deploy-staging
echo "--- Deploying Pod 4 ---"
python3 deploy_pod.py 4
```

```bash
cd /c/Users/bono/racingpoint/deploy-staging
echo "--- Deploying Pod 5 ---"
python3 deploy_pod.py 5
```

```bash
cd /c/Users/bono/racingpoint/deploy-staging
echo "--- Deploying Pod 6 ---"
python3 deploy_pod.py 6
```

```bash
cd /c/Users/bono/racingpoint/deploy-staging
echo "--- Deploying Pod 7 ---"
python3 deploy_pod.py 7
```

Show per-pod status as each completes. If a pod fails (deploy_pod.py exits non-zero), log the error and **continue to the next pod** — do not abort the entire fleet for one pod failure.

After all 7 pods: list which succeeded and which failed.

---

## Step 6: Final fleet health check

```bash
sleep 15
```

Wait 15 seconds for all agents to reconnect their WebSocket connections.

```bash
curl -sf http://192.168.31.23:8080/api/v1/fleet/health | python3 -m json.tool
```

Show full fleet health output. Count how many pods have `ws_connected: true`.

---

## Step 7: Summary

Print a deployment summary table:

```
=== Fleet Deploy Summary ===
Pod 8 (canary): PASSED verify.sh — deployed
Pod 1: deployed
Pod 2: deployed
Pod 3: deployed
Pod 4: deployed
Pod 5: deployed
Pod 6: deployed
Pod 7: deployed

Fleet health: X/8 pods connected
```

If any pods failed deploy or show `ws_connected: false`, list them with next steps:
- For failed deploy: re-run `python3 deploy-staging/deploy_pod.py <pod_number>` manually
- For disconnected pods: wait 30s and re-check fleet/health, or verify pod is powered on
- Pod IP reference: see CLAUDE.md Network Map (Pod 1: .89, Pod 2: .33, Pod 3: .28, Pod 4: .88, Pod 5: .86, Pod 6: .87, Pod 7: .38, Pod 8: .91)

---

## Errors

| Symptom | Action |
|---------|--------|
| Staging HTTP :9998 not running | Tell James to start the scheduled task or run http_server.py manually |
| rc-agent.exe missing or < 8MB | Tell James to run `/rp:deploy` first |
| deploy_pod.py fails on a pod | Pod may be off or :8090 unreachable — log error, skip, continue fleet |
| verify.sh exit code != 0 | STOP — do NOT proceed to fleet rollout. Fix canary issues first. |
| Fleet health shows disconnected pods after deploy | Wait 30s and re-check; if still disconnected, check if pod is powered on |
| "Timeout" from deploy_pod.py start step | Expected — rc-agent runs indefinitely, start command will always timeout |

## Notes

- Use `python3` (not `python`) for all deploy_pod.py invocations
- Use `deploy_pod.py` (NOT `deploy-all-pods.py`) — avoids the hardcoded TARGET_SIZE issue
- Sequential pod deploy (not parallel) — prevents RCAGENT_SELF_RESTART race conditions
- The approval prompt in Step 4 is a natural conversation pause — James types y/n in chat
- Always run `/rp:deploy` before `/rp:deploy-fleet` to ensure binary is fresh
