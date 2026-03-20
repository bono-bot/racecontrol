---
phase: 55-netdata-fleet-deploy
plan: 01
subsystem: infra
tags: [netdata, monitoring, windows-msi, deploy-script, python, bash, fleet]

requires:
  - phase: 53-deployment-automation
    provides: deploy_pod.py pattern — post_json and pod_exec helpers reused verbatim

provides:
  - Netdata Windows MSI (154MB) downloaded to deploy-staging :9998
  - deploy-netdata.py: fleet deploy script for pods via rc-agent :8090/exec
  - tests/e2e/netdata-fleet.sh: verifies all 9 hosts (server + 8 pods) at :19999/api/v1/info

affects:
  - 55-02 (pod fleet deploy — uses deploy-netdata.py and netdata-fleet.sh)
  - 56-whatsapp-alerting (will use netdata metrics once MON-05 complete)

tech-stack:
  added: [netdata-x64.msi v2.9.0, urllib.request (pure stdlib — no new deps)]
  patterns:
    - pod_exec() pattern from deploy_pod.py reused with timeout_ms=180000 for msiexec
    - Canary-first rollout: Pod 8 (.91) deployed first, abort if canary fails
    - API-check over dashboard-check: /api/v1/info works on free tier, dashboard UI locked

key-files:
  created:
    - deploy-staging/deploy-netdata.py
    - tests/e2e/netdata-fleet.sh
  modified: []

key-decisions:
  - "deploy-netdata.py uses raw string literals (r prefix) for Windows paths — avoids backslash escape errors in shell commands sent over /exec"
  - "Firewall rule uses name=Netdata (no quotes) in netsh cmd — unquoted form avoids shell quoting issues over rc-agent /exec"
  - "netdata.msi excluded from git via .gitignore (154MB binary artifact) — downloaded fresh per deploy"
  - "check_netdata_api() checks for version field in JSON body — works whether API returns minimal or full response"
  - "netdata-fleet.sh sources lib/common.sh for color output but degrades gracefully to plain text if missing"
  - "msiexec timeout_ms=180000 (CRITICAL) — default 10s exec timeout kills install mid-run; 180s observed upper bound per RESEARCH.md"

patterns-established:
  - "Pattern 1: Canary pod (Pod 8 .91) first, then Pods 1-7 sequentially for all fleet deploy scripts"
  - "Pattern 2: API verify from James machine after each pod install — not just sc query service"

requirements-completed: []  # MON-04 requires server .23 installed — blocked on Task 2 (human-action checkpoint)

duration: 12min
completed: 2026-03-20
---

# Phase 55 Plan 01: Netdata Fleet Deploy — Scripts + MSI Download Summary

**Netdata deploy script (pod fleet via rc-agent :8090) and E2E verification script (9 hosts) created; MSI (154MB) staged at deploy-staging :9998**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-20T09:28:22Z
- **Completed:** 2026-03-20T09:40:00Z
- **Tasks:** 1/2 complete (Task 2 is a human-action checkpoint — server install)
- **Files created:** 2

## Accomplishments

- Downloaded netdata-x64.msi v2.9.0 (154MB) to `C:/Users/bono/racingpoint/deploy-staging/netdata.msi`
- Created `deploy-netdata.py` (282 lines) — supports single pod, all 8 pods, --check verify-only mode
- Created `tests/e2e/netdata-fleet.sh` (78 lines) — checks all 9 hosts, PASS/FAIL per host, exits with failure count

## Task Commits

1. **Task 1: Download MSI + create deploy script + verification script** - `17e7b95` (feat) — racecontrol repo
   - Also committed in deploy-staging repo: `beb8467` — deploy-netdata.py + .gitignore

## Files Created/Modified

- `deploy-staging/deploy-netdata.py` — Netdata fleet deploy script for pods via rc-agent :8090/exec
- `tests/e2e/netdata-fleet.sh` — E2E verification: all 9 hosts :19999/api/v1/info
- `deploy-staging/netdata.msi` — 154MB MSI binary (not in git; served from staging HTTP :9998)
- `deploy-staging/.gitignore` — excludes *.msi and *.exe from git

## Decisions Made

- msiexec timeout_ms=180000: default 10s exec timeout would kill the install mid-run. RESEARCH.md confirms 60-180s is typical.
- Raw string literals (r prefix) for Windows paths in Python: avoids backslash escape issues when commands are sent as JSON strings over :8090/exec.
- MSI excluded from git (.gitignore): 154MB binary artifact, downloaded fresh each deploy from staging :9998.
- Canary Pod 8 first: same pattern as deploy_pod.py — fail fast before fleet-wide rollout.

## Deviations from Plan

None — plan executed exactly as specified. deploy_pod.py pattern (post_json, pod_exec) reused directly.

## Issues Encountered

- Bash heredoc with single quotes in Python content caused shell parsing failures. Resolved by using `cat >> file << 'MARKER'` for separate sections, appending each function block independently.

## User Setup Required

**Task 2 is a human-action checkpoint — Netdata must be installed on server .23 manually.**

Run these commands on the server via webterm at http://192.168.31.27:9999:

```
powershell -Command "Add-MpPreference -ExclusionPath 'C:\RacingPoint'"
curl.exe -s -f -o C:\RacingPoint\netdata.msi http://192.168.31.27:9998/netdata.msi
msiexec /qn /i C:\RacingPoint\netdata.msi /norestart
netsh advfirewall firewall add rule name="Netdata" dir=in action=allow protocol=TCP localport=19999
sc query netdata
del /Q C:\RacingPoint\netdata.msi
```

Then verify from James's machine:
```
curl -sf http://192.168.31.23:19999/api/v1/info
```

Should return JSON with a "version" field.

## Next Phase Readiness

- `deploy-netdata.py` ready for Plan 02 (pod fleet deploy — just run `python deploy-netdata.py all`)
- `netdata-fleet.sh` ready to verify fleet status at any time
- Staging HTTP server at :9998 serves netdata.msi to all pods on LAN
- Server .23 Netdata install is the only remaining gate for MON-04

---
*Phase: 55-netdata-fleet-deploy*
*Completed: 2026-03-20*
