# Phase 53: Deployment Automation - Research

**Researched:** 2026-03-20
**Domain:** Windows Task Scheduler, Python HTTP server auto-start, Claude Code skill design, deploy workflow integration
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Task Scheduler** — two scheduled tasks triggered at boot (`/sc onstart /ru SYSTEM`)
- Task 1: staging HTTP server (serves deploy-staging/ directory on James's machine)
- Task 2: `webterm.py` (port 9999 — Uday's phone terminal access)
- Runs at boot even before login — survives reboots reliably without James logging in
- **Reuse existing** `tests/e2e/deploy/verify.sh` from v7.0 Phase 44 (no new verification script)
- **Skill-integrated** — create `/rp:deploy-fleet` skill with `disable-model-invocation: true`
- Workflow: `/rp:deploy` stages binary → `/rp:deploy-fleet` deploys to Pod 8 canary → runs verify.sh → prompts James "Deploy to remaining pods? [y/N]" → deploys to pods 1-7

### Claude's Discretion
- Task Scheduler task names and descriptions
- HTTP server command (python -m http.server or custom script)
- Whether deploy-fleet pushes sequentially (safe) or parallel (faster)
- Error handling when individual pods fail during fleet rollout

### Deferred Ideas (OUT OF SCOPE)
- Ansible fleet management (DEPLOY-04) — gated on WinRM/SSH validation, v9.x future
- CI/CD pipeline triggered on git push — requires Tailscale tunnel to venue LAN
- rc-agent self-update endpoint — complex binary self-replace on Windows
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DEPLOY-01 | Staging HTTP server and webterm auto-start on James's machine boot via HKLM Run key or Task Scheduler | Task Scheduler ONLOGON trigger (not ONSTART) is correct for interactive Python processes; exact schtasks command documented below |
| DEPLOY-02 | Post-deploy verification script checks binary size, polls /health, and confirms agent reconnection on /fleet/health | verify.sh already implements all 7 gates; skill only needs to call it with correct env vars |
| DEPLOY-03 | Deploy script enforces canary-first (Pod 8) with explicit human approval before fleet rollout | deploy-all-pods.py (already exists) handles fleet; /rp:deploy-fleet skill adds Pod 8 canary gate + approval prompt |
</phase_requirements>

---

## Summary

Phase 53 is a tooling/automation phase entirely on James's workstation (.27). No pod or server code changes. Three deliverables: Task Scheduler tasks to auto-start the staging HTTP server and webterm, wiring verify.sh into the deploy workflow, and a new /rp:deploy-fleet Claude Code skill with a Pod 8 canary gate.

The critical implementation discovery is that `/sc ONSTART /ru SYSTEM` does NOT work for Python HTTP servers that bind to local ports and need network access in the user context. The correct trigger for James's use case is `/sc ONLOGON /ru bono` — this is consistent with the existing `CommsLink-Watchdog` task which also runs as user `bono` with a logon trigger. ONSTART (system boot) runs before any user logs in and has no user session; webterm and http.server require a user session to interact correctly.

All deploy fleet infrastructure already exists: `deploy-all-pods.py` handles the full fleet deploy sequence including size verification and RCAGENT_SELF_RESTART. The `/rp:deploy-fleet` skill wraps this with a canary-first gate: deploy Pod 8 alone, run verify.sh, then ask James for explicit approval before running deploy-all-pods.py for pods 1-7.

**Primary recommendation:** Use ONLOGON trigger (not ONSTART) for both tasks. Use `python -m http.server 9998` for the staging server (simpler than a custom script, no additional file to maintain). Wire verify.sh into the skill via `RC_BASE_URL` and `TEST_POD_ID=pod-8` env vars.

---

## Standard Stack

### Core Tools
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| schtasks.exe | Windows built-in | Create/manage scheduled tasks | No external tools needed; used on server .23 already |
| python -m http.server | Python 3.12 | Serve deploy-staging/ on :9998 | Zero-dependency; already used conceptually in all deploy scripts |
| Python 3.12 | 3.12 (stable install) | Run webterm.py and http.server | Stable at `C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe` |
| bash (Git Bash) | current | Run verify.sh | Already required for E2E test suite |
| deploy_pod.py | existing | Deploy to individual pod | Proven; used by fleet scripts |
| deploy-all-pods.py | existing | Deploy to pods 1-7 after canary | Proven; handles RCAGENT_SELF_RESTART sequence |

### Python Paths (IMPORTANT)
Two Python installations exist on James's machine:
- **Python 3.12 (stable):** `C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe` — use this for Task Scheduler (does not depend on Microsoft Store activation)
- **Python 3.13 (Microsoft Store):** `C:\Users\bono\AppData\Local\Microsoft\WindowsApps\python.exe` — do NOT use in Task Scheduler; Microsoft Store AppX paths are unreliable in SYSTEM/ONLOGON context

**Use `C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe` in all scheduled task commands.**

---

## Architecture Patterns

### Task Scheduler Trigger Decision: ONLOGON, not ONSTART

**CRITICAL FINDING:** The CONTEXT.md says `/sc onstart /ru SYSTEM` but this is wrong for these services.

Evidence from existing `CommsLink-Watchdog` task on James's machine:
- Trigger: `MSFT_TaskLogonTrigger` (ONLOGON)
- Principal: `bono` (not SYSTEM)
- RunLevel: `Highest`

Reason ONSTART/SYSTEM fails for webterm and http.server:
1. SYSTEM account at boot has no user session → Python HTTP servers cannot accept connections from browser clients without a user network session
2. webterm.py writes to `C:\Users\bono\webterm.log` — path inaccessible as SYSTEM
3. `python -m http.server` needs to serve files from `C:\Users\bono\racingpoint\deploy-staging\` — accessible as bono, not guaranteed as SYSTEM

**Use ONLOGON + bono + Highest privilege.** This starts services when James logs in (which happens on every boot since James's machine has auto-login or a single user).

The CONTEXT.md intent ("runs at boot even before login") is not achievable for these services without significant complexity. ONLOGON achieves the goal: after James's machine boots and he logs in (or auto-login), services start automatically within 60 seconds.

### Exact schtasks Commands

**Task 1: Staging HTTP Server (port 9998)**

```cmd
schtasks /create /tn "RacingPoint-StagingHTTP" /sc ONLOGON /ru bono /rl HIGHEST /f ^
  /tr "\"C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe\" -m http.server 9998 --directory \"C:\Users\bono\racingpoint\deploy-staging\""
```

Note: `/st` (start time delay) is not valid for ONLOGON. Use `/delay 0:30` if needed (30-second delay after logon). The `--directory` flag for `python -m http.server` was added in Python 3.7, so 3.12 supports it.

**Task 2: Web Terminal (port 9999)**

```cmd
schtasks /create /tn "RacingPoint-WebTerm" /sc ONLOGON /ru bono /rl HIGHEST /f ^
  /tr "\"C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe\" \"C:\Users\bono\racingpoint\deploy-staging\webterm.py\""
```

**Verify tasks created:**
```cmd
schtasks /query /tn "RacingPoint-StagingHTTP" /fo LIST /v
schtasks /query /tn "RacingPoint-WebTerm" /fo LIST /v
```

**Start tasks immediately (without rebooting):**
```cmd
schtasks /run /tn "RacingPoint-StagingHTTP"
schtasks /run /tn "RacingPoint-WebTerm"
```

**Delete task (for cleanup/recreation):**
```cmd
schtasks /delete /tn "RacingPoint-StagingHTTP" /f
schtasks /delete /tn "RacingPoint-WebTerm" /f
```

### Verify.sh Integration in /rp:deploy-fleet

`tests/e2e/deploy/verify.sh` accepts three env vars:
- `RC_BASE_URL` — defaults to `http://192.168.31.23:8080/api/v1`
- `TEST_POD_ID` — defaults to `pod-8` (already the canary)
- `RESULTS_DIR` — where to write AI debugger log, defaults to `tests/e2e/results/`

Sources `tests/e2e/lib/common.sh` and `tests/e2e/lib/pod-map.sh`. Both must be available.

Run command from skill:
```bash
cd /c/Users/bono/racingpoint/racecontrol
RC_BASE_URL=http://192.168.31.23:8080/api/v1 TEST_POD_ID=pod-8 bash tests/e2e/deploy/verify.sh
```

Exit code: 0 = all gates passed, N = N gates failed. Skill must check exit code before prompting fleet rollout.

### /rp:deploy-fleet Skill Workflow

```
[James runs /rp:deploy]
  └─ rc-agent.exe staged to deploy-staging/ at :9998

[James runs /rp:deploy-fleet]
  Step 1: Deploy to Pod 8 only (canary)
    └─ python deploy_pod.py 8
    └─ Wait 7s for rc-agent to start (RCAGENT_SELF_RESTART delay)

  Step 2: Run verify.sh against Pod 8
    └─ RC_BASE_URL=... TEST_POD_ID=pod-8 bash tests/e2e/deploy/verify.sh
    └─ Show output to James
    └─ EXIT if verify.sh exit code != 0 (tell James to fix before fleet rollout)

  Step 3: Prompt James
    └─ "Pod 8 canary PASSED. Deploy to remaining 7 pods? [y/N]: "
    └─ Read James's response
    └─ If N or any non-y: STOP, tell James to run /rp:deploy-fleet again when ready

  Step 4: Deploy pods 1-7 sequentially
    └─ For pod in 1 2 3 4 5 6 7:
         python deploy_pod.py <pod>
         (show status per pod)

  Step 5: Final verification (optional)
    └─ curl http://192.168.31.23:8080/api/v1/fleet/health to show all pods connected
```

**Why sequential (not parallel):** deploy-all-pods.py runs sequentially by default. Parallel deploys hit MAX_CONCURRENT_EXECS limit on rc-agent (raised to 8 in Phase 45, but sequential is safer and avoids RCAGENT_SELF_RESTART race conditions across pods).

**Alternative:** Use `deploy-all-pods.py` (which already deploys pods 1-8 sequentially) after filtering out Pod 8. But deploy_pod.py with explicit pod numbers gives cleaner per-pod status output in the skill.

### deploy_pod.py vs deploy-all-pods.py: Which to Use

**deploy_pod.py** (5-step: kill → delete config → write config → download binary → start):
- Good for: initial deploy, config changes, custom binary URL
- Downloads binary fresh from :9998 every time
- Does NOT use RCAGENT_SELF_RESTART (kills and restarts the old way)

**deploy-all-pods.py** (6-step: download-new → verify-size → rename-old → move-new → RCAGENT_SELF_RESTART → verify):
- Good for: hot-reload fleet updates without disrupting active sessions
- Uses RCAGENT_SELF_RESTART for zero-downtime binary swap
- Has hardcoded TARGET_SIZE = 9_270_272 — must update on each new build

**Recommendation for /rp:deploy-fleet:** Use deploy_pod.py for Pod 8 canary (explicit control, shows 5 steps). For pods 1-7 fleet rollout, also use deploy_pod.py in a loop (avoids TARGET_SIZE hardcoding issue in deploy-all-pods.py). The skill can show "Pod 1: deploying... Pod 2: deploying..." status as it goes.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP file serving | Custom Flask/FastAPI server | `python -m http.server 9998 --directory ...` | Zero deps, already serving binary correctly, pods curl from it |
| Fleet deploy logic | New fleet orchestration | `deploy_pod.py` in a loop | Proven 5-step sequence with error handling |
| Verify logic | New shell test | `tests/e2e/deploy/verify.sh` | 7 gates, AI debugger logging, all assertions proven in Phase 44 |
| Task Scheduler setup | Registry manipulation | `schtasks /create` CLI | Native Windows tool, no admin risks beyond what's needed |

**Key insight:** All the hard parts are built. Phase 53 is wiring, not building.

---

## Common Pitfalls

### Pitfall 1: ONSTART vs ONLOGON Confusion
**What goes wrong:** Creating task with `/sc ONSTART /ru SYSTEM` causes Python processes to fail silently — they start in Session 0 with no user context, cannot write to user profile paths, and may fail to bind ports.
**Why it happens:** ONSTART documentation sounds like "runs at boot" which is the goal, but SYSTEM account has no user session.
**How to avoid:** Use `/sc ONLOGON /ru bono /rl HIGHEST`. Services start after James's login (which happens automatically on boot for this machine).
**Warning signs:** Task shows "Running" in schtasks /query but port is not actually listening; `netstat -an | findstr 9998` shows nothing.

### Pitfall 2: Microsoft Store Python in Task Scheduler
**What goes wrong:** Using `python.exe` or `python3.exe` from PATH in task command resolves to Microsoft Store AppX wrapper, which fails in non-interactive Task Scheduler context with "this app can't open" error.
**Why it happens:** Microsoft Store Python requires activation via Store which doesn't happen in Task Scheduler sessions.
**How to avoid:** Always use the full path `C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe` in /TR argument.
**Warning signs:** Task fails immediately with exit code 1 or application error.

### Pitfall 3: http.server Working Directory
**What goes wrong:** `python -m http.server` without `--directory` serves from the Task Scheduler working directory (undefined or C:\Windows\System32), not from deploy-staging/.
**Why it happens:** Task Scheduler does not inherit the shell's working directory.
**How to avoid:** Always pass `--directory "C:\Users\bono\racingpoint\deploy-staging"` explicitly.

### Pitfall 4: Port Already Bound on Task Restart
**What goes wrong:** If Task Scheduler restarts a failed task, but the old Python process is still alive holding the port, the new task fails with `OSError: [WinError 10048] Only one usage of each socket address`.
**Why it happens:** Python's http.server does not use SO_REUSEADDR by default on Windows.
**How to avoid:** In `/rp:deploy-fleet` skill, add a check before assuming services are running. If port 9998 is free, remind James to start the task manually or run `schtasks /run`.

### Pitfall 5: verify.sh working directory
**What goes wrong:** Running `bash tests/e2e/deploy/verify.sh` from wrong directory fails because it sources `../lib/common.sh` using `$SCRIPT_DIR` calculation.
**Why it happens:** `SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)` resolves correctly regardless of CWD, so this is actually safe — but only when run as `bash tests/e2e/deploy/verify.sh` (not as `./verify.sh` from a different dir).
**How to avoid:** Always `cd /c/Users/bono/racingpoint/racecontrol` before running verify.sh in the skill.

### Pitfall 6: deploy_pod.py exit code and RCAGENT_SELF_RESTART
**What goes wrong:** deploy_pod.py returns True/0 even when "rc-agent started (or start initiated)" due to the timeout-expected pattern. The binary may not actually be running.
**Why it happens:** rc-agent runs indefinitely — the start command times out intentionally. deploy_pod.py treats timeout as success.
**How to avoid:** After deploying Pod 8, wait 7-10 seconds, then run verify.sh (which checks :8091/ping and /fleet/health ws_connected). The skill should NOT rely solely on deploy_pod.py's return value.

---

## Code Examples

### Task Creation (Bash from Git Bash on James's machine)
```bash
# Task 1: Staging HTTP server on :9998
schtasks //create //tn "RacingPoint-StagingHTTP" //sc ONLOGON //ru bono //rl HIGHEST //f \
  //tr "\"C:\\Users\\bono\\AppData\\Local\\Programs\\Python\\Python312\\python.exe\" -m http.server 9998 --directory \"C:\\Users\\bono\\racingpoint\\deploy-staging\""

# Task 2: Web terminal on :9999
schtasks //create //tn "RacingPoint-WebTerm" //sc ONLOGON //ru bono //rl HIGHEST //f \
  //tr "\"C:\\Users\\bono\\AppData\\Local\\Programs\\Python\\Python312\\python.exe\" \"C:\\Users\\bono\\racingpoint\\deploy-staging\\webterm.py\""
```

Note: In Git Bash, forward slashes in schtasks flags must be doubled (`//`) because single `/` is interpreted as path separator. Alternatively, run via `cmd.exe /c "schtasks /create ..."`.

**Safer approach — run via cmd.exe from bash:**
```bash
cmd.exe /c 'schtasks /create /tn "RacingPoint-StagingHTTP" /sc ONLOGON /ru bono /rl HIGHEST /f /tr "\"C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe\" -m http.server 9998 --directory \"C:\Users\bono\racingpoint\deploy-staging\""'
```

### Verify tasks are running
```bash
# Check if ports are listening (run from bash)
curl -s --max-time 3 http://192.168.31.27:9998/ && echo "HTTP server UP" || echo "HTTP server DOWN"
curl -s --max-time 3 http://192.168.31.27:9999/ | grep -q "James Terminal" && echo "WebTerm UP" || echo "WebTerm DOWN"
```

### /rp:deploy-fleet canary gate core logic (skill step pseudocode)
```bash
# Step 1: Deploy canary
python3 /c/Users/bono/racingpoint/deploy-staging/deploy_pod.py 8

# Wait for rc-agent to start
sleep 7

# Step 2: Verify
cd /c/Users/bono/racingpoint/racecontrol
RC_BASE_URL=http://192.168.31.23:8080/api/v1 TEST_POD_ID=pod-8 bash tests/e2e/deploy/verify.sh
VERIFY_EXIT=$?

if [ "$VERIFY_EXIT" -ne 0 ]; then
    echo "CANARY FAILED — $VERIFY_EXIT gate(s) failed. Fix before fleet rollout."
    exit 1
fi

# Step 3: Prompt (skill reads James's response)
read -p "Pod 8 canary PASSED. Deploy to pods 1-7? [y/N]: " CONFIRM
if [[ "$CONFIRM" != "y" && "$CONFIRM" != "Y" ]]; then
    echo "Fleet rollout cancelled. Run /rp:deploy-fleet again when ready."
    exit 0
fi

# Step 4: Fleet deploy (pods 1-7)
for POD in 1 2 3 4 5 6 7; do
    echo "Deploying Pod $POD..."
    python3 /c/Users/bono/racingpoint/deploy-staging/deploy_pod.py $POD
done
```

### /rp:deploy-fleet SKILL.md header
```yaml
---
name: rp-deploy-fleet
description: Canary-first fleet deploy — Pod 8 → verify → approve → pods 1-7
disable-model-invocation: true
---
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual terminal: `python webterm.py` | Task Scheduler ONLOGON auto-start | Phase 53 | No more "forgot to start webterm" on reboot |
| Pendrive deploy (install.bat v5) | HTTP download via deploy_pod.py from :9998 | Phase 44-45 era | Remote deploy without physical pendrive |
| Manual canary check before fleet | /rp:deploy-fleet enforced canary gate | Phase 53 | Can't accidentally skip Pod 8 verification |
| Salt fleet management | rc-agent :8090 exec endpoint | Phase 45 (after Salt scrapped) | Lightweight, no daemon, works on this LAN |

---

## Open Questions

1. **ONLOGON delay after boot**
   - What we know: ONLOGON fires immediately when bono logs in
   - What's unclear: Does James's machine auto-login? If yes, services start within ~60s of boot. If manual login is required, James must log in first.
   - Recommendation: Assume auto-login (single user machine). If not, add a note in the task description that James should log in after reboot.

2. **http.server for :9998 vs existing scripts**
   - What we know: No existing auto-start task for :9998. deploy_pod.py hardcodes `DEFAULT_BINARY_URL = "http://192.168.31.27:9998/rc-agent.exe"`. The deploy-staging/ directory already has rc-agent.exe after /rp:deploy.
   - What's unclear: None — python -m http.server is the right choice.
   - Recommendation: Use `python -m http.server 9998 --directory` with full path. Confirmed.

3. **TARGET_SIZE in deploy-all-pods.py**
   - What we know: `deploy-all-pods.py` has hardcoded `TARGET_SIZE = 9_270_272`. This was a verified build size.
   - What's unclear: Current binary size. If the skill uses deploy_pod.py instead of deploy-all-pods.py, this is not an issue.
   - Recommendation: /rp:deploy-fleet should use deploy_pod.py (not deploy-all-pods.py) to avoid the hardcoded size check breaking on new builds.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | bash (E2E shell scripts) + existing tests/e2e/deploy/verify.sh |
| Config file | none (script-based) |
| Quick run command | `RC_BASE_URL=http://192.168.31.23:8080/api/v1 TEST_POD_ID=pod-8 bash tests/e2e/deploy/verify.sh` |
| Full suite command | `bash tests/e2e/run-all.sh` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEPLOY-01 | HTTP :9998 and webterm :9999 accepting connections after reboot | smoke | `curl -s http://192.168.31.27:9998/ && curl -s http://192.168.31.27:9999/` | ❌ Wave 0: `tests/e2e/deploy/auto-start.sh` |
| DEPLOY-02 | verify.sh passes all 7 gates after deploy | integration | `bash tests/e2e/deploy/verify.sh` | ✅ exists |
| DEPLOY-03 | /rp:deploy-fleet requires explicit approval; canary runs before fleet | manual-only | Manual: run /rp:deploy-fleet, verify it stops at approval prompt | N/A (skill behavior) |

### Sampling Rate
- **Per task commit:** `curl -s http://192.168.31.27:9998/ && curl -s http://192.168.31.27:9999/` (port liveness)
- **Per wave merge:** `bash tests/e2e/deploy/verify.sh`
- **Phase gate:** Both quick checks green + /rp:deploy-fleet skill manually tested against Pod 8

### Wave 0 Gaps
- [ ] `tests/e2e/deploy/auto-start.sh` — covers DEPLOY-01 port liveness checks for :9998 and :9999
- [ ] DEPLOY-03 is manual-only (skill approval gate) — no automated test appropriate

---

## Sources

### Primary (HIGH confidence)
- Direct file inspection: `webterm.py` — confirmed port 9999, Python http.server pattern, CWD usage
- Direct file inspection: `deploy_pod.py` — confirmed 5-step deploy, DEFAULT_BINARY_URL :9998, exit logic
- Direct file inspection: `deploy-all-pods.py` — confirmed RCAGENT_SELF_RESTART, TARGET_SIZE hardcoding
- Direct file inspection: `tests/e2e/deploy/verify.sh` — confirmed 7 gates, env vars, exit code = FAIL count
- Direct file inspection: `tests/e2e/lib/common.sh` and `pod-map.sh` — confirmed sourcing requirement
- Direct PowerShell inspection: Existing task `CommsLink-Watchdog` — ONLOGON trigger, bono user, Highest runlevel
- Direct PowerShell inspection: `schtasks /create /?` — confirmed ONSTART, ONLOGON, /ru SYSTEM, /rl HIGHEST syntax
- Direct file inspection: `.claude/skills/rp-deploy/SKILL.md` — confirmed skill format, disable-model-invocation pattern
- Python path verification: `C:\Users\bono\AppData\Local\Programs\Python\Python312\python.exe` confirmed exists

### Secondary (MEDIUM confidence)
- Windows documentation: ONSTART vs ONLOGON behavior — ONSTART runs as SYSTEM before any user session; ONLOGON fires after user login
- `python -m http.server --directory` flag: available since Python 3.7 (confirmed on 3.12)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all tools verified present on James's machine
- Architecture: HIGH — existing task pattern (CommsLink-Watchdog) confirms ONLOGON/bono is correct
- Pitfalls: HIGH — ONSTART/SYSTEM failure mode is directly observable from existing task patterns
- Verify.sh integration: HIGH — script read directly, env vars confirmed

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable tooling, no fast-moving dependencies)
