---
phase: 437-zomato-partner-driver
phase_number: 437
milestone: v50.0 rc-agent-mobile
name: "Zomato Partner Driver (P1) — First Production Driver"
status: ready-to-execute
goal: >
  Ship the FIRST production driver in the rc-agent-mobile framework — automating the
  Zomato Partner Android app end-to-end: detect incoming order within 10s, query POS
  rc-agent /kitchen/capacity, auto-accept when can_accept=true (with configurable
  grace window before auto-reject when false), mark ready on admin/POS trigger, forward
  order details to WhatsApp + Discord via existing comms-link bot channels, alert staff
  on session expiry (PersistentSession strategy). THIS IS THE HIGHEST ToS-RISK PHASE IN
  v50.0 — mandatory MMA audit with dual reasoning modes (abstract + trace-level), Uday
  sign-off checkpoint BEFORE live-account drill executes.
requirements: [ZOMATO-01, ZOMATO-02, ZOMATO-03, ZOMATO-04, ZOMATO-05, ZOMATO-06]
depends_on: [433-selector-dsl-hot-reload, 434-credential-abstraction, 435-humanize-layer-audit-log, 436-feature-flag-system]
wave: 5
plan_count: 12
plans:
  - 437-01-PLAN: Zomato selector-map authoring (manual James capture → commit zomato-partner/v<current>/selectors.yaml)
  - 437-02-PLAN: NotificationListenerService + permission + Zomato-specific filter
  - 437-03-PLAN: ZomatoDriver AppDriver impl + lifecycle hooks + feature-flag gating
  - 437-04-PLAN: Order-detection action path — notification -> open Zomato Partner -> navigate to order screen
  - 437-05-PLAN: Capacity query to POS rc-agent (add GET /api/v1/kitchen/capacity endpoint — racecontrol/rc-agent cross-crate change)
  - 437-06-PLAN: Auto-accept / auto-reject action flow with configurable grace window
  - 437-07-PLAN: Mark-ready flow — triggered by admin dashboard or POS kitchen screen
  - 437-08-PLAN: WhatsApp + Discord forwarding (add /order-forward endpoints on Bono VPS bots)
  - 437-09-PLAN: Session-expiry handling — PersistentSession.isSessionValid() false -> pause + alert
  - 437-10-PLAN: Kill-switch compliance — pause_all_drivers=true halts Zomato within 10s
  - 437-11-PLAN: MMA audit (dual reasoning modes) + ToS risk review with Uday sign-off CHECKPOINT
  - 437-12-PLAN: Tab Plus drill — real Zomato Partner test account, 5 simulated orders end-to-end
autonomous: false # Plans 437-01, 437-11, and 437-12 are human-gated (James manual selector capture, Uday ToS sign-off, live-account drill).
files_modified:
  # rc-agent-mobile (Kotlin)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/zomato/ZomatoDriver.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/zomato/OrderDetector.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/zomato/OrderActions.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/zomato/CapacityClient.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/zomato/SessionGuard.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/zomato/ForwardClient.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/notify/ZomatoNotificationListener.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/notify/NotificationListenerPermission.kt
  - rc-agent-mobile/app/src/main/AndroidManifest.xml                              # + permission + <service>
  - rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/<version>/selectors.yaml
  - rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/manifest.json
  - rc-agent-mobile/app/src/main/assets/drivers.json                              # register zomato entry
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/zomato/*.kt
  # rc-agent-mobile docs
  - rc-agent-mobile/docs/DRIVERS/ZOMATO-PARTNER.md
  - rc-agent-mobile/docs/TOS-PLAYBOOK-ZOMATO.md
  # racecontrol server — new capacity endpoint
  - crates/racecontrol/src/api/kitchen_routes.rs                                  # NEW
  - crates/racecontrol/src/api/routes.rs                                          # wire new route
  - crates/racecontrol/src/cafe.rs                                                # KitchenCapacity computation
  - crates/racecontrol/src/cafe_orders.rs                                         # queue depth aggregation
  - crates/rc-common/src/lib.rs                                                   # KitchenCapacityResponse type
  - crates/racecontrol/tests/kitchen_capacity_test.rs                             # NEW
  # Bono VPS bots — new /order-forward endpoints
  - "<bono_vps>:/root/racingpoint-whatsapp-bot/routes/order-forward.js"           # NEW (via comms-link deploy)
  - "<bono_vps>:/root/racingpoint-discord-bot/routes/order-forward.js"            # NEW (via comms-link deploy)
  - comms-link/shared/order-forward-protocol-v1.md                                # JSON contract for both bots
  # Admin dashboard — mark-ready trigger
  - racingpoint-admin/app/reception/zomato/[orderId]/actions.ts                   # server action
  - racingpoint-admin/app/reception/zomato/page.tsx                               # minimal list + mark-ready button
  # POS — kitchen-screen mark-ready trigger
  - web/src/app/kitchen/page.tsx                                                  # mark-ready affordance
  # Audit / planning
  - .planning/phases/437-zomato-partner-driver/SUMMARY.md
  - .planning/phases/437-zomato-partner-driver/MMA-AUDIT.md
  - .planning/phases/437-zomato-partner-driver/UDAY-SIGNOFF.md

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [racecontrol]                  # new /api/v1/kitchen/capacity endpoint
  frontend_rebuild: [admin, web]              # admin reception page + POS kitchen screen
  config_change: "racecontrol.toml [cafe] kitchen_capacity_max (NEW optional key, default 8)"
  db_migration: none                          # read-only endpoint, uses existing cafe_orders table
  infrastructure: >
    (1) Android APK (rc-agent-mobile) reinstall on Tab Plus.
    (2) Zomato Partner APK installed + logged in on Tab Plus with a *test* account (NOT live).
    (3) NotificationListenerService permission granted once post-install (manual Settings toggle).
    (4) Bono VPS bots (racingpoint-whatsapp-bot + racingpoint-discord-bot) restarted after /order-forward route added. pm2 restart <name>.
  data_files: >
    rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/<version>/selectors.yaml (James captures via Phase 433 debug mode and commits BEFORE 437-03 executes).
    rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/manifest.json (driver metadata).
  bat_file: none                              # no Windows script changes
  cloud_parity:
    - racecontrol binary deploy to cloud (Bono VPS) — /api/v1/kitchen/capacity must exist on BOTH envs (CLAUDE.md DEPLOY PARITY)
    - admin dashboard rebuild on cloud
    - whatsapp-bot + discord-bot /order-forward routes deployed on Bono VPS only (bots live there; venue has no copy)
  targets:
    - tab_plus                    # primary driver device (per OQ-5 default — see §9)
    - server_23                   # racecontrol binary with /kitchen/capacity
    - bono_vps                    # cloud racecontrol parity + whatsapp-bot + discord-bot route additions
    - pos_130                     # POS kitchen screen (Next.js rebuild for mark-ready button)
    - james_27                    # comms-link relay (no code change; listening for new ForwardClient -> bot traffic)
  rollback:
    - "Feature-flag OFF: enable_zomato_on_tab_plus=false -> ZomatoDriver.uninstall() within 10s (per Phase 436 FLAG-03)"
    - "Global kill-switch: pause_all_drivers=true halts all drivers within 10s (Phase 436 FLAG-04, re-verified in 437-10)"
    - "Binary rollback: keep racecontrol-prev.exe on server .23 + Bono VPS for 72h per CLAUDE.md rollback-window rule"
    - "Selector-map rollback: remote push UI (Phase 443) can revert to previous YAML version within 10s; for 437 pre-443, manual git revert + APK rebuild"
    - "Session-expired hard stop: driver self-pauses regardless of flags (437-09)"

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip           # Admin dashboard reception view is the subject of Phase 441; 437 adds only a minimal mark-ready button (Phase 441 supersedes). POS kitchen screen addition is a single button — below UI-SPEC threshold.
  ui_auditor: skip              # Same reason — Phase 441 audits the full reception view.
  nyquist_auditor: required     # Capacity-query logic, auto-accept decision, grace-window timer, session-expiry detection are all business logic.
  mma_audit: required           # Cross-system bridge: Kotlin agent -> Zomato Partner app (external, ToS-regulated) -> POS rc-agent -> racecontrol -> WhatsApp+Discord bots. Dual reasoning modes MANDATORY (abstract for architecture + trace-level for accept/reject decision paths). Budget: $5.
  integration_checker: required # Agent -> POS -> racecontrol -> admin -> bot fan-out is the most cross-system flow in the milestone.
  codebase_mapper: skip         # rc-agent-mobile module already mapped in Phase 429; no new top-level module.
  uday_signoff: required        # 437-11 blocks 437-12 (live-account drill) until Uday approves. ToS risk is HIGH.

risks_summary:
  - "ToS risk (HIGH): Zomato Partner ToS prohibits automated interaction. Mitigation: humanize (Phase 435), capacity-query-driven rejections preserve plausibility (auto-accepting 100% looks botlike), business-hours gate, kill-switch ready, MMA + Uday sign-off gate before live drill."
  - "Selector drift: Zomato app updates change resource-ids without notice. Mitigation: Phase 433 selector DSL + versioned per-app-version + fallback chain; Phase 443 remote push UI (future)."
  - "NotificationListenerService permission pain: some OEM skins hide the toggle (Samsung One UI: Settings -> Apps -> Special access -> Device admin apps/Notification access). Mitigation: Phase 431 first-run UX guides user; 437-02 adds an in-app 'open notification-access settings' affordance if agent detects permission revoked."
  - "Android NotificationListenerService can be killed silently by the OS (especially One UI) — a rebind is required. Mitigation: implement the re-bind trick (toggle component enabled -> disabled -> enabled) on detection of stale connection."
  - "Bot-detection evolving on Zomato backend: they may fingerprint Accessibility Service usage. Low-probability but high-impact. Mitigation: humanize delays, capacity-driven decisions (not 100% accept), don't bot outside business hours. If detected, Phase 444 ToS playbook triggers full kill-switch + fall-back-to-manual."
  - "Session cookie expiry silent: PersistentSession's isSessionValid() check is only as good as the signal we can detect via Accessibility. Mitigation: 437-09 treats ANY login-screen detection as session-expired + double-checks with 10s delay before firing alert (prevents flapping)."
  - "Kitchen capacity endpoint must exist (new /api/v1/kitchen/capacity) — 437-05 is a PRE-REQUISITE for 437-06 and is explicitly a racecontrol-crate task (not just agent-side)."
  - "Bono VPS bot endpoints must be added BEFORE 437-08 can run end-to-end. James cannot directly modify /root/racingpoint-whatsapp-bot on Bono VPS — this is a Bono task routed via INBOX.md + comms-link relay (DEPLOY PARITY rule — see 437-08 for handoff format)."
  - "Grace window mechanics: if capacity says 'can_accept: false, reason: queue_full' the driver should wait N seconds (configurable, default 20s) and re-query before rejecting. If it rejects immediately every time, Zomato operations team flags the account. Mitigation: grace window + exponential re-query in 437-06."
  - "iRacing launch logic is unrelated but shares the `pause_all_drivers` killswitch contract — do NOT accidentally couple. 437-10 test explicitly isolates the Zomato driver from unrelated flags."
---

# Phase 437 — Zomato Partner Driver (P1)

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 437 |
| Name | Zomato Partner Driver (P1) — First Production Driver |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | ZOMATO-01, ZOMATO-02, ZOMATO-03, ZOMATO-04, ZOMATO-05, ZOMATO-06 |
| Dependencies | 433 (selectors), 434 (credentials), 435 (humanize + audit), 436 (feature flags) |
| Wave | 5 (runs after Wave 4 = {433, 434, 435, 436}) |
| Status | Ready to execute |
| Autonomous | No — 437-01 (manual selector authoring), 437-11 (Uday sign-off), 437-12 (human-verify drill) are gated |
| Ship test | 5 simulated Zomato orders: ≥ 4 auto-accepted (capacity OK) with WhatsApp+Discord push within 15s; 1 auto-rejected during simulated queue-full; all events in admin audit log + selector.yaml recoverable on drift |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 9)

1. **Detection latency:** Incoming order detected within 10s; auto-accept decision made within 30s.
2. **Capacity honoring:** Capacity query to POS rc-agent honored — no auto-accept when `can_accept: false`.
3. **Forwarding:** Order details forwarded to WhatsApp + Discord via existing comms-link channels.
4. **Mark ready:** `mark ready` trigger from admin dashboard or POS kitchen screen completes in UI within 15s.
5. **Session expiry:** Session-expired state pauses driver + alerts staff; driver does not fail silently.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)

- **T-1 (SC-1, ZOMATO-01):** When a new Zomato order arrives at the Partner app on Tab Plus, the RC Agent Mobile notification changes to "Zomato: order #XXXX detected" within 10s of the notification firing.
- **T-2 (SC-1, ZOMATO-02):** Within 30s of detection, the agent has queried `GET http://<pos_ip>:8080/api/v1/kitchen/capacity` (or server capacity fallback — see OQ-1) and reached an accept-or-reject decision visible in the audit log.
- **T-3 (SC-2, ZOMATO-02/03):** When capacity endpoint returns `can_accept: false`, audit log shows `decision: rejected` AND the Zomato UI does NOT show the order as accepted. Conversely, when `can_accept: true`, audit log shows `decision: accepted` AND the Zomato UI shows the order in "accepted" state.
- **T-4 (SC-3, ZOMATO-05):** Within 15s of accept, WhatsApp group `Racing Point Cafe Orders` receives a message `New Zomato order #XXXX — <item list> — total Rs<N>`. Same message appears on Discord `#cafe-orders` channel.
- **T-5 (SC-4, ZOMATO-04):** Pressing "Mark Ready" on admin dashboard OR POS kitchen screen causes the Zomato Partner UI on Tab Plus to reach "order ready" state within 15s (visible in audit log + observable on the device).
- **T-6 (SC-5, ZOMATO-06):** If the Zomato session cookie expires (simulated by `adb shell pm clear com.application.zomato.merchant`), within one health-check cycle (≤ 5min) the agent: (a) pauses new order processing, (b) emits `SessionExpiredEvent` to the admin dashboard, (c) updates its persistent notification to "Zomato: session expired — staff action required", (d) sends WhatsApp alert to staff group.
- **T-7 (kill-switch, FLAG-04):** Flipping `pause_all_drivers=true` in admin halts the Zomato driver within 10s — next incoming order is NOT processed; audit log shows `paused_by_killswitch`.
- **T-8 (ToS plausibility):** Over a 5-order drill, inter-action delays (notification -> open -> scroll -> accept) have randomization variance ≥ 15% (humanize interceptor from Phase 435 active) and no rapid-fire accept cycles. This is the plausibility proof that differentiates us from a 100%-accept bot.

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/<ver>/selectors.yaml` | Selector map for Zomato Partner app | 80 | notification-filter, order-list-screen, order-detail-screen, accept-button, reject-button, mark-ready-button, login-screen (session-expired signal) |
| `rc-agent-mobile/.../drivers/zomato/ZomatoDriver.kt` | AppDriver impl | 120 | `install() onAppUpdate() healthCheck() uninstall()` + feature-flag check + driver manifest |
| `rc-agent-mobile/.../drivers/zomato/OrderDetector.kt` | Notification + poll fallback | 80 | reads NotificationListener events, filters by package, emits `OrderDetectedEvent` |
| `rc-agent-mobile/.../drivers/zomato/OrderActions.kt` | UI action sequences | 150 | open app, navigate to order, tap accept or reject, with HumanizeInterceptor on every tap |
| `rc-agent-mobile/.../drivers/zomato/CapacityClient.kt` | HTTP client to /kitchen/capacity | 60 | OkHttp GET with 5s timeout, retries 1x, structured fallback on failure |
| `rc-agent-mobile/.../drivers/zomato/SessionGuard.kt` | PersistentSession impl | 80 | detects login-screen presence via Accessibility, emits `SessionExpiredEvent`, integrates CRED-02 |
| `rc-agent-mobile/.../drivers/zomato/ForwardClient.kt` | WhatsApp + Discord push | 50 | POST to Bono VPS bot /order-forward endpoints via comms-link |
| `rc-agent-mobile/.../notify/ZomatoNotificationListener.kt` | NotificationListenerService | 60 | filter for `com.application.zomato.merchant` (or confirmed package), onNotificationPosted emits to OrderDetector |
| `crates/racecontrol/src/api/kitchen_routes.rs` | Server-side capacity endpoint | 80 | GET /api/v1/kitchen/capacity, reads cafe_orders queue depth, config-sourced max_capacity |
| `crates/rc-common/src/lib.rs` (extended) | Shared KitchenCapacityResponse type | +20 | `{can_accept: bool, current_queue_depth: u32, max_capacity: u32, reason: Option<String>}` |
| `comms-link/shared/order-forward-protocol-v1.md` | JSON contract for bot endpoints | 120 | envelope shape, auth (PSK), fields: order_id, items[], total_paise, customer_name_masked, eta_mins |
| `rc-agent-mobile/docs/DRIVERS/ZOMATO-PARTNER.md` | Driver design doc | 200 | architecture, selector strategy, credential model, ToS posture, debug procedure |
| `rc-agent-mobile/docs/TOS-PLAYBOOK-ZOMATO.md` | ToS incident response | 150 | warning detection, kill-switch procedure, fallback-to-manual, contact Zomato support, account-recovery checklist |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| ZomatoNotificationListener.onNotificationPosted | OrderDetector.onNotification | event bus | grep `OrderDetector` in `ZomatoNotificationListener.kt` |
| OrderDetector (order detected) | OrderActions.openAndAccept | coroutine dispatch | grep `OrderActions.openAndAccept` in `OrderDetector.kt` |
| OrderActions (before decision) | CapacityClient.query | HTTP call | grep `CapacityClient.query` in `OrderActions.kt` |
| CapacityClient | racecontrol `GET /api/v1/kitchen/capacity` | OkHttp | grep `kitchen/capacity` in `CapacityClient.kt` |
| OrderActions (tap) | HumanizeInterceptor (Phase 435) | delay injection | grep `HumanizeInterceptor` or `humanize` in `OrderActions.kt` |
| OrderActions (tap) | AuditLog.write (Phase 435) | log write | grep `AuditLog` or `auditLog` in `OrderActions.kt` |
| OrderActions (after accept) | ForwardClient.forward | function call | grep `ForwardClient.forward` in `OrderActions.kt` |
| ForwardClient | Bono VPS whatsapp-bot `/order-forward` | HTTP POST via comms-link | grep `order-forward` in `ForwardClient.kt` |
| ForwardClient | Bono VPS discord-bot `/order-forward` | HTTP POST via comms-link | grep `order-forward` in `ForwardClient.kt` |
| ZomatoDriver.healthCheck | SessionGuard.isSessionValid | function call | grep `isSessionValid` in `ZomatoDriver.kt` |
| SessionGuard (expired) | AdminAlert.sessionExpired | event bus | grep `SessionExpiredEvent` in `SessionGuard.kt` |
| FeatureFlagManager (Phase 436) | ZomatoDriver.install/uninstall | flag watcher | grep `enable_zomato_on_tab_plus` in `ZomatoDriver.kt` |
| FeatureFlagManager `pause_all_drivers` | ZomatoDriver.pause | flag watcher | grep `pause_all_drivers` in `ZomatoDriver.kt` |
| Admin `Mark Ready` button | racecontrol `POST /zomato/mark-ready/<order_id>` | kiosk/admin API | grep `mark-ready` in `racingpoint-admin/` |
| racecontrol `POST /zomato/mark-ready` | ZomatoDriver.markReady via comms-link | relay message | grep `mark-ready` in `crates/racecontrol/src/api/` |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md  # v50.0 section at top
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md  # structure template + protocol envelope
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md  # AppDriver interface (when it exists)
@./.planning/phases/433-selector-dsl-hot-reload/PLAN.md  # selector YAML schema + debug-capture mode
@./.planning/phases/434-credential-abstraction/PLAN.md  # CredentialStrategy + PersistentSession
@./.planning/phases/435-humanize-layer-audit-log/PLAN.md  # HumanizeInterceptor + AuditLog
@./.planning/phases/436-feature-flag-system/PLAN.md  # FeatureFlagManager + pause_all_drivers contract
@./rc-agent-mobile/docs/PROTOCOL.md  # JSON envelope
@./comms-link/docs/PROTOCOL.md  # relay PSK + identity allowlist
@./crates/racecontrol/src/cafe.rs  # current cafe state — read cafe_orders queue logic
@./crates/racecontrol/src/cafe_orders.rs  # order queue schema
@./crates/racecontrol/src/api/routes.rs  # route wiring pattern
@./crates/rc-common/src/lib.rs  # shared types

### Interfaces executors will need

**AppDriver (from Phase 432 — required for 437-03):**

```kotlin
interface AppDriver {
    val manifestId: String
    val supportedDeviceTypes: List<DeviceType>
    val credentialStrategy: CredentialStrategy
    suspend fun install(context: DriverContext): Result<Unit>
    suspend fun onAppUpdate(oldVersion: String, newVersion: String)
    suspend fun healthCheck(): HealthStatus
    suspend fun uninstall()
}
```

**HumanizeInterceptor (from Phase 435 — required for 437-04, 437-06, 437-07):**

```kotlin
class HumanizeInterceptor(private val config: HumanizeConfig) {
    suspend fun beforeAction(actionType: ActionType) { /* delay + rate-limit + business-hours gate */ }
}
```

**FeatureFlagManager (from Phase 436 — required for 437-03, 437-10):**

```kotlin
class FeatureFlagManager {
    fun isEnabled(flag: String): Boolean
    fun onChange(flag: String, listener: (Boolean) -> Unit): Subscription
}
```

**KitchenCapacityResponse (NEW in 437-05 — added to `crates/rc-common/src/lib.rs`):**

```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct KitchenCapacityResponse {
    pub can_accept: bool,
    pub current_queue_depth: u32,
    pub max_capacity: u32,
    pub reason: Option<String>,   // e.g., "queue_full", "closed_hours", "staff_offline"
    pub retry_after_secs: Option<u32>,  // suggested grace window for driver
}
```

**Order forward envelope (NEW in 437-08 — `comms-link/shared/order-forward-protocol-v1.md`):**

```json
{
  "v": 1,
  "type": "order_forward",
  "source": "zomato",
  "order_id": "ZM-2026041801234",
  "items": [{"name": "Paneer Butter Masala", "qty": 1, "price_paise": 36000}],
  "total_paise": 54000,
  "customer_name_masked": "Ar***",
  "eta_mins": 30,
  "detected_at": 1713500000000,
  "accepted_at": 1713500012000
}
```

## 5. Atomic plan breakdown (12 plans)

Each plan is ONE session, ONE commit (or one git-coupled commit pair), ONE acceptance criterion.

---

### 437-01-PLAN — Zomato selector-map authoring (manual James task)

**Goal:** Commit a working `selectors.yaml` for the current Zomato Partner app version, captured on the actual Tab Plus. This is the PRE-REQUISITE for 437-03 and cannot be automated — it requires physical screen inspection.

**Covers:** SELECTOR-01 (specific instance), indirectly enables all ZOMATO-* requirements.

**Dependencies:** 433 (Phase 433 debug-capture mode must be working).

**Type:** `checkpoint:human-action` (truly unavoidable — requires James physically at Tab Plus with Zomato Partner installed).

#### Preconditions

- Tab Plus has Zomato Partner installed. **PACKAGE NAME UNKNOWN (OQ-4 below).** James must first install on Tab Plus and capture the package name via `adb shell pm list packages | grep -i zomato`. Likely candidates (per user prompt context): `com.application.zomato.merchant`, `com.zomato.restaurant`, `com.grofers.zomato-partner`. Cannot plan further until confirmed.
- Zomato Partner account (TEST account, NOT live venue account) logged in. **OQ-2: do we have a test account?**
- Phase 433's debug-capture mode works: trigger it with `adb shell am broadcast -a in.racingpoint.rcagentmobile.DEBUG_CAPTURE` (exact action TBD from Phase 433 plan).

#### Tasks (by James, at Tab Plus)

1. Install Zomato Partner app on Tab Plus via Play Store.
2. Log into TEST account.
3. Run `adb shell pm list packages | grep -i zomato` — record exact package name.
4. For each screen below, bring the screen into view, trigger Phase 433 debug capture, commit the captured YAML stub:
   - **Notification filter screen:** (captured from the ZomatoNotificationListener test harness in 437-02, not via Accessibility — this is a notification-level filter, not a screen selector. Record the exact `android.title`/`android.text` patterns emitted by Zomato Partner when a new order arrives.)
   - **Main dashboard screen:** the screen the app opens on.
   - **Order list screen:** where incoming + active orders are listed.
   - **Order detail screen (incoming, accept/reject buttons visible):** the decision screen.
   - **Order detail screen (accepted, mark-ready button visible):** the ready-trigger screen.
   - **Login screen:** the session-expired signal — we detect this screen's presence to fire `SessionExpiredEvent`.
5. Consolidate into `rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/<version>/selectors.yaml`. Example structure (exact schema from Phase 433):

```yaml
app_package: "com.application.zomato.merchant"   # CONFIRM
app_version: "3.14.2"                            # from `adb shell dumpsys package <pkg> | grep versionName`
captured_at: "2026-04-18T10:00:00+05:30"
captured_by: "james"
screens:
  order_list:
    identifier:
      strategy: resource_id
      value: "com.application.zomato.merchant:id/order_list_container"
      fallback:
        - strategy: content_description
          value: "Orders"
    elements:
      new_order_card:
        strategy: resource_id
        value: "com.application.zomato.merchant:id/incoming_order_card"
      accept_button:
        strategy: text
        value: "Accept"
        fallback:
          - strategy: resource_id
            value: "com.application.zomato.merchant:id/btn_accept"
      reject_button:
        strategy: text
        value: "Reject"
  order_detail:
    identifier: ...
    elements:
      mark_ready_button: ...
  login_screen:
    identifier:
      strategy: resource_id
      value: "com.application.zomato.merchant:id/login_container"
# ... one entry per screen above
```

6. Write `rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/manifest.json`:
```json
{
  "driver_id": "zomato-partner",
  "name": "Zomato Partner",
  "supported_device_types": ["tablet", "phone"],
  "target_packages": ["com.application.zomato.merchant"],
  "credential_strategy": "PersistentSession",
  "selector_map_version": "3.14.2",
  "min_agent_version": "0.1.0"
}
```

7. Commit:

```
feat(437-01): zomato-partner selector-map v<zomato_version>

Manual capture on Tab Plus by james. Covers: order_list, order_detail,
login_screen. 3 fallback chains per critical element. Resource-ids
versioned per Zomato Partner <version>.

Covers: SELECTOR-01 (instance for Zomato)
Blocks: 437-03 depends on this file existing.
```

#### Acceptance

- File `rc-agent-mobile/app/src/main/assets/app-drivers/zomato-partner/<ver>/selectors.yaml` exists + parses (`yq eval '.' <file>` returns no error).
- File `manifest.json` exists and is valid JSON.
- Every element in §T-5 has at least one primary selector and at least one fallback.
- James reports: "captured on Tab Plus with test account; Zomato Partner version = X.Y.Z; package name = Z".

#### G4 NOT TESTED list

- Live order drill — deferred to 437-12.
- Selector-drift handling — tested in 437-03 (driver install) + 437-04 (navigation).
- Selector hot-reload — tested in Phase 433's own acceptance; re-verified in Phase 443.

#### Checkpoint (human-action)

James confirms: "Zomato Partner installed on Tab Plus, test account logged in, selector YAML captured and committed. Package name = <X>. Zomato version = <Y>."

**BLOCKS 437-03.**

---

### 437-02-PLAN — NotificationListenerService + permission + Zomato filter

**Goal:** Agent listens to Android notifications, filters for Zomato Partner's "new order" notifications, emits `OrderDetectedEvent` to the driver. Also handles the permission-grant UX and OS-kill re-bind trick.

**Covers:** partial ZOMATO-01 (order detection within 10s).

**Dependencies:** 437-01 (need confirmed Zomato package name + notification signature).

**Type:** `auto` with one inline manual permission-grant on first run.

#### Tasks

1. Create `rc-agent-mobile/.../notify/ZomatoNotificationListener.kt` extending `NotificationListenerService`:
   - `onNotificationPosted(sbn: StatusBarNotification)`: filter by `sbn.packageName == "com.application.zomato.merchant"` (from 437-01).
   - Extract title + text from `sbn.notification.extras` (`EXTRA_TITLE`, `EXTRA_TEXT`).
   - Match against Zomato "new order" text pattern (captured in 437-01 — likely "New order received" or similar).
   - On match: emit `OrderDetectedEvent(orderId = <extracted or null>, rawNotification = <bundle>, detectedAt = now)` via the driver's event channel (coroutine `Channel<OrderDetectedEvent>`).
   - Write to AuditLog (Phase 435): `"notification.received", source: "zomato", matched: true/false`.

2. Create `rc-agent-mobile/.../notify/NotificationListenerPermission.kt`:
   - Utility: `isGranted(context: Context): Boolean` — checks `Settings.Secure.getString(context.contentResolver, "enabled_notification_listeners")` contains our component.
   - Utility: `openSettingsPage(context: Context)` — starts `Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)` with `FLAG_ACTIVITY_NEW_TASK`.
   - Utility: `rebindIfStale(context: Context)` — implements the classic re-bind trick (toggle `PackageManager` component enabled state false -> true) to force OS to re-bind after kill. Called from `ZomatoDriver.healthCheck()` every 5min.

3. Amend `AndroidManifest.xml`:
   ```xml
   <service android:name=".notify.ZomatoNotificationListener"
            android:label="RC Agent Mobile — Notification Access"
            android:permission="android.permission.BIND_NOTIFICATION_LISTENER_SERVICE"
            android:exported="false">
       <intent-filter>
           <action android:name="android.service.notification.NotificationListenerService" />
       </intent-filter>
   </service>
   ```
   - Permission `BIND_NOTIFICATION_LISTENER_SERVICE` is manifest-only (not runtime-requestable); user grants via Settings toggle.

4. Update `AgentForegroundService.onCreate` to:
   - Check `NotificationListenerPermission.isGranted()`.
   - If not granted: update persistent notification to "RC Agent Mobile — Notification access required (tap to grant)". Tap action opens Settings.
   - If granted: log + proceed.

5. Tests:
   - Unit `ZomatoNotificationListenerTest.filtersZomatoPackage` — mock `StatusBarNotification`, assert non-Zomato packages ignored, Zomato + "new order" title emits `OrderDetectedEvent`.
   - Unit `NotificationListenerPermissionTest.rebindToggle` — mock `PackageManager`, verify `COMPONENT_ENABLED_STATE_DISABLED` then `_ENABLED` called in order.
   - Instrumented (skipped CI): fire a mock Zomato notification via `adb shell cmd notification post -S bigtext -t 'New order' tag 'com.application.zomato.merchant' 1`, assert agent log shows `OrderDetectedEvent`.

#### Acceptance

- Unit tests pass.
- Manual check on Tab Plus: install new APK, grant notification access, trigger a mock notification via adb — logcat shows `OrderDetectedEvent`.
- Permission-revoke test: in Settings, revoke notification access; agent notification updates within 30s to "Notification access required".

#### G4 NOT TESTED list

- Actual Zomato notifications — deferred to 437-12 drill (requires live Zomato account).
- Long-running re-bind behavior over 24h — deferred to Phase 444 E2E drills.

#### Commit message

```
feat(437-02): Zomato NotificationListenerService + permission flow + re-bind

Listens for com.application.zomato.merchant notifications, filters
by "new order" text, emits OrderDetectedEvent. Permission UX:
persistent notification prompts on revoke, opens Settings on tap.
Re-bind trick (component toggle) in healthCheck prevents OS-kill.

Covers: ZOMATO-01 (detection path; full ZOMATO-01 closed by 437-04 end-to-end).
Not tested: live Zomato notifications (deferred to 437-12).
```

---

### 437-03-PLAN — ZomatoDriver AppDriver impl + lifecycle hooks

**Goal:** `ZomatoDriver` class implementing `AppDriver` interface from Phase 432, feature-flag-gated (Phase 436), with all lifecycle hooks wired. This is the driver's registration entry point — it does NOT yet drive orders (that's 437-04); it sets up the skeleton.

**Covers:** partial DRIVER-01..05 instance for Zomato; enables ZOMATO-*.

**Dependencies:** 437-01 (selector map), 437-02 (notification listener), 432 (AppDriver interface), 436 (FeatureFlagManager).

**Type:** `auto`.

#### Tasks

1. Create `rc-agent-mobile/.../drivers/zomato/ZomatoDriver.kt`:
   ```kotlin
   class ZomatoDriver(
       private val flags: FeatureFlagManager,
       private val selectors: SelectorMap,       // from Phase 433
       private val humanize: HumanizeInterceptor,// from Phase 435
       private val auditLog: AuditLog,           // from Phase 435
       private val credStrategy: PersistentSession, // from Phase 434
       private val notificationListener: ZomatoNotificationListener,
       private val scope: CoroutineScope,
   ) : AppDriver {
       override val manifestId = "zomato-partner"
       override val supportedDeviceTypes = listOf(DeviceType.Tablet, DeviceType.Phone)
       override val credentialStrategy = credStrategy

       private var orderDetectorJob: Job? = null

       override suspend fun install(ctx: DriverContext): Result<Unit> {
           // Guard: flag must be on
           if (!flags.isEnabled("enable_zomato_on_tab_plus")) return Result.failure(...)
           // Guard: notification permission
           if (!NotificationListenerPermission.isGranted(ctx.androidContext)) { ... }
           // Start OrderDetector in scope
           orderDetectorJob = scope.launch { OrderDetector(...).run() }
           auditLog.write(DriverEvent.Installed(manifestId))
           return Result.success(Unit)
       }

       override suspend fun onAppUpdate(oldVersion: String, newVersion: String) {
           // Attempt to load new selector-map version
           selectors.loadVersion("zomato-partner", newVersion, fallbackToPrevious = true)
           auditLog.write(DriverEvent.AppUpdated(manifestId, oldVersion, newVersion))
       }

       override suspend fun healthCheck(): HealthStatus {
           val notifOk = NotificationListenerPermission.isGranted(...)
           if (!notifOk) NotificationListenerPermission.rebindIfStale(...)
           val sessionOk = credStrategy.isSessionValid()
           val selectorOk = selectors.currentVersion("zomato-partner") != null
           return HealthStatus(
               healthy = notifOk && sessionOk && selectorOk,
               details = mapOf("notif" to notifOk, "session" to sessionOk, "selector" to selectorOk)
           )
       }

       override suspend fun uninstall() {
           orderDetectorJob?.cancel()
           orderDetectorJob = null
           auditLog.write(DriverEvent.Uninstalled(manifestId))
       }
   }
   ```

2. Feature-flag wiring (per D-03 user decision — feature-flag gating is mandatory from Phase 436):
   - Subscribe to `enable_zomato_on_tab_plus` flag changes.
   - On change true -> false: call `uninstall()` within 10s.
   - On change false -> true: call `install()` within 10s.
   - Subscribe to `pause_all_drivers` flag.
   - On true: set internal `paused` flag; OrderDetector sees it and stops emitting (drops events — no queueing; tested in 437-10).

3. Register driver in `drivers.json` (asset file from Phase 432):
   ```json
   {
     "drivers": [
       {"id": "zomato-partner", "enabled_flag": "enable_zomato_on_tab_plus",
        "manifest_path": "app-drivers/zomato-partner/manifest.json"}
     ]
   }
   ```

4. Tests:
   - Unit `ZomatoDriverTest.installSucceedsWhenFlagOn` — mock flag + permission + selector-load, assert Result.success.
   - Unit `ZomatoDriverTest.installFailsWhenFlagOff` — mock flag off, assert Result.failure.
   - Unit `ZomatoDriverTest.uninstallCancelsDetectorJob` — assert `orderDetectorJob.isCancelled == true`.
   - Unit `ZomatoDriverTest.healthCheckAllHealthy` — all green, HealthStatus.healthy == true.
   - Unit `ZomatoDriverTest.healthCheckDetectsSessionExpired` — credStrategy returns false, HealthStatus.healthy == false.

#### Acceptance

- Unit tests pass.
- APK builds with ZomatoDriver registered.
- On Tab Plus: set `enable_zomato_on_tab_plus=true` via admin -> logcat shows `DriverEvent.Installed` within 10s. Set false -> `DriverEvent.Uninstalled` within 10s.

#### G4 NOT TESTED list

- Live order dispatch — 437-04.
- Long-running healthCheck cadence — 437-12.

#### Commit message

```
feat(437-03): ZomatoDriver AppDriver impl + lifecycle + feature-flag gating

ZomatoDriver plugs into Phase 432 driver framework. install() guarded by
enable_zomato_on_tab_plus flag and notification-permission check. Flag
off -> uninstall within 10s. pause_all_drivers wired for 437-10. Health
check covers notification-listener, session, selector-map. Registered
in drivers.json.

Covers: DRIVER-01/02/03/05 instance for Zomato. Enables ZOMATO-*.
```

---

### 437-04-PLAN — Order-detection action path (notification -> open -> navigate to order screen)

**Goal:** When `OrderDetectedEvent` fires, agent opens the Zomato Partner app (if not already foreground), navigates to the new-order detail screen. Every tap goes through Phase 435 HumanizeInterceptor + AuditLog.

**Covers:** ZOMATO-01 full (10s detection + navigation within additional time to decision).

**Dependencies:** 437-01, 437-02, 437-03.

**Type:** `auto`.

#### Tasks

1. Create `rc-agent-mobile/.../drivers/zomato/OrderDetector.kt`:
   - Consumes the `OrderDetectedEvent` channel from the NotificationListener.
   - Also has a **poll fallback** (every 60s when idle) that reads the order-list screen via Accessibility and detects new cards. This is defense-in-depth if NotificationListenerService is silently killed.
   - For each detected order, dispatches `OrderActions.openAndDecide(orderId)` via scope.launch (bounded concurrency = 1; decisions serialize to prevent double-processing the same order).

2. Create `rc-agent-mobile/.../drivers/zomato/OrderActions.kt` with navigation logic. Initial scope: only `openAndDecide` — actual accept/reject in 437-06.
   ```kotlin
   suspend fun openZomatoIfNotForeground() {
       humanize.beforeAction(ActionType.AppOpen)
       if (!isForeground("com.application.zomato.merchant")) {
           launchPackage("com.application.zomato.merchant")
           awaitScreen("order_list", timeoutMs = 5000)
       }
       auditLog.write(UiAction.AppOpened(manifestId))
   }

   suspend fun navigateToOrder(orderId: String?): OrderScreenContext {
       humanize.beforeAction(ActionType.Navigation)
       // Use selector map's "order_list" screen definition to find incoming order card
       val card = selectors.find("order_list", "new_order_card", orderId = orderId)
       if (card == null) {
           auditLog.write(UiAction.SelectorMiss(manifestId, "new_order_card"))
           throw SelectorMissException(...)
       }
       tap(card)
       humanize.beforeAction(ActionType.Tap)
       awaitScreen("order_detail", timeoutMs = 3000)
       return OrderScreenContext(screenshot = captureScreen(), orderId = extractOrderId(...))
   }
   ```

3. All taps flow through:
   - `humanize.beforeAction(actionType)` — Phase 435's randomized delay + business-hours + rate-limit.
   - `auditLog.write(UiAction)` — Phase 435's audit log with screenshot hash.

4. Wire `ZomatoDriver.install` -> start OrderDetector coroutine (replacing the stub from 437-03).

5. Tests:
   - Unit `OrderDetectorTest.seriesSerializesOrders` — emit 3 OrderDetectedEvents rapid-fire, assert `OrderActions.openAndDecide` called 3 times in sequence (not parallel).
   - Unit `OrderActionsTest.openAndNavigate` — mock Accessibility, assert correct tap sequence, assert humanize called on every action.
   - Unit `OrderActionsTest.selectorMissIsAudited` — selector.find returns null, assert `SelectorMiss` event written + exception thrown.
   - Unit `OrderActionsTest.pollFallbackActivatesOnIdle` — simulate 60s of no notifications, assert poll fires once.

#### Acceptance

- Unit tests pass.
- On Tab Plus with Zomato test account: manually post a fake notification via adb (same as 437-02 test), observe logcat — OrderActions opens Zomato Partner and reaches order_detail screen within 10s. AuditLog file shows tap sequence with humanize delays recorded.

#### G4 NOT TESTED list

- Accept/reject decision — 437-06.
- Real notifications — 437-12.

#### Commit message

```
feat(437-04): OrderDetector + OrderActions navigation path

NotificationListener -> OrderDetector (serialized, with 60s poll
fallback) -> OrderActions.openAndDecide. Opens Zomato Partner if not
foreground, navigates to order_detail screen. Every tap through
Phase 435 HumanizeInterceptor + AuditLog (screenshot hash per
action). SelectorMiss events recoverable for Phase 443 remote push.

Covers: ZOMATO-01 (detection + navigation path).
Not tested: accept/reject decision (437-06), live drill (437-12).
```

---

### 437-05-PLAN — Capacity query to POS rc-agent (add GET /api/v1/kitchen/capacity)

**Goal:** Add the `GET /api/v1/kitchen/capacity` endpoint to the racecontrol server (NOT rc-agent on pods — this is server-level because cafe_orders queue lives in the server DB). Mobile agent queries it via the LAN IP of the server (.23). Build `CapacityClient.kt` in rc-agent-mobile. Define fallback behavior on timeout/error.

**Covers:** partial ZOMATO-02 (capacity query path; accept decision in 437-06).

**Dependencies:** 437-03.

**Type:** `auto` with tdd="true" (capacity-query logic is business logic per CLAUDE.md Subagent Gates + nyquist rule).

#### Design decision: server OR POS rc-agent?

**Decision: racecontrol server (.23:8080).**

Rationale:
- `cafe_orders` queue already lives in the server DB (see `crates/racecontrol/src/cafe_orders.rs` + `cafe_stock.rs`).
- POS rc-agent is a Windows agent running on the POS PC (.130); it doesn't own the orders table — it's a client of the server.
- Adding the endpoint to the server keeps the single source of truth and avoids duplicating queue-depth logic.
- The user prompt acknowledges this is likely: "(or racecontrol)".

Alternative considered (POS rc-agent `/kitchen/capacity`): rejected because it would require POS to re-implement the queue-depth query, creating two paths to truth.

**The user prompt's phrasing "POS rc-agent /kitchen/capacity" is treated as a shorthand for "the service that owns kitchen state"** — which is the racecontrol server. ForwardClient and CapacityClient on the Android agent reach the server via the server's Tailscale-assigned `100.125.108.37:8080` (or LAN `192.168.31.23:8080` — configurable, LAN-first default).

#### Tasks

1. **Server-side (Rust) — `crates/racecontrol/src/api/kitchen_routes.rs`:**
   ```rust
   use axum::{extract::State, Json};
   use serde::Serialize;
   use rc_common::KitchenCapacityResponse;

   pub async fn get_kitchen_capacity(State(state): State<AppState>) -> Json<KitchenCapacityResponse> {
       let queue_depth = state.cafe_orders.current_kitchen_queue_depth().await;
       let max_capacity = state.config.cafe.kitchen_capacity_max.unwrap_or(8);
       let business_open = state.business_hours.is_open_now();
       let (can_accept, reason, retry_after) = match (business_open, queue_depth >= max_capacity) {
           (false, _) => (false, Some("closed_hours"), Some(3600)),
           (true, true) => (false, Some("queue_full"), Some(60)),
           (true, false) => (true, None, None),
       };
       Json(KitchenCapacityResponse {
           can_accept, current_queue_depth: queue_depth, max_capacity,
           reason: reason.map(String::from), retry_after_secs: retry_after,
       })
   }
   ```

2. Wire route in `crates/racecontrol/src/api/routes.rs`:
   - `.route("/api/v1/kitchen/capacity", get(get_kitchen_capacity))` in `public_routes` (no auth — mobile agent queries via LAN and/or Tailscale). **OR** in `service_routes` with X-Service-Key — SECURITY DECISION: since the endpoint leaks business state (queue depth + max capacity), gate it behind `X-Service-Key` (same pattern as rc-agent exec endpoints — CLAUDE.md "Pod HTTP endpoints default to protected"). Mobile agent sends the service key from EncryptedSharedPreferences (key distributed via Phase 431 first-run UX — placeholder for 437).

3. Add `crates/rc-common/src/lib.rs`: `KitchenCapacityResponse` struct (see §4 above).

4. Add config key: `crates/racecontrol/src/config/mod.rs` — `kitchen_capacity_max: Option<u32>` under `[cafe]` section, default 8. Document in `C:\RacingPoint\racecontrol.toml` (venue + Bono VPS).

5. **Cloud parity (CLAUDE.md DEPLOY PARITY):** deploy same binary to Bono VPS. Config `kitchen_capacity_max` applies to cloud too (Bono cafe_orders table syncs bi-directionally — see v21.0 cloud sync).

6. **Android-side (Kotlin) — `rc-agent-mobile/.../drivers/zomato/CapacityClient.kt`:**
   ```kotlin
   class CapacityClient(private val http: OkHttpClient, private val baseUrl: String, private val serviceKey: String) {
       data class Response(val canAccept: Boolean, val queueDepth: Int, val maxCapacity: Int, val reason: String?, val retryAfterSecs: Int?)
       suspend fun query(): Result<Response> {
           val req = Request.Builder()
               .url("$baseUrl/api/v1/kitchen/capacity")
               .header("X-Service-Key", serviceKey)
               .get()
               .build()
           return try {
               val resp = http.newCall(req).await(timeoutMs = 5000)
               if (resp.code != 200) return Result.failure(...)
               val body = Json.decodeFromString<CapacityPayload>(resp.body!!.string())
               Result.success(Response(...))
           } catch (t: Throwable) {
               Result.failure(t)
           }
       }
   }
   ```

7. **Fallback policy on error** (documented in `ZOMATO-PARTNER.md`):
   - Timeout / network error: return `Result.failure`. `OrderActions` treats this as `can_accept: false` by default (fail-closed — safer to reject a Zomato order than accept an un-processable one).
   - HTTP 500: same as timeout.
   - HTTP 4xx (auth): log ERROR + persistent notification update + fail-closed.
   - This is the "never fail silently" enforcement.

8. Tests (TDD per tdd="true"):
   - **Rust (contract test, `crates/racecontrol/tests/kitchen_capacity_test.rs`):** spin up test server + stub cafe_orders with queue_depth = 5 + max_capacity = 8; assert `can_accept: true`. Set queue_depth = 8; assert `can_accept: false, reason: queue_full`. Toggle business_hours.is_open_now = false; assert `can_accept: false, reason: closed_hours`.
   - **Kotlin unit:** `CapacityClientTest.successPath` (mockwebserver returns canned JSON); `CapacityClientTest.timeoutPath` (mockwebserver delays > 5s); `CapacityClientTest.authErrorPath` (mockwebserver 401).
   - **Behavior spec (TDD — write first):**
     - Test 1: `can_accept == true` when queue_depth < max_capacity AND business_open.
     - Test 2: `can_accept == false, reason == "queue_full"` when queue_depth >= max_capacity.
     - Test 3: `can_accept == false, reason == "closed_hours"` when !business_open.
     - Test 4: Client timeout -> Result.failure (for 437-06 to fail-closed).

#### Behavior (tdd="true" block per CLAUDE.md)

```
- Test 1: Queue at 5/8, business open -> can_accept=true, reason=null
- Test 2: Queue at 8/8, business open -> can_accept=false, reason="queue_full", retry_after_secs=60
- Test 3: Queue at 5/8, business closed -> can_accept=false, reason="closed_hours", retry_after_secs=3600
- Test 4: Server unreachable (timeout 5s) -> Client returns Result.failure; caller fail-closed
- Test 5: Server returns 401 -> Client returns Result.failure, log ERROR
```

#### Acceptance

- Rust tests pass: `cargo test -p racecontrol-crate kitchen_capacity_test`.
- Kotlin tests pass: `./gradlew :app:testDebugUnitTest --tests 'CapacityClientTest*'`.
- Manual: `curl -H "X-Service-Key: <key>" http://192.168.31.23:8080/api/v1/kitchen/capacity` returns JSON with the 5 required fields.
- DEPLOY PARITY: same curl against Bono VPS returns JSON.

#### G4 NOT TESTED list

- Accept/reject decision consuming the response — 437-06.
- Actual queue-depth under real load — deferred (out-of-scope for phase 437; will be stress-tested in Phase 444 E2E drill).

#### Commit message

```
feat(437-05): GET /api/v1/kitchen/capacity + Kotlin CapacityClient

Server-side endpoint reads cafe_orders queue depth and business_hours,
returns KitchenCapacityResponse {can_accept, queue_depth, max_capacity,
reason, retry_after_secs}. Service-key auth. New config key [cafe]
kitchen_capacity_max (default 8). Kotlin CapacityClient has 5s timeout,
fail-closed on error (never silent-accept). DEPLOY PARITY: deployed to
venue .23 + Bono VPS.

Covers: ZOMATO-02 (capacity query path). Blocks 437-06.
Not tested: accept/reject consumption (437-06), real load (Phase 444).
```

---

### 437-06-PLAN — Auto-accept / auto-reject with configurable grace window

**Goal:** Wire `OrderActions.openAndDecide` to: query capacity, decide accept/reject, wait out grace window on temporary rejection reasons, tap accept or reject button. This closes the decision loop.

**Covers:** ZOMATO-02, ZOMATO-03.

**Dependencies:** 437-05.

**Type:** `auto` with tdd="true".

#### Tasks

1. Extend `OrderActions.kt`:
   ```kotlin
   suspend fun openAndDecide(orderId: String?): Decision {
       openZomatoIfNotForeground()
       val ctx = navigateToOrder(orderId)
       val realOrderId = ctx.orderId
       humanize.beforeAction(ActionType.Decide)  // longest humanize delay — "staff is reading the order"
       // Capacity decision loop with grace window
       val cfg = driverConfig.acceptGraceWindow  // default 20s
       val maxRetries = driverConfig.acceptGraceRetries  // default 3
       var decision: Decision = Decision.Pending
       repeat(maxRetries) { attempt ->
           val cap = capacityClient.query()
           when {
               cap.isFailure -> { decision = Decision.Reject("capacity_unreachable"); return@repeat }
               cap.getOrNull()!!.canAccept -> { decision = Decision.Accept; return@repeat }
               cap.getOrNull()!!.reason == "queue_full" && attempt < maxRetries - 1 -> {
                   // grace: wait, re-query
                   auditLog.write(DriverEvent.CapacityGraceWaiting(realOrderId, attempt, cap.getOrNull()!!.retryAfterSecs ?: cfg))
                   delay((cap.getOrNull()!!.retryAfterSecs?.toLong() ?: cfg.toLong()) * 1000)
               }
               else -> { decision = Decision.Reject(cap.getOrNull()!!.reason ?: "unknown"); return@repeat }
           }
       }
       if (decision == Decision.Pending) decision = Decision.Reject("grace_exhausted")
       executeDecision(realOrderId, decision)
       return decision
   }

   private suspend fun executeDecision(orderId: String, decision: Decision) {
       when (decision) {
           is Decision.Accept -> {
               val btn = selectors.find("order_detail", "accept_button")
                   ?: throw SelectorMissException("accept_button")
               humanize.beforeAction(ActionType.Tap)
               tap(btn)
               auditLog.write(UiAction.Accepted(manifestId, orderId, captureScreenHash()))
               // trigger ForwardClient in 437-08
               forwardClient.forwardAccepted(orderId, extractOrderDetails())
           }
           is Decision.Reject -> {
               val btn = selectors.find("order_detail", "reject_button")
                   ?: throw SelectorMissException("reject_button")
               humanize.beforeAction(ActionType.Tap)
               tap(btn)
               auditLog.write(UiAction.Rejected(manifestId, orderId, decision.reason, captureScreenHash()))
           }
           Decision.Pending -> error("unreachable")
       }
   }
   ```

2. `driverConfig` (new) — configurable per-driver settings, hot-reloadable from Phase 436:
   - `accept_grace_window_secs`: default 20
   - `accept_grace_retries`: default 3
   - `humanize_decision_delay_ms_mean`: default 8000 (8s — "reading the order")
   - `humanize_decision_delay_ms_stddev`: default 2000

3. Tests (TDD per tdd="true"):

#### Behavior (tdd="true" block)

```
- Test 1: Capacity can_accept=true on first query -> Decision.Accept, accept button tapped, ForwardClient called
- Test 2: Capacity can_accept=false (queue_full), retry_after=30s, next query can_accept=true -> Decision.Accept (grace waited), audit shows 1 CapacityGraceWaiting event
- Test 3: Capacity can_accept=false (queue_full) 3 times in a row -> Decision.Reject("grace_exhausted"), reject button tapped
- Test 4: Capacity can_accept=false (closed_hours) -> Decision.Reject("closed_hours") IMMEDIATELY (no grace — business-hours failures don't improve with waiting)
- Test 5: Capacity.query() returns Result.failure -> Decision.Reject("capacity_unreachable") (fail-closed)
- Test 6: SelectorMissException on accept button -> logged, no action on Zomato UI, order_id re-queued for retry (bounded 2x then abandoned)
- Test 7: Humanize delay on tap verified — tap events not less than mean - 2*stddev apart
```

4. Tests (unit, `OrderActionsDecisionTest*`):
   - `decidesAcceptWhenCapacityOk`
   - `waitsGraceThenAcceptsWhenQueueClearsDuringGrace`
   - `rejectsAfterGraceExhausted`
   - `rejectsImmediatelyOnClosedHours`
   - `rejectsOnCapacityUnreachable`
   - `reQueuesOnSelectorMissThenAbandons`
   - `humanizeDelayEnforced`

#### Acceptance

- Unit tests all pass.
- Manual test on Tab Plus with mock order-screen (via debug harness): inject `capacity can_accept=true`, observe accept button tapped within 30s total (includes humanize "decide" delay). Inject `queue_full` with 10s `retry_after` -> observe 10s wait + re-query + accept. Inject `queue_full` persistent -> observe reject after 3*10s grace exhausted.

#### G4 NOT TESTED list

- Real Zomato UI accept flow — 437-12.
- WhatsApp + Discord forwarding integration — 437-08.
- Mark-ready after accept — 437-07.

#### Commit message

```
feat(437-06): auto-accept/reject with configurable grace window

OrderActions.openAndDecide queries CapacityClient and either (a) taps
accept, (b) waits grace window and re-queries on queue_full, or (c)
taps reject with reason. Fail-closed on CapacityClient errors. Grace
does NOT apply to closed_hours (waiting won't help). Max 3 grace
retries default. Every tap humanized + audited.

Covers: ZOMATO-02, ZOMATO-03.
Not tested: ForwardClient (437-08), mark-ready (437-07), live drill (437-12).
```

---

### 437-07-PLAN — Mark-ready flow (triggered by admin dashboard or POS kitchen screen)

**Goal:** Close the order lifecycle — when kitchen finishes cooking and staff taps "Mark Ready", the Zomato Partner UI on Tab Plus receives the mark-ready tap within 15s. Trigger paths: (a) admin dashboard button, (b) POS `web/src/app/kitchen/page.tsx` button.

**Covers:** ZOMATO-04.

**Dependencies:** 437-06 (accept path active).

**Type:** `auto` with tdd="true".

#### Tasks

1. **Server-side (racecontrol):** new route `POST /api/v1/zomato/mark-ready/:order_id` (staff JWT auth — this is a staff action):
   - Looks up the order's assigned driver device (Tab Plus = `rcm-tab-plus`).
   - Sends a `mark_ready_request` envelope via comms-link relay to that device's WS connection (re-use Phase 429 protocol envelope).

2. **Kotlin agent:** in `ZomatoDriver`, handle incoming `mark_ready_request`:
   - Parse `order_id`.
   - Dispatch `OrderActions.markReady(orderId)` via scope.
   - `OrderActions.markReady`:
     - `openZomatoIfNotForeground()`.
     - `navigateToOrder(orderId)` — but to the ACCEPTED-order detail screen (different from the incoming-order detail screen in 437-04).
     - `selectors.find("order_detail_accepted", "mark_ready_button")`.
     - `humanize.beforeAction(ActionType.MarkReady)` — shorter delay (~1-3s, not the "reading" delay).
     - `tap(button)`.
     - `auditLog.write(UiAction.MarkedReady(manifestId, orderId, screenshotHash))`.

3. **Admin UI (minimal — Phase 441 adds full view):** `racingpoint-admin/app/reception/zomato/page.tsx` — list of accepted orders with "Mark Ready" button. Server action posts to `/api/v1/zomato/mark-ready/:order_id`.

4. **POS kitchen screen:** `web/src/app/kitchen/page.tsx` — for each Zomato order in the in-progress list, add a "Mark Ready" button firing the same endpoint.

5. Tests:

#### Behavior (tdd="true" block)

```
- Test 1: POST /api/v1/zomato/mark-ready/ZM-123 (valid staff JWT) -> 200, relay message enqueued to rcm-tab-plus
- Test 2: POST without staff JWT -> 401
- Test 3: POST for unknown order -> 404
- Test 4: Agent receives mark_ready_request -> OrderActions.markReady called, mark_ready_button tapped in < 15s (measured end-to-end)
- Test 5: Agent receives request but selector miss -> SelectorMiss audit event + alert to staff; no silent failure
- Test 6: Agent receives request while paused (pause_all_drivers=true) -> request queued OR dropped per killswitch policy (see 437-10 decision) — document the chosen policy in ZOMATO-PARTNER.md
```

6. **Policy decision (must be recorded in ZOMATO-PARTNER.md):** when `pause_all_drivers=true`, mark-ready requests are **dropped** (not queued). Rationale: staff are using the killswitch because something is wrong; queueing mark-ready signals could fire bad actions when kill is lifted. Staff can re-press "Mark Ready" once pause is cleared.

#### Acceptance

- All tests pass.
- Manual: admin dashboard -> click Mark Ready on a test order -> Tab Plus Zomato UI shows mark-ready tap within 15s (audit log + visible).
- Same from POS kitchen screen.

#### G4 NOT TESTED list

- Real Zomato mark-ready UI consequences — 437-12.
- Long-running reliability — 437-12 + Phase 444.

#### Commit message

```
feat(437-07): mark-ready trigger from admin dashboard + POS kitchen screen

New racecontrol route POST /api/v1/zomato/mark-ready/:order_id sends a
mark_ready_request envelope via comms-link to rcm-tab-plus. Agent
ZomatoDriver handles the envelope -> OrderActions.markReady -> humanize
+ tap mark_ready_button. Minimal admin + POS buttons wired (full admin
view deferred to Phase 441). Killswitch policy: drop (don't queue).

Covers: ZOMATO-04.
Not tested: real Zomato mark-ready UI consequences (437-12).
```

---

### 437-08-PLAN — WhatsApp + Discord forwarding

**Goal:** On accept, agent forwards order details to Bono VPS WhatsApp bot + Discord bot via comms-link. Requires NEW endpoints on the bots (they don't exist yet — inferred from `MEMORY.md` which places the bots at `/root/racingpoint-whatsapp-bot` and `/root/racingpoint-discord-bot` on Bono VPS).

**Covers:** ZOMATO-05.

**Dependencies:** 437-06.

**Type:** `auto` + cross-repo handoff to Bono.

#### Tasks

1. **Define protocol — `comms-link/shared/order-forward-protocol-v1.md`** (see §4 envelope above). PSK-authenticated, JSON POST.

2. **Bono VPS bot additions (routed via INBOX.md as Bono cannot be James-touched):**

   This plan has a **handoff step** for Bono — James writes an `INBOX.md` entry describing the required endpoint and protocol, Bono implements + deploys on Bono VPS. James verifies via `curl`.

   Handoff content (written to `comms-link/INBOX.md` per CLAUDE.md Comms rule):
   ```markdown
   ## YYYY-MM-DD HH:MM IST — from james

   Phase 437-08 requires new endpoints on both Bono VPS bots:

   ### WhatsApp bot (racingpoint-whatsapp-bot)
   - New route: `POST /order-forward` (listen on existing bot port).
   - Auth: `Authorization: Bearer $COMMS_PSK` header.
   - Request body: see `comms-link/shared/order-forward-protocol-v1.md`.
   - On receipt: post to WhatsApp group `Racing Point Cafe Orders` with formatted message:
     `Zomato order #<order_id>\n<items (one per line)>\nTotal: Rs<N>\nETA: <mins> min`
   - Response: 200 + `{ok: true, message_id: "..."}` or 4xx/5xx.
   - Spec doc: `comms-link/shared/order-forward-protocol-v1.md` (in racecontrol repo, auto-pulled to Bono via standing sync).

   ### Discord bot (racingpoint-discord-bot)
   - Same route shape.
   - On receipt: post to `#cafe-orders` channel with the same message (Discord-markdown-formatted).

   ### Deploy
   - pm2 restart both bots.
   - Verify: `curl -H 'Authorization: Bearer <PSK>' -X POST https://<bono_vps>/order-forward -d '<test payload>'` returns 200 on both.

   I'll verify from here once you confirm.
   ```

   Also: `git push` + WS send per CLAUDE.md comms rule.

3. **Kotlin — `rc-agent-mobile/.../drivers/zomato/ForwardClient.kt`:**
   ```kotlin
   class ForwardClient(
       private val http: OkHttpClient,
       private val whatsappUrl: String,   // https://<bono_vps>/whatsapp-bot/order-forward
       private val discordUrl: String,    // https://<bono_vps>/discord-bot/order-forward
       private val psk: String,
       private val auditLog: AuditLog,
   ) {
       suspend fun forwardAccepted(orderId: String, details: OrderDetails): ForwardResult {
           val payload = buildJson(orderId, details, acceptedAt = now())
           val whatsappOk = postWithRetry(whatsappUrl, payload, timeoutMs = 5000, retries = 2)
           val discordOk = postWithRetry(discordUrl, payload, timeoutMs = 5000, retries = 2)
           auditLog.write(DriverEvent.OrderForwarded(orderId, whatsappOk, discordOk))
           // Not fatal if one fails — never block the order flow on forwarding
           return ForwardResult(whatsappOk, discordOk)
       }
   }
   ```
   - **Fail-non-blocking:** forwarding failures don't block the order. Zomato accept already happened; the customer sees the order accepted. Forwarding is for our ops visibility only.
   - **Retry:** 2 retries with 1s + 2s backoff on 5xx or network error.

4. **PII masking (privacy):** `customer_name_masked` field — agent masks customer name before sending (e.g., "Arjun S" -> "Ar****"). This is partially DPDP compliance — see CLAUDE.md GDPR erase contract rule. Specifically: only the first 2 chars retained; the order-forward message contains no phone number, no address.

5. Tests:
   - Unit `ForwardClientTest.successPath` — mockwebserver, both return 200, assert `ForwardResult(true, true)` + audit written.
   - Unit `ForwardClientTest.oneFails` — whatsapp 500, discord 200, assert `ForwardResult(false, true)` but order is NOT re-processed (not a retryable failure from Zomato's perspective).
   - Unit `ForwardClientTest.masksCustomerName` — "Arjun Singh" in input, "Ar****" in payload.
   - Integration: after 437-06 accept, verify ForwardClient is called with correct payload.

#### Acceptance

- Unit tests pass.
- Manual: trigger a test accept on Tab Plus, verify WhatsApp group receives message + Discord channel receives message within 15s of accept.
- PII check: message contains NO phone, NO unmasked name, NO address.

#### G4 NOT TESTED list

- Message deliverability during WhatsApp rate-limiting — out of scope for 437; Phase 444 drill tests sustained load.
- Bono's bot implementation — James verifies interface contract only; Bono owns implementation.

#### Commit message

```
feat(437-08): WhatsApp + Discord order-forward on accept

ForwardClient POSTs to Bono VPS bot /order-forward endpoints with
PSK auth. customer_name_masked (PII minimization). Fail-non-blocking
(never blocks order flow). 2 retries with backoff. Requires Bono to
add /order-forward endpoints on both bots (INBOX handoff sent).
Protocol spec: comms-link/shared/order-forward-protocol-v1.md.

Covers: ZOMATO-05.
Not tested: bot implementation (Bono task), sustained rate (Phase 444).
```

---

### 437-09-PLAN — Session-expiry handling

**Goal:** When Zomato Partner session expires (login screen detected), driver pauses new-order processing, emits SessionExpiredEvent, updates persistent notification, sends WhatsApp alert — does NOT fail silently.

**Covers:** ZOMATO-06.

**Dependencies:** 434 (PersistentSession), 437-03 (ZomatoDriver health-check).

**Type:** `auto` with tdd="true".

#### Tasks

1. Create `rc-agent-mobile/.../drivers/zomato/SessionGuard.kt`:
   ```kotlin
   class SessionGuard(
       private val selectors: SelectorMap,
       private val accessibility: AccessibilityBridge,
       private val auditLog: AuditLog,
       private val alerter: AdminAlerter,
       private val forwardClient: ForwardClient,  // for WhatsApp staff alert
   ) : PersistentSession {

       override suspend fun isSessionValid(): Boolean {
           // Signal 1: if Zomato Partner is currently foreground AND login_screen selector matches -> invalid.
           if (isForeground("com.application.zomato.merchant")) {
               val loginScreenPresent = selectors.find("login_screen", "identifier") != null
               if (loginScreenPresent) return false
           }
           // Signal 2: opportunistic — open Zomato briefly once per 5 min during healthCheck to verify.
           // (BUT: this incurs ToS risk — pinging the app silently could look botlike. Mitigation:
           // only run during business hours, with full humanize delay; skip during active order processing.)
           return true
       }

       suspend fun handleExpiry() {
           // Debounce: require 2 consecutive false reads 10s apart to prevent flap
           if (!confirmedExpiryAfterDebounce()) return
           auditLog.write(DriverEvent.SessionExpired(manifestId = "zomato-partner"))
           alerter.sessionExpired(manifestId = "zomato-partner")
           forwardClient.staffAlert(
               "Zomato session expired on Tab Plus. Log in again to resume auto-orders."
           )
           // ZomatoDriver observes this via healthCheck returning unhealthy -> pauses new-order processing
       }
   }
   ```

2. Integrate with `ZomatoDriver.healthCheck`:
   - Every 5 min, call `SessionGuard.isSessionValid()`.
   - If false: set driver's internal `paused` flag -> OrderDetector drops new events.
   - Fire `SessionGuard.handleExpiry()`.

3. Persistent notification update: `AgentForegroundService.updateNotification("Zomato: session expired — staff action required")`.

4. Debounce logic: 2 reads 10s apart to prevent flap (e.g., app transiently shows login in a loading state). Documented in `ZOMATO-PARTNER.md`.

5. Tests:

#### Behavior (tdd="true" block)

```
- Test 1: login_screen selector matches -> isSessionValid() returns false
- Test 2: login_screen doesn't match, normal screen visible -> isSessionValid() returns true
- Test 3: handleExpiry() fires audit + admin alert + WhatsApp alert + pauses driver within 10s
- Test 4: Debounce — one false read followed by one true read within 10s -> no expiry event fired
- Test 5: Session recovers (staff logs back in) -> next healthCheck returns true -> driver unpaused, audit SessionResumed
- Test 6: Expired during active order -> active order completes (don't abort mid-tap), but next order blocked
```

6. Tests (unit, `SessionGuardTest*`):
   - `detectsExpiryOnLoginScreen`
   - `noExpiryOnNormalScreen`
   - `debouncePreventsFlap`
   - `handleExpiryFiresAllAlerts`
   - `sessionResumesAfterRelogin`

#### Acceptance

- All tests pass.
- Manual on Tab Plus: `adb shell pm clear com.application.zomato.merchant` -> within 5min, admin dashboard shows SessionExpiredEvent + WhatsApp group receives alert + persistent notification updated. `ZomatoDriver.healthCheck().healthy == false`.
- Recovery: log back in on Tab Plus -> within 5min, audit shows SessionResumed, driver.healthy=true.

#### G4 NOT TESTED list

- Real 24h+ session persistence — Phase 444.
- Interaction with other drivers' session guards — out of scope (Zomato is only P1 driver).

#### Commit message

```
feat(437-09): SessionGuard + PersistentSession integration + staff alert

SessionGuard detects Zomato login screen -> isSessionValid()=false.
Debounce (2x 10s) prevents flap. On expiry: audit + admin alert +
WhatsApp staff alert + persistent notification update + driver paused.
Active order completes; next order blocked. Recovery observed via
healthCheck cycle.

Covers: ZOMATO-06.
Not tested: 24h+ persistence (Phase 444).
```

---

### 437-10-PLAN — Kill-switch compliance

**Goal:** Flipping `pause_all_drivers=true` halts Zomato driver within 10s. Specifically: no new orders processed, but active in-flight order completes (don't abort mid-tap — worse for ToS than finishing cleanly).

**Covers:** FLAG-04 (instance for Zomato); satisfies ToS-playbook killswitch requirement.

**Dependencies:** 436 (FLAG-04), 437-03 (driver installed).

**Type:** `auto` with tdd="true".

#### Tasks

1. In `ZomatoDriver`, subscribe to `pause_all_drivers` flag (already stubbed in 437-03). On true:
   - Set internal `paused = true`.
   - OrderDetector checks `paused` on every emit -> drops new events (logs as `DriverEvent.KillswitchPaused(orderId)` so we know WHAT was dropped).
   - Active in-flight order (if any) is **allowed to complete** — the decision loop finishes its current iteration. Rationale: aborting mid-tap is worse than finishing one order cleanly. The killswitch prevents FURTHER orders.
   - Document this policy in `ZOMATO-PARTNER.md` + `TOS-PLAYBOOK-ZOMATO.md`.

2. On false (killswitch released):
   - `paused = false`.
   - OrderDetector resumes — next incoming order processed normally.
   - Missed-during-pause orders are NOT retroactively processed (would be too late anyway — Zomato's 5-min acceptance window passed).

3. Isolation test: Zomato driver listens ONLY for `pause_all_drivers` and `enable_zomato_on_tab_plus`. It does NOT react to e.g. `enable_hyperpure_on_m07` or iRacing-related flags. This prevents accidental coupling — see risks_summary.

4. Tests:

#### Behavior (tdd="true" block)

```
- Test 1: pause_all_drivers=true -> within 10s, OrderDetector drops any newly arriving events with DriverEvent.KillswitchPaused
- Test 2: Active order in-flight when killswitch flips -> that order's decision loop completes normally
- Test 3: pause_all_drivers=false released -> next incoming event processed
- Test 4: Orders that arrived DURING pause are NOT retroactively processed
- Test 5: Flipping enable_hyperpure_on_m07 has NO effect on Zomato driver (isolation)
- Test 6: Flipping enable_zomato_on_tab_plus=false while pause_all_drivers=true -> uninstall() runs (flag-specific wins over killswitch for uninstall path)
```

5. Tests (unit, `ZomatoKillswitchTest*`):
   - `killswitchBlocksNewOrders`
   - `killswitchAllowsInflightToComplete`
   - `killswitchReleaseResumes`
   - `missedOrdersNotRetroactivelyProcessed`
   - `unrelatedFlagsIgnored`
   - `featureFlagOffWinsOverPause`

#### Acceptance

- All tests pass.
- Manual: start driver; mock 3 incoming orders with 5s spacing; at T=2s, flip killswitch -> verify only the first order processes; flip back at T=30s -> next order processes normally.

#### G4 NOT TESTED list

- Multi-device killswitch (HyperPure, Blinkit) coupling — out of scope for 437; verified at milestone in Phase 444.

#### Commit message

```
feat(437-10): killswitch compliance + in-flight completion policy

pause_all_drivers=true halts Zomato within 10s (verified by test).
Policy: active in-flight order completes; new orders dropped with
DriverEvent.KillswitchPaused. Missed orders NOT retroactively
processed. Isolation from unrelated flags verified.

Covers: FLAG-04 (Zomato instance).
Not tested: multi-driver coupling (Phase 444).
```

---

### 437-11-PLAN — MMA audit (dual reasoning modes) + Uday sign-off

**Goal:** Run the Unified MMA Protocol v3.0 with **dual reasoning modes** (abstract + trace-level) against the Zomato driver implementation. THEN gate live-drill execution on explicit Uday approval of the MMA findings + ToS playbook.

**Covers:** ToS risk mitigation + CLAUDE.md "MMA audit is MANDATORY before deploying new cross-system bridges" rule (this is 3-language, 4-process: Kotlin agent -> Zomato external -> racecontrol -> bots).

**Dependencies:** 437-01 through 437-10.

**Type:** `checkpoint:decision` (Uday approves or rejects).

#### Tasks

1. **MMA audit setup:**
   - Artifact to audit: all of `rc-agent-mobile/.../drivers/zomato/*.kt` + `crates/racecontrol/src/api/kitchen_routes.rs` + `comms-link/shared/order-forward-protocol-v1.md` + selector YAML.
   - Command: `node scripts/multi-model-audit.js` (default 5 models consensus mode).
   - Budget: $5 (per CLAUDE.md; request Uday for higher if first run unconvincing).

2. **Dual reasoning modes (CLAUDE.md requires):**
   - Round 1 (non-thinking / abstract): DeepSeek V3.2, Qwen3 Coder, Grok Code Fast, Mistral Medium, Nemotron 3 Super. Prompt: "Audit this Zomato driver for architectural risks: ToS exposure, lock-held-across-await, fail-open vs fail-closed, cross-boundary serialization, privacy leakage, killswitch correctness."
   - Round 2 (thinking / trace-level): DeepSeek R1 0528, GPT-5.4 Nano, Kimi K2.5, MiMo v2 Pro, Gemini 2.5 Flash. Prompt: "Trace the exact decision path for: (a) new order arrives at 23:59 IST (business-hours boundary), (b) capacity.query() times out at 4.999s, (c) two identical notifications fire 200ms apart for the same order_id. For each, what's the exact final state of the Zomato UI, the audit log, and the WhatsApp message?"
   - Round 3 if consensus < 3/5: add 2 more models + repeat worst-flagged area.

3. **Deterministic pre-check (Step 4 of MMA Convergence Engine — before model adversarial):**
   - `cargo test -p racecontrol-crate kitchen_capacity_test` passes.
   - `./gradlew :app:testDebugUnitTest --tests 'drivers.zomato.*'` passes.
   - `node comms-link/test/security-check.js` passes (SEC-GATE-01).
   - `curl http://<bono_vps>/whatsapp-bot/order-forward` returns 200 on a canned test payload (validates 437-08 handoff completed).

4. Write findings to `.planning/phases/437-zomato-partner-driver/MMA-AUDIT.md`:
   - Model list + rounds + cost.
   - Consensus P0/P1/P2 findings.
   - Fixes applied during audit vs deferred to follow-up plans.
   - Structural vs tactical findings.

5. **ToS playbook completion:** update `rc-agent-mobile/docs/TOS-PLAYBOOK-ZOMATO.md` with MMA's ToS-specific findings. At minimum:
   - Indicators: Zomato warning message, account-state change, unusual acceptance rate flagged.
   - Action on warning: (1) flip `pause_all_drivers=true`, (2) staff revert to manual accept/reject for 7 days, (3) contact Zomato Partner support, (4) after 7 clean days, resume with reduced auto-accept ratio (80% capacity-driven rejections minimum).
   - Escalation: Uday is final authority on when to resume.

6. **Uday sign-off (checkpoint:decision):**

   Present to Uday:
   - Executive summary of 437 implementation (1 page).
   - MMA audit findings (all P0/P1 items fixed, P2 items listed).
   - ToS-playbook (reviewed by Uday).
   - Recommended max orders/day for initial drill (suggest: 5 simulated in 437-12, max 20/day for first week in production).
   - Request: explicit approval to proceed with 437-12 (live-account drill) + production flag-on after drill.

#### Acceptance

- MMA-AUDIT.md exists with at least 2 rounds + dual reasoning modes + ≥ 3 vendor families.
- All P0/P1 consensus findings fixed or explicitly accepted with documented rationale.
- Deterministic pre-check all green.
- TOS-PLAYBOOK-ZOMATO.md reviewed + signed off (Uday's sign-off recorded in UDAY-SIGNOFF.md).
- UDAY-SIGNOFF.md exists with signed text: "Approved for 437-12 drill. Max orders/week: <N>. Kill-switch threshold: <indicator>. Uday Singh. <date>."

#### Checkpoint (decision)

Present Uday with:
1. MMA-AUDIT.md
2. TOS-PLAYBOOK-ZOMATO.md
3. Executive summary
4. 3 options:
   - **Option A (approve):** proceed with 437-12 drill, production flag-on after. Initial cap: 20 orders/week.
   - **Option B (narrow-approve):** proceed with 437-12 drill ONLY (read-only-like — drill, no production flag-on).
   - **Option C (reject):** do not proceed. Document what would need to change.

Resume signal: Uday types one of "A", "B", "C" with any conditions.

**BLOCKS 437-12.**

#### G4 NOT TESTED list

- Live drill — 437-12 (after sign-off).
- Production flag-on — post-drill.

#### Commit message

```
docs(437-11): MMA audit + ToS playbook + Uday sign-off recorded

Dual reasoning mode MMA (abstract + trace-level). 2 rounds, N models,
N consensus findings, all P0/P1 addressed. TOS-PLAYBOOK-ZOMATO.md
complete. UDAY-SIGNOFF.md records approval path (A/B/C) + initial
orders/week cap + killswitch threshold indicator.

Covers: ToS risk mitigation + CLAUDE.md MMA rule + Uday sign-off gate.
Blocks: 437-12.
```

---

### 437-12-PLAN — Tab Plus drill (real Zomato Partner test account, 5 simulated orders)

**Goal:** Full end-to-end drill on Tab Plus with a REAL Zomato Partner test account. 5 simulated orders covering accept, reject-by-capacity, grace-window-accept, mark-ready, session-expiry paths. This is the ship gate.

**Covers:** ALL of ZOMATO-01..06 (verification, not net-new implementation).

**Dependencies:** 437-01 through 437-11 (especially 437-11 sign-off).

**Type:** `checkpoint:human-verify` (physical device + live Zomato test account + timing measurements).

#### Preconditions

- Uday approved via 437-11 checkpoint.
- Tab Plus running latest rc-agent-mobile APK with selector-map from 437-01.
- Zomato Partner test account logged in. (If no test account: STOP — cannot proceed. OQ-2.)
- POS PC online (.130); racecontrol server online (.23:8080) with `/api/v1/kitchen/capacity` deployed.
- Bono VPS bots running with `/order-forward` endpoints (437-08 handoff complete).
- `pause_all_drivers=false`, `enable_zomato_on_tab_plus=true`.
- Admin dashboard + POS kitchen screen open to reception view.
- Humanize config on default; audit-log tail visible via `adb shell logcat | grep AuditLog`.

#### Drill script

**Simulation:** Zomato Partner "test orders" are limited on the Partner app. For this drill, we use **two order sources**:
- (a) Real test orders through the Zomato Partner sandbox (if available — confirm with Zomato Partner support).
- (b) Fallback: craft notifications via adb matching Zomato's notification signature (validates the agent path end-to-end except for the real Zomato UI response).

Drill flow (run in this order):

1. **DRILL-1 (accept path, capacity OK):**
   - Trigger order (real or simulated).
   - Expected: within 10s, persistent notification updates to "Zomato: order #X detected".
   - Expected: within 30s, order accepted on Zomato UI.
   - Expected: within 15s of accept, WhatsApp group receives message + Discord channel receives message.
   - Record: detection latency, accept latency, forward latency.

2. **DRILL-2 (reject path, closed hours):**
   - Flip business-hours config to "closed" temporarily (or set `kitchen_capacity_max=0`).
   - Trigger order.
   - Expected: agent rejects immediately with reason=closed_hours (or queue_full).
   - Expected: no WhatsApp forward (rejects don't forward — confirm policy in doc).
   - Record: rejection latency, reason logged.

3. **DRILL-3 (grace-window accept):**
   - Set `kitchen_capacity_max=1`; manually insert 1 queue row; trigger order.
   - After 10s (during grace), delete the queue row.
   - Expected: agent waits grace window, re-queries, finds capacity OK, accepts.
   - Record: grace-retry count in audit log.

4. **DRILL-4 (mark-ready):**
   - On an accepted order (DRILL-1 result), tap "Mark Ready" on admin dashboard.
   - Expected: within 15s, Zomato UI shows order marked ready.
   - Repeat from POS kitchen screen on a second order.
   - Record: mark-ready latency.

5. **DRILL-5 (session-expiry):**
   - `adb shell pm clear com.application.zomato.merchant` to clear session.
   - Expected: within 5min healthCheck cycle, admin dashboard shows SessionExpiredEvent; WhatsApp staff alert; persistent notification updated.
   - Log back in on Tab Plus.
   - Expected: next healthCheck resumes driver.
   - Trigger order.
   - Expected: order processed normally.

6. **DRILL-6 (killswitch):**
   - Flip `pause_all_drivers=true` in admin.
   - Trigger order.
   - Expected: agent drops event with DriverEvent.KillswitchPaused; no Zomato UI interaction; no WhatsApp forward.
   - Flip `pause_all_drivers=false`.
   - Trigger order.
   - Expected: processed normally.

7. **DRILL-7 (humanize plausibility):**
   - Over the 5 orders, analyze audit log inter-action delays.
   - Expected: variance ≥ 15% (no rapid-fire identical delays); no two taps less than humanize-min apart.

#### Acceptance (all must pass)

- [ ] SC-1: detection within 10s (DRILL-1)
- [ ] SC-1: decision within 30s (DRILL-1)
- [ ] SC-2: capacity honored — reject path (DRILL-2) + grace accept (DRILL-3)
- [ ] SC-3: WhatsApp + Discord forward within 15s (DRILL-1, DRILL-3)
- [ ] SC-4: mark-ready within 15s from both admin + POS (DRILL-4)
- [ ] SC-5: session expiry paused + alerted (DRILL-5)
- [ ] Killswitch halts within 10s (DRILL-6)
- [ ] Humanize variance ≥ 15% (DRILL-7)

#### Artifacts saved to SUMMARY.md

- Drill timings table (all 7 drills).
- `adb pull /sdcard/Android/data/in.racingpoint.rcagentmobile/files/logs` attached excerpts.
- WhatsApp + Discord screenshots of forwarded messages.
- Uday sign-off reference (UDAY-SIGNOFF.md).
- MMA audit reference (MMA-AUDIT.md).

#### Checkpoint (human-verify)

James (and optionally Uday) runs drills 1–7 on Tab Plus with real Zomato test account. Reports pass/fail for each SC with timing measurements. If any SC fails, create gap-closure plan (437-13+) — do NOT mark Phase 437 complete.

Resume signal: user reports all 8 acceptance items with timings, or types "all passed".

**If approved:** flip `enable_zomato_on_tab_plus=true` in production (initial 20-orders/week cap per Uday sign-off).

#### Commit message

```
test(437-12): Tab Plus drill — 5 simulated Zomato orders end-to-end

All 7 drills executed on Tab Plus with Zomato test account.
Evidence: drill timings + log excerpts + WA/Discord screenshots in
SUMMARY.md. Production flag-on with 20-orders/week cap per Uday
sign-off (UDAY-SIGNOFF.md).

Covers: ZOMATO-01..06 acceptance gate.
```

---

## 6. Risks and pitfalls (Zomato-specific, beyond generic Android risks from Phase 429)

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **ToS violation (account-ban)** | Humanize delays + capacity-driven rejections + business-hours gate + killswitch + MMA + Uday sign-off + 20-orders/week initial cap + full TOS-PLAYBOOK-ZOMATO.md. If Zomato warns: immediate killswitch + 7-day manual fallback. |
| R-2 | **Selector drift on Zomato app update** | Phase 433 selector DSL + versioned per-app-version + fallback chain + Phase 443 remote-push UI (future). 437-03 onAppUpdate triggers re-load with fallback. |
| R-3 | **NotificationListenerService killed by OEM** | Re-bind trick in healthCheck (437-02). Poll fallback (60s) in OrderDetector (437-04). |
| R-4 | **Capacity endpoint returns stale data** | Mitigation: endpoint reads live queue depth on every request (no caching). Fail-closed on any error. |
| R-5 | **Duplicate orders (same notification fires twice)** | OrderDetector serializes by order_id (bounded concurrency=1); duplicate within 60s deduplicated in-memory. |
| R-6 | **Mark-ready tapped after customer cancels** | Audit-only — Zomato's own logic handles this (tapping mark-ready on a cancelled order is a no-op in the Zomato UI). Log as UiAction.MarkedReadyOnCancelled if detectable. |
| R-7 | **WhatsApp/Discord bot silently fails** | Fail-non-blocking (order still accepted). Audit log shows `forward_failed`. Staff can re-forward manually from admin if needed (Phase 441). |
| R-8 | **Zomato rate-limits the Partner app API** | We interact with the UI, not API — this is mitigated structurally. But the app itself might throttle. Observable as slow UI responses; humanize absorbs this. |
| R-9 | **Test account scarcity** | OQ-2: do we have a Zomato Partner test account? If no: 437-12 drill uses production account with 5 real small-value orders and Uday sign-off covers the risk. |
| R-10 | **Session guard false-positive during app loading** | Debounce (2x 10s) in 437-09 prevents flap. |
| R-11 | **PII in audit logs / forwards** | customer_name_masked in ForwardClient (437-08). Full name NOT logged in AuditLog (Phase 435 contract). No phone, no address in any log. DPDP compliance. |
| R-12 | **Grace-window causing customer delays** | Grace adds up to `grace_window * retries` = 60s default. Zomato Partner app gives 5min to accept; we stay well within. If `retry_after_secs` from server is high (e.g., 300s), we reject immediately rather than exceed Zomato's window — explicit cap in CapacityClient. |
| R-13 | **Killswitch while mid-tap** | 437-10 policy: allow in-flight to complete. Aborting mid-tap can leave Zomato UI in inconsistent state. |

## 7. Test plan

### Unit tests (JVM + Rust)
- Kotlin: `ZomatoDriverTest` (437-03), `ZomatoNotificationListenerTest` + `NotificationListenerPermissionTest` (437-02), `OrderDetectorTest` + `OrderActionsTest` (437-04), `CapacityClientTest` (437-05), `OrderActionsDecisionTest*` (437-06), `MarkReadyTest` (437-07), `ForwardClientTest` (437-08), `SessionGuardTest` (437-09), `ZomatoKillswitchTest` (437-10).
- Rust: `kitchen_capacity_test` (437-05).
- Protocol: schema-validate `order-forward-protocol-v1.md` against sample payloads.

### Instrumented (device)
- 437-02 notification reception.
- 437-04 navigation on Tab Plus with mock screens.

### Physical device drill (human-verify)
- 437-01 selector capture.
- 437-12 full drill (7 sub-drills).

### Cross-repo integration
- Bono VPS bot `/order-forward` endpoints verified by `curl`.

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** Run before 437-11 against all of `drivers/zomato/*.kt` + `kitchen_routes.rs`. Capacity-decision loop + session-expiry + killswitch + grace-window are the hot business-logic paths.
- **MMA audit (required, dual reasoning modes):** 437-11 is the phase's audit plan. Budget $5. 2 rounds minimum (abstract + trace-level).
- **integration-checker (required):** Cross-system flow agent -> POS -> racecontrol -> admin -> bots is the hardest in v50.0. Run before Phase 437 ship + again before milestone ship.
- **SEC gate:** `node comms-link/test/security-check.js` must pass. 437-05 adds a service-key-gated endpoint; 437-08 adds PSK-gated bot endpoints.
- **Deploy Manifest Protocol (DMP):** `deploy:` section in frontmatter must be ticked item-by-item.
- **Backlog gate (CLAUDE.md CGP v4.3):** 437 must reach DEPLOYED-VERIFIED before Phase 438 (HyperPure) begins.
- **Uday sign-off (unique to 437):** Required before 437-12 drill. Distinct from all other phases — this is a ToS gate.

## 9. Open questions the planner cannot decide

These require a user decision before executing the flagged plans. Listed in execution-blocking order.

**OQ-1 — Capacity endpoint host (racecontrol vs POS rc-agent) (DECIDED in 437-05 — but user should confirm).**
The user prompt phrased it as "POS rc-agent at `/kitchen/capacity`". Plan decides **racecontrol server** because the cafe_orders queue lives there (see `crates/racecontrol/src/cafe_orders.rs`). POS rc-agent doesn't own queue state. If user prefers POS rc-agent as the endpoint, we'd need to add a caching layer in POS that reads from the server — adds complexity for no benefit. **Recommendation:** confirm racecontrol server; proceed. If user rejects, 437-05 scope expands by ~30% (add POS rc-agent route + cache + sync).

**OQ-2 — Zomato Partner test account availability (BLOCKS 437-01 + 437-12).**
Do we have a Zomato Partner test account? If not, two options:
- (a) Acquire one from Zomato Partner support.
- (b) Run 437-12 drill against the production account with explicit Uday sign-off for ≤ 5 low-value test orders.
Preferred: (a). Fallback: (b) with 5-order cap baked into Uday sign-off (437-11).

**OQ-3 — Which device gets the Zomato driver? (DECIDES 437 target device).**
Per user prompt: "OQ-5 from planner wave A: which device gets Zomato driver? Likely M07 since it's more phone-like". Plan **defaults to Tab Plus** because:
- (a) Tab Plus has larger screen -> Zomato Partner UI is easier to navigate reliably via Accessibility.
- (b) Per `REQUIREMENTS-v50.md` CAPREG-01 example: "Tab Plus might run HyperPure + cardboard; M07 might run Zomato only" — ambiguous.
- (c) ToS risk is HIGHEST for Zomato; better on Tab Plus where we have more control (larger device, stays in reception).
**Recommendation:** Tab Plus for Zomato. Confirm before 437-01. Either is supported via `supported_device_types: ["tablet", "phone"]` in manifest — easily flippable via feature flag `enable_zomato_on_m07` vs `enable_zomato_on_tab_plus`.

**OQ-4 — Zomato Partner exact package name (BLOCKS 437-01).**
Candidates from user prompt: `com.application.zomato.merchant`. Also possible: `com.grofers.zomato-partner`, `com.zomato.restaurant`. Must be confirmed by installing on Tab Plus + running `adb shell pm list packages | grep -i zomato`. 437-01 captures this in its first task.

**OQ-5 — WhatsApp bot + Discord bot API shapes (BLOCKS 437-08).**
Plan defines the intended API (`POST /order-forward` + JSON envelope). Bono must implement on the bot side. If the bots already have an `/order-forward` endpoint or similar, we align to theirs instead. 437-08's INBOX handoff asks Bono to confirm.

**OQ-6 — Initial orders-per-week cap post-drill.**
Plan suggests 20 orders/week first week. Uday decides final number in 437-11 sign-off. This becomes a rate-limit config on the driver for the first N weeks.

**OQ-7 — ForwardClient authenticity — does the message include agent device_id? Does it include selector-version for audit?**
Plan includes `device_id` (implicit — sent via comms-link relay); includes `source: "zomato"`. Does NOT include selector-version (not staff-relevant). Confirm if selector-version is desired for audit purposes.

**OQ-8 — Grace window config per-device or global?**
Plan puts grace config in `driverConfig` (per-driver, hot-reloadable via Phase 436). Hence per-device per-driver = effectively per-deployment. If user wants per-device override: already supported via feature-flag-namespacing (`zomato_grace_window_tab_plus` vs `..._m07`).

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` (ZOMATO-01..06)
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Phase 429 (structure template):** `.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md`
- **Phase 433 (selectors):** `.planning/phases/433-selector-dsl-hot-reload/PLAN.md`
- **Phase 434 (credentials):** `.planning/phases/434-credential-abstraction/PLAN.md`
- **Phase 435 (humanize + audit):** `.planning/phases/435-humanize-layer-audit-log/PLAN.md`
- **Phase 436 (feature flags):** `.planning/phases/436-feature-flag-system/PLAN.md`
- **Phase 441 (admin dashboard — full reception view):** downstream consumer of 437.
- **Phase 444 (E2E drills + ToS playbook):** sustains and extends 437-11 + 437-12.
- **Bono VPS bots:** `/root/racingpoint-whatsapp-bot`, `/root/racingpoint-discord-bot` (comms-link-managed).
- **CLAUDE.md rules invoked:** MMA audit (cross-system bridge, dual modes), DEPLOY PARITY, Subagent Gates (nyquist + integration), Backlog Gate, Session 1 for GUI, no-lock-across-await (Kotlin equivalent), verify-before-generate, cascade updates (racecontrol + rc-common + bots + admin + POS all updated in one phase).

## 11. Output (at phase close)

At the end of Plan 437-12 (drill pass), create `.planning/phases/437-zomato-partner-driver/SUMMARY.md` capturing:

- Which commits implemented each plan (437-01 through 437-12).
- Actual drill timings for SC-1 through SC-5 + killswitch + humanize variance.
- Log excerpts (tailed JSONL from Tab Plus).
- WhatsApp + Discord screenshot of forwarded messages.
- MMA audit results (link to MMA-AUDIT.md).
- Uday sign-off reference (link to UDAY-SIGNOFF.md).
- ToS playbook reference (link to TOS-PLAYBOOK-ZOMATO.md).
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked.
- Open questions resolved during execution (update §9 state).
- Any risks encountered + how resolved.
- Handoff to Phase 438 (HyperPure driver) — what's reusable (driver framework, humanize, selector patterns), what's Zomato-specific and not.

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 9 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).

Also amend top-level `.planning/ROADMAP.md` to reflect v50.0 Phase 9 as shipped.
