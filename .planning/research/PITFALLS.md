# Pitfalls Research

**Domain:** Windows LAN kiosk URL reliability — local DNS, static IPs, Edge kiosk mode, process supervision, port conflicts
**Researched:** 2026-03-13
**Confidence:** HIGH (combination of verified production issues already hit in v1.0 + current Microsoft docs + community post-mortems)

---

## Critical Pitfalls

### Pitfall 1: Session 0 GUI Blindness (Already Hit in v1.0 — Do Not Repeat)

**What goes wrong:**
Any Windows service (SYSTEM account) that restarts rc-agent also restarts it in Session 0. The lock screen at `127.0.0.1:18923` is a GUI surface — it requires Session 1. The process is alive, the WebSocket reconnects, and the HTTP server binds the port — but `msedge --kiosk 127.0.0.1:18923` in Session 0 displays nothing. Customers see a blank pod.

**Why it happens:**
Windows Session 0 Isolation (introduced Vista, permanent since Win7) prohibits Session 0 processes from rendering to the interactive desktop. SYSTEM services, NSSM-managed services, and any `sc.exe`-started process inherits Session 0. The process looks healthy from a networking standpoint because the HTTP server is running.

**How to avoid:**
- Never start rc-agent from a Windows Service (SYSTEM). The HKLM `Run` key (`start-rcagent.bat`) is the correct pattern — it executes at user login in Session 1.
- For the staff kiosk (Next.js on Server .23), use the same HKLM `Run` key pattern OR a Task Scheduler task with trigger "At log on of specific user" and option "Run only when user is logged on." This guarantees Session 1.
- The critical test: after restart, can you actually see the window on screen? Process running + port open is a necessary but not sufficient check.

**Warning signs:**
`netstat -ano` shows port 18923 or 3000 LISTENING; `tasklist` shows msedge.exe and node.exe; but the physical monitor shows a black screen or Windows desktop.

**Phase to address:** Phase 1 (diagnose) must confirm Session 1 startup for all services. Phase 2 (staff kiosk pinning) must use HKLM Run or Scheduled Task "Run only when logged on."

---

### Pitfall 2: Edge `--kiosk` Flag Silently Drops on Auto-Update

**What goes wrong:**
Edge auto-updates in the background on all 8 pods. After an update, Edge may: (a) show a white screen on launch (confirmed with Edge 128.0.2739.42), (b) ignore `--kiosk` and open the normal browser frame, (c) launch into the "Welcome to Edge" first-run experience despite `--no-first-run`, or (d) require a profile migration step that blocks the kiosk URL from loading.

In the Edge 128 incident, organizations had to roll back to version 124 venue-wide. A hotfix was released in 128.0.2739.54 but this confirms updates can break kiosk behavior between patch versions.

**Why it happens:**
Microsoft periodically changes how kiosk mode responds to command-line flags. Edge updates ship silently to Windows machines without operator approval unless Group Policy blocks them. InPrivate mode (which `--kiosk` forces) can break when the profile directory has incompatible cached state from a prior version.

**How to avoid:**
- Block Edge auto-updates on pods via Group Policy: `Computer Configuration → Administrative Templates → Microsoft Edge Update → Update Policy Override`. Set to "Updates disabled" or "Manual updates only."
- Pin to a known-good Edge version using a Group Policy template or simply disable `EdgeUpdate` service on pods: `sc config edgeupdate start=disabled && sc config edgeupdatem start=disabled`.
- Add Edge version to the deploy verification checklist. Know the current version on all pods (`msedge --version` via pod-agent).
- Never rely on `--no-first-run` alone. Also pass `--user-data-dir=C:\RacingPoint\EdgeKiosk` to isolate the profile from system Edge updates affecting the default profile.

**Warning signs:**
Pod lock screen shows blank white or the normal Edge chrome (address bar visible). Staff kiosk shows pod as Online but customers report broken screen. Edge version changed in Task Manager or `msedge --version` output differs from expected.

**Phase to address:** Phase 2 (pod lock screen fix) must pin Edge version + add `--user-data-dir` flag.

---

### Pitfall 3: `.local` DNS Conflicts with mDNS

**What goes wrong:**
If the custom hostname chosen uses the `.local` TLD (e.g., `kiosk.local`), Windows 11's built-in mDNS resolver (via dnscache service) handles it differently than a conventional unicast DNS query. Resolution is unreliable: it depends on multicast UDP packets reaching all clients, which any managed switch, VLAN, or firewall rule can silently block. Worse, installing Apple Bonjour on any pod (e.g., from iTunes, Apple Music, or some gaming peripherals) creates a second mDNS responder that conflicts with Windows' native one, causing resolution to flip between correct and incorrect IPs.

On Windows 10 1809+, Microsoft changed the precedence of `.local` resolution: mDNS now takes priority over the Windows HOSTS file for `.local` names on some configurations, meaning hosts file entries can be overridden by a multicast response from a different device.

**Why it happens:**
`.local` is a special-use domain standardized for mDNS (RFC 6762). It was never intended for use with authoritative DNS servers. Windows resolvers implement a complex fallback chain (mDNS → LLMNR → DNS → NetBIOS), and the order changed between Windows 10 versions.

**How to avoid:**
- Use a non-`.local` TLD for the custom hostname. `.rp` or `.racingpoint` are safe choices — they are not delegated TLDs, not used by mDNS, and Windows resolves them via the hosts file or DNS server only.
- The simplest reliable approach for a small LAN: add entries to `C:\Windows\System32\drivers\etc\hosts` on every client. `kiosk.rp` → `192.168.31.23`. No DNS server needed, zero mDNS interference.
- If deploying a DNS server (e.g., dnsmasq on the router or a Pi-hole), avoid `.local` in the zone and configure Windows clients to use the DNS server IP as their primary DNS.

**Warning signs:**
`ping kiosk.local` works from James's machine (.27) but fails from pods. Hostname resolves correctly after `ipconfig /flushdns` but breaks again after 60 seconds. Resolution varies between pods. `nslookup kiosk.local` returns the wrong IP.

**Phase to address:** Phase 3 (local DNS) must choose `.rp` or similar non-`.local` TLD. Hosts-file deployment must cover all 8 pods + server + James's workstation.

---

### Pitfall 4: DHCP Drift Survives "Static IP" Configuration

**What goes wrong:**
Setting a static IP on the Windows NIC (via `netsh` or Settings) on the server eliminates DHCP for that adapter — but the server was showing `.23` in monitoring while DHCP had drifted it to `.51` (as already observed). If a static IP is set to the drifted address (`.51`) rather than the intended address (`.23`), nothing breaks immediately. But if the router DHCP pool also contains `.51`, a lease conflict occurs when another device gets `.51` from DHCP. Additionally, if the static IP is set only in Windows (not as a DHCP reservation in the router), a router firmware update, factory reset, or ISP modem replacement erases no configuration — but power cycling the server may cause APIPA address assignment if the NIC resets before the IP is re-applied.

**Why it happens:**
Static IPs configured at the OS level survive router resets but are invisible to the router's DHCP conflict detection. The router may still lease the same IP to another device that joins the LAN, causing an IP collision that manifests as intermittent packet loss rather than a clean failure.

**How to avoid:**
Two-layer static IP strategy:
1. Set a DHCP reservation in the router for Server `.23` MAC address `30-56-0F-05-xx-xx` (check and record actual MAC). This prevents the router from leasing `.23` to anyone else.
2. Also set the NIC to a static IP on the server. Both layers together mean: even if the router resets and loses the reservation, the server holds `.23`; even if Windows networking glitches, the reservation ensures the router won't give `.23` away.
3. Document the server MAC address. After any router firmware update, re-enter the reservation first thing.
4. Place the static IPs (.23, .27, etc.) outside the DHCP pool range. If the router's pool is `.100-200`, static assignments at `.23` and `.27` are never in the pool and cannot conflict.

**Warning signs:**
`arp -a` on any pod shows two entries for `.23` or `.51`. Staff kiosk loads intermittently. `ping 192.168.31.23` shows request timeouts mixed with replies. Router DHCP lease table shows `.51` still assigned to another device.

**Phase to address:** Phase 1 (diagnosis) must verify the server's current IP situation. Phase 3 (static IP + DNS) must implement both layers simultaneously.

---

### Pitfall 5: Windows DNS Cache Survives Hosts File Changes

**What goes wrong:**
After updating `C:\Windows\System32\drivers\etc\hosts` on a pod to point `kiosk.rp` at `192.168.31.23`, the change may not take effect immediately. The Windows DNS Client service (dnscache) caches negative and positive lookups. Background processes (Edge, Windows Update, telemetry) continuously make DNS queries that re-populate the cache. `ipconfig /flushdns` on one pod flushes the cache but another pod may have cached a stale entry that won't expire for the default TTL. Edge also maintains its own internal DNS cache that is separate from the OS cache and is not cleared by `ipconfig /flushdns` — it requires `edge://net-internals/#dns` or a browser restart.

**Why it happens:**
Windows dnscache service has its own TTL (default positive: 86400s, negative: 300s). Adding a hosts file entry does not flush the existing cache — the service reads the hosts file at query time but may serve the cached response instead. Edge's internal DNS resolver is Chromium's `//net` stack, which has its own independent cache.

**How to avoid:**
- After updating the hosts file on a pod, run `ipconfig /flushdns` AND restart Edge (kill all msedge.exe processes). The flush + restart combination clears both caches.
- For deployment automation via pod-agent: chain the hosts file write, `ipconfig /flushdns`, and `taskkill /F /IM msedge.exe` into one command sequence.
- Verify resolution from within the pod using `nslookup kiosk.rp 127.0.0.1` — not just `ping`, which may use a cached route.

**Warning signs:**
`nslookup kiosk.rp` returns correct IP but typing `kiosk.rp` in Edge still shows "site cannot be reached." Hosts file edit was deployed but the issue reappears on the next Edge launch. Different pods resolve the same hostname to different IPs.

**Phase to address:** Phase 3 (DNS) deployment scripts must include flush + browser restart. Do not mark "DNS deployed" as done until verified from Edge, not just from command line.

---

### Pitfall 6: Port 8080 and 3000 Have Silent Competitors on Windows

**What goes wrong:**
rc-core runs on port 8080. Next.js dev server runs on port 3000. Both are "common" ports with known squatters on Windows:
- Port 8080: Hyper-V Management Service uses it when Hyper-V is installed. Jenkins, Apache Tomcat, and various Windows SDK tools claim 8080. On Windows Server, IIS Application Pool defaults to 8080 as a secondary binding.
- Port 3000: Some Bluetooth stack implementations claim 3000. iTunes/Bonjour uses port 3689 but some versions used 3000 during transition.
- Port 80: HTTP.sys (`Microsoft-HTTPAPI/2.0`) binds port 80 by default on Windows when IIS or WinRM is installed, even without explicit configuration. This matters if the plan is to serve the kiosk on port 80 without a reverse proxy.

On gaming PCs, ports 9996 (AC), 20777 (F1), 5300 (Forza), 6789 (iRacing) are already reserved by rc-agent for game telemetry — any of these would conflict with UDP listeners from actual running games if rc-agent starts before the game initializes the UDP socket.

**Why it happens:**
Windows does not warn when a new service starts and finds a port already bound. The new service either silently fails to bind (returning no error if using `SO_REUSEADDR` incorrectly), or the old service is pre-empted depending on start order. The issue only surfaces when both services are running simultaneously.

**How to avoid:**
- Verify port availability on the server before choosing ports: `netstat -ano | findstr :8080` and `netstat -ano | findstr :3000`.
- For the Next.js production build on the server, use port 3200 (already used by `racingpoint-admin`) or an uncontested port like 3100 or 3050. Avoid port 3000 in production.
- Disable HTTP.sys on gaming pods if any future kiosk page is served on port 80: `netsh http delete urlacl url=http://+:80/` or disable `W3SVC` if IIS is installed.
- For rc-core port 8080, verify with `netstat -ano | findstr :8080 | findstr LISTEN` on the server — if anything other than rc-core is listed, investigate before assuming rc-core bound successfully.

**Warning signs:**
rc-core or Next.js starts without error message but returns connection refused. `netstat -ano` shows the expected port is LISTENING under a different PID than rc-core or node.exe. Server startup logs show "address already in use" (Rust: `AddrInUse` error code) — but only if error handling is not silently swallowed.

**Phase to address:** Phase 1 (diagnosis) must run `netstat -ano` on the server and record what currently holds each relevant port. Phase 2 (staff kiosk pinning) must verify port selection before deploying.

---

### Pitfall 7: Next.js Production Build Requires Explicit Build Step Before Serving

**What goes wrong:**
Running `next start` on the server without a prior `next build` serves either stale content from a previous build or fails entirely with "Could not find a production build." If the server runs `next dev` instead of `next build && next start`, the kiosk works but is 3-5x slower, recompiles on each request, and crashes if the CPU is under load. The distinction between dev and prod mode is easy to miss when setting up auto-start for the first time.

**Why it happens:**
Next.js dev mode (`next dev`) and prod mode (`next start`) use the same port but behave completely differently. A startup script that calls `npm start` from the wrong directory or without the correct package.json `start` script will launch dev mode transparently.

**How to avoid:**
- The auto-start script must call `next build` first (once), then `next start`. The HKLM Run or Scheduled Task must point to a script that runs `next start` only (build is a one-time step performed during deploy, not startup).
- Verify production mode is active: `next start` output includes "ready - started server on 0.0.0.0:PORT" without webpack compilation messages. Dev mode prints "event compiled client and server files."
- Use `cross-env NODE_ENV=production next start` to make the environment explicit.

**Warning signs:**
The kiosk URL works but is slow on first load. Server CPU spikes to 100% when a new page is opened. `next start` output shows webpack compilation lines. Port 3000 is bound but the kiosk shows an error about missing `.next` directory.

**Phase to address:** Phase 2 (staff kiosk pinning) deploy script must include explicit `npm run build` before configuring auto-start.

---

### Pitfall 8: NSSM Is Abandoned — Use Task Scheduler or SC Instead

**What goes wrong:**
NSSM (Non-Sucking Service Manager) is the commonly cited solution for running Node.js/Next.js as a Windows service. It is unmaintained (last release 2017), flagged by Windows Defender and other AV as "potentially unwanted software" (because malware extensively uses it), and leaves undescribed events in the Windows Event Log because the event manifest is never registered. On Windows 11 22H2+, NSSM-created services have caused intermittent startup failures that only manifest after Windows Feature Updates.

**Why it happens:**
NSSM was the de-facto standard 2012-2018. Most tutorials and Stack Overflow answers still recommend it because they were written during that window. The project stagnation is not obvious until you check the GitHub commit date.

**How to avoid:**
Two preferred alternatives:
1. **Task Scheduler with "At logon" trigger** (for Session 1 GUI processes like the kiosk staff terminal and Edge kiosk): Use `schtasks /create` with trigger `ONLOGON` and the "Run only when user is logged on" security option. This is the same approach used for rc-agent via HKLM Run.
2. **`sc.exe` with a wrapper .bat** (for background non-GUI services like rc-core on the server): `sc create rccore binPath="C:\RacingPoint\rc-core.exe" start=auto` with `sc failure rccore actions= restart/5000/restart/30000/restart/60000`. Native Windows services with native recovery actions. No third-party dependency.
3. **PM2 with pm2-windows-service** is acceptable for Next.js on the server but requires setting `PM2_HOME` as a system environment variable (not user-level) and using `pm2-windows-service` rather than `pm2 startup`. The npm-start script in `package.json` must call `next start`, not `next dev`.

**Warning signs:**
Defender quarantines or flags `nssm.exe`. Service fails to start after a Windows Feature Update. Event Viewer shows `Event ID 0: The description for Event ID 0 from source NSSM cannot be found` repeatedly.

**Phase to address:** Phase 2 (staff kiosk pinning) must choose sc.exe or Task Scheduler for rc-core/Next.js auto-start. Do not introduce NSSM.

---

### Pitfall 9: Edge `StartupBoost` Launches a Background Edge Instance Before the Kiosk Launch

**What goes wrong:**
Edge's `StartupBoostEnabled` policy (enabled by default in managed and unmanaged installs since Edge 88) launches an Edge process in the background at Windows startup. This background process holds the default Edge profile open. When rc-agent then launches Edge with `--kiosk 127.0.0.1:18923 --user-data-dir=C:\RacingPoint\EdgeKiosk`, if `--user-data-dir` is not set, the new instance tries to reuse the already-open profile and either: (a) spawns a second window that is not in kiosk mode (the existing lock screen bug), or (b) the startup-boost instance prevents `--kiosk` from applying correctly, showing normal Edge UI. Microsoft explicitly lists `StartupBoostEnabled` as a feature that does not work with kiosk mode and must be disabled.

**Why it happens:**
StartupBoost pre-loads the Edge browser process before any user intent. It does not respect command-line flags passed to subsequent Edge invocations because it was launched without those flags. This conflicts with `--kiosk` which requires Edge to start fresh with those flags applied.

**How to avoid:**
Disable `StartupBoostEnabled` on all pods via Group Policy: `Computer Configuration → Administrative Templates → Microsoft Edge → StartupBoostEnabled = Disabled`. Or via registry: `HKLM\SOFTWARE\Policies\Microsoft\Edge\StartupBoostEnabled = 0 (DWORD)`.
Also disable `BackgroundModeEnabled` for the same reason — another policy Microsoft lists as incompatible with kiosk mode.

**Warning signs:**
Task Manager shows `msedge.exe` processes running before any user opens a browser. The Edge kiosk window sometimes shows the address bar (normal mode) instead of full-screen kiosk. Edge kiosk stacking (multiple windows) that the close_browser() fix addressed in v1.0 recurs after an Edge update.

**Phase to address:** Phase 2 (pod lock screen fix) must disable `StartupBoostEnabled` and `BackgroundModeEnabled` on all pods before configuring Edge kiosk launch.

---

### Pitfall 10: `127.0.0.1` Only Works If rc-agent Is Already Listening When Edge Opens

**What goes wrong:**
rc-agent serves the lock screen on `127.0.0.1:18923`. If Edge is launched before rc-agent's HTTP server has finished binding the port, Edge shows "ERR_CONNECTION_REFUSED" and does not retry. In kiosk mode there is no address bar and no refresh mechanism — the customer sees the error page permanently until the tab is manually reloaded (impossible in kiosk mode) or the session restarts. This is distinct from the HKLM Run startup issue — it can also happen when rc-agent restarts mid-session and Edge was already open.

**Why it happens:**
Edge in kiosk mode loads the URL once at launch. Unlike a normal browser where the user can press F5, kiosk mode suppresses the refresh gesture. `ERR_CONNECTION_REFUSED` in kiosk mode is a dead end with no recovery path visible to the customer.

**How to avoid:**
The v1.0 "TCP readiness" fix (rc-agent waits for HTTP server before signaling Edge to launch) addresses the startup case — verify it is actually active and not regressed. For the crash-restart case, the recovery flow must: (1) kill the Edge kiosk instance, (2) wait for rc-agent HTTP server to respond (poll `127.0.0.1:18923/health`), (3) relaunch Edge. A direct Edge `--kiosk` relaunch without waiting for the server will reproduce the error.

**Warning signs:**
Edge shows `ERR_CONNECTION_REFUSED` on the lock screen. Pod WebSocket reconnects (rc-agent is alive) but the pod screen shows the browser error page. This typically happens 3-8 seconds after rc-agent restarts (before the HTTP server finishes binding).

**Phase to address:** Phase 2 (pod lock screen fix) — verify the TCP readiness check is in the relaunch path, not just the startup path.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Use `next dev` for the kiosk server | No build step, instant changes | 3-5x slower, crashes under CPU load, not production-safe | Never — dev mode in production is always wrong |
| Use NSSM for Windows service management | One command setup | AV flags binary, maintenance nightmares after Win11 updates, abandoned project | Never — use sc.exe or Task Scheduler |
| Set static IP without DHCP reservation | Server holds its IP immediately | Router reset causes IP conflict with another device | Only if you can guarantee the router will never reset |
| Use `.local` TLD for local hostname | Familiar pattern, "just works" on Mac | mDNS conflicts on Windows, Bonjour fights, resolution flips | Never — use `.rp` or another safe TLD |
| Rely on `ipconfig /flushdns` alone after hosts file change | One command, seems sufficient | Edge internal DNS cache is separate and not flushed | Never sufficient — must also restart Edge |
| Skip `--user-data-dir` on Edge kiosk launch | Simpler launch command | Startup Boost conflict, profile corruption on update, multiple Edge instances fighting | Never — always isolate the kiosk profile |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Edge kiosk mode | Starting Edge without killing existing msedge.exe first | `close_browser()` must kill both `msedge.exe` AND `msedgewebview2.exe` before relaunching kiosk (already fixed in 80ec001, verify it stays) |
| Windows hosts file | Writing hosts file entries in wrong format or with Windows CRLF | Use `192.168.31.23 kiosk.rp` with LF line endings; extra whitespace or CRLF causes silent resolution failure on some Windows versions |
| PM2 on Windows | Setting `PM2_HOME` as user environment variable only | Must be set as SYSTEM environment variable; user-level PM2_HOME is not visible to the service user account |
| Next.js `next start` | Running from wrong working directory | Must run from the directory containing `.next/` folder; use `--cwd` or set working directory explicitly in Task Scheduler or sc.exe |
| Windows Firewall | Forgetting new ports after static IP change | Adding rc-core or Next.js on a new port requires a new inbound firewall rule; `netsh advfirewall firewall add rule name="RaceControl" dir=in action=allow protocol=TCP localport=<PORT>` |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Next.js dev mode in production | Page loads take 3-5 seconds, CPU spikes on navigation | Enforce `next build && next start` in all deployment scripts | Immediately — from first customer use |
| DNS query for every lock screen load | Sub-second latency added to every lock screen render | Use IP address or hosts file; avoid making the kiosk URL depend on upstream DNS resolution | Any time LAN DNS latency exceeds 50ms |
| Edge kiosk profile on spinning disk | Lock screen takes 4-8 seconds to show | Point `--user-data-dir` to an SSD path (C: drive on pods is typically SSD; verify) | When pods have spinning disk OS drive |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Leaving Edge kiosk address bar accessible via `--kiosk` but wrong type | Customer can navigate away from lock screen, escape kiosk | Use `--edge-kiosk-type=fullscreen` which disables the address bar entirely; verify F11 and Ctrl+N are blocked |
| Hosts file writable by non-admin users | Any process running as the customer user can redirect `kiosk.rp` to any IP | Hosts file should be owned by SYSTEM with read-only permissions for non-admin users; verify `icacls C:\Windows\System32\drivers\etc\hosts` on pods |
| pm2-windows-service running as SYSTEM with full Node.js access | Compromise of Next.js app = full system access | Run the service under a restricted local user account, not SYSTEM, for Next.js on the server |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| `ERR_CONNECTION_REFUSED` in kiosk mode | Customer stuck on error page with no recovery path | Kiosk URL must point to a page that exists independently of rc-agent status — a static fallback HTML served by a separate lightweight server, or use a retry page |
| Staff kiosk URL changes when server IP drifts | Staff bookmarks stop working; IT support calls | Pin server to static IP AND provide hostname (`kiosk.rp`) so staff always have a stable URL regardless of IP changes |
| No visual indicator when kiosk service is starting up | Staff see "site cannot be reached" and think system is broken | Add a splash screen or status page that is served immediately at startup (even before Next.js hydrates) |

---

## "Looks Done But Isn't" Checklist

- [ ] **Static IP on server:** Verify with `ipconfig /all` on the server that the IP is marked as static (not DHCP), AND verify the DHCP reservation is in the router admin panel.
- [ ] **Hosts file deployed:** After deploying hosts file to a pod, verify from Edge (not just `nslookup`) by navigating to `kiosk.rp` in a non-kiosk Edge window on that pod.
- [ ] **Edge version pinned:** Run `msedge --version` via pod-agent on all 8 pods and confirm they are all on the same known-good version. Do not assume updates are blocked without verifying `EdgeUpdate` service is disabled.
- [ ] **Kiosk launch script uses `--user-data-dir`:** Check the rc-agent kiosk launch command in source code — if `--user-data-dir` is absent, the kiosk is vulnerable to startup boost conflicts.
- [ ] **StartupBoost disabled:** Verify via `reg query HKLM\SOFTWARE\Policies\Microsoft\Edge /v StartupBoostEnabled` on a pod. If the key is absent, StartupBoost is enabled by default.
- [ ] **Next.js running in production mode:** SSH/exec to server and check `next start` output or `NODE_ENV` — look for webpack compilation messages that indicate dev mode.
- [ ] **Port conflicts checked:** On the server, run `netstat -ano | findstr LISTEN` before deploying and confirm rc-core (port 8080) and Next.js kiosk are the only processes on their respective ports.
- [ ] **TCP readiness in relaunch path:** rc-agent restart test — kill rc-agent, wait for it to restart, observe whether Edge shows ERR_CONNECTION_REFUSED or correctly waits. The readiness check must cover the restart path, not just cold boot.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Session 0 blind restart | LOW | Wait for next customer login (HKLM Run fires in Session 1); no manual action needed if HKLM Run is installed correctly |
| Edge white screen after update | MEDIUM | Roll back Edge via pod-agent: kill edgeupdate, download prior MSI, install silently; disable EdgeUpdate service to prevent recurrence |
| DNS resolution failure (.local conflict) | LOW | Switch to hosts file entries for `kiosk.rp`; deploy via pod-agent to all pods in sequence |
| DHCP drift (IP collision) | MEDIUM | Set server NIC to static IP; update router DHCP reservation; `arp -d *` on affected pods to flush stale ARP cache |
| Port conflict on 8080 or 3000 | MEDIUM | `netstat -ano` to identify competing process; disable or relocate competing service; change rc-core or Next.js port if needed and update all config references |
| NSSM flagged by AV | MEDIUM | Stop and delete the NSSM service; reinstall using sc.exe or Task Scheduler; may need to temporarily disable Defender real-time scanning to complete migration |
| ERR_CONNECTION_REFUSED in kiosk | LOW | Via pod-agent: kill msedge, verify rc-agent port 18923 is up, relaunch Edge; add HTTP readiness poll to relaunch script to prevent recurrence |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Session 0 GUI blindness | Phase 1 (diagnose) + Phase 2 (auto-start) | After reboot, verify kiosk screen is visible on physical monitor, not just `tasklist` showing process running |
| Edge auto-update breaking kiosk | Phase 2 (pod lock screen fix) | `reg query` confirms EdgeUpdate disabled; run `msedge --version` after simulated update block |
| `.local` TLD mDNS conflicts | Phase 3 (local DNS) | Test from all 8 pods using `nslookup kiosk.rp` and Edge navigation; confirm no Bonjour service installed |
| DHCP drift | Phase 1 (diagnose) + Phase 3 (static IP) | `ping 192.168.31.23` from all pods returns consistent <1ms; `ipconfig /all` on server shows static assignment |
| DNS cache staleness | Phase 3 (DNS deploy scripts) | After hosts file deploy, test from Edge (not just nslookup); include flush + Edge restart in deploy sequence |
| Port 8080/3000 conflicts | Phase 1 (diagnose) | Document current port occupancy; `netstat -ano` baseline on server before any changes |
| Next.js dev vs prod | Phase 2 (staff kiosk) | `next start` output has no webpack lines; page load < 500ms for cached routes |
| NSSM dependency | Phase 2 (auto-start) | `sc query` shows service type as `WIN32_OWN_PROCESS` without NSSM wrapper; no NSSM binary on server |
| StartupBoost conflict | Phase 2 (pod lock screen) | Registry key present and set to 0; no background msedge.exe processes before any user opens browser |
| ERR_CONNECTION_REFUSED in kiosk | Phase 2 (pod lock screen) | Restart rc-agent, observe Edge kiosk — should show lock screen within 5 seconds, not error page |

---

## Sources

- **MEMORY.md / v1.0 codebase (HIGH):** Session 0 fix history (HKLM Run key), close_browser() edge stacking fix (80ec001), TCP readiness overlay fix — all confirmed production bugs already hit
- **[Microsoft: Configure Edge kiosk mode](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-configure-kiosk-mode) (HIGH):** `StartupBoostEnabled` and `BackgroundModeEnabled` listed as incompatible with kiosk mode; `--user-data-dir` flag; kiosk type options
- **[Edge 128 kiosk white screen bug](https://learn.microsoft.com/en-us/answers/questions/2403205/white-screen-on-kiosk-mode-after-ms-edge-updated-t) (HIGH):** Confirmed production regression; fix in 128.0.2739.54; organizations rolled back to 124
- **[Microsoft: Kiosk mode troubleshooting](https://learn.microsoft.com/en-us/troubleshoot/windows-client/shell-experience/kiosk-mode-issues-troubleshooting) (HIGH):** Sign-in issues, automatic logon, AssignedAccess log channel
- **[Task Scheduler GUI app with "run whether logged on or not"](https://learn.microsoft.com/en-us/archive/msdn-technet-forums/d0ed7784-3475-4218-95c4-477d84233cb3) (HIGH):** Confirmed: GUI apps run in background session, invisible on desktop
- **[NSSM SaltStack deprecation issue](https://github.com/saltstack/salt/issues/59148) (MEDIUM):** Confirms project abandoned, AV flagging, Windows 11 compatibility issues
- **[mDNS .local conflicts on Windows](https://community.start9.com/t/solved-mdns-on-windows-11-partially-works/1859) (MEDIUM):** Windows 11 native mDNS vs Bonjour conflict confirmed
- **[Windows DNS resolver .local behavior change 1803→1809](https://social.technet.microsoft.com/Forums/en-US/966ba488-6f79-412f-9873-21155ff635e6/resolving-domain-local-changed-behavoir-from-windows-10-1803-to-windows-10-1809) (MEDIUM):** Confirmed behavior change in how Windows prioritizes mDNS over DNS for .local
- **[HTTP.sys / Microsoft-HTTPAPI/2.0 port 80 occupancy](https://learn.microsoft.com/en-us/archive/msdn-technet-forums/bcc1f713-1fc9-42c9-8b9e-0a172d34c1c6) (HIGH):** Confirmed default Windows behavior
- **[PM2 Windows service PM2_HOME requirement](https://blog.cloudboost.io/nodejs-pm2-startup-on-windows-db0906328d75) (MEDIUM):** Confirmed system-level PM2_HOME requirement; startup type "Automatic delayed" recommendation

---
*Pitfalls research for: RaceControl v2.0 — Kiosk URL Reliability (Windows LAN kiosk context)*
*Researched: 2026-03-13*
