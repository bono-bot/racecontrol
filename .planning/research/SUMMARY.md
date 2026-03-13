# Project Research Summary

**Project:** RaceControl v2.0 — Kiosk URL Reliability
**Domain:** Windows LAN kiosk service hosting, local hostname resolution, pod lock screen resilience
**Researched:** 2026-03-13
**Confidence:** HIGH

## Executive Summary

RaceControl v2.0 addresses a single failure class: URLs that break. Two URL surfaces fail regularly — the staff Next.js kiosk terminal on Racing-Point-Server (.23) lacks auto-start and a stable address, and the pod lock screens show an unbranded "Site cannot be reached" error when rc-agent hasn't finished binding its HTTP port. Both failures are well-understood infrastructure problems with well-documented solutions: production Next.js requires a `next build` + auto-start mechanism, servers on a managed LAN need pinned IPs, and browser kiosk mode requires the HTTP server to be ready before the browser is launched.

The recommended approach keeps the existing architecture intact and adds four targeted fixes in order of dependency: (1) pin the server IP via DHCP reservation, (2) auto-start the Next.js kiosk on server reboot using the HKLM Run key pattern already proven for rc-agent, (3) add a TCP readiness probe in rc-agent's `launch_browser()` to prevent Edge from racing ahead of the HTTP server, and (4) deploy `kiosk.rp` hosts file entries on James's machine and the reception tablet. No new infrastructure, no new Rust dependencies, no running daemon.

**Critical conflict resolved:** STACK.md recommended NSSM 2.24 for Windows service management. PITFALLS.md correctly identifies NSSM as abandoned (last release 2017), AV-flagged on Windows 11, and known to cause startup failures after Windows Feature Updates. ARCHITECTURE.md independently reached the same conclusion — the HKLM Run key pattern already used for rc-agent is the correct primitive for the kiosk server too. **NSSM must not be introduced.** Use the HKLM Run key for the Next.js kiosk (consistent with rc-agent pattern), and `sc.exe` with native recovery actions if a true Windows Service is ever needed for rc-core.

## Key Findings

### Recommended Stack

The v2.0 stack adds no new Rust dependencies and no new Node packages. All new components are either OS built-ins (hosts file, HKLM Run key) or the standalone Next.js build already present on the server. The most significant "stack decision" is the auto-start mechanism — and the correct answer is the HKLM Run key pattern, not NSSM.

**Core technologies:**

- **HKLM Run key + `start-kiosk.bat`**: Auto-start Next.js kiosk on server reboot — same proven pattern as rc-agent on all 8 pods; fires in Session 1 (logged-in user context), no third-party dependency, works with the standalone build already compiled at `kiosk/.next/standalone/`
- **`sc.exe` with failure actions** (if a true Windows Service is needed): Native Windows service management for background non-GUI processes like rc-core; `sc failure <svc> actions=restart/5000/restart/30000/restart/60000` provides crash recovery without NSSM; should NOT be used for GUI or kiosk processes (Session 0 blindness)
- **Task Scheduler "At logon, run only when logged on"**: Alternative to HKLM Run key if finer scheduling control is needed; identical Session 1 guarantee; use over NSSM for any process that requires an interactive desktop
- **Windows hosts file** (`C:\Windows\System32\drivers\etc\hosts`): LAN hostname resolution for `kiosk.rp` → `192.168.31.23`; checked before DNS, works offline, persistent across reboots; deploy via pod-agent exec as one-liner PowerShell
- **DHCP reservation at router** (Xiaomi, 192.168.31.1): Pins server MAC to .23 permanently; prevents IP drift that has already caused outages; single source of truth; one-time config
- **Next.js standalone production build** (`next build`): Already compiled at `kiosk/.next/standalone/`; `node server.js` with `PORT=3300`; prerequisite for all auto-start approaches
- **Vanilla JS polling loop** (embedded in rc-agent lock screen HTML): Shows branded "Connecting..." splash instead of Chrome `ERR_CONNECTION_REFUSED` while rc-agent HTTP server starts; `setInterval` fetch-poll every 2s to `http://127.0.0.1:18923/health`; no new Rust dependencies

**What NOT to use:**

- **NSSM 2.24**: Abandoned (last release 2017), flagged by Windows Defender as PUP, causes intermittent startup failures on Windows 11 22H2+. All tutorials recommending it were written 2012–2018. Do not introduce it.
- **PM2 / pm2-windows-service**: Unreliable on Windows — `pm2 resurrect` on service boot is flaky, requires `PM2_HOME` at SYSTEM level, multiple open issues with no resolution.
- **`.local` TLD**: mDNS-triggered on Windows 11, resolution unreliable when multicast is blocked (which consumer routers do silently); use `.rp` instead.

### Expected Features

The feature set is small, precisely scoped, and fully P1-weighted. There is almost nothing to defer.

**Must have (table stakes — v2.0 launch):**

- DHCP reservation for server .23 MAC in router — prevents IP drift, unblocks everything else
- `next build` production build of kiosk — prerequisite for reliable service; current `next dev` usage is never acceptable in production
- HKLM Run key for kiosk auto-start (`start-kiosk.bat`) — kiosk survives server reboots; consistent with existing rc-agent pattern
- `kiosk.rp` hosts file entry on James's machine and reception tablet — permanent staff URL; eliminates manual IP recall
- TCP readiness probe in rc-agent `launch_browser()` — poll `:18923` before spawning Edge; max 1s delay, invisible to customer
- `show_disconnected()` called immediately after `start_server()` in rc-agent main — branded "Connecting..." screen replaces blank/white Edge window during WS handshake

**Should have (add after v2.0 validation):**

- rc-core as native Windows service via `sc.exe` — auto-recovers after server reboot; independent of kiosk fix
- NSSM log rotation replacement: stdout/stderr redirect in `start-kiosk.bat` via `>> C:\RacingPoint\logs\kiosk.log 2>&1` — log management without NSSM dependency
- "Backend offline" terminal state in kiosk UI (`useKioskSocket.ts`) — shows definitive error after 30s instead of perpetual "Connecting..."

**Defer to v2.x/v3+:**

- Startup attempt counter in waiting/splash HTML — polish, not reliability
- Hosts file push to all 8 pods — pods use raw IPs, don't browse to `kiosk.rp`; low priority
- Nginx reverse proxy on port 80 — future convenience, adds complexity now

**Anti-features confirmed — do not implement:**

- Static IP on Windows NIC instead of DHCP reservation — causes IP conflicts if router also has a lease for the same IP
- Local DNS server (dnsmasq, Pi-hole) — unnecessary infrastructure for 8 pods; hosts file achieves the same with no running process
- HTTPS on closed LAN — certificate management overhead with zero security benefit
- Serving lock screen from rc-core instead of rc-agent — breaks offline-first requirement

### Architecture Approach

The v2.0 architecture makes no structural changes. It wires up four independent, shallow changes against existing components. The entire implementation lives at the edge of the system — startup scripts, hosts file, one new Rust function, and one new JS polling loop. The core data flow (rc-agent ↔ rc-core WebSocket, kiosk ↔ rc-core API) is unchanged and requires no modification.

**Major components and what changes:**

1. **`start-kiosk.bat` (NEW, on Racing-Point-Server .23)** — `node .next/standalone/.../server.js` with `PORT=3300`; registered under `HKLM\...\Run\RCKiosk`; auto-starts Next.js in Session 1 on server login
2. **`rc-agent/src/lock_screen.rs` (MODIFY)** — add `wait_for_lock_screen_server()` function that probes `127.0.0.1:18923` via `TcpStream::connect_timeout` up to 10 times (100ms each) before spawning Edge; add call to `show_disconnected()` immediately after `start_server()` in main.rs
3. **Windows hosts file on .23 and .27 (NEW entries)** — `192.168.31.23 kiosk.rp api.rp`; applied once via PowerShell `Add-Content`; idempotent with check-before-write
4. **DHCP reservation on Xiaomi router (ONE-TIME CONFIG)** — MAC→IP binding for Racing-Point-Server; confirms and locks the .23 address permanently

**Key constraint preserved:** The kiosk's `api.ts` uses `http://${window.location.hostname}:8080` — hostname-relative API URL detection already works correctly when served from `kiosk.rp`. Do not set `NEXT_PUBLIC_API_URL` env var. Do not hardcode IPs. No change needed.

**Build order for v2.0 (layers are largely independent):**

- Layer 1 (Investigation): Confirm current failure modes, baseline `netstat -ano` on .23, check current DHCP state
- Layer 2 (Server-side, zero risk): DHCP reservation + `next build` verification + `start-kiosk.bat` + hosts file entries
- Layer 3 (Pod lock screen, requires Rust compile + deploy): `wait_for_lock_screen_server()` probe + `show_disconnected()` on startup
- Layer 4 (Independent, concurrent with Layer 2): Verify Edge policy settings on pods (disable `StartupBoostEnabled`, `BackgroundModeEnabled`, EdgeUpdate service)

### Critical Pitfalls

1. **Do not use NSSM** — It is abandoned (2017), AV-flagged, and breaks on Windows 11 Feature Updates. Use HKLM Run key for Session 1 processes (kiosk, rc-agent). Use `sc.exe` with failure actions for headless background services (rc-core). Task Scheduler "At logon, run only when logged on" is an acceptable alternative for HKLM Run.

2. **Session 0 GUI blindness** — Any Windows Service (SYSTEM account, `sc.exe`, NSSM) that starts a GUI or kiosk process will render nothing — the process runs, the port binds, but the screen is black. rc-agent is already correctly using HKLM Run (Session 1). The new kiosk bat file must also use HKLM Run. Never use `sc.exe` or any SYSTEM service for processes that need to display on screen.

3. **Edge auto-update breaks kiosk flags** — Edge 128 caused venue-wide white screen on kiosk mode. Disable `EdgeUpdate` service and `EdgeUpdateM` service on all pods. Set `StartupBoostEnabled = 0` and `BackgroundModeEnabled = 0` via registry. Pass `--user-data-dir=C:\RacingPoint\EdgeKiosk` on every Edge kiosk launch to isolate the profile.

4. **TCP readiness probe must cover the restart path, not just cold boot** — The `wait_for_lock_screen_server()` probe prevents the boot-time race. The crash-restart path also needs it: when rc-agent restarts mid-session, the existing Edge window will show `ERR_CONNECTION_REFUSED` unless the relaunch waits for port 18923. Verify the readiness check is in every call path that spawns Edge.

5. **DNS cache requires flush + Edge restart after hosts file change** — `ipconfig /flushdns` clears the Windows DNS Client cache but Edge maintains a separate internal DNS cache (Chromium's `//net` stack). After deploying the hosts file entry, also kill all `msedge.exe` processes and relaunch. Verification must be done from inside Edge, not just `nslookup`.

6. **DHCP reservation is not enough alone — use two-layer IP pinning** — Set the DHCP reservation in the router AND set a static IP on the server NIC. If the router resets and loses its reservation, the server still holds .23. If the NIC has only a static IP (no reservation), the router may lease .23 to another device causing an IP collision that manifests as intermittent packet loss rather than a clean failure.

7. **`.local` TLD causes mDNS conflicts on Windows 11** — Do not use `kiosk.local`. Windows 11 routes `.local` through the mDNS resolver which depends on multicast UDP. Xiaomi consumer router mDNS relay behavior is untested. Use `.rp` TLD — it is not a real TLD, not used by mDNS, and resolves exclusively via hosts file.

## Implications for Roadmap

Based on the dependency graph in FEATURES.md and the Layer 1–4 build order in ARCHITECTURE.md, the natural phase structure is four phases with the DHCP/IP fix mandatory first.

### Phase 1: Diagnosis and Baseline

**Rationale:** ARCHITECTURE.md is explicit — implementing fixes without confirmed root causes risks solving the wrong problem. All other phases must be informed by what is actually failing. This phase has zero code changes and zero deployment risk.

**Delivers:** Confirmed root cause list; baseline `netstat -ano` on .23; documented current server IP state (DHCP vs static); Edge version on all 8 pods; current kiosk start mechanism; port occupancy map

**Features addressed:** None directly — enables all subsequent phases

**Pitfalls avoided:** Solving the wrong failure mode; port conflict surprises in Phase 2; building on a drifting IP that changes during the work

**Research flag:** Standard investigation. No additional research needed.

### Phase 2: Server-Side Pinning (Zero Pod Risk)

**Rationale:** Server-side changes (DHCP reservation, production build, HKLM Run key, hosts file) carry zero risk to pod operation. No Rust compile, no pod deploy. These can be validated by a staff member navigating to `http://kiosk.rp:3300/kiosk` before any pod changes are made. DHCP reservation must come first because the hosts file maps to the server IP — if the IP drifts after the entry is written, the name resolves to the wrong address.

**Delivers:** Server .23 locked to stable IP; Next.js kiosk running as production build from `start-kiosk.bat` HKLM Run key; `kiosk.rp` resolves on James's machine and reception tablet; kiosk survives server reboots

**Features addressed:** DHCP reservation, `next build`, HKLM Run auto-start, hosts file entries

**Pitfalls avoided:** Pitfall 1 (NSSM — use HKLM Run instead), Pitfall 2 (Session 0 — HKLM Run fires in Session 1), Pitfall 7 (next dev in production), Pitfall 4 (DHCP drift — two-layer IP pinning)

**Research flag:** Standard patterns. HKLM Run key is already documented in MEMORY.md and proven on 8 pods. `next build` + `next start` is standard Next.js. No research needed.

### Phase 3: Pod Lock Screen Hardening

**Rationale:** This is the only phase requiring a Rust compile and pod deploy. It should come after Phase 2 is validated so the server is stable before pods are touched. ARCHITECTURE.md specifies both code changes precisely — `wait_for_lock_screen_server()` in `lock_screen.rs` and a `show_disconnected()` call in `main.rs`. These are small, targeted changes with well-defined test gates.

**Delivers:** rc-agent no longer shows `ERR_CONNECTION_REFUSED` on boot or restart; branded "Connecting..." screen shows immediately after rc-agent starts; Edge is never launched before port 18923 is ready; crash-restart path also covered

**Features addressed:** TCP readiness probe, `show_disconnected()` on startup, branded waiting state

**Pitfalls avoided:** Pitfall 10 (`ERR_CONNECTION_REFUSED` in kiosk — readiness probe on all Edge launch paths), Pitfall 1 (Session 0 — HKLM Run already in place, no regression), Pitfall 9 (StartupBoost — disable via registry on all pods before deploying)

**Research flag:** No additional research needed. Code patterns are fully specified in ARCHITECTURE.md with working Rust examples. Test gate is: reboot pod, verify "Connecting..." screen appears within 5 seconds, no browser error page visible.

### Phase 4: Edge Policy Hardening

**Rationale:** Edge auto-updates have caused venue-wide kiosk failures before (Edge 128 incident). Disabling `EdgeUpdate`, `StartupBoostEnabled`, and `BackgroundModeEnabled` via registry is independent of Phases 2 and 3 and can proceed in parallel. However, it must be done before Phase 3 is deployed to ensure the new rc-agent kiosk launch flags (`--user-data-dir`) work correctly. Sequenced after Phase 3 because it relies on the updated launch command.

**Delivers:** EdgeUpdate service disabled on all 8 pods; `StartupBoostEnabled = 0` and `BackgroundModeEnabled = 0` via registry; `--user-data-dir=C:\RacingPoint\EdgeKiosk` on every Edge kiosk launch; Edge version pinned and documented

**Features addressed:** Edge kiosk mode stability, protection against future Edge updates breaking kiosk behavior

**Pitfalls avoided:** Pitfall 2 (Edge auto-update breaking kiosk flags), Pitfall 9 (StartupBoost conflict with `--kiosk` flag)

**Research flag:** Standard patterns. Microsoft Group Policy templates and registry keys for Edge are stable and documented. No research needed.

### Phase Ordering Rationale

- Phase 1 (diagnosis) must come first — architecture explicitly recommends investigation before implementation
- Phase 2 (server-side) has the highest impact-to-risk ratio: fixes the most frequent staff pain point (unreachable kiosk URL) with zero pod deployment risk
- Phase 3 (pod lock screen) requires Rust compile + deploy to all 8 pods — lower risk after Phase 2 is stable and the server IP is locked
- Phase 4 (Edge policies) can be done alongside Phase 3 or as a follow-on; it is prerequisite to declaring the lock screen fix complete

### Research Flags

All phases use standard, well-documented patterns. No phases need `/gsd:research-phase`.

- **Phase 1:** Pure investigation — run netstat, ipconfig, tasklist on .23
- **Phase 2:** HKLM Run pattern already in MEMORY.md; Next.js standalone already documented; hosts file is OS built-in
- **Phase 3:** Code patterns with working Rust examples in ARCHITECTURE.md; TDD with Pod 8 first
- **Phase 4:** Registry keys from Microsoft docs; no new logic

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Conflict resolved: NSSM explicitly rejected in favor of HKLM Run key (ARCHITECTURE.md) and `sc.exe` (PITFALLS.md). DHCP reservation, hosts file, standalone Next.js are all well-documented OS primitives. |
| Features | HIGH | Derived from direct codebase inspection of `kiosk/package.json`, `lock_screen.rs`, `useKioskSocket.ts`, `next.config.ts`, `rc-agent.example.toml`. Priority matrix is exact — almost all features are P1. |
| Architecture | HIGH | Build order confirmed by direct file inspection (`kiosk/.next/standalone/` already compiled, `LockScreenState::Disconnected` already exists). All "what must NOT change" items verified in source. |
| Pitfalls | HIGH | Session 0 blindness and DHCP drift are production-observed failures at this venue. Edge 128 kiosk regression is confirmed via Microsoft support docs. NSSM abandonment confirmed via GitHub commit history and SaltStack deprecation issue. |

**Overall confidence:** HIGH

### Gaps to Address

- **NSSM conflict fully resolved:** STACK.md recommended NSSM; ARCHITECTURE.md and PITFALLS.md both independently rejected it. The synthesis conclusion is unambiguous — HKLM Run key is correct for Session 1 processes; `sc.exe` for background headless services. Roadmap must contain no NSSM references.
- **Server .23 MAC address:** The DHCP reservation requires the exact server MAC. This must be retrieved via `ipconfig /all` on Racing-Point-Server during Phase 1 before the router config is touched.
- **rc-core CORS and `kiosk.rp` origin:** ARCHITECTURE.md notes that CORS allows `origin.starts_with("http://192.168.31.")`. When the kiosk is served from `kiosk.rp`, the browser sends `Origin: http://kiosk.rp`. Verify this origin passes the CORS check in `rc-core/src/main.rs` — a `|| origin.contains("kiosk.rp")` guard may be needed. Low risk but must be checked during Phase 2.
- **kiosk port:** FEATURES.md references port 3300, `kiosk/package.json` confirms `"start": "next start -p 3300"`. STACK.md diagram shows port 3000. Confirm actual production port during Phase 1 before configuring auto-start.

## Sources

### Primary (HIGH confidence)

- `crates/rc-agent/src/lock_screen.rs` — `launch_browser()`, `start_server()`, `LockScreenState::Disconnected`
- `crates/rc-agent/src/main.rs` — startup sequence, `early_lock_screen` pattern
- `kiosk/src/lib/api.ts` — `API_BASE` hostname-relative detection (unchanged)
- `kiosk/src/hooks/useKioskSocket.ts` — WS_URL, 3s reconnect loop (unchanged)
- `kiosk/next.config.ts` — `output: "standalone"`, `basePath: "/kiosk"`
- `kiosk/package.json` — `start: "next start -p 3300"`
- `kiosk/.next/standalone/` — standalone build already compiled (verified via ls)
- `crates/rc-core/src/main.rs` — CORS allows `192.168.31.*` origins
- `.planning/PROJECT.md` — v2.0 requirements: investigation-first, static IP, DNS name, lock screen fallback
- `MEMORY.md` — HKLM Run key pattern, network map, pod deploy rules, Session 0 fix history

### Secondary (MEDIUM confidence)

- [Microsoft: Configure Edge kiosk mode](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-configure-kiosk-mode) — StartupBoostEnabled incompatibility, --user-data-dir requirement, kiosk type options
- [Edge 128 kiosk white screen](https://learn.microsoft.com/en-us/answers/questions/2403205/white-screen-on-kiosk-mode-after-ms-edge-updated-t) — confirmed production regression from auto-update
- [NSSM SaltStack deprecation](https://github.com/saltstack/salt/issues/59148) — abandoned, AV-flagged, Windows 11 issues
- [Static IP vs DHCP Reservation](https://www.stephenwagner.com/2019/05/07/static-ip-vs-dhcp-reservation/) — two-layer pinning rationale
- [mDNS .local conflicts on Windows 11](https://community.start9.com/t/solved-mdns-on-windows-11-partially-works/1859) — confirmed multicast reliability issue
- [DHCP reservation best practice](https://inventivehq.com/blog/why-dhcp-is-better-than-static-ip-addresses-even-for-servers) — router as single source of truth

### Tertiary (LOW confidence — needs validation during implementation)

- Xiaomi router DHCP reservation UI — varies by firmware version; verify at http://192.168.31.1 before Phase 2
- rc-core CORS `kiosk.rp` origin behavior — inferred from source; verify with a live request during Phase 2

---
*Research completed: 2026-03-13*
*Ready for roadmap: yes*
