# Architecture Research

**Domain:** Kiosk URL reliability — permanent service hosting, local DNS, pod lock screen resilience
**Researched:** 2026-03-13
**Confidence:** HIGH — derived from direct codebase inspection of all relevant modules

## Standard Architecture

### System Overview (v2.0 target state)

```
Racing-Point-Server (.23 — static IP)
┌──────────────────────────────────────────────────────────────────────────────┐
│  rc-core (Rust/Axum, port 8080) — unchanged, already supervised by no        │
│  watchdog (deleted Mar 11). Started by staff manually or via HKLM Run key.   │
│                                                                               │
│  NEXT.JS KIOSK SERVER (NEW)                                                   │
│  node .next/standalone/.../server.js  — port 3300                            │
│  Started as: HKLM\...\Run\RCKiosk → start-kiosk.bat                         │
│  Serves: http://192.168.31.23:3300/kiosk  (or http://kiosk.rp/kiosk)        │
│                                                                               │
│  WINDOWS DNS SUFFIX / HOSTS FILE (NEW)                                        │
│  C:\Windows\System32\drivers\etc\hosts on every machine that needs kiosk.rp  │
│  192.168.31.23  kiosk.rp                                                     │
│  192.168.31.23  api.rp                                                       │
└─────────────────────────────────────────────────────────────────────────────-┘
                      │ WebSocket ws://192.168.31.23:8080/ws/agent
                      │ HTTP      http://192.168.31.23:8080/api/v1
                      │ UDP heartbeat (server receives on :9996/20777/etc)
          ────────────┼───────────────────────────────────────
          │           │           │                        │
    Pod 1 (.89)  Pod 2 (.33)  ...                    Pod 8 (.91)
    ┌──────────────────────────────────────────────────────────┐
    │  rc-agent — lock screen HTTP :18923                      │
    │    ├── HKLM Run key (start-rcagent.bat) — Session 1      │
    │    ├── start_server() — binds :18923 FIRST               │
    │    ├── show_disconnected() if WS not yet connected        │
    │    └── retry_loop: poll :18923 before launching Edge      │
    │  pod-agent (Node.js :8090) — remote exec                 │
    └──────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | What Changes in v2.0 |
|-----------|---------------|----------------------|
| `rc-core` (Rust, :8080) | API, WebSocket hub, billing, cloud sync | No code changes — only startup supervision |
| Next.js kiosk server | Staff terminal + customer PIN grid | NEW: pinned to .23, production build, auto-start |
| `rc-agent` lock screen (:18923) | Per-pod auth UI, customer screens | MODIFY: fallback page if WS not ready; retry loop before Edge launch |
| Windows hosts file | LAN name resolution | NEW: kiosk.rp → .23, api.rp → .23 on .27 and .23 |
| HKLM Run key (server) | Auto-start kiosk on server boot | NEW: start-kiosk.bat for Next.js server |
| HKLM Run key (pods) | Auto-start rc-agent Session 1 | Exists. Keep. No change. |
| pod-agent (:8090) | Remote exec for deploy | Unchanged |

## Recommended Project Structure

Changes relative to current codebase:

```
C:\RacingPoint\                      (on Racing-Point-Server .23)
    start-rcagent.bat                EXISTING — rc-core auto-start
    start-kiosk.bat                  NEW — Next.js kiosk auto-start
    racecontrol.toml                 EXISTING — no changes needed

C:\Users\bono\racingpoint\racecontrol\kiosk\
    .next\standalone\               EXISTING — production build already present
    next.config.ts                  EXISTING — output: "standalone", basePath: "/kiosk"

kiosk\src\lib\api.ts                EXISTING — already uses window.location.hostname:8080
                                    → VERIFY: hardcoded "8080" is correct for prod; no change needed
                                    → VERIFY: NEXT_PUBLIC_API_URL env var is unset (hostname detection fires)

C:\Windows\System32\drivers\etc\hosts
    (on .23 server)                 ADD: 192.168.31.23  kiosk.rp api.rp
    (on .27 James workstation)      ADD: 192.168.31.23  kiosk.rp api.rp

crates\rc-agent\src\lock_screen.rs  MODIFY: show_disconnected() on :18923 before WS connects
                                    MODIFY: wait_for_server() in launch_browser() — poll :18923
                                            before spawning Edge (prevents "site cannot be reached")
```

### Structure Rationale

- **Standalone Next.js on .23:** The `next build` output with `output: "standalone"` already exists at `kiosk/.next/standalone/`. It needs only `node server.js` to run. Hosting it on the same machine as rc-core (.23) means kiosk URL and API URL share one machine to keep stable. Staff tap one address regardless of which room they're in.
- **Hosts file over router DNS:** The Asus/TP-Link home routers on 192.168.31.x typically support custom DNS host records, but configuration requires router admin access and survives router firmware updates inconsistently. Windows hosts file is applied via one-time command, is persistent across reboots, and works even if the router is factory-reset. Scope: only .23 and .27 need the name; pods use raw IPs in rc-agent.toml and don't browse to kiosk.rp.
- **HKLM Run key for kiosk auto-start:** This is the pattern already proven for rc-agent on all 8 pods (Session 1 fix). It fires for any user login — both the Racing-Point staff user and the auto-login account. Simpler than a Windows Service and avoids Session 0 GUI problems. The kiosk is a CLI Node.js process (no GUI), so Session 0 vs. 1 does not matter for it.
- **Lock screen HTTP readiness check:** rc-agent currently calls `close_browser()` then immediately spawns Edge pointing at `http://127.0.0.1:18923`. The HTTP server is started via `tokio::spawn` — it binds asynchronously. If Edge races ahead of the tokio task, the first request gets "connection refused" and Edge shows "Site cannot be reached". The fix is a synchronous probe in `launch_browser()`: retry `GET http://127.0.0.1:18923/` up to 10 times with 100ms sleep before spawning Edge.
- **Disconnected state on early startup:** rc-agent's main.rs starts the lock screen server before connecting to rc-core (`start_server()` is called, then `show_config_error()` path). When config is valid, `show_disconnected()` should be called immediately after `start_server()` so the screen shows a branded "Connecting..." message instead of a blank browser error during the 1-3s WebSocket connection window.

## Architectural Patterns

### Pattern 1: HTTP Readiness Probe Before Browser Launch

**What:** `launch_browser()` in `lock_screen.rs` polls `http://127.0.0.1:18923/` with a short timeout before calling `std::process::Command::new(edge_path).spawn()`. If the server is not yet bound, the probe retries with exponential backoff up to a fixed maximum, then launches Edge regardless (avoids infinite hang on server startup failure).

**When to use:** Every time `launch_browser()` is called, including the initial `show_disconnected()` call on startup.

**Trade-offs:** Adds up to ~500ms delay before browser appears. This is invisible to the user since rc-agent starts 1-3 seconds before the lock screen would be interactive anyway.

**Example:**
```rust
fn wait_for_lock_screen_server(port: u16) {
    // Poll up to 10 times with 100ms sleep = max 1s wait
    for attempt in 0..10 {
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            std::time::Duration::from_millis(100),
        ).is_ok() {
            return; // server is ready
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        if attempt == 9 {
            tracing::warn!("Lock screen server not ready after 1s — launching Edge anyway");
        }
    }
}
```

**Integration point:** Called at the top of `launch_browser()` before any Edge spawn attempt. No changes to `LockScreenManager` API.

### Pattern 2: Disconnected State on rc-agent Startup

**What:** After `start_server()` is called and config is validated successfully, rc-agent's main loop calls `lock_screen.show_disconnected()` before entering the WebSocket connection loop. This renders a branded "Connecting to server..." page immediately, replacing the blank/white Edge window that currently appears during the WS handshake window.

**When to use:** Once in `main.rs`, after successful config load and `start_server()`, before the first `connect_async()` attempt.

**Trade-offs:** None significant. `LockScreenState::Disconnected` already exists and renders correctly. This is a one-line addition.

**Integration point:** `rc-agent/src/main.rs` — between `early_lock_screen` teardown and first WebSocket connect attempt. The main `LockScreenManager` takes over from `early_lock_screen` after config loads.

### Pattern 3: Next.js Production Server as Windows Run Key

**What:** A `start-kiosk.bat` file on Racing-Point-Server (.23) runs `node C:\RacingPoint\kiosk\.next\standalone\...\server.js` with `PORT=3300` set. This bat file is registered under `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\RCKiosk`. On server reboot, the logged-in user session starts Node.js automatically.

**When to use:** One-time setup. Rerun only when deploying a new kiosk build.

**Trade-offs:** HKLM Run fires per user-login, not as a Windows Service. If the server reboots and nobody logs in, the kiosk is not running. For Racing Point's usage pattern (server is logged in 24/7 with auto-login), this is fine. If a Windows Service is ever needed, the same `server.js` can run via NSSM with no code changes.

**Example (start-kiosk.bat):**
```bat
@echo off
set PORT=3300
set HOSTNAME=0.0.0.0
cd /D C:\RacingPoint\kiosk
node .next\standalone\racingpoint\racecontrol\kiosk\server.js
```

**Integration point:** One-time deploy to .23. No Rust code changes. No changes to kiosk Next.js source.

### Pattern 4: Windows Hosts File for LAN Name Resolution

**What:** `C:\Windows\System32\drivers\etc\hosts` on .23 and .27 gets two entries: `192.168.31.23 kiosk.rp` and `192.168.31.23 api.rp`. Staff type `http://kiosk.rp:3300/kiosk` instead of `http://192.168.31.23:3300/kiosk`. Pods do not need this — they use the raw IP already configured in rc-agent.toml.

**When to use:** One-time setup on each machine where staff browse. Applied via `Add-Content` in PowerShell (one command, run as admin).

**Trade-offs:** Not automatic — new machines need the entry added manually. But the IP .23 is static (MAC-based DHCP reservation or manual assignment), so the entry does not expire. Router-based DNS would auto-propagate but requires router admin access and survives firmware updates unreliably.

**Example (PowerShell, run as admin on .23 and .27):**
```powershell
Add-Content C:\Windows\System32\drivers\etc\hosts "`n192.168.31.23  kiosk.rp api.rp"
```

## Data Flow

### Startup Ordering (Server .23)

```
.23 boots → auto-login fires HKLM Run keys
    │
    ├── start-rcagent.bat (existing) → rc-core starts on :8080
    │     └── rc-core ready: ~3-5s (config load + DB init)
    │
    └── start-kiosk.bat (NEW) → node server.js starts on :3300
          └── kiosk ready: ~2-4s (Next.js cold start)

Staff browser → http://kiosk.rp:3300/kiosk
    │
    └── Next.js kiosk page loads
          │
          └── useKioskSocket() connects: ws://192.168.31.23:8080/ws/dashboard
                └── rc-core must be up (if not, 3s retry loop in useKioskSocket)
```

### Pod Lock Screen Startup Ordering

```
Pod boots → HKLM Run key fires
    │
    └── start-rcagent.bat → rc-agent starts in Session 1
          │
          ├── start_server() — binds :18923 (tokio::spawn)
          │
          ├── show_disconnected() ← NEW: render immediately after start_server
          │     │
          │     └── launch_browser()
          │           ├── wait_for_lock_screen_server() ← NEW: probe :18923 up to 1s
          │           └── msedge.exe --kiosk http://127.0.0.1:18923
          │                 └── renders "Connecting..." branded page
          │
          └── connect_async(ws://192.168.31.23:8080/ws/agent) — 1-3s
                │
                ├── SUCCESS → SetLockScreen messages update UI state
                │             (Edge polls :18923 every 1s — page refreshes automatically)
                │
                └── FAILURE → reconnect loop (1s × 3 then exponential to 30s)
                              lock screen stays on "Disconnected" page
                              NO "Site cannot be reached" error
```

### DNS Resolution Flow (Staff browsing)

```
Staff types: http://kiosk.rp:3300/kiosk (or http://kiosk.rp/kiosk via nginx proxy — future)
    │
    └── Windows resolves kiosk.rp
          │
          ├── hosts file has "192.168.31.23  kiosk.rp" → resolves to .23
          │
          └── TCP connect to 192.168.31.23:3300
                └── Next.js kiosk server responds with 200
                      └── page loads, WS connects to :8080
```

### Kiosk API Call Flow (Unchanged)

```
Kiosk page action (e.g., "Start Billing")
    │
    └── api.ts: fetchApi("/billing/start", ...)
          │
          └── API_BASE = http://${window.location.hostname}:8080
                         (when served from kiosk.rp, hostname = kiosk.rp = .23 = correct)
                │
                └── POST http://192.168.31.23:8080/api/v1/billing/start
                      └── rc-core processes, responds
```

**Note on API_BASE:** `api.ts` already computes `http://${window.location.hostname}:8080`. When the kiosk is served from `kiosk.rp:3300`, `window.location.hostname` is `kiosk.rp`, which resolves to .23. This means API calls automatically target the correct server. No code change needed. This design is forward-compatible with a future nginx proxy on port 80.

## Integration Points

### Internal Boundaries (What Changes vs. Stays Same)

| Boundary | Communication | Change in v2.0 |
|----------|---------------|----------------|
| Staff browser ↔ kiosk Next.js | HTTP on :3300 | NEW: permanent host on .23, HKLM Run startup |
| kiosk Next.js ↔ rc-core | HTTP /api/v1 on :8080, WS /ws/dashboard | No change — hostname-relative URL already works |
| rc-agent lock screen ↔ Edge | HTTP on :18923, Edge polls every 1s | MODIFY: readiness probe + show_disconnected on startup |
| rc-agent ↔ rc-core | WebSocket ws://.23:8080/ws/agent | No change — existing reconnect loop is correct |
| pod-agent ↔ rc-core | HTTP POST /exec on :8090 | No change |
| .23/.27 ↔ kiosk.rp name | Windows hosts file lookup | NEW: one-time hosts file entry |

### What Must NOT Change

- `rc-agent.toml` on pods: `core.url = "ws://192.168.31.23:8080/ws/agent"` — pods use raw IP, not DNS name. Correct. No change.
- `kiosk/src/lib/api.ts` API_BASE: already correct — `http://${window.location.hostname}:8080`. No change.
- `kiosk/src/hooks/useKioskSocket.ts` WS_URL: already correct — `ws://${window.location.hostname}:8080/ws/dashboard`. No change.
- `next.config.ts`: `output: "standalone"` and `basePath: "/kiosk"` — correct. No change.
- rc-core CORS: already allows `origin.starts_with("http://192.168.31.")` — kiosk.rp resolves to .23 which is in this range. No change needed. If kiosk.rp is ever used directly (not resolved to .23 in the browser origin header), a `|| origin.contains("kiosk.rp")` guard may be needed.
- `LockScreenState::Disconnected` — already exists, already renders a branded page. No new state variants needed.

### Build Order (Phase Dependencies)

The four v2.0 work items are largely independent. The dependency graph is shallow.

```
Layer 1 — Investigation (no code, inform all other layers)
  Read error/debug logs from pods and server
  Identify which failure mode is most common:
    (a) Edge gets "Site cannot be reached" before :18923 binds
    (b) Pod reboots and nobody types a URL — no kiosk starts
    (c) rc-agent crashes — Edge shows stale/blank page
    (d) DHCP drift — .23 gets a new IP, all hardcoded URLs break
  → Output: confirmed root cause list

Layer 2 — Server-side (independent of pod changes)
  A) Deploy Next.js production build to permanent location on .23
  B) Register HKLM Run key for kiosk auto-start (start-kiosk.bat)
  C) Add hosts file entries on .23 and .27
  D) Verify rc-core CORS allows kiosk.rp origin if needed
  → Gate: http://kiosk.rp:3300/kiosk loads, WS connects, pod list appears

Layer 3 — Pod lock screen hardening (requires Layer 1 diagnosis)
  A) Add wait_for_lock_screen_server() probe in lock_screen.rs launch_browser()
  B) Call show_disconnected() immediately after start_server() in main.rs
  → Gate: reboot a pod, verify "Connecting..." appears within 5s, no browser error page

Layer 4 — Static IP enforcement (blocks all other layers' DHCP drift risk)
  A) Configure static IP .23 for Racing-Point-Server (DHCP reservation by MAC or
     manual IP assignment on the server's NIC)
  B) Verify no DHCP drift after 48h
  → Gate: ping 192.168.31.23 from all pods, confirm consistent response
```

**Ordering rationale:**
- Layer 1 (investigation) must come first — implementing fixes without confirmed root causes risks solving the wrong problem
- Layer 4 (static IP) is technically a prerequisite for all other layers, but in practice .23's DHCP has been stable. It should be scheduled early but not block Layer 2/3 development
- Layer 2 and Layer 3 are fully independent — they can be developed in parallel if desired
- Layer 2 has higher business value (staff terminal is used every session) and zero risk of breaking existing functionality
- Layer 3 requires a Rust compile and pod deploy — schedule it after Layer 2 is validated

## Anti-Patterns

### Anti-Pattern 1: Router-Based DNS

**What people do:** Configure custom DNS records on the LAN router (Asus/TP-Link) admin panel so that `kiosk.rp` resolves for all devices automatically.

**Why it's wrong:** Router firmware updates erase custom DNS config without warning. At Racing Point, the router has been factory-reset before. DHCP-assigned router address may also drift. DNS TTL issues on Windows cause stale caching. A hosts file entry on two machines (.23 and .27) is more durable and requires zero router access.

**Do this instead:** Windows hosts file on .23 and .27. Pods don't need the name — they use raw IPs.

### Anti-Pattern 2: Next.js Dev Server in Production

**What people do:** `npm run dev` on the server, relied on as the permanent kiosk URL. The dev server is already compiled (`.next/standalone/` exists) but dev mode is used instead.

**Why it's wrong:** Dev server hot-reloads on every file change (causes the kiosk to blink/reload during unrelated work), consumes 2-3x more RAM, and exits on any unhandled exception. The standalone build is already compiled — using dev mode adds cost and fragility for zero benefit.

**Do this instead:** `node .next/standalone/.../server.js` with `PORT=3300`. Already built. Just needs a startup script.

### Anti-Pattern 3: Spawning Edge Before :18923 Binds

**What people do:** Call `launch_browser()` immediately after `start_server()` (which spawns a tokio task). The tokio task may not have bound the port yet. Edge gets "connection refused" on the first GET and shows a browser error page.

**Why it's wrong:** Browser error pages have no auto-retry. The customer sees a broken screen that persists until rc-agent re-triggers a lock screen state update (which only happens on a WebSocket message). If the WebSocket also hasn't connected yet, the screen stays broken indefinitely.

**Do this instead:** `wait_for_lock_screen_server()` probe in `launch_browser()`. At most 1s delay, invisible to the user.

### Anti-Pattern 4: Hardcoding IPs in api.ts

**What people do:** Set `NEXT_PUBLIC_API_URL=http://192.168.31.23:8080` in a `.env` file baked into the Next.js build.

**Why it's wrong:** If the server IP changes (DHCP drift, hardware swap), the kiosk is broken and requires a new build + deploy. The current `window.location.hostname`-relative approach is already correct — it derives the API host from where the kiosk is served, which is always the same machine as rc-core.

**Do this instead:** Leave `api.ts` unchanged. The hostname-relative detection already works correctly.

### Anti-Pattern 5: Windows Service for Next.js Kiosk

**What people do:** Install Node.js kiosk server as a Windows Service (via NSSM or sc.exe) to guarantee it runs regardless of login state.

**Why it's wrong for this venue:** The server has auto-login enabled. A Windows Service runs in Session 0 (fine for a Node.js HTTP server with no GUI), but adds NSSM as a dependency, complicates upgrades (stop service → replace binary → start service vs. kill process → restart bat), and requires admin intervention to manage. The HKLM Run key pattern already works for rc-agent on 8 pods. Apply the same pattern.

**Do this instead:** HKLM Run key + `start-kiosk.bat`. Consistent with existing rc-agent pattern. Can be upgraded to a Service later if the server ever stops having auto-login.

## Scaling Considerations

This is a fixed 8-pod venue. Scaling to more pods or machines is not a concern. The relevant reliability axis is "survives venue incidents":

| Incident | Impact Without v2.0 | Impact With v2.0 |
|----------|--------------------|-----------------------|
| Server reboot | Kiosk URL unreachable until staff manually starts it | HKLM Run key restarts kiosk automatically |
| Pod reboot | Lock screen shows "Site cannot be reached" until rc-agent starts | Readiness probe prevents browser error |
| DHCP drift (.23 gets new IP) | All kiosk URLs and rc-agent.toml break | Static IP assignment prevents drift |
| rc-agent crash mid-session | Lock screen stuck on last state or browser error | show_disconnected() fires on reconnect; customer sees branded screen |
| Staff types wrong URL | 404 or "refused" | kiosk.rp always resolves, no manual IP recall needed |

## Sources

- `crates/rc-agent/src/lock_screen.rs` — `launch_browser()`, `start_server()`, `LockScreenState::Disconnected` (HIGH)
- `crates/rc-agent/src/main.rs` — startup sequence: `start_server()` before config load, `early_lock_screen` pattern (HIGH)
- `kiosk/src/lib/api.ts` — `API_BASE` hostname-relative detection (HIGH)
- `kiosk/src/hooks/useKioskSocket.ts` — `WS_URL` hostname-relative, 3s reconnect loop (HIGH)
- `kiosk/next.config.ts` — `output: "standalone"`, `basePath: "/kiosk"` (HIGH)
- `kiosk/package.json` — `start: "next start -p 3300"`, confirms production port (HIGH)
- `kiosk/.next/standalone/` — standalone build already compiled (HIGH — verified via ls)
- `crates/rc-core/src/main.rs` — CORS allows `192.168.31.*` origins (HIGH)
- `racecontrol.toml` — server binds `0.0.0.0:8080` confirming it accepts LAN requests (HIGH)
- `MEMORY.md` — HKLM Run key pattern from Session 0 fix (HIGH), network map confirming .23 IP (HIGH), pod deploy kit pattern (HIGH)
- `.planning/PROJECT.md` — v2.0 requirements: investigation-first, static IP, DNS name, lock screen fallback (HIGH)

---
*Architecture research for: RaceControl v2.0 Kiosk URL Reliability*
*Researched: 2026-03-13*
