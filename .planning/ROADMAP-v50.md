# ROADMAP v50.0 — rc-agent-mobile

**Milestone:** v50.0 rc-agent-mobile — Reception Automation Hub
**Created:** 2026-04-18
**Phase range:** 409–424 (16 phases, concrete numbers assigned at kickoff)
**Source spec:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
**Requirements:** `.planning/REQUIREMENTS-v50.md`

## Goal

Ship a Kotlin Android agent that automates reception-workflow apps (Zomato Partner, HyperPure, Blinkit, future cardboard vendor) via Accessibility Service on 1× Tab Plus + 1× M07. Future-proof architecture — pluggable drivers, hot-reloadable selectors, remote push, feature flags, humanize layer, audit log.

## Starting phase number

Will be determined at kickoff (`/gsd:plan-phase` for phase 1). Suggested: **409** (after v52.0 range 393-408). Actual number assigned when v52.0 completes or at explicit user direction.

## Phase summary

| # | Phase | Goal | Requirements | Success Criteria |
|---|-------|------|--------------|------------------|
| 1 | Kotlin scaffold + HTTP + registration | Agent installs and registers with comms-link | AGENT-01..08 | 4 |
| 2 | Accessibility Service foundation | Agent can read screens and dispatch tap/swipe/text on both devices | ACCESS-01..05 | 4 |
| 3 | Bootstrap install + first-run UX | Non-technical staff can install + enable in < 5min | INSTALL-01..03 | 3 |
| 4 | Driver framework + capability registry | Drivers are plugins; device declares capabilities | DRIVER-01..05, CAPREG-01..04 | 5 |
| 5 | Selector DSL + hot-reload | YAML selectors hot-reload within 10s, versioned per app version | SELECTOR-01..06 | 4 |
| 6 | Credential abstraction | PersistentSession ships; OtpFlow/OAuth slots defined | CRED-01..04 | 3 |
| 7 | Humanize layer + audit log | All UI actions humanized and logged with screenshot hash | HUMANIZE-01..04, AUDIT-01..04 | 4 |
| 8 | Feature flag system | Per-device + per-driver toggles push-sync within 10s | FLAG-01..04 | 3 |
| 9 | Zomato Partner driver (P1) | Auto-accept/reject/mark-ready orders via Accessibility; WhatsApp/Discord forwarding | ZOMATO-01..06 | 5 |
| 10 | HyperPure driver (P2) | Bulk supply reorder from Core inventory trigger | HYPER-01..05 | 4 |
| 11 | Blinkit driver (P3) | Emergency top-up from staff trigger | BLINK-01..04 | 3 |
| 12 | Cardboard vendor driver (P4, deferred) | Drop-in when vendor app is identified | CARDBOARD-01..02 | 2 |
| 13 | Admin dashboard reception view | Unified orders/deliveries/status view in admin portal | ADMIN-01..03 | 3 |
| 14 | Feature flag + capability UI | Admin can toggle drivers and view device capabilities | ADMIN-04, FLAG-01..04 | 3 |
| 15 | Selector-map remote push UI | Admin uploads signed YAML, targets devices, rolls back on failure | ADMIN-05, SELECTOR-04 | 3 |
| 16 | E2E drills + ToS playbook | Agent-recovery, selector-miss, Zomato end-to-end drills; ToS incident playbook | E2E-01..04, ADMIN-06 | 4 |

**Total:** 16 phases | 54 requirements mapped | All covered ✓

## Phase details

### Phase 1: Kotlin scaffold + HTTP + registration
**Goal:** Agent installs on Tab Plus + M07, runs Foreground Service, exposes local HTTP endpoints, registers with comms-link, sends heartbeat, survives reboot.
**Requirements:** AGENT-01, AGENT-02, AGENT-03, AGENT-04, AGENT-05, AGENT-06, AGENT-07, AGENT-08
**Success criteria:**
1. Both devices show up in `/fleet/health` within 30s of device boot
2. Killing the agent process triggers Foreground Service auto-restart within 10s
3. Device reboot → agent re-registers without human action
4. Protocol-version negotiation rejects v2 messages gracefully on v1 agent
**Dependencies:** None (first phase)

### Phase 2: Accessibility Service foundation
**Goal:** Agent reads screen-tree and dispatches tap/swipe/text on any foreground app.
**Requirements:** ACCESS-01..05
**Success criteria:**
1. `/screen/tree` endpoint returns full node hierarchy of foreground app in < 500ms
2. `POST /ui/tap` with resource-id hits the target element with ≥ 95% success on test harness
3. Agent refuses actions with 503 when Accessibility is disabled, with human-readable message
4. First-run setup opens Settings page and waits for toggle confirmation
**Dependencies:** Phase 1

### Phase 3: Bootstrap install + first-run UX
**Goal:** Non-technical staff installs agent via MTP sideload + Files app, completes first-run permissions checklist in < 5min.
**Requirements:** INSTALL-01..03
**Success criteria:**
1. Staff can install agent with printed 5-step guide (no James involvement)
2. First-run Activity guides through Accessibility + overlay + install-unknown-apps + battery-optimization-off in one screen
3. Agent self-update accepts APK from comms-link with single user-confirmation tap
**Dependencies:** Phase 1

### Phase 4: Driver framework + capability registry
**Goal:** Drivers are plugins registered via manifest; device declares supported driver types; driver failures isolated.
**Requirements:** DRIVER-01..05, CAPREG-01..04
**Success criteria:**
1. Adding a new driver = drop a module dir + manifest entry; zero core-agent code changes
2. Device registration payload includes capability list; visible in admin
3. Crashing a driver does not kill the agent or sibling drivers (isolation test)
4. Lifecycle hooks fire deterministically — install on enable, onAppUpdate on package change, healthCheck every 5min, uninstall on disable
5. Manifest `supported_device_types` blocks installing a tablet-only driver on a phone
**Dependencies:** Phase 2

### Phase 5: Selector DSL + hot-reload
**Goal:** YAML selectors are the source of truth; hot-reload within 10s; versioned per app version; fallback chain.
**Requirements:** SELECTOR-01..06
**Success criteria:**
1. Editing `selectors.yaml` on device takes effect in < 10s without agent restart
2. App version change triggers matching selector-map selection; old version remains as fallback
3. Selector miss emits event with screenshot hash + last-known-good + app version
4. James can capture current-screen YAML stub via debug mode — single command produces commit-ready file
**Dependencies:** Phase 4

### Phase 6: Credential abstraction
**Goal:** `CredentialStrategy` interface with `PersistentSession` impl; OTP/OAuth slots ready.
**Requirements:** CRED-01..04
**Success criteria:**
1. Driver declares credential strategy in manifest; agent enforces at runtime
2. `PersistentSession` detects session expiry within one health-check cycle (≤ 5min)
3. Adding a new strategy class + manifest entry does NOT require core code change
**Dependencies:** Phase 4

### Phase 7: Humanize layer + audit log
**Goal:** All UI actions pass through humanize interceptor (delays, business-hours, rate limit) and emit audit events with screenshot hash.
**Requirements:** HUMANIZE-01..04, AUDIT-01..04
**Success criteria:**
1. Every tap/swipe/text event logged with timestamp + driver + selector + outcome + screenshot hash
2. Business-hours gate configurable; outside window, driver queues or drops per policy
3. Rate limiter enforces per-app ceiling; excess actions queue or drop
4. Logs rotate locally at 500MB cap; hourly shipping to server succeeds via comms-link
**Dependencies:** Phase 4

### Phase 8: Feature flag system
**Goal:** Server-side per-device + per-driver flags push-sync to agent within 10s; kill-switch halts all drivers fleet-wide.
**Requirements:** FLAG-01..04
**Success criteria:**
1. Toggling `enable_zomato_on_tab_plus` fires driver `install()` or `uninstall()` within 10s
2. Global `pause_all_drivers` flag halts every driver on every device within 10s
3. Flag changes audit-logged with actor + timestamp
**Dependencies:** Phase 4

### Phase 9: Zomato Partner driver (P1)
**Goal:** Auto-accept (capacity-gated) / auto-reject / mark-ready Zomato orders; WhatsApp/Discord forwarding; session-expiry alerting.
**Requirements:** ZOMATO-01..06
**Success criteria:**
1. Incoming order detected within 10s; auto-accept decision made within 30s
2. Capacity query to POS rc-agent honored — no auto-accept when `can_accept: false`
3. Order details forwarded to WhatsApp + Discord via existing comms-link channels
4. `mark ready` trigger from admin dashboard or POS kitchen screen completes in UI within 15s
5. Session-expired state pauses driver + alerts staff; driver does not fail silently
**Dependencies:** Phases 5, 6, 7, 8

### Phase 10: HyperPure driver (P2)
**Goal:** Accept bulk order from Core inventory trigger, navigate HyperPure app, check out, log confirmation.
**Requirements:** HYPER-01..05
**Success criteria:**
1. Bulk order manifest (SKU + quantity list) from Core executes end-to-end in HyperPure app
2. Out-of-stock SKUs skipped + logged + alerted to staff
3. Order confirmation number + delivery window logged back to Core
4. Business-hours + max-orders-per-day limits enforced
**Dependencies:** Phase 9

### Phase 11: Blinkit driver (P3)
**Goal:** Accept emergency top-up from staff trigger; navigate Blinkit; log order + ETA.
**Requirements:** BLINK-01..04
**Success criteria:**
1. Staff-triggered top-up executes end-to-end in Blinkit app
2. Order number + ETA logged back to Core
3. Humanize + rate limits enforced
**Dependencies:** Phase 9

### Phase 12: Cardboard vendor driver (P4, deferred)
**Goal:** Drop-in driver when vendor app is identified (Q2 unresolved).
**Requirements:** CARDBOARD-01..02
**Success criteria:**
1. Phase auto-skips ship gate if Q2 remains unresolved at milestone close
2. Driver framework pluggability verified by this deferred slot — adding the driver when ready requires no core changes
**Dependencies:** Phase 9, vendor app identified

### Phase 13: Admin dashboard reception view
**Goal:** Unified reception page in admin dashboard showing orders/deliveries/device status.
**Requirements:** ADMIN-01, ADMIN-02, ADMIN-03
**Success criteria:**
1. Reception page shows pending Zomato + HyperPure + Blinkit state with real-time updates
2. Manual action buttons (accept/reject/cancel/retry) fire through comms-link to the right device
3. Device status panel shows heartbeat, agent version, enabled drivers, last action per driver
**Dependencies:** Phase 9

### Phase 14: Feature flag + capability UI
**Goal:** Admin toggles driver enablement and views per-device capability list.
**Requirements:** ADMIN-04, FLAG-01..04
**Success criteria:**
1. Admin can toggle `enable_<driver>_on_<device>` flags with audit trail
2. Capability list viewable in admin dashboard per device
3. Toggle change visible on device within 10s
**Dependencies:** Phases 8, 13

### Phase 15: Selector-map remote push UI
**Goal:** Admin uploads signed selector YAML, targets devices, rolls back on failure.
**Requirements:** ADMIN-05, SELECTOR-04
**Success criteria:**
1. Upload + target + push flow completes in < 2min
2. Signature verification rejects unsigned or tampered patches
3. Rollback restores previous selector-map within 10s
**Dependencies:** Phases 5, 13

### Phase 16: E2E drills + ToS playbook
**Goal:** All failure paths drilled end-to-end; ToS-incident runbook documented.
**Requirements:** E2E-01..04, ADMIN-06
**Success criteria:**
1. Zomato end-to-end drill — simulated order → auto-accept → kitchen → ready → WhatsApp — passes with all events visible in admin
2. Agent-recovery drill — kill agent and reboot device, verify full auto-recovery within 2min
3. Selector-miss recovery drill — break a selector, verify alert, James pushes fix, recovery in < 5min
4. ToS-incident playbook doc reviewed and signed off by Uday
**Dependencies:** Phases 9, 10, 11, 13

---

## Dependency graph

```
1 → 2 → 3
1 → 2 → 4 → 5 → (selectors)
            → 6 (credentials)
            → 7 (humanize + audit)
            → 8 (flags)
4,5,6,7,8 → 9 (Zomato)
9 → 10 (HyperPure)
9 → 11 (Blinkit)
9 → 12 (cardboard, deferred)
9 → 13 (admin view)
8,13 → 14 (flag UI)
5,13 → 15 (remote push UI)
9,10,11,13 → 16 (E2E drills)
```

**Parallelizable:** Phases 5, 6, 7, 8 can run in parallel after phase 4 completes. Phases 10, 11 can run in parallel after phase 9. Phases 14, 15 can run in parallel after phases 8, 13, 5 are done.

## Milestone ship gate

- [ ] All 54 requirements checked
- [ ] UI-SPEC.md + UI-REVIEW.md for phases 13, 14, 15 (frontend subagent gates)
- [ ] nyquist-audit for phases 1, 4, 6, 7, 8, 9 (business logic)
- [ ] integration-checker across phases 9–13 (cross-phase flows)
- [ ] MMA audit — ToS-risky cross-system flows (Zomato, HyperPure, Blinkit)
- [ ] E2E drills in phase 16 passed
- [ ] Deploy manifest signed off per DMP
