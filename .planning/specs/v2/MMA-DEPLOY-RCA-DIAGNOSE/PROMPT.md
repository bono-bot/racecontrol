# MMA Step 1 DIAGNOSE — rc-agent fleet deployment mechanism RCA

You are an SRE/code-reviewer doing root-cause analysis of a Windows-pod fleet deployment mechanism that failed today (2026-05-09).

## Architecture
- 8 sim-racing pods (Windows 11, LAN .89, .33, .28, .88, .86, .87, .38, .91) each run:
  - **rc-agent.exe** (port 8090) — main pod agent (Rust, tokio reactor)
  - **rc-sentry.exe** (port 8091) — separate process, exec endpoint, survives rc-agent kill
  - **rc-watchdog.exe** (Windows service) — polls rc-agent health, respawns if dead, can rollback binary
- Server .23 has racecontrol.exe (port 8080) controlling fleet
- James .27 hosts HTTP staging server (port 18889) serving binaries for pod download

## Goal of today's deploy
Push 27MB updated rc-agent binary (PR #66 silent-loop-death fix, hash `780d8b1a234f...`, build_id `8e378f4d`) to all 8 pods atomically.

## Canonical deploy script: scripts/deploy-pod.sh per-pod sequence
1. Download via curl on pod (separate /exec call): `curl -s -o C:\RacingPoint\rc-agent-new.exe http://192.168.31.27:18889/rc-agent.exe`
2. SHA256 verify (separate /exec): `certutil -hashfile C:\RacingPoint\rc-agent-new.exe SHA256 | findstr /v hash | findstr /v Cert`
3. Clear sentinels: `del MAINTENANCE_MODE GRACEFUL_RELAUNCH ...`
4. Set sentinel: `echo $epoch > C:\RacingPoint\OTA_DEPLOYING`
5. Stop rc-agent (separate /exec): `taskkill /F /IM rc-agent.exe`
6. sleep 3
7. Preserve (separate /exec): `copy /Y rc-agent.exe rc-agent-prev.exe`
8. Swap (separate /exec): `move /Y rc-agent-new.exe rc-agent.exe`
9. Sync bat file (separate /exec)
10. Start rc-agent (separate /exec): `start "rc-agent" start-rcagent.bat`
11. Verify build_id via /health
12. Clear OTA_DEPLOYING

## Watchdog behavior (rc-watchdog/src/service.rs + rollback_manager.rs)
- Polls `tasklist | findstr rc-agent.exe` every POLL_INTERVAL (~5-10s)
- If rc-agent missing: spawns via `CreateProcessAsUserW(exe_path: "C:\RacingPoint\rc-agent.exe", session=Console)`
- After spawn, runs `health_poller::poll_agent_health()` (3 polls × 10s on `/health`)
- If health fails AND restart_count >= 2: calls `perform_rollback()` which:
  - Suppressed if `OTA_DEPLOYING` sentinel file exists
  - Otherwise: kill rc-agent → delete rc-agent.exe → rename rc-agent-prev.exe → rc-agent.exe
- BLOCKED_PATTERNS list in rc-sentry/src/main.rs:722: `["&&", "||", "| ", "|\"", "`", "$(", ">\\\\", "certutil -urlcache", "bitsadmin"]` — rejects /exec calls containing these patterns

## 11 issues encountered today

1. **Orphan HTTP server PID 53024** from prior session was serving from wrong directory `racecontrol/deploy-staging/` (NOT `~/racingpoint/deploy-staging/`); deploy-pod.sh's `python3 -m http.server 18889 --bind 0.0.0.0 > /dev/null 2>&1 &` silently failed to bind (port taken) but proceeded; pods downloaded WRONG (stale 12.4MB) binary; SHA verify caught it pre-swap.

2. **JSON escaping for /exec** through cmd.exe via rc-sentry: `\R` invalid JSON escape; multiple shell-layer escaping inconsistent (heredoc/printf/Python json.dump produced different bytes); jq is the reliable encoder.

3. **deploy-pod.sh:138 SHA filter contains " | " pipe-space** which is in rc-sentry BLOCKED_PATTERNS; every fleet deploy hits 403 silently; the `| findstr /v hash | findstr /v Cert` filter blocked.

4. **Misdiagnosed root cause** as the literal "Cert" string without grepping rc-sentry source for BLOCKED_PATTERNS — patched filter still contained " | " pipe-space, was a no-op.

5. **No dry-test of patched script on single target before fleet** — burned 7 pods cycling same failure.

6. **Non-atomic kill+swap race**: deploy-pod.sh does kill/copy/move in 3 SEPARATE /exec HTTP calls; RCWatchdog (5-10s polling) wins the race vs the multi-roundtrip swap; old binary respawns. CLAUDE.md "Remote deploy sequence" prescribes SINGLE /exec atomic chain.

7. **Bg task EPERM** on bash subshells: silent crashes (uv_spawn EPERM) leaving 0 bytes output but bg harness reports "completed exit 0". Multiple bash/python orphans accumulate across session.

8. **Pod 5 modal "select app to open .dll" dialog** blocked Windows shell input for ~4h, eventually causing LAN unresponsiveness; trigger source unknown.

9. **Manual atomic-chain swap STILL failed on Pod 1** despite single-/exec chain. Root cause: `perform_rollback()` triggered when watchdog's health_poll fails 2+ times after restart; my inline atomic-deploy script omitted the OTA_DEPLOYING sentinel that suppresses rollback. Pod 8 worked because I set OTA_DEPLOYING and held it 15s+ before clearing.

10. **Multiple bash/python orphans accumulating** — handle exhaustion contributes to Issue 7.

11. **Misinterpreted Pod 5 outage timing** — asserted "during deploy" but server last_seen showed 4h gap; cross-reference timing across ≥2 sources.

## Outcome
- Pod 8: SHIPPED on PR #66 binary `8e378f4d` 5.7h+ stable, heartbeat advancing every 30s
- Pods 1-7: NOT-DEPLOYED (Pod 1 restarted with same OLD binary; Pods 2-4, 6-7 untouched; Pod 5 OFFLINE→intermittent)

## Your task
Identify SYSTEMIC root causes that single-author RCA may have missed. For each finding:
- **CATEGORY**: design / process / discipline / environment / coordination / observability
- **SEVERITY**: P0 (blocks deploy entirely) / P1 (causes silent failures) / P2 (operational friction)
- **EVIDENCE**: cite issue numbers + file paths
- **STRUCTURAL FIX**: concrete code/process change (not "I'll remember next time")
- **VERIFY**: how would you confirm the fix worked

Look especially for: race conditions, observability gaps, unsafe defaults, non-idempotent operations, error-swallowing paths, cross-system invariants violated, missing health-coordination protocol, missing dry-run/preflight tooling, missing bilateral consistency, missing self-healing, sentinel-discipline gaps, manifest-trust gaps.

Identify any issues NOT in the 11 listed that you can infer from the architecture description.

Output as JSON only:
```json
{
  "findings": [
    {"id": "F-1", "category": "...", "severity": "P0|P1|P2", "title": "...", "evidence": "...", "root_cause": "...", "structural_fix": "...", "verify": "...", "novel": true|false}
  ],
  "missed_in_session_rca": ["..."],
  "recommended_priority_order": ["F-N", "F-N", ...]
}
```
