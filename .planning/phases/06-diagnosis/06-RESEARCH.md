# Phase 6: Diagnosis - Research

**Researched:** 2026-03-13
**Domain:** Windows LAN diagnostic commands — log collection, port audit, Edge registry inspection, network IP/MAC identification — all executed remotely via pod-agent
**Confidence:** HIGH

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DIAG-01 | Staff can collect error/debug logs from all 8 pods and server to confirm actual URL failure patterns | Log path `C:\RacingPoint\rc-agent-log.txt` confirmed via deploy-staging scripts; tracing writes to stdout/stderr; collection pattern established |
| DIAG-02 | Staff can run a port audit on Server (.23) to identify port conflicts before deploying the kiosk | `netstat -ano` + `tasklist` correlation; port 3300 and 8080 are the targets; server SSH/pod-agent access pattern documented |
| DIAG-03 | Staff can verify Edge version and kiosk mode settings (StartupBoost, EdgeUpdate, BackgroundMode) across all pods | Registry paths for all three settings verified; `sc query` for EdgeUpdate service; `msedge.exe --version` command confirmed |
| DIAG-04 | Staff can confirm Server (.23) IP assignment type (DHCP vs static) and retrieve MAC address for DHCP reservation | `ipconfig /all` on server returns both; DHCP vs static identification from "Autoconfiguration" field documented |
</phase_requirements>

## Summary

Phase 6 is a pure information-gathering phase — zero code changes, zero deployments. Its single job is to answer the four open questions that prevent Phases 7–9 from being planned correctly: what exactly fails in the logs, what ports are occupied on the server, what Edge configuration state the pods are in, and whether the server IP is stable.

All diagnostic commands are standard Windows CLI tools (`netstat`, `ipconfig /all`, `reg query`, `sc query`, `tasklist`). The existing pod-agent infrastructure (port 8090 on every pod, `POST /exec` with `{"cmd": "..."}`) already executes arbitrary commands remotely and has been used extensively in deploy-staging. For the server (.23), there is no pod-agent; access is via direct RDP or by deploying a one-shot command file. The log collection pattern is already codified in `start-rcagent-log.bat` and `read-log.json` in deploy-staging.

The critical unresolved question from STATE.md is the kiosk port conflict: FEATURES.md says 3300, STACK.md diagram shows 3000. `kiosk/package.json` confirms `"start": "next start -p 3300"` (HIGH confidence from source inspection). The server port audit will settle this definitively. The MAC address of Racing-Point-Server must be retrieved before Phase 7 touches the router.

**Primary recommendation:** Execute all four diagnostic tasks as sequential pod-agent commands collected into a single output document. No scripting complexity needed — plain `{"cmd": "..."}` JSON payloads, results read by James and pasted into the findings doc.

## Standard Stack

### Core (Diagnostic Tooling — All Windows Built-ins)

| Tool | Available On | Purpose | How to Invoke |
|------|-------------|---------|---------------|
| `netstat -ano` | Windows (Server + Pods) | Show all listening/established ports with PIDs | Direct cmd.exe |
| `tasklist /NH /FO LIST` | Windows | Map PIDs to process names | Direct cmd.exe |
| `ipconfig /all` | Windows | Full NIC config including DHCP/static and MAC | Direct cmd.exe |
| `reg query` | Windows | Read registry values (Edge policies) | Direct cmd.exe |
| `sc query EdgeUpdate` | Windows | EdgeUpdate service state | Direct cmd.exe |
| `msedge.exe --version` | Pods | Get installed Edge version | Direct cmd.exe |
| `type C:\RacingPoint\rc-agent-log.txt` | Pods | Read captured rc-agent log | Direct cmd.exe |
| pod-agent `/exec` | Pods (.89/.33/.28/.88/.86/.87/.38/.91 port 8090) | Execute cmd remotely | POST JSON `{"cmd": "..."}` |

### Supporting

| Tool | Available On | Purpose | When to Use |
|------|-------------|---------|-------------|
| `start-rcagent-log.bat` | deploy-staging, served via HTTP | Restart rc-agent with stdout capture to file | When log file missing or empty |
| `run-log-script.json` | deploy-staging | Deploys and runs `start-rcagent-log.bat` via pod-agent | Bootstrapping log capture on a pod |
| debug server port 18924 | Every pod | `GET /status` returns JSON lock screen state | Quick pod health check without log capture |
| RDP to server (.23) | Server | Direct console access for server-side commands | When server-side netstat/ipconfig needed |

### What NOT to Use

- **PowerShell remoting (WinRM)**: Not configured on venue machines.
- **SSH**: Not installed on Windows 11 pods by default.
- **NSSM**: Banned — see project constraints.
- **Any new software install**: This phase makes zero changes, zero installs.

## Architecture Patterns

### Log Collection Pattern (Established in deploy-staging)

rc-agent writes `tracing` output to stdout/stderr using `tracing_subscriber::fmt()`. The tracing level defaults to `rc_agent=info` (from `main.rs` line 200). Logs are NOT written to a file by default — rc-agent must be (re)started with stdout redirected.

The existing pattern in `start-rcagent-log.bat`:
```bat
taskkill /F /IM rc-agent.exe 2>nul
timeout /t 3 /nobreak >nul
cd /D C:\RacingPoint
start "" /D C:\RacingPoint cmd /c "C:\RacingPoint\rc-agent.exe > C:\RacingPoint\rc-agent-log.txt 2>&1"
timeout /t 10 /nobreak >nul
type C:\RacingPoint\rc-agent-log.txt
```

After log is created, read it via pod-agent:
```json
{"cmd": "type C:\\RacingPoint\\rc-agent-log.txt 2>nul || echo NO_LOG_FILE"}
```

For more verbose output, `RUST_LOG=rc_agent=debug` can be set before launch to capture all connection attempts and URL resolution failures.

**Important:** rc-agent logs go to `C:\RacingPoint\rc-agent-log.txt` only if started via `start-rcagent-log.bat`. The HKLM Run key (`start-rcagent.bat`) does NOT redirect to file — it runs interactively in Session 1 with no file capture. Both need to be checked.

### Port Audit Pattern (Server .23)

Standard Windows port identification:

```cmd
netstat -ano | findstr LISTENING
tasklist /NH /FO CSV
```

The combined command for a single pod-agent call:
```json
{"cmd": "netstat -ano | findstr LISTENING & echo === & tasklist /NH /FO CSV", "timeout_ms": 10000}
```

Expected ports already in use on server .23:
- `8080` — racecontrol (Axum HTTP + WebSocket)
- `9996`, `20777`, `5300`, `6789`, `5555` — UDP telemetry listeners (racecontrol `udp_heartbeat.rs`)
- `18923` would only be on pods (rc-agent lock screen), NOT server
- `3300` — kiosk if already running (unlikely in current state)
- `3000` — web/legacy Next.js app (if running)

Port conflict candidates to confirm: nothing should be on 3300 yet. If something is on 3300, Phase 7 needs a different port.

### Edge Inspection Pattern (Pods)

All three Edge settings are registry values. Paths confirmed from Microsoft documentation:

**StartupBoostEnabled:**
```cmd
reg query "HKLM\SOFTWARE\Policies\Microsoft\Edge" /v StartupBoostEnabled 2>nul
reg query "HKCU\SOFTWARE\Policies\Microsoft\Edge" /v StartupBoostEnabled 2>nul
```

**BackgroundModeEnabled:**
```cmd
reg query "HKLM\SOFTWARE\Policies\Microsoft\Edge" /v BackgroundModeEnabled 2>nul
reg query "HKCU\SOFTWARE\Policies\Microsoft\Edge" /v BackgroundModeEnabled 2>nul
```

**EdgeUpdate service:**
```cmd
sc query EdgeUpdate
sc query MicrosoftEdgeUpdate
```

**Edge version:**
```cmd
reg query "HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients\{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}" /v pv 2>nul
```
Or via the exe itself (slower but authoritative):
```cmd
"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --version
```

**Combined pod-agent single command for DIAG-03:**
```json
{
  "cmd": "echo ==EDGE_VERSION== & reg query \"HKLM\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}\" /v pv 2>nul & echo ==STARTUP_BOOST_HKLM== & reg query \"HKLM\\SOFTWARE\\Policies\\Microsoft\\Edge\" /v StartupBoostEnabled 2>nul & echo ==BG_MODE_HKLM== & reg query \"HKLM\\SOFTWARE\\Policies\\Microsoft\\Edge\" /v BackgroundModeEnabled 2>nul & echo ==EDGE_UPDATE_SVC== & sc query EdgeUpdate 2>nul & sc query MicrosoftEdgeUpdate 2>nul",
  "timeout_ms": 15000
}
```

Note: If `HKLM\SOFTWARE\Policies\Microsoft\Edge` key does not exist, the `reg query` returns error code 1 — that means the policy has never been set (not the same as "enabled"). Both absent and present-but-1 mean StartupBoost IS active. Present-and-0 means it is disabled.

### IP/MAC Identification Pattern (Server .23)

**`ipconfig /all`** on the server returns:

```
Ethernet adapter Ethernet:
   Connection-specific DNS Suffix  . :
   Description . . . . . . . . . . . : Realtek PCIe GbE Family Controller
   Physical Address. . . . . . . . . : XX-XX-XX-XX-XX-XX   ← MAC for DHCP reservation
   DHCP Enabled. . . . . . . . . . . : Yes                  ← DHCP (not static)
   Autoconfiguration Enabled . . . . : Yes
   IPv4 Address. . . . . . . . . . . : 192.168.31.23(Preferred)
   Subnet Mask . . . . . . . . . . . : 255.255.255.0
   Lease Obtained. . . . . . . . . . : [date]
   Lease Expires . . . . . . . . . . : [date]
   Default Gateway . . . . . . . . . : 192.168.31.1
```

If `DHCP Enabled: No`, the IP is statically configured on the NIC (no router dependency). If `DHCP Enabled: Yes`, a reservation must be made at the router before Phase 7 touches it.

The MAC in `Physical Address` is the exact value needed for the DHCP reservation in the Xiaomi router at 192.168.31.1.

The server does not have pod-agent installed, so this command must be run via RDP. However, it is a single command — one RDP session, one copy-paste.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Log capture mechanism | Custom Rust logging sink | `cmd /c "rc-agent.exe > log.txt 2>&1"` | Already proven in deploy-staging; zero new code |
| Remote command execution | Any new agent or script | Existing pod-agent `/exec` endpoint | Pod-agent v0.5.0 on all 8 pods already does this |
| Port scanner | PowerShell port-scan loop | `netstat -ano` | Built into Windows, instant, authoritative |
| Edge version detection | Parsing file metadata | `reg query EdgeUpdate\Clients\{GUID} /v pv` | Direct registry read, faster than file stat |
| Batch script deployment | Separate deploy phase | Serve via `python3 -m http.server 9998` + pod-agent download+exec | Already established pattern in deploy-staging |

**Key insight:** Phase 6 produces ZERO code. Every tool needed already exists as a Windows built-in or a deploy-staging script. The only work is executing commands and recording results.

## Common Pitfalls

### Pitfall 1: rc-agent Log File May Not Exist
**What goes wrong:** `type C:\RacingPoint\rc-agent-log.txt` returns `NO_LOG_FILE` because rc-agent was started via the HKLM Run key (`start-rcagent.bat`), which runs interactively in Session 1 with no file redirection.
**Why it happens:** The production startup path does not redirect stdout to a file — it just launches the exe.
**How to avoid:** Use `run-log-script.json` to deploy and run `start-rcagent-log.bat` first. This kills rc-agent, restarts it with log capture, waits 10 seconds, then reads the file. Accept the brief pod downtime (10–15 seconds) during log capture.
**Warning signs:** `NO_LOG_FILE` response from `type` command.

### Pitfall 2: netstat PIDs Map to svchost, Not Named Processes
**What goes wrong:** `netstat -ano` shows PID 1234 listening on 3300, but `tasklist` shows PID 1234 is `svchost.exe`.
**Why it happens:** Some Windows services run inside svchost — need `tasklist /SVC /FO LIST` or `sc query` to resolve service names.
**How to avoid:** Run `tasklist /SVC /FO CSV` alongside `netstat -ano` and cross-reference. For any svchost entry, use `netstat -b` (requires admin) or accept that the service name is sufficient.
**Warning signs:** Multiple entries mapping to svchost on different ports.

### Pitfall 3: Edge Registry Policy Key May Not Exist
**What goes wrong:** `reg query "HKLM\SOFTWARE\Policies\Microsoft\Edge" /v StartupBoostEnabled` returns `ERROR: The system was unable to find the specified registry key or value.`
**Why it happens:** If no Group Policy or manual registry entry has been set, the key tree does not exist — Windows uses the compiled-in Edge defaults, which have StartupBoostEnabled = 1 (on).
**How to avoid:** Treat a missing key as "policy not set = default = ENABLED." Both absent and explicitly set to 1 mean Phase 9 must disable it. Do not interpret error response as "already disabled."
**Warning signs:** The `reg query` exits with a non-zero code and no value printed.

### Pitfall 4: Server .23 Does Not Have Pod-Agent
**What goes wrong:** Attempting to run diagnostic commands on Server (.23) via `curl http://192.168.31.23:8090/exec` fails — connection refused.
**Why it happens:** Pod-agent is deployed only on the 8 gaming pods, not the server.
**How to avoid:** DIAG-02 and DIAG-04 require direct server access via RDP. Plan for James to RDP to .23 and run the commands manually. The commands are short (under 2 minutes of work).
**Warning signs:** curl to port 8090 on .23 returns "Connection refused."

### Pitfall 5: racecontrol Logs Are Also Not in a File by Default
**What goes wrong:** Looking for a log file for racecontrol on the server and finding nothing.
**Why it happens:** racecontrol (the Axum server) also uses `tracing_subscriber::fmt()` writing to stdout, same pattern as rc-agent. No file redirect in the current startup mechanism.
**How to avoid:** On the server, run `type C:\RacingPoint\racecontrol-log.txt 2>nul || echo NO_LOG_FILE` to check. If missing, restart racecontrol with: `C:\RacingPoint\racecontrol.exe > C:\RacingPoint\racecontrol-log.txt 2>&1`. This is acceptable for a brief diagnostic capture.
**Warning signs:** No log file in `C:\RacingPoint\`.

### Pitfall 6: Edge Service Name Varies by Windows Version
**What goes wrong:** `sc query EdgeUpdate` returns "The specified service does not exist" on some pods.
**Why it happens:** The service may be named `MicrosoftEdgeUpdate`, `EdgeUpdate`, or `edgeupdate` depending on the Edge channel (Stable vs Beta) and Windows 11 build.
**How to avoid:** Query all three names: `sc query EdgeUpdate 2>nul & sc query MicrosoftEdgeUpdate 2>nul & sc query edgeupdate 2>nul`. At least one will return results.
**Warning signs:** All three queries return "does not exist" — means Edge Update service was already removed (good), or Edge is not installed (unexpected).

### Pitfall 7: JSON Escaping in cmd Field for Complex Commands
**What goes wrong:** Backslashes and double-quotes in Windows paths get mangled when constructing pod-agent JSON in bash on Windows Git Bash.
**Why it happens:** Git Bash on Windows has a known escaping issue with `\\` in both single and double-quoted strings passed to curl.
**How to avoid:** Write complex JSON payloads to a file with the Write tool, then pass to curl via `-d @file.json`. This is already the established deploy-staging pattern (all `.json` files in deploy-staging are payload files, not inline strings).
**Warning signs:** Commands fail with syntax errors or unexpected behavior on the pod.

## Code Examples

### DIAG-01: Collect rc-agent Log From One Pod

Step 1 — Deploy log capture script and start rc-agent with logging:
```json
{"cmd": "curl -s -o C:\\RacingPoint\\start-rcagent-log.bat http://192.168.31.27:9998/start-rcagent-log.bat && C:\\RacingPoint\\start-rcagent-log.bat", "timeout_ms": 30000}
```
(Uses existing `run-log-script.json` — serve via HTTP server on James's machine first)

Step 2 — Read the log 30 seconds later:
```json
{"cmd": "type C:\\RacingPoint\\rc-agent-log.txt 2>nul || echo NO_LOG_FILE", "timeout_ms": 5000}
```

For more verbose output, add RUST_LOG to the bat file before running. But info-level is sufficient to see WebSocket connection attempts, URL resolution, and lock screen state transitions.

### DIAG-02: Port Audit on Server (.23) — Run via RDP

```cmd
netstat -ano | findstr LISTENING
tasklist /SVC /NH /FO CSV
```

Or as a single combined command to paste into cmd.exe on the server:
```cmd
echo ==LISTENING PORTS== & netstat -ano | findstr LISTENING & echo ==PROCESS LIST== & tasklist /SVC /NH /FO CSV
```

Redirect to a file for easy copy-paste:
```cmd
(echo ==LISTENING PORTS== & netstat -ano | findstr LISTENING & echo ==PROCESS LIST== & tasklist /SVC /NH /FO CSV) > C:\RacingPoint\port-audit.txt & type C:\RacingPoint\port-audit.txt
```

### DIAG-03: Edge Inspection on One Pod — JSON Payload File

Write this to `edge-inspect.json` in deploy-staging:
```json
{
  "cmd": "echo ==EDGE_VERSION== & reg query \"HKLM\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{56EB18F8-B008-4CBD-B6D2-8C97FE7E9062}\" /v pv 2>nul & echo ==STARTUP_BOOST== & reg query \"HKLM\\SOFTWARE\\Policies\\Microsoft\\Edge\" /v StartupBoostEnabled 2>nul & echo ==BG_MODE== & reg query \"HKLM\\SOFTWARE\\Policies\\Microsoft\\Edge\" /v BackgroundModeEnabled 2>nul & echo ==EDGE_UPDATE_SVC== & sc query EdgeUpdate 2>nul & sc query MicrosoftEdgeUpdate 2>nul",
  "timeout_ms": 15000
}
```

Execute via curl:
```bash
curl -s -X POST http://192.168.31.91:8090/exec -H "Content-Type: application/json" -d @edge-inspect.json
```

### DIAG-04: IP and MAC on Server (.23) — Run via RDP

```cmd
ipconfig /all
```

The key fields to record:
- `Physical Address` — MAC for DHCP reservation (format: `XX-XX-XX-XX-XX-XX`)
- `DHCP Enabled` — `Yes` = needs router reservation, `No` = already static
- `IPv4 Address` — confirm it is `.23`
- `Lease Expires` — if DHCP, shows when the lease was last renewed

### Loop Pattern: Run Command Across All 8 Pods

```bash
for ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do
  echo "=== Pod $ip ==="
  curl -s -X POST "http://$ip:8090/exec" -H "Content-Type: application/json" -d @edge-inspect.json
  echo ""
done
```

(Run from Git Bash on James's machine at .27)

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Veyon classroom management | Removed (all 8 pods) | Mar 11, 2026 | No more phantom input; pod-agent is sole remote control mechanism |
| rc-agent watchdog scheduled task | Deleted; HKLM Run key replaces it | Mar 11, 2026 | rc-agent now starts in Session 1, no Session 0 blindness |
| Inline log reading (no file) | `start-rcagent-log.bat` pattern for capture | Established in deploy-staging | Logs now retrievable without console access |
| Manual pod visits | pod-agent v0.5.0 remote exec | All 8 pods as of Mar 11 | All diagnostic commands can run from James's machine |

**Current log state:**
- rc-agent: Writes to stdout when started interactively. If started via HKLM Run key (production path), logs go to the Session 1 console window only — no file unless `start-rcagent-log.bat` redirects it.
- racecontrol: Same — tracing writes to stdout only. No log file unless started with redirect.
- pod-agent: Node.js, writes to its own console. No known log file.

## Open Questions

1. **What does the rc-agent log actually say on a failing pod?**
   - What we know: The `Disconnected` lock screen state is shown when WebSocket to racecontrol fails (from `lock_screen.rs`)
   - What's unclear: Are the pods failing because racecontrol is unreachable (wrong IP), because port 8080 is blocked, or because the URL in `rc-agent.toml` has a stale hostname?
   - Recommendation: DIAG-01 resolves this. Run log capture on a pod that has shown "Site cannot be reached" and read what WebSocket connect error appears.

2. **What port is racecontrol currently listening on in production?**
   - What we know: `rc-agent.example.toml` defaults to `ws://127.0.0.1:8080/ws/agent` and `config.rs` defaults to port 8080
   - What's unclear: Whether the production `racecontrol.toml` on server .23 overrides this
   - Recommendation: DIAG-02 confirms this. `netstat -ano | findstr LISTENING` on .23 will show exactly what port the `racecontrol.exe` process holds.

3. **Is port 3300 free on server .23?**
   - What we know: kiosk `package.json` confirms `"start": "next start -p 3300"`. The kiosk is not currently auto-starting.
   - What's unclear: Whether any other process (legacy web app, PM2 remnants, the web/ Next.js app) is occupying 3300.
   - Recommendation: DIAG-02 settles this definitively.

4. **Is server .23 IP already stable or has it drifted?**
   - What we know: STATE.md flags "Server (.23) MAC address needed for DHCP reservation — must retrieve during Phase 6." The DHCP has "drifted from .51" historically (noted in MEMORY.md).
   - What's unclear: Whether it's currently on a DHCP lease that could change, or whether someone has since set a static NIC IP.
   - Recommendation: DIAG-04 — single `ipconfig /all` on server answers this.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | None — Phase 6 has no code changes |
| Config file | N/A |
| Quick run command | N/A |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` (existing suite, run to confirm baseline is green before starting) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DIAG-01 | Staff can view pod and server logs showing URL failure patterns | manual-only | — | N/A (output is human-readable log text) |
| DIAG-02 | Staff can read port audit showing port conflicts | manual-only | — | N/A (output is netstat text) |
| DIAG-03 | Staff can confirm Edge version and settings on every pod | manual-only | — | N/A (output is reg query + sc query text) |
| DIAG-04 | Staff can confirm server IP type and MAC address | manual-only | — | N/A (output is ipconfig text) |

All four requirements produce human-readable diagnostic output, not software behavior. They are manual-only verification tasks by design.

### Sampling Rate

- **Per task commit:** Run `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` to confirm no regressions (47 existing tests must remain green — Phase 6 makes no code changes, so this is a sanity gate only).
- **Per wave merge:** Same full suite.
- **Phase gate:** All four DIAG requirements have documented findings. No code changes to test.

### Wave 0 Gaps

None — existing test infrastructure covers all phase requirements. Phase 6 creates diagnostic output artifacts (text files/docs), not software.

## Sources

### Primary (HIGH confidence)

- `deploy-staging/start-rcagent-log.bat` — confirmed log capture pattern, rc-agent log path `C:\RacingPoint\rc-agent-log.txt`
- `deploy-staging/read-log.json` — confirmed `type C:\RacingPoint\rc-agent-log.txt` as log read command
- `deploy-staging/run-log-script.json` — confirmed log script deployment pattern via HTTP + pod-agent
- `deploy-staging/pod8-edge-check.json` — confirmed `tasklist /NH | findstr msedge` pattern
- `deploy-staging/pod8-full-diag.json` — confirmed combined diagnostic command pattern
- `crates/rc-agent/src/main.rs` lines 197–202 — tracing setup: `rc_agent=info` default level, writes to stdout only
- `crates/racecontrol/src/main.rs` lines 92–98 — racecontrol tracing: `racecontrol=info,tower_http=info` default, writes to stdout only
- `crates/rc-agent/src/debug_server.rs` — debug server port 18924, `/status` endpoint available on all pods
- `kiosk/package.json` (from SUMMARY.md) — confirmed `"start": "next start -p 3300"`
- `.planning/STATE.md` — confirmed server MAC address needed before Phase 7, port 3300 vs 3000 ambiguity flagged
- `.planning/research/SUMMARY.md` — confirmed all architectural decisions, pitfall list, NSSM ban

### Secondary (MEDIUM confidence)

- Microsoft Docs: [Edge registry settings for enterprises](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-policies) — StartupBoostEnabled and BackgroundModeEnabled registry paths under `HKLM\SOFTWARE\Policies\Microsoft\Edge`
- Microsoft Docs: [Configure EdgeUpdate service](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-update-policies) — service names `EdgeUpdate` and `MicrosoftEdgeUpdate`
- MEMORY.md — pod-agent v0.5.0 on all 8 pods (Pod 8 has v0.4.0), port 8090, `cmd` field (not `command`)

### Tertiary (LOW confidence — verify during execution)

- Exact EdgeUpdate service name on venue pods: querying both `EdgeUpdate` and `MicrosoftEdgeUpdate` covers both possibilities; actual name depends on Edge channel installed
- Whether racecontrol is currently running on server .23 at all: assumed yes, but not verified since last reboot

## Metadata

**Confidence breakdown:**

- Log collection pattern: HIGH — `start-rcagent-log.bat` and `read-log.json` are real files in deploy-staging, already proven in past sessions
- Port audit commands: HIGH — `netstat -ano` is a Windows built-in, unchanged since Windows XP
- Edge registry paths: MEDIUM — paths from Microsoft docs are correct for Edge stable channel; service name ambiguity (`EdgeUpdate` vs `MicrosoftEdgeUpdate`) is LOW until confirmed on a pod
- IP/MAC identification: HIGH — `ipconfig /all` output format is deterministic; DHCP field is unambiguous
- Pod-agent access: HIGH — v0.5.0 confirmed on all pods as of Mar 11, 2026; `cmd` field confirmed in MEMORY.md

**Research date:** 2026-03-13
**Valid until:** 2026-06-13 (stable — Windows built-in commands and Edge registry paths change only on major OS/browser updates)
