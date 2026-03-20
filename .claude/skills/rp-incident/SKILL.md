---
name: rp-incident
description: Structured incident response following 4-tier debug order with auto-logging
---

# /rp:incident — Structured Incident Response

## When to Use
When James describes a pod problem (e.g., "Pod 3 lock screen blank", "Pod 6 not connecting", "billing stuck on pod 2"). Can be auto-triggered when Claude detects an incident description in conversation.

Usage: `/rp:incident <description>` (e.g., `/rp:incident Pod 3 lock screen blank`)

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

## 4-Tier Debug Order

Always follow this order. Do NOT skip tiers.

**Tier 1: Deterministic Fixes (auto-apply, no confirmation needed)**
Known patterns with guaranteed fixes:
- Stale sockets: restart rc-agent
- Game cleanup: kill orphaned game processes
- Temp file buildup: clear `C:\RacingPoint\temp\`
- WerFault: kill WerFault.exe
- Edge stacking: kill msedge.exe and msedgewebview2.exe

**Tier 2: Memory-Based Fixes (check LOGBOOK first)**
Search LOGBOOK.md for similar past incidents:
```bash
grep -i "KEYWORD" /c/Users/bono/racingpoint/racecontrol/LOGBOOK.md | tail -5
```
Replace KEYWORD with terms from the incident description. If a proven fix exists, apply it.

**Tier 3: Local Ollama Diagnosis**
Query qwen3:0.6b on James's machine for diagnosis:
```bash
curl -sf http://localhost:11434/api/generate -d '{"model":"qwen3:0.6b","prompt":"Diagnose this RC-Agent issue: INCIDENT_DESCRIPTION. Pod state: POD_STATE_JSON","stream":false}' | python3 -c "import json,sys; print(json.load(sys.stdin).get('response','No response'))"
```

**Tier 4: Cloud Claude Escalation**
If tiers 1-3 fail, this is the current conversation — Claude IS the cloud escalation. Analyze all gathered data and propose a fix.

## Steps

Step 1 — Parse incident: Extract pod number and issue description from input.

Step 2 — Auto-query pod status (read-only, runs automatically):
```bash
curl -sf http://192.168.31.23:8080/api/v1/fleet/health | python3 -c "
import json, sys
data = json.load(sys.stdin)
pod = next((p for p in data if p.get('pod_number') == POD_NUMBER), None)
if pod:
    print(json.dumps(pod, indent=2))
else:
    print('Pod not found')
"
```
If server unreachable, switch to guide-only mode (present the 4-tier checklist without running queries).

Step 3 — Tier 1 check: Based on the incident description, identify if any deterministic fix applies. If yes, propose the specific fix command.

Step 4 — Tier 2 check: Search LOGBOOK for similar incidents.

Step 5 — Propose fix. For read-only actions (curl, tasklist, log queries): execute automatically. For destructive actions (process kill, billing end, rc-agent restart, reboot): **ASK JAMES FOR CONFIRMATION BEFORE EXECUTING.**

Destructive commands that REQUIRE confirmation:
- `taskkill` on any process
- Billing end/cancel API calls
- rc-agent restart via :8090
- Pod reboot commands
- Any command that modifies state

Step 6 — After fix is confirmed working, auto-log to LOGBOOK:
```bash
TIMESTAMP=$(python3 -c "from datetime import datetime, timezone, timedelta; ist=timezone(timedelta(hours=5,minutes=30)); print(datetime.now(ist).strftime('%Y-%m-%d %H:%M IST'))")
COMMIT=$(cd /c/Users/bono/racingpoint/racecontrol && git rev-parse --short HEAD)
echo "| $TIMESTAMP | James | \`$COMMIT\` | INCIDENT: DESCRIPTION — RESOLVED: RESOLUTION |" >> /c/Users/bono/racingpoint/racecontrol/LOGBOOK.md
cd /c/Users/bono/racingpoint/racecontrol && git add LOGBOOK.md && git commit -m "logbook: incident $(date +%Y-%m-%d)" && git push
```

## Fallback Mode
If `curl http://192.168.31.23:8080/api/v1/fleet/health` fails (server unreachable):
1. Report: "Server .23 unreachable — switching to guide-only mode"
2. Present the 4-tier checklist as a manual diagnostic guide
3. Skip auto-query steps
4. Still offer to log the incident to LOGBOOK when resolved

## Output
Present findings as a structured report:
- **Pod:** N (IP)
- **Status:** (from fleet health or "server unreachable")
- **Tier 1 Check:** (deterministic fix applicable? which one?)
- **Tier 2 Check:** (similar past incidents in LOGBOOK?)
- **Diagnosis:** (what's likely wrong)
- **Proposed Fix:** (specific command, with confirmation gate if destructive)
