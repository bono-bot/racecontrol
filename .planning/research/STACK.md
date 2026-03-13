# Stack Research

**Domain:** Windows LAN kiosk URL reliability — permanent hostname/service/DNS/fallback stack
**Researched:** 2026-03-13
**Confidence:** HIGH (Windows service management), HIGH (DHCP/IP), MEDIUM (DNS options), HIGH (fallback patterns)

---

> **Note:** This file covers v2.0 Kiosk URL Reliability stack additions.
> The v1.0 Rust/async stack (tokio, axum, tokio-tungstenite) remains unchanged.
> See archived v1.0 research at `.planning/archive/` for the previous full stack doc.

---

## Context

Existing stack that is NOT changing:
- rc-core: Rust/Axum, port 8080
- rc-agent: Rust, per-pod, lock screen on port 18923
- pod-agent: Node.js, port 8090
- kiosk: Next.js (running mode unknown — investigate in Phase 1)
- All machines: Windows 11

New additions needed for v2.0:
1. Windows service manager (auto-start Next.js on server .23)
2. Local DNS / hostname resolution (stable friendly URL for all pods to reach kiosk)
3. IP address stability (prevent DHCP drift on server .23)
4. Lock screen fallback/retry (browser shows branded screen instead of "Site cannot be reached")
5. Health check endpoint (machine-readable liveness for NSSM and the retry loop)

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| NSSM (Non-Sucking Service Manager) | 2.24 | Run Next.js `next start` as a native Windows service with auto-restart on crash | Single standalone EXE, no runtime dependency on Node.js version. Wraps any CLI as a Windows service. Built-in restart logic with configurable `AppRestartDelay` and `AppThrottle`. Writes stdout/stderr to log files. `SERVICE_AUTO_START` survives reboots without a logged-in user. Battle-tested for Node.js/Next.js on Windows; active GitHub forks as of 2025. Simpler than WinSW (no XML config) and more reliable than PM2 on Windows (see What NOT to Use). |
| Windows DHCP Reservation (router at .1) | Xiaomi router firmware | Pin server (.23) and all 8 pods to their current IPs permanently | MAC-to-IP binding at the DHCP server — one-time config at 192.168.31.1 covers all future lease renewals. No per-machine manual config. Centralized change management. Standard best practice for fixed-role LAN devices. The Xiaomi router at .1 supports static IP assignment in Advanced Settings > DHCP. |
| Windows hosts file (per machine) | OS built-in (all Windows versions) | Map `kiosk.rp` → 192.168.31.23 on every pod and on James's machine | Zero dependencies, zero running process. Works offline. Survives DNS failures. Deployed in one shot via pod-agent exec (existing infra). Persistent across reboots. The correct choice for a fixed small LAN where the IP-to-hostname mapping never changes. |
| Next.js health check route (`/api/health`) | Next.js App Router (14+) or Pages Router | Machine-readable liveness endpoint for NSSM and for the lock screen JS retry loop | Standard 10-line pattern. Returns `{"status":"ok","uptime":N}` with HTTP 200 when Next.js is serving. Used by NSSM to confirm the service is actually healthy after restart. Used by the lock screen polling loop to know when rc-agent is ready. |
| JavaScript polling loop (lock screen HTML) | Vanilla JS, no library | Show branded "Connecting..." splash instead of Chrome "Site cannot be reached" when rc-agent starts slowly | rc-agent serves the lock screen as static HTML from port 18923. A `setInterval` fetch-poll to `/health` on port 18923 every 2 seconds replaces the blank browser error with a branded branded branded branded branded branded branded branded screen until rc-agent is ready. No new Rust dependencies — a change to the embedded HTML string. |

### Supporting Tools

| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| Acrylic DNS Proxy | 2.2.1 (Feb 2025) | Local DNS proxy on James's workstation (.27) with wildcard and custom TLD support | Use ONLY on .27 if staff terminal navigation or debugging requires wildcard DNS (e.g., `*.rp` → .23). Not needed on pods — hosts file covers pods. Not needed on server — it is the server. Open source (GPLv3), released Feb 2025, compatible with Windows 11 dnscache. |
| PowerShell `Set-Content` / `Add-Content` | OS built-in | Deploy hosts file entries to pods via pod-agent exec | One-liner run via pod-agent HTTP exec endpoint. Idempotent with a check-before-add pattern. pod-agent runs as SYSTEM so has write access to `C:\Windows\System32\drivers\etc\hosts`. |
| `sc query` / `sc start` | OS built-in | Verify NSSM-managed service state in deploy scripts | Use in the post-deploy verification step to confirm `RaceControlKiosk` service is in `RUNNING` state before declaring deploy complete. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `nssm.exe install` (CLI) | Register and configure Windows service | Place at `C:\Tools\nssm\win64\nssm.exe`. All config via CLI flags — no GUI needed. Run as admin. |
| `nssm.exe dump ServiceName` | Print current service config | Useful for verifying parameters after install without opening Services MMC. |

---

## Installation

```bash
# ---- ON RACING-POINT-SERVER (.23) — one-time setup ----

# 1. Download NSSM 2.24 from https://nssm.cc/download
#    Extract nssm-2.24/win64/nssm.exe to C:\Tools\nssm\win64\nssm.exe

# 2. Build Next.js kiosk for production
cd C:\RacingPoint\kiosk
npm run build

# 3. Install as Windows service
C:\Tools\nssm\win64\nssm.exe install RaceControlKiosk "C:\Program Files\nodejs\node.exe"
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk AppParameters "node_modules\.bin\next start -p 3000"
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk AppDirectory "C:\RacingPoint\kiosk"
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk AppStdout "C:\RacingPoint\logs\kiosk-out.log"
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk AppStderr "C:\RacingPoint\logs\kiosk-err.log"
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk AppRestartDelay 5000
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk AppThrottle 3000
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk Start SERVICE_AUTO_START
C:\Tools\nssm\win64\nssm.exe set RaceControlKiosk ObjectName LocalSystem

# 4. Start immediately
net start RaceControlKiosk

# ---- ON ALL 8 PODS + SERVER — push hosts entry via pod-agent exec ----
# Payload (write to file, pass via -d @file to avoid Git Bash escaping):
# { "cmd": "powershell -Command \"$h='C:\\Windows\\System32\\drivers\\etc\\hosts'; $e='192.168.31.23 kiosk.rp server.rp'; if (!(Get-Content $h | Select-String -Quiet $e)) { Add-Content $h \\\"`n$e\\\"\" }\"" }

# ---- ROUTER — DHCP reservation (one-time via browser at http://192.168.31.1) ----
# Advanced Settings > DHCP > Static IP assignment
# Add MAC→IP mapping for all 10 devices (8 pods + server + James)
# Server MAC: verify via ipconfig /all on .23
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| NSSM | WinSW (Windows Service Wrapper) | WinSW is equally reliable. Use it if you prefer XML declarative service config over CLI flags. Either works on Windows 11. NSSM chosen here because it has more community examples for Node.js/Next.js. |
| NSSM | PM2 + pm2-windows-service | Use PM2 only if you need cluster mode (multi-core load balancing) or a process dashboard. Neither applies here. PM2 Windows service is unreliable (see What NOT to Use). |
| NSSM | Windows Task Scheduler (on-startup trigger) | Use Task Scheduler for GUI processes that need Session 1 (like rc-agent uses HKLM Run key). Next.js `next start` is headless — no session needed. A true Windows service via NSSM is the correct primitive. |
| DHCP reservation (router) | Static IP at machine level (network adapter) | Use machine-level static IP if the router is unreliable, not under your control, or if you need IP to work before the router hands a DHCP lease (e.g., PXE boot). Not applicable here — router is stable and under control. |
| Hosts file (pods) | mDNS / .local auto-discovery | Use mDNS if machines advertise themselves dynamically and IPs change frequently. Not the case here — IPs are pinned by DHCP reservation. Hosts file needs no running process and is simpler. |
| Hosts file (pods) | Acrylic DNS Proxy on every pod | Acrylic on every pod is overkill. Hosts file achieves the same result with no running process and no port conflict risk. Acrylic is justified only on .27 for development-time wildcard flexibility. |
| JS polling loop (lock screen) | Service Worker / Workbox background sync | Service Workers are correct for PWAs with complex offline strategies. The rc-agent lock screen is a Rust-served static HTML page — not a Next.js app, no service worker context. A `setInterval` fetch loop is the right primitive. |
| Next.js `/api/health` | Axum `/health` on rc-core | rc-core already has health infrastructure. The NSSM-supervised process is Next.js, not rc-core. The health endpoint must be served by the same process NSSM is monitoring. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `pm2-windows-service` npm package | Abandoned (last commit 2018). pm2 resurrect on service boot is flaky — `pm2 list` shows empty after reboot until manual `pm2 resurrect` is called. Requires PM2_HOME set at SYSTEM level (easy to misconfigure). Multiple open GitHub issues with no resolution. | NSSM wrapping `next start` directly |
| `node-windows` npm package | Known compatibility failures on Windows 11 and Windows Server 2022/2025. Couples service config to the Node.js version it was installed with. Not maintained for modern Windows. | NSSM (standalone Win32 binary, no Node version coupling) |
| Docker on Windows (for kiosk, rc-core, or any pod service) | Windows containers require Hyper-V or WSL2. Significant RAM overhead. Breaks direct localhost port assumptions that rc-agent (port 18923) and pod-agent (port 8090) rely on. Container networking adds a layer between host and guest that complicates port binding. No benefit over native services in a controlled single-tenant LAN. Gaming pods cannot afford the overhead. | Native NSSM services |
| `.local` TLD for custom LAN names | Microsoft docs explicitly recommend against `.local` for custom DNS names. `.local` is reserved for mDNS/Bonjour and causes resolution conflicts on some network configurations. | `.rp` TLD in hosts file (custom, short, unambiguous) |
| External/cloud DNS for internal service names | Venue LAN must work when WAN is down. Any internal URL that requires outbound DNS resolution is a single point of failure. | Hosts file (offline-first) |
| `NSSM AppThrottle` at default (1500ms) for Next.js | Next.js cold start takes 2–5 seconds on first boot. At 1500ms throttle, NSSM treats repeated "fast exits" as a crash loop and increases delays. Set `AppThrottle 3000` to give Next.js enough time to initialize before NSSM classifies it as flapping. | `nssm set RaceControlKiosk AppThrottle 3000` |
| Hardcoded IP in kiosk UI or browser bookmarks | DHCP drift has already caused "server moved from .51 to .23" issues in this venue. Any URL with a raw IP is one router-reset away from breaking. | Hostname `kiosk.rp` resolved via hosts file |

---

## Stack Patterns by Variant

**For the kiosk server (.23) — Next.js production service:**
- NSSM wraps `next start`, `SERVICE_AUTO_START`, `ObjectName LocalSystem`
- `AppRestartDelay 5000` — wait 5s between crash restarts (Next.js needs time to bind port 3000)
- `AppThrottle 3000` — override default 1500ms to prevent false flap detection on slow starts
- Log to `C:\RacingPoint\logs\kiosk-*.log` for debugging
- Verify: `sc query RaceControlKiosk` must return `STATE: 4 RUNNING`

**For all 8 pods — lock screen fallback when rc-agent hasn't started yet:**
- Add to the HTML embedded in rc-agent's lock screen server: a JS polling loop
- Poll `http://localhost:18923/health` (or the lock screen root `/`) every 2 seconds
- Show branded "Racing Point — Loading..." splash during the poll window
- On 200 response, clear the interval and reveal the full lock screen UI (or `location.reload()`)
- This eliminates the Chrome "ERR_CONNECTION_REFUSED" error screen on pod boot

**For all 8 pods + James's machine — hostname resolution:**
- Deploy hosts file entry `192.168.31.23 kiosk.rp server.rp` via pod-agent exec
- Check before writing to avoid duplicates (idempotent)
- Pods staff bookmark: `http://kiosk.rp:3000`
- Lock screen WebSocket target in rc-agent.toml: use `ws://server.rp:8080/ws` once hosts entry is in place

**For James's workstation (.27) — optional DNS wildcard support:**
- Install Acrylic DNS Proxy 2.2.1
- Point adapter DNS to 127.0.0.1
- Add `kiosk.rp` → `192.168.31.23` in AcrylicHosts.txt
- Enables future `*.rp` wildcards without touching hosts file per subdomain

---

## NSSM Key Parameters Reference

| Parameter | Registry Key | Description | Recommended Value |
|-----------|-------------|-------------|-------------------|
| AppRestartDelay | `HKLM\...\Parameters\AppRestartDelay` | Milliseconds to wait between restarts | 5000 (5s — give Next.js time to release port) |
| AppThrottle | `HKLM\...\Parameters\AppThrottle` | "Fast exit" threshold in ms (exits faster than this = throttled) | 3000 (Next.js cold start can be 2-5s) |
| Start | Registry value | Service start type | `SERVICE_AUTO_START` |
| ObjectName | Registry value | Account to run service as | `LocalSystem` (has write access to C:\RacingPoint) |
| AppStdout | Registry value | Log file for stdout | `C:\RacingPoint\logs\kiosk-out.log` |
| AppStderr | Registry value | Log file for stderr | `C:\RacingPoint\logs\kiosk-err.log` |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| NSSM 2.24 | Windows 11, any Node.js version | Standalone Win32 binary. No runtime coupling. Works with Next.js 13/14/15. Last stable release. |
| Next.js App Router health route | Next.js 13.4+ | `app/api/health/route.ts` with `export async function GET()`. Pages Router uses `pages/api/health.js` with `res.json(...)`. Use whichever the existing kiosk already uses. |
| Acrylic DNS Proxy 2.2.1 | Windows 11 | Released Feb 12, 2025. Compatible with Windows 11 dnscache integration. Must set primary DNS of the network adapter to 127.0.0.1 on the machine running Acrylic. |
| Hosts file | All Windows versions | No version constraint. Path: `C:\Windows\System32\drivers\etc\hosts`. Requires admin write. |

---

## Integration Points with Existing Stack

### What does NOT change

| Component | Reason |
|-----------|--------|
| rc-core (Rust/Axum, port 8080) | DHCP reservation ensures server is always .23:8080. No code change. |
| rc-agent (Rust, per-pod) | DHCP reservation + hosts file makes rc-agent.toml server address stable. Only change: lock screen HTML gets a JS retry loop added. |
| pod-agent (Node.js, port 8090) | Used as the deployment vehicle for hosts file changes. No code change. |
| Billing, game launch, WS protocol | All unaffected — this milestone is infrastructure-only. |
| Session 1 / HKLM Run key pattern | rc-agent still starts via HKLM Run key (GUI process requirement). Next.js is headless — NSSM Windows service is the correct mechanism. |

### What changes

| Component | Change |
|-----------|--------|
| kiosk (Next.js) | Add `/api/health` route. Run `next build`. Register with NSSM. |
| rc-agent (Rust) | Add JS retry loop to embedded lock screen HTML. Add `/health` HTTP endpoint if not already present. |
| Server (.23) | NSSM installed, `RaceControlKiosk` service registered. |
| Router (.1) | DHCP reservations added for all 10 devices. |
| All pods + James's machine | Hosts file entry `192.168.31.23 kiosk.rp server.rp` added. |

---

## Sources

- NSSM official site: https://nssm.cc/usage — AppRestartDelay, AppThrottle, install commands (HIGH confidence)
- NSSM + Next.js community: https://github.com/vercel/next.js/discussions/25266 — confirmed working pattern (MEDIUM confidence)
- NSSM GitHub: https://github.com/dkxce/NSSM — active fork, Windows 11 verified (MEDIUM confidence)
- PM2 Windows service resurrection bug: https://github.com/jon-hall/pm2-windows-service/issues/27 — confirmed unreliable (HIGH confidence — open issue, multiple affected users)
- Next.js health check pattern: https://hyperping.com/blog/nextjs-health-check-endpoint — `/api/health` implementation (HIGH confidence)
- Acrylic DNS Proxy: https://mayakron.altervista.org/ and https://github.com/racpast/Acrylic — v2.2.1 Feb 2025, Windows 11 compatible (HIGH confidence)
- DHCP reservation best practice 2025: https://inventivehq.com/blog/why-dhcp-is-better-than-static-ip-addresses-even-for-servers (MEDIUM confidence)
- Xiaomi DHCP reservation: https://www.hardreset.info/devices/xiaomi/xiaomi-mi-wifi-router-4c/configure-dhcp/ (MEDIUM confidence — UI may differ slightly by firmware version)
- mDNS on Windows: https://www.w3tutorials.net/blog/standard-mdns-service-on-windows/ — built into dnscache on Win 10 1703+ (MEDIUM confidence)
- Hosts file multi-machine deploy: https://batchpatch.com/deploying-a-standardized-hosts-file-to-multiple-computers-on-a-network (MEDIUM confidence)
- .local TLD warning: https://forum.openwrt.org/t/local-dns-mdns-local-vs-lan-vs-standalone-hostnames/242966 — use .rp instead (MEDIUM confidence)

---

*Stack research for: RaceControl v2.0 Kiosk URL Reliability — Windows LAN*
*Researched: 2026-03-13*
