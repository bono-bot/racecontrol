# Requirements: RaceControl v2.0 Kiosk URL Reliability

**Defined:** 2026-03-13
**Core Value:** Every URL in the venue always works — staff kiosk, customer PIN grid, and pod lock screens are permanently accessible with zero manual intervention.

## v2.0 Requirements

### Diagnosis & Investigation

- [x] **DIAG-01**: Staff can collect error/debug logs from all 8 pods and server to confirm actual URL failure patterns
- [x] **DIAG-02**: Staff can run a port audit on Server (.4, drifted from .23) to identify port conflicts before deploying the kiosk
- [x] **DIAG-03**: Staff can verify Edge version and kiosk mode settings (StartupBoost, EdgeUpdate, BackgroundMode) across all pods
- [x] **DIAG-04**: Staff can confirm Server (.4) IP assignment type (DHCP, lease expires nightly) and retrieve MAC address (BC-FC-E7-2C-F2-CE) for DHCP reservation

### Staff Kiosk Hosting

- [ ] **HOST-01**: Staff kiosk runs as a production Next.js build on Server (.23) — no dev server
- [ ] **HOST-02**: Staff kiosk auto-starts on Server (.23) boot via HKLM Run key (Session 1)
- [ ] **HOST-03**: Server (.23) IP is pinned via DHCP reservation at the router so it never drifts
- [ ] **HOST-04**: Staff can access the kiosk at `kiosk.rp` from any device on the LAN via hosts file entries

### Pod Lock Screen Resilience

- [ ] **LOCK-01**: Pod startup waits for rc-agent HTTP server (port 18923) to be ready before launching Edge kiosk browser
- [ ] **LOCK-02**: Pod lock screen shows a branded "Connecting..." page on startup instead of a blank window or browser error
- [ ] **LOCK-03**: Pod lock screen HTML auto-retries connection to rc-agent and recovers without manual intervention when rc-agent restarts

### Kiosk Mode Control

- [ ] **KIOSK-01**: Staff can toggle full pod lockdown (taskbar, Win key, Edge kiosk) on or off for a specific pod from the staff kiosk dashboard
- [ ] **KIOSK-02**: Staff can lock or unlock all 8 pods at once from the staff kiosk dashboard (e.g., opening/closing the venue)

### Pod Power Management

- [ ] **PWR-01**: Staff can power off a specific pod remotely from the staff kiosk dashboard
- [ ] **PWR-02**: Staff can restart a specific pod remotely from the staff kiosk dashboard
- [ ] **PWR-03**: Staff can power on a specific pod remotely from the staff kiosk dashboard (Wake-on-LAN)
- [ ] **PWR-04**: Staff can power off all 8 pods at once from the staff kiosk dashboard
- [ ] **PWR-05**: Staff can restart all 8 pods at once from the staff kiosk dashboard
- [ ] **PWR-06**: Staff can power on all 8 pods at once from the staff kiosk dashboard (Wake-on-LAN)

### Screen Branding & Wallpaper

- [ ] **BRAND-01**: Lock screen displays the Racing Point logo prominently
- [ ] **BRAND-02**: Staff can set a dynamic or static wallpaper for the blanking/lock screen from the kiosk dashboard
- [ ] **BRAND-03**: A branded loading screen with Racing Point identity is shown before each game session launches

### Session Results Display

- [ ] **SESS-01**: After each session, the pod displays telemetry summary (lap times, top speed, best lap)
- [ ] **SESS-02**: After each session, the pod displays race position if racing against AI or in multiplayer (1st, 2nd, 3rd, etc.)
- [ ] **SESS-03**: Session results remain visible on the pod screen until a new session is initialized by staff or customer

### Edge Browser Hardening

- [ ] **EDGE-01**: EdgeUpdate service is disabled on all 8 pods to prevent auto-updates from breaking kiosk mode
- [ ] **EDGE-02**: StartupBoostEnabled is disabled via registry on all 8 pods to prevent background Edge conflicts
- [ ] **EDGE-03**: BackgroundModeEnabled is disabled via registry on all 8 pods to prevent Edge persisting after close

## Future Requirements

### Monitoring & Polish

- **MON-01**: Health check endpoint on kiosk server for uptime monitoring
- **MON-02**: CORS update in rc-core for `kiosk.rp` origin header
- **MON-03**: Edge version pinning across all pods
- **MON-04**: Log rotation for kiosk server output

## Out of Scope

| Feature | Reason |
|---------|--------|
| NSSM service manager | Abandoned (last release 2017), AV-flagged on Windows 11 |
| mDNS / `.local` domain | Conflicts with Windows 11 mDNS resolver and Bonjour |
| Docker containerization | Breaks localhost port assumptions, GPU overhead on gaming pods |
| Full DNS server (Acrylic/Unbound) | Overkill for 10-device LAN; hosts file is simpler and offline-safe |
| Static NIC IP (no DHCP) | Router reset loses config; DHCP reservation + NIC backup is safer |
| HTTPS on LAN | No external exposure, adds cert management complexity for zero benefit |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DIAG-01 | Phase 6 | Complete |
| DIAG-02 | Phase 6 | Pending |
| DIAG-03 | Phase 6 | Complete |
| DIAG-04 | Phase 6 | Pending |
| HOST-01 | Phase 7 | Pending |
| HOST-02 | Phase 7 | Pending |
| HOST-03 | Phase 7 | Pending |
| HOST-04 | Phase 7 | Pending |
| LOCK-01 | Phase 8 | Pending |
| LOCK-02 | Phase 8 | Pending |
| LOCK-03 | Phase 8 | Pending |
| KIOSK-01 | Phase 10 | Pending |
| KIOSK-02 | Phase 10 | Pending |
| PWR-01 | Phase 10 | Pending |
| PWR-02 | Phase 10 | Pending |
| PWR-03 | Phase 10 | Pending |
| PWR-04 | Phase 10 | Pending |
| PWR-05 | Phase 10 | Pending |
| PWR-06 | Phase 10 | Pending |
| BRAND-01 | Phase 11 | Pending |
| BRAND-02 | Phase 11 | Pending |
| BRAND-03 | Phase 11 | Pending |
| SESS-01 | Phase 11 | Pending |
| SESS-02 | Phase 11 | Pending |
| SESS-03 | Phase 11 | Pending |
| EDGE-01 | Phase 9 | Pending |
| EDGE-02 | Phase 9 | Pending |
| EDGE-03 | Phase 9 | Pending |

**Coverage:**
- v2.0 requirements: 28 total
- Mapped to phases: 28
- Unmapped: 0

---
*Requirements defined: 2026-03-13*
*Last updated: 2026-03-13 after roadmap created (phases 6–11)*
