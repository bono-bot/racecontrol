---
name: rp-pod-status
description: Query a specific pod's rc-agent status from fleet health endpoint
---

# /rp:pod-status — Pod Health Query

## When to Use

When James asks about a specific pod's status, or when Claude needs pod state during a conversation. Safe to auto-trigger — read-only query.

Usage: `/rp:pod-status <pod-number>` (e.g., `/rp:pod-status 8` or `/rp:pod-status pod-3`)

## Pod IP Map

| Pod | IP |
|-----|----|
| 1 | 192.168.31.89 |
| 2 | 192.168.31.33 |
| 3 | 192.168.31.28 |
| 4 | 192.168.31.88 |
| 5 | 192.168.31.86 |
| 6 | 192.168.31.87 |
| 7 | 192.168.31.38 |
| 8 | 192.168.31.91 |

## Steps

### Step 1: Parse pod number from input

Extract the numeric pod number (1-8) from the user's input. Accept formats: "pod-3", "pod 3", "3", "Pod 3".

### Step 2: Query fleet health

```bash
curl -sf http://192.168.31.23:8080/api/v1/fleet/health 2>&1
```

If this fails, report: "Server .23 unreachable — check if racecontrol is running on port 8080."

### Step 3: Extract specific pod data (filter by pod_number field, NOT array index)

```bash
curl -sf http://192.168.31.23:8080/api/v1/fleet/health | python3 -c "
import json, sys
data = json.load(sys.stdin)
pod = next((p for p in data if p.get('pod_number') == POD_NUMBER), None)
if pod:
    print(f'Pod {pod[\"pod_number\"]} (IP: {pod.get(\"ip_address\", \"unknown\")})')
    print(f'  WS Connected: {pod[\"ws_connected\"]}')
    print(f'  HTTP Reachable: {pod[\"http_reachable\"]}')
    print(f'  Version: {pod.get(\"version\", \"unknown\")}')
    print(f'  Build ID: {pod.get(\"build_id\", \"unknown\")}')
    print(f'  Uptime: {pod.get(\"uptime_secs\", \"unknown\")}s')
    print(f'  Last Seen: {pod.get(\"last_seen\", \"never\")}')
    print(f'  Crash Recovery: {pod.get(\"crash_recovery\", False)}')
else:
    print(f'Pod POD_NUMBER not found in fleet health response')
" 2>&1
```

Replace `POD_NUMBER` with the actual integer parsed from input.

## Output

Present the pod status in a clean summary. Highlight issues:
- `ws_connected: false` = rc-agent not connected to server WebSocket
- `http_reachable: false` = rc-agent HTTP endpoint (:8090) not responding
- Both false = pod likely offline or rc-agent not running
- `crash_recovery: true` = pod is in crash recovery mode

## Errors

- Server unreachable: "racecontrol server (.23:8080) not responding. Is the service running?"
- Pod not in response: "Pod N not registered with server. rc-agent may not have connected yet."
- Invalid pod number: "Pod number must be 1-8."
