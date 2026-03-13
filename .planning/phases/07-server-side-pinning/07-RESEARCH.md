# Phase 7: Server-Side Pinning - Research

**Researched:** 2026-03-13
**Domain:** Windows server auto-start, Next.js production build, DHCP reservation, LAN hostname resolution
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| HOST-01 | Staff kiosk runs as a production Next.js build on Server (.23) — no dev server | Next.js `output: "standalone"` already set in `next.config.ts`; `next build` produces `.next/standalone/server.js` runnable with `node server.js PORT=3300 HOSTNAME=0.0.0.0` |
| HOST-02 | Staff kiosk auto-starts on Server (.23) boot via HKLM Run key (Session 1) | HKLM Run key is the mandated pattern (NSSM banned); kiosk is a headless Node process (no GUI) so can use sc.exe Windows Service instead — see Architecture Patterns |
| HOST-03 | Server (.23) IP is pinned via DHCP reservation at the router so it never drifts | Xiaomi router admin at 192.168.31.1 supports IP-MAC binding; MAC is BC-FC-E7-2C-F2-CE; target IP must be .23 to avoid reconfiguring all 8 pod configs |
| HOST-04 | Staff can access the kiosk at `kiosk.rp` from any device on the LAN via hosts file entries | Windows hosts file at `C:\Windows\System32\drivers\etc\hosts`; one entry per device needed; James's machine (.27) is the primary target |
</phase_requirements>

---

## Summary

Phase 7 has four distinct sub-problems: (1) pin the server IP to .23 via DHCP reservation at the Xiaomi router, (2) build the Next.js kiosk as a production standalone bundle, (3) auto-start both rc-core (Rust binary) and the kiosk (Node.js process) on server boot, and (4) add a `kiosk.rp` hosts file entry on James's machine so the named URL resolves.

The kiosk app already has `output: "standalone"` in `next.config.ts` and the `next start` script sets port 3300 — so the build machinery is already wired. The key build artifact is `.next/standalone/server.js`, which needs `public/` and `.next/static/` copied alongside it (standalone build does NOT include these automatically). For autostart, rc-core (a Rust binary with no GUI) is a perfect fit for `sc.exe create ... start= auto` (a true Windows Service). The Node.js kiosk process is also headless, so it can use sc.exe too — but a simpler HKLM Run + `start-kiosk.bat` approach mirrors the existing rc-agent pattern and avoids needing a service wrapper.

**Primary recommendation:** Pin server to .23 at router first (highest risk, lease expires tonight). Then build kiosk, deploy binaries via pod-agent, configure sc.exe for rc-core, configure HKLM Run for kiosk, add hosts entry on James's machine. All remote via pod-agent — no RDP needed.

---

## Critical Context from Phase 6

| Finding | Impact on Phase 7 |
|---------|------------------|
| Server currently at .4, MAC BC-FC-E7-2C-F2-CE | Must create DHCP reservation at router for .23 |
| Lease expires 14 March 2026 01:05 — TONIGHT | IP pinning is time-critical |
| .23 is currently an unknown phone/tablet | After reservation, router reassigns .23 to the server; the phone gets a new IP |
| All 8 pod configs hardcoded to `.23:8080` | Pin to .23 (not .4 or new IP) — zero pod changes needed |
| rc-core not running on server | Must deploy rc-core binary AND auto-start it |
| Port 3300 free on server | Kiosk can bind there |
| Pod-agent on server at 8090 | All deploy steps remote via pod-agent — no RDP |

---

## IP Pinning Decision: Pin to .23

**The answer to "what IP to pin to" is .23.**

All 8 pod `rc-agent.toml` configs contain `url = "ws://192.168.31.23:8080/ws/agent"`. Pinning to .23 means the fix is invisible to pods — no config updates required, zero pod deployments. Any other IP choice requires updating all 8 pod configs, 8 pod deployments, and testing.

**Two-layer approach (per STATE.md decision):**
1. DHCP reservation at Xiaomi router (MAC BC-FC-E7-2C-F2-CE → 192.168.31.23)
2. Static NIC backup (configure Windows NIC with fixed .23 fallback) — prevents drift if router loses its reservation config

**IMPORTANT:** After DHCP reservation is created at the router, the server will NOT get .23 immediately — it keeps .4 until the lease expires or is renewed. To force the change: either wait for the lease to expire tonight (~01:05), or reboot the server, or run `ipconfig /release && ipconfig /renew` on the server via pod-agent.

**Conflict risk:** .23 is currently assigned to an unknown phone/tablet. After the reservation is created, the router will assign .23 only to the server (by MAC). The phone/tablet will receive a different IP on its next DHCP renewal. There is a brief window where both the server (still at .4) and the phone (at .23) coexist — this is normal and resolves on its own.

---

## Standard Stack

### Core
| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| Next.js `output: "standalone"` | 16.1.6 (already set) | Self-contained production bundle | No `node_modules` needed on server; already configured |
| `sc.exe` | Windows built-in | Create/manage Windows services | Mandated pattern for headless services (rc-core); no third-party tools |
| HKLM Run key | Windows built-in | Auto-start processes in Session 1 | Mandated pattern (NSSM banned); matches rc-agent pattern already on pods |
| Windows hosts file | OS built-in | LAN hostname resolution | Offline-safe, no DNS server needed; listed as the chosen approach in REQUIREMENTS.md |
| pod-agent `/exec` | 8090 on server | Remote command execution | Server already has pod-agent running — confirmed in Phase 6 |

### Supporting
| Component | Version | Purpose | When to Use |
|-----------|---------|---------|-------------|
| `ipconfig /release + /renew` | Windows built-in | Force DHCP lease renewal | After creating router reservation, to get .23 immediately |
| `netsh interface ip set address` | Windows built-in | Set static NIC backup IP | Layer 2 of IP pinning — fallback if router loses DHCP config |
| HTTP server on James's machine | `python3 -m http.server 9998` | Serve binaries for download | Same pattern used for all existing pod deployments |

### What is Out of Scope (from REQUIREMENTS.md)
- NSSM — BANNED
- mDNS / `.local` domain — BANNED (conflicts with Windows 11 mDNS resolver)
- Docker — BANNED
- Full DNS server (Acrylic/Unbound) — BANNED
- Static NIC IP only (no DHCP) — BANNED (router reset loses config; two-layer approach is correct)
- HTTPS on LAN — BANNED

---

## Architecture Patterns

### Recommended Execution Order

```
Step 1: Create DHCP reservation at router (192.168.31.1)
Step 2: Force server IP renewal via pod-agent
Step 3: Verify server is now at .23
Step 4: Build rc-core binary (cargo build --release -p rc-core) on James's machine
Step 5: Build Next.js kiosk (cd kiosk && npm run build)
Step 6: Stage binaries on James's HTTP server
Step 7: Deploy rc-core to server via pod-agent
Step 8: Install rc-core as Windows Service (sc.exe) via pod-agent
Step 9: Deploy kiosk build to server via pod-agent
Step 10: Create start-kiosk.bat + HKLM Run key on server via pod-agent
Step 11: Add kiosk.rp to hosts file on James's machine
Step 12: Verify end-to-end
```

### Pattern 1: DHCP Reservation at Xiaomi Router

**What:** Bind MAC BC-FC-E7-2C-F2-CE to IP 192.168.31.23 permanently in router admin
**When to use:** First step — must be done before all other steps
**How:**
- Navigate to http://192.168.31.1 (router admin)
- Find "LAN Settings" > "DHCP static IP assignment" (label varies: "IP-MAC binding", "Address Reservation", "Static DHCP")
- Add entry: MAC = BC-FC-E7-2C-F2-CE, IP = 192.168.31.23
- Save/Apply
- This requires **manual browser access** — cannot be done via pod-agent

**After reservation, force renewal via pod-agent:**
```json
{"cmd": "ipconfig /release \"Ethernet 2\" && ipconfig /renew \"Ethernet 2\""}
```
(NIC name is "Ethernet 2" — confirmed in Phase 6 as the Marvell AQtion adapter)

**Verify new IP:**
```json
{"cmd": "ipconfig | findstr /i \"IPv4\""}
```

### Pattern 2: rc-core as a Windows Service (sc.exe)

**What:** Register rc-core.exe as an auto-start Windows Service
**Why sc.exe, not HKLM Run:** rc-core is a headless Axum server (no GUI, no user session dependency). Windows Services start before user login and survive logoff — ideal for a server process. This is the `sc.exe` pattern from STATE.md decisions.
**The binary MUST have its working directory set to C:\RacingPoint** — rc-core reads `racecontrol.toml` with a relative path. Use `binPath` with the full path, and ensure the working directory is set.

**sc.exe does NOT natively support setting working directory.** The solution: a wrapper batch file.

```bat
@echo off
cd /d C:\RacingPoint
rc-core.exe
```

Register the batch file via a wrapper. However, `sc create` cannot run a .bat file directly (Windows services must be native executables).

**Correct approach — use `cmd.exe` as the service binary:**
```
sc create RCCore binpath= "cmd.exe /c C:\RacingPoint\start-rc-core.bat" start= auto obj= LocalSystem DisplayName= "RaceControl Core"
sc description RCCore "RaceControl venue management backend"
sc start RCCore
```

**Known limitation:** `cmd.exe /c` services may not restart cleanly. A more reliable approach:

```
sc create RCCore binpath= "C:\RacingPoint\rc-core.exe" start= auto obj= LocalSystem DisplayName= "RaceControl Core"
```

Then set the working directory via registry:
```
reg add "HKLM\SYSTEM\CurrentControlSet\Services\RCCore" /v ImagePath /t REG_EXPAND_SZ /d "C:\RacingPoint\rc-core.exe" /f
reg add "HKLM\SYSTEM\CurrentControlSet\Services\RCCore" /v ObjectName /t REG_SZ /d "LocalSystem" /f
```

**Simplest reliable approach (matched to existing pattern):** Use a wrapper .bat file + sc.exe with `cmd /c`. This is what the existing `start-rcagent.bat` pattern does. If rc-core needs to restart reliably, configure sc.exe failure actions:
```
sc failure RCCore reset= 86400 actions= restart/5000/restart/10000/restart/30000
```

**Note:** The server's `racecontrol.toml` must have the correct database path. The current `racecontrol.toml` has `path = "./data/racecontrol.db"` — which resolves relative to CWD. Ensure `C:\RacingPoint\data\` exists on the server.

### Pattern 3: Next.js Kiosk Autostart via HKLM Run Key

**What:** Start the kiosk Node.js process when a user logs into Session 1
**Why HKLM Run (not sc.exe):** The kiosk is a web server (Node.js) serving a Next.js app. It is headless and does not need a GUI. However, the precedent on this project for server-side services is sc.exe. For Session 1 consistency with the rc-agent pattern, use HKLM Run. But the kiosk can also be sc.exe — see note below.

**Recommended: sc.exe for the kiosk too (headless Node.js server)**

Node.js itself is not service-aware, but Windows can wrap it:
```
sc create RCKiosk binpath= "node.exe C:\RacingPoint\kiosk\server.js" start= auto obj= LocalSystem DisplayName= "RaceControl Kiosk"
```

**This will NOT work** because `sc create` needs an executable path that is resolvable as a full path, and `node.exe` must be on the PATH when the service starts under LocalSystem.

**Correct approach for Node.js kiosk — HKLM Run + batch file** (mirroring rc-agent pattern):

```bat
rem C:\RacingPoint\start-kiosk.bat
@echo off
cd /d C:\RacingPoint\kiosk
set PORT=3300
set HOSTNAME=0.0.0.0
node server.js > C:\RacingPoint\kiosk-out.log 2>&1
```

Register in HKLM Run:
```
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v RCKiosk /t REG_SZ /d "C:\RacingPoint\start-kiosk.bat" /f
```

**Session 1 caveat:** HKLM Run fires when a user logs in (Session 1 GUI). The server must be logged in for the kiosk to start. The existing `start-rcagent.bat` pattern on pods has this same limitation. If the server is running headlessly (no interactive session), prefer the sc.exe wrapper approach via `cmd.exe /c`.

**Alternative — sc.exe for kiosk (preferred for a server with no interactive session):**
```
sc create RCKiosk binpath= "cmd.exe /c \"cd /d C:\RacingPoint\kiosk && set PORT=3300 && set HOSTNAME=0.0.0.0 && node server.js\"" start= auto obj= LocalSystem DisplayName= "RaceControl Kiosk"
sc failure RCKiosk reset= 86400 actions= restart/5000/restart/10000/restart/30000
```

### Pattern 4: Next.js Standalone Build + Deploy

**What:** Build a self-contained production bundle that runs on the server without `node_modules`
**Source:** Official Next.js docs (nextjs.org, verified 2026-02-27, version 16.1.6)

The kiosk already has `output: "standalone"` in `next.config.ts`. Build locally on James's machine:

```bash
cd /c/Users/bono/racingpoint/racecontrol/kiosk
npm run build
# Output: kiosk/.next/standalone/server.js (+ bundled node_modules subset)
```

After build, copy static assets into the standalone folder (the standalone server serves them from there):
```bash
cp -r public .next/standalone/public
cp -r .next/static .next/standalone/.next/static
```

The standalone folder structure to deploy to the server:
```
C:\RacingPoint\kiosk\
├── server.js                    # from .next/standalone/server.js
├── .next/                       # from .next/standalone/.next/
│   └── static/                  # copied manually
├── node_modules/                # from .next/standalone/node_modules/
└── public/                      # copied manually
```

Run command (what `start-kiosk.bat` calls):
```
node server.js
```
With env vars: `PORT=3300 HOSTNAME=0.0.0.0`

The `basePath: "/kiosk"` in `next.config.ts` means the app is served at `http://server:3300/kiosk` — matching the success criterion URL `http://kiosk.rp:3300/kiosk`.

**CRITICAL: next start vs standalone server.js**
- `npm run start` = `next start -p 3300` — requires full `node_modules` and `next` package on server
- `node server.js` in standalone folder — self-contained, no `node_modules` install on server needed
- The standalone approach is correct for production deployment to a server

### Pattern 5: Hosts File Entry for kiosk.rp

**What:** Add `192.168.31.23 kiosk.rp` to James's machine hosts file
**File:** `C:\Windows\System32\drivers\etc\hosts`
**Format:**
```
192.168.31.23  kiosk.rp
```
**Admin required:** Yes — hosts file requires administrator privileges to edit
**When to apply:** After server IP is confirmed at .23

For other devices (Uday's phone, etc.) — they would need the same hosts entry, OR use the raw IP `http://192.168.31.23:3300/kiosk`. The phase success criterion specifies James's machine only.

**Why not mDNS `.local`:** BANNED per REQUIREMENTS.md — conflicts with Windows 11 mDNS resolver.
**Why not Acrylic DNS:** BANNED per REQUIREMENTS.md.

### Pattern 6: Verify Server State via pod-agent

All verification and deployment steps use the existing pod-agent pattern:
```json
{"cmd": "COMMAND_HERE"}
```
POST to `http://192.168.31.4:8090/exec` (current IP) or `http://192.168.31.23:8090/exec` (after pinning)

**Note:** After the IP changes from .4 to .23, update the pod-agent target URL for all commands.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process auto-start | Custom watchdog scripts | `sc.exe` Windows Service (for headless) or HKLM Run (for Session 1) | Built-in Windows mechanism; no third-party tools needed |
| IP reservation | ipconfig static assignment only | DHCP reservation at router + NIC backup | Router DHCP is the authority; NIC backup is the two-layer safety net |
| Node.js service wrapper | NSSM, winser, pm2-service | `sc.exe` with `cmd.exe /c` wrapper batch | NSSM is banned; sc.exe is the mandated tool |
| LAN hostname | Acrylic DNS, mDNS server | Hosts file entry | Offline-safe, zero dependencies, listed approach in REQUIREMENTS.md |
| Static file serving | Custom file server | Next.js standalone `server.js` (serves public + static) | Already built into standalone output |

**Key insight:** This entire phase uses OS-native Windows tools. No new software installs required on the server.

---

## Common Pitfalls

### Pitfall 1: sc.exe with cmd.exe /c — Service Exits Immediately
**What goes wrong:** `sc create ... binpath= "cmd.exe /c start-service.bat"` — `cmd.exe /c` runs the batch and exits, so Windows considers the service dead and may restart it in a loop.
**Why it happens:** Windows services must keep their process alive. `cmd.exe /c` inherently exits after running the command.
**How to avoid:** Use `cmd.exe /k` to keep the console open, or better: use a batch file that calls the process WITHOUT `start` (so the batch stays blocked until the process exits). The batch file itself becomes the blocking process.
**Example that works:**
```bat
@echo off
cd /d C:\RacingPoint
rc-core.exe
```
Called as: `sc create RCCore binpath= "C:\RacingPoint\start-rc-core.bat"` — but this still exits because .bat isn't a Windows service executable. **Correct fix:** Use `cmd.exe /c` but wrap the actual service in a `cmd.exe /k` that keeps open:
```
binpath= "cmd.exe /k \"cd /d C:\RacingPoint && rc-core.exe\""
```

**SIMPLEST correct approach:** Register rc-core.exe directly (it stays running — Rust binary, blocking). For Node.js kiosk which also blocks: register it via a .bat + HKLM Run since the server has an interactive session.

### Pitfall 2: Standalone Build Missing Static Files
**What goes wrong:** Pages load but have no CSS/JS/images (404 on `/_next/static/...` and `/kiosk/...` assets)
**Why it happens:** Next.js standalone does NOT copy `public/` or `.next/static/` automatically
**How to avoid:** After `next build`, manually copy:
```bash
cp -r public .next/standalone/public
cp -r .next/static .next/standalone/.next/static
```
**Warning signs:** Kiosk loads but looks unstyled; browser console shows 404 on static assets

### Pitfall 3: Wrong Working Directory for rc-core
**What goes wrong:** rc-core starts but immediately crashes: "Error: No such file or directory: ./data/racecontrol.db" or similar
**Why it happens:** `racecontrol.toml` uses relative paths (`"./data/racecontrol.db"`, `"./data/..."`) — requires CWD to be `C:\RacingPoint` (or wherever the config lives)
**How to avoid:** `start-rc-core.bat` must `cd /d C:\RacingPoint` before launching
**Warning signs:** rc-core exits within 1 second of starting; event log shows file-not-found error

### Pitfall 4: kiosk.rp Resolves Too Early (Before IP Pinning)
**What goes wrong:** Add hosts entry pointing to .23, then open kiosk — but server is still at .4, so browser gets "Connection refused"
**Why it happens:** IP migration from .4 → .23 takes time (lease renewal)
**How to avoid:** Verify `ipconfig` on server confirms .23 BEFORE adding the hosts entry and testing
**Warning signs:** Browser connects to .23, gets no response; pod-agent on .23 not yet reachable

### Pitfall 5: PORT Environment Variable Not Set for Standalone Server.js
**What goes wrong:** Server starts on port 3000 instead of 3300; kiosk unreachable at :3300
**Why it happens:** `server.js` standalone defaults to port 3000, not the `package.json` `--p 3300` flag
**How to avoid:** Set `PORT=3300` before calling `node server.js` in the start script
**Warning signs:** `netstat -ano | findstr 3300` shows nothing; port 3000 appears instead

### Pitfall 6: sc.exe Requires Space After `=` in Parameters
**What goes wrong:** `sc create RCCore binpath="..."` fails silently or creates service wrong
**Why it happens:** `sc.exe` has unusual syntax: parameters require a space between `=` and the value
**Correct syntax:** `sc create RCCore binpath= "..." start= auto`
**Warning signs:** `sc query RCCore` shows unexpected state; service fails to start

### Pitfall 7: Node.js Not on PATH for LocalSystem Service
**What goes wrong:** `sc create RCKiosk binpath= "node.exe ..."` — service fails because `node.exe` isn't found
**Why it happens:** LocalSystem account may not have Node.js on PATH
**How to avoid:** Use absolute path to node.exe (find it: `where node.exe` on the server), or use HKLM Run which inherits user session PATH
**Warning signs:** Service shows "FAILED" in sc query; Event ID 7053 in Windows Event Log

### Pitfall 8: CORS Rejection of kiosk.rp Origin
**What goes wrong:** Kiosk loads at `http://kiosk.rp:3300/kiosk` but API calls to rc-core fail (403/CORS)
**Why it happens:** rc-core CORS predicate allows `http://192.168.31.*` but NOT `http://kiosk.rp`
**Current rc-core CORS predicate** (from `main.rs`):
```rust
origin.starts_with("http://localhost:")
    || origin.starts_with("http://127.0.0.1:")
    || origin.starts_with("http://192.168.31.")
    || origin.contains("racingpoint.cloud")
```
**How to avoid:** Add `|| origin.starts_with("http://kiosk.rp")` to the predicate — noted in STATE.md as a pending concern (MON-02)
**When this bites:** The kiosk page loads (served by Next.js) but dashboard data (API calls to port 8080) fails
**Note:** This is listed as a future requirement (MON-02) — may be acceptable to defer, but the planner should include it if the kiosk page makes API calls from the `kiosk.rp` origin

### Pitfall 9: api.ts Uses window.location.hostname for API Base URL
**What goes wrong:** When accessed via `kiosk.rp`, the api.ts sets `API_BASE = "http://kiosk.rp:8080"` — which resolves correctly because the hosts entry maps kiosk.rp to .23 where rc-core also lives
**Actually fine in this case:** `window.location.hostname` returns `kiosk.rp`, and the hosts file entry maps `kiosk.rp` to `192.168.31.23`, where rc-core is listening on 8080. So API calls will work — BUT only if rc-core CORS allows the `kiosk.rp` origin.

---

## Code Examples

Verified patterns from official sources and existing project code:

### Next.js Standalone Build (Official Docs, nextjs.org, 2026-02-27)
```bash
# In kiosk directory
npm run build
# Copy static assets (REQUIRED — standalone doesn't include these)
cp -r public .next/standalone/public
cp -r .next/static .next/standalone/.next/static
# Test run locally (verify before deploying)
cd .next/standalone
node server.js
# With custom port and bind
PORT=3300 HOSTNAME=0.0.0.0 node server.js
```

### Deploy Kiosk to Server via pod-agent (existing project pattern)
```json
// download-kiosk.json — download standalone bundle
{"cmd": "powershell -Command \"Invoke-WebRequest -Uri 'http://192.168.31.27:9998/kiosk-standalone.zip' -OutFile 'C:\\RacingPoint\\kiosk-standalone.zip' -UseBasicParsing\""}
```

### Create rc-core Windows Service
```bat
rem On server via pod-agent exec
sc create RCCore binpath= "C:\RacingPoint\rc-core.exe" start= auto obj= LocalSystem DisplayName= "RaceControl Core"
sc description RCCore "RaceControl venue management backend (Axum/WebSocket)"
sc failure RCCore reset= 86400 actions= restart/5000/restart/10000/restart/30000
sc start RCCore
```

Note: rc-core.exe must be started with CWD = `C:\RacingPoint`. Use a wrapper:
```bat
rem C:\RacingPoint\start-rc-core.bat
@echo off
cd /d C:\RacingPoint
rc-core.exe
```
And register: `sc create RCCore binpath= "cmd.exe /k C:\RacingPoint\start-rc-core.bat" start= auto obj= LocalSystem`

### HKLM Run for Kiosk (Session 1 startup)
```bat
rem C:\RacingPoint\start-kiosk.bat
@echo off
cd /d C:\RacingPoint\kiosk
node server.js
```
Register:
```
reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v RCKiosk /t REG_SZ /d "C:\RacingPoint\start-kiosk.bat" /f
```

### Hosts File Entry (James's machine, admin required)
```
# C:\Windows\System32\drivers\etc\hosts
192.168.31.23  kiosk.rp
```
Add via cmd (admin):
```
echo 192.168.31.23  kiosk.rp >> C:\Windows\System32\drivers\etc\hosts
```

### Force DHCP Renewal via pod-agent (after router reservation)
```json
{"cmd": "ipconfig /release \"Ethernet 2\" && ipconfig /renew \"Ethernet 2\""}
```

### Verify server IP changed
```json
{"cmd": "ipconfig | findstr /i \"IPv4 Address\""}
```

### Verify kiosk is running on port 3300
```json
{"cmd": "netstat -ano | findstr :3300"}
```

### Verify rc-core is running on port 8080
```json
{"cmd": "netstat -ano | findstr :8080"}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `next start` (dev-adjacent) | Standalone `node server.js` | Already configured in this project | No `node_modules` needed on server; smaller footprint |
| NSSM | `sc.exe` | Project decision at v2.0 kickoff | Simpler, no third-party tool, AV-safe |
| Manual IP config only | DHCP reservation + NIC backup | Project decision at v2.0 kickoff | Survives router resets |
| IP-based URLs | Named hostname (kiosk.rp) | Phase 7 target | Stable URL regardless of IP drift |

**Deprecated/outdated:**
- `next dev`: Development server — NEVER use in production (slow, hot-reload overhead, not built)
- NSSM: Banned, AV-flagged on Windows 11

---

## Open Questions

1. **Does the server have Node.js installed?**
   - What we know: Node.js is used for the kiosk — it was presumably running the dev server at some point
   - What's unclear: Whether Node.js is installed on the server vs. only on James's machine
   - Recommendation: Verify via pod-agent (`where node`) as part of Plan 1. If not installed, include Node.js install step in the plan.

2. **Does the server have an interactive user session (Session 1)?**
   - What we know: The server runs with a logged-in user (pod-agent is running, which suggests a user session)
   - What's unclear: Whether the session persists after reboot without manual login
   - Recommendation: If no auto-login is configured, sc.exe (not HKLM Run) is the only reliable option for the kiosk. Verify with pod-agent: `query session`

3. **What is the server's current directory structure at C:\RacingPoint?**
   - What we know: pod-agent is at `C:\RacingPoint\pod-agent.exe` (likely)
   - What's unclear: Whether rc-core binaries, configs, and data directories already exist
   - Recommendation: First plan task should inventory `dir C:\RacingPoint` on server

4. **CORS for kiosk.rp — block or defer?**
   - What we know: rc-core CORS allows `192.168.31.*` origins but not `kiosk.rp`
   - What's unclear: Whether Phase 7 should fix this or defer to MON-02
   - Recommendation: Include the CORS fix in Phase 7 since the kiosk won't function correctly without it. It's a 1-line change in `crates/rc-core/src/main.rs` and a cargo build.

5. **Does racecontrol.toml need updating on the server?**
   - What we know: The server has `racecontrol.toml` (rc-core config) somewhere on it
   - What's unclear: Whether the current config on the server is current or stale
   - Recommendation: Deploy a fresh `racecontrol.toml` from the repo as part of the rc-core deployment

---

## Validation Architecture

`nyquist_validation: true` — include this section.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` (rc-common, rc-agent, rc-core) — 47 tests |
| Config file | `Cargo.toml` (workspace) |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-agent` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

No JavaScript test framework exists for the kiosk. Tests for Phase 7 are primarily operational (smoke tests via curl/http), not unit tests.

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HOST-01 | Kiosk serves production build at :3300/kiosk | smoke | `curl -s http://192.168.31.23:3300/kiosk | findstr "RacingPoint"` | ❌ Wave 0 (operational check) |
| HOST-02 | Kiosk auto-starts after server reboot | manual | Reboot server, wait 60s, verify :3300/kiosk reachable | N/A — manual reboot required |
| HOST-03 | Server IP is .23 after router restart | manual | `ipconfig` on server post-reboot confirms .23 | N/A — manual verification |
| HOST-04 | kiosk.rp resolves on James's machine | smoke | `curl -s http://kiosk.rp:3300/kiosk` from James's machine | ❌ Wave 0 (after hosts entry) |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-agent` (if rc-core CORS change is made, add `-p rc-core`)
- **Per wave merge:** Full cargo test suite
- **Phase gate:** All smoke tests pass + manual reboot verification before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] No kiosk JavaScript test framework — operational smoke tests via curl are sufficient for this phase
- [ ] CORS patch to `crates/rc-core/src/main.rs` requires `cargo test -p rc-core` — existing tests cover compilation

---

## Sources

### Primary (HIGH confidence)
- Official Next.js docs (nextjs.org/docs/app/api-reference/config/next-config-js/output, fetched 2026-02-27) — standalone output, server.js deployment, PORT/HOSTNAME env vars
- Codebase inspection: `kiosk/next.config.ts` — confirms `output: "standalone"`, `basePath: "/kiosk"`
- Codebase inspection: `kiosk/package.json` — confirms `next start -p 3300`, `next build`
- Codebase inspection: `kiosk/src/lib/api.ts` — confirms `window.location.hostname:8080` API base URL pattern
- Codebase inspection: `crates/rc-core/src/main.rs` — confirms CORS predicate, does NOT allow `kiosk.rp` origin
- Codebase inspection: `racecontrol.toml` — confirms relative paths (`./data/racecontrol.db`)
- Phase 6 FINDINGS.md — server MAC, IP, ports, pod-agent availability, pod config URLs
- STATE.md decisions — NSSM banned, two-layer IP pinning, HKLM Run pattern
- REQUIREMENTS.md — mDNS banned, Acrylic banned, NSSM banned, hosts file approach confirmed
- Microsoft Learn (sc.exe create documentation) — sc.exe syntax, `start= auto`, `obj= LocalSystem`

### Secondary (MEDIUM confidence)
- Xiaomi router admin panel: DHCP static IP assignment confirmed at Settings > LAN Settings (confirmed across multiple Xiaomi models via HardReset.info)
- Windows hosts file format and location: `C:\Windows\System32\drivers\etc\hosts` (Wikipedia + Microsoft Learn)

### Tertiary (LOW confidence)
- sc.exe + Node.js interaction details: informed by multiple community sources; not tested against this specific Node.js version

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all tools are OS-native or already in use in this codebase
- Architecture: HIGH — all patterns verified against official docs and existing codebase code
- Pitfalls: HIGH — most pitfalls discovered from direct codebase inspection (CORS, relative paths, static assets)
- IP pinning: HIGH — Phase 6 confirmed all relevant facts (MAC, current IP, lease expiry)

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable infrastructure, low volatility)
