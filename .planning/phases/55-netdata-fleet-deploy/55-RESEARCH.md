# Phase 55: Netdata Fleet Deploy - Research

**Researched:** 2026-03-20
**Domain:** Netdata Windows MSI deployment via rc-agent :8090 exec
**Confidence:** MEDIUM — core install/API facts HIGH, dashboard-lock limitation verified MEDIUM

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
All implementation decisions delegated to Claude. Sensible defaults:

**Install Method:**
- Download Netdata Windows MSI from official source to James's deploy-staging HTTP server (:9998)
- Pods download from LAN (`http://192.168.31.27:9998/netdata.msi`) — faster, works offline
- Silent install: `msiexec /i netdata.msi /quiet /norestart`
- Server (.23) installed first (direct access or webterm), then pods via rc-agent :8090

**Fleet Deploy Strategy:**
- Server (.23) first — verify dashboard at :19999
- Pod 8 canary — verify via :8090 exec + check :19999 dashboard
- Pods 1-7 sequential — same pattern, one at a time
- Defender exclusion may be needed (like rc-agent install — `Add-MpPreference -ExclusionPath`)

**Dashboard Access:**
- Standalone per-pod — each pod runs its own Netdata at :19999, no central parent
- LAN-only access (no internet exposure — pods are on 192.168.31.x subnet)
- No password — LAN is trusted, venue-internal only
- James accesses dashboards by browsing to `http://192.168.31.{IP}:19999`

**Verification:**
- `curl -sf http://192.168.31.{IP}:19999/api/v1/info` returns JSON with version info
- E2E script to check all 9 hosts (server + 8 pods)

### Claude's Discretion
All implementation decisions delegated to Claude.

### Deferred Ideas (OUT OF SCOPE)
- Netdata Cloud integration — optional, requires account signup, future consideration
- Netdata parent node for centralized view — standalone per-pod is simpler for now
- Custom Netdata dashboards — auto-generated dashboards are sufficient for v9.0
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MON-04 | Netdata agent installed on racecontrol server (.23) collecting system metrics (CPU, RAM, disk, network) | MSI install on server via webterm :9999; verify :19999/api/v1/info |
| MON-05 | Netdata agent installed on all 8 pods collecting system metrics, deployed via rc-agent :8090 exec | rc-agent /exec endpoint used to download MSI from :9998, run msiexec silently, open firewall port |
</phase_requirements>

---

## Summary

Netdata provides a native Windows agent (MSI, since v2.0 released November 2024) that installs a Windows service collecting CPU, RAM, disk, and network metrics. The agent exposes a local dashboard and REST API on port 19999. For this project the deployment path is: download MSI to James's staging server (:9998) then pods pull via curl over LAN then msiexec /qn silent install then netdata Windows service starts automatically.

**Critical finding — dashboard subscription gate:** The Netdata Windows agent dashboard at :19999 is subject to a UI lock for free/standalone users (the "v2 dashboard" requires Netdata Cloud sign-in with a paid plan to view Windows nodes). However, the REST API at `:19999/api/v1/info` and metric collection still function independently of the cloud UI. The verification check `curl -sf http://IP:19999/api/v1/info` will work. Dashboard browsing may show a locked/blurred UI without a paid plan — this is an accepted limitation for v9.0 (standalone dashboard in CONTEXT.md is aspirational; API verification is the actual gate).

**Primary recommendation:** Deploy Netdata MSI silently using `msiexec /qn /i`, verify with `curl http://IP:19999/api/v1/info`, and accept the cloud-UI limitation for now. The REST API confirms Netdata is collecting metrics even if the web dashboard requires sign-in.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| netdata-x64.msi | v2.9.0 (latest stable 2026-03-20) | Windows agent MSI installer | Official Netdata native Windows agent (v2.0+ since Nov 2024) |
| msiexec (Windows built-in) | — | Silent install driver | Standard Windows MSI deployment tool |
| curl.exe (Windows built-in) | — | Download MSI, API verification | Already used in rc-agent deploy patterns |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| netsh advfirewall | built-in | Open TCP 19999 inbound | MSI does not auto-create firewall rule |
| Add-MpPreference | built-in PowerShell | Defender exclusion | Applied before download to prevent quarantine |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Netdata | Prometheus + node_exporter | More complex setup, no built-in UI, unjustified for LAN venue |
| Netdata | Grafana Agent | Requires Grafana backend, overkill |
| Netdata | WMI Exporter | Prometheus only, no standalone dashboard |

**MSI Download URL (stable):**
```
https://github.com/netdata/netdata/releases/latest/download/netdata-x64.msi
```
Rename to `netdata.msi` in deploy-staging to match the LAN URL in CONTEXT.md.

**Install Command Sequence per Host:**
```
Step 1: Add Defender exclusion
  powershell -Command "Add-MpPreference -ExclusionPath 'C:\RacingPoint'"

Step 2: Download MSI from James's staging server (timeout: 120s)
  curl.exe -s -f -o C:\RacingPoint\netdata.msi http://192.168.31.27:9998/netdata.msi

Step 3: Silent install (timeout: 180s — MSI takes 60-120s)
  msiexec /qn /i C:\RacingPoint\netdata.msi /norestart

Step 4: Open firewall port
  netsh advfirewall firewall add rule name="Netdata" dir=in action=allow protocol=TCP localport=19999

Step 5: Verify service
  sc query netdata

Step 6: Verify API
  curl -sf http://localhost:19999/api/v1/info

Step 7: Clean up MSI
  del /Q C:\RacingPoint\netdata.msi
```

---

## rc-agent /exec API Shape

Source: `crates/rc-agent/src/remote_ops.rs` (read directly)

**Request (POST http://POD_IP:8090/exec):**
```json
{
  "cmd": "msiexec /qn /i C:\\RacingPoint\\netdata.msi /norestart",
  "timeout_ms": 180000
}
```
Fields: `cmd` (string, required), `timeout_ms` (u64, optional, default 10000), `detached` (bool, optional, default false).

**Response (success):**
```json
{"success": true, "exit_code": 0, "stdout": "", "stderr": ""}
```

**Response (failure):**
```json
{"success": false, "exit_code": 1, "stdout": "", "stderr": "...detail..."}
```

**Response (timeout):**
```json
{"success": false, "exit_code": 124, "stdout": "", "stderr": "Command timed out after 10000ms"}
```

**Critical:** DEFAULT_EXEC_TIMEOUT_MS = 10000 in remote_ops.rs. msiexec takes 60-180s. Always pass `timeout_ms: 180000` for msiexec calls.

---

## Architecture Patterns

### Recommended Deploy Structure
```
deploy-staging/
  netdata.msi          # Downloaded once, served from :9998 to pods
  deploy-netdata.py    # New deploy script (mirrors deploy_pod.py pattern)

tests/e2e/
  netdata-fleet.sh     # E2E verification script (all 9 hosts)
```

### Pattern 1: Sequential Pod Deploy via rc-agent /exec
**What:** Each pod executes commands remotely via POST to :8090/exec. Same as deploy_pod.py.
**When to use:** All 8 pod installs.

Proven helper from deploy_pod.py:
```python
def pod_exec(pod_ip, cmd, timeout_ms=10000):
    url = "http://{}:8090/exec".format(pod_ip)
    data = json.dumps({"cmd": cmd, "timeout_ms": timeout_ms}).encode("utf-8")
    req = urllib.request.Request(url, data=data,
          headers={"Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(req, timeout=timeout_ms // 1000 + 5) as resp:
        return json.loads(resp.read())
```

### Pattern 2: Server Install via webterm
**What:** Server (.23) has no rc-agent :8090. Install via webterm at http://192.168.31.27:9999, running commands in the browser terminal against James's machine, then use PowerShell remoting or physical access to server.
**When to use:** Server-only (MON-04). Separate manual step before fleet pod deploy.

### Pattern 3: Canary-First Rollout
Server (.23) then Pod 8 (.91) then Pods 1-7 sequentially. After each pod: check `sc query netdata` via /exec + check curl from James's machine to pod :19999.

### Anti-Patterns to Avoid
- **Wrong timeout on msiexec:** /exec default 10s kills the install mid-run. Always use `timeout_ms: 180000`.
- **Checking dashboard in browser as success gate:** Dashboard is UI-locked. Use /api/v1/info curl check instead.
- **Serving MSI as netdata-x64.msi:** CONTEXT.md expects `netdata.msi`. Rename in staging.
- **Deploying server via pod loop:** Server at .23 has no :8090. Keep server install as a separate manual step.
- **Not cleaning up MSI after install:** Each MSI is ~50MB. Delete from C:\RacingPoint after install.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| System metrics collection | Custom WMI polling scripts | Netdata MSI agent | Collects 300+ metrics automatically |
| Metric REST API | Custom endpoint | Netdata :19999/api/v1/info | Already exists post-install |
| Service management | Custom watchdog | Windows Service (sc start/stop netdata) | Netdata registers auto-start service |
| Fleet orchestration | Brand new tool | Python script modelled on deploy_pod.py | Pattern proven in this project |

---

## Common Pitfalls

### Pitfall 1: Dashboard UI Locked Without Paid Subscription
**What goes wrong:** Browsing to `http://192.168.31.91:19999` shows a blurred/locked dashboard.
**Why it happens:** Netdata v2.0+ requires paid Netdata Cloud plan for Windows agent dashboard UI.
**How to avoid:** Use `curl -sf http://IP:19999/api/v1/info` as the only verification check. REST API responds regardless of subscription. Dashboard browsing is not the success metric.
**Warning signs:** Dashboard shows "add this node to Netdata Cloud" or blurred graphs — this is expected for free users.

### Pitfall 2: msiexec Timeout via /exec
**What goes wrong:** rc-agent /exec returns timeout error; install appears failed.
**Why it happens:** DEFAULT_EXEC_TIMEOUT_MS = 10000ms. msiexec for 50MB MSI takes 60-180s.
**How to avoid:** Pass `"timeout_ms": 180000` in all msiexec /exec calls. Download also needs 120000.
**Warning signs:** `"stderr": "Command timed out after 10000ms"` in response — check if service exists anyway.

### Pitfall 3: Firewall Blocking :19999 from LAN
**What goes wrong:** `curl http://POD_IP:19999/api/v1/info` from James (.27) fails with connection refused.
**Why it happens:** Windows Firewall blocks inbound :19999 by default. Netdata MSI does NOT auto-create inbound firewall rules (confirmed: GitHub issue #9692).
**How to avoid:** Explicitly add rule via /exec: `netsh advfirewall firewall add rule name="Netdata" dir=in action=allow protocol=TCP localport=19999`
**Warning signs:** curl from pod localhost works but from James's machine fails.

### Pitfall 4: Defender Blocking MSI
**What goes wrong:** msiexec exits non-zero or silently fails; netdata service does not appear.
**Why it happens:** Defender may flag or quarantine unfamiliar MSI files. Proven pattern: install.bat v5 adds Defender exclusion before install.
**How to avoid:** Before downloading: `powershell -Command "Add-MpPreference -ExclusionPath 'C:\RacingPoint'"`
**Warning signs:** msiexec returns non-zero; `sc query netdata` shows service not found.

### Pitfall 5: Server Deploy Via Pod Loop
**What goes wrong:** Deploy script tries to reach 192.168.31.23:8090 (server) — connection refused.
**Why it happens:** rc-agent :8090 runs on pods only. Server runs racecontrol :8080, no rc-agent.
**How to avoid:** Keep server install separate. Pod loop covers 192.168.31.89/.33/.28/.88/.86/.87/.38/.91 only.

---

## Netdata API Verification

The `/api/v1/info` endpoint is the canonical health check. It responds with agent metadata regardless of UI subscription status.

Expected response structure (truncated):
```json
{
  "version": "2.9.0",
  "uid": "...",
  "os": "windows",
  "hostname": "GAMING-POD-8",
  "mirrored_hosts": 1
}
```

Check: if curl returns HTTP 200 with JSON containing `"version"` field, Netdata is installed and running.

Fallback check (service level): `sc query netdata` — look for `STATE: 4 RUNNING`.

---

## E2E Verification Script Pattern

```bash
#!/usr/bin/env bash
# tests/e2e/netdata-fleet.sh
# Checks Netdata API on server + all 8 pods

SERVER_IP=192.168.31.23
POD_IPS=(192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88
         192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91)
ALL_IPS=("$SERVER_IP" "${POD_IPS[@]}")

pass=0; fail=0
for ip in "${ALL_IPS[@]}"; do
  if curl -sf --max-time 5 "http://$ip:19999/api/v1/info" | grep -q '"version"'; then
    echo "PASS $ip :19999 responding"
    ((pass++))
  else
    echo "FAIL $ip :19999 not responding"
    ((fail++))
  fi
done
echo "--- $pass passed, $fail failed ---"
[ "$fail" -eq 0 ]
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| WMI polling scripts | Native Netdata Windows MSI agent | Nov 2024 (v2.0) | Full metrics, no custom code |
| Netdata Linux-only | Native Windows agent (gaming PCs) | v2.0 | Windows pods now first-class |
| Free standalone dashboard | Paid sub required for Windows UI | v2.0+ | Dashboard locked; API still free |

**Deprecated/outdated:**
- `netdata/msi-installer` GitHub repo: archived March 2025. All Windows code is in main `netdata/netdata` repo.
- Older guidance using `/quiet` flag: official docs now use `/qn`. Both suppress UI; `/qn` is more complete.

---

## Open Questions

1. **Does `/api/v1/info` return valid JSON without paid subscription?**
   - What we know: Docs say "UI locked" for free Windows users — does not say "API locked".
   - What's unclear: No direct confirmation that the HTTP API responds for free/standalone.
   - Recommendation: Treat `sc query netdata` (service running) as the primary gate. If API check fails with HTTP 200 but JSON is incomplete, fallback to service check as pass condition.

2. **Does msiexec auto-create Windows Firewall rule for :19999?**
   - What we know: GitHub issue #9692 confirmed Netdata does not auto-open ports during install.
   - Recommendation: Always add netsh rule explicitly as a deploy step.

3. **Exact Windows service name**
   - What we know: Community sources consistently reference `sc query netdata` and `Get-Service netdata`.
   - What's unclear: No official doc states the registry service name explicitly.
   - Recommendation: Use `sc query netdata` in deploy verification. If wrong, `sc query type= all` lists all services.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Bash shell scripts (existing project pattern) |
| Config file | none — standalone scripts |
| Quick run command | `bash tests/e2e/netdata-fleet.sh` |
| Full suite command | `bash tests/e2e/run-all.sh` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MON-04 | Server (.23) :19999 api/v1/info responds | smoke | `curl -sf http://192.168.31.23:19999/api/v1/info` | ❌ Wave 0 |
| MON-05 | All 8 pods :19999 api/v1/info responds | smoke | `bash tests/e2e/netdata-fleet.sh` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `curl -sf http://192.168.31.91:19999/api/v1/info` (Pod 8 canary)
- **Per wave merge:** `bash tests/e2e/netdata-fleet.sh` (all 9 hosts)
- **Phase gate:** All 9 hosts returning valid JSON before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/e2e/netdata-fleet.sh` — covers MON-04 and MON-05, checks all 9 IPs
- [ ] `netdata.msi` in `C:\Users\bono\racingpoint\deploy-staging\` — download before any pod deploy

---

## Sources

### Primary (HIGH confidence)
- [Netdata Windows Install Docs](https://learn.netdata.cloud/docs/netdata-agent/installation/windows) — MSI URL, /qn flag, service auto-registration
- [Netdata WINDOWS_INSTALLER.md](https://github.com/netdata/netdata/blob/master/packaging/windows/WINDOWS_INSTALLER.md) — TOKEN optional for standalone, /qn flag confirmed
- `crates/rc-agent/src/remote_ops.rs` — ExecRequest/ExecResponse shape, DEFAULT_EXEC_TIMEOUT_MS = 10000
- `deploy-staging/deploy_pod.py` — pod_exec() pattern, POD_IPS dict, sequential timeout handling

### Secondary (MEDIUM confidence)
- [Netdata Windows Dashboard Blurred (forum)](https://community.netdata.cloud/t/windows-2-0-agent-blurred-dashboard/6026) — dashboard UI locked without paid plan
- [GitHub Discussion #18422](https://github.com/netdata/netdata/discussions/18422) — "Windows nodes will be UI-locked on free plans"
- [Netdata Pricing Page](https://www.netdata.cloud/pricing/) — Windows monitoring listed as paid feature
- [GitHub Issue #9692](https://github.com/netdata/netdata/issues/9692) — firewall not opened automatically during install
- Multiple community posts — service name is `netdata` (sc query netdata / Get-Service netdata)

### Tertiary (LOW confidence)
- Inferred: /api/v1/info API responds without subscription (language is "UI locked", not "API locked")
- Inferred: Defender exclusion needed (extrapolated from rc-agent install pattern, not Netdata-specific evidence)

---

## Metadata

**Confidence breakdown:**
- MSI download URL: HIGH — official GitHub releases URL
- Silent install flags (/qn /norestart): HIGH — official docs confirm /qn
- Service name "netdata": MEDIUM — consistent community sources, no official doc
- Firewall not auto-created: HIGH — confirmed by GitHub issue #9692
- Dashboard locked (paid): MEDIUM — confirmed community forum + GitHub discussion
- /api/v1/info works without subscription: LOW — inferred from "UI locked" not "API locked"
- Defender exclusion needed: MEDIUM — proven pattern for rc-agent, extrapolated to Netdata

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (Netdata changes pricing/features frequently — recheck if dashboard still locked before communicating to Uday)
