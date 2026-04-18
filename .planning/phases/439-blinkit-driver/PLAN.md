---
phase: 439-blinkit-driver
phase_number: 439
milestone: v50.0 rc-agent-mobile
name: "Blinkit Driver (P3) — Emergency Top-up Automation"
status: ready-to-execute
goal: >
  Third production driver — automates the Blinkit Android app for emergency top-up
  orders triggered by staff via admin dashboard or Core inventory alerts. Navigates
  Blinkit, searches SKUs, adds to cart, checks out, confirms purchase, captures the
  order number + ETA, and logs back to Core. Respects humanize delays + per-day rate
  limits. Smaller scale than HyperPure (few items, not hundreds) — the flow is
  simpler but the ETA text parser must handle Blinkit's relative-time formats
  ("8 mins", "in 10 min", "Today, 4:30 PM").
requirements: [BLINK-01, BLINK-02, BLINK-03, BLINK-04]
depends_on: [437-zomato-partner-driver]       # Phase 437 established the driver-as-production-plugin pattern + ToS mitigation infrastructure. Phase 438 (HyperPure) is sibling-parallel; this plan reuses its patterns but does NOT block on it.
wave: 3                                       # Wave 1 = phases 429/430/431, Wave 2 = phases 432-436, Wave 3 = drivers 437/438/439
plan_count: 8
plans:
  - 439-01-PLAN: Blinkit selector-map authoring (James — Phase 433 debug capture)
  - 439-02-PLAN: BlinkitDriver AppDriver impl + manifest entry + lifecycle hooks
  - 439-03-PLAN: Staff-trigger consumer (admin dashboard button + Core inventory alert -> comms-link)
  - 439-04-PLAN: Search + cart + checkout flow orchestration
  - 439-05-PLAN: Order confirmation parser (order number + ETA normalization)
  - 439-06-PLAN: Log-back-to-Core integration (order record + ETA event)
  - 439-07-PLAN: Rate-limit + humanize verification (per-day max, business hours)
  - 439-08-PLAN: Integration test (real Blinkit test account, mocked staff trigger, full E2E)
autonomous: false   # 439-01, 439-02 (smoke-install), and 439-08 have human-verify checkpoints (physical device, real account). The rest are auto.
files_modified:
  - rc-agent-mobile/app-drivers/blinkit/                                                  # new driver module
  - rc-agent-mobile/app-drivers/blinkit/manifest.json                                     # driver manifest entry
  - rc-agent-mobile/app-drivers/blinkit/selectors/v-UNKNOWN/selectors.yaml                # placeholder; real version filled in 439-01
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitDriver.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitTriggerConsumer.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitCheckoutFlow.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitConfirmationParser.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/EtaParser.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitDriverTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitTriggerConsumerTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitCheckoutFlowTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/BlinkitConfirmationParserTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/blinkit/EtaParserTest.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/DriverRegistry.kt  # add Blinkit registration
  - rc-agent-mobile/docs/BLINKIT-DRIVER.md                                                     # driver spec
  - crates/racecontrol/src/api/reception/blinkit.rs                                            # new server route: POST /api/v1/reception/blinkit/top-up
  - crates/racecontrol/src/api/reception/mod.rs                                                # route wiring
  - crates/racecontrol/src/db/migrations/NNN_blinkit_orders.sql                                # blinkit_orders table
  - crates/racecontrol/src/services/blinkit_orders.rs                                          # order record CRUD + ETA update
  - apps/admin/app/reception/blinkit/page.tsx                                                  # admin dashboard button
  - apps/admin/app/reception/blinkit/components/TopUpTriggerForm.tsx
  - comms-link/james/routes/blinkit-trigger.js                                                 # relay-side forwarder (POS -> Tab Plus/M07)
  - .planning/phases/439-blinkit-driver/SUMMARY.md                                             # filled at phase close

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [racecontrol]                   # new /api/v1/reception/blinkit/* routes + blinkit_orders service
  frontend_rebuild: [admin]                    # new /reception/blinkit/ page in admin dashboard
  config_change: >
    rc-agent-mobile drivers.json — add Blinkit entry.
    rc-agent-mobile humanize.toml — add [blinkit] section (per-app overrides for delay + rate limit).
    comms-link james/index.js — register new message type `blinkit_trigger` (authorized senders: admin dashboard service account + Core inventory daemon).
  db_migration: "blinkit_orders table (id, triggered_at, triggered_by, sku_list_json, order_number, eta_minutes, eta_raw_text, eta_parsed_at, order_placed_at, status, device_id, created_at, updated_at)"
  infrastructure: >
    Blinkit test/staff account provisioned (account auth + payment method + venue delivery address saved to account).
    Android app installed on Tab Plus (primary) with signed-in Blinkit session (PersistentSession per CRED-02).
    Device firewall allows inbound from comms-link relay (port 8090).
  data_files: "rc-agent-mobile/app-drivers/blinkit/selectors/v-{detected_version}/selectors.yaml — captured in 439-01 via Phase 433 debug capture mode."
  bat_file: none                               # no Windows scripts touched
  cloud_parity:
    - racecontrol binary (venue .23) + cloud racecontrol (Bono VPS) — DEPLOY PARITY rule
    - admin dashboard (venue .23:3201) + cloud admin dashboard (Bono VPS) — DEPLOY PARITY rule
    - comms-link relay James .27 + comms-link relay Bono VPS — `blinkit_trigger` message type both sides
    - blinkit_orders migration on venue DB AND cloud DB (Phase 301 cloud_data_sync_v2 will replicate going forward)
  targets:
    - server_23              # racecontrol binary + DB migration
    - bono_vps               # cloud racecontrol + DB migration
    - james_27               # comms-link relay config
    - tab_plus               # primary device for Blinkit driver (M07 is fallback — see OQ-1)
    - admin_frontend         # admin dashboard rebuild
  rollback:
    - "Feature flag `enable_blinkit_on_tab_plus` = false halts driver instantly (Phase 436 flag system)."
    - "Driver uninstall via flag toggle triggers BlinkitDriver.uninstall() — clears in-flight state, no DB rollback needed."
    - "If server binary has issues: revert to prev via Sept deploy-server.sh auto-rollback (standing rule)."
    - "If blinkit_orders migration fails: migration is additive (new table), safe to drop and re-apply."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: required       # Admin dashboard has a new form (TopUpTriggerForm) — staff-facing UI. Per CLAUDE.md: "No frontend phase ships without UI-SPEC.md AND UI-REVIEW.md."
  ui_auditor: required          # Same reason.
  nyquist_auditor: required     # EtaParser + BlinkitConfirmationParser are pure business logic with defined I/O — MUST have test coverage audit (per CLAUDE.md: "No business logic phase ships without nyquist test audit.")
  mma_audit: required           # Cross-system bridge: Kotlin driver <-> comms-link (Node.js) <-> racecontrol (Rust) <-> admin dashboard (Next.js) <-> SQLite. Dual reasoning modes REQUIRED per CLAUDE.md. ToS-risk domain (Blinkit ToS, MEDIUM risk).
  integration_checker: required # Multi-phase driver; must integrate cleanly with DriverRegistry (432), humanize (435), rate limiter (435), feature flags (436), audit log (435).
  codebase_mapper: skip         # No new top-level modules (everything lives under existing rc-agent-mobile/, crates/racecontrol/, apps/admin/). codebase map refresh deferred to milestone close.

risks_summary:
  - "R-1: **Blinkit package name** — Blinkit rebranded from Grofers in 2022; package is `com.grofers.customerapp` on Play Store as of 2025 check. MUST CONFIRM during 439-01 selector capture — if Blinkit has shipped a fresh package (`com.blinkit.customerapp` or similar), selectors are captured against the NEW package. Never hardcode a package name in code; read from manifest.json `target_package` field."
  - "R-2: **Cart/session persistence** — Blinkit aggressively times out carts (~15 min idle). A staff trigger that sits queued for 20 min (e.g., outside business hours) will find an empty cart on resume. Mitigation: driver MUST detect empty-cart state at checkout and re-add items, OR refuse to check out and log `cart_expired` event."
  - "R-3: **Delivery address edge cases** — Blinkit uses the account's saved address by default. If the venue address is NOT set as default, driver may auto-ship to the wrong address. Mitigation: checkout flow includes a `verify_delivery_address` step that reads the address text via Accessibility and compares against `BLINKIT_VENUE_ADDRESS_SUBSTRING` config — abort checkout if mismatch."
  - "R-4: **ETA text parsing fragility** — Blinkit ETA formats vary: `\"8 mins\"`, `\"in 10 min\"`, `\"Arriving in 15 min\"`, `\"Today, 4:30 PM\"`, `\"30-45 mins\"` (range). Parser MUST handle all known formats and log raw text on parse failure so James can extend EtaParser. Never silently return null — log + alert."
  - "R-5: **Out-of-stock at checkout** — unlike HyperPure, Blinkit may remove OOS items silently from cart at the checkout stage. Mitigation: driver reads cart total + item count immediately before pay-tap; if item count < expected, log `items_removed_from_cart` with diff + abort (staff reviews + retries manually)."
  - "R-6: **Payment confirmation step** — Blinkit uses saved payment methods (UPI, card). If the default payment method has expired or requires OTP/PIN re-entry, the driver will hang on the payment screen. Mitigation: timeout at payment step (30s), log `payment_step_stuck`, alert staff via WhatsApp + admin dashboard. Do NOT attempt to enter PINs via Accessibility — ToS red line."
  - "R-7: **Rate limit bypass risk** — if both admin dashboard AND Core inventory alerts fire simultaneously, the rate limiter might serialize queue two orders. Mitigation: rate check is applied at staff-trigger-consumer BEFORE enqueue, with atomic counter."
  - "R-8: **ToS / account ban risk** — Blinkit has aggressive bot detection. Emergency top-up (few items, human-like pace) is lower risk than HyperPure bulk, but still risked. Mitigation: humanize with wider delay distribution (2-8s between actions for Blinkit vs 1-3s for HyperPure), business-hours only, max 3 orders/day default."
  - "R-9: **ADMIN dashboard Uday-sign-off** — top-up triggers spend real money. The admin dashboard form MUST show a confirmation dialog with SKU summary + expected cost range (derived from last order) before firing. Pre-execution checkpoint REQUIRED."
  - "R-10: **Tab Plus vs M07 capability split (OQ-1)** — which device runs Blinkit? Default recommendation: Tab Plus (larger screen = more selector stability). Fallback: M07 (if Tab Plus is dedicated to HyperPure). Final decision in 439-01 per capability registry (CAPREG-01)."
---

# Phase 439 — Blinkit Driver (P3) — Emergency Top-up Automation

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 439 |
| Name | Blinkit Driver (P3) — Emergency Top-up Automation |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | BLINK-01, BLINK-02, BLINK-03, BLINK-04 |
| Dependencies | Phase 437 (driver-as-production-plugin pattern, ToS mitigation infrastructure). Sibling-parallel with Phase 438 (HyperPure). |
| Wave | 3 |
| Status | Ready to execute — pending pre-execution checkpoint (Uday sign-off + Blinkit test account provisioned) |
| Autonomous | No — 3 of 8 plans have human-verify checkpoints (physical device + real Blinkit account) |
| Ship test | Staff-triggered Blinkit top-up executes end-to-end in the Blinkit app, order number + ETA logged back to Core, humanize + rate limits enforced across a 24h run. |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 11)

1. **Staff-triggered top-up executes end-to-end in Blinkit app.**
2. **Order number + ETA logged back to Core.**
3. **Humanize + rate limits enforced.**

## 3. Goal-backward must-haves

### Truths (user-observable)

- **T-1:** Staff clicks "Emergency Top-up" in admin dashboard -> fills SKU list + quantities -> confirms -> within 5 min (humanize delay + Blinkit checkout) a Blinkit order is placed.
- **T-2:** `GET /api/v1/reception/blinkit/orders?latest=5` (or admin UI equivalent) shows the just-placed order with `order_number` (e.g., `BL-20260418-XXXX`) and `eta_minutes` (e.g., 15) populated.
- **T-3:** Core's inventory alert webhook fires a `blinkit_trigger` message -> within 5 min a Blinkit order is placed + logged.
- **T-4:** Driver refuses to place a 4th order in the same calendar day (default `max_orders_per_day=3`); excess trigger is logged to `blinkit_rate_limited` audit event + admin dashboard sees a "rate limited" status badge.
- **T-5:** Driver refuses to place an order outside business hours (configurable, default 08:00-22:00 IST); trigger is queued for next business-hours window or dropped per policy.
- **T-6:** Between any two UI actions in the Blinkit flow, the humanize interceptor injects a delay drawn from `N(mean_ms=4000, stddev_ms=1500)` truncated to `[2000, 8000]` ms.
- **T-7:** If Blinkit cart is empty at checkout entry (timeout), driver re-adds items from the original trigger payload and resumes (or logs `cart_expired_unrecoverable` if re-add fails).
- **T-8:** If delivery address does NOT match venue's configured substring, driver aborts checkout + logs `delivery_address_mismatch` with the actual vs expected text.
- **T-9:** On parse failure (ETA text format unseen), driver logs `eta_parse_failed` with the raw text + order still succeeds (ETA left null) + staff gets WhatsApp alert to extend EtaParser.
- **T-10:** Admin dashboard `/reception/blinkit/` shows: (a) last 10 orders, (b) rate-limit counter for today, (c) driver health (connected + last heartbeat), (d) "Emergency Top-up" trigger form with SKU autocomplete.
- **T-11:** Feature flag `enable_blinkit_on_tab_plus = false` halts the driver within 10s (Phase 436 kill-switch).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/app-drivers/blinkit/manifest.json` | Driver manifest entry | 25 | `target_package` (TBD per OQ-2), `credential_strategy: PersistentSession`, `supported_device_types: ["tablet","phone"]`, `humanize_profile: blinkit` |
| `rc-agent-mobile/app-drivers/blinkit/selectors/v-<VER>/selectors.yaml` | Selector map for Blinkit screens | 80 | Home, Search, SearchResults, ProductDetail, Cart, Checkout, AddressReview, PaymentMethod, OrderPlaced screens — per Phase 433 selector DSL |
| `.../drivers/blinkit/BlinkitDriver.kt` | AppDriver impl | 150 | `install()`, `onAppUpdate()`, `healthCheck()`, `uninstall()`, typed action methods: `placeTopUp(order: TopUpOrder): TopUpResult` |
| `.../drivers/blinkit/BlinkitTriggerConsumer.kt` | Consumes `blinkit_trigger` messages from comms-link | 120 | Rate-limit check, humanize gate, business-hours gate, enqueue + dispatch to BlinkitDriver |
| `.../drivers/blinkit/BlinkitCheckoutFlow.kt` | Orchestrates search->cart->checkout->confirm | 200 | Uses selectors.yaml; one method per screen transition; failure handling per R-2..R-6 |
| `.../drivers/blinkit/BlinkitConfirmationParser.kt` | Extracts order number + ETA from confirmation screen | 80 | Reads AccessibilityNodeInfo tree, delegates to EtaParser |
| `.../drivers/blinkit/EtaParser.kt` | Parses ETA strings -> `EtaResult(minutes: Int?, raw_text: String, format_matched: String?)` | 100 | Regex tier for each known format + fallback (see §5 plan 439-05) |
| `crates/racecontrol/src/api/reception/blinkit.rs` | Server routes | 120 | `POST /api/v1/reception/blinkit/top-up` (staff JWT required), `GET /api/v1/reception/blinkit/orders`, `POST /api/v1/reception/blinkit/order-update` (from agent) |
| `crates/racecontrol/src/db/migrations/NNN_blinkit_orders.sql` | DB schema | 25 | `CREATE TABLE IF NOT EXISTS blinkit_orders (...)` with FKs + indexes |
| `crates/racecontrol/src/services/blinkit_orders.rs` | Service module | 150 | Create, update (with order_number + ETA), list, rate-limit-count query |
| `apps/admin/app/reception/blinkit/page.tsx` | Admin UI | 150 | TopUpTriggerForm + OrderHistoryTable + DriverStatusBadge |
| `comms-link/james/routes/blinkit-trigger.js` | Relay forwarder | 60 | Accepts `blinkit_trigger` POST from racecontrol, forwards to the Android device carrying Blinkit capability |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| Admin dashboard button | `POST /api/v1/reception/blinkit/top-up` | fetchApi | grep `/reception/blinkit/top-up` in `TopUpTriggerForm.tsx` |
| racecontrol route handler | comms-link `/relay/send/blinkit_trigger` | Node fetch | grep `blinkit_trigger` in `crates/racecontrol/src/api/reception/blinkit.rs` |
| comms-link relay | Tab Plus / M07 agent WS | existing WS channel from Phase 429 | grep `rcm-tab-plus` in `comms-link/james/routes/blinkit-trigger.js` |
| BlinkitTriggerConsumer.onMessage | RateLimiter.checkAndIncrement | Kotlin call | grep `rateLimiter.check` in `BlinkitTriggerConsumer.kt` |
| BlinkitTriggerConsumer.dispatch | BlinkitDriver.placeTopUp | Kotlin call | grep `blinkitDriver.placeTopUp` in `BlinkitTriggerConsumer.kt` |
| BlinkitDriver.placeTopUp | BlinkitCheckoutFlow.execute | Kotlin call | grep `checkoutFlow.execute` in `BlinkitDriver.kt` |
| BlinkitCheckoutFlow.confirm | BlinkitConfirmationParser.extract | Kotlin call | grep `confirmationParser.extract` in `BlinkitCheckoutFlow.kt` |
| BlinkitConfirmationParser | EtaParser.parse | Kotlin call | grep `etaParser.parse` in `BlinkitConfirmationParser.kt` |
| BlinkitDriver.placeTopUp result | `POST /api/v1/reception/blinkit/order-update` | HTTP via comms-link | grep `order-update` in `BlinkitDriver.kt` |
| All UI actions | HumanizeInterceptor.interceptDelay | Phase 435 wrapper | grep `humanize.wrap` in `BlinkitCheckoutFlow.kt` |
| BlinkitDriver lifecycle | AuditLog.write | Phase 435 wrapper | grep `audit.log` in each class |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md          # STRUCTURE TEMPLATE (this phase mirrors its format)
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md      # AppDriver interface + DriverRegistry (dependency)
@./.planning/phases/433-selector-dsl-hot-reload/PLAN.md                   # selectors.yaml format + debug capture mode (used in 439-01)
@./.planning/phases/434-credential-abstraction/PLAN.md                    # PersistentSession strategy (Blinkit uses this)
@./.planning/phases/435-humanize-layer-audit-log/PLAN.md                  # HumanizeInterceptor + RateLimiter + AuditLog
@./.planning/phases/436-feature-flag-system/PLAN.md                       # kill-switch flag per-device per-driver
@./.planning/phases/437-zomato-partner-driver/PLAN.md                     # P1 driver — establishes driver-as-production-plugin pattern + ToS mitigation
@./.planning/phases/438-hyperpure-driver/PLAN.md                          # P2 driver — sibling pattern, reuses same checkout shell (Blinkit is smaller-scale sibling)
@./rc-agent-mobile/docs/PROTOCOL.md                                       # comms-link envelope schema (from Phase 429)
@./crates/racecontrol/src/api/reception/                                  # existing reception routes (Zomato lives here from Phase 437)
@./apps/admin/app/reception/                                              # existing admin reception pages

### Interfaces executors will need

**AppDriver** (from Phase 432) — BlinkitDriver implements:
```kotlin
interface AppDriver {
    val id: String                                      // "blinkit"
    val targetPackage: String                           // from manifest (verified in 439-01)
    val credentialStrategy: CredentialStrategy          // PersistentSession
    val supportedDeviceTypes: Set<DeviceType>           // [TABLET, PHONE]
    suspend fun install()
    suspend fun onAppUpdate(newVersion: String)
    suspend fun healthCheck(): HealthStatus
    suspend fun uninstall()
}
```

**TopUpOrder** (new; defined in this phase):
```kotlin
@Serializable
data class TopUpOrder(
    val id: String,                                     // UUID from server
    val skus: List<SkuItem>,
    val triggeredBy: String,                            // "admin:usingh" or "core:inventory-alert"
    val note: String? = null
)
@Serializable
data class SkuItem(val name: String, val quantity: Int, val sku_hint: String? = null)

@Serializable
data class TopUpResult(
    val orderId: String,
    val outcome: Outcome,
    val orderNumber: String? = null,
    val etaMinutes: Int? = null,
    val etaRawText: String? = null,
    val errorCode: String? = null,
    val errorDetails: String? = null
)
enum class Outcome { SUCCESS, RATE_LIMITED, OUTSIDE_BUSINESS_HOURS, CART_EXPIRED_UNRECOVERABLE, DELIVERY_ADDRESS_MISMATCH, ITEMS_REMOVED_FROM_CART, PAYMENT_STEP_STUCK, ETA_PARSE_FAILED_ORDER_OK, SELECTOR_MISS, APP_CRASHED, UNKNOWN_FAILURE }
```

**Server API contract** (new):
```
POST /api/v1/reception/blinkit/top-up           (staff JWT)
  Body: { skus: [{name, quantity, sku_hint?}], note? }
  Response: { order_id: uuid, status: "queued"|"rate_limited"|"outside_hours", queued_for_device: "rcm-tab-plus" }

GET /api/v1/reception/blinkit/orders?latest=N    (staff JWT)
  Response: { orders: [{order_id, triggered_at, triggered_by, skus, order_number, eta_minutes, eta_raw_text, status, device_id}] }

POST /api/v1/reception/blinkit/order-update      (service key — from agent)
  Body: TopUpResult
  Response: { ok: true }
```

**comms-link message** (new — extends Phase 429 protocol):
```json
{
  "v": 1, "protocol_version": 1, "type": "blinkit_trigger",
  "from": "racecontrol-server", "to": "rcm-tab-plus",
  "ts": 1713500000000, "id": "uuid",
  "payload": { "order_id": "...", "skus": [...], "triggered_by": "admin:usingh" }
}
```

## 5. Atomic plan breakdown (8 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

---

### 439-01-PLAN — Blinkit selector-map authoring

**Goal:** Commit a working `selectors.yaml` for Blinkit's current app version, generated via the Phase 433 debug capture mode. This is a MANUAL James task.

**Covers:** SELECTOR-01, SELECTOR-02 applied to Blinkit — prereq for BLINK-02.

**Dependencies:** Phase 433 (debug capture), Phase 434 (PersistentSession login done once)

**Type:** `checkpoint:human-verify` (physical device + real Blinkit app install + signed-in account)

**Pre-execution checkpoint (BLOCKS ALL OF PHASE 439):**
- [ ] Uday sign-off on Blinkit test account usage (ToS risk acknowledged)
- [ ] Blinkit test account provisioned (venue address set as default; at least one saved payment method)
- [ ] Blinkit app installed on Tab Plus AND signed in (PersistentSession — user logs in once manually per CRED-02)

#### Tasks

1. **Verify package name (resolves R-1 / OQ-2):**
   ```bash
   adb -s <tab_plus_serial> shell pm list packages | grep -i -E "grofers|blinkit"
   ```
   Record the exact package in `rc-agent-mobile/app-drivers/blinkit/manifest.json` field `target_package`. **Do NOT hardcode** `com.grofers.customerapp` or `com.blinkit.customerapp` — whatever pm returns is ground truth. If neither appears, Blinkit is installed under a different package — investigate via Play Store listing before proceeding.

2. **Verify app version:**
   ```bash
   adb shell dumpsys package <target_package> | grep -E "versionName|versionCode"
   ```
   Use `versionName` (e.g., `15.11.4`) to name the selector directory: `selectors/v-15.11.4/selectors.yaml`.

3. **Capture selectors for every screen in the checkout flow** using Phase 433's debug capture mode. Screens required (match the flow in 439-04):
   - **Home screen** — identify the search bar (entrypoint).
   - **Search input** — text field + submit.
   - **Search results** — first product tile + "Add" button per tile.
   - **Product detail** (if Blinkit routes through it) — quantity selector + "Add to cart" button.
   - **Cart screen** — item list, quantity steppers, total amount, "Proceed to checkout" button.
   - **Checkout / Address review** — address text block, "Change address" link (must detect), "Continue" button.
   - **Payment method screen** — payment options, default method, "Pay" button. **Do NOT capture PIN/OTP entry fields** (ToS red line — never automate payment auth).
   - **Order placed / confirmation screen** — order number text, ETA text, success indicator.

4. **Selector strategies per element** — provide a fallback chain in YAML:
   ```yaml
   # selectors/v-15.11.4/selectors.yaml (fragment)
   home:
     search_entry:
       - strategy: resource_id
         value: "<target_package>:id/search_bar"
       - strategy: content_description
         value: "Search products"
       - strategy: text
         value: "Search"
       - strategy: xpath
         value: "//android.widget.EditText[contains(@content-desc,'search')]"
   search_results:
     first_product_tile:
       - strategy: resource_id
         value: "<target_package>:id/product_card"
         index: 0
     add_button_on_tile:
       - strategy: content_description
         value: "Add to cart"
         scope: "within:first_product_tile"
   order_placed:
     order_number:
       - strategy: text_regex
         value: "^(BL|BK)[-]?\\d{6,}"
     eta_text:
       - strategy: resource_id
         value: "<target_package>:id/delivery_eta"
       - strategy: text_regex
         value: "(mins?|min|hours?|Today|Tomorrow)"
   ```

5. **Commit the captured YAML and update manifest.json** with package + version range:
   ```json
   {
     "id": "blinkit",
     "display_name": "Blinkit",
     "target_package": "<confirmed_package>",
     "selector_version_policy": "exact_then_fallback",
     "known_versions": ["15.11.4"],
     "credential_strategy": "PersistentSession",
     "supported_device_types": ["tablet", "phone"],
     "humanize_profile": "blinkit",
     "risk_class": "MEDIUM"
   }
   ```

6. **Validate selector freshness** by running the Phase 433 selector-dry-run: for each `screen.element`, assert the matcher finds a unique node in the current captured tree. No duplicates, no misses.

#### Acceptance

- `rc-agent-mobile/app-drivers/blinkit/manifest.json` exists with a non-placeholder `target_package`.
- `rc-agent-mobile/app-drivers/blinkit/selectors/v-<version>/selectors.yaml` exists, >= 80 lines, covers all 8 screens in §3 task 3.
- Phase 433 dry-run selector validator passes against a freshly captured node tree (live Blinkit app open on Tab Plus).
- Git-tracked screenshots of each captured screen saved to `.planning/phases/439-blinkit-driver/evidence/selectors-v-<version>/` (NO PII — blur name/phone).

#### Checkpoint (human-verify)

James (or Uday) opens Blinkit on Tab Plus, walks through home -> search -> cart -> checkout -> confirmation manually with the debug capture active. Commits the YAML + screenshots. Reports: "Captured Blinkit v<version> selectors, all 8 screens OK" OR describes missing screens / ambiguous selectors.

#### Commit message

```
feat(439-01): Blinkit selector-map v-<version> captured

Manifest target_package confirmed via adb pm list.
All 8 checkout flow screens covered with fallback chains.
Debug-capture evidence committed to .planning/phases/439-blinkit-driver/evidence/.

Covers: prereq for BLINK-02 (no net-new requirement coverage)
Not tested: end-to-end flow (439-04), confirmation parsing (439-05).
```

---

### 439-02-PLAN — BlinkitDriver AppDriver impl + manifest entry + lifecycle hooks

**Goal:** Create `BlinkitDriver.kt` implementing the `AppDriver` interface from Phase 432. Register in `DriverRegistry`. Lifecycle hooks (`install`, `onAppUpdate`, `healthCheck`, `uninstall`) are wired but `placeTopUp` is a stub that returns `Outcome.UNKNOWN_FAILURE` — real logic in 439-04/05.

**Covers:** BLINK-02 (structural slot), DRIVER-01..05 applied to Blinkit.

**Dependencies:** 439-01 (manifest exists), Phase 432 (AppDriver interface), Phase 434 (PersistentSession)

**Type:** `auto` + `checkpoint:human-verify` at the end (install + healthCheck on physical device).

#### Tasks

1. Create `.../drivers/blinkit/BlinkitDriver.kt`:
   ```kotlin
   class BlinkitDriver(
       private val context: Context,
       private val selectorRepo: SelectorRepository,
       private val credentialStrategy: CredentialStrategy,
       private val auditLog: AuditLog,
   ) : AppDriver {
       override val id = "blinkit"
       override val targetPackage: String by lazy {
           Manifest.read("blinkit").targetPackage
       }
       override val credentialStrategy = credentialStrategy
       override val supportedDeviceTypes = setOf(DeviceType.TABLET, DeviceType.PHONE)

       override suspend fun install() {
           auditLog.log("blinkit", "driver_install")
           // Verify target package is installed on device
           val pkg = context.packageManager.getPackageInfo(targetPackage, 0)
           auditLog.log("blinkit", "target_app_version", mapOf("version" to pkg.versionName))
           // Hot-reload selectors for this version
           selectorRepo.loadForDriver("blinkit", pkg.versionName)
       }

       override suspend fun onAppUpdate(newVersion: String) {
           auditLog.log("blinkit", "target_app_updated", mapOf("version" to newVersion))
           selectorRepo.loadForDriver("blinkit", newVersion)
       }

       override suspend fun healthCheck(): HealthStatus {
           val sessionValid = credentialStrategy.isSessionValid()
           val targetInstalled = try { context.packageManager.getPackageInfo(targetPackage, 0); true } catch (e: Exception) { false }
           val selectorsLoaded = selectorRepo.hasSelectorsFor("blinkit")
           return when {
               !targetInstalled -> HealthStatus.Unhealthy("target_app_not_installed")
               !sessionValid -> HealthStatus.Unhealthy("session_expired")
               !selectorsLoaded -> HealthStatus.Unhealthy("selectors_missing")
               else -> HealthStatus.Healthy
           }
       }

       override suspend fun uninstall() {
           auditLog.log("blinkit", "driver_uninstall")
           // BlinkitTriggerConsumer listens on a flag — uninstall signals it to stop accepting
       }

       /** Placeholder — real implementation in 439-04. */
       suspend fun placeTopUp(order: TopUpOrder): TopUpResult =
           TopUpResult(orderId = order.id, outcome = Outcome.UNKNOWN_FAILURE, errorCode = "not_implemented_yet_439-02")
   }
   ```

2. Register in `DriverRegistry.kt` (existing from Phase 432):
   ```kotlin
   registry.register(BlinkitDriver(context, selectorRepo, credentialStrategy, auditLog))
   ```

3. Add feature flag hook: `enable_blinkit_on_tab_plus` (default false until 439-08 drill passes, then flipped true). Flag read via Phase 436 FlagClient on driver start.

4. Write unit tests:
   - `BlinkitDriverTest.installLogsTargetVersion` — mock PackageManager, assert audit log entry.
   - `BlinkitDriverTest.healthCheckReportsSessionExpired` — mock CredentialStrategy to return false, assert `Unhealthy("session_expired")`.
   - `BlinkitDriverTest.healthCheckReportsMissingSelectors` — mock SelectorRepository, assert `Unhealthy("selectors_missing")`.

#### Acceptance

- BlinkitDriver registers in DriverRegistry on app boot.
- `healthCheck()` returns `Healthy` after 439-01 selectors loaded + PersistentSession valid.
- `healthCheck()` returns `Unhealthy("session_expired")` if user manually signs out of Blinkit on the device.
- Unit tests pass.

#### Checkpoint (human-verify)

User toggles `enable_blinkit_on_tab_plus` = true via admin dashboard (Phase 436 UI from Phase 443). Checks device logs: `driver_install` event emitted. User confirms: "Driver loaded OK, healthCheck returned Healthy."

#### G4 NOT TESTED list

- `placeTopUp` real execution (439-04).
- Trigger consumer (439-03).
- Confirmation parsing (439-05).

#### Commit message

```
feat(439-02): BlinkitDriver AppDriver impl + DriverRegistry registration

BlinkitDriver implements Phase 432 AppDriver interface.
Lifecycle hooks wired: install loads selectors, healthCheck validates session + target app.
placeTopUp() stub returns UNKNOWN_FAILURE — real logic in 439-04.

Covers: BLINK-02 (structural), DRIVER-01..05 applied to Blinkit
Not tested: placeTopUp flow, trigger consumer, confirmation parsing.
```

---

### 439-03-PLAN — Staff-trigger consumer (admin dashboard + Core inventory alert -> comms-link)

**Goal:** End-to-end trigger path works: admin dashboard button OR Core inventory alert -> racecontrol route -> comms-link -> BlinkitTriggerConsumer on device. BlinkitDriver.placeTopUp still returns the stub — we're validating plumbing only.

**Covers:** BLINK-01 (trigger acceptance, not yet execution).

**Dependencies:** 439-02

**Type:** `auto`

#### Tasks

1. **Server side — routes + service + migration:**
   - `crates/racecontrol/src/db/migrations/NNN_blinkit_orders.sql`:
     ```sql
     CREATE TABLE IF NOT EXISTS blinkit_orders (
         id TEXT PRIMARY KEY,
         triggered_at INTEGER NOT NULL,
         triggered_by TEXT NOT NULL,
         sku_list_json TEXT NOT NULL,
         order_number TEXT,
         eta_minutes INTEGER,
         eta_raw_text TEXT,
         eta_parsed_at INTEGER,
         order_placed_at INTEGER,
         status TEXT NOT NULL DEFAULT 'queued',
         device_id TEXT,
         error_code TEXT,
         error_details TEXT,
         note TEXT,
         created_at INTEGER NOT NULL,
         updated_at INTEGER NOT NULL
     );
     CREATE INDEX IF NOT EXISTS idx_blinkit_triggered_at ON blinkit_orders(triggered_at DESC);
     CREATE INDEX IF NOT EXISTS idx_blinkit_status ON blinkit_orders(status);
     ```
     **MUST ALSO `ALTER TABLE ADD COLUMN`** guards per CLAUDE.md "DB migrations must cover ALL consumers" rule (this table is new so no ALTER needed, but document in SUMMARY.md).

   - `crates/racecontrol/src/services/blinkit_orders.rs`:
     - `create_order(pool, order)` — insert row, status `queued`.
     - `update_order_placed(pool, order_id, device_id, order_number, eta_minutes, eta_raw_text)` — update on agent-side success.
     - `mark_failed(pool, order_id, error_code, error_details)` — mark failure.
     - `count_today(pool) -> u32` — rate-limit query (`WHERE triggered_at >= today_ist_start AND status NOT IN ('rate_limited','outside_hours')`).
     - `list_recent(pool, limit)`.

   - `crates/racecontrol/src/api/reception/blinkit.rs`:
     - `POST /api/v1/reception/blinkit/top-up` (staff JWT middleware, body validated):
       1. Rate-limit check via `count_today()` — reject with `{status: "rate_limited", count: N, max: 3}` if >= max.
       2. Business-hours check — reject with `{status: "outside_hours"}` outside configured window.
       3. Generate order_id (UUID), insert row with status `queued`.
       4. POST to comms-link `/relay/send` with `type: "blinkit_trigger"` payload, targeted at the device with Blinkit capability (queried from Capability Registry from Phase 432).
       5. Return `{order_id, status: "queued", queued_for_device: "rcm-tab-plus"}`.
     - `GET /api/v1/reception/blinkit/orders?latest=N` (staff JWT).
     - `POST /api/v1/reception/blinkit/order-update` — **service-key middleware (NOT staff JWT)** — receives `TopUpResult` from agent, updates row. Per CLAUDE.md: "Pod HTTP endpoints default to protected. New endpoint goes behind require_service_key UNLESS there is a documented reason for it to be public."

2. **comms-link side:**
   - `comms-link/james/routes/blinkit-trigger.js` — accept POST from racecontrol (service-key auth), forward WS message to `rcm-tab-plus` (or `rcm-m07` per OQ-1). If target device is not connected, queue + retry with 30s backoff up to 5 min, then NACK back to racecontrol (which marks order failed).
   - Register `blinkit_trigger` as an allowed message type in the identity allowlist from Phase 429.
   - **DEPLOY PARITY:** identical change on Bono VPS relay.

3. **Agent side — BlinkitTriggerConsumer.kt:**
   ```kotlin
   class BlinkitTriggerConsumer(
       private val driver: BlinkitDriver,
       private val humanize: HumanizeInterceptor,     // Phase 435
       private val rateLimiter: RateLimiter,          // Phase 435 — per-driver, per-day
       private val businessHours: BusinessHoursGate,  // Phase 435
       private val auditLog: AuditLog,                // Phase 435
       private val flagClient: FlagClient,            // Phase 436
       private val scope: CoroutineScope,
   ) {
       fun attachTo(commsClient: CommsLinkClient) {
           commsClient.onMessageOfType("blinkit_trigger") { envelope ->
               scope.launch { handle(envelope) }
           }
       }

       private suspend fun handle(envelope: Envelope<BlinkitTriggerPayload>) {
           if (!flagClient.isEnabled("enable_blinkit_on_${deviceId()}")) {
               auditLog.log("blinkit", "trigger_dropped_flag_off", mapOf("order_id" to envelope.payload.order_id))
               return
           }
           if (!businessHours.inWindow("blinkit")) {
               sendUpdate(envelope.payload.order_id, TopUpResult.outsideHours())
               return
           }
           if (!rateLimiter.checkAndIncrement("blinkit")) {
               sendUpdate(envelope.payload.order_id, TopUpResult.rateLimited())
               return
           }
           val order = TopUpOrder(envelope.payload.order_id, envelope.payload.skus, envelope.payload.triggered_by, envelope.payload.note)
           val result = driver.placeTopUp(order)                        // stub until 439-04
           sendUpdate(order.id, result)
       }

       private suspend fun sendUpdate(orderId: String, result: TopUpResult) {
           // POST to comms-link -> racecontrol /order-update with service key.
           // Write to audit log on both success and failure.
       }
   }
   ```

4. **Admin dashboard — minimal:** in this plan, wire a curl/Postman-triggered test. Full UI lands in 439-08 UI-SPEC + subsequent plan iteration (UI-SPEC/UI-REVIEW gates).

5. **Tests:**
   - `BlinkitTriggerConsumerTest.flagOffDropsTrigger` — flag false -> audit log entry, no driver call.
   - `BlinkitTriggerConsumerTest.outsideHoursReturnsOutsideHours` — business hours mocked false.
   - `BlinkitTriggerConsumerTest.rateLimitedReturnsRateLimited` — rate limiter returns false.
   - `BlinkitTriggerConsumerTest.happyPathCallsDriver` — asserts `driver.placeTopUp(order)` invoked.
   - Rust tests: `blinkit_orders_service::count_today_ignores_rate_limited_rows`, `api::reception::blinkit::rate_limit_returns_4xx` (or 200 with status field — whichever pattern matches existing reception routes).

#### Acceptance

- `curl -X POST http://localhost:8080/api/v1/reception/blinkit/top-up -H "Authorization: Bearer <staff_jwt>" -d '{"skus":[{"name":"Milk","quantity":2}]}'` returns `{order_id, status:"queued", queued_for_device:"rcm-tab-plus"}`.
- Row appears in `blinkit_orders` with `status='queued'`.
- Device receives `blinkit_trigger` via WS; agent logs `trigger_received order_id=...`.
- Agent calls `driver.placeTopUp()` (stub returns UNKNOWN_FAILURE) -> agent POSTs `/order-update` -> server updates row with `error_code=not_implemented_yet_439-02`.
- All unit tests + Rust tests pass.

#### G4 NOT TESTED list

- Full checkout execution (439-04).
- Confirmation parsing (439-05).
- UI gate (admin form lands in 439-08 prep + UI-SPEC subagent).

#### Commit message

```
feat(439-03): Blinkit staff-trigger consumer + server routes + comms-link forwarder

POST /api/v1/reception/blinkit/top-up (staff JWT) inserts order + triggers device.
POST /api/v1/reception/blinkit/order-update (service key) receives agent result.
blinkit_orders table migrated on venue + cloud.
comms-link blinkit_trigger message type registered James relay + Bono VPS (DEPLOY PARITY).
BlinkitTriggerConsumer applies flag / business-hours / rate-limit before driver call.

Covers: BLINK-01 (trigger acceptance), BLINK-04 structural gate (rate limit applied)
Not tested: real placeTopUp (stub returns UNKNOWN_FAILURE until 439-04).
```

---

### 439-04-PLAN — Search + cart + checkout flow orchestration

**Goal:** BlinkitDriver.placeTopUp actually drives Blinkit through home -> search -> add each SKU -> cart -> checkout -> address verify -> payment tap -> order confirmation. Stops one step before reading order number / ETA (that's 439-05). Emits the correct `Outcome` for each failure class in risks R-2 through R-6.

**Covers:** BLINK-02 (execution).

**Dependencies:** 439-01 (selectors), 439-02 (driver skeleton), 439-03 (trigger plumbing), Phase 435 (humanize)

**Type:** `auto`

#### Tasks

1. Create `.../drivers/blinkit/BlinkitCheckoutFlow.kt`:
   - One method per screen transition:
     - `openBlinkit()`
     - `searchSku(name: String): Boolean` — returns true if at least one result.
     - `addFromResults(quantity: Int): Boolean` — returns true if added.
     - `goBackToSearch()`
     - `openCart()`
     - `verifyCartItemCount(expected: Int): Boolean` (R-5 mitigation)
     - `proceedToCheckout()`
     - `verifyDeliveryAddress(): Boolean` (R-3 mitigation; compares against `BLINKIT_VENUE_ADDRESS_SUBSTRING` config)
     - `goToPayment()`
     - `tapPay()` — taps the pay button IF default payment method is ready; does NOT interact with PIN/OTP fields.
     - `waitForOrderPlacedScreen(timeout: Duration): Boolean` (R-6 mitigation; 30s timeout).

   - Each tap/swipe/text-input wrapped in `humanize.wrap { ... }` (Phase 435 interceptor).

   - Orchestration method:
     ```kotlin
     suspend fun execute(order: TopUpOrder): TopUpResult {
         auditLog.log("blinkit", "checkout_start", mapOf("order_id" to order.id, "sku_count" to order.skus.size))
         try {
             openBlinkit()
             for (sku in order.skus) {
                 if (!searchSku(sku.name)) return TopUpResult.selectorMiss(order.id, "search_no_results:${sku.name}")
                 if (!addFromResults(sku.quantity)) return TopUpResult.selectorMiss(order.id, "add_failed:${sku.name}")
                 goBackToSearch()
             }
             openCart()
             if (!verifyCartItemCount(order.skus.size)) return TopUpResult.itemsRemovedFromCart(order.id)
             proceedToCheckout()
             // R-2: if cart was empty at checkout entry, BlinkitCheckoutFlow.openCart would have found 0 items
             //     -> one retry path: re-add from SKU list, log `cart_expired_retried`. If still empty -> CART_EXPIRED_UNRECOVERABLE.
             if (!verifyDeliveryAddress()) return TopUpResult.deliveryAddressMismatch(order.id)
             goToPayment()
             tapPay()
             if (!waitForOrderPlacedScreen(timeout = 30.seconds)) return TopUpResult.paymentStepStuck(order.id)
             // Success path — hand off to 439-05 confirmation parser
             return confirmationParser.extract(order.id)
         } catch (e: SelectorMissException) {
             return TopUpResult.selectorMiss(order.id, e.screenElement)
         } catch (e: Exception) {
             auditLog.log("blinkit", "unknown_failure", mapOf("order_id" to order.id, "exception" to e.toString()))
             return TopUpResult.unknownFailure(order.id, e.toString())
         }
     }
     ```

2. Update BlinkitDriver.placeTopUp:
   ```kotlin
   override suspend fun placeTopUp(order: TopUpOrder): TopUpResult {
       val healthCheck = healthCheck()
       if (healthCheck !is HealthStatus.Healthy) {
           return TopUpResult.unknownFailure(order.id, "health=${healthCheck}")
       }
       return BlinkitCheckoutFlow(accessibility, selectorRepo, humanize, auditLog, confirmationParser).execute(order)
   }
   ```

3. **Humanize profile `blinkit`** in `humanize.toml`:
   ```toml
   [humanize.profiles.blinkit]
   default_delay_mean_ms = 4000
   default_delay_stddev_ms = 1500
   default_delay_min_ms = 2000
   default_delay_max_ms = 8000
   tap_delay_mean_ms = 3500
   text_input_per_char_mean_ms = 180
   # wider than HyperPure profile because Blinkit bot detection is more aggressive at emergency-top-up cadence
   ```

4. Tests:
   - `BlinkitCheckoutFlowTest.happyPath` — mock AccessibilityService, seed fake node trees for each screen, assert success Outcome. Uses `kotlinx-coroutines-test` virtual time to skip humanize delays.
   - `BlinkitCheckoutFlowTest.cartExpiredRecovers` — fake cart screen returns 0 items -> assert re-add attempt -> assert success.
   - `BlinkitCheckoutFlowTest.cartExpiredUnrecoverable` — re-add also fails -> assert `CART_EXPIRED_UNRECOVERABLE`.
   - `BlinkitCheckoutFlowTest.deliveryAddressMismatch` — address text doesn't contain `BLINKIT_VENUE_ADDRESS_SUBSTRING` -> abort.
   - `BlinkitCheckoutFlowTest.itemsRemovedFromCart` — cart shows 2 items when 3 were added -> `ITEMS_REMOVED_FROM_CART`.
   - `BlinkitCheckoutFlowTest.paymentStepStuck` — order-placed screen never appears within 30s -> `PAYMENT_STEP_STUCK`.
   - `BlinkitCheckoutFlowTest.selectorMiss` — mocked selector repo returns miss for `search_entry` -> `SELECTOR_MISS`.

#### Acceptance

- All unit tests pass.
- On physical Tab Plus with Blinkit test account, a manual `placeTopUp(TopUpOrder("test-001", [{Milk, 1}], "manual-test"))` call (via `/debug/driver/blinkit/place-top-up` — add a debug endpoint gated by service key) navigates the real app up to the order-placed screen. **DO NOT actually place a real order in 439-04** — the last step (`tapPay`) is mocked to log "would_tap_pay" in debug mode. Real pay-tap is exercised in 439-08.

#### Risks addressed

- R-2 cart expiry: detected + one retry + explicit outcome.
- R-3 delivery address: verified via substring match + explicit outcome.
- R-5 items removed: detected at cart verify + explicit outcome.
- R-6 payment stuck: 30s timeout + explicit outcome.

#### Commit message

```
feat(439-04): BlinkitCheckoutFlow orchestration

home -> search -> cart -> checkout -> address-verify -> payment-tap -> order-placed.
R-2/R-3/R-5/R-6 failure classes each emit a distinct Outcome.
tapPay() mocked in DEBUG_BLINKIT_NO_PAY=true mode — 439-08 exercises real pay.

Covers: BLINK-02 (execution logic)
Not tested: real order placement (deferred to 439-08 drill), confirmation parsing (439-05).
```

---

### 439-05-PLAN — Order confirmation parser (order number + ETA normalization)

**Goal:** After the order-placed screen appears, extract `order_number` and `eta_minutes` + `eta_raw_text`. ETA parser MUST handle all known Blinkit formats and log raw text on failure (NEVER silently return null). Dedicated TDD plan — this is pure business logic with defined I/O.

**Covers:** BLINK-03 (data capture half).

**Dependencies:** 439-04

**Type:** `tdd` (per CLAUDE.md TDD detection heuristic — `expect(etaParser.parse("8 mins")).toBe(EtaResult(minutes=8, raw="8 mins", format="x_mins"))`)

#### TDD feature 1: EtaParser

**Behavior (testable I/O table):**

| Input text | Expected `minutes` | Expected `format_matched` |
|---|---|---|
| `"8 mins"` | 8 | `x_mins` |
| `"8 min"` | 8 | `x_min` |
| `"in 10 min"` | 10 | `in_x_min` |
| `"Arriving in 15 min"` | 15 | `arriving_in_x_min` |
| `"30-45 mins"` | 30 | `x_to_y_mins` (take lower bound) |
| `"1 hour"` | 60 | `x_hour` |
| `"2 hours"` | 120 | `x_hours` |
| `"1 hr 30 min"` | 90 | `x_hr_y_min` |
| `"Today, 4:30 PM"` | *current_ist_time_to_430pm* | `today_hhmm` |
| `"Tomorrow, 10:00 AM"` | *current_ist_time_to_tomorrow_10am* | `tomorrow_hhmm` |
| `"Delivered"` | 0 | `delivered` |
| `"Delayed"` | null | `delayed_unknown_eta` |
| `""` (empty) | null | `empty_input` |
| `"zorkblorp"` (unknown) | null | `unrecognized` |

**RED step:** write `EtaParserTest.kt` with 14 cases above. Run -> all fail.

**GREEN step:** implement `EtaParser.kt`:
```kotlin
class EtaParser(private val clock: Clock = IstClock) {
    fun parse(text: String): EtaResult {
        val t = text.trim()
        if (t.isEmpty()) return EtaResult(null, text, "empty_input")
        // Regex tier, ordered most-specific to least-specific
        val patterns = listOf(
            Pattern("x_mins",               Regex("^(\\d+)\\s*mins$"),                  { m -> m.groupValues[1].toInt() }),
            Pattern("x_min",                Regex("^(\\d+)\\s*min$"),                   { m -> m.groupValues[1].toInt() }),
            Pattern("in_x_min",             Regex("^in\\s+(\\d+)\\s*min"),              { m -> m.groupValues[1].toInt() }),
            Pattern("arriving_in_x_min",    Regex("^Arriving in\\s+(\\d+)\\s*min"),     { m -> m.groupValues[1].toInt() }),
            Pattern("x_to_y_mins",          Regex("^(\\d+)-(\\d+)\\s*mins$"),           { m -> m.groupValues[1].toInt() }),
            Pattern("x_hour",               Regex("^1\\s+hour$"),                       { _ -> 60 }),
            Pattern("x_hours",              Regex("^(\\d+)\\s+hours$"),                 { m -> m.groupValues[1].toInt() * 60 }),
            Pattern("x_hr_y_min",           Regex("^(\\d+)\\s*hr\\s+(\\d+)\\s*min$"),   { m -> m.groupValues[1].toInt() * 60 + m.groupValues[2].toInt() }),
            Pattern("today_hhmm",           Regex("^Today,\\s+(\\d{1,2}):(\\d{2})\\s+(AM|PM)$"),    ::todayHhmmToMinutes),
            Pattern("tomorrow_hhmm",        Regex("^Tomorrow,\\s+(\\d{1,2}):(\\d{2})\\s+(AM|PM)$"), ::tomorrowHhmmToMinutes),
        )
        for (p in patterns) {
            p.regex.matchEntire(t)?.let { return EtaResult(p.extractor(it), text, p.name) }
        }
        if (t.equals("Delivered", true)) return EtaResult(0, text, "delivered")
        if (t.equals("Delayed", true)) return EtaResult(null, text, "delayed_unknown_eta")
        return EtaResult(null, text, "unrecognized")
    }
}
```

RUN -> all 14 cases pass.

**REFACTOR:** extract `Pattern` table to a companion object if needed. Verify tests still pass.

#### TDD feature 2: BlinkitConfirmationParser

**Behavior:**

| Input | Expected |
|---|---|
| Mock node tree with `order_number` text `"BL-20260418-0042"` + ETA `"12 mins"` | `TopUpResult(outcome=SUCCESS, order_number="BL-20260418-0042", eta_minutes=12, eta_raw="12 mins")` |
| Mock node tree with `order_number` missing | `TopUpResult(outcome=ETA_PARSE_FAILED_ORDER_OK, ...)` — wait, order number missing is worse than ETA missing. Separate outcome: `ORDER_NUMBER_MISSING` — treat as SUCCESS but alert loudly (order was placed; we just can't track it). Actually — if order number is missing, we should ABORT writing the result and alert staff immediately so they can open Blinkit and find the order manually. Define outcome `ORDER_PLACED_BUT_NUMBER_UNREADABLE`. |
| Mock node tree with `order_number` present + ETA `"zorkblorp"` | `TopUpResult(outcome=ETA_PARSE_FAILED_ORDER_OK, order_number="BL-...", eta_minutes=null, eta_raw="zorkblorp")` |

**RED -> GREEN -> REFACTOR** same cycle.

#### Tasks

1. Write `EtaParserTest.kt` with 14 cases. Commit RED: `test(439-05): add failing tests for EtaParser`.
2. Write `EtaParser.kt`. Commit GREEN: `feat(439-05): implement EtaParser`.
3. Refactor if needed. Commit REFACTOR: `refactor(439-05): extract pattern table`.
4. Write `BlinkitConfirmationParserTest.kt`. Commit RED.
5. Write `BlinkitConfirmationParser.kt`. Commit GREEN.
6. Add one new outcome: `ORDER_PLACED_BUT_NUMBER_UNREADABLE`. Update `Outcome` enum + server-side `status` enum + admin dashboard status badge handling.
7. Wire BlinkitConfirmationParser into BlinkitCheckoutFlow's success path (from 439-04).

#### Acceptance

- All 14 EtaParser cases green.
- All BlinkitConfirmationParser cases green.
- `ETA_PARSE_FAILED_ORDER_OK` outcome triggers `eta_parse_failed` audit event with the raw text — verified in test.
- `ORDER_PLACED_BUT_NUMBER_UNREADABLE` triggers WhatsApp alert (stub in test; real alert wiring in 439-06).

#### Commit message

```
test(439-05-RED): failing tests for EtaParser + BlinkitConfirmationParser
feat(439-05-GREEN): EtaParser handles all known Blinkit ETA formats
feat(439-05-GREEN): BlinkitConfirmationParser extracts order number + ETA
refactor(439-05): extract pattern table

Covers: BLINK-03 (data capture)
Not tested: end-to-end real order (439-08).
```

---

### 439-06-PLAN — Log-back-to-Core integration (order record + ETA event)

**Goal:** When `BlinkitDriver.placeTopUp` returns a result, the agent POSTs to racecontrol `/api/v1/reception/blinkit/order-update` with the `TopUpResult`, server updates `blinkit_orders` row, and the admin dashboard reflects the new status. On parse failure (ETA unrecognized or order number missing), WhatsApp + Discord + comms-link alerts fire per CLAUDE.md alert conventions.

**Covers:** BLINK-03 (log-back half).

**Dependencies:** 439-05

**Type:** `auto`

#### Tasks

1. Agent-side: wire `sendUpdate()` in BlinkitTriggerConsumer (stub from 439-03) to POST to `/order-update` via comms-link relay (which forwards to racecontrol, using service key). Include idempotency header `X-Idempotency-Key: <order_id>` to handle WS retries.

2. Server-side (`crates/racecontrol/src/api/reception/blinkit.rs`):
   - `POST /order-update` handler:
     - Validate idempotency — if `order_id` already has final status, return 200 with existing row (no double-update).
     - On `Outcome.SUCCESS` -> update row with `order_number`, `eta_minutes`, `eta_raw_text`, `eta_parsed_at`, `order_placed_at`, `status='placed'`.
     - On `ETA_PARSE_FAILED_ORDER_OK` -> update with `order_number`, `eta_raw_text`, status `placed_eta_unparsed`. Fire WhatsApp alert via existing WhatsApp bot channel: "Blinkit order placed but ETA unreadable: raw='<text>'".
     - On `ORDER_PLACED_BUT_NUMBER_UNREADABLE` -> status `placed_number_missing`. Fire WhatsApp + Discord alert: "Blinkit order placed but order number unreadable — check Blinkit app manually".
     - On any failure outcome -> status `failed`, write `error_code` + `error_details`.

3. Admin dashboard `/reception/blinkit/` updates (minimal):
   - OrderHistoryTable renders status badges:
     - `queued` -> gray
     - `placed` -> green
     - `placed_eta_unparsed` -> amber
     - `placed_number_missing` -> red
     - `failed` -> red
     - `rate_limited` / `outside_hours` -> gray with reason tooltip
   - Data via `GET /api/v1/reception/blinkit/orders?latest=20`, auto-refresh every 10s via WS or polling.

4. Tests:
   - Rust: `api::reception::blinkit::order_update_idempotent` — posting same order_id twice produces one audit log entry + no double alert.
   - Rust: `services::blinkit_orders::mark_failed_preserves_trigger_metadata` — failure does NOT nuke `triggered_by` / `sku_list_json`.
   - TypeScript: `TopUpStatusBadge.renders_for_each_status` — render test for all 6 status badges.

5. **Cloud parity** (CLAUDE.md DEPLOY PARITY):
   - racecontrol binary deployed venue + Bono VPS.
   - Admin dashboard rebuilt + deployed venue + Bono VPS.
   - blinkit_orders migration applied on both DBs.

#### Acceptance

- End-to-end trigger -> stub driver now returns a synthesized success (use a DEBUG_FORCE_SUCCESS flag) -> `/order-update` fires -> row updated -> admin dashboard shows the order in < 10s.
- Idempotency test: replay the same `/order-update` twice -> same state, no duplicate audit/alert.
- Alerts fire for `placed_eta_unparsed` + `placed_number_missing` in test harness.

#### Commit message

```
feat(439-06): Blinkit log-back-to-Core integration + admin dashboard status

Agent POSTs TopUpResult to /order-update (service key, idempotent).
Server updates blinkit_orders row, fires alerts on degraded outcomes.
Admin /reception/blinkit/ renders status badges + 10s auto-refresh.
DEPLOY PARITY: racecontrol + admin dashboard + DB on venue + Bono VPS.

Covers: BLINK-03 (log-back)
Not tested: humanize + rate-limit enforcement across 24h (439-07), real order (439-08).
```

---

### 439-07-PLAN — Rate-limit + humanize verification (per-day max, business hours)

**Goal:** Prove that the configured rate limit (`max_orders_per_day=3`) and humanize profile (`blinkit`) are actually enforced end-to-end. No code changes — this is a verification plan.

**Covers:** BLINK-04 (verification).

**Dependencies:** 439-06

**Type:** `auto` (time-accelerated test harness + short live drill)

#### Tasks

1. **Synthetic rate-limit test (Rust integration test):**
   - Hit `/api/v1/reception/blinkit/top-up` 5 times in a row (same minute, all with valid body).
   - Assert: first 3 return `status: queued`, 4th + 5th return `status: rate_limited, count: 3, max: 3`.
   - Assert: only 3 rows in `blinkit_orders` have `status IN ('queued','placed',...)`, 2 rows have `status='rate_limited'`.

2. **Synthetic business-hours test:**
   - Mock clock to 23:30 IST -> trigger -> expect `status: outside_hours`.
   - Mock clock to 08:30 IST -> trigger -> expect `status: queued`.

3. **Humanize delay distribution test (Kotlin unit test):**
   - Mock `Random` with a fixed seed. Run `HumanizeInterceptor.delay("blinkit")` 100 times. Collect delays.
   - Assert: all delays in [2000, 8000] ms. Mean within ±10% of 4000. Stddev within ±20% of 1500.

4. **Short live drill (Tab Plus + Blinkit test account):**
   - Fire 3 legitimate triggers spaced 10 min apart during business hours. Observe all 3 place real orders. Observe humanize delays feel human (visual spot-check; log stopwatch measurements between actions).
   - Fire a 4th trigger -> observe `rate_limited` status in admin dashboard.

5. **Audit log contains expected events:**
   - `rate_limit_allowed` -> `rate_limit_allowed` -> `rate_limit_allowed` -> `rate_limit_denied` (4th trigger).
   - `humanize_delay_applied` with per-action delay value for each of the ~20 UI actions per order.
   - `business_hours_check_pass` at each trigger.

#### Acceptance

- All synthetic tests pass.
- Live drill: 3 orders placed, 4th rejected, admin dashboard reflects correct counts, audit log has all expected events.

#### Commit message

```
test(439-07): verify rate-limit (3/day) + humanize delay distribution + business hours

Synthetic integration tests + Kotlin humanize stats + short live drill.
All 3 legitimate triggers placed orders; 4th rate-limited as expected.

Covers: BLINK-04 verification
Not tested: full 8-hour endurance drill (deferred to v50.0 milestone E2E phase 444).
```

---

### 439-08-PLAN — Integration test (real Blinkit test account, mocked staff trigger, full E2E)

**Goal:** End-to-end ship gate — real staff trigger on real admin dashboard UI -> real Blinkit order placed on real test account -> order number + ETA logged -> admin dashboard reflects. Exercises R-9 (Uday sign-off on real-money trigger).

**Covers:** BLINK-01, BLINK-02, BLINK-03, BLINK-04 (all, end-to-end).

**Dependencies:** 439-01 through 439-07, Phase 443 (admin dashboard reception view — or wire the minimal Blinkit form inline if 443 not yet shipped).

**Type:** `checkpoint:human-verify` (physical device + real Blinkit account + small real money spend).

#### Preconditions

- [ ] Phases 429-438 all at DEPLOYED-VERIFIED state.
- [ ] Pre-execution checkpoint for 439 resolved (Uday sign-off + test account provisioned).
- [ ] Blinkit test account balance / payment method live.
- [ ] Feature flag `enable_blinkit_on_tab_plus` = true.
- [ ] UI-RESEARCHER agent produced `UI-SPEC.md` for `/reception/blinkit/` form (required gate).
- [ ] Admin dashboard deployed with Blinkit page at venue + Bono VPS.

#### Drill script

1. **Capture pre-drill state:** `curl /api/v1/reception/blinkit/orders?latest=5` (baseline), device `/health`, admin dashboard screenshot.

2. **Trigger via admin dashboard (primary path):**
   - Open `https://admin.racingpoint.cloud/reception/blinkit/` (cloud) OR `http://.23:3201/reception/blinkit/` (venue).
   - Fill form: SKU = "Milk 500ml" quantity 1, note = "439-08 integration drill".
   - Click "Place emergency top-up" -> confirmation dialog shows cost estimate + SKU summary -> confirm.
   - Start stopwatch.

3. **Observe device:**
   - Persistent notification updates: "Blinkit order in progress... placing order 439-08-drill-01".
   - Screen recording of Tab Plus shows Blinkit app opening, searching Milk, adding to cart, checkout, payment tap.
   - Stopwatch target: order placed within 5 min (reasonable with humanize @ 4s mean * ~20 actions + Blinkit UI load).

4. **Observe server + dashboard:**
   - Admin dashboard shows order transitioning `queued` -> `placed`. Order number appears within 10s of placement.
   - ETA field populated (e.g., `15 mins`).

5. **Trigger via Core inventory alert (secondary path):**
   - `curl -X POST http://localhost:8080/api/v1/reception/blinkit/top-up -H "Authorization: Bearer <service_key_as_core>" -d '{"skus":[{"name":"Bread","quantity":1}], "note":"439-08 core path"}'`.
   - Observe same flow.

6. **Failure-mode drills:**
   - Trigger a 4th order same day -> observe `rate_limited` status + reason toast in admin dashboard.
   - Trigger at 23:45 IST (mock clock on server OR wait to real 23:45) -> observe `outside_hours`.
   - Simulate address mismatch by temporarily changing the venue-address substring config to a wrong value + trigger -> observe `delivery_address_mismatch` status. Revert config after.
   - Simulate ETA parse failure by modifying BlinkitConfirmationParser to return `eta_raw_text="zorkblorp"` (local patched build) -> trigger -> observe `placed_eta_unparsed` status + WhatsApp alert received.

7. **Evidence capture:**
   - Screen recording of Tab Plus (full happy path).
   - Admin dashboard screenshots at each status transition.
   - `adb pull` of audit log for the 6 triggers.
   - Blinkit app: verify the 2 real orders show up in order history with correct items + address.

#### Acceptance (all four must pass)

- [ ] SC-1 (BLINK-01): Staff-triggered top-up executes end-to-end, both admin + Core paths.
- [ ] SC-2 (BLINK-02): Real Blinkit order placed on the test account (verify in Blinkit app order history).
- [ ] SC-3 (BLINK-03): Order number + ETA logged in `blinkit_orders` + visible in admin dashboard.
- [ ] SC-4 (BLINK-04): Rate limit rejects 4th order; humanize delays measured in logs within expected distribution; business-hours gate rejects out-of-window triggers.

#### Checkpoint (human-verify)

James + Uday watch the drill (or James runs + Uday reviews evidence). Reports pass/fail for each SC with stopwatch measurements + screenshots. If any SC fails, create a gap-closure plan (439-0N or new 440-prep) and do NOT mark Phase 439 complete.

#### Artifacts to save in SUMMARY.md

- Screen recording link (Drive, redacted if any PII).
- Audit log excerpts for all 6 triggers (2 success + 4 failure-mode).
- Admin dashboard screenshots.
- Blinkit app order history screenshots (PII-redacted).
- Stopwatch measurements: end-to-end time per order.
- Explicit checkmark per SC.

#### Commit message

```
test(439-08): Phase 439 integration drill — full E2E Blinkit top-up

2 real orders placed (Milk + Bread) via admin dashboard and Core paths.
4 failure-mode drills passed (rate_limit, outside_hours, address_mismatch, eta_unparsed).
Stopwatch: Milk = 4m 12s, Bread = 3m 47s — within 5min target.

Covers: BLINK-01, BLINK-02, BLINK-03, BLINK-04 (full verification)
Milestone v50.0 Phase 11 Ship Gate: PASSED (all 3 ROADMAP success criteria).
```

---

## 6. Risks and pitfalls (Blinkit-specific)

Summary of frontmatter `risks_summary` expanded here with detailed mitigations. Each risk has a test or gate that guards against it.

| # | Risk | Detail | Mitigation | Tested in |
|---|------|--------|------------|-----------|
| R-1 | Package name drift | Blinkit = formerly Grofers; app may ship under `com.grofers.customerapp` OR `com.blinkit.customerapp` OR another name | 439-01 task 1 confirms via `pm list packages`; never hardcode | 439-01 |
| R-2 | Cart session expired | Blinkit cart times out ~15 min idle; a delayed trigger finds an empty cart | Re-add items at checkout entry; if still fails -> `CART_EXPIRED_UNRECOVERABLE` | 439-04 unit test + 439-08 manual (idle the device 20 min + trigger) |
| R-3 | Delivery address edge cases | Blinkit uses account-default address; venue address must be default, and driver must verify | Substring-match on `BLINKIT_VENUE_ADDRESS_SUBSTRING` config; abort on mismatch | 439-04 test + 439-08 manual |
| R-4 | ETA text parsing fragility | 10+ known ETA formats; unknowns should log + alert | 14-case TDD table in 439-05; fallback outcome `ETA_PARSE_FAILED_ORDER_OK` never silently null-returns | 439-05 |
| R-5 | Out-of-stock at checkout | Blinkit may silently remove items from cart | Verify cart item count at checkout entry | 439-04 |
| R-6 | Payment screen stuck | Saved payment expired or requires OTP | 30s timeout + `PAYMENT_STEP_STUCK` + WhatsApp alert; NEVER auto-enter PINs/OTPs (ToS red line) | 439-04 test + 439-08 (pre-expire the saved payment to force the failure) |
| R-7 | Rate limit race | Two triggers at once | Atomic counter at trigger consumer entry | 439-07 synthetic |
| R-8 | ToS / bot detection | Blinkit flags bot-like pacing | Humanize profile wider than HyperPure; max 3/day; business hours only | 439-07 distribution test + 439-08 manual observation |
| R-9 | Real-money trigger | Admin dashboard must require explicit confirmation | Confirmation dialog with cost estimate; Uday sign-off before 439-01 executes | Pre-execution checkpoint; UI-SPEC gate |
| R-10 | Device assignment ambiguity | Tab Plus or M07? | OQ-1 resolved in 439-01; capability registry (CAPREG-01) enforces | 439-01 |

## 7. Test plan

### Unit tests (JVM, fast, on every build)
- `BlinkitDriverTest` (439-02)
- `BlinkitTriggerConsumerTest` (439-03)
- `BlinkitCheckoutFlowTest` (439-04)
- `EtaParserTest` (439-05, 14 cases)
- `BlinkitConfirmationParserTest` (439-05)

All run via `./gradlew :app:testDebugUnitTest`.

### Rust integration tests (venue DB fixture)
- `api::reception::blinkit::top_up_rate_limit` (439-07)
- `api::reception::blinkit::top_up_outside_hours` (439-07)
- `api::reception::blinkit::order_update_idempotent` (439-06)
- `services::blinkit_orders::count_today_ignores_rate_limited_rows` (439-03)

Run via `cargo test -p racecontrol-crate --features test-fixtures`.

### Frontend tests
- `TopUpTriggerForm.renders_confirmation_dialog` (439-03)
- `TopUpStatusBadge.renders_for_each_status` (439-06)

### Physical device tests (human-verify)
- 439-01 checkpoint: selector capture.
- 439-02 checkpoint: install + healthCheck.
- 439-07 short live drill: 3 real orders + 1 rate-limited.
- 439-08 full drill: 2 real orders + 4 failure modes.

### Subagent audits (per §1 gates)
- UI-RESEARCHER: produces `UI-SPEC.md` for admin form BEFORE 439-03 admin page lands.
- UI-AUDITOR: produces `UI-REVIEW.md` AFTER 439-06 admin page deployed.
- NYQUIST-AUDITOR: runs against EtaParser + BlinkitConfirmationParser + BlinkitTriggerConsumer after 439-05/06.
- MMA AUDIT: runs before 439-08 ship gate, dual reasoning modes. Budget $5.
- INTEGRATION-CHECKER: runs before v50.0 milestone ship (after 439-08).

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** BlinkitTriggerConsumer + EtaParser + BlinkitConfirmationParser are business logic with defined I/O. Run `gsd-nyquist-auditor` before 439-08.
- **MMA audit (required — cross-system bridge + ToS-risky):** Kotlin <-> Node.js <-> Rust <-> Next.js <-> SQLite. Dual reasoning modes. Run before 439-08. Budget: $5.
- **UI-researcher (required):** `/reception/blinkit/` is staff-facing. UI-SPEC.md before admin page lands in 439-03/06.
- **UI-auditor (required):** UI-REVIEW.md after admin page deployed.
- **integration-checker (required):** multi-phase, multi-language; run before milestone ship.
- **codebase-mapper:** skip (no new top-level modules).
- **SEC gate:** `node comms-link/test/security-check.js` must pass after 439-03 amends relay identity allowlist with `blinkit_trigger` message type. Service-key check on `/order-update`.
- **DMP:** frontmatter `deploy:` checklist ticked by executor; verifier confirms deployed state matches.
- **Backlog gate (CLAUDE.md CGP v4.3):** Phase 439 at DEPLOYED-VERIFIED before v50.0 can ship. COMMITTED ≠ SHIPPED.

## 9. Open questions the planner cannot decide

**OQ-1 — Tab Plus vs M07 for Blinkit driver.**
Default recommendation: **Tab Plus.** Rationale: larger screen -> more stable selector matching; Blinkit app UI is less dense on a tablet vs a phone. Fallback: M07, if Tab Plus is dedicated to HyperPure (Phase 438 decision dependency). Final call in 439-01 per capability registry (CAPREG-01). **USER TO CONFIRM before 439-01 kicks off.**

**OQ-2 — Blinkit package name.**
Proposed: discovered via `pm list packages` in 439-01. But if the user has prior knowledge (e.g., "Blinkit app is currently `com.grofers.customerapp` version 15.11.4 on both devices"), that short-cuts 439-01 task 1. **USER TO CONFIRM or defer to 439-01.**

**OQ-3 — Blinkit account strategy.**
Options:
- (a) Real staff account (e.g., Uday's personal Blinkit account that he already uses for the venue).
- (b) Dedicated `racingpoint-ops@gmail.com` Blinkit account created for automation.

Recommendation: **(b)** — separation of concerns; ToS incident on (b) doesn't affect Uday's personal account. If Blinkit requires KYC / phone verification, (b) uses a venue-owned SIM (or Uday's number with a note). **USER TO DECIDE before pre-execution checkpoint closes.**

**OQ-4 — Test-account ordering to venue address.**
If OQ-3 goes with (b) a dedicated account, the first time Blinkit is opened the account has no default address. Setting the venue as default is a one-time manual task. Confirm: does James set this up, or does Uday? **Recommended: James, with Uday present to confirm address string matches config.**

**OQ-5 — Confirmation dialog cost estimate source.**
Admin dashboard confirmation dialog should show "Expected cost: ₹XXX-YYY" so staff know what they're authorizing. Where does this estimate come from? Options:
- (a) Last order's per-SKU cost from `blinkit_orders` history.
- (b) Static mapping in config (e.g., `Milk 500ml = ₹30-35`).
- (c) No estimate — just show SKU list and rely on staff judgement.

**Recommendation: (a) with (c) fallback** — first trigger for a SKU has no history, show "first-order estimate unavailable". **USER TO CONFIRM.**

**OQ-6 — Humanize profile parameters.**
Proposed: `mean=4000ms, stddev=1500ms, min=2000ms, max=8000ms` (wider than HyperPure). This is a conservative guess based on ToS-risk asymmetry (Blinkit likely has tighter bot detection than HyperPure's B2B flow). Actual tuning may need adjustment after 439-08 drill observations. **Default accepted; revisit if drill shows issues.**

**OQ-7 — Alert channels for degraded outcomes.**
`placed_eta_unparsed`, `placed_number_missing`, `eta_parse_failed` should alert staff. Channels:
- WhatsApp (via existing bot) — YES.
- Discord (via existing bot) — YES.
- comms-link WhatsApp alert to Uday personal — only for `placed_number_missing` (most severe).
- Admin dashboard badge — YES for all.

**Recommendation: matches above. USER TO CONFIRM.**

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` BLINK-01..04 block
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Structure template:** `.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md`
- **Driver framework dependency:** Phase 432
- **Selector DSL dependency:** Phase 433
- **Credential dependency:** Phase 434
- **Humanize + audit dependency:** Phase 435
- **Feature-flag dependency:** Phase 436
- **Sibling-parallel P1 driver:** Phase 437 (Zomato)
- **Sibling-parallel P2 driver:** Phase 438 (HyperPure)
- **Admin reception view (future):** Phase 443

## 11. Pre-execution checkpoint (BLOCKS Phase 439 kickoff)

Before 439-01 executes, Uday must sign off on:

1. [ ] Blinkit test account usage authorized (ToS risk acknowledged, MEDIUM).
2. [ ] Blinkit account decision from OQ-3 (a or b).
3. [ ] Test account provisioned with:
   - Venue delivery address set as default.
   - At least one saved payment method (UPI or card).
   - Phone number verified.
4. [ ] Blinkit app installed + signed in on Tab Plus (PersistentSession — user logs in once).
5. [ ] OQ-1 (Tab Plus vs M07) decided.
6. [ ] Budget authorized for 439-08 drill real orders (~₹200-500 total for 2-3 small orders).
7. [ ] WhatsApp + Discord alert channels active (inherited from Phase 437 setup — verify not broken).

Resume signal: Uday replies "Phase 439 pre-exec checklist signed off — go" or lists blockers.

## 12. Output (at phase close)

At the end of Plan 439-08 (E2E drill pass), create `.planning/phases/439-blinkit-driver/SUMMARY.md` capturing:

- Which commits implemented each plan (439-01 through 439-08).
- Actual stopwatch measurements for success criteria SC-1..SC-4.
- Real order numbers placed (redacted where needed).
- Screen recording + dashboard screenshot links.
- ETA parser format coverage stats (how many formats encountered in the wild vs planned 14).
- Any risks encountered and how they were resolved.
- Any open questions resolved during execution (update §9 state).
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked.
- Handoff to Phase 443 (admin reception view) — what's ready for reuse.

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 11 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
