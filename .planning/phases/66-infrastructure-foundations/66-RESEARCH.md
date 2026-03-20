# Phase 66: Infrastructure Foundations - Research

**Researched:** 2026-03-20
**Domain:** Network DHCP stability, rc-agent HTTP exec, comms-link exec_request protocol
**Confidence:** HIGH — all findings from direct source file inspection, no guesswork

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Belt-and-suspenders: static IP on server NIC AND TP-Link router DHCP reservation for MAC 10-FF-E0-80-B1-A7 → 192.168.31.23
- Use rc-agent :8090 exec endpoint (already running on server) for James → Server .23 exec
- Server Tailscale IP unknown — discover during phase execution via `tailscale status` on server
- Prefer Tailscale IP if available; LAN .23 is fallback
- Use comms-link exec_request protocol (already shipping) for James → Bono VPS exec
- Add 4 new commands to COMMAND_REGISTRY: `activate_failover`, `deactivate_failover`, `racecontrol_health`, `config_apply`
- Keep existing commands alongside new ones
- Approval tiers: `racecontrol_health` = AUTO, `config_apply` = NOTIFY, `activate_failover`/`deactivate_failover` = NOTIFY
- James does NOT need direct exec on individual pods

### Claude's Discretion
- Whether to disable DHCP client on server NIC or keep it with DHCP reservation
- Whether to use `netsh` or `New-NetIPAddress` (PowerShell) for static IP assignment
- DNS settings for static NIC config (likely 192.168.31.1 as gateway + DNS)
- TP-Link router admin UI steps (model-dependent)
- DHCP pool range adjustment if router doesn't support reservation outside pool

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INFRA-01 | Server .23 has DHCP reservation pinned to MAC 10-FF-E0-80-B1-A7 | Static IP commands documented, reservation workflow documented, verification step defined |
| INFRA-02 | James can execute commands on Server .23 via rc-agent :8090 over Tailscale IP | /exec endpoint contract fully documented from source, request/response format confirmed |
| INFRA-03 | James can execute commands on Bono VPS via comms-link exec_request protocol | Bono exec_request handling confirmed (currently stubs with reject), new COMMAND_REGISTRY entries fully specified |
</phase_requirements>

---

## Summary

Phase 66 has three independent tasks: pin server IP, verify James→Server exec path, and wire James→Bono exec path. All three have strong existing infrastructure — this phase is about configuration and a small code addition, not building from scratch.

The biggest surprise: **Bono's exec_request handler is not yet implemented.** `bono/index.js` line 146-159 explicitly logs "not implemented on Bono side yet" and returns a reject result. Phase 66 must implement the Bono-side ExecHandler. James's side already works fully — `james/index.js` handles `exec_request` by calling `execHandler.handleExecRequest(msg)`.

The second critical finding: **rc-agent `/exec` takes a shell string, not a structured command.** The `ExecRequest` struct has `cmd: String` which is passed to `cmd /C <cmd>`. This means James POSTs `{"cmd": "tailscale status"}` not `{"binary": "tailscale", "args": [...]}`. No security registry on the server side — it's a raw shell exec.

**Primary recommendation:** Three sequential tasks — (1) set static IP + router reservation, (2) test rc-agent POST exec via curl from James, (3) implement Bono ExecHandler + add 4 COMMAND_REGISTRY entries + test round-trip.

---

## Standard Stack

### Core (already in place — no new dependencies)
| Library/Tool | Version | Purpose | Status |
|---|---|---|---|
| rc-agent | running on server .23 | HTTP exec endpoint :8090 | Already deployed, binds 0.0.0.0:8090 |
| comms-link | v2.0 (shipping) | WebSocket exec_request protocol | James side works; Bono side needs ExecHandler |
| PowerShell | built-in Win11 | Static IP assignment on server | New-NetIPAddress cmdlet preferred |
| netsh | built-in Win11 | Static IP (alternative) | Works but less readable than PowerShell |
| Tailscale | installed on server | Stable IP overlay for James→Server | IP must be discovered on-site |

### No New npm/cargo Dependencies Needed
All code assets exist. Only changes:
- Add 4 entries to `shared/exec-protocol.js` COMMAND_REGISTRY
- Add ExecHandler wiring to `bono/index.js`

---

## Architecture Patterns

### INFRA-01: Belt-and-Suspenders IP Stability

**Approach:** Set static IP on the server's Gigabyte Z870 NIC (via PowerShell) AND add DHCP reservation in TP-Link router. Static IP wins if DHCP is unreachable. DHCP reservation is the safety net if static IP config gets cleared.

**Recommended: Keep DHCP client enabled with static IP assigned manually.**
Do NOT disable DHCP client — disabling it is harder to recover from remotely if the static IP is wrong. Instead assign the static IP and leave DHCP client intact. The static IP assignment via `New-NetIPAddress` coexists with DHCP on Windows.

**PowerShell commands for static IP on Windows Server (NIC: Gigabyte Z870)**
```powershell
# Step 1: Find the interface index
Get-NetAdapter | Select-Object Name, InterfaceIndex, MacAddress, Status

# Step 2: Remove existing IP (if set)
# Replace "Ethernet" with actual adapter name from step 1
Remove-NetIPAddress -InterfaceAlias "Ethernet" -Confirm:$false -ErrorAction SilentlyContinue
Remove-NetRoute -InterfaceAlias "Ethernet" -Confirm:$false -ErrorAction SilentlyContinue

# Step 3: Set static IP
New-NetIPAddress -InterfaceAlias "Ethernet" -IPAddress 192.168.31.23 -PrefixLength 24 -DefaultGateway 192.168.31.1

# Step 4: Set DNS
Set-DnsClientServerAddress -InterfaceAlias "Ethernet" -ServerAddresses 192.168.31.1
```

**Alternative: netsh (if PowerShell remoting unavailable)**
```cmd
netsh interface ip set address "Ethernet" static 192.168.31.23 255.255.255.0 192.168.31.1
netsh interface ip set dns "Ethernet" static 192.168.31.1
```

**TP-Link DHCP Reservation (general workflow — exact UI varies by model):**
1. Login to router admin UI at http://192.168.31.1
2. Navigate to DHCP → Address Reservation (or Binding)
3. Add entry: MAC `10-FF-E0-80-B1-A7` → IP `192.168.31.23`
4. Note: TP-Link typically allows reservation of IPs outside the DHCP pool range. If it rejects .23 because the pool starts at .100, either adjust the pool to start at .24 or enable .23 as a reserved address.
5. Save and reboot router for reservation to take effect

**DHCP Pool Concern (Claude's Discretion):**
If the TP-Link router's DHCP pool covers .23 (e.g., pool is 192.168.31.2–192.168.31.254), reserving .23 is valid — the router will not assign it dynamically. If the reservation feature rejects .23 for being "in use" by static, set the pool to start at .24.

### INFRA-02: James → Server .23 Exec via rc-agent :8090

**The /exec endpoint contract (confirmed from remote_ops.rs lines 407-524):**

```
POST http://<server-ip>:8090/exec
Content-Type: application/json

{"cmd": "<shell command string>", "timeout_ms": 10000}
```

Response on success (HTTP 200):
```json
{"success": true, "exit_code": 0, "stdout": "...", "stderr": ""}
```

Response on failure (HTTP 500):
```json
{"success": false, "exit_code": 1, "stdout": "", "stderr": "error message"}
```

Response on slot exhaustion (HTTP 429):
```json
{"success": false, "exit_code": null, "stdout": "", "stderr": "Too many concurrent commands (8 max)..."}
```

**Key facts:**
- `cmd` is a raw shell string passed to `cmd /C <cmd>` — NOT a structured binary+args
- `timeout_ms` is optional, defaults to 10,000ms
- `detached: true` fires-and-forgets (for self-restart only; James should not use this)
- Semaphore: 8 concurrent slots. Each request blocks until slot available or 429
- `Connection: close` is set on every response (CLOSE_WAIT prevention built-in)
- Special sentinel: `"RCAGENT_SELF_RESTART"` triggers `relaunch_self()` — never send this accidentally

**How James POSTs (no Rust needed — curl from bash):**
```bash
# Discover Tailscale IP first (on server, run: tailscale status)
# Then from James:
curl -s -X POST http://<tailscale-ip>:8090/exec \
  -H "Content-Type: application/json" \
  -d '{"cmd": "tailscale status", "timeout_ms": 5000}'
```

**Tailscale IP Discovery:**
- From server: `tailscale status` shows own IP in first line
- From James: `tailscale status` shows all peers including server
- Once discovered, hardcode in a config or environment variable — Tailscale IPs are stable per device
- LAN fallback: `http://192.168.31.23:8090/exec` always works on local network

**rc-agent is also running on server .23** — CLAUDE.md §Server Services confirms `rc-agent remote_ops 8090` runs on "All pods" including the server. The server hosts both racecontrol (port 8080) AND rc-agent (port 8090).

### INFRA-03: James → Bono VPS via comms-link exec_request

**Current state (confirmed from bono/index.js lines 144-160):**
```javascript
// Handle exec_request from James (symmetric -- James can also request Bono to execute)
if (msg.type === 'exec_request') {
  console.log(`[EXEC] Received exec_request from James: ${msg.payload?.command} (not implemented on Bono side yet)`);
  // For now, reject with "not supported"
  const rejectPayload = { ... stderr: 'Bono-side execution not yet implemented', tier: 'rejected' };
  ws.send(rejectRaw);
  return;
}
```

**This must be replaced with a real ExecHandler wiring.** The James-side pattern (from `james/index.js` lines 100-113) is the model:

```javascript
// James pattern (already working):
const execHandler = new ExecHandler({
  sendResultFn: (execId, result) => {
    connectionMode.sendCritical('exec_result', { execId, ...result });
  },
  notifyFn: async (text) => { client.send('message', { text, channel: 'whatsapp_notify' }); },
  safeEnv: buildSafeEnv(),
});

// In message handler:
if (msg.type === 'exec_request') {
  execHandler.handleExecRequest(msg);
  return;
}
```

**Bono's equivalent** should:
1. Import ExecHandler from `../james/exec-handler.js` (it's in james/ but is reusable)
2. Instantiate with `sendResultFn` that sends `exec_result` back via `ws.send(createMessage('exec_result', 'bono', ...))`
3. Wire into the `exec_request` branch of `wireBono()` in `bono/index.js`

**Note on buildSafeEnv() for Bono VPS (Linux):** The current `buildSafeEnv()` in `shared/exec-protocol.js` lines 134-142 includes `SYSTEMROOT` and Windows TEMP paths. On the Bono VPS (Linux), `SYSTEMROOT` will be undefined but that's harmless. PATH and HOME will work normally on Linux.

**The 4 new COMMAND_REGISTRY entries:**

The failover commands operate on Bono's VPS. `activate_failover` and `deactivate_failover` start/stop the cloud racecontrol process. The ecosystem.config.cjs shows pm2 manages `comms-link` — racecontrol on the VPS would also be managed by pm2.

```javascript
// Add to COMMAND_REGISTRY in shared/exec-protocol.js:

racecontrol_health: {
  binary: 'curl',
  args: ['-s', '-o', '/dev/null', '-w', '%{http_code}', 'http://localhost:8080/api/v1/health'],
  tier: ApprovalTier.AUTO,
  timeoutMs: 5000,
  description: 'Check cloud racecontrol HTTP health endpoint',
},
activate_failover: {
  binary: 'pm2',
  args: ['start', 'racecontrol'],
  tier: ApprovalTier.NOTIFY,
  timeoutMs: 15000,
  cwd: '/root/racecontrol',
  description: 'Start cloud racecontrol process (activate failover mode)',
},
deactivate_failover: {
  binary: 'pm2',
  args: ['stop', 'racecontrol'],
  tier: ApprovalTier.NOTIFY,
  timeoutMs: 15000,
  description: 'Stop cloud racecontrol process (deactivate failover mode)',
},
config_apply: {
  binary: 'git',
  args: ['pull', 'origin', 'main'],
  tier: ApprovalTier.NOTIFY,
  timeoutMs: 30000,
  cwd: '/root/racecontrol',
  description: 'Pull latest config/code on Bono VPS',
},
```

**Open question on activate_failover/deactivate_failover:** The exact pm2 app name for racecontrol on the VPS is unknown — it's not in ecosystem.config.cjs (only `comms-link` is listed). This must be verified on the VPS before coding. Alternative: use `pm2 start /root/racecontrol/start-racecontrol.sh` or the cargo binary path.

**How James sends an exec_request to Bono:**

The `wireBono()` function already has `sendExecRequest(ws, { command, reason })` at line 83. But James sends TO Bono — so James uses the existing protocol:

```javascript
// From james/index.js, using sendTaskRequest pattern:
// James sends exec_request message via comms-link WebSocket:
const execId = `ex_${randomUUID().slice(0, 8)}`;
const payload = { execId, command: 'racecontrol_health', reason: 'Phase 66 verification', requestedBy: 'james' };
client.send('exec_request', payload);
```

The result comes back as `exec_result` message from Bono. James's message handler at line 229 currently does not handle `exec_result` — it falls through to the `console.log('Received:', ...)` catch-all. The planner should note that James needs an `exec_result` handler too (or it just logs, which is acceptable for now).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Bono exec handler | Custom message router in bono/index.js | Import and instantiate ExecHandler class from james/exec-handler.js | ExecHandler already has dedup, tier routing, timeout, truncation |
| Static IP on server | Custom script | PowerShell New-NetIPAddress or netsh | Built-in, idempotent, well-tested |
| Tailscale IP discovery | Custom API calls | `tailscale status` CLI | Simplest, already installed |
| rc-agent auth | Token/PSK layer | None needed | rc-agent has no auth — it relies on network-level access control (Tailscale or LAN) |

---

## Common Pitfalls

### Pitfall 1: NIC Adapter Name is Not "Ethernet"
**What goes wrong:** PowerShell commands fail because the Gigabyte Z870 NIC might be named "Ethernet 2", "Local Area Connection", or similar.
**Why it happens:** Windows names NICs based on install order.
**How to avoid:** Always run `Get-NetAdapter` first and use the exact name from output.
**Warning signs:** `Remove-NetIPAddress: No matching MSFT_NetIPAddress found`

### Pitfall 2: Static IP + DHCP Conflict
**What goes wrong:** After static IP is set, Windows DHCP client also gets an IP, resulting in two IPs on the NIC.
**Why it happens:** Setting a static IP via PowerShell does not disable DHCP client by default.
**How to avoid:** Run `Set-NetIPInterface -InterfaceAlias "Ethernet" -Dhcp Disabled` after setting static, OR use `netsh` which sets static mode automatically.
**Warning signs:** `Get-NetIPAddress` shows two IPv4 addresses on same interface.

### Pitfall 3: rc-agent /exec Returns HTTP 500 for Non-Zero Exit Code
**What goes wrong:** A command succeeds in intent but returns non-zero exit (e.g., `ping -n 1 192.168.31.1` on some Windows configs), causing rc-agent to return HTTP 500.
**Why it happens:** `remote_ops.rs` line 507-510: only returns HTTP 200 if `out.status.success()` — any non-zero exit = 500.
**How to avoid:** Check both HTTP status code AND `success`/`exit_code` fields in response. For verification, parse the response body, not just the HTTP status.

### Pitfall 4: Bono ExecHandler buildSafeEnv() Windows Paths on Linux
**What goes wrong:** `buildSafeEnv()` sets `SYSTEMROOT: process.env.SYSTEMROOT || 'C:\\Windows'` — on Linux the fallback is a Windows path string, but it's just set as an env var and ignored by Linux processes.
**Why it happens:** buildSafeEnv() was written for James (Windows).
**How to avoid:** This is harmless — Linux commands don't use SYSTEMROOT. No fix needed.

### Pitfall 5: pm2 App Name for Racecontrol on Bono VPS is Unknown
**What goes wrong:** `pm2 start racecontrol` or `pm2 stop racecontrol` may fail if the pm2 app is registered under a different name.
**Why it happens:** ecosystem.config.cjs only shows `comms-link` — no racecontrol entry exists in the current config.
**How to avoid:** Before writing activate_failover/deactivate_failover commands, run `pm2 list` on Bono VPS to verify the app name. If racecontrol is not yet managed by pm2, the command may need to be a shell startup script path instead.

### Pitfall 6: exec_result Not Handled on James Side
**What goes wrong:** James sends exec_request to Bono, Bono processes and sends back exec_result, but James has no handler — the result is swallowed by `console.log('Received:', ...)`.
**Why it happens:** James's message handler has no `exec_result` branch (james/index.js line 368: `console.log('Received:', JSON.stringify(msg))`).
**How to avoid:** Add `exec_result` handler in james/index.js message listener — at minimum log it clearly, optionally store result by execId for programmatic use.

### Pitfall 7: DHCP Reservation Outside Pool Range (TP-Link)
**What goes wrong:** TP-Link router rejects the reservation for 192.168.31.23 because the DHCP pool starts at .23 or .1 and the UI doesn't allow reserving IPs that are "in use".
**Why it happens:** Some TP-Link models require reserved IPs to be either inside the pool or outside it — UI enforces this inconsistently.
**How to avoid:** If reservation fails, adjust DHCP pool start to 192.168.31.100 (or similar) first, then add the reservation. This also prevents future conflicts.

---

## Code Examples

### rc-agent POST /exec — Exact Request Format
```bash
# Source: remote_ops.rs ExecRequest struct (lines 407-415)
# cmd is a shell string passed to cmd /C
# timeout_ms is optional (default 10000)
curl -s -X POST http://192.168.31.23:8090/exec \
  -H "Content-Type: application/json" \
  -d '{"cmd": "ipconfig /all", "timeout_ms": 5000}'

# Response (HTTP 200 on success, 500 on non-zero exit):
# {"success":true,"exit_code":0,"stdout":"Windows IP Configuration...","stderr":""}
```

### New COMMAND_REGISTRY Entry Pattern
```javascript
// Source: shared/exec-protocol.js (confirmed pattern from lines 28-127)
// Must match: { binary, args, tier, timeoutMs, description, cwd? }
racecontrol_health: {
  binary: 'curl',
  args: ['-s', '-o', '/dev/null', '-w', '%{http_code}', 'http://localhost:8080/api/v1/health'],
  tier: ApprovalTier.AUTO,
  timeoutMs: 5000,
  description: 'Check cloud racecontrol HTTP health endpoint',
},
```

### Bono ExecHandler Wiring Pattern
```javascript
// Source: james/exec-handler.js + james/index.js (confirmed working pattern)
// bono/index.js wireBono() function needs these additions:

import { ExecHandler } from '../james/exec-handler.js';
import { buildSafeEnv } from '../shared/exec-protocol.js';

// Inside wireBono({ wss, ... }):
const bonoExecHandler = new ExecHandler({
  sendResultFn: (execId, result) => {
    // Send exec_result back to James via the WebSocket
    for (const client of wss.clients) {
      if (client.readyState === 1) {
        client.send(createMessage('exec_result', 'bono', { execId, ...result }));
        break;
      }
    }
  },
  notifyFn: async (text) => {
    // Bono can send WhatsApp directly
    alertManager?.handleNotification?.({ text }) ?? console.log('[EXEC-NOTIFY]', text);
  },
  safeEnv: buildSafeEnv(),
});

// Replace the stub in message handler:
if (msg.type === 'exec_request') {
  bonoExecHandler.handleExecRequest(msg);
  return;
}
```

### James exec_result Handler (missing, should be added)
```javascript
// Add to james/index.js message handler (after exec_request handler):
// Source: bono/index.js lines 135-142 (Bono already handles this from James side)
if (msg.type === 'exec_result') {
  console.log(`[EXEC] Result for ${msg.payload?.execId}: ${msg.payload?.command} exitCode=${msg.payload?.exitCode}`);
  if (msg.payload?.stdout) console.log(`[EXEC] stdout: ${msg.payload.stdout.slice(0, 200)}`);
  if (msg.payload?.stderr) console.log(`[EXEC] stderr: ${msg.payload.stderr.slice(0, 200)}`);
  return;
}
```

### PowerShell Static IP (Server Side)
```powershell
# Source: Windows PowerShell networking cmdlets (standard)
# Run on server .23 as ADMIN

# 1. Find adapter name
Get-NetAdapter | Where-Object {$_.Status -eq 'Up'} | Select Name, MacAddress

# 2. Set static IP (replace "Ethernet" with actual name)
New-NetIPAddress -InterfaceAlias "Ethernet" -IPAddress 192.168.31.23 `
  -PrefixLength 24 -DefaultGateway 192.168.31.1

# 3. Disable DHCP client on this interface
Set-NetIPInterface -InterfaceAlias "Ethernet" -Dhcp Disabled

# 4. Set DNS
Set-DnsClientServerAddress -InterfaceAlias "Ethernet" -ServerAddresses 192.168.31.1

# 5. Verify
Get-NetIPAddress -InterfaceAlias "Ethernet" -AddressFamily IPv4
```

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual verification (no automated test suite for network/infra) |
| Config file | None |
| Quick run command | `ping -n 1 192.168.31.23` (from James, after server reboot) |
| Full suite command | See Phase Requirements → Test Map below |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Command | Notes |
|--------|----------|-----------|---------|-------|
| INFRA-01 | Server .23 responds after reboot | Manual smoke | Reboot server → wait 2 min → `ping 192.168.31.23` from James | Must succeed within 2 min of server boot |
| INFRA-02 | rc-agent exec returns output | Manual smoke | `curl -s -X POST http://<server-ip>:8090/exec -H "Content-Type: application/json" -d '{"cmd":"hostname"}'` | Run from James, expect `{"success":true,...}` |
| INFRA-03 | exec_request round-trip via comms-link | Manual smoke | Send `racecontrol_health` exec_request, confirm `exec_result` in James logs | Requires Bono ExecHandler to be deployed first |

### Wave 0 Gaps
- [ ] No automated tests exist for network/DHCP verification — manual-only is intentional and acceptable
- [ ] `racecontrol_health` curl target `http://localhost:8080/api/v1/health` — verify this endpoint exists on Bono VPS before adding the command (the racecontrol process may not be running yet)
- [ ] pm2 app name for racecontrol on Bono VPS — run `pm2 list` before writing activate_failover/deactivate_failover binary/args

---

## Open Questions

1. **pm2 app name for racecontrol on Bono VPS**
   - What we know: ecosystem.config.cjs only has `comms-link`. No racecontrol pm2 config in the repo.
   - What's unclear: Is racecontrol running on VPS at all? What is its pm2 name if so?
   - Recommendation: Planner should include a task step to SSH/exec `pm2 list` on Bono VPS as part of the failover command definition task. The activate_failover/deactivate_failover args may need adjustment based on what's found.

2. **Tailscale IP of server .23**
   - What we know: Server has Tailscale installed (CLAUDE.md shows POS PC has Tailscale at 100.95.211.1). Server Tailscale IP is undiscovered.
   - What's unclear: Whether server .23 has Tailscale active at all.
   - Recommendation: Plan must include a step to run `tailscale status` on server via LAN rc-agent first (`http://192.168.31.23:8090/exec`), then use the discovered Tailscale IP for subsequent verification.

3. **Server NIC adapter name**
   - What we know: NIC is a Gigabyte Z870 board (changed 2026-03-17).
   - What's unclear: Exact Windows adapter name.
   - Recommendation: Plan's static IP task must start with `Get-NetAdapter` before running `New-NetIPAddress`.

4. **racecontrol health endpoint on Bono VPS**
   - What we know: Local racecontrol exposes `GET /api/v1/fleet/health` at port 8080 (from CLAUDE.md).
   - What's unclear: Whether the cloud racecontrol on srv1422716 exposes the same endpoint.
   - Recommendation: For `racecontrol_health` command, use a conservative target like `http://localhost:8080/health` or `http://localhost:8080/api/v1/health` — verify on VPS.

---

## Sources

### Primary (HIGH confidence)
- `C:/Users/bono/racingpoint/comms-link/shared/exec-protocol.js` — COMMAND_REGISTRY structure, ApprovalTier values, buildSafeEnv()
- `C:/Users/bono/racingpoint/comms-link/shared/protocol.js` — MessageType.exec_request, exec_result, createMessage()
- `C:/Users/bono/racingpoint/comms-link/james/exec-handler.js` — ExecHandler class, full implementation
- `C:/Users/bono/racingpoint/comms-link/james/index.js` — How exec_request is wired on James side (working)
- `C:/Users/bono/racingpoint/comms-link/bono/index.js` — Bono exec_request stub (line 144-160: "not implemented")
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/remote_ops.rs` (lines 407-525) — ExecRequest struct, exact JSON contract, semaphore, timeout handling
- `C:/Users/bono/racingpoint/racecontrol/CLAUDE.md` — Network map, server services, key paths

### Secondary (MEDIUM confidence)
- PowerShell `New-NetIPAddress` / `Set-NetIPInterface` cmdlets — standard Windows networking, well-documented
- TP-Link DHCP reservation workflow — general TP-Link UI pattern, exact steps model-dependent

---

## Metadata

**Confidence breakdown:**
- INFRA-01 (static IP): HIGH — PowerShell commands are standard, DHCP reservation pattern well-known
- INFRA-02 (rc-agent exec): HIGH — ExecRequest struct and response format read directly from source
- INFRA-03 (comms-link exec): HIGH for protocol; MEDIUM for Bono-side pm2 app name (unknown)

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (stable infrastructure; comms-link v2.0 API frozen)
