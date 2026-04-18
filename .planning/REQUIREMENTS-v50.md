# REQUIREMENTS v50.0 — rc-agent-mobile (Reception Automation Hub)

**Milestone:** v50.0 rc-agent-mobile — Reception Automation Hub
**Started:** 2026-04-18 (Planning phase)
**Status:** KICKOFF-READY — all open questions locked, awaiting phase 1 execution
**Source spec:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`

## Goal

Build a Kotlin Android agent (`rc-agent-mobile`) that turns 1× Lenovo Tab Plus (TB-351FU) + 1× Samsung Galaxy M07 into a reception automation hub. The agent drives third-party Android apps (Zomato Partner, HyperPure, Blinkit, cardboard vendor) via Accessibility Service — registering with the existing comms-link relay like Windows rc-agent on pods/POS. **Future-proofing is a first-class non-negotiable** — every extensibility feature below must ship in v50.0.

## Locked architectural decisions (2026-04-18)

| Q | Decision |
|---|----------|
| Priority | **Zomato first**, then HyperPure, then Blinkit, cardboard deferred |
| Fleet | **1× Tab Plus + 1× M07** (no provisioning phase required) |
| Credentials | **CredentialStrategy pattern** — `PersistentSession` ships v50, `OtpFlow`/`OAuth` are future-compatible slots |
| Selector maintainer | **James owns** — YAML files in `app-drivers/<app>/selectors.yaml`, git-versioned, hot-reloadable |
| Language | **Kotlin** (Android-native). Shared JSON protocol spec with Rust rc-agent, NOT shared code |

## Out of scope (v50.0)

- In-store PWA cafe ordering (`app.racingpoint.cloud` — customer orders food, pays from credits) → SEPARATE MILESTONE (backlog)
- OTP/2FA credential flow → future phase
- Multi-language UI (English only)
- iOS (Android only)

---

## v1 Requirements

### Agent Core (AGENT-*)

- [ ] **AGENT-01**: Kotlin Android app builds with Gradle and installs via ADB to Tab Plus + M07
- [ ] **AGENT-02**: Agent runs a local HTTP server on port 8090 exposing `/health`, `/build_id`, `/heartbeat`, `/capability` endpoints
- [ ] **AGENT-03**: Agent registers with comms-link relay (James .27 or cloud Bono VPS) on startup with device identifier, capability list, and agent version
- [ ] **AGENT-04**: Agent sends heartbeat every 30s to comms-link and survives relay restarts (reconnect with exponential backoff, jittered)
- [ ] **AGENT-05**: Agent has a Foreground Service with persistent notification so Android OS does not kill it in background
- [ ] **AGENT-06**: Agent survives device reboot (auto-starts via BOOT_COMPLETED broadcast receiver) and registers to relay without manual intervention
- [ ] **AGENT-07**: Agent logs every lifecycle event (start, stop, crash, reconnect) locally in rotating log file capped at 50MB per device
- [ ] **AGENT-08**: Protocol version negotiation on registration — agent declares `protocol_version: 1`, relay accepts or rejects; forward-compat: unknown fields in messages are ignored

### Accessibility Service (ACCESS-*)

- [ ] **ACCESS-01**: Accessibility Service runs with `TYPE_WINDOW_CONTENT_CHANGED` + `TYPE_VIEW_CLICKED` + `TYPE_VIEW_FOCUSED` events enabled
- [ ] **ACCESS-02**: Screen tree reader returns full `AccessibilityNodeInfo` hierarchy of the foreground app on request
- [ ] **ACCESS-03**: Tap/swipe/text-input primitives dispatchable by (resource-id | content-description | text | xpath) with 100ms default retry on miss
- [ ] **ACCESS-04**: Staff can enable Accessibility Service via first-run guided setup (agent detects and opens the Android Settings page, waits for toggle)
- [ ] **ACCESS-05**: Agent refuses to operate (returns 503 on HTTP) if Accessibility Service is not enabled; visible warning on the persistent notification

### Bootstrap & Install (INSTALL-*)

- [ ] **INSTALL-01**: Agent APK can be sideloaded via MTP file transfer from Windows to device, then installed via Android Files app with "install unknown apps" permission
- [ ] **INSTALL-02**: First-run checklist guides staff through required permissions (Accessibility, "display over other apps", "install unknown apps", disable battery optimization) in a single Activity screen
- [ ] **INSTALL-03**: Agent self-updates from comms-link-hosted APK when a newer version is announced (user-confirmed — Android forbids silent install without device owner)

### Pluggable Driver Framework (DRIVER-*)

- [ ] **DRIVER-01**: Driver is a Kotlin class implementing `AppDriver` interface — `install()`, `onAppUpdate()`, `healthCheck()`, `uninstall()` lifecycle + typed action methods
- [ ] **DRIVER-02**: Drivers register via manifest file (`drivers.json` bundled in APK + remotely pushable) — no hardcoded if/else in agent core
- [ ] **DRIVER-03**: New driver = new module directory + manifest entry; adding a driver does not require core agent code changes
- [ ] **DRIVER-04**: Driver lifecycle hooks run deterministically — `install` at first enable, `onAppUpdate` when package version changes, `healthCheck` every 5min, `uninstall` on feature-flag off
- [ ] **DRIVER-05**: Driver failures are isolated — a crashing driver does not bring down the agent or other drivers (each runs in a coroutine scope with exception handler)

### Selector DSL & Hot-Reload (SELECTOR-*)

- [ ] **SELECTOR-01**: Selectors defined in YAML at `app-drivers/<app>/selectors.yaml`, structured as `screen → element → [selector strategy]` with fallback chain
- [ ] **SELECTOR-02**: Selectors are versioned per target-app version (e.g., `zomato-partner/v3.14.2/selectors.yaml`); agent picks the matching version automatically, falls back to previous, fails loudly if nothing matches
- [ ] **SELECTOR-03**: Selector hot-reload — editing the YAML in place (via remote push or local ADB) takes effect within 10s without agent restart
- [ ] **SELECTOR-04**: Remote selector push — admin dashboard can ship a signed selector-map patch to specific devices via comms-link; patches are versioned with rollback support
- [ ] **SELECTOR-05**: Selector failure emits `SelectorMissEvent` with screenshot hash + last-known-good selector + app version to the audit log
- [ ] **SELECTOR-06**: James can author selectors via a debug mode in the agent that captures the current screen's AccessibilityNodeInfo tree to a YAML stub ready for commit

### Credential Abstraction (CRED-*)

- [ ] **CRED-01**: `CredentialStrategy` interface defines `login()`, `isSessionValid()`, `refresh()`, `logout()` contract — each driver declares which strategy it requires
- [ ] **CRED-02**: `PersistentSession` strategy implemented — human logs in once, agent verifies session-cookie presence via Accessibility checks before each action
- [ ] **CRED-03**: `OtpFlow` and `OAuth` strategy slots defined as interfaces (no implementation in v50.0) — swapping requires no agent-core changes, only new strategy class + driver manifest update
- [ ] **CRED-04**: Session-expiry detection — if `isSessionValid()` returns false, agent emits `SessionExpiredEvent`, pauses the driver, and notifies staff via admin dashboard + WhatsApp

### Capability Registry (CAPREG-*)

- [ ] **CAPREG-01**: Each device declares its driver capability list in its registration payload — Tab Plus might run HyperPure + cardboard; M07 might run Zomato only
- [ ] **CAPREG-02**: Registry is queryable from admin dashboard; staff see which device runs which drivers
- [ ] **CAPREG-03**: Manifest declares `supported_device_types: ["tablet", "phone"]` per driver; agent refuses to install a driver on an unsupported device type
- [ ] **CAPREG-04**: Multi-device-type readiness — architecture supports future `smart_display`, `ps5_tablet`, `kiosk_phone` types without schema migration

### Feature Flags (FLAG-*)

- [ ] **FLAG-01**: Per-device + per-driver feature flags stored server-side, pushed to agent via comms-link on change
- [ ] **FLAG-02**: Admin dashboard UI toggles flags (e.g., `enable_zomato_on_tab_plus=true`, `enable_hyperpure_on_m07=false`) with audit trail
- [ ] **FLAG-03**: Driver enable/disable is instant — toggling a flag triggers the driver's `install()` or `uninstall()` lifecycle hook within 10s
- [ ] **FLAG-04**: Kill-switch — a global `pause_all_drivers` flag halts every driver on every device within 10s (ToS incident response)

### Humanize Layer & Rate Limits (HUMANIZE-*)

- [ ] **HUMANIZE-01**: Shared `HumanizeInterceptor` injects randomized delay `N(mean_ms, stddev_ms)` between every two UI actions; per-action-type config
- [ ] **HUMANIZE-02**: Business-hours gate — configurable window (e.g., 08:00–23:00 IST); outside window, drivers queue actions or drop per policy
- [ ] **HUMANIZE-03**: Rate limiter — max N actions per minute per app (configurable); excess actions queue or drop
- [ ] **HUMANIZE-04**: Humanize config is per-driver + overridable per-device; hot-reloadable without restart

### Audit Log (AUDIT-*)

- [ ] **AUDIT-01**: Every UI action (tap, swipe, text-input) logged with: timestamp, driver, screen, selector used, selector match confidence, screenshot hash, action outcome
- [ ] **AUDIT-02**: Logs stored locally in rolling file (max 500MB), shipped to server every hour via comms-link
- [ ] **AUDIT-03**: Admin dashboard exposes a log viewer filterable by device + driver + time range
- [ ] **AUDIT-04**: Failed selectors logged with screenshot hash so James can diff and repair the YAML

### Zomato Partner Driver (ZOMATO-*)

- [ ] **ZOMATO-01**: Driver detects an incoming Zomato Partner order (new-order notification or dashboard update) within 10s
- [ ] **ZOMATO-02**: Driver auto-accepts orders when kitchen-capacity query (to POS rc-agent `/kitchen/capacity`) returns `can_accept: true`
- [ ] **ZOMATO-03**: Driver auto-rejects orders when capacity is exceeded, with a configurable grace window before rejecting
- [ ] **ZOMATO-04**: Driver marks orders "ready" when staff triggers via admin dashboard or POS kitchen screen
- [ ] **ZOMATO-05**: Driver forwards order details (items, total, customer name masked) to WhatsApp + Discord bots via existing comms-link channels
- [ ] **ZOMATO-06**: Driver uses `PersistentSession` credential strategy; on session-expired, emits alert and stops taking new orders

### HyperPure Driver (HYPER-*)

- [ ] **HYPER-01**: Driver accepts bulk order manifests from RaceControl Core (inventory depletion trigger) — list of SKU + quantity
- [ ] **HYPER-02**: Driver navigates HyperPure Android app, adds each SKU to cart, proceeds through checkout, confirms order
- [ ] **HYPER-03**: Driver logs order confirmation number + scheduled delivery window back to Core
- [ ] **HYPER-04**: Driver respects business-hours gate and max-orders-per-day rate limit (default: 3)
- [ ] **HYPER-05**: Driver handles "out of stock" with a deterministic fallback: skip + log + alert staff

### Blinkit Driver (BLINK-*)

- [ ] **BLINK-01**: Driver accepts emergency top-up orders (SKU + quantity) from staff via admin dashboard or Core inventory alerts
- [ ] **BLINK-02**: Driver navigates Blinkit app, adds items, checks out, confirms
- [ ] **BLINK-03**: Driver logs order number + ETA back to Core
- [ ] **BLINK-04**: Driver respects humanize + rate limits

### Cardboard Vendor Driver (CARDBOARD-*, deferred)

- [ ] **CARDBOARD-01**: Driver is defined as a phase-12 stub — no-op until vendor app is identified (open question Q2)
- [ ] **CARDBOARD-02**: Phase auto-skips milestone-close gate if Q2 remains unresolved at ship time (driver framework is pluggable — this is a drop-in when ready)

### Admin Dashboard Reception View (ADMIN-*)

- [ ] **ADMIN-01**: Reception page in admin dashboard shows unified view: pending Zomato orders + HyperPure deliveries + Blinkit status
- [ ] **ADMIN-02**: Staff can trigger manual actions (accept/reject Zomato, cancel HyperPure, retry Blinkit) from the page
- [ ] **ADMIN-03**: Device status panel shows heartbeat, agent version, enabled drivers, last action per driver for both Tab Plus + M07
- [ ] **ADMIN-04**: Feature flag UI — admin toggles `enable_<driver>_on_<device>` flags with audit trail
- [ ] **ADMIN-05**: Selector-map push UI — admin uploads signed selector YAML, targets devices, rolls back on failure
- [ ] **ADMIN-06**: Log viewer with filter (device, driver, time) and screenshot preview

### E2E + ToS Incident Response (E2E-*)

- [ ] **E2E-01**: Documented ToS-incident playbook — what to do when Zomato/HyperPure/Blinkit account shows warning (kill-switch, fall back to manual, contact support)
- [ ] **E2E-02**: End-to-end drill — simulate Zomato order → auto-accept → kitchen → ready → WhatsApp push, all visible in admin dashboard
- [ ] **E2E-03**: Agent-recovery drill — kill the agent process, verify it auto-restarts via Foreground Service + device reboots and re-registers
- [ ] **E2E-04**: Selector-miss recovery drill — intentionally break a selector, verify agent emits `SelectorMissEvent`, admin sees alert, James can push a fix within 5min

---

## Future Requirements (post-v50.0)

- **OTP/SMS credential flow** (CRED-OTP-*) — for apps that force 2FA re-login
- **OAuth credential flow** (CRED-OAUTH-*)
- **Multi-device-type expansion** — smart display, PS5-attached tablet, kiosk phone
- **Biometric/passkey strategy**
- **iOS agent port**
- **Cardboard vendor driver** (awaits vendor app identification)

## Out of Scope (permanent)

- Silent APK install (Android policy)
- Modifying secure system settings (dev options, USB debugging)
- Bypassing Play Protect scanner
- In-store PWA cafe ordering — separate milestone

---

## Traceability (filled by ROADMAP-v50.md)

| REQ-ID | Phase | Plan | Status |
|--------|-------|------|--------|
| AGENT-01..08 | Phase 1 (scaffold + HTTP + registration + heartbeat) | TBD | Pending |
| ACCESS-01..05 | Phase 2 (Accessibility foundation) | TBD | Pending |
| INSTALL-01..03 | Phase 3 (Bootstrap install) | TBD | Pending |
| DRIVER-01..05, CAPREG-01..04 | Phase 4 (Driver framework + capability registry) | TBD | Pending |
| SELECTOR-01..06 | Phase 5 (Selector DSL + hot-reload) + Phase 15 (Remote push UI) | TBD | Pending |
| CRED-01..04 | Phase 6 (Credential abstraction) | TBD | Pending |
| HUMANIZE-01..04, AUDIT-01..04 | Phase 7 (Humanize + audit log) | TBD | Pending |
| FLAG-01..04 | Phase 8 (Feature flags) + Phase 14 (Admin UI) | TBD | Pending |
| ZOMATO-01..06 | Phase 9 (Zomato driver) | TBD | Pending |
| HYPER-01..05 | Phase 10 (HyperPure driver) | TBD | Pending |
| BLINK-01..04 | Phase 11 (Blinkit driver) | TBD | Pending |
| CARDBOARD-01..02 | Phase 12 (deferred) | TBD | Pending |
| ADMIN-01..06 | Phase 13 (Admin reception view) | TBD | Pending |
| E2E-01..04 | Phase 16 (E2E drills + ToS playbook) | TBD | Pending |

**Total:** 54 requirements across 16 phases. All requirements mapped. 0 orphaned.
