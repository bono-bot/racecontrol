# Roadmap: RaceControl

## Completed Milestones

<details>
<summary>v1.0 RaceControl HUD & Safety — 5 phases, 15 plans (Shipped 2026-03-13)</summary>

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for full phase details and plan breakdown.

Phases: State Wiring & Config Hardening → Watchdog Hardening → WebSocket Resilience → Deployment Pipeline Hardening → Blanking Screen Protocol

</details>

## Current Milestone

### v2.0 Kiosk URL Reliability (Phases 6–11)

**Milestone Goal:** Eliminate all "Site cannot be reached" and 404 errors across the venue — every kiosk URL works permanently after any reboot, crash, or network change.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 6: Diagnosis** - Confirm actual URL failure modes before touching anything
- [ ] **Phase 7: Server-Side Pinning** - Lock server IP and auto-start kiosk as production build
- [ ] **Phase 8: Pod Lock Screen Hardening** - Rust compile + deploy: readiness probe + branded waiting state
- [ ] **Phase 9: Edge Browser Hardening** - Disable auto-update, StartupBoost, BackgroundMode on all pods
- [ ] **Phase 10: Staff Dashboard Controls** - Power management and kiosk lockdown controls in the UI
- [ ] **Phase 11: Customer Experience Polish** - Session results display and branded lock screen identity

## Phase Details

### Phase 6: Diagnosis
**Goal**: Staff have a confirmed root-cause map of URL failures, baseline state of every relevant system component, and no open questions that would cause Phase 7 to solve the wrong problem
**Depends on**: Nothing (first v2.0 phase)
**Requirements**: DIAG-01, DIAG-02, DIAG-03, DIAG-04
**Success Criteria** (what must be TRUE):
  1. Staff can view collected error and debug logs from all 8 pods and the server, revealing which URLs fail and under what conditions
  2. Staff can read a port audit of Server (.23) showing which ports are occupied and whether any conflict with port 3300 or 8080
  3. Staff can confirm the Edge version and the current values of StartupBoostEnabled, BackgroundModeEnabled, and EdgeUpdate service status on every pod
  4. Staff can confirm whether Server (.23) holds a static IP or a DHCP lease, and can read its MAC address for use in the DHCP reservation
**Plans:** 2/2 plans executed

Plans:
- [x] 06-01-PLAN.md — Collect rc-agent logs and Edge settings baseline from all 8 pods
- [x] 06-02-PLAN.md — Server port audit and IP/MAC identification (via pod-agent, not RDP)

### Phase 7: Server-Side Pinning
**Goal**: The staff kiosk is reachable at a stable, named address from any device on the LAN and survives server reboots without manual intervention — with zero changes to pods
**Depends on**: Phase 6
**Requirements**: HOST-01, HOST-02, HOST-03, HOST-04
**Success Criteria** (what must be TRUE):
  1. Staff can open `http://kiosk.rp:3300/kiosk` in a browser on James's machine and reach the staff kiosk terminal
  2. After a full server reboot, the kiosk is accessible at `http://kiosk.rp:3300/kiosk` within 60 seconds — no manual start needed
  3. Server IP address remains .23 across router restarts and lease renewals (DHCP reservation confirmed in router admin)
  4. The kiosk runs from a production Next.js build (`next build` output), not the development server
**Plans**: TBD

### Phase 8: Pod Lock Screen Hardening
**Goal**: Pod lock screens never show a browser error page — pods display a branded waiting state on startup and recover gracefully when rc-agent restarts
**Depends on**: Phase 7
**Requirements**: LOCK-01, LOCK-02, LOCK-03
**Success Criteria** (what must be TRUE):
  1. On pod reboot, the Edge kiosk window shows the branded lock screen (never "Site cannot be reached") within 10 seconds of the desktop appearing
  2. When rc-agent is not yet running, the pod screen shows a branded "Connecting..." page — no blank window, no browser error
  3. When rc-agent crashes and restarts mid-session, the pod screen automatically recovers to the lock screen within 30 seconds — no staff intervention required
**Plans**: TBD

### Phase 9: Edge Browser Hardening
**Goal**: Edge on all 8 pods is locked to its current version and configured so that auto-updates, startup boost, and background mode cannot break kiosk behavior
**Depends on**: Phase 8
**Requirements**: EDGE-01, EDGE-02, EDGE-03
**Success Criteria** (what must be TRUE):
  1. The EdgeUpdate and EdgeUpdateM services are stopped and disabled on all 8 pods — confirmed via `sc query EdgeUpdate` on each pod
  2. `StartupBoostEnabled` is set to 0 in the registry on all 8 pods — confirmed via registry query
  3. `BackgroundModeEnabled` is set to 0 in the registry on all 8 pods — confirmed via registry query
**Plans**: TBD

### Phase 10: Staff Dashboard Controls
**Goal**: Staff can manage all 8 pods from the kiosk dashboard without touching a keyboard on the pod — power cycling, rebooting, waking, and toggling lockdown are all one-click operations
**Depends on**: Phase 7
**Requirements**: KIOSK-01, KIOSK-02, PWR-01, PWR-02, PWR-03, PWR-04, PWR-05, PWR-06
**Success Criteria** (what must be TRUE):
  1. Staff can toggle full lockdown (taskbar hidden, Win key blocked, Edge kiosk mode) on or off for any individual pod from the staff kiosk dashboard
  2. Staff can lock all 8 pods at once (venue opening/closing) and unlock all 8 pods at once from a single action in the dashboard
  3. Staff can shut down, restart, or wake any individual pod remotely from the dashboard — and confirm the action took effect by seeing the pod status change
  4. Staff can shut down, restart, or wake all 8 pods simultaneously from the dashboard with a single action
**Plans**: TBD

### Phase 11: Customer Experience Polish
**Goal**: Customers see Racing Point branding at every transition — before a session, during a session, and after — and session results remain on screen so customers can review their performance
**Depends on**: Phase 8
**Requirements**: BRAND-01, BRAND-02, BRAND-03, SESS-01, SESS-02, SESS-03
**Success Criteria** (what must be TRUE):
  1. The lock screen displays the Racing Point logo prominently — no generic Windows screen visible to customers
  2. Staff can set a wallpaper (static or dynamic) for the blanking/lock screen from the kiosk dashboard — the change is visible on the pod within 10 seconds
  3. A branded Racing Point loading screen is shown on the pod between session start and game launch — no desktop or game loading screen visible to the customer
  4. After each session ends, the pod displays the customer's lap times, top speed, best lap, and race position — and the results remain on screen until staff or customer initiates a new session
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 6 → 7 → 8 → 9 → 10 → 11

Note: Phase 10 depends on Phase 7 (not Phase 9) — it requires the stable server URL but not the Edge hardening. Phase 11 depends on Phase 8 (not Phase 10) — it requires lock screen infrastructure. Both can proceed after their respective prerequisites.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. State Wiring & Config Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 2. Watchdog Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 3. WebSocket Resilience | v1.0 | 3/3 | Complete | 2026-03-13 |
| 4. Deployment Pipeline Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 5. Blanking Screen Protocol | v1.0 | 3/3 | Complete | 2026-03-13 |
| 6. Diagnosis | v2.0 | 2/2 | Complete | 2026-03-13 |
| 7. Server-Side Pinning | v2.0 | 0/TBD | Not started | - |
| 8. Pod Lock Screen Hardening | v2.0 | 0/TBD | Not started | - |
| 9. Edge Browser Hardening | v2.0 | 0/TBD | Not started | - |
| 10. Staff Dashboard Controls | v2.0 | 0/TBD | Not started | - |
| 11. Customer Experience Polish | v2.0 | 0/TBD | Not started | - |
