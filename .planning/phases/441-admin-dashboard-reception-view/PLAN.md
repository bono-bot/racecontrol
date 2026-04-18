---
phase: 441-admin-dashboard-reception-view
phase_number: 441
milestone: v50.0 rc-agent-mobile
name: "Admin Dashboard Reception View"
status: ready-to-execute
goal: >
  Unified reception page in racingpoint-admin Next.js dashboard (/mobile/reception)
  that shows, in real time: pending Zomato orders (from Phase 437 driver), in-flight
  HyperPure deliveries (Phase 438), Blinkit top-up status (Phase 439), and per-device
  status for Tab Plus + M07 (heartbeat age, agent build_id, enabled drivers,
  last-action-per-driver timestamp/outcome). Staff authenticate with existing staff
  JWT and can trigger manual actions — accept/reject Zomato, cancel HyperPure, retry
  Blinkit, mark-ready — each of which flows racingpoint-admin -> racecontrol
  POST /api/v1/mobile/drivers/:device_id/:driver_id/action -> comms-link relay ->
  Kotlin agent -> Accessibility tap. The page includes an audit log viewer filterable
  by device + driver + time range (paginated, server-side) reading real storage from
  the mobile_audit_events SQLite table (this phase upgrades Phase 435's stub
  POST /api/v1/mobile-audit/ingest into persistent storage + query endpoints).
  Frontend uses Next.js 16 + React 19 + Tailwind 4 + SWR polling (existing stack),
  applies Racing Red #E10600 + Asphalt Black #1A1A1A brand tokens, and respects
  basePath conventions (redirects, rewrites) proven in the existing (dashboard)
  route group. Real-time updates use the existing WebSocket channel pattern
  already serving other dashboard pages (falls back to 2s SWR polling if WS absent
  since the dashboard does not currently have a WS hook — discovery task 441-02A
  confirms). Phase MUST pass the UI-SPEC (gsd-ui-researcher) gate BEFORE any
  component is authored and the UI-REVIEW (gsd-ui-auditor) gate BEFORE ship.
requirements: [ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-06]
depends_on: [437]                  # Zomato driver is first real data source; 438/439 not yet built -> panels degrade to "no driver active" state gracefully
wave: 6                            # 437 is wave 5 (depends on 432/435/436); 441 consumes 437 output
plan_count: 10
plans:
  - 441-01-PLAN: UI-SPEC via gsd-ui-researcher subagent (PRE-REQUIREMENT for 441-02+)
  - 441-02-PLAN: Reception page scaffold + WS/SWR realtime strategy + DeviceStatusPanel
  - 441-03-PLAN: Zomato orders panel (pending/accepted list + manual accept/reject/mark-ready)
  - 441-04-PLAN: HyperPure deliveries panel (in-flight + confirmation + cancel)
  - 441-05-PLAN: Blinkit status panel (in-flight top-ups + retry)
  - 441-06-PLAN: Audit log viewer (server-paginated filter by device+driver+time + screenshot preview)
  - 441-07-PLAN: Server-side audit storage (SQLite mobile_audit_events table + migration, upgrade Phase 435 stub)
  - 441-08-PLAN: Server-side manual-action endpoint POST /api/v1/mobile/drivers/:device_id/:driver_id/action + comms-link dispatch
  - 441-09-PLAN: UI-REVIEW via gsd-ui-auditor subagent (6-pillar audit, MANDATORY gate)
  - 441-10-PLAN: Playwright E2E drill (mock driver events -> UI -> action round-trip)
autonomous: false                  # 441-01 (UI-SPEC) and 441-09 (UI-REVIEW) are subagent-gated checkpoints; 441-10 has physical-device verify; 441-02 has a human-verify checkpoint for visual brand
files_modified:
  # Admin frontend — new /mobile/reception route group
  - racingpoint-admin/src/app/(dashboard)/mobile/layout.tsx                       # sidebar entry
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/page.tsx              # main page
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/DeviceStatusPanel.tsx # Tab Plus + M07 cards
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/ZomatoPanel.tsx
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/HyperPurePanel.tsx
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/BlinkitPanel.tsx
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/AuditLogViewer.tsx
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/ScreenshotDialog.tsx
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/useReceptionLive.ts   # WS or SWR hook
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/actions.ts            # server actions for manual triggers
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/types.ts              # shared TS types
  # Admin API library (new client wrappers matching existing src/lib/api/ pattern)
  - racingpoint-admin/src/lib/api/mobile.ts                                       # client for /api/v1/mobile/* endpoints
  # Server (racecontrol) — storage + query + manual action
  - crates/racecontrol/src/api/mobile_audit.rs                                    # UPGRADED from 435 stub: real storage + query
  - crates/racecontrol/src/api/mobile_reception.rs                                # NEW: live state + manual-action dispatch
  - crates/racecontrol/src/api/mod.rs                                             # route registration
  - crates/racecontrol/src/api/routes.rs                                          # route registration
  - crates/racecontrol/src/db/migrations/NNNN_mobile_audit_events.sql             # NEW migration (idempotent ALTER-safe)
  - crates/racecontrol/src/db/migrations.rs                                       # migration registration
  - crates/rc-common/src/mobile_types.rs                                          # NEW: shared types (MobileAuditRow, DeviceSnapshot, DriverSnapshot, ManualActionRequest)
  - crates/rc-common/src/lib.rs                                                   # re-export
  # Relay bridge (comms-link)
  - comms-link/james/mobile-action-dispatch.js                                    # NEW: forwards POST /mobile/drivers/:device_id/:driver_id/action to WS envelope
  # Shared protocol extension
  - rc-agent-mobile/docs/PROTOCOL.md                                              # appended: manual_action_request / manual_action_ack envelopes
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/ManualActionHandler.kt  # NEW: dispatches to right driver
  # Tests
  - racingpoint-admin/tests/mobile-reception.spec.ts                              # Playwright E2E
  - racingpoint-admin/src/app/(dashboard)/mobile/reception/__tests__/*.test.tsx   # RTL unit tests
  - crates/racecontrol/tests/mobile_audit_storage.rs                              # integration test
  - crates/racecontrol/tests/mobile_manual_action.rs                              # integration test
  # Docs
  - rc-agent-mobile/docs/AUDIT-LOG.md                                             # UPDATED: real storage section (supersedes stub note from Phase 435)
  - racingpoint-admin/docs/MOBILE-RECEPTION.md                                    # NEW: operator runbook
  # Phase artifacts
  - .planning/phases/441-admin-dashboard-reception-view/UI-SPEC.md                # authored by gsd-ui-researcher in 441-01
  - .planning/phases/441-admin-dashboard-reception-view/UI-REVIEW.md              # authored by gsd-ui-auditor in 441-09
  - .planning/phases/441-admin-dashboard-reception-view/SUMMARY.md

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [racecontrol]        # 441-07 + 441-08 add new endpoints + storage
  frontend_rebuild: [admin]         # /mobile/reception is a new route in racingpoint-admin
  config_change: >
    racecontrol.toml: new optional section [mobile_reception] with
      audit_retention_days = 30, max_page_size = 100, screenshot_storage = "inline_sha256_only".
    Omitted = hardcoded defaults.  No mandatory config change.
  db_migration: >
    mobile_audit_events table (primary storage for Phase 435 events).  Idempotent
    CREATE TABLE IF NOT EXISTS.  See 441-07 for full schema.  Includes indexes on
    (device_id, ts_ms), (driver_id, ts_ms), and (app_package, ts_ms).
  infrastructure: >
    comms-link relay (James .27:8765 AND Bono VPS:8765) must forward:
      - POST /api/v1/mobile/drivers/:device_id/:driver_id/action envelopes
        to the target Kotlin agent's WS connection
      - manual_action_ack envelopes BACK to racecontrol server
    Relay identity allowlist from Phase 429 already includes rcm-tab-plus, rcm-m07.
    New: mobile-action-dispatch.js Node module added to relay.
  data_files: >
    None net-new.  Existing mobile_audit JSONL ingest (Phase 435) now has real
    storage backing.  No external static assets added (icons via lucide-react
    already in package.json; Racing Red brand applied via Tailwind classes).
  bat_file: none
  cloud_parity:
    - racecontrol binary deploys to Bono VPS (DEPLOY PARITY rule — cloud/admin customers on racingpoint.cloud get identical /mobile/reception)
    - racingpoint-admin frontend rebuilds + deploys to venue .23:3201 AND Bono VPS :3201
    - comms-link cloud relay gets the same mobile-action-dispatch.js module
    - DB migration runs on BOTH venue racecontrol.db AND Bono VPS racecontrol DB
  targets:
    - server_23                     # venue racecontrol binary (new endpoints + migration)
    - bono_vps                      # cloud racecontrol + cloud admin
    - admin_23                      # venue admin frontend rebuild
    - admin_bono_vps                # cloud admin frontend rebuild
    - james_27                      # comms-link relay new module
    - tab_plus                      # APK reinstall (ManualActionHandler.kt new class)
    - m07                           # APK reinstall (same)
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  post_deploy_verification:
    - "curl -s http://192.168.31.23:8080/api/v1/mobile/reception/state returns 200 with devices, orders, deliveries arrays"
    - "curl -s http://192.168.31.23:8080/api/v1/mobile-audit/query?limit=5 returns 200 with rows array (may be empty on first deploy)"
    - "curl -s http://192.168.31.23:3201/kiosk/api/health/deep returns {healthy:true} (admin API proxy intact — per CLAUDE.md frontend deploy rule)"
    - "Visit /mobile/reception from a NON-server browser (James .27), verify WS or polling populates within 5s — per CLAUDE.md Frontend deploy rule"
    - "Trigger manual mark-ready on a test Zomato order, verify audit row appears within 30s and Tab Plus logcat shows manual_action_request received"
  rollback:
    - "Admin frontend: bash racingpoint-admin/scripts/admin-deploy.sh --rollback"
    - "Server: rename racecontrol-prev.exe + schtasks StartRCTemp (per CLAUDE.md deploy-server.sh auto-rollback)"
    - "DB migration: mobile_audit_events is additive — no DROP needed on rollback; old binary simply ignores the table"
    - "Relay: git checkout comms-link/james/mobile-action-dispatch.js + pm2 restart comms-link"
    - "APK: adb install -r /sdcard/Download/rc-agent-mobile-prev.apk on both devices"

# Subagent gates (per CLAUDE.md > Subagent Gates — frontend phase hard-gated)
gates:
  ui_researcher: required           # PRE-REQUIREMENT (plan 441-01). Authors UI-SPEC.md BEFORE any component code.
  ui_auditor: required              # POST-REQUIREMENT (plan 441-09). Authors UI-REVIEW.md BEFORE milestone ship.
  nyquist_auditor: required         # 441-07 (storage), 441-08 (manual action dispatch) are business logic with defined I/O.
  mma_audit: required               # Cross-system bridge: Admin(Next.js) -> racecontrol(Rust) -> comms-link(Node) -> Kotlin agent -> Accessibility. Dual reasoning modes REQUIRED per CLAUDE.md MMA rule.
  integration_checker: required     # Agent <-> server <-> admin <-> back-to-agent round-trip is cross-phase (429+435+437+441); integration-checker runs before milestone ship.
  codebase_mapper: skip             # Admin repo already in .planning/codebase/; Kotlin agent added to codebase map in Phase 429. No NEW top-level module.

risks_summary:
  - "Admin dashboard currently has no WebSocket hook (grep confirmed: only 2 files reference WebSocket, neither is a realtime dashboard pattern). Risk: live updates degrade to polling which may miss transient events. Mitigation: Plan 441-02A (discovery sub-task) enumerates the EXACT realtime mechanism the existing /fleet page uses. If WS exists, reuse it; if not, 2s SWR polling is acceptable for reception UX (orders arrive in tens of seconds, not sub-second)."
  - "WS reconnect during live demo: admin WS churn (CLAUDE.md 2026-04-03 incident — 800+ reconnects/min) invisibly broke the kiosk dashboard for 4 days. Mitigation: /mobile/reception useReceptionLive hook MUST log reconnect count + publish it to a visible connection-status pill (reuse existing src/components/ConnectionIndicator.tsx). Admin dashboard WS churn metric MUST remain < 10 conns/min (enforced in post-deploy verify)."
  - "Action-dispatch race when target device is offline: staff clicks Accept when Tab Plus WS is disconnected. Current PoE: without explicit handling, the relay silently drops the envelope. Mitigation (441-08): server validates device.ws_connected BEFORE enqueuing dispatch; if offline, returns HTTP 409 {error: 'device_offline', last_seen_secs_ago: N} with a staff-facing toast explaining the fleet state. Mandatory integration test."
  - "Screenshot preview XSS risk: AuditEvent screenshot_sha256 is a 64-hex SHA256, but if the audit-ingest endpoint ever accepts arbitrary text as sha, the admin viewer could render unescaped user-controlled input. Mitigation: 441-07 server-side VALIDATES sha256 shape (64 hex OR 'sha256:unavailable:<kebab>' sentinel) on INSERT; 441-06 renders via text-only Tailwind (no dangerouslySetInnerHTML anywhere)."
  - "Storage upgrade from Phase 435 stub is NON-TRIVIAL: the stub (POST /api/v1/mobile-audit/ingest) currently returns 200 and discards data. On 441 deploy, the endpoint starts writing rows — but the agent's hourly shipping client will re-ship events the server previously 'accepted but discarded'. Mitigation: agent ShippedCursor (435-07) only advances on 200. Since stub also returned 200, the cursor already advanced past historically-stubbed events = THOSE EVENTS ARE PERMANENTLY LOST (acceptable — v50.0 Phase 435-441 is pre-prod; no ToS-relevant data yet). Document the one-time lossy upgrade in MOBILE-RECEPTION.md."
  - "Manual-action endpoint auth: existing staff JWT covers POS/kitchen actions. Extending it to drive Android Accessibility needs explicit scope review. Decision (locked): use the same staff JWT (reception counter staff already authorized to take orders manually). Alternative (manager-only scope) deferred to FLAG-UI Phase 442 if needed."
  - "Tailwind 4 + Next.js 16 + React 19 combined is NEW territory (package.json confirms). Risk: SSR hydration mismatch around the realtime connection pill. Mitigation: follow the existing pattern from src/app/(dashboard)/fleet/page.tsx (Server Component for initial load, Client Component only for the live-updating panels). UI-SPEC gate (441-01) will call this out."
  - "Playwright E2E (441-10) requires driver event simulation. Playwright is already set up (package.json @playwright/test ^1.58.2 + playwright.config.ts present). Strategy: use a MOCK mode (env RC_MOCK_MOBILE_RECEPTION=1) that serves fixture JSON instead of querying the real server. This avoids requiring physical devices in CI. Physical-device drill remains in Phase 444 E2E drills."
  - "Brand drift risk: CLAUDE.md forbids old orange #FF4400. UI-SPEC MUST explicitly mandate #E10600 Racing Red for primary CTAs, #1A1A1A Asphalt Black for background, #5A5A5A Gunmetal for secondary, and Montserrat/Enthocentric font stack. UI-REVIEW verifies all three tokens are applied."
  - "Key links most likely to break: (a) racingpoint-admin/src/app/(dashboard)/mobile/reception/actions.ts MUST post to racecontrol /api/v1/mobile/drivers/:device_id/:driver_id/action NOT any admin-local stub — grep enforces; (b) useReceptionLive hook MUST read from the SAME source (WS or polling) that Phase 437-07 admin mark-ready button currently uses (reduce dual-reality risk) — 441-02A discovery identifies; (c) mobile_audit_events migration MUST be idempotent (CREATE TABLE IF NOT EXISTS + ALTER TABLE ADD COLUMN per CLAUDE.md migration rule) — 441-07 acceptance test enforces."
---

# Phase 441 — Admin Dashboard Reception View

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 441 |
| Name | Admin Dashboard Reception View |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-06 |
| NOT covered (this phase) | ADMIN-04 (feature-flag toggle UI — Phase 442), ADMIN-05 (selector-map push UI — Phase 443) |
| Dependencies | Phase 437 (Zomato driver — first real data source). Phases 438/439 not yet built (panels degrade gracefully). Phase 435 stub ingest endpoint (UPGRADED here). |
| Wave | 6 |
| Status | Ready to execute |
| Autonomous | No — UI-SPEC (441-01) and UI-REVIEW (441-09) are subagent-gated; 441-02 + 441-10 include human-verify visual checkpoints. |
| Ship test | Reception page shows pending Zomato + HyperPure + Blinkit state updating in real time; manual action buttons fire through comms-link to the right device; device panel shows heartbeat + build_id + enabled drivers + last-action-per-driver; audit log viewer paginates real events filterable by device + driver + time. |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 13)

1. **Reception page** shows pending Zomato + HyperPure + Blinkit state with real-time updates.
2. **Manual action buttons** (accept/reject/cancel/retry) fire through comms-link to the right device.
3. **Device status panel** shows heartbeat, agent version, enabled drivers, last action per driver.

(ROADMAP lists 3 SC; this plan adds a 4th implicit SC from ADMIN-06: audit log viewer filterable by device + driver + time with screenshot preview.)

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)

- T-1: When a new Zomato order arrives on Tab Plus (via Phase 437 OrderDetector), within 5s the /mobile/reception page shows it in the "Pending Zomato" panel without the staff member refreshing the page (SC-1, ADMIN-01).
- T-2: When staff clicks "Accept" next to a pending Zomato order, within 15s the Tab Plus Zomato Partner app shows the tap dispatched (via Phase 435 humanize + Phase 437 OrderActions); the UI reflects "Accepted" state within the same cycle (SC-2, ADMIN-02).
- T-3: When staff clicks "Reject", same flow with reject audit row + Zomato UI rejection; page returns to pending state for other orders (SC-2, ADMIN-02).
- T-4: When staff clicks "Mark Ready" on an accepted order, the Tab Plus Zomato "Mark Ready" button is tapped within 15s; UI moves order to "Completed" (SC-2, ADMIN-02 + ZOMATO-04).
- T-5: Tab Plus device card shows: "Last heartbeat: 12s ago" (updating), "Build: abc1234", "Drivers enabled: zomato-partner", "Last action: accepted ZM-2026... 34s ago" (SC-3, ADMIN-03).
- T-6: Samsung M07 device card shows identical schema (empty "Drivers enabled" list until Phase 438 enables HyperPure) (SC-3, ADMIN-03).
- T-7: When Tab Plus goes offline (unplug ethernet or kill agent), within 60s the device card shows "OFFLINE — last seen 45s ago" with red ring; Accept/Reject buttons on ALL Zomato orders become disabled with tooltip "Device offline — action unavailable" (risk: action-dispatch race; must prevent silent dispatch) (SC-2, ADMIN-03).
- T-8: Audit log viewer default view shows the 50 most recent events across all devices/drivers, newest first; selecting "device = rcm-tab-plus" + "driver = zomato-partner" + "last 1h" returns only matching events (SC-4, ADMIN-06).
- T-9: Clicking a row's screenshot-hash pill opens a dialog showing either the image (if storage supports) OR the sentinel reason (e.g., "unavailable: flag_secure") — no XSS, no broken image icon, no crash (SC-4, ADMIN-06).
- T-10: HyperPure panel shows an empty-state "HyperPure driver not yet installed — Phase 438 pending" and does NOT crash or render a broken table (graceful degradation for SC-1 when drivers aren't yet online).
- T-11: Blinkit panel shows the same graceful empty-state pre-Phase 439.
- T-12: /mobile/reception load time (TTFB to interactive) on a James .27 browser hitting the venue admin :3201 is ≤ 3s on the first visit, ≤ 1s on repeat visits (SWR cache) (performance SC — noted in CLAUDE.md "shipped means works for the user").
- T-13: Page works from both the venue admin and cloud admin (racingpoint.cloud) — DEPLOY PARITY rule.
- T-14: Brand verification: all primary CTAs use Racing Red #E10600; no orange #FF4400 anywhere (enforced by UI-REVIEW 441-09).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains / exports |
|------|----------|-----------|---------------------|
| `.planning/phases/441-admin-dashboard-reception-view/UI-SPEC.md` | Full UI specification authored by gsd-ui-researcher | 400 | Page layout, component tree, empty/error/loading states, brand tokens, responsive breakpoints, keyboard nav, a11y notes |
| `racingpoint-admin/src/app/(dashboard)/mobile/reception/page.tsx` | Server Component root | 60 | `export default async function ReceptionPage()` — fetches initial state server-side, passes to client panels |
| `.../reception/DeviceStatusPanel.tsx` | Client Component showing both device cards | 120 | Heartbeat age, build_id, enabled drivers, last action per driver; offline banner |
| `.../reception/ZomatoPanel.tsx` | Zomato orders list + manual action buttons | 180 | Pending/Accepted/Completed sections; Accept/Reject/Mark-Ready buttons with disabled-when-offline state |
| `.../reception/HyperPurePanel.tsx` | In-flight deliveries + cancel | 120 | Empty state pre-Phase 438; confirmation-number badge; cancel button |
| `.../reception/BlinkitPanel.tsx` | In-flight top-ups + retry | 100 | Empty state pre-Phase 439; ETA badge; retry button |
| `.../reception/AuditLogViewer.tsx` | Paginated filterable log | 200 | Device + driver + time filters; table with ts, driver, screen, selector, outcome, screenshot hash pill |
| `.../reception/ScreenshotDialog.tsx` | SHA256 preview modal | 60 | Renders image if hex SHA256 OR sentinel text; no dangerouslySetInnerHTML |
| `.../reception/useReceptionLive.ts` | Realtime hook | 120 | WS subscription OR SWR polling fallback; connection status + reconnect count |
| `.../reception/actions.ts` | Server actions for manual triggers | 100 | `acceptOrderAction`, `rejectOrderAction`, `markReadyAction`, `cancelDeliveryAction`, `retryTopupAction` — all call racecontrol via fetch |
| `.../reception/types.ts` | Shared TS types | 100 | `DeviceSnapshot`, `DriverSnapshot`, `ZomatoOrderRow`, `HyperPureDeliveryRow`, `BlinkitOrderRow`, `AuditEventRow`, `ManualActionRequest`, `ManualActionResponse` |
| `racingpoint-admin/src/lib/api/mobile.ts` | Client SDK | 100 | `getMobileReceptionState()`, `queryMobileAudit()`, `dispatchMobileAction()` |
| `crates/racecontrol/src/api/mobile_audit.rs` | Ingest + query storage | 250 | UPGRADED from stub. `POST /api/v1/mobile-audit/ingest` writes rows; `GET /api/v1/mobile-audit/query?device_id&driver_id&from_ms&to_ms&limit&cursor` returns paginated rows |
| `crates/racecontrol/src/api/mobile_reception.rs` | Live state + manual action | 200 | `GET /api/v1/mobile/reception/state` (aggregated snapshot); `POST /api/v1/mobile/drivers/:device_id/:driver_id/action` dispatches via comms-link |
| `crates/racecontrol/src/db/migrations/NNNN_mobile_audit_events.sql` | SQLite schema | 40 | `CREATE TABLE IF NOT EXISTS mobile_audit_events` + indexes |
| `crates/rc-common/src/mobile_types.rs` | Shared types | 120 | `MobileAuditRow`, `DeviceSnapshot`, `DriverSnapshot`, `ManualActionRequest`, `ManualActionResponse`, `ManualActionError` |
| `comms-link/james/mobile-action-dispatch.js` | Relay forwarder | 80 | Validates incoming ManualActionRequest, forwards as `manual_action_request` envelope to target WS |
| `rc-agent-mobile/.../comms/ManualActionHandler.kt` | Kotlin dispatcher | 80 | Receives `manual_action_request`, routes to correct driver (zomato-partner / hyperpure / blinkit), sends `manual_action_ack` |
| `rc-agent-mobile/docs/PROTOCOL.md` | Appended envelope definitions | +40 | `manual_action_request`, `manual_action_ack` shapes |
| `racingpoint-admin/tests/mobile-reception.spec.ts` | Playwright E2E | 250 | Mock mode enabled via env; simulates driver events, clicks buttons, verifies round-trip |
| `.planning/phases/441-admin-dashboard-reception-view/UI-REVIEW.md` | gsd-ui-auditor 6-pillar audit | 300 | Layout, brand, a11y, perf, error states, responsive — one section each |
| `racingpoint-admin/docs/MOBILE-RECEPTION.md` | Operator runbook | 150 | How to use the page; auth requirements; offline handling; one-time Phase 435-to-441 storage upgrade note |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| ReceptionPage (Server Component) | `GET /api/v1/mobile/reception/state` | fetch with staff JWT | grep `/api/v1/mobile/reception/state` in `page.tsx` |
| useReceptionLive hook | WS channel OR SWR polling | existing dashboard WS hook (from 441-02A discovery) | grep `useReceptionLive` — MUST use the discovered mechanism, NOT a new inline ws:// connection |
| ZomatoPanel Accept button | `dispatchMobileAction("rcm-tab-plus", "zomato-partner", "accept", {order_id})` | server action -> racecontrol | grep `dispatchMobileAction` + `accept` in `ZomatoPanel.tsx` |
| racecontrol `POST /api/v1/mobile/drivers/:device_id/:driver_id/action` | comms-link relay | axum handler -> relay HTTP | grep `mobile-action-dispatch` in `crates/racecontrol/src/api/mobile_reception.rs` |
| comms-link `mobile-action-dispatch.js` | Tab Plus WS (`from=rcm-tab-plus`) | WebSocket send | grep `manual_action_request` in `comms-link/james/mobile-action-dispatch.js` |
| Kotlin `ManualActionHandler` | ZomatoDriver.accept / .reject / .markReady | Kotlin call | grep `manual_action_request` in `ManualActionHandler.kt` |
| `POST /api/v1/mobile-audit/ingest` (real storage) | `mobile_audit_events` table | rusqlite INSERT | grep `INSERT INTO mobile_audit_events` in `mobile_audit.rs` |
| AuditLogViewer filter | `GET /api/v1/mobile-audit/query?device_id=&driver_id=&from_ms=&to_ms=&limit=&cursor=` | admin fetch | grep `/api/v1/mobile-audit/query` in `AuditLogViewer.tsx` |
| Device offline state | Disable Accept/Reject buttons | button disabled prop | grep `device_offline` + `disabled` in `ZomatoPanel.tsx` |
| Manual action when device offline | HTTP 409 + staff toast | racecontrol handler | grep `device_offline` in `mobile_reception.rs` |
| Brand colours | Tailwind config (Racing Red `#E10600`) | className | grep `#E10600` OR `rp-red` OR `racing-red` in /mobile/reception/ |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./racingpoint-admin/CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md         # agent scaffold + protocol
@./.planning/phases/435-humanize-layer-audit-log/PLAN.md                 # audit schema + stub endpoint
@./.planning/phases/437-zomato-partner-driver/PLAN.md                    # Zomato order shape + minimal admin stub at /reception/zomato
@./racingpoint-admin/package.json                                        # confirm Next 16 / React 19 / Tailwind 4 / SWR / Playwright present
@./racingpoint-admin/src/app/(dashboard)/fleet/page.tsx                  # TEMPLATE — server component + client live panels pattern
@./racingpoint-admin/src/lib/api/base.ts                                 # fetch/auth wrapper
@./racingpoint-admin/src/components/ConnectionIndicator.tsx              # reuse for live-state pill
@./crates/racecontrol/src/api/mobile_audit.rs                            # existing stub from Phase 435 (if merged)
@./crates/rc-common/src/lib.rs                                           # shared types registration point

### Interfaces executors will need

**MobileAuditRow (rc-common — NEW in 441-07):**

```rust
#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct MobileAuditRow {
    pub id: i64,                        // autoincrement
    pub device_id: String,              // "rcm-tab-plus" | "rcm-m07"
    pub agent_build_id: String,
    pub driver_id: String,              // "zomato-partner" | "hyperpure" | "blinkit"
    pub app_package: String,
    pub screen: String,
    pub selector_id: Option<String>,
    pub selector_match_confidence: Option<f64>,
    pub action_type: String,            // TAP | SWIPE | TEXT_INPUT | SCREEN_READ | MARK_READY | ACCEPT | REJECT ...
    pub outcome: String,                // SUCCESS | RATE_LIMITED | DROPPED_BUSINESS_HOURS | QUEUED_BUSINESS_HOURS | SELECTOR_MISS | ERROR
    pub duration_ms: i64,
    pub screenshot_sha256: String,      // 64-hex OR "sha256:unavailable:<reason>"
    pub error_class: Option<String>,
    pub error_message: Option<String>,
    pub ts_ms: i64,                     // agent-side timestamp (authoritative)
    pub received_at_ms: i64,            // server insert time (for lag metric)
    pub correlation_id: Option<String>, // optional cross-event linkage (Phase 435-02)
}
```

**DeviceSnapshot / DriverSnapshot (rc-common — NEW in 441-08):**

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeviceSnapshot {
    pub device_id: String,              // "rcm-tab-plus"
    pub device_model: String,           // "Lenovo TB-351FU"
    pub ws_connected: bool,
    pub http_reachable: bool,
    pub agent_build_id: String,
    pub last_heartbeat_age_secs: Option<u64>,
    pub drivers: Vec<DriverSnapshot>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DriverSnapshot {
    pub driver_id: String,              // "zomato-partner"
    pub enabled: bool,
    pub last_action_ts_ms: Option<i64>,
    pub last_action_outcome: Option<String>,
    pub last_action_summary: Option<String>,  // human-readable "Accepted ZM-2026041801234"
    pub healthy: bool,
    pub health_detail: Option<String>,        // e.g., "session_expired"
}
```

**ManualActionRequest / Response (rc-common — NEW in 441-08):**

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManualActionRequest {
    pub device_id: String,
    pub driver_id: String,
    pub action: String,                 // "accept" | "reject" | "mark_ready" | "cancel" | "retry"
    pub payload: serde_json::Value,     // driver-specific (order_id, delivery_id, etc.)
    pub correlation_id: String,         // for matching with manual_action_ack
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManualActionResponse {
    pub ok: bool,
    pub dispatched_at_ms: i64,
    pub error: Option<ManualActionError>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ManualActionError {
    pub code: String,                   // "device_offline" | "driver_disabled" | "unknown_action" | "unauthorized"
    pub message: String,
    pub last_seen_secs_ago: Option<u64>,
}
```

**TypeScript mirror (racingpoint-admin — 441-02):**

```ts
// src/app/(dashboard)/mobile/reception/types.ts
export interface DeviceSnapshot { device_id: string; device_model: string; ws_connected: boolean; http_reachable: boolean; agent_build_id: string; last_heartbeat_age_secs: number | null; drivers: DriverSnapshot[] }
export interface DriverSnapshot { driver_id: string; enabled: boolean; last_action_ts_ms: number | null; last_action_outcome: string | null; last_action_summary: string | null; healthy: boolean; health_detail: string | null }
export interface ZomatoOrderRow { order_id: string; status: "pending" | "accepted" | "completed" | "rejected"; items: Array<{ name: string; qty: number; price_paise: number }>; total_paise: number; detected_at_ms: number; accepted_at_ms: number | null }
export interface AuditEventRow { id: number; device_id: string; driver_id: string; screen: string; selector_id: string | null; action_type: string; outcome: string; ts_ms: number; screenshot_sha256: string }
export interface ManualActionResponse { ok: boolean; dispatched_at_ms: number; error: { code: string; message: string; last_seen_secs_ago: number | null } | null }
```

**Envelope (appended to rc-agent-mobile/docs/PROTOCOL.md):**

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "manual_action_request",
  "from": "racecontrol",
  "to": "rcm-tab-plus",
  "ts": 1713600000000,
  "id": "uuid-v4",
  "payload": {
    "device_id": "rcm-tab-plus",
    "driver_id": "zomato-partner",
    "action": "accept",
    "payload": { "order_id": "ZM-2026041801234" },
    "correlation_id": "ma-uuid-v4"
  }
}
```

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "manual_action_ack",
  "from": "rcm-tab-plus",
  "to": "racecontrol",
  "ts": 1713600001200,
  "id": "uuid-v4",
  "payload": {
    "correlation_id": "ma-uuid-v4",
    "ok": true,
    "dispatched_at_ms": 1713600001200,
    "error": null
  }
}
```

## 5. Atomic plan breakdown (10 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

Dependency order (parentheses indicate prerequisite plan within phase):
- 441-01 UI-SPEC (GATE, no code dependency within phase — reads 435+437 plan outputs as source)
- 441-02 scaffold + realtime strategy depends on 441-01 (must have UI-SPEC merged)
- 441-07 storage upgrade is an INDEPENDENT track — can land any time before 441-06
- 441-08 manual-action endpoint depends on 441-07 (shares types in rc-common) and is consumed by 441-03/04/05
- 441-03, 441-04, 441-05 depend on 441-02 + 441-08
- 441-06 depends on 441-02 + 441-07 (real query endpoint)
- 441-09 UI-REVIEW gate depends on 441-02..441-06 (needs something to audit)
- 441-10 E2E depends on everything

---

### 441-01-PLAN — UI-SPEC via gsd-ui-researcher subagent (MANDATORY GATE)

**Goal:** Author `.planning/phases/441-admin-dashboard-reception-view/UI-SPEC.md` via the `gsd-ui-researcher` subagent BEFORE any component is written. Per CLAUDE.md Subagent Gates: "No frontend phase ships without UI-SPEC.md AND UI-REVIEW.md." UI-SPEC is the blueprint; without it, executors make ad-hoc layout decisions that later require rework.

**Covers:** Hard gate prerequisite for ADMIN-01, ADMIN-02, ADMIN-03, ADMIN-06.

**Dependencies:** None within phase (reads Phase 437 / 435 plans as inputs).

**Type:** `checkpoint:subagent` (invoke gsd-ui-researcher — returns UI-SPEC.md committed to phase dir).

#### Subagent invocation

Invoke `gsd-ui-researcher` with the following prompt summary:

- Page route: `/mobile/reception` inside existing racingpoint-admin (dashboard) group, basePath-aware.
- Target users: reception counter staff at the Racing Point venue, using a shared desktop with a mounted 24" monitor + occasional iPad access (responsive 768px+).
- Required sections: Device Status Panel (2 cards — Tab Plus + M07), Zomato Panel (pending/accepted/completed sub-sections), HyperPure Panel (empty state + in-flight), Blinkit Panel (empty state + in-flight), Audit Log Viewer (filter bar + table + pagination + screenshot dialog).
- States per panel: loading, empty, error, degraded-device-offline, populated.
- Brand tokens (NON-NEGOTIABLE): Racing Red `#E10600` primary CTA, Asphalt Black `#1A1A1A` background, Gunmetal `#5A5A5A` secondary, Card `#222222`, Border `#333333`. Montserrat body, Enthocentric headers. NO orange `#FF4400`.
- Interaction patterns: buttons DISABLE (not hide) when device offline, with tooltip explaining fleet state; toast (sonner) on action dispatch success + failure; connection status pill top-right (reuse ConnectionIndicator.tsx).
- Accessibility: keyboard-only operation for Accept/Reject/Mark-Ready, aria-live on incoming orders, screen reader labels for status pills, contrast ratio >= 4.5:1 for all text.
- Responsive: 1280px primary (desktop), 1024px tablet fallback, 768px mobile degraded (audit viewer becomes cards).
- Data patterns referenced: §4 Interfaces above (DeviceSnapshot, ZomatoOrderRow, AuditEventRow, ManualActionResponse).
- Discovery the subagent MUST perform: find the existing realtime mechanism used by (dashboard)/fleet/page.tsx and document whether this phase should reuse WS or fall back to SWR polling. This feeds 441-02A.

#### Acceptance

- `UI-SPEC.md` file exists at `.planning/phases/441-admin-dashboard-reception-view/UI-SPEC.md`.
- Contains sections: Overview, Page Layout (ASCII/textual wireframe), Component Tree, States (loading/empty/error/degraded/populated per panel), Brand Tokens (explicit hex values + Tailwind class mapping), Interaction Specs (hover/focus/disabled/loading), Accessibility, Responsive Breakpoints, Realtime Strategy Discovery (441-02A input).
- Committed to git with message `docs(441-01): author UI-SPEC via gsd-ui-researcher`.

#### Checkpoint

User confirms UI-SPEC.md exists and subagent run completed. No visual verification yet — the page isn't built.

#### Commit message

```
docs(441-01): UI-SPEC.md authored via gsd-ui-researcher

Full page layout, component tree, state matrix, brand tokens, a11y,
responsive breakpoints for /mobile/reception.  Prerequisite for 441-02+.
Gate: PRE-EXECUTION (no code yet).

Covers: UI-SPEC subagent gate (CLAUDE.md Subagent Gates section)
```

---

### 441-02-PLAN — Reception page scaffold + realtime strategy + DeviceStatusPanel

**Goal:** Scaffold `/mobile/reception` route with server-component initial load + client-component live panels. Determine the realtime strategy (441-02A discovery) and wire DeviceStatusPanel against the real `GET /api/v1/mobile/reception/state` (stubbed if 441-08 not yet merged — see dependency note). Land empty-state versions of the other three panels (ZomatoPanel / HyperPurePanel / BlinkitPanel) so the layout can be visually verified end-to-end before data flows in.

**Covers:** ADMIN-03 (device status panel).

**Dependencies:** 441-01 (UI-SPEC merged). Soft dependency on 441-08 (state endpoint). If 441-08 lands AFTER 441-02, scaffold this plan against a local fixture JSON file at `racingpoint-admin/src/app/(dashboard)/mobile/reception/__fixtures__/state.json` and gate the real-fetch swap behind a one-line change in page.tsx.

**Type:** `auto` + `tdd="true"` for the reception-live hook + `checkpoint:human-verify` at end (visual brand verification — CLAUDE.md visual verification rule).

#### Sub-task 441-02A (discovery, runs FIRST)

Enumerate the existing realtime mechanism in racingpoint-admin:

```bash
grep -rn "useWebSocket\|WebSocket\|EventSource\|useSWR.*refresh\|refreshInterval" racingpoint-admin/src/ | head -30
```

Record in `UI-SPEC.md` (amendment to 441-01 output) one of:
- **Option A:** Existing WS hook found at `<path>` — reuse via `import { useWebSocket } from "<path>"`.
- **Option B:** No WS hook — use SWR with `refreshInterval: 2000` on `getMobileReceptionState()`; add reconnect-count pill via manual counter in the fetcher.
- **Option C:** WS exists but only for specific dashboards (e.g. /fleet) and is not reusable — document the reason and default to Option B.

Expected outcome per §5 risks: likely Option B (admin has 2 WS refs, neither a reusable hook).

#### Behavior (tdd="true" block for useReceptionLive)

- Test 1: `useReceptionLive returns initialData on first render, then updates on poll` — mock fetch returns { devices: [...], updated_at_ms: 1000 }, advance 2100ms, assert second fetch fires, state updates.
- Test 2: `useReceptionLive exposes connection status: 'connected' | 'reconnecting' | 'degraded'` — on fetch 500, status becomes `degraded` within one cycle.
- Test 3: `useReceptionLive stops polling on unmount` — unmount the hook, advance 5s, assert no more fetches.
- Test 4: `useReceptionLive dedupes in-flight requests` — rapidly re-trigger, assert only one fetch per interval.
- Test 5: `useReceptionLive reconnects after transient failure` — 2 consecutive 500s -> 200 -> status returns to 'connected'.

#### Tasks

1. Create sidebar entry in `racingpoint-admin/src/app/(dashboard)/mobile/layout.tsx` matching existing (dashboard) layout pattern. Add "Reception" menu item with a lucide-react icon (e.g., `UtensilsCrossed` or `ConciergeBell`).
2. Create `page.tsx` as a **Server Component**:
   - Validates staff JWT via existing `src/lib/auth.ts` pattern.
   - Server-fetches initial state: `const initial = await fetch("http://127.0.0.1:8080/api/v1/mobile/reception/state", { headers: { Authorization: ... }, cache: "no-store" })`.
   - If fetch fails (e.g., 441-08 not yet merged), reads `__fixtures__/state.json`.
   - Renders `<ReceptionClient initial={initial} />`.
3. Create `<ReceptionClient>` (client component wrapper):
   - Uses `useReceptionLive(initial)` hook.
   - Renders grid: `<DeviceStatusPanel>` (full width row 1), then `<ZomatoPanel> | <HyperPurePanel> | <BlinkitPanel>` (3-col row 2), then `<AuditLogViewer>` (full width row 3).
   - Shows `<ConnectionIndicator status={live.status} reconnectCount={live.reconnectCount} />` top-right.
4. Create `<DeviceStatusPanel>`:
   - Two side-by-side cards (Tab Plus + M07).
   - Each card shows: device_id, device_model, WS/HTTP status pill, build_id (monospace), last heartbeat age (updating), list of drivers (driver_id + enabled pill + "Last action: ..." line + healthy indicator).
   - Offline state: red ring + "Last seen Ns ago" + greyed action buttons in sibling panels (enforced via React context `DeviceOnlineContext` that Zomato/HyperPure/Blinkit panels consume).
   - Uses brand tokens from UI-SPEC.
5. Create stub versions of ZomatoPanel / HyperPurePanel / BlinkitPanel — empty state + "Pending implementation in 441-03/04/05" label, visually correct per UI-SPEC.
6. Create `useReceptionLive.ts` implementing Option B (SWR polling with reconnect counter) — or Option A if discovery found a WS hook.
7. Create `src/lib/api/mobile.ts` with `getMobileReceptionState()` following the existing `src/lib/api/fleet.ts` pattern (staff JWT header, error shape).
8. Unit tests (RTL + Vitest or existing admin test runner — follow what's already there; if no runner exists, defer unit tests to 441-10 Playwright).

#### Acceptance

- `/mobile/reception` loads on `http://localhost:3000/mobile/reception` (dev) and renders the scaffold without crash.
- DeviceStatusPanel populates from fixture JSON (2 devices, each with an empty drivers list).
- Connection status pill is visible and shows "connected" when fetch succeeds, "degraded" after simulated failure.
- All 5 useReceptionLive tests pass.
- `npm run lint` passes (no `any`, no unused imports).
- `npm run build` succeeds (Next.js + Tailwind 4 build).

#### Checkpoint (human-verify — brand sanity)

User visits /mobile/reception in a browser; confirms:
- Primary action buttons use Racing Red `#E10600` (not orange).
- Background is Asphalt Black `#1A1A1A` (or the existing admin theme base).
- Montserrat body font + Enthocentric headers applied.
- No broken icons, no missing fonts (F12 network tab).

Resume signal: "approved" OR bug report.

#### Commit message

```
feat(441-02): /mobile/reception scaffold + DeviceStatusPanel + useReceptionLive

Server Component page.tsx fetches initial state (real endpoint OR fixture fallback
if 441-08 not yet merged). Client wrapper renders DeviceStatusPanel + stub child
panels per UI-SPEC. useReceptionLive hook polls every 2s with reconnect counter.
Realtime strategy discovery (441-02A): <A|B|C> -> <chosen mechanism>.

Covers: ADMIN-03 (device status panel) + scaffold for ADMIN-01/02/06.
Not tested: real data flow (awaits 441-03/04/05/06/08).
```

---

### 441-03-PLAN — Zomato orders panel (pending list + manual accept/reject/mark-ready)

**Goal:** Replace the ZomatoPanel stub (from 441-02) with the real panel that reads pending/accepted/completed Zomato orders, lets staff click Accept/Reject (on pending) and Mark-Ready (on accepted), and dispatches each click via `POST /api/v1/mobile/drivers/rcm-tab-plus/zomato-partner/action` through the server action chain.

**Covers:** ADMIN-01 (pending Zomato), ADMIN-02 (accept/reject/mark-ready).

**Dependencies:** 441-02 + 441-08 (manual-action endpoint).

**Type:** `auto` + `tdd="true"`.

#### Behavior (tdd="true" block)

- Test 1: `ZomatoPanel renders pending / accepted / completed sub-sections` — given 3 orders one of each status, each appears in the right section.
- Test 2: `Accept button dispatches acceptOrderAction and optimistically marks order 'accepting...'` — click, assert fetch called with (device_id, driver_id, action="accept", payload={order_id}), assert local UI state transition.
- Test 3: `Accept on device-offline is DISABLED with tooltip` — given DeviceOnlineContext.tabPlus=false, Accept button has `disabled` attr + tooltip "Tab Plus offline — action unavailable".
- Test 4: `Reject button dispatches rejectOrderAction` — same as Test 2 but with action="reject".
- Test 5: `Mark-Ready only shown on accepted orders` — pending/completed rows do NOT render Mark-Ready.
- Test 6: `Server action returns 409 device_offline -> shows toast 'Tab Plus is offline, try again when it reconnects'` — fetches rejects with error code.
- Test 7: `Server action returns 200 -> audit row appears in AuditLogViewer within one live-update cycle` — integration-lite.
- Test 8: `correlation_id flows through: dispatch includes correlation_id; manual_action_ack arrives with matching correlation_id; UI reconciles from 'accepting...' to 'accepted'` — happy-path round-trip.

#### Tasks

1. Replace `ZomatoPanel.tsx` stub with real implementation:
   - Three collapsible sub-sections (Pending / Accepted / Completed) with count badges.
   - Each row: order_id, customer_name_masked, item summary ("3 items, Rs 540"), detected_at relative time, action buttons conditional on status.
   - Uses DeviceOnlineContext to disable buttons + tooltip.
   - Uses sonner for toasts.
2. Create `actions.ts` server actions: `acceptOrderAction(orderId, correlationId)`, `rejectOrderAction`, `markReadyAction` — each POSTs to `/api/v1/mobile/drivers/rcm-tab-plus/zomato-partner/action` with the right action string + payload.
3. Add optimistic UI state: clicking Accept sets status to `accepting` immediately; on ack (via audit row OR explicit ack), reconciles to `accepted`. On 409 error, reverts to `pending`.
4. Wire the orders list into `useReceptionLive` — orders are part of the state snapshot returned by `GET /api/v1/mobile/reception/state`.
5. Tests (RTL or Playwright component mode per 441-02 decision).

#### Acceptance

- All 8 tests pass.
- Manual: seed fixture state with 2 pending + 1 accepted + 1 completed order. Click Accept on pending — UI transitions to `accepting...` within 50ms (optimistic), then to `accepted` within 2s (live update cycle).
- Device offline: buttons visibly disabled with tooltip.
- 409 response: toast appears with correct message.

#### G4 NOT TESTED list

- Real Tab Plus Zomato UI dispatch (requires physical device — Phase 444 E2E drill).
- Long-running flood of orders (Phase 444 stress test).

#### Commit message

```
feat(441-03): ZomatoPanel pending/accepted/completed + manual actions

Replaces 441-02 stub. Accept/Reject/Mark-Ready buttons dispatch via
POST /api/v1/mobile/drivers/rcm-tab-plus/zomato-partner/action.
Optimistic UI with ack reconciliation. Disabled-with-tooltip when Tab
Plus offline. Sonner toasts on 4xx/5xx.

Covers: ADMIN-01, ADMIN-02 (Zomato slice)
```

---

### 441-04-PLAN — HyperPure deliveries panel (in-flight + cancel)

**Goal:** HyperPurePanel showing in-flight deliveries with confirmation number + scheduled delivery window + Cancel button. Pre-Phase 438 gracefully shows empty state "HyperPure driver not yet installed" without crashing.

**Covers:** ADMIN-01 (HyperPure slice), ADMIN-02 (cancel action).

**Dependencies:** 441-02 + 441-08.

**Type:** `auto` + `tdd="true"`.

#### Behavior (tdd="true" block)

- Test 1: `HyperPurePanel renders empty state when no driver enabled anywhere` — heading + "HyperPure driver not yet installed (Phase 438 pending)" text + greyed section.
- Test 2: `HyperPurePanel renders in-flight list when driver enabled` — given fixture with 2 in-flight deliveries, each shows confirmation_number, delivery_window, cancel button.
- Test 3: `Cancel button dispatches cancelDeliveryAction` — click -> POST /api/v1/mobile/drivers/<device>/hyperpure/action with action="cancel".
- Test 4: `Offline device disables Cancel` — DeviceOnlineContext says M07 offline -> Cancel disabled with tooltip.

#### Tasks

1. Replace HyperPurePanel stub with real implementation.
2. Add `cancelDeliveryAction` to actions.ts.
3. Add graceful empty state keyed off `drivers.some(d => d.driver_id === "hyperpure" && d.enabled)`.
4. Tests.

#### Acceptance

- All 4 tests pass.
- Manual: with Phase 438 NOT merged, panel shows empty state (not a broken skeleton). With fixture override enabling HyperPure, panel renders list.

#### Commit message

```
feat(441-04): HyperPurePanel with cancel button + graceful pre-438 empty state

Empty state when no device has hyperpure driver enabled. When enabled,
shows in-flight deliveries with confirmation_number + window + cancel.
Cancel dispatches via /api/v1/mobile/drivers/<device>/hyperpure/action.

Covers: ADMIN-01 (HyperPure slice), ADMIN-02 (cancel)
```

---

### 441-05-PLAN — Blinkit status panel (in-flight top-ups + retry)

**Goal:** BlinkitPanel showing in-flight top-ups with ETA + Retry button on failed/stalled. Pre-Phase 439 empty state same pattern as HyperPure.

**Covers:** ADMIN-01 (Blinkit slice), ADMIN-02 (retry action).

**Dependencies:** 441-02 + 441-08.

**Type:** `auto` + `tdd="true"`.

#### Behavior (tdd="true" block)

- Test 1: `BlinkitPanel renders empty state when blinkit driver not enabled` — identical pattern to HyperPure test 1.
- Test 2: `BlinkitPanel renders active top-ups with ETA` — fixture with 1 top-up at 12min ETA -> row shows "Blinkit #BLK-123, ETA 12min".
- Test 3: `Retry button only shown on status=failed | stalled` — status=active hides retry.
- Test 4: `Retry dispatches retryTopupAction` — click -> POST ... action=retry.
- Test 5: `Offline M07 disables Retry` — DeviceOnlineContext offline -> disabled + tooltip.

#### Tasks

1. Replace BlinkitPanel stub with real implementation.
2. Add `retryTopupAction` to actions.ts.
3. Graceful empty state.
4. Tests.

#### Acceptance

- All 5 tests pass.

#### Commit message

```
feat(441-05): BlinkitPanel with retry button + graceful pre-439 empty state

Empty state when no device has blinkit driver enabled. When enabled,
shows in-flight top-ups with ETA + retry on failed/stalled.

Covers: ADMIN-01 (Blinkit slice), ADMIN-02 (retry)
```

---

### 441-06-PLAN — Audit log viewer (server-paginated filter + screenshot preview)

**Goal:** `<AuditLogViewer>` renders a filter bar (device dropdown, driver dropdown, time range preset: last 15min / 1h / 24h / custom, custom from-ts + to-ts) and a paginated table of audit events. Clicking a row's screenshot-hash pill opens `<ScreenshotDialog>` showing either the image (if 441-07 supports image retrieval) OR the sentinel reason. Pagination uses cursor-based (`next_cursor` from server) not offset-based (performance on large tables).

**Covers:** ADMIN-06 (log viewer + filter + screenshot preview).

**Dependencies:** 441-02 (scaffold) + 441-07 (real query endpoint).

**Type:** `auto` + `tdd="true"`.

#### Behavior (tdd="true" block)

- Test 1: `AuditLogViewer default view shows last 50 events across all devices/drivers, newest first` — fixture with 60 events, first page shows 50.
- Test 2: `Device filter narrows results` — select rcm-tab-plus -> query includes device_id=rcm-tab-plus.
- Test 3: `Driver filter narrows results` — select zomato-partner -> query includes driver_id=zomato-partner.
- Test 4: `Time range preset sends correct from_ms/to_ms` — "last 1h" -> from_ms = now - 3600_000.
- Test 5: `Custom range with invalid dates shows validation error` — from > to -> inline error, no fetch.
- Test 6: `Pagination advances via cursor` — click Next -> fetch includes cursor=<prev_response.next_cursor>.
- Test 7: `Screenshot pill with 64-hex opens dialog attempting image render` — sha `abc...` -> dialog shows `<img src="/api/v1/mobile-audit/screenshot/abc..." />` OR sentinel if 441-07 chose inline-sha256-only storage (project decision per racecontrol.toml [mobile_reception] section).
- Test 8: `Screenshot pill with sentinel opens dialog showing reason` — sha `"sha256:unavailable:flag_secure"` -> dialog text "Screenshot unavailable: FLAG_SECURE (app blocks capture)" — NO image request made.
- Test 9: `No dangerouslySetInnerHTML anywhere` — grep test in CI.

#### Tasks

1. Create `AuditLogViewer.tsx`:
   - Filter bar top: device select (populated from DeviceSnapshot list + "All devices"), driver select ("All drivers" + union of all driver_ids seen), time range segmented control (15m / 1h / 24h / custom), custom from/to inputs.
   - Table: columns ts (relative), device_id, driver_id, screen, selector_id, action_type, outcome (color-coded pill), duration_ms, screenshot pill.
   - Pagination footer: "Showing 1-50 of N" + [Next] button (disabled when no next_cursor).
2. Create `ScreenshotDialog.tsx`:
   - Takes `sha256: string` prop.
   - If sentinel (starts with `"sha256:unavailable:"`), renders text per Test 8.
   - Else, renders `<img>` attempting `/api/v1/mobile-audit/screenshot/<sha>`; on 404, falls back to "Image not available on this deploy (see MOBILE-RECEPTION.md storage mode)".
   - Uses only Tailwind text rendering — no dangerouslySetInnerHTML.
3. Add `queryMobileAudit({ device_id?, driver_id?, from_ms?, to_ms?, limit?, cursor? })` to `src/lib/api/mobile.ts`.
4. Wire filter debounce (300ms) to avoid hammering the server on dropdown changes.
5. Tests (RTL or Playwright component mode).

#### Acceptance

- All 9 tests pass.
- Manual: with Phase 435 events replayed into the real DB (test script writes 100 rows), filter by device = rcm-tab-plus + driver = zomato-partner + "last 24h" returns only matching rows, paginated.

#### Commit message

```
feat(441-06): AuditLogViewer with device+driver+time filter + cursor pagination

Filter bar (device, driver, time preset, custom range) + table of audit
events + cursor-paginated Next. Screenshot pill opens dialog rendering
image OR sentinel reason. Zero dangerouslySetInnerHTML.

Covers: ADMIN-06 (log viewer + screenshot preview)
```

---

### 441-07-PLAN — Server-side audit storage upgrade (SQLite mobile_audit_events)

**Goal:** UPGRADE `crates/racecontrol/src/api/mobile_audit.rs` from the Phase 435 stub (returns 200, discards data) into real persistent storage. Creates `mobile_audit_events` SQLite table via idempotent migration, writes each ingested event, and exposes a query endpoint for the audit viewer.

**Covers:** Storage backing for AUDIT-01/02/03/04 (Phase 435 requirements); prerequisite for ADMIN-06.

**Dependencies:** Phase 435 (existing stub endpoint + ingest contract).

**Type:** `auto` + `tdd="true"`.

#### Behavior (tdd="true" block)

- Test 1: `migration creates mobile_audit_events table idempotently` — run migration twice on same DB, no error, schema stable.
- Test 2: `INSERT stores all 15 fields of MobileAuditRow` — ingest a batch of 3 events, SELECT * asserts round-trip.
- Test 3: `Ingest validates screenshot_sha256 shape` — a payload with sha = "<script>alert(1)</script>" returns 400 with error "invalid screenshot_sha256 format"; accepted shapes are 64-hex OR /^sha256:unavailable:[a-z_]+$/.
- Test 4: `Ingest is idempotent on (device_id, agent_build_id, ts_ms, driver_id, action_type)` — same event shipped twice -> second INSERT ignored (UNIQUE constraint), server returns accepted=count_new.
- Test 5: `Query by device_id + driver_id + from_ms/to_ms returns correct rows` — seed 10 rows across 2 devices + 2 drivers + 3 time buckets; assert each combination returns the right subset.
- Test 6: `Query pagination with cursor returns next page stably under concurrent inserts` — seed 100 rows; page 1 limit=20; insert 5 new rows with newer ts_ms; page 2 using cursor from page 1 returns rows 21-40 of the ORIGINAL set (not mixed with new inserts — cursor is (ts_ms, id) not offset).
- Test 7: `Retention policy (optional [mobile_reception] audit_retention_days) evicts rows older than N days` — seed 1 row with ts_ms = (now - 31d), set retention=30, run GC task, assert row deleted.
- Test 8: `Query endpoint requires staff JWT` — no auth -> 401.

#### Tasks

1. Create migration `crates/racecontrol/src/db/migrations/NNNN_mobile_audit_events.sql` (NNNN = next free number in the existing migration sequence — enumerate first):

```sql
CREATE TABLE IF NOT EXISTS mobile_audit_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    agent_build_id TEXT NOT NULL,
    driver_id TEXT NOT NULL,
    app_package TEXT NOT NULL,
    screen TEXT NOT NULL,
    selector_id TEXT,
    selector_match_confidence REAL,
    action_type TEXT NOT NULL,
    outcome TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    screenshot_sha256 TEXT NOT NULL,
    error_class TEXT,
    error_message TEXT,
    ts_ms INTEGER NOT NULL,
    received_at_ms INTEGER NOT NULL,
    correlation_id TEXT,
    UNIQUE(device_id, agent_build_id, ts_ms, driver_id, action_type)
);
CREATE INDEX IF NOT EXISTS idx_mobile_audit_device_ts ON mobile_audit_events(device_id, ts_ms DESC);
CREATE INDEX IF NOT EXISTS idx_mobile_audit_driver_ts ON mobile_audit_events(driver_id, ts_ms DESC);
CREATE INDEX IF NOT EXISTS idx_mobile_audit_app_ts ON mobile_audit_events(app_package, ts_ms DESC);
CREATE INDEX IF NOT EXISTS idx_mobile_audit_correlation ON mobile_audit_events(correlation_id) WHERE correlation_id IS NOT NULL;
```

2. Register migration in `crates/racecontrol/src/db/migrations.rs` (follow existing pattern).
3. UPGRADE `ingest_handler` in `crates/racecontrol/src/api/mobile_audit.rs`:
   - Parse each JSONL line of `batch.events_jsonl` into a MobileAuditRow.
   - Validate `screenshot_sha256` regex: `^[a-f0-9]{64}$|^sha256:unavailable:[a-z_]+$` — reject batch with 400 if any line fails.
   - INSERT OR IGNORE (leverages UNIQUE constraint for idempotency).
   - Return `{ accepted: count_actually_inserted, duplicates_ignored: count_duplicates, storage: "sqlite" }`.
4. ADD `query_handler`:
   - Route: `GET /api/v1/mobile-audit/query?device_id=&driver_id=&from_ms=&to_ms=&limit=&cursor=` (staff JWT required).
   - `cursor` format: `<ts_ms>_<id>` base64 (composite key — stable under concurrent inserts per Test 6).
   - `limit` capped at `max_page_size` from [mobile_reception] config (default 100).
   - Response: `{ rows: [MobileAuditRow...], next_cursor: Option<String>, total_estimate: Option<i64> }`.
5. ADD optional `GET /api/v1/mobile-audit/screenshot/:sha256` — for phase-441 we implement the 404 stub (storage mode = "inline_sha256_only" — no actual image bytes stored). Future phase extends to image storage if needed. Sentinel form returns 404 immediately.
6. ADD optional retention GC background task (runs hourly, deletes rows older than `audit_retention_days` — default 30).
7. Integration test: `crates/racecontrol/tests/mobile_audit_storage.rs` — spins up an in-memory DB, runs migration, ingests 100 events, queries with filters.

#### Acceptance

- All 8 tests pass.
- Migration runs on an EXISTING racecontrol.db (venue server copy) without error — idempotent.
- `curl -X POST http://127.0.0.1:8080/api/v1/mobile-audit/ingest -H "Authorization: Bearer $SERVICE_KEY" -d @test-batch.json` returns 200 with `{accepted: N, storage: "sqlite"}` (NOT `storage: "stub"` — cascade update check).
- `curl "http://127.0.0.1:8080/api/v1/mobile-audit/query?limit=10" -H "Authorization: Bearer $STAFF_JWT"` returns rows.

#### Cascade update checklist

Per CLAUDE.md Cascade updates (RECURSIVE) rule:
- [ ] Update `rc-agent-mobile/docs/AUDIT-LOG.md` — remove the "stub — Phase 441 replaces" note, add the real storage contract.
- [ ] Update `comms-link/james/mobile-audit-forward.js` if any routing changed (expected: no change — still forwards to same URL).
- [ ] Update server-side OpenAPI or contract tests if they exist (grep `mobile_audit_batch_received` and `storage.*stub`).
- [ ] Grep for any code that depended on the stub's `storage: "stub"` response — must be updated to accept `"sqlite"`.

#### Commit message

```
feat(441-07): upgrade mobile_audit ingest from stub to SQLite storage

New table mobile_audit_events with (device_id,ts_ms), (driver_id,ts_ms),
(app_package,ts_ms), (correlation_id) indexes. UNIQUE constraint on
(device_id,agent_build_id,ts_ms,driver_id,action_type) for idempotent
re-ship. Query endpoint GET /api/v1/mobile-audit/query with cursor
pagination (stable under concurrent insert). Screenshot endpoint
returns 404 sentinel (inline_sha256_only storage mode). Migration
runs idempotently on existing racecontrol.db. DEPLOY PARITY: runs on
Bono VPS too.

Covers: AUDIT-01/02/03/04 real storage (Phase 435 stub -> sqlite).
Prerequisite for Phase 441-06 AuditLogViewer.
Note: One-time lossy upgrade — historically-stubbed events permanently
lost (acceptable pre-prod, documented in MOBILE-RECEPTION.md).
```

---

### 441-08-PLAN — Server-side manual-action endpoint + comms-link dispatch

**Goal:** ADD `POST /api/v1/mobile/drivers/:device_id/:driver_id/action` on racecontrol server; validates staff JWT, device online status, and driver state; forwards envelope via comms-link `mobile-action-dispatch.js` to the target agent's WS; returns HTTP 200 on dispatch, 409 on device offline, 401 on auth, 404 on unknown device/driver. Also adds `GET /api/v1/mobile/reception/state` aggregating device + driver + order state for page load.

**Covers:** ADMIN-01, ADMIN-02 (all action dispatch flow); prerequisite for 441-03/04/05.

**Dependencies:** 441-07 (shared types), Phase 429 (relay protocol), Phase 437 (Zomato driver to dispatch to).

**Type:** `auto` + `tdd="true"`.

#### Behavior (tdd="true" block)

- Test 1: `POST /api/v1/mobile/drivers/rcm-tab-plus/zomato-partner/action with valid staff JWT + online device + valid action "accept" -> 200 dispatched_at_ms set`.
- Test 2: `POST ... without staff JWT -> 401`.
- Test 3: `POST ... target device unknown -> 404 {error: "unknown_device"}`.
- Test 4: `POST ... target driver unknown -> 404 {error: "unknown_driver"}`.
- Test 5: `POST ... device offline (ws_connected=false) -> 409 {error: {code: "device_offline", last_seen_secs_ago: N}}`.
- Test 6: `POST ... action unknown -> 400 {error: "unknown_action"}`.
- Test 7: `POST ... relay dispatch fails -> 502 {error: "relay_unavailable"}`.
- Test 8: `manual_action_ack from agent updates a pending-action map; second GET /reception/state reflects the ack`.
- Test 9: `GET /api/v1/mobile/reception/state aggregates 2 devices + their drivers + their last actions in <100ms for a 1000-row DB`.
- Test 10: `correlation_id round-trip: request carries ma-uuid-X; dispatched envelope has same id; ack with id=ma-uuid-X matches back to the original request context`.

#### Tasks

1. Create `crates/rc-common/src/mobile_types.rs` with the types from §4 Interfaces. Re-export from `lib.rs`.
2. Create `crates/racecontrol/src/api/mobile_reception.rs`:
   - `dispatch_action_handler(Path((device_id, driver_id)), State(app), Json(req))`:
     - Validate staff JWT (reuse existing middleware).
     - Look up device in fleet state (mobile devices registered via Phase 429 — new `FleetState::mobile_devices()` accessor).
     - If device not found: 404.
     - If driver not in device.drivers: 404.
     - If device.ws_connected = false: 409 with last_seen.
     - If req.action not in driver.supported_actions: 400.
     - Build `manual_action_request` envelope per Protocol §4.
     - POST to `http://localhost:8766/relay/forward` (or the configured comms-link URL) — NEW relay endpoint from `mobile-action-dispatch.js`.
     - If relay returns non-200: 502.
     - Register pending-action in in-memory map keyed by correlation_id (TTL 60s).
     - Return 200 with dispatched_at_ms.
   - `reception_state_handler(State(app))`:
     - Query fleet state for all rcm-* devices.
     - For each device, query its drivers (enabled flags from Phase 436 / Phase 442 feature-flag system — if Phase 442 not merged, treat all manifest-declared drivers as enabled).
     - For each driver, query mobile_audit_events for last action (ts_ms, outcome, action_type) — LIMIT 1 per (device_id, driver_id).
     - Query Zomato orders state (Phase 437 added `/api/v1/zomato/orders` or similar — if not, return empty array with a TODO comment marked WITH the Phase 437 cross-ref).
     - Similarly HyperPure / Blinkit — empty arrays pre-Phase-438/439.
     - Aggregate + return.
3. Register routes in `crates/racecontrol/src/api/mod.rs` + `crates/racecontrol/src/api/routes.rs` (staff-JWT gated — NOT public routes).
4. Create `comms-link/james/mobile-action-dispatch.js`:
   - Exports a handler that accepts POST from racecontrol `{envelope}` and forwards via the existing WS-sender to the target `to` identity.
   - Returns `{ok: true, forwarded_to: "rcm-tab-plus"}` or `{ok: false, error: "client_not_connected"}`.
   - Registered in the relay's router (likely `comms-link/james/index.js` — enumerate first).
   - DEPLOY PARITY: applied to Bono VPS relay too.
5. Create `rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/ManualActionHandler.kt`:
   - Registered as a listener on CommsLinkClient (Phase 429).
   - On envelope `type == "manual_action_request"`:
     - Validate `payload.device_id` matches this device's id (defense against mis-routing).
     - Look up driver from `DriverRegistry` (Phase 432).
     - Call driver's matching method (zomato.accept(orderId), hyperpure.cancel(deliveryId), blinkit.retry(topupId)) on the driver's coroutine scope.
     - On completion, send `manual_action_ack` envelope with correlation_id + ok + error.
6. Integration test `crates/racecontrol/tests/mobile_manual_action.rs` — spins up an in-memory server, a mock relay, simulates ack.

#### Acceptance

- All 10 tests pass.
- Manual: with Tab Plus offline, clicking Accept in the UI -> 409 -> toast appears correctly (cross-links to 441-03 acceptance).
- Round-trip drill: start Tab Plus, dispatch action via curl, Tab Plus logcat shows `manual_action_request received driver=zomato-partner action=accept`, ack arrives on server within 5s.

#### Commit message

```
feat(441-08): POST /api/v1/mobile/drivers/:device/:driver/action + reception/state

New racecontrol endpoints:
- POST /api/v1/mobile/drivers/:device_id/:driver_id/action — staff JWT,
  validates device online + driver + action, forwards via
  comms-link/james/mobile-action-dispatch.js to agent WS
- GET /api/v1/mobile/reception/state — aggregated device+driver+orders

New Kotlin ManualActionHandler routes manual_action_request to the
correct driver, sends manual_action_ack. New comms-link relay module
mobile-action-dispatch.js (DEPLOY PARITY James + Bono VPS).

Covers: ADMIN-01, ADMIN-02 (action dispatch backbone).
Prerequisite for 441-03/04/05 panels to actually DO something.
```

---

### 441-09-PLAN — UI-REVIEW via gsd-ui-auditor subagent (MANDATORY GATE)

**Goal:** Author `.planning/phases/441-admin-dashboard-reception-view/UI-REVIEW.md` via the `gsd-ui-auditor` subagent. Per CLAUDE.md Subagent Gates: "No frontend phase ships without UI-SPEC.md AND UI-REVIEW.md." UI-REVIEW is the six-pillar audit (layout, brand, a11y, perf, error states, responsive) that gates milestone ship.

**Covers:** Hard gate POST-requirement for ADMIN-01/02/03/06.

**Dependencies:** 441-02 through 441-06 merged (there must be something to audit) + 441-07/08 backend live (so the audit can exercise real data).

**Type:** `checkpoint:subagent` (invoke gsd-ui-auditor).

#### Subagent invocation

Invoke `gsd-ui-auditor` with the UI-SPEC from 441-01 + the implemented files as input. Expected audit pillars:

1. **Layout fidelity** — does the built page match UI-SPEC wireframe and component tree?
2. **Brand** — Racing Red `#E10600` / Asphalt Black `#1A1A1A` / Gunmetal `#5A5A5A` applied to primary CTAs, background, secondary; NO orange `#FF4400`; Montserrat + Enthocentric present.
3. **Accessibility** — keyboard navigation full-page; aria-live on incoming orders; screen reader labels; contrast >= 4.5:1 verified via axe-core or manual DevTools.
4. **Performance** — TTFB + TTI + CLS measurements meet T-12 (≤ 3s first / ≤ 1s repeat); SWR dedup firing; no N+1 fetches.
5. **Error states** — device-offline disables actions with tooltip; 4xx/5xx show toast; empty states for pre-438/439 drivers gracefully rendered; F12 shows no unhandled promise rejections.
6. **Responsive** — 1280 / 1024 / 768 breakpoints per UI-SPEC; audit-log viewer degrades to cards on mobile; no overflow / overlap.

Expected output: `UI-REVIEW.md` with one section per pillar, verdict (PASS / WARN / FAIL), and per-finding severity (P0/P1/P2). P0 = ship-blocker; P1 = fix before milestone close; P2 = backlog.

#### Acceptance

- `UI-REVIEW.md` exists with six pillar sections populated + verdict.
- Zero P0 findings; P1 findings either addressed in-session or explicitly deferred with justification.
- Subagent commit: `docs(441-09): author UI-REVIEW via gsd-ui-auditor`.

#### If P0 found

- STOP. Fix in-session before marking phase complete. Re-run UI-REVIEW to confirm P0 cleared.

#### Commit message

```
docs(441-09): UI-REVIEW.md authored via gsd-ui-auditor

Six-pillar audit: layout, brand, a11y, perf, error states, responsive.
Verdict: <PASS|WARN|FAIL>. P0 findings: N (MUST be 0). P1 findings: N
(addressed or deferred). P2: N (backlog).

Covers: UI-REVIEW subagent gate (CLAUDE.md Subagent Gates section)
```

---

### 441-10-PLAN — Playwright E2E drill (mock driver events round-trip)

**Goal:** Playwright E2E test that (a) starts the admin dashboard in mock-mode (RC_MOCK_MOBILE_RECEPTION=1 serves deterministic fixture driver events), (b) visits /mobile/reception, (c) simulates a Zomato order arriving, (d) clicks Accept, (e) verifies the manual-action dispatch fires + audit row appears, (f) filters the audit viewer and confirms the row matches. Ships as a CI test that runs against a local dev build — no physical devices in CI.

**Covers:** End-to-end verification of ADMIN-01/02/03/06 as an executable regression test.

**Dependencies:** 441-02 through 441-08 all merged.

**Type:** `auto` + `checkpoint:human-verify` at end (visual check of the page AFTER Playwright drills it — CLAUDE.md visual verification rule).

#### Tasks

1. Create `racingpoint-admin/tests/mobile-reception.spec.ts`:
   - Test 1: "Reception page renders with 2 device cards" — navigate, screenshot, assert text "Tab Plus" and "M07".
   - Test 2: "Connection status pill shows 'connected' on successful fetch" — mock server 200 -> pill text.
   - Test 3: "Simulated Zomato order appears in pending section within 3s" — POST to mock state endpoint, advance time, assert row.
   - Test 4: "Click Accept -> optimistic UI + dispatch to action endpoint" — intercept fetch, assert POST body matches `{device_id: "rcm-tab-plus", driver_id: "zomato-partner", action: "accept", payload: {order_id: "..."}, correlation_id: "..."}`.
   - Test 5: "Audit row appears in AuditLogViewer within 5s" — wait for network idle, assert row count increases.
   - Test 6: "Filter by driver=zomato-partner narrows the list" — apply filter, assert only matching rows visible.
   - Test 7: "Device offline disables Accept with tooltip" — mock state with ws_connected=false, hover the button, assert aria-disabled + tooltip text.
   - Test 8: "Screenshot pill opens dialog with sentinel text" — row with sha=sha256:unavailable:flag_secure -> click -> dialog shows "unavailable: FLAG_SECURE".
2. Add mock-mode harness in `racingpoint-admin/src/app/(dashboard)/mobile/reception/__mock__/server.ts` — intercepted by MSW or Playwright route interceptors.
3. Add test to `playwright.config.ts` if not auto-discovered.
4. CI integration: add `npm run test:e2e:mobile-reception` script to package.json.

#### Acceptance

- All 8 Playwright tests pass locally.
- `npm run test:e2e:mobile-reception` completes in < 60s on James .27.
- Visual verification: screenshot artifacts from tests 1-8 look correct (human check — attach to PR).

#### Checkpoint (human-verify)

User opens the page manually AFTER running the Playwright suite to confirm nothing was left in a broken state by test cleanup. Resume signal: "approved".

#### G4 NOT TESTED list

- Real Tab Plus / M07 device — covered in Phase 444 (E2E drills + ToS playbook).
- Cloud admin parity — covered by DEPLOY PARITY post-deploy verify.
- Long-running stability — Phase 444.

#### Commit message

```
test(441-10): Playwright E2E drill for /mobile/reception

8 tests in mock mode (RC_MOCK_MOBILE_RECEPTION=1): page render, device
status, simulated Zomato order, Accept dispatch round-trip, audit row
appearance, filter narrowing, offline-disable, screenshot dialog.
Completes in <60s, no physical devices required.

Covers: ADMIN-01/02/03/06 as executable regression.
Real-device drill: Phase 444.
```

---

## 6. Verification (overall phase)

Before marking phase 441 complete, confirm ALL of:

**Subagent gates (MANDATORY per CLAUDE.md):**
- [ ] UI-SPEC.md exists (from 441-01) and was produced by `gsd-ui-researcher`
- [ ] UI-REVIEW.md exists (from 441-09) and was produced by `gsd-ui-auditor`
- [ ] UI-REVIEW verdict is PASS (or WARN with zero P0, all P1 addressed-or-deferred-with-justification)
- [ ] nyquist-auditor ran over 441-07 (storage) + 441-08 (manual-action dispatch)
- [ ] integration-checker ran across 429 + 435 + 437 + 441 (cross-phase round-trip)
- [ ] mma-audit ran with dual reasoning modes (abstract AND trace-level per CLAUDE.md MMA rule) on the cross-system bridge

**Functional verification:**
- [ ] `/mobile/reception` loads on venue admin :3201 AND cloud admin (DEPLOY PARITY)
- [ ] DeviceStatusPanel shows both devices with live-updating heartbeat
- [ ] Real Zomato order (or mock) appears in pending panel within 5s of emission
- [ ] Accept button -> Tab Plus Zomato tap dispatched within 15s
- [ ] Audit log viewer filters work (device, driver, time) + pagination advances via cursor
- [ ] Screenshot dialog opens with image OR sentinel text (no broken icon, no XSS)

**Deploy verification (per CLAUDE.md deploy rules):**
- [ ] racecontrol binary deployed to venue .23 AND Bono VPS; build_id matches HEAD on both
- [ ] racingpoint-admin frontend rebuilt + deployed; API proxy verified via `curl .../kiosk/api/health/deep`
- [ ] DB migration ran on BOTH venue racecontrol.db and Bono VPS racecontrol.db
- [ ] comms-link relay module deployed on BOTH James .27 and Bono VPS
- [ ] APK reinstalled on both Tab Plus and M07; manual_action_request echo verified
- [ ] Post-deploy curl probes (per `deploy.post_deploy_verification`) all return expected responses
- [ ] Visual verification from NON-server browser (James .27 accessing .23:3201)

**Cascade update verification (CLAUDE.md Cascade update rule):**
- [ ] AUDIT-LOG.md updated to remove "stub" note
- [ ] PROTOCOL.md appended with manual_action_request/ack envelopes
- [ ] MOBILE-RECEPTION.md operator runbook committed
- [ ] No code anywhere depends on old `storage: "stub"` response string
- [ ] Phase 437's minimal admin stub at `/reception/zomato` either (a) removed and replaced with a redirect to `/mobile/reception` or (b) kept with explicit "legacy — use /mobile/reception" banner (decision: redirect; documented in MOBILE-RECEPTION.md)

## 7. Success criteria (measurable)

- T-1 through T-14 all observable by a human using the application (§3).
- All 8 acceptance criteria per-plan met.
- Zero P0 findings in UI-REVIEW.md.
- Zero unhandled promise rejections in browser console during the Playwright drill.
- Admin dashboard WS churn metric < 10 conns/min (if reusing WS per 441-02A Option A).
- Manual action round-trip latency p50 < 5s, p99 < 15s (action click to agent logcat visible).

## 8. Output

After completion, create `.planning/phases/441-admin-dashboard-reception-view/SUMMARY.md` with:
- What shipped (per-plan)
- Which requirements closed (ADMIN-01/02/03/06)
- Which cross-phase gates passed (UI-SPEC, UI-REVIEW, nyquist, integration, MMA)
- What was NOT shipped (ADMIN-04 -> Phase 442, ADMIN-05 -> Phase 443, real Tab Plus/M07 drill -> Phase 444)
- Known risks carried forward
- Deploy state (venue + cloud + all 11 targets)

## 9. Open questions (must be resolved before marking phase complete)

- **OQ-1:** Admin dashboard realtime strategy — WS (Option A) or SWR polling (Option B)? Resolved by 441-02A discovery task. Default assumption: Option B.
- **OQ-2:** Playwright already set up in racingpoint-admin — confirmed via package.json (`@playwright/test ^1.58.2` + `playwright.config.ts` present). No action needed; use existing config.
- **OQ-3:** Screenshot storage mode — does 441-07 store only SHA256 (inline_sha256_only) or actual image bytes? Decision: `inline_sha256_only` for v50.0 (less storage, less risk). Image bytes deferred to a future phase if Uday requests visual audit. Noted in racecontrol.toml `[mobile_reception]` section.
- **OQ-4:** Auth scope for manual-action endpoint — staff JWT sufficient, or manager-only scope needed? Decision: staff JWT (reception counter staff already authorized to take orders). Manager scope deferred to Phase 442 feature-flag UI if Uday requests.
- **OQ-5:** Does the admin dashboard middleware `STAFF_ROUTES` list need to include `/mobile/reception`? Per CLAUDE.md "Never middleware-protect a login page" and existing (dashboard) pattern — YES, `/mobile/*` should be in STAFF_ROUTES since it's behind auth. Verify during 441-02.
- **OQ-6:** Does Phase 437's `racingpoint-admin/app/reception/zomato/page.tsx` stub conflict with the new `racingpoint-admin/src/app/(dashboard)/mobile/reception/page.tsx`? The stub was pre-app-dir `app/` (old router) vs new `src/app/(dashboard)/` — if it exists at `app/reception/zomato/`, it needs removal/migration to avoid dual-URL confusion. Enumerate during 441-02 and either delete or redirect.
- **OQ-7:** One-time lossy upgrade from Phase 435 stub to 441-07 real storage — any ToS-relevant events already discarded that Uday needs to know about? Decision: pre-prod, no ToS risk yet. Document in MOBILE-RECEPTION.md. If Uday wants a pre-upgrade re-ship window, Phase 435 agent can be configured to reset ShippedCursor before the 441 deploy — NOT planned by default.

## 10. G4 NOT TESTED list (carry into phase-complete commit)

- Real Tab Plus / M07 long-running stability — Phase 444 E2E drills.
- Actual Zomato UI consequences of manual actions (real orders) — Phase 444 (staff-led live drill).
- HyperPure / Blinkit panels against REAL drivers — Phases 438 / 439.
- Admin dashboard on tablet (iPad) physical device — manual Uday check post-deploy.
- Cloud parity under real WAN latency — Bono VPS drill required; James can only probe via Tailscale.
- UI-REVIEW P2 findings (backlog) — tracked in SUMMARY.md and will not block ship.

---

**End of PLAN.md (Phase 441)**
