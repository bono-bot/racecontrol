# Feature Research

**Domain:** On-premises LAN kiosk URL reliability for Windows sim racing venue
**Researched:** 2026-03-13
**Confidence:** HIGH (codebase inspection + Windows service/DNS research)

## Context

This is the v2.0 milestone for an existing system. v1.0 shipped watchdog hardening,
WebSocket resilience, and clean branded screens. The remaining pain: URLs break.

Two URL surfaces need permanent reliability:

1. **Staff kiosk terminal** — Next.js on Server (.23), port 3300. Staff browse to it on
   a tablet or PC. Currently runs `next dev` which means no guaranteed port, no auto-start,
   and no production build. IP drifts (DHCP assigned .23 but documented as .51 in some places).

2. **Pod lock screens** — rc-agent serves HTML on `127.0.0.1:18923`. Edge is launched in
   kiosk mode pointing at that URL. When rc-agent hasn't started yet (boot race) or has
   crashed, Edge shows "Site cannot be reached" — the unbranded browser error, not a
   Racing Point screen.

Three root failure categories:
- **Service startup race:** rc-agent and kiosk server both need to be running before Edge
  tries to connect. Currently no ordering guarantee.
- **IP/port instability:** Server IP drifts via DHCP; `next dev` picks any available port;
  port 8080 for rc-core has no conflict protection.
- **No fallback on failure:** When the HTTP server is down, Edge shows the browser error
  page instead of a branded waiting screen with retry logic.

---

## Feature Landscape

### Table Stakes (System Keeps Breaking Without These)

Features an operator assumes work in any always-on kiosk system. Missing any of these
means the venue opens with broken screens regularly.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Staff kiosk runs as a Windows service (auto-start on boot) | Any production web server runs as a service. `next dev` is a development command — it compiles on every request, is not stable under load, and does not auto-start after reboot. Every kiosk product ships with the server as a background service. | LOW | Use NSSM (stable, proven for Node.js on Windows) or WinSW (actively maintained, XML config). Command: `nssm install RacingPointKiosk node.exe "next start -p 3300"`. Working directory must be `C:\racingpoint\racecontrol\kiosk`. Requires `next build` to be run first. |
| Kiosk server runs a production build, not `next dev` | `next dev` recompiles on every request and picks whatever port is free. `next start` serves a pre-built static bundle from a fixed port every time. Every production kiosk uses `next start`. | LOW | Run `npm run build` in `kiosk/` directory once. Then service uses `npm run start` (which maps to `next start -p 3300`). Rebuild only needed when code changes are deployed. |
| Server gets a stable IP (no DHCP drift) | Pod configs bake in the server IP as `ws://192.168.31.23:8080/ws/agent`. If the server drifts to `.51` or another IP, all 8 pods disconnect from rc-core simultaneously. DHCP drift is documented as a known issue. Router already has all pod MACs in the network map. | LOW | Add DHCP reservation for server MAC in the Xiaomi router admin (same process used for pods). Router is at 192.168.31.1. Server MAC is on the `.23` machine. Alternatively assign a static IP directly in Windows adapter settings. DHCP reservation is preferred — single point of truth at the router. |
| Pod rc-agent starts in Session 1 before Edge launches the lock screen | Edge is launched as part of the HKLM Run key sequence. If Edge starts before port 18923 is bound, it shows "Site cannot be reached". The existing Session 0 fix (HKLM Run key for `start-rcagent.bat`) does not guarantee rc-agent is ready before Edge starts. | MEDIUM | rc-agent must start first and bind port 18923 before the Edge process is spawned. Options: (a) rc-agent's `start-rcagent.bat` starts rc-agent then waits until port 18923 is connectable before launching Edge — a simple TCP connect loop with 200ms retries up to 30s. (b) Keep rc-agent startup separate and have Edge's launch wrapper do the TCP readiness check. Option (a) keeps the logic in one place. |
| Pod lock screen shows a branded waiting page when rc-agent is not ready | When the HTTP server on port 18923 is down, Edge shows an unbranded "Hmm, can't reach this page" error. This breaks the Racing Point brand and confuses customers. Digital signage and kiosk products always show a branded fallback, never a browser error. | MEDIUM | Serve a local file instead of hitting the network URL on first load. The lock screen HTML can include a `<meta http-equiv="refresh" content="3">` or a JavaScript `setInterval` that polls `127.0.0.1:18923` and navigates there once it responds. The fallback file lives at `C:\RacingPoint\waiting.html` — a static HTML file with Racing Point branding, "Starting up... please wait" message, and auto-retry every 3 seconds. This file is always present even when rc-agent is not running. Edge is launched pointing at `file:///C:/RacingPoint/waiting.html` instead of directly at `127.0.0.1:18923`. |
| Staff kiosk URL is permanent and human-typeable | Staff currently need to know the server IP (192.168.31.23) and port (3300). If the IP changes or the port changes, they cannot reach the kiosk. Any reliable kiosk system uses a stable hostname or URL like `kiosk.rp:3300` or `http://kiosk.rp`. | LOW | Add `192.168.31.23 kiosk.rp` to the Windows hosts file on James's machine and any staff device that accesses the kiosk. The hosts file is checked before DNS — resolution is instant and works offline. No DNS server required. The `.rp` TLD is not a real TLD, so no DNS collision. Alternative: use `kiosk.local` but `.local` triggers mDNS on Windows 11 which adds latency. Plain `kiosk.rp` resolves via hosts file only — fast and reliable. |
| rc-core stays on port 8080 reliably (port conflict protection) | Port 8080 is well-known and other software occasionally claims it. If rc-core fails to bind 8080 at startup, it exits silently and the kiosk and all pods lose connectivity. rc-core should fail fast with a clear logged error when the port is taken, not silently die. | LOW | Already handled by Axum: bind failure returns an error. Verify the error propagates to a log line at ERROR level and rc-core exits with a non-zero code so NSSM (if used for rc-core too) can detect and alert. The real fix is ensuring nothing else runs on 8080 — document 8080 and 3300 as reserved ports in the ops playbook. |
| Deployment of kiosk code change triggers a service restart | When staff deploy a new kiosk build (updated Next.js code), the NSSM service must be restarted to pick up the new build. If the deploy script just copies files without restarting the service, the old build keeps serving. | LOW | Extend the existing pod-agent deploy pattern: after copying new kiosk build files to server, call `nssm restart RacingPointKiosk` via a remote exec on the server. Document the sequence: build → copy → restart service → verify HTTP 200. |

### Differentiators (Resilience Features Worth Having)

Features that make URL reliability airtight rather than just adequate. These add depth
but are not blockers if skipped for the initial implementation.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| rc-core also runs as a Windows service | rc-core on the server is currently started manually or via a script. If the server reboots (power cut, Windows Update), rc-core does not come back up automatically. Running it as a NSSM service means the whole stack auto-recovers after a reboot. | LOW | `nssm install RaceControlCore rc-core.exe` with working directory `C:\racingpoint\racecontrol`. WatchdogConfig.enabled already handles crash restarts; NSSM handles boot-time start. Two separate concerns. |
| Waiting HTML page shows connection attempt count / elapsed time | The basic `file:///waiting.html` with meta-refresh is functional but gives no feedback. Showing "Connecting to RaceControl... attempt 3" reassures staff that something is happening, not that the system is broken. | LOW | Pure HTML/JS in the static `waiting.html` file — no build step, no dependencies. A `setInterval` that increments a counter and updates the DOM is sufficient. |
| rc-agent startup script verifies port is bound before returning | The TCP readiness check in `start-rcagent.bat` is a bat file loop: try TCP connect to 127.0.0.1:18923, sleep 200ms, retry up to 150 times (30s total). PowerShell's `Test-NetConnection` or `New-Object System.Net.Sockets.TcpClient` works for the TCP check. On success, launch Edge. On timeout (30s), launch Edge anyway — it will fall through to the waiting HTML. | MEDIUM | The bat file already uses PowerShell for the HKLM Run setup pattern. This is an extension of the existing `start-rcagent.bat`. Verify: launch Edge with `--kiosk file:///C:/RacingPoint/waiting.html --kiosk-printing --no-first-run --disable-features=TranslateUI` then the waiting page auto-navigates to 127.0.0.1:18923 once the server is up. |
| Hosts file entries pushed to all pods via pod-agent | If staff devices need to access `kiosk.rp`, each needs the hosts file entry. A pod-agent exec command can append the entry if it's not already present (`findstr /C:"kiosk.rp" %WINDIR%\System32\drivers\etc\hosts || echo 192.168.31.23 kiosk.rp >> %WINDIR%\System32\drivers\etc\hosts`). | LOW | Useful for standardizing all 8 pod configs but low priority — pods don't browse to the staff kiosk URL. Mainly relevant for James's machine and the reception tablet. |
| Staff kiosk terminal shows "Backend offline" instead of a connection error | The kiosk's `useKioskSocket` already has a 15s disconnect debounce that shows "Connecting..." in the UI. If rc-core is completely down (e.g., service crashed), the WebSocket never connects and the UI keeps showing "Connecting...". A more informative state would be "Backend offline — rc-core is not responding" after 30s with a retry button. | MEDIUM | Requires a small change to `useKioskSocket.ts`: if the WS connection attempt fails more than N times within 60s, set a `backendDown` state that renders a different UI. The 15s debounce is already in place; this extends it with a terminal state. |
| NSSM configured with stdout/stderr log rotation | NSSM can redirect stdout/stderr to files with size-based rotation. Without this, the kiosk service logs grow unbounded on the server. | LOW | `nssm set RacingPointKiosk AppStdout C:\racingpoint\logs\kiosk-out.log`, `nssm set RacingPointKiosk AppRotateFiles 1`, `nssm set RacingPointKiosk AppRotateBytes 10485760` (10MB). Same pattern for rc-core. |

### Anti-Features (Commonly Suggested, Wrong for This Setup)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Set Windows-assigned static IP on server NIC | "Static IP is more reliable than DHCP" — technically true for the wrong reasons | Setting a static IP in Windows adapter settings bypasses the DHCP server and can cause IP conflicts if someone later adds the same IP via DHCP reservation. On a venue LAN where 13 cameras, 8 pods, and several PCs are all DHCP-managed from the router, mixing static-NIC and DHCP-reservation creates a maintenance split where two people need to check two places. The router is the single source of truth for IPs. | DHCP reservation at the router for the server MAC. One place to manage, zero conflict risk, same stability result. |
| Run a full local DNS server (dnsmasq, Pi-hole, Windows DNS) | "Proper hostnames without hosts file editing" | A DNS server is a new network infrastructure dependency. On a venue LAN where the internet connection is already unreliable, adding a critical DNS server means every device's name resolution goes down if that machine has a problem. The hosts file approach requires zero infrastructure and works fully offline. DNS server is appropriate at 50+ devices; at 8 pods + 1 server + 1 tablet, it's overkill. | Hosts file entries on the 2-3 machines that need `kiosk.rp` — James's machine and the reception tablet. Total effort: 5 minutes once. |
| Use mDNS `.local` hostname (e.g. `kiosk.local`) | "Standard zero-config hostname discovery" | On Windows 11, `.local` domains trigger the mDNS resolver which adds latency and is unreliable when multicast is blocked or filtered on the router. The venue router is a Xiaomi consumer device whose mDNS relay behavior is unknown. A hostname that sometimes works is worse than one that always works. The `.local` TLD has known reliability issues on Windows as documented in multiple Microsoft support threads. | Use a custom TLD like `.rp` in the hosts file. No mDNS involved, instant resolution, completely under our control. |
| Run the kiosk as a UWP app or Progressive Web App installed to the OS | "PWAs have offline support and can be installed to the taskbar for reliable launching" | The kiosk is already a functioning Next.js web app. Converting it to a UWP or PWA shell is a significant architecture change with no reliability benefit for this failure class. The failures are service startup and port availability, not browser cache issues. | Fix the underlying service startup and port binding. The web app stays a web app served by Next.js. |
| Serve the lock screen from rc-core (server) instead of rc-agent (local) | "Single server for all HTML eliminates the rc-agent port as a failure point" | The lock screen on each pod is served by rc-agent on `127.0.0.1:18923` for a good reason: it works during LAN outages and requires no network round-trip for customer PIN entry. Moving it to the server would break the offline-first requirement and add network latency to every lock screen interaction. | Fix rc-agent's startup race so it binds port 18923 before Edge launches. The architecture is correct; the startup ordering is not. |
| Implement HTTP → HTTPS redirect for the kiosk | "HTTPS is best practice" | The kiosk runs on a closed, private LAN with no internet exposure. Adding TLS requires managing certificates (which expire), configuring HTTPS in Next.js, and updating all 8 pod configs to use `wss://` instead of `ws://`. This is significant complexity for zero security benefit on a closed LAN. Browser mixed-content warnings are also triggered if HTTPS pages make HTTP API calls, requiring a full audit. | HTTP on a private LAN is standard practice for internal venue management systems. This is not a problem to solve. |
| Auto-detect the server IP at rc-agent startup (no hardcoded IPs) | "Dynamic discovery means no config changes when IPs change" | Service discovery adds complexity (mDNS, beacon broadcasts, or a registry lookup). The current design (server IP baked into rc-agent.toml) is simple and explicit. The correct fix for IP drift is to stop the IP from drifting via a DHCP reservation — not to add discovery machinery. | DHCP reservation at the router. One config change, permanent fix. |

---

## Feature Dependencies

```
[Server stable IP (DHCP reservation)]
    └──required by──> [Staff kiosk permanent URL (kiosk.rp hosts entry)]
    └──required by──> [Pod rc-agent configs (server_url stays valid)]

[next build (production build)]
    └──required by──> [Kiosk service auto-start (NSSM + next start)]

[Kiosk service (NSSM)]
    └──required by──> [Staff kiosk permanent URL works after reboot]
    └──enhanced by──> [NSSM log rotation]

[rc-agent startup race fix (TCP readiness in start-rcagent.bat)]
    └──requires──> [Waiting HTML page (fallback for Edge)]
    └──eliminates──> ["Site cannot be reached" on pod lock screen]

[Waiting HTML page (file:///C:/RacingPoint/waiting.html)]
    └──required by──> [rc-agent startup race fix]
    └──enhanced by──> [Attempt counter / elapsed time display]

[rc-core as NSSM service]
    └──enhances──> [Kiosk service auto-start (both survive reboots)]
    └──independent of──> [rc-agent startup race fix]
```

### Dependency Notes

- **Stable IP must come before hosts file entry:** The hosts file maps a name to the server IP. If the IP is still drifting when the hosts file entry is added, the name will resolve to the wrong IP after the next drift. Fix the IP first.
- **Production build must come before NSSM service:** NSSM runs `next start` which requires a `.next/` build directory. If `next build` hasn't run, the service will fail to start. Sequence: build → install service → start service → verify.
- **Waiting HTML is independent of rc-agent changes:** The `waiting.html` file is a static asset. It can be written and deployed to pods independently of any rc-agent code changes. It should be deployed before the startup race fix, because the startup race fix relies on it.
- **TCP readiness check depends on waiting HTML being in place:** If Edge launches immediately to `file:///C:/RacingPoint/waiting.html` and the page has auto-retry logic, the startup bat file can launch Edge immediately without waiting. If the waiting HTML is not in place yet, the bat file must do the full 30s poll before launching Edge. Easier to deploy waiting HTML first.
- **NSSM for rc-core is independent:** It improves full-stack reboot recovery but does not affect the pod lock screen failure mode. Can be done in any order relative to other items.

---

## MVP Definition

### Launch With (v2.0 — this milestone)

Minimum set to eliminate all "Site cannot be reached" and URL-change failures.

- [ ] DHCP reservation for server (.23) MAC in router — prevents IP drift, unblocks everything else
- [ ] `next build` production build of kiosk — prerequisite for reliable service
- [ ] NSSM service for kiosk (`RacingPointKiosk`) — kiosk survives reboots, starts automatically
- [ ] Hosts file entry `kiosk.rp` on James's machine + reception tablet — permanent staff URL
- [ ] `waiting.html` static file deployed to all 8 pods — fallback for when rc-agent is not ready
- [ ] `start-rcagent.bat` TCP readiness check — Edge launches to `waiting.html`, auto-navigates to lock screen once rc-agent is up

### Add After Validation (v2.x)

- [ ] rc-core as NSSM service — add after kiosk service is verified working; both survive server reboots
- [ ] NSSM log rotation for kiosk and rc-core — add when log files grow noticeable
- [ ] "Backend offline" terminal state in kiosk UI — add if staff report confusion when rc-core is restarting

### Future Consideration (v3+)

- [ ] Hosts file push to all pods via pod-agent — if pods ever need to browse to `kiosk.rp` (they don't today)
- [ ] Startup attempt counter in `waiting.html` — polish; the meta-refresh is sufficient for v2.0

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| DHCP reservation (server IP stable) | HIGH | LOW | P1 |
| Production build (`next build`) | HIGH | LOW | P1 |
| NSSM kiosk service | HIGH | LOW | P1 |
| Hosts file entry (`kiosk.rp`) | HIGH | LOW | P1 |
| `waiting.html` fallback on pods | HIGH | LOW | P1 |
| TCP readiness check in startup bat | HIGH | MEDIUM | P1 |
| rc-core as NSSM service | MEDIUM | LOW | P2 |
| NSSM log rotation | LOW | LOW | P2 |
| "Backend offline" kiosk UI state | MEDIUM | MEDIUM | P2 |
| Attempt counter in `waiting.html` | LOW | LOW | P3 |
| Hosts file push to pods | LOW | LOW | P3 |

**Priority key:**
- P1: Must have for v2.0 launch — eliminates the documented failure modes
- P2: Should have, add within this milestone if P1s are done cleanly
- P3: Nice to have, future milestone or low-urgency task

---

## Competitor Feature Analysis

This is a bespoke venue management system, not a commercial product. The relevant
comparison class is commercial digital signage and kiosk systems.

| Feature | Commercial kiosk (e.g., Mvix, BrightSign) | DIY LAN kiosk (FullyKiosk + local server) | Our Approach |
|---------|--------------------------------------------|-------------------------------------------|--------------|
| Server auto-start on boot | Service install included in product. Always on. | PM2 or systemd manages Node.js process. | NSSM service for Next.js. Same pattern, different tool. |
| Permanent hostname | Cloud-managed — device registers to cloud DNS. Not applicable on LAN. | Hosts file or local mDNS. `device.local` common pattern. | Hosts file with `.rp` TLD. No mDNS fragility. |
| Fallback when content server unreachable | Built-in offline content cache + branded error page. Retries automatically. | FullyKiosk has "start URL" retry with configurable interval. | Static `waiting.html` with JavaScript auto-retry. Same pattern, inline implementation. |
| Browser startup race | Content players boot in defined order; player waits for network before rendering. | FullyKiosk: configurable startup delay before first URL load. | TCP readiness poll in startup bat before launching Edge. Equivalent outcome. |
| IP stability | Static IP assignment or DHCP reservation managed via cloud portal. | DHCP reservation at router, or static IP in OS. | DHCP reservation at router — consistent with how all other venue devices are managed. |

---

## Sources

- **Codebase inspection (HIGH confidence):** `kiosk/src/hooks/useKioskSocket.ts`, `kiosk/package.json`, `crates/rc-agent/src/lock_screen.rs`, `racecontrol.toml`, `rc-agent.example.toml` — direct inspection
- **PROJECT.md (HIGH confidence):** v2.0 milestone goal, existing constraints, out-of-scope items
- **MEMORY.md (HIGH confidence):** Network map, pod IPs, Session 0 issue, deployment rules, server .23 vs .51 discrepancy
- **NSSM documentation (MEDIUM confidence):** [NSSM usage](https://nssm.cc/usage), [Next.js + NSSM discussion](https://github.com/vercel/next.js/discussions/25266) — NSSM is stable for this use case despite being last released in 2017
- **Windows name resolution (MEDIUM confidence):** [Microsoft TCP/IP host name resolution order](https://support.microsoft.com/en-us/topic/microsoft-tcp-ip-host-name-resolution-order-dae00cc9-7e9c-c0cc-8360-477b99cb978a), [hosts file guide](https://dev.to/techelopment/using-and-editing-the-windows-hosts-file-410f) — hosts file checked before DNS on all Windows versions
- **Windows mDNS `.local` reliability issues (MEDIUM confidence):** [mDNS WSL2 issue](https://github.com/microsoft/WSL/issues/12354), [`.local` Wikipedia](https://en.wikipedia.org/wiki/.local) — `.local` on Windows 11 goes through mDNS resolver, adding latency
- **Edge kiosk mode and offline page (MEDIUM confidence):** [Microsoft Edge kiosk mode docs](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-configure-kiosk-mode), [Windows kiosk offline page Q&A](https://learn.microsoft.com/en-us/answers/questions/793388/windows-kiosk-add-page-to-show-offline-if-not-inte) — `file://` URLs work in kiosk mode; meta-refresh and JavaScript work for retry logic
- **DHCP reservation pattern (HIGH confidence):** [Static IP vs DHCP Reservation](https://www.stephenwagner.com/2019/05/07/static-ip-vs-dhcp-reservation/) — DHCP reservation at router is the right pattern for a managed LAN

---
*Feature research for: RaceControl v2.0 Kiosk URL Reliability*
*Researched: 2026-03-13*
