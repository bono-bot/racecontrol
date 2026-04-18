---
phase: 438-hyperpure-driver
phase_number: 438
milestone: v50.0 rc-agent-mobile
name: "HyperPure Driver (P2) — Bulk Supply Reorder from Core Inventory Trigger"
status: ready-to-execute
goal: >
  Ship the second production AppDriver — a HyperPure (com.hyperpure) driver that accepts
  bulk-order manifests (SKU + quantity list) from RaceControl Core when cafe inventory
  depletion is detected, navigates the HyperPure Android app, adds each SKU to cart,
  handles out-of-stock deterministically (skip + log + staff alert), proceeds through
  checkout, captures the confirmation number + scheduled delivery window, and logs the
  result back to Core via a new `POST /api/v1/inventory/order-placed` endpoint.
  Respects the Phase 435 humanize layer (business-hours gate 08:00-23:00 IST) and
  enforces a per-app rate limit of max 3 orders per day (configurable via the
  Phase 436 feature-flag system). Reuses the Phase 437 PersistentSession + AppDriver +
  audit-log + feature-flag infrastructure — HyperPure is NOT a net-new framework;
  it is the second production plugin, proving the framework's pluggability.
requirements: [HYPER-01, HYPER-02, HYPER-03, HYPER-04, HYPER-05]
depends_on: [437]                # Phase 437 Zomato driver establishes AppDriver + PersistentSession + ToS gates
wave: 11                         # After wave 10 (Phase 437 Zomato)
plan_count: 9
plans:
  - 438-01-PLAN: HyperPure selector-map authoring (James, pre-execution debug capture)
  - 438-02-PLAN: HyperPureDriver scaffold — AppDriver impl + drivers.json manifest entry
  - 438-03-PLAN: Order-trigger consumer — comms-link bulk-order manifest listener
  - 438-04-PLAN: Core side — cafe_stock depletion emits BulkOrderManifest + new /order-placed endpoint
  - 438-05-PLAN: Cart-population flow — iterate SKU list, add each to cart
  - 438-06-PLAN: Out-of-stock detection + skip + staff WhatsApp alert
  - 438-07-PLAN: Checkout flow — navigate cart, confirm, capture confirmation number + delivery window
  - 438-08-PLAN: Rate limit (max 3/day) + business-hours gate wiring + kill-switch
  - 438-09-PLAN: E2E integration test — mock Core manifest (5 SKUs, 1 OOS) on Tab Plus
autonomous: false                # 438-01 (James selector capture) + 438-09 (physical device drill) are human-verify checkpoints
files_modified:
  # Android (Kotlin) — rc-agent-mobile
  - rc-agent-mobile/app-drivers/hyperpure/                           # new driver module directory
  - rc-agent-mobile/app-drivers/hyperpure/v<current>/selectors.yaml  # authored in 438-01 by James
  - rc-agent-mobile/app-drivers/hyperpure/manifest.json              # driver metadata entry
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/HyperPureDriver.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/BulkOrderConsumer.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/CartPopulator.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/OutOfStockHandler.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/CheckoutFlow.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/OrderRateLimiter.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/HyperPureProtocol.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/hyperpure/
  - rc-agent-mobile/drivers.json                                     # add HyperPure entry
  # Rust (racecontrol) — Core inventory endpoint + depletion dispatcher
  - crates/racecontrol/src/cafe_stock.rs                             # amend: depletion → bulk-order builder
  - crates/racecontrol/src/inventory_dispatch.rs                     # NEW: BulkOrderManifest builder + dispatcher
  - crates/racecontrol/src/api/inventory.rs                          # NEW: POST /api/v1/inventory/order-placed
  - crates/racecontrol/src/api/routes.rs                             # wire new route (staff-auth or service-key)
  - crates/racecontrol/src/db/migrate_hyperpure_orders.rs            # NEW: hyperpure_orders table
  - crates/rc-common/src/protocol.rs                                 # NEW: BulkOrderManifest + OrderPlaced message types
  # Shared docs
  - rc-agent-mobile/docs/PROTOCOL.md                                 # amend §Drivers > HyperPure
  - rc-agent-mobile/app-drivers/hyperpure/README.md                  # selector-map authoring notes for James
  - .planning/phases/438-hyperpure-driver/SUMMARY.md                 # filled at phase close

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [racecontrol]      # Core-side inventory endpoint + depletion dispatcher
  frontend_rebuild: [none]        # No frontend change in this phase (admin reception view is Phase 13)
  config_change: >
    racecontrol.toml [hyperpure] section added: enabled=false (opt-in), max_orders_per_day=3,
    business_hours_start="08:00", business_hours_end="23:00", target_device_id="rcm-tab-plus",
    staff_alert_phone=<Uday's number>. New inventory-dispatch service-key used by rc-agent-mobile
    to POST /api/v1/inventory/order-placed — generated and recorded in racecontrol.toml
    [inventory_dispatch] service_key, distributed to the Tab Plus via first-run UX (Phase 431)
    or manual ADB-push of a .properties file for v50.0.
  db_migration: >
    hyperpure_orders table: id, manifest_id, status (pending|placed|failed|skipped),
    confirmation_number, delivery_window_start, delivery_window_end, skipped_skus (json),
    placed_at, raw_receipt_text, driver_version, device_id.  FK to cafe_items(id) via
    a sibling hyperpure_order_items child table (manifest_id, sku, qty, status).  Idempotent
    migration in db/migrate_hyperpure_orders.rs — safe to re-run.
  infrastructure: >
    Tab Plus (rcm-tab-plus) must have HyperPure installed (verify via Tab Plus Play Store
    install — package id is assumed com.hyperpure pending Plan 438-01 verification; see OQ-1).
    James must be logged into HyperPure app with a PersistentSession valid for the test account.
    HyperPure dashboard / partner app account provisioned (see OQ-2).
  data_files: >
    rc-agent-mobile/app-drivers/hyperpure/v<current>/selectors.yaml — authored by James
    pre-execution in Plan 438-01 using Phase 433's debug selector-capture mode.
    rc-agent-mobile/app-drivers/hyperpure/v<current>/stub-manifest.json — 5-SKU fixture
    used by Plan 438-09 E2E drill.
  bat_file: none
  cloud_parity:
    - "racecontrol binary deployed to Bono VPS (cloud parity — same /api/v1/inventory/order-placed endpoint)."
    - "DB migration applied on Bono VPS (hyperpure_orders table must exist on both environments so cloud sync of confirmations does not drop records)."
    - "racecontrol.toml [hyperpure] section parity — same enabled flag, rate limit, service key (distinct per environment for security)."
    - "NO cloud_parity for the Android APK itself — the APK runs on a single physical device (Tab Plus) in the venue. Bono VPS is not a target."
  targets:
    - server           # .23 — racecontrol binary with /order-placed endpoint + hyperpure_orders table
    - cloud            # Bono VPS — parity per rule
    - tab_plus         # rcm-tab-plus — HyperPure driver installed + enabled via feature flag
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  rollback:
    - "Feature flag `enable_hyperpure_on_tab_plus=false` in admin dashboard halts all HyperPure activity within 10s (Phase 436 FLAG-03)."
    - "Global `pause_all_drivers=true` halts HyperPure + every other driver fleet-wide within 10s (FLAG-04 kill-switch)."
    - "Previous APK retained at /sdcard/Download/rc-agent-mobile-prev.apk; `adb install -r <prev>` rolls back the driver code (selectors stay via hot-reload pathway)."
    - "DB: migration is additive (new table + child table); rollback = DROP TABLE hyperpure_order_items; DROP TABLE hyperpure_orders; (no data loss on non-HyperPure code)."
    - "If HyperPure places a wrong order: cancel via HyperPure app manually (Uday), mark the hyperpure_orders row status='cancelled_manual', disable feature flag, investigate via audit log + selector-miss events."

# Subagent gates (per CLAUDE.md > Subagent Gates)
gates:
  ui_researcher: skip              # No venue-facing UI built in this phase. Admin reception view is Phase 13.
  ui_auditor: skip                 # Same reason.
  nyquist_auditor: required        # Rate limiter + business-hours gate + out-of-stock skip path are business logic.
  mma_audit: required              # Cross-system bridge: Core (Rust) → comms-link (Node) → agent (Kotlin) → HyperPure (third-party Android app) → agent → Core. Four boundaries, three languages, one ToS-sensitive third party. DUAL REASONING MODES required per CLAUDE.md v27.0 rule.
  integration_checker: required    # Integration flow: cafe_stock depletion → inventory_dispatch → comms-link → BulkOrderConsumer → HyperPureDriver → /order-placed handler → hyperpure_orders DB → admin reception view. Must run before milestone ship.
  codebase_mapper: required        # Two new Rust modules (inventory_dispatch, api/inventory) + new Kotlin driver module. Refresh .planning/codebase/ after 438-04 and 438-05.
  pre_execution_checkpoint: >
    Before Plan 438-09 E2E drill: Uday sign-off + HyperPure test account provisioned
    (see OQ-2). Checkpoint is BLOCKING — do not execute the E2E drill without both.

risks_summary:
  - "R-1 HyperPure UI mid-flow drift — cart persisted across session drops is UNKNOWN. If agent crashes at checkout, cart may retain added SKUs and re-submit on next trigger (double-order)."
  - "R-2 OOS detection false negatives — HyperPure may surface 'out of stock' only after user taps 'Add to cart' and sees a toast / snackbar that auto-dismisses; selector must catch multiple markers (button disabled, grey 'Sold out' label, post-tap toast, 'Notify me' substitute button)."
  - "R-3 ToS risk (MEDIUM per PROJECT.md) — humanize delays mandatory on all actions; max-3-orders/day structurally low to avoid pattern-detection; business-hours window gates runs to human-plausible times."
  - "R-4 Confirmation capture brittleness — order-placed screen UI varies by city / store / promo banner. Selector map must include multiple confirmation patterns + fallback to OCR if selectors miss (Phase 435 audit log captures screenshot hash either way)."
  - "R-5 Cart persistence race — two manifests submitted within the same hour could interleave if BulkOrderConsumer is not serialized. Consumer MUST be a single-threaded coroutine scope with a bounded channel; second manifest queues until first completes (or fails)."
  - "R-6 Delivery window parsing — HyperPure may use relative ('tomorrow 8-10 AM') or absolute ('2026-04-19 08:00-10:00') text. Parser must handle both; on parse failure, store raw_receipt_text and flag for manual review."
  - "R-7 Core endpoint auth — /api/v1/inventory/order-placed accepts input from the agent. Must use X-Service-Key (NOT public, NOT staff JWT — the agent has no staff session). Reuses the rc-agent-service-key pattern from Phase 427 (MI Seeder)."
  - "R-8 Depletion event storm — if 5 items breach threshold simultaneously, Core must not emit 5 separate bulk-order manifests. Debounce window (default 5min) + bulk-order aggregator coalesces co-occurring depletions into a single manifest."
  - "R-9 Selector drift — HyperPure app auto-updates through Play Store. Phase 432 onAppUpdate lifecycle hook + Phase 433 selector version matching handles most cases; Phase 434 selector-miss event triggers James's attention before the driver fails silently."
  - "R-10 Silent-success anti-pattern — HyperPure app may return success animation even when order is not actually placed (e.g., backend outage, payment failure). Confirmation number + delivery window MUST both be captured before marking status='placed' in hyperpure_orders; missing either → status='placed_unverified' + staff alert."
---

# Phase 438 — HyperPure Driver (P2)

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 438 |
| Name | HyperPure Driver (P2) — Bulk Supply Reorder from Core Inventory Trigger |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | HYPER-01, HYPER-02, HYPER-03, HYPER-04, HYPER-05 |
| Dependencies | Phase 437 (Zomato Partner driver — establishes AppDriver interface, PersistentSession credential pattern, audit-log + feature-flag infrastructure, ToS mitigation gates) |
| Wave | 11 |
| Status | Ready to execute (Plan 438-01 is a James manual task — debug capture — gated on HyperPure Tab Plus install + test account) |
| Autonomous | No — 438-01 (selector capture, James human task) + 438-09 (physical-device E2E drill) are human-verify checkpoints |
| Ship test | (1) Bulk manifest of 5 SKUs (1 OOS) executes end-to-end in HyperPure app on Tab Plus; (2) OOS SKU is skipped + logged + alerted via WhatsApp; (3) Confirmation number + delivery window logged back to Core at /api/v1/inventory/order-placed, visible in hyperpure_orders DB + admin reception view; (4) Rate-limit: 4th order attempt same day returns `rate_limited`; business-hours gate: 03:00 IST attempt queues until 08:00. |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 10)

1. **End-to-end execution:** Bulk order manifest (SKU + quantity list) from Core executes end-to-end in HyperPure app.
2. **OOS handling:** Out-of-stock SKUs skipped + logged + alerted to staff.
3. **Confirmation loopback:** Order confirmation number + delivery window logged back to Core.
4. **Rate + hours enforcement:** Business-hours + max-orders-per-day limits enforced.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable — Uday/staff perspective)

- **T-1 (SC-1):** When cafe_stock breaches threshold for N countable items, Core emits a single BulkOrderManifest within the debounce window (default 5min) and it reaches the Tab Plus agent's BulkOrderConsumer within 10s (observable: Core audit log + agent `/logs/tail` both show the manifest_id within seconds of each other).
- **T-2 (SC-1):** HyperPureDriver opens the HyperPure app, navigates to each SKU, taps "Add to cart", iterates through the full manifest, and arrives at the checkout screen (observable: screenshot in audit log at each step — Phase 435 audit stores screenshot hashes).
- **T-3 (SC-2):** If HyperPure marks a SKU as "Sold out" / "Out of stock" / "Notify me", the driver does NOT tap "Add to cart" for that SKU (observable: audit log `skipped_sku` event with reason); staff receive a WhatsApp message within 30s listing the skipped SKU(s) and the manifest_id.
- **T-4 (SC-2):** OOS skip does NOT abort the manifest — remaining SKUs continue processing.
- **T-5 (SC-3):** After checkout confirms, the driver reads the confirmation number (e.g., `HP-2026-041812345`) and the scheduled delivery window (e.g., "Tomorrow, 8-10 AM") from the order-placed screen and POSTs them to Core `/api/v1/inventory/order-placed` (observable: `hyperpure_orders` DB row with status='placed', confirmation_number populated, delivery_window_start + delivery_window_end parsed).
- **T-6 (SC-3):** If confirmation number OR delivery window fails to parse, status='placed_unverified' is recorded + staff alert fires (no silent success).
- **T-7 (SC-4):** With `hyperpure.max_orders_per_day=3` and 3 successful orders recorded today: the 4th trigger is rejected with `rate_limited` reason in the agent audit log (observable: `hyperpure_orders` has exactly 3 placed rows for today + 1 `rate_limited` row with no HyperPure app interaction).
- **T-8 (SC-4):** Outside business hours (e.g., 03:00 IST with window 08:00–23:00): manifest queues; the driver does NOT open the HyperPure app; at 08:00 IST the queued manifest is picked up and processed.
- **T-9 (kill-switch):** Toggling `enable_hyperpure_on_tab_plus=false` halts the driver within 10s per FLAG-03; any in-flight cart population aborts at the next tick (driver's `uninstall()` hook is called — partial-cart state is recorded in audit log as `aborted_by_kill_switch`).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/app-drivers/hyperpure/v<current>/selectors.yaml` | Selector map for HyperPure screens | 80 | Screens: home, search, sku_detail, cart, checkout, confirmation; elements: search_input, add_to_cart_btn, sold_out_marker, cart_icon, checkout_btn, confirm_order_btn, confirmation_number_label, delivery_window_label. Each element MUST have `primary` + at least one `fallback` selector strategy. |
| `rc-agent-mobile/app-drivers/hyperpure/manifest.json` | Driver metadata | 25 | `{driver_id: "hyperpure", package: "com.hyperpure", supported_device_types: ["tablet"], credential_strategy: "PersistentSession", business_hours_gated: true, rate_limited: true, max_orders_per_day_default: 3}` |
| `.../drivers/hyperpure/HyperPureDriver.kt` | AppDriver impl | 150 | `install()`, `onAppUpdate()`, `healthCheck()`, `uninstall()`, `onBulkOrderReceived(manifest)`, delegates to CartPopulator + CheckoutFlow + OutOfStockHandler |
| `.../drivers/hyperpure/BulkOrderConsumer.kt` | Listens for BulkOrderManifest from comms-link | 80 | Single-threaded coroutine scope, bounded channel (capacity 4), serializes manifest processing (R-5 mitigation) |
| `.../drivers/hyperpure/CartPopulator.kt` | Iterate SKU list, add each to cart | 120 | For each SKU: navigate to SKU (search or direct nav), detect OOS (delegate to OutOfStockHandler), tap "Add to cart", verify cart-count increment, emit audit event per SKU |
| `.../drivers/hyperpure/OutOfStockHandler.kt` | OOS detection + skip + alert | 60 | Selector-based OOS markers (≥4 variants per R-2); on detect: emit `SkippedSkuEvent`, queue a per-manifest staff WhatsApp alert (coalesced at manifest close) |
| `.../drivers/hyperpure/CheckoutFlow.kt` | Navigate cart → checkout → confirm → capture | 120 | Tap cart icon, tap checkout, tap confirm, wait-for-confirmation-screen (timeout 60s), read confirmation_number via selector (fallback: OCR via Phase 433 screen capture), read delivery_window text, parse, POST /order-placed |
| `.../drivers/hyperpure/OrderRateLimiter.kt` | Per-day rate limit + business-hours gate | 80 | SQLite-backed counter (today-UTC boundary configurable), reads `hyperpure.max_orders_per_day` from feature flags, checks business_hours window before dispatch, queues out-of-window manifests |
| `.../drivers/hyperpure/HyperPureProtocol.kt` | Typed messages | 50 | `@Serializable BulkOrderManifest`, `@Serializable OrderPlacedReceipt`, `@Serializable SkippedSkuEvent` |
| `crates/racecontrol/src/inventory_dispatch.rs` | Core-side manifest builder + dispatcher | 120 | Listens on cafe_stock depletion events, debounces (5min), aggregates into BulkOrderManifest, sends via comms-link to target_device_id, records `hyperpure_orders` row with status='dispatched' |
| `crates/racecontrol/src/api/inventory.rs` | POST /api/v1/inventory/order-placed | 80 | X-Service-Key auth, accepts `OrderPlacedReceipt`, updates `hyperpure_orders` row, triggers Phase 13 admin reception view WS event |
| `crates/racecontrol/src/db/migrate_hyperpure_orders.rs` | DB migration | 60 | `CREATE TABLE IF NOT EXISTS hyperpure_orders (...)` + `CREATE TABLE IF NOT EXISTS hyperpure_order_items (...)` + `ALTER TABLE` columns for backfill safety (CLAUDE.md DB migration rule) |
| `crates/rc-common/src/protocol.rs` amendment | BulkOrderManifest + OrderPlaced message types | 40 | `@Serializable` Rust types that match Kotlin `HyperPureProtocol.kt` byte-for-byte (JSON tagged) |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| `cafe_stock::check_low_stock_alerts` | `inventory_dispatch::enqueue_depletion` | Rust function call | grep `inventory_dispatch::enqueue_depletion` in `cafe_stock.rs` |
| `inventory_dispatch::flush_debounced` | comms-link send | Rust `ws::send` call | grep `send_to_device.*rcm-tab-plus` in `inventory_dispatch.rs` |
| comms-link relay | `BulkOrderConsumer.onMessage` | WS message with `type: "bulk_order_manifest"` | grep `bulk_order_manifest` in both `HyperPureProtocol.kt` AND `protocol.rs` (SERDE TAG PARITY — CLAUDE.md cross-boundary serialization rule) |
| `BulkOrderConsumer` | `OrderRateLimiter.admit(manifest)` | Kotlin call | grep `OrderRateLimiter.admit` in `BulkOrderConsumer.kt` |
| `OrderRateLimiter.admit` | `HyperPureDriver.onBulkOrderReceived` | Kotlin call (after admit returns true) | grep `onBulkOrderReceived` in `OrderRateLimiter.kt` |
| `HyperPureDriver.onBulkOrderReceived` | `CartPopulator.populate(manifest)` | Kotlin call | grep `CartPopulator.populate` in `HyperPureDriver.kt` |
| `CartPopulator.addSku` | `OutOfStockHandler.checkBeforeAddToCart` | Kotlin call | grep `OutOfStockHandler.checkBeforeAddToCart` in `CartPopulator.kt` |
| `CheckoutFlow.captureConfirmation` | `POST /api/v1/inventory/order-placed` | OkHttp POST | grep `/api/v1/inventory/order-placed` in `CheckoutFlow.kt` |
| `POST /order-placed` handler | `hyperpure_orders` UPDATE | sqlx query | grep `UPDATE hyperpure_orders` in `api/inventory.rs` |
| `api/inventory` | admin reception view (Phase 13) | WS broadcast `OrderPlacedEvent` | grep `OrderPlacedEvent` in `api/inventory.rs` |
| feature flag `enable_hyperpure_on_tab_plus=false` | `HyperPureDriver.uninstall()` | AppDriver lifecycle hook (Phase 432) | grep `uninstall` in `HyperPureDriver.kt` + Phase 432 flag-change dispatcher |
| feature flag `pause_all_drivers=true` | ALL drivers halt | Phase 436 kill-switch dispatcher | verify via Phase 436 FLAG-04 kill-switch test (MANDATORY — not re-implemented here, just verified to cover HyperPure) |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./comms-link/CLAUDE.md
@./comms-link/docs/PROTOCOL.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md                                       # v50.0 section at top — app priorities + ToS posture
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md   # Kotlin agent structure template
@./.planning/phases/437-zomato-partner-driver/PLAN.md         # AppDriver pattern, PersistentSession, audit log reuse
@./crates/racecontrol/src/cafe_stock.rs                       # depletion trigger origin
@./crates/racecontrol/src/cafe_alerts.rs                      # existing alert pattern to mirror
@./crates/rc-common/src/protocol.rs                           # types to extend with BulkOrderManifest + OrderPlacedReceipt

### Interfaces executors will need

The HyperPure driver reuses three Phase 437 interfaces:

1. **`AppDriver` interface** (defined in Phase 432):
   ```kotlin
   interface AppDriver {
       val driverId: String                        // "hyperpure"
       val packageName: String                     // "com.hyperpure"  (TENTATIVE — see OQ-1)
       val supportedDeviceTypes: List<DeviceType>  // [TABLET]
       val credentialStrategy: CredentialStrategy  // PersistentSession
       suspend fun install(context: DriverContext)
       suspend fun onAppUpdate(oldVersion: String, newVersion: String)
       suspend fun healthCheck(): HealthStatus
       suspend fun uninstall(reason: UninstallReason)
   }
   ```
2. **`PersistentSession` credential strategy** (defined in Phase 434): HyperPureDriver declares this in its manifest; session-expiry detection + staff alert already exist — no net-new work in Phase 438 for auth.
3. **`AuditLog.emit(event)`** (defined in Phase 435): every UI action, OOS skip, and order confirmation emits through the shared audit log; Phase 438 only defines new event subtypes (`BulkOrderReceivedEvent`, `SkippedSkuEvent`, `OrderPlacedEvent`, `OrderFailedEvent`).

Key new interface introduced in this phase:

```kotlin
// HyperPureProtocol.kt — wire-compatible with Rust crates/rc-common/src/protocol.rs
@Serializable
data class BulkOrderManifest(
    val manifest_id: String,                // UUID generated by Core inventory_dispatch
    val issued_at_ms: Long,
    val target_device_id: String,           // "rcm-tab-plus"
    val items: List<BulkOrderItem>,
    val source: String,                     // "cafe_stock_depletion"
    val callback_service_key_hint: String?  // optional: if the device needs to refresh its service key
)

@Serializable
data class BulkOrderItem(
    val sku: String,                        // HyperPure product SKU (NOT the internal cafe_items.id — see OQ-4 on SKU mapping)
    val quantity: Int,
    val cafe_item_id: String,               // round-trip link back to Core cafe_items.id for reconciliation
    val display_name: String                // human-readable name for audit log (e.g., "Amul Milk 1L")
)

@Serializable
data class OrderPlacedReceipt(
    val manifest_id: String,
    val status: OrderStatus,                // placed | placed_unverified | failed | skipped_all | rate_limited | out_of_hours_queued
    val confirmation_number: String?,
    val delivery_window_start_iso: String?, // ISO-8601 UTC — Kotlin parses IST display text to UTC
    val delivery_window_end_iso: String?,
    val skipped_skus: List<SkippedSku>,
    val placed_at_ms: Long,
    val raw_receipt_text: String,           // full OCR/selector capture for auditability
    val driver_version: String,
    val device_id: String
)

@Serializable
data class SkippedSku(val sku: String, val cafe_item_id: String, val reason: String)
```

```rust
// crates/rc-common/src/protocol.rs amendment — mirrored Rust types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BulkOrderManifest { /* same fields as Kotlin */ }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OrderPlacedReceipt { /* same fields as Kotlin */ }
```

**Cross-boundary serialization gate:** Before Plan 438-09, the integration checker MUST verify field-name and shape parity between `HyperPureProtocol.kt` and `crates/rc-common/src/protocol.rs`. CLAUDE.md "every kiosk/frontend field MUST have a matching Rust struct field" rule applies here too — Kotlin sends, Rust parses; mismatch = silent drop, observable only when Core never receives the confirmation.

## 5. Atomic plan breakdown (9 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

---

### 438-01-PLAN — HyperPure selector-map authoring (James, human task)

**Goal:** James uses the Phase 433 selector debug-capture mode on the Tab Plus to produce `rc-agent-mobile/app-drivers/hyperpure/v<HyperPure-version>/selectors.yaml`, committed before 438-02 begins.

**Covers:** HYPER-02 partial (selectors are the substrate the driver acts on)

**Dependencies:** Phase 433 (selector hot-reload + debug capture), HyperPure installed on Tab Plus, James logged in with test account

**Type:** `checkpoint:human-verify`

#### Preconditions
- Tab Plus has HyperPure app installed (verify package: `adb shell pm list packages | grep -i hyperpure`; confirms OQ-1).
- James is logged in (PersistentSession — test account).
- Phase 433 debug mode is accessible on the agent (enabled via admin or `local.properties`).

#### James's capture sequence (MANDATORY order — each screen in sequence)

1. **home** — after app launch, top-of-home screen. Elements: `search_input`, `cart_icon`, `greeting_banner` (to detect "logged in" state).
2. **search** — tap search, type one known SKU. Elements: `search_input` (with focus), `search_result_item_first`.
3. **sku_detail** — tap first result. Elements: `sku_name_label`, `sku_price_label`, `add_to_cart_btn`, `quantity_input` (may be +/- pair), `sold_out_marker` (capture by forcing an OOS SKU — James picks a known-OOS item; if none available, capture from a screenshot James prepared outside the agent and add manually).
4. **cart** — tap cart icon with ≥1 item. Elements: `cart_item_row`, `cart_total_label`, `checkout_btn`, `empty_cart_marker`.
5. **checkout** — tap checkout. Elements: `delivery_address_label`, `delivery_window_selector`, `payment_method_selector`, `confirm_order_btn`.
6. **confirmation** — complete an order for ₹0 test item (if HyperPure allows ₹0 test SKUs — OQ-5; otherwise capture from a real order placed manually). Elements: `confirmation_number_label`, `delivery_window_label`, `order_summary_section`.

#### Output format

`selectors.yaml` structure (one file):

```yaml
app: com.hyperpure
version: "<HyperPure app version, e.g., 5.12.3>"
captured_by: james
captured_at: "2026-04-XX"
screens:
  home:
    search_input:
      primary: { strategy: resource_id, value: "com.hyperpure:id/search_edit" }
      fallback:
        - { strategy: content_description, value: "Search products" }
        - { strategy: xpath, value: "//android.widget.EditText[@hint='Search']" }
    cart_icon:
      primary: { strategy: resource_id, value: "com.hyperpure:id/cart_fab" }
      fallback: [ ... ]
  search:
    search_result_item_first:
      primary: { strategy: xpath, value: "//androidx.recyclerview.widget.RecyclerView/android.view.ViewGroup[1]" }
      fallback: [ ... ]
  sku_detail:
    add_to_cart_btn:
      primary: { strategy: resource_id, value: "com.hyperpure:id/btn_add_to_cart" }
      fallback:
        - { strategy: text, value: "Add to Cart" }
        - { strategy: text, value: "ADD TO CART" }
    sold_out_marker:
      primary: { strategy: text, value: "Sold out" }
      fallback:
        - { strategy: text, value: "Out of stock" }
        - { strategy: text, value: "Notify me" }
        - { strategy: resource_id, value: "com.hyperpure:id/sold_out_label" }
        - { strategy: content_description, value: "unavailable" }
  # ... cart, checkout, confirmation
```

**R-2 mitigation (OOS false negatives):** `sold_out_marker` MUST have ≥4 fallback variants — button disabled state, "Sold out" text, "Out of stock" text, "Notify me" substitute button, grey-out styling via content-description — so OutOfStockHandler has ≥4 independent signals to consult. Capture all of them in 438-01 even if it requires James to manually induce OOS states.

#### Commit + ship

- File: `rc-agent-mobile/app-drivers/hyperpure/v<version>/selectors.yaml`.
- Accompanying: `rc-agent-mobile/app-drivers/hyperpure/README.md` — authoring notes + which HyperPure version was captured + how to re-capture.
- Commit: `feat(438-01): hyperpure v<ver> selector map (6 screens, 32 elements, 4-variant OOS fallback)`.

#### Checkpoint (human-verify)

James confirms: all 6 screens captured, OOS markers include at least 4 fallback variants, README.md notes what was NOT capturable and why (e.g., "couldn't force a ₹0 test SKU, captured confirmation screen from screenshot").

#### G4 NOT TESTED list

- Selectors are static captures — runtime stability during the E2E drill is verified in Plan 438-09.
- Selector drift after HyperPure app update — verified on next `onAppUpdate` lifecycle fire (Phase 432).

---

### 438-02-PLAN — HyperPureDriver scaffold + manifest entry

**Goal:** Empty `HyperPureDriver` class implements `AppDriver` lifecycle with no-op bodies + manifest entry in `drivers.json` enables it under a feature flag (default OFF). Agent loads the driver on boot without crashing.

**Covers:** HYPER-01 partial, DRIVER-01..05 reuse verification

**Dependencies:** 438-01, Phase 432 (driver framework), Phase 436 (feature flags)

**Type:** `auto`

#### Tasks

1. Create `HyperPureDriver.kt`:
   ```kotlin
   class HyperPureDriver(
       private val context: Context,
       private val auditLog: AuditLog,
       private val selectorMap: SelectorMap,
       private val featureFlags: FeatureFlags,
       private val rateLimiter: OrderRateLimiter,
       private val bulkOrderConsumer: BulkOrderConsumer
   ) : AppDriver {
       override val driverId = "hyperpure"
       override val packageName = "com.hyperpure"  // see OQ-1
       override val supportedDeviceTypes = listOf(DeviceType.TABLET)
       override val credentialStrategy = PersistentSession

       override suspend fun install(context: DriverContext) {
           auditLog.emit(DriverInstalledEvent("hyperpure"))
           bulkOrderConsumer.start(onManifest = ::onBulkOrderReceived)
       }

       override suspend fun onAppUpdate(oldVersion: String, newVersion: String) {
           auditLog.emit(DriverAppUpdateEvent("hyperpure", oldVersion, newVersion))
           selectorMap.reloadForVersion(newVersion)  // Phase 433 hot-reload
       }

       override suspend fun healthCheck(): HealthStatus {
           // Verify HyperPure app installed + session valid + selector map loaded
           return HealthStatus.healthy()  // placeholder; wired in 438-05..07
       }

       override suspend fun uninstall(reason: UninstallReason) {
           bulkOrderConsumer.stop()
           auditLog.emit(DriverUninstalledEvent("hyperpure", reason))
       }

       suspend fun onBulkOrderReceived(manifest: BulkOrderManifest) {
           // Placeholder — wired in 438-05..07
           auditLog.emit(BulkOrderReceivedEvent(manifest.manifest_id, manifest.items.size))
       }
   }
   ```

2. Add manifest entry in `drivers.json`:
   ```json
   {
     "driver_id": "hyperpure",
     "class": "in.racingpoint.rcagentmobile.drivers.hyperpure.HyperPureDriver",
     "package": "com.hyperpure",
     "supported_device_types": ["tablet"],
     "credential_strategy": "PersistentSession",
     "business_hours_gated": true,
     "rate_limited": true,
     "max_orders_per_day_default": 3,
     "selectors_path": "app-drivers/hyperpure",
     "enabled_by_default": false,
     "required_feature_flag": "enable_hyperpure_on_<device_id>"
   }
   ```

3. Unit tests (JVM):
   - `HyperPureDriverTest.installRegistersConsumer` — mock BulkOrderConsumer, assert `start()` is called.
   - `HyperPureDriverTest.uninstallStopsConsumer` — assert `stop()` is called with the correct UninstallReason.
   - `HyperPureDriverTest.healthyWhenSelectorMapLoaded` — mock loaded map, assert HealthStatus.healthy().
   - `HyperPureDriverTest.unhealthyWhenSessionExpired` — mock PersistentSession.isValid() = false, assert HealthStatus.unhealthy(reason="session_expired").

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*HyperPureDriver*'` passes.
- APK builds; on install, `adb shell cat /sdcard/Android/data/.../files/logs/rc-agent-mobile.log.jsonl | grep driver_installed` shows `{"driver": "hyperpure", ...}`.
- Toggling `enable_hyperpure_on_rcm-tab-plus=true` fires `install()` within 10s (FLAG-03 verification).
- Toggling it back to `false` fires `uninstall()` within 10s.

#### G4 NOT TESTED list
- Actual HyperPure app interaction (438-05..07)
- BulkOrderConsumer payload handling beyond "start/stop" (438-03)
- End-to-end manifest execution (438-09)

#### Commit message
```
feat(438-02): HyperPureDriver scaffold + drivers.json entry

Empty AppDriver impl with install/uninstall/healthCheck/onAppUpdate.
Feature-flag gated (enable_hyperpure_on_<device_id>, default false).
Registered with BulkOrderConsumer for manifest dispatch (payload handling stubs).

Covers: HYPER-01 (partial), DRIVER-01..05 reuse verified.
Not tested: HyperPure UI interaction (438-05..07), E2E (438-09).
```

---

### 438-03-PLAN — Order-trigger consumer (BulkOrderConsumer)

**Goal:** Agent listens for `{"type": "bulk_order_manifest"}` messages from comms-link, deserializes into `BulkOrderManifest`, and dispatches to the registered handler. Single-threaded, bounded-channel serialization (R-5 mitigation).

**Covers:** HYPER-01 (core of "accepts bulk order manifests")

**Dependencies:** 438-02, Phase 429 (comms-link client), Phase 432 (driver framework)

**Type:** `auto`

#### Tasks

1. Create `BulkOrderConsumer.kt`:
   ```kotlin
   class BulkOrderConsumer(
       private val commsLinkClient: CommsLinkClient,
       private val auditLog: AuditLog,
       private val persistentSession: PersistentSession
   ) {
       private val channel = Channel<BulkOrderManifest>(capacity = 4, onBufferOverflow = DROP_OLDEST_WITH_ALERT)
       private var processorJob: Job? = null
       private var onManifest: (suspend (BulkOrderManifest) -> Unit)? = null

       fun start(onManifest: suspend (BulkOrderManifest) -> Unit) {
           this.onManifest = onManifest
           commsLinkClient.registerHandler("bulk_order_manifest") { json ->
               val manifest = Json.decodeFromString<BulkOrderManifest>(json)
               val accepted = channel.trySend(manifest).isSuccess
               if (!accepted) {
                   auditLog.emit(BulkOrderDroppedEvent(manifest.manifest_id, reason = "channel_full"))
                   // Also report back to Core so Uday is not blind to a dropped order
                   reportDroppedToCore(manifest)
               }
           }
           processorJob = scope.launch {
               for (manifest in channel) {
                   try {
                       if (!persistentSession.isValid()) {
                           auditLog.emit(ManifestFailedEvent(manifest.manifest_id, "session_expired"))
                           reportFailedToCore(manifest, OrderStatus.FAILED, "session_expired")
                           continue
                       }
                       onManifest?.invoke(manifest)
                   } catch (e: CancellationException) { throw e }
                   catch (e: Throwable) {
                       auditLog.emit(ManifestFailedEvent(manifest.manifest_id, e.stackTraceToString()))
                       reportFailedToCore(manifest, OrderStatus.FAILED, "exception:${e::class.simpleName}")
                   }
               }
           }
       }

       fun stop() { processorJob?.cancel(); commsLinkClient.unregisterHandler("bulk_order_manifest") }
   }
   ```

2. **DROP_OLDEST_WITH_ALERT semantics:** If the channel is full (4 manifests queued, 5th arrives), the oldest is dropped AND a `BulkOrderDroppedEvent` fires with `reason="channel_full"` AND Core is notified via `reportDroppedToCore` (POST /order-placed with status=failed, manifest_id of the dropped manifest). **Never silently drop — the user must know.**

3. **Lock-across-await audit:** Use `kotlinx-coroutines-debug` to verify no mutex/semaphore is held across the `channel.send` or `onManifest.invoke` calls. CLAUDE.md "Never hold a lock across .await" rule applies.

4. Tests:
   - `BulkOrderConsumerTest.singleManifestProcessed` — send 1 manifest, assert handler called once with correct payload.
   - `BulkOrderConsumerTest.serializedProcessing` — send 3 manifests rapidly, assert they process in order (second starts only after first completes).
   - `BulkOrderConsumerTest.channelFullDropsOldest` — send 5 manifests (capacity 4), assert oldest dropped + `BulkOrderDroppedEvent` fired + `reportDroppedToCore` called.
   - `BulkOrderConsumerTest.sessionExpiredSkipsHandler` — mock PersistentSession.isValid() = false, assert `onManifest` NOT called + `ManifestFailedEvent` fired + `reportFailedToCore` called.
   - `BulkOrderConsumerTest.handlerExceptionReportedToCore` — handler throws, assert `reportFailedToCore` called with `exception:IOException`.
   - `BulkOrderConsumerTest.stopCancelsProcessor` — stop(), assert processorJob is cancelled AND comms-link handler unregistered.

#### Acceptance

- All unit tests pass.
- Integration (local): send a crafted manifest via comms-link relay, observe audit log + handler invocation (via instrumented test on device).

#### G4 NOT TESTED list
- Actual HyperPure UI interaction (438-05..07)
- Real comms-link relay dispatch from Core (438-04 wires Core side)
- E2E end-to-end (438-09)

#### Commit message
```
feat(438-03): BulkOrderConsumer — serialized manifest processing

Single-threaded coroutine processor, bounded channel (capacity 4, drop-oldest-with-alert).
Session-expired manifests skipped + reported to Core. Exceptions captured + reported.
No silent drops — Core always learns of dropped or failed manifests.

Covers: HYPER-01 (core accept/dispatch path)
Not tested: HyperPure UI interaction, Core-side dispatch (438-04).
```

---

### 438-04-PLAN — Core-side dispatcher + /order-placed endpoint + DB migration

**Goal:** Racecontrol server detects cafe_stock depletion, debounces 5min, aggregates into a `BulkOrderManifest`, sends it to rcm-tab-plus via comms-link, persists `hyperpure_orders` row with status='dispatched'. Exposes `POST /api/v1/inventory/order-placed` (X-Service-Key auth) that accepts `OrderPlacedReceipt` from the agent and updates the row.

**Covers:** HYPER-01 (trigger), HYPER-03 (loopback endpoint)

**Dependencies:** 438-02 (Kotlin protocol types), existing cafe_stock.rs + cafe_alerts.rs

**Type:** `auto` (server-side only — no device involvement)

#### Tasks

1. **DB migration** — `crates/racecontrol/src/db/migrate_hyperpure_orders.rs`:
   ```sql
   CREATE TABLE IF NOT EXISTS hyperpure_orders (
     id TEXT PRIMARY KEY,                    -- UUID (same as manifest_id)
     status TEXT NOT NULL,                   -- dispatched|placed|placed_unverified|failed|skipped_all|rate_limited|out_of_hours_queued|cancelled_manual
     target_device_id TEXT NOT NULL,
     confirmation_number TEXT,
     delivery_window_start TEXT,             -- ISO-8601 UTC
     delivery_window_end TEXT,
     skipped_skus TEXT,                      -- JSON array of SkippedSku
     dispatched_at TEXT NOT NULL,
     placed_at TEXT,
     raw_receipt_text TEXT,
     driver_version TEXT,
     device_id TEXT,
     source TEXT NOT NULL                    -- "cafe_stock_depletion" | "staff_manual" (future)
   );
   CREATE TABLE IF NOT EXISTS hyperpure_order_items (
     id INTEGER PRIMARY KEY AUTOINCREMENT,
     order_id TEXT NOT NULL,                 -- FK to hyperpure_orders.id
     cafe_item_id TEXT NOT NULL,             -- FK to cafe_items.id
     sku TEXT NOT NULL,
     quantity INTEGER NOT NULL,
     status TEXT NOT NULL,                   -- requested|added|skipped_oos
     display_name TEXT,
     FOREIGN KEY (order_id) REFERENCES hyperpure_orders(id) ON DELETE CASCADE,
     FOREIGN KEY (cafe_item_id) REFERENCES cafe_items(id) ON DELETE RESTRICT
   );
   CREATE INDEX IF NOT EXISTS idx_hyperpure_orders_dispatched_at ON hyperpure_orders(dispatched_at);
   CREATE INDEX IF NOT EXISTS idx_hyperpure_order_items_order_id ON hyperpure_order_items(order_id);
   ```
   **CLAUDE.md DB migration rule:** Add explicit `ALTER TABLE ADD COLUMN IF NOT EXISTS` statements for every column so upgrades from older schemas don't silently miss fields. `ON DELETE RESTRICT` on cafe_item_id per CLAUDE.md's C1 FK policy (billing_sessions precedent).

2. **`inventory_dispatch.rs`** — new module:
   ```rust
   pub struct InventoryDispatcher { /* db, config, ws handle, debounce state */ }
   impl InventoryDispatcher {
       pub async fn enqueue_depletion(&self, cafe_item_id: &str) { /* add to debounce buffer, schedule flush_debounced in 5min */ }
       async fn flush_debounced(&self) {
           // 1. Collect all buffered cafe_item_ids from last 5min
           // 2. Look up hyperpure_sku mapping (OQ-4) for each cafe_item_id
           // 3. Build BulkOrderManifest with UUID, target_device_id from config
           // 4. INSERT hyperpure_orders row with status='dispatched'
           // 5. INSERT hyperpure_order_items rows with status='requested'
           // 6. Send via comms-link: commslink::send_to_device(target_device_id, "bulk_order_manifest", manifest_json)
           // 7. If comms-link send fails: UPDATE status='failed', emit WhatsApp alert
       }
   }
   ```
   Debounce: `tokio::time::sleep(Duration::from_secs(300))` from first depletion; if another depletion occurs, resets timer. Max-wait (upper bound): 15min — after 15min regardless of continued depletions, flush.

3. **`api/inventory.rs`** — new endpoint:
   ```rust
   // POST /api/v1/inventory/order-placed
   // Auth: X-Service-Key header (NOT staff JWT — agent has no staff session).
   //       Key matches racecontrol.toml [inventory_dispatch].service_key.
   pub async fn order_placed_handler(
       State(state): State<Arc<AppState>>,
       headers: HeaderMap,
       Json(receipt): Json<OrderPlacedReceipt>,
   ) -> Result<Json<Value>, StatusCode> {
       // 1. Verify X-Service-Key header
       // 2. Look up hyperpure_orders row by manifest_id
       // 3. UPDATE status = receipt.status, confirmation_number, delivery_window_*, raw_receipt_text, placed_at
       // 4. UPDATE hyperpure_order_items statuses based on receipt.skipped_skus
       // 5. Broadcast OrderPlacedEvent on WS (for Phase 13 admin reception view)
       // 6. If status=placed: WhatsApp "Order confirmed: <conf#>, delivery <window>"
       //    If status=placed_unverified: WhatsApp + staff escalation
       //    If status=failed|skipped_all: WhatsApp "Order failed: <reason>"
       //    If status=rate_limited|out_of_hours_queued: no WhatsApp (these are informational)
       // 7. Return {"ok": true, "order_id": manifest_id}
   }
   ```

4. **Wire in `routes.rs`** — add under a service-key-protected routing group (NOT `public_routes`, NOT `staff_routes`). Pattern: reuse the `/audit-check-service` pattern from Phase 427 (CLAUDE.md MI Seeder Gap 4 — service key header auth).

5. **Wire `cafe_stock.rs`**:
   - In `check_low_stock_alerts` (existing WhatsApp alert path), after the alert fires, ALSO call `inventory_dispatch.enqueue_depletion(item_id)`.
   - Config gate: only enqueue if `config.hyperpure.enabled=true` AND `item.auto_reorder_enabled=true` (new column on `cafe_items` — requires an additional migration OR a separate allowlist table; default: use existing `auto_reorder_enabled` column if present, else fall back to an allowlist in `racecontrol.toml [hyperpure].auto_reorder_skus`).

6. **racecontrol.toml additions:**
   ```toml
   [hyperpure]
   enabled = false                           # kill-switch at Core level
   target_device_id = "rcm-tab-plus"
   max_orders_per_day = 3
   business_hours_start = "08:00"
   business_hours_end = "23:00"
   debounce_secs = 300
   debounce_max_wait_secs = 900
   auto_reorder_skus = []                    # list of cafe_items.id eligible for auto-reorder (empty = none)

   [inventory_dispatch]
   service_key = "<generated UUID>"          # used by agent to POST /order-placed
   ```

7. **Rust unit tests:**
   - `inventory_dispatch_tests::debounce_aggregates_within_window` — enqueue 3 items within 1 min, assert 1 manifest emitted after 5 min.
   - `inventory_dispatch_tests::debounce_max_wait_flushes` — enqueue every 4 min continuously, assert flush at 15 min regardless.
   - `api/inventory_tests::order_placed_updates_row` — POST OrderPlacedReceipt, assert UPDATE applied.
   - `api/inventory_tests::order_placed_rejects_wrong_service_key` — POST without / with wrong X-Service-Key, assert 401.
   - `api/inventory_tests::order_placed_broadcasts_ws_event` — POST, assert WS `OrderPlacedEvent` fires.
   - `api/inventory_tests::order_placed_fires_whatsapp_on_placed` — mock WhatsApp, POST with status=placed, assert message sent.

#### Acceptance

- `cargo test -p racecontrol-crate inventory_dispatch` passes.
- `cargo test -p racecontrol-crate api::inventory` passes.
- `cargo check --bin racecontrol` passes.
- Migration runs idempotently on a fresh DB AND on a prod-clone DB (verify: `sqlx migrate run` twice in a row with no errors).
- Manual curl test: `curl -X POST http://localhost:8080/api/v1/inventory/order-placed -H "X-Service-Key: <key>" -H "Content-Type: application/json" -d @test-receipt.json` returns `{"ok": true}` and the `hyperpure_orders` row is updated.

#### G4 NOT TESTED list
- Real end-to-end with agent (438-09)
- Production deploy with real cafe_stock depletion (post-438-09 monitoring)

#### Commit message
```
feat(438-04): Core inventory dispatcher + /order-placed endpoint + DB migration

New Rust module inventory_dispatch: cafe_stock depletion → 5min debounce →
BulkOrderManifest → comms-link send to rcm-tab-plus. New endpoint
POST /api/v1/inventory/order-placed (X-Service-Key auth) receives
OrderPlacedReceipt, updates hyperpure_orders, broadcasts WS event, WhatsApp.
Migration adds hyperpure_orders + hyperpure_order_items tables (FK ON DELETE
RESTRICT on cafe_items).

Covers: HYPER-01 (trigger), HYPER-03 (loopback)
Not tested: end-to-end with physical HyperPure app (438-05..09).
```

---

### 438-05-PLAN — Cart-population flow (CartPopulator)

**Goal:** Given a BulkOrderManifest, HyperPureDriver iterates each SKU, navigates to it (via search or direct URL if HyperPure supports deep links — see OQ-3), taps "Add to cart" (after OOS check), verifies cart-count increment. Emits audit event per SKU.

**Covers:** HYPER-02 (cart population part)

**Dependencies:** 438-02, 438-03, Phase 430 (Accessibility tap/swipe/text), Phase 433 (selector map + hot-reload), Phase 435 (humanize layer)

**Type:** `auto`

#### Tasks

1. Create `CartPopulator.kt`:
   ```kotlin
   class CartPopulator(
       private val ui: UiController,              // Phase 430 tap/swipe/text primitives
       private val selectorMap: SelectorMap,      // Phase 433
       private val humanize: HumanizeInterceptor, // Phase 435
       private val oosHandler: OutOfStockHandler, // 438-06
       private val auditLog: AuditLog
   ) {
       suspend fun populate(manifest: BulkOrderManifest): CartPopulationResult {
           val skipped = mutableListOf<SkippedSku>()
           val added = mutableListOf<String>()
           ensureOnHomeScreen()
           for (item in manifest.items) {
               humanize.beforeAction(ActionType.NAVIGATION)  // randomized delay 500-2000ms
               val navResult = navigateToSku(item.sku, item.display_name)
               if (navResult.failed) { skipped.add(SkippedSku(item.sku, item.cafe_item_id, "navigation_failed")); continue }
               val oosCheck = oosHandler.checkBeforeAddToCart()
               if (oosCheck.isOutOfStock) {
                   skipped.add(SkippedSku(item.sku, item.cafe_item_id, oosCheck.marker))
                   continue
               }
               humanize.beforeAction(ActionType.TAP)
               val addResult = ui.tap(selectorMap.get("sku_detail", "add_to_cart_btn"))
               if (!addResult.success) { skipped.add(SkippedSku(item.sku, item.cafe_item_id, "add_to_cart_tap_failed")); continue }
               val cartIncremented = verifyCartIncrement(expectedDelta = item.quantity)
               if (!cartIncremented) { skipped.add(SkippedSku(item.sku, item.cafe_item_id, "cart_count_did_not_increment")); continue }
               added.add(item.sku)
               auditLog.emit(SkuAddedEvent(manifest.manifest_id, item.sku, item.quantity))
           }
           return CartPopulationResult(added, skipped)
       }
   }
   ```

2. **Quantity handling:** If `quantity > 1`, either (a) tap the quantity `+` button `quantity-1` times (preferred — avoids OOS at higher quantity), or (b) type into `quantity_input`. Choose based on what selectors were captured in 438-01. For each + tap, insert a humanize delay.

3. **Navigation strategy:**
   - **Tier 1:** If HyperPure supports deep-link URIs (e.g., `hyperpure://product/<sku>` — OQ-3), use `Intent.ACTION_VIEW` with that URI to navigate directly.
   - **Tier 2 (fallback):** Use `search_input`, type `display_name`, tap first result. This is brittle — if the first search result is a substitute product, we'll order the wrong thing. Mitigation: after tapping, read `sku_name_label` and verify it matches `display_name` (fuzzy match, e.g., Levenshtein distance ≤ 3). On mismatch, skip with reason `search_disambiguation_failed` and staff-alert.

4. **Verify cart increment:** Read cart badge count (selector: `cart_icon` has badge) before and after tap. If delta ≠ quantity, record mismatch and skip this SKU. This is the failsafe against "tap succeeded but app didn't register" (same class as CLAUDE.md "build_id match ≠ fix works" — UI success animation ≠ logical success).

5. Tests (instrumented, on Tab Plus):
   - `CartPopulatorInstrumentedTest.addSingleSku` — manifest with 1 SKU, assert cart-badge shows 1 after.
   - `CartPopulatorInstrumentedTest.addMultipleSkus` — manifest with 3 SKUs, assert cart-badge shows 3 after.
   - `CartPopulatorInstrumentedTest.skuNotFoundSkipped` — manifest with 1 bogus SKU, assert skipped with reason.
   - `CartPopulatorInstrumentedTest.humanizeDelaysInserted` — assert `humanize.beforeAction` called before each tap.
   - `CartPopulatorInstrumentedTest.searchDisambiguationMismatch` — send manifest with "Amul Milk 1L", capture situation where first result is "Amul Milk 500ml", assert skipped with `search_disambiguation_failed`.

6. Unit tests (JVM, with mocked UiController):
   - `CartPopulatorTest.navigationFailedRecorded` — mock UiController to return failed, assert skipped list contains entry with reason.
   - `CartPopulatorTest.allSkusSkippedReturnsEmptyAdded` — mock all OOS, assert `added` is empty, `skipped` has all items.

#### Acceptance

- All unit tests pass.
- Instrumented test on Tab Plus: manifest with 3 real test SKUs → cart populated, 1 OOS → skipped with reason, 1 bogus → skipped with `search_disambiguation_failed`.

#### G4 NOT TESTED list
- Checkout completion (438-07)
- Concurrent-manifest race condition under load (438-09 drill exercises serialized queue)

#### Commit message
```
feat(438-05): CartPopulator — iterate SKUs, add each to cart, verify increment

Tier 1 deep-link navigation (if HyperPure supports hyperpure://product/<sku>), Tier 2
search + disambiguation fallback. Per-SKU humanize delay. Cart-badge increment
verification catches silent-success failures. OOS check via OutOfStockHandler
(438-06). Per-SKU audit events (SkuAddedEvent, SkuSkippedEvent).

Covers: HYPER-02 (cart population)
Not tested: checkout (438-07), E2E (438-09).
```

---

### 438-06-PLAN — Out-of-stock detection + skip + staff alert

**Goal:** Deterministically detect OOS markers (≥4 variants per R-2), skip the SKU, log with reason, coalesce into a per-manifest WhatsApp alert at manifest close.

**Covers:** HYPER-05

**Dependencies:** 438-01 (selector map has OOS fallbacks), 438-02, 438-03, comms-link WhatsApp channel (existing)

**Type:** `auto`

#### Tasks

1. Create `OutOfStockHandler.kt`:
   ```kotlin
   class OutOfStockHandler(
       private val ui: UiController,
       private val selectorMap: SelectorMap,
       private val auditLog: AuditLog,
       private val whatsapp: WhatsappNotifier  // Phase 436 wraps comms-link WhatsApp channel
   ) {
       private val perManifestSkipped = mutableMapOf<String, MutableList<SkippedSku>>()

       suspend fun checkBeforeAddToCart(): OosCheckResult {
           val soldOutSelectors = selectorMap.getAllFallbacks("sku_detail", "sold_out_marker")
           for (selector in soldOutSelectors) {
               if (ui.isPresent(selector, timeoutMs = 500)) {
                   return OosCheckResult(isOutOfStock = true, marker = selector.describe())
               }
           }
           // Additional check: is "Add to cart" button disabled?
           val addBtn = ui.find(selectorMap.get("sku_detail", "add_to_cart_btn"))
           if (addBtn?.isEnabled == false) return OosCheckResult(isOutOfStock = true, marker = "add_btn_disabled")
           return OosCheckResult(isOutOfStock = false)
       }

       fun recordSkipped(manifestId: String, sku: SkippedSku) {
           perManifestSkipped.getOrPut(manifestId) { mutableListOf() }.add(sku)
           auditLog.emit(SkuSkippedEvent(manifestId, sku))
       }

       suspend fun finalizeManifest(manifestId: String) {
           val skipped = perManifestSkipped.remove(manifestId) ?: return
           if (skipped.isEmpty()) return
           val lines = skipped.joinToString("\n") { "- ${it.sku} (${it.reason})" }
           whatsapp.send(
               to = "uday",
               template = "hyperpure_oos_skips",
               body = "HyperPure manifest $manifestId: ${skipped.size} SKU(s) skipped:\n$lines"
           )
       }
   }
   ```

2. **Post-tap OOS detection (R-2 fallback):** HyperPure may only reveal OOS after you tap Add-to-cart (toast/snackbar). Wire a post-tap check in `CartPopulator` (438-05) that waits 2s for a toast containing "out of stock"/"unavailable"/"notify me"/"not available" and treats it as OOS.

3. **Coalescing rule:** One WhatsApp message per manifest, not per SKU. Message fires at manifest close (called by CheckoutFlow in 438-07) regardless of whether checkout succeeded or failed. If zero SKUs skipped, no message.

4. Tests:
   - `OutOfStockHandlerTest.detectsPrimaryMarker` — mock UI returns "Sold out" text, assert detected.
   - `OutOfStockHandlerTest.detectsFallbackMarker` — mock UI returns "Notify me" only (no primary), assert detected.
   - `OutOfStockHandlerTest.detectsDisabledAddBtn` — mock add-btn.isEnabled=false, all other markers absent, assert detected.
   - `OutOfStockHandlerTest.noFalsePositiveWhenInStock` — all markers absent, add-btn enabled, assert not detected.
   - `OutOfStockHandlerTest.coalesceWhatsappSingleMessage` — record 3 skips across 1 manifest, finalize, assert WhatsApp called once with body containing all 3 SKUs.
   - `OutOfStockHandlerTest.noWhatsappIfZeroSkipped` — finalize empty manifest, assert WhatsApp NOT called.

#### Acceptance

- Unit tests pass.
- Instrumented test on Tab Plus: manifest with 1 known-OOS SKU + 2 in-stock → 1 skipped, 2 added, 1 WhatsApp message fired with the skipped SKU listed.

#### G4 NOT TESTED list
- Post-tap toast detection in real HyperPure app (requires a reliably OOS SKU — may be flaky in test — 438-09 drill handles)

#### Commit message
```
feat(438-06): OutOfStockHandler — multi-fallback detection + coalesced alert

≥4 OOS markers consulted (text variants + disabled-button check). Post-tap toast
detection as fallback. Per-manifest coalesced WhatsApp alert at manifest close.
Zero-skip manifests produce no alert (no noise).

Covers: HYPER-05
Not tested: real HyperPure OOS variance (438-09).
```

---

### 438-07-PLAN — Checkout flow + confirmation + loopback

**Goal:** After CartPopulator returns, navigate to cart, tap checkout, tap confirm, wait for confirmation screen, capture confirmation number + delivery window, parse, POST to `/api/v1/inventory/order-placed`.

**Covers:** HYPER-02 (checkout), HYPER-03 (loopback)

**Dependencies:** 438-04 (Core endpoint exists), 438-05 (cart populated), 438-06 (OOS finalize), Phase 434 (PersistentSession), Phase 435 (humanize), Phase 435 (audit log — screenshot hashes)

**Type:** `auto`

#### Tasks

1. Create `CheckoutFlow.kt`:
   ```kotlin
   class CheckoutFlow(
       private val ui: UiController,
       private val selectorMap: SelectorMap,
       private val humanize: HumanizeInterceptor,
       private val auditLog: AuditLog,
       private val httpClient: OkHttpClient,
       private val config: HyperPureConfig,
       private val oosHandler: OutOfStockHandler
   ) {
       suspend fun runCheckout(manifest: BulkOrderManifest, populationResult: CartPopulationResult): OrderPlacedReceipt {
           if (populationResult.added.isEmpty()) {
               val receipt = buildFailedReceipt(manifest, status = OrderStatus.SKIPPED_ALL, skipped = populationResult.skipped, raw = "no_items_added")
               postToCore(receipt); oosHandler.finalizeManifest(manifest.manifest_id); return receipt
           }
           humanize.beforeAction(ActionType.NAVIGATION)
           navigateToCart()
           humanize.beforeAction(ActionType.TAP)
           ui.tap(selectorMap.get("cart", "checkout_btn"))
           waitForScreen("checkout", timeoutMs = 10_000)
           humanize.beforeAction(ActionType.TAP)
           ui.tap(selectorMap.get("checkout", "confirm_order_btn"))
           val screenReached = waitForScreen("confirmation", timeoutMs = 60_000)
           if (!screenReached) {
               val receipt = buildFailedReceipt(manifest, status = OrderStatus.FAILED, skipped = populationResult.skipped, raw = "confirmation_screen_timeout")
               postToCore(receipt); oosHandler.finalizeManifest(manifest.manifest_id); return receipt
           }
           val confNum = readText(selectorMap.get("confirmation", "confirmation_number_label"))
           val windowRaw = readText(selectorMap.get("confirmation", "delivery_window_label"))
           val windowParsed = parseDeliveryWindow(windowRaw)
           val rawReceiptText = captureFullScreenText()  // for auditability
           val status = when {
               confNum == null || windowParsed == null -> OrderStatus.PLACED_UNVERIFIED
               else -> OrderStatus.PLACED
           }
           val receipt = OrderPlacedReceipt(
               manifest_id = manifest.manifest_id,
               status = status,
               confirmation_number = confNum,
               delivery_window_start_iso = windowParsed?.startUtc,
               delivery_window_end_iso = windowParsed?.endUtc,
               skipped_skus = populationResult.skipped,
               placed_at_ms = System.currentTimeMillis(),
               raw_receipt_text = rawReceiptText,
               driver_version = BuildConfig.VERSION_NAME,
               device_id = config.deviceId
           )
           postToCore(receipt)
           oosHandler.finalizeManifest(manifest.manifest_id)
           auditLog.emit(OrderPlacedEvent(manifest.manifest_id, status, confNum))
           return receipt
       }

       private suspend fun postToCore(receipt: OrderPlacedReceipt) {
           val url = "${config.coreBaseUrl}/api/v1/inventory/order-placed"
           val body = Json.encodeToString(receipt).toRequestBody("application/json".toMediaType())
           val req = Request.Builder().url(url).header("X-Service-Key", config.inventoryDispatchServiceKey).post(body).build()
           // Retry with exponential backoff up to 5 attempts — network flakiness must NOT lose an OrderPlacedReceipt
           retry(attempts = 5, backoffBase = 2_000) { httpClient.newCall(req).execute().use { r -> if (!r.isSuccessful) error("http ${r.code}") } }
       }
   }
   ```

2. **Delivery-window parser** (handles both relative and absolute per R-6):
   - Relative: "Tomorrow, 8-10 AM" → parse "tomorrow" against device-local date (IST) → build `2026-04-19T08:00:00+05:30` → convert to UTC.
   - Absolute: "19 Apr 2026, 8-10 AM" → direct parse.
   - Regex bank with at least 6 patterns; unit-tested heavily. On total parse failure: return null, flow records status=placed_unverified.

3. **Retry policy for /order-placed POST:** 5 attempts, exponential backoff starting 2s. If all fail, persist the receipt to device-local SQLite (`pending_receipts` table) and retry every 1 min in a background coroutine until success. CLAUDE.md rule: "after fixing a production issue, Core MUST receive the receipt — silent loss is unacceptable."

4. Tests:
   - `DeliveryWindowParserTest` — ≥10 real HyperPure delivery-window strings (James captures samples in 438-01 or 438-09), each parsed correctly OR explicitly marked parse-failed.
   - `CheckoutFlowTest.happyPath` — mock UI + Core, assert status=placed posted.
   - `CheckoutFlowTest.confirmationTimeoutReturnsFailed` — mock `waitForScreen` timeout, assert status=failed posted.
   - `CheckoutFlowTest.missingConfNumReturnsUnverified` — mock confNum=null, assert status=placed_unverified posted.
   - `CheckoutFlowTest.coreUnreachableRetries` — mock HTTP 503, assert 5 retries + persistence to pending_receipts.
   - `CheckoutFlowTest.emptyCartSkipsCheckout` — populationResult.added empty, assert skipped_all status + no HyperPure UI taps.

5. **Cart-persistence safeguard (R-1 mitigation):** Before starting checkout, call `PersistentSession.isValid()`. If invalid, do NOT proceed — the cart may have state from a previous session; abandon and alert. If valid but the cart total ≠ sum(added SKU prices), abandon and alert (detects partial-cart corruption).

#### Acceptance

- Unit tests pass (including ≥10 delivery-window parser cases).
- Instrumented test on Tab Plus: full manifest → checkout → capture → POST observed in Core logs.

#### G4 NOT TESTED list
- Real HyperPure delivery-window text variations in production (438-09 drill captures real samples; known-unknown risk R-6)
- Payment method selection (assumed: HyperPure remembers last-used method for the PersistentSession user; if it prompts, the confirm_order_btn flow above will NOT reach confirmation screen — treated as timeout → placed_unverified → staff alert → manual intervention)

#### Commit message
```
feat(438-07): CheckoutFlow — navigate, confirm, capture, POST

Cart → checkout → confirm → confirmation-screen capture (60s timeout).
Delivery-window parser handles relative + absolute forms (≥10 patterns).
POST /api/v1/inventory/order-placed with 5-attempt retry + disk-backed queue
for network flakiness. Cart-persistence safeguard (R-1) validates session
+ cart-total before checkout. Never silently loses a receipt.

Covers: HYPER-02 (checkout), HYPER-03 (loopback)
Not tested: real HyperPure delivery-window variance (438-09).
```

---

### 438-08-PLAN — Rate limit + business-hours gate + kill-switch verification

**Goal:** Enforce max-3-orders-per-day and business-hours window (08:00–23:00 IST default) BEFORE the HyperPureDriver opens the HyperPure app. Verify Phase 436 FLAG-03 / FLAG-04 correctly halts HyperPure when `enable_hyperpure_*=false` or `pause_all_drivers=true`.

**Covers:** HYPER-04, HUMANIZE-02 reuse verification, FLAG-03/04 reuse verification

**Dependencies:** 438-02, 438-03, Phase 435 (humanize — business-hours gate), Phase 436 (feature flags)

**Type:** `auto`

#### Tasks

1. Create `OrderRateLimiter.kt`:
   ```kotlin
   class OrderRateLimiter(
       private val db: SqliteDb,                  // local SQLite on device
       private val featureFlags: FeatureFlags,    // Phase 436
       private val auditLog: AuditLog,
       private val clock: Clock
   ) {
       suspend fun admit(manifest: BulkOrderManifest): AdmitResult {
           val maxPerDay = featureFlags.getInt("hyperpure.max_orders_per_day", default = 3)
           val todayStart = clock.todayStartUtcMs()  // 00:00 IST -> UTC ms
           val placedToday = db.countPlacedSince("hyperpure_orders", todayStart)
           if (placedToday >= maxPerDay) {
               auditLog.emit(ManifestRateLimitedEvent(manifest.manifest_id, placedToday, maxPerDay))
               reportToCore(manifest, OrderStatus.RATE_LIMITED)
               return AdmitResult.Rejected("rate_limited")
           }
           val window = featureFlags.getBusinessHoursWindow("hyperpure")
           if (!window.contains(clock.nowIst())) {
               queueForWindow(manifest, window)
               auditLog.emit(ManifestQueuedEvent(manifest.manifest_id, window.nextOpenIst()))
               reportToCore(manifest, OrderStatus.OUT_OF_HOURS_QUEUED)
               return AdmitResult.Queued(window.nextOpenIst())
           }
           return AdmitResult.Accepted
       }
   }
   ```

2. **Today boundary:** IST 00:00, stored as UTC ms. Rolling window, not calendar-week or anchor-hour.

3. **Queued manifests:** Persist to SQLite `queued_manifests` table. On business-hours-open (observer on `clock.nowIst()` crossing `window.start`), re-admit queued manifests one at a time (respecting rate limit — if 3 queued from yesterday, only 3 admitted today, excess re-queued to tomorrow).

4. **Kill-switch integration:**
   - `enable_hyperpure_on_<device_id>=false` → Phase 432 calls `HyperPureDriver.uninstall()` → `BulkOrderConsumer.stop()` → any in-flight manifest aborts at the next awaitable boundary (tap/wait/http). Partial-cart state recorded as `aborted_by_kill_switch`.
   - `pause_all_drivers=true` → Phase 436 FLAG-04 dispatcher calls `uninstall()` on all drivers including HyperPure. Same abort path.

5. Tests:
   - `OrderRateLimiterTest.acceptsBelowLimit` — 2 placed today, admit 3rd → Accepted.
   - `OrderRateLimiterTest.rejectsAtLimit` — 3 placed today, admit 4th → Rejected + reportToCore(RATE_LIMITED).
   - `OrderRateLimiterTest.queuesOutsideHours` — 03:00 IST admit → Queued + reportToCore(OUT_OF_HOURS_QUEUED).
   - `OrderRateLimiterTest.dequeueAt0800` — queued at 03:00, fast-forward clock to 08:00, assert dequeue fires.
   - `OrderRateLimiterTest.customFlagOverridesDefault` — feature_flag `hyperpure.max_orders_per_day=5`, 4 placed today, admit 5th → Accepted.
   - `OrderRateLimiterTest.killSwitchAbortsInFlight` — fire `enable_hyperpure_*=false` during processing, assert `uninstall()` called + in-flight manifest aborts with `aborted_by_kill_switch`.
   - `OrderRateLimiterTest.pauseAllDriversAborts` — same, via FLAG-04.

6. **Acceptance-scale check:** Run one real-fleet dry-run — mock clock at 22:55 IST, admit a manifest, run it; then fast-forward to 23:05, admit second manifest, assert Queued; fast-forward to 08:05 next day, assert dequeue.

#### Acceptance

- Unit tests pass.
- Instrumented test: mock clock at 03:00 IST, dispatch manifest from Core → agent records Queued status in Core → mock clock to 08:05 IST → agent dequeues + processes.
- Instrumented test: 3 manifests back-to-back at 10:00 IST → 3 placed. 4th → rate_limited in Core DB, no HyperPure app interaction.

#### G4 NOT TESTED list
- Multi-day rollover behavior (requires multi-day soak — covered by post-ship monitoring)
- Feature-flag change race during manifest in-flight (covered by kill-switch test but not exhaustively)

#### Commit message
```
feat(438-08): OrderRateLimiter — max/day + business-hours + kill-switch

SQLite-backed daily counter (IST 00:00 boundary). Business-hours gate queues
out-of-window manifests with auto-dequeue at window open. Feature flag
hyperpure.max_orders_per_day overrides default 3. Kill-switch abort path
records aborted_by_kill_switch in audit + Core. Rate-limited + queued
manifests reported to Core (no silent drops).

Covers: HYPER-04
Not tested: multi-day soak (post-ship).
```

---

### 438-09-PLAN — E2E integration drill (Tab Plus, 5 SKUs, 1 OOS, full round trip)

**Goal:** Full end-to-end drill validating all four Phase 10 success criteria in one uninterrupted run. This is the ship gate.

**Covers:** all of HYPER-01..05 (verification, not net-new implementation)

**Dependencies:** 438-01 through 438-08, Uday sign-off (see pre-execution checkpoint)

**Type:** `checkpoint:human-verify` (physical Tab Plus + real HyperPure test account + live Core + comms-link)

#### Pre-execution checkpoint (BLOCKING — do not start drill without both)

1. **Uday sign-off** — Uday confirms verbally or in INBOX.md that this drill may run on the HyperPure test account. Without this, ANY order placed is a billing/legal risk.
2. **HyperPure test account provisioned** (see OQ-2) — the app on Tab Plus is logged in as the test account (PersistentSession valid).

#### Preconditions

- Tab Plus reachable on LAN, battery-unrestricted.
- `enable_hyperpure_on_rcm-tab-plus` initially `false`; flip to `true` at the start of the drill.
- Server on .23:8080 up with 438-04 code + migration applied.
- Cloud VPS racecontrol up with same code + migration (parity).
- racecontrol.toml [hyperpure] `auto_reorder_skus = ["test-sku-1","test-sku-2","test-sku-3","test-sku-4","test-sku-5"]` with one known-OOS item in the list.
- Manual fixture: `stub-manifest.json` with 5 SKUs (1 OOS) — for dry-run without waiting on real depletion.

#### Drill script

1. **Flip feature flag** `enable_hyperpure_on_rcm-tab-plus=true` via admin dashboard. Verify within 10s: audit log shows `driver_installed`. **SC-1 part 1 ✓**
2. **Dry-run manifest dispatch:** Issue test endpoint `POST /api/v1/inventory/test-dispatch-manifest` (gated by staff JWT, ADMIN role) with `stub-manifest.json` body. Start timer.
3. Watch agent `/logs/tail`: within 10s, expect `bulk_order_received` with manifest_id.
4. **Within ~5 minutes** (depending on humanize delays + HyperPure load), observe:
   - `sku_added` event for 4 in-stock SKUs
   - `sku_skipped` event for 1 OOS SKU (reason includes the OOS marker)
   - `order_placed` event with status=placed + confirmation_number + delivery window
   - WhatsApp message to Uday listing the 1 skipped SKU
   - Core `hyperpure_orders` row has status=placed, confirmation_number populated, delivery_window_* parsed.
   **SC-1 ✓ SC-2 ✓ SC-3 ✓**
5. **Rate limit verification:** Trigger 2 more dispatches back-to-back (total placed today = 3). Then trigger a 4th. Observe:
   - 4th manifest admitted by consumer, but OrderRateLimiter rejects.
   - Core receives `{status: "rate_limited"}` within seconds.
   - `hyperpure_orders` row status='rate_limited', no HyperPure app interaction.
   **SC-4 part 1 ✓**
6. **Business-hours verification:** Set device clock to 03:00 IST (via ADB `date` or `settings put system time_12_24`). Dispatch a manifest. Observe:
   - Core receives `{status: "out_of_hours_queued"}`.
   - Set clock to 08:05 IST. Within 2 min, queued manifest dequeues and processes (if rate limit allows — may be rate-limited from step 5, which is fine, assert accordingly).
   **SC-4 part 2 ✓**
7. **Kill-switch verification:** During a manifest in-flight, flip `enable_hyperpure_on_rcm-tab-plus=false`. Assert:
   - Within 10s, `driver_uninstalled` event fires.
   - In-flight manifest aborts at next boundary; Core receives `{status: "aborted_by_kill_switch"}`.
   **FLAG-03 reuse verified.**
8. **Grab evidence:**
   - `adb pull /sdcard/Android/data/in.racingpoint.rcagentmobile/files/logs ./drill-438/`.
   - Screenshot of admin reception view (Phase 13 — if already exists) showing all order rows; otherwise SQL dump: `sqlite3 racecontrol.db 'SELECT * FROM hyperpure_orders WHERE date(dispatched_at)=date("now")'`.
   - WhatsApp thread screenshot showing skipped-SKU alert.
   - Screenshots of HyperPure app's order-history screen showing the test orders placed.

#### Acceptance (all four must pass)

- [ ] SC-1: BulkOrderManifest executed end-to-end in HyperPure app (4 SKUs added + 1 skipped, confirmation received)
- [ ] SC-2: OOS SKU skipped + logged in audit + WhatsApp alert to staff
- [ ] SC-3: Confirmation number + delivery window captured + POSTed to Core + visible in DB
- [ ] SC-4: Rate limit (4th rejected) + business-hours gate (03:00 queued, 08:00 dequeued) + kill-switch (in-flight abort) all verified

#### Artifacts to save in SUMMARY.md

- `drill-438/rc-agent-mobile.log.jsonl` (last 500 lines covering the full drill)
- Stopwatch measurements per step
- Screenshots listed above
- Cost estimate (if real test SKUs placed; may be ₹0 if HyperPure test account uses credit)
- Parser-failure samples if delivery-window parsing had misses (feed into regex bank enrichment post-ship)

#### Checkpoint (human-verify)

James runs the drill, reports pass/fail for each SC with numeric measurements. If any SC fails, create a gap-closure plan (438-0N or a new 438-10-gap plan) — do NOT mark Phase 438 complete.

#### Commit message
```
test(438-09): E2E drill — 5-SKU manifest (1 OOS), rate limit, hours, kill-switch

Full Phase 10 success criteria exercised on Tab Plus with real HyperPure test
account. 4 added / 1 skipped + WhatsApp alert + confirmation-loopback + all
gates (rate + hours + kill-switch) verified.

Covers: full Phase 438 acceptance gate.
Artifacts: drill-438/ logs + screenshots + SQL dump in SUMMARY.md.
```

---

## 6. Risks and pitfalls

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | Cart persistence across session drops — agent crashes mid-checkout may re-submit on next trigger | 438-07 cart-persistence safeguard: validate session + cart total before checkout; BulkOrderConsumer serialization (R-5) prevents concurrent cart manipulation. Post-ship: monitor for duplicate orders in first 7 days. |
| R-2 | OOS false negatives (HyperPure has multiple markers varying by context) | 438-01 captures ≥4 fallback selectors; 438-06 checks all markers + disabled-button state + post-tap toast. Any miss → staff alert (no silent "added when actually OOS"). |
| R-3 | ToS risk MEDIUM — HyperPure may flag automated usage | Humanize delays on every action (Phase 435); max-3/day structurally low; business-hours-only window; kill-switch ready (FLAG-04) for emergency ToS incident response (Phase 16). |
| R-4 | Confirmation capture brittleness — screen varies by store/promo banner | 438-01 captures multiple confirmation_number selectors; 438-07 falls back to full-screen text capture (for audit); on selector miss, status=placed_unverified + staff alert — no silent PLACED claim without evidence. |
| R-5 | Cart-population race if two manifests processed concurrently | BulkOrderConsumer is single-threaded with bounded channel (438-03). Second manifest queues until first completes. Verified by `BulkOrderConsumerTest.serializedProcessing`. |
| R-6 | Delivery-window text format variance | 438-07 parser handles ≥6 patterns (relative + absolute), both IST and UTC forms. On parse failure → status=placed_unverified (not placed) + staff alert. Real samples captured in 438-09 enrich regex bank for post-ship updates. |
| R-7 | Core endpoint auth — agent has no staff JWT | X-Service-Key pattern (reuses Phase 427 rc-agent-service-key approach per CLAUDE.md MI Seeder Gap 4). Key rotates with standard secret-rotation cadence. Agent fetches key via Phase 431 first-run UX. |
| R-8 | Depletion event storm — 5 items breach threshold simultaneously | 438-04 debounces 5 min (configurable), max-wait 15 min. One manifest per window regardless of how many items deplete. Verified by `inventory_dispatch_tests`. |
| R-9 | Selector drift when HyperPure app updates | Phase 432 `onAppUpdate` lifecycle hook + Phase 433 version matching + Phase 434 selector-miss event. James gets alerted before driver fails silently; Phase 15 admin remote-push UI allows fleet-wide selector fix without rebuild. |
| R-10 | Silent-success anti-pattern — HyperPure animates success but order fails upstream | 438-07 requires BOTH confirmation_number AND delivery_window to assert status=placed. Missing either → placed_unverified + staff alert. This is the direct application of CLAUDE.md "verify the EXACT behavior, not proxies" rule. |
| R-11 | SKU mapping drift — cafe_items.id vs HyperPure SKU | OQ-4. Mitigation: `hyperpure_sku` column on `cafe_items` OR mapping table `cafe_item_hyperpure_sku (cafe_item_id, hyperpure_sku, last_verified_at)`. Staff maintains. Mismatch = skip SKU + `mapping_missing` reason. |
| R-12 | Integration-checker false-positives on the Kotlin↔Rust protocol drift | MMA audit on `BulkOrderManifest`/`OrderPlacedReceipt` field parity between Kotlin + Rust BEFORE 438-09. Dual reasoning modes required (abstract + trace-level per CLAUDE.md v27.0). |
| R-13 | Network flakiness during POST /order-placed loses the receipt | 438-07 5-retry exponential backoff + disk-backed `pending_receipts` queue. Never silently lose a receipt. |
| R-14 | Uday or staff cancels an order manually in HyperPure app after agent placed it | DB status `cancelled_manual` column. Phase 13 admin reception view should expose a "mark cancelled" button to reconcile. Not in Phase 438 scope; note for Phase 13. |

## 7. Test plan

### Unit tests (JVM, fast, on every build)
- `HyperPureDriverTest` (438-02) — 4 cases
- `BulkOrderConsumerTest` (438-03) — 6 cases
- `CartPopulatorTest` (438-05) — 2 JVM + 5 instrumented
- `OutOfStockHandlerTest` (438-06) — 6 cases
- `CheckoutFlowTest` (438-07) — 5 cases
- `DeliveryWindowParserTest` (438-07) — ≥10 cases (real samples)
- `OrderRateLimiterTest` (438-08) — 7 cases
- Rust: `inventory_dispatch_tests` (438-04) — 2 cases
- Rust: `api/inventory_tests` (438-04) — 4 cases

All unit tests run as part of `./gradlew :app:testDebugUnitTest` + `cargo test -p racecontrol-crate`. Gradle + cargo return non-zero on any failure.

### Instrumented tests (Tab Plus, before release)
- `CartPopulatorInstrumentedTest` (438-05) — 5 cases with real HyperPure
- `CheckoutFlowInstrumentedTest` (438-07) — 2 cases (happy path, timeout)
- `OrderRateLimiterInstrumentedTest` (438-08) — 2 cases (hours + kill-switch)

### Integration test (server + agent)
- 438-09 drill covers this — includes Core-side dispatch via comms-link.

### Physical device tests (human-verify)
- 438-01 checkpoint: James captures selector map on Tab Plus.
- 438-09 drill: full E2E with real HyperPure test account.

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** Rate limiter + business-hours gate + OOS skip path + delivery-window parser are business logic. Run `gsd-nyquist-auditor` on 438-06..08 deliverables before 438-09.
- **MMA audit (required — cross-system bridge):** Rust Core (inventory_dispatch) ↔ comms-link ↔ Kotlin agent (BulkOrderConsumer) ↔ HyperPure app ↔ Kotlin agent (CheckoutFlow) ↔ Rust Core (/order-placed). 6-hop cross-language round-trip with one ToS-sensitive third-party app. Dual reasoning modes MANDATORY (abstract for architecture; trace-level for serialization + state transitions). Budget: $5–7 (larger surface than Phase 429).
- **integration-checker (required):** Before 438-09 and before milestone ship. Must verify:
  - `HyperPureProtocol.kt` vs `rc-common/src/protocol.rs` field parity (CLAUDE.md cross-boundary serialization rule).
  - `/api/v1/inventory/order-placed` contract test (OpenAPI doc + contract test updated).
  - `hyperpure_orders` table present on both server + cloud (DB parity).
- **codebase-mapper (required):** New Rust modules (`inventory_dispatch`, `api/inventory`) + new Kotlin driver module. Refresh `.planning/codebase/` after 438-04 and 438-05 land.
- **ui-researcher / ui-auditor:** Skip. No frontend in this phase. (Phase 13 admin reception view + Phase 14 feature flag UI are separate.)
- **SEC gate:** `node comms-link/test/security-check.js` + Rust `route_uniqueness_tests::no_duplicate_route_registrations` + verify `/order-placed` is NOT in `public_routes` (CLAUDE.md "pod HTTP endpoints default to protected" rule — same posture applies to inventory-dispatch endpoint).
- **Deploy Manifest Protocol (DMP):** Captured in frontmatter `deploy:` section. Executor ticks each item; verifier confirms deployed state matches.
- **Backlog gate (CGP v4.3):** Phase 438 must reach DEPLOYED-VERIFIED (438-09 drill passed + server + cloud + Tab Plus all running the code) before Phase 439 (Blinkit) begins. COMMITTED ≠ SHIPPED.
- **Pre-execution checkpoint (mandatory):** Uday sign-off + HyperPure test account provisioned before Plan 438-09 runs. Documented in 438-09 Drill preconditions.

## 9. Open questions the planner cannot decide

Listed in execution-blocking order.

**OQ-1 — HyperPure Android package id (BLOCKS 438-01).**
The scope assumption is `com.hyperpure`. Actual package id must be verified on the Tab Plus via `adb shell pm list packages | grep -i hyperpure` as the FIRST step of Plan 438-01. If the real package id differs (e.g., `com.hyperpure.b2b`, `com.zomato.hyperpure`), every selector path, `manifest.json` `package`, and `HyperPureDriver.packageName` string must be updated. **Recommendation:** do not start 438-01 until James has installed HyperPure from the Play Store on Tab Plus and captured the real package id. If Play Store region-locks HyperPure, fall back to APK sideload + document the source.

**OQ-2 — HyperPure test account / sandbox (BLOCKS 438-09; affects 438-01 capture quality).**
Does HyperPure offer a dev/sandbox environment where test orders (₹0 or test SKUs) can be placed without real fulfilment? If not, Plan 438-01 selector capture will require placing a real order (cost: ≤ ₹500 for a small test). Plan 438-09 E2E drill places 3 real test orders (total cost: ≤ ₹1500). **Recommendation:** budget ₹2000 for Phase 438 HyperPure test orders; document in SUMMARY.md. Alternatively, ask HyperPure sales for a dev account — but do not block on this; proceed with paid test orders if needed.

**OQ-3 — HyperPure deep-link support.**
Does HyperPure support `hyperpure://product/<sku>` deep-link URIs? If yes, 438-05 uses Tier 1 direct-navigation (robust). If no, 438-05 uses Tier 2 search+disambiguation (brittle — Levenshtein fallback mitigates). **Recommendation:** test via `adb shell am start -a android.intent.action.VIEW -d "hyperpure://product/test-sku"` on Tab Plus during Plan 438-01. Document result in `app-drivers/hyperpure/README.md`. If unsupported, ensure 438-01 captures search screen selectors rigorously.

**OQ-4 — SKU mapping: cafe_items.id vs HyperPure SKU.**
`cafe_items` has an internal id (UUID). HyperPure has its own SKU strings. Mapping strategy options:
- (a) Add `hyperpure_sku TEXT` column to `cafe_items` table (simplest, staff maintains per-item).
- (b) Separate mapping table `cafe_item_hyperpure_sku (cafe_item_id, hyperpure_sku, last_verified_at)` — supports multi-supplier future.
- (c) Store mapping in `racecontrol.toml [hyperpure].sku_map = {"cafe-uuid-1": "HP-SKU-001", ...}` — config-only, no DB migration.
**Recommendation:** (b) — supports v50.0 future (Blinkit also needs mapping). Adds one new table to the 438-04 migration. Staff UI to manage mappings is out of Phase 438 scope; for v50.0, direct SQL via admin terminal is acceptable.

**OQ-5 — HyperPure ₹0 test SKU availability.**
Does HyperPure have a free test SKU (common: "Free sample" items)? If yes, 438-01 and 438-09 can capture confirmation screens + complete real orders at ₹0. If no, see OQ-2 budget. **Recommendation:** James checks on Tab Plus during 438-01 setup; if unavailable, commit to paid test orders per OQ-2.

**OQ-6 — Per-manifest vs per-order debounce.**
Current design: 5-min debounce aggregates all depletions into ONE manifest. Edge case: item A depletes at T=0, item B at T=4min55sec — both in same manifest (correct). Item C depletes at T=5min10sec — goes in NEXT manifest (potentially 5-10 min later). This can result in two orders < 20 min apart. Does this violate humanize expectations? **Recommendation:** accept this as the default (5min debounce is already forgiving); if post-ship monitoring shows too-frequent back-to-back orders, widen debounce window via config. No code change needed.

**OQ-7 — Admin reception view (Phase 13) schema readiness for HyperPure.**
Phase 13 admin reception view is the admin-facing UI for viewing HyperPure orders. Is Phase 13 planned/in-progress? If yes, does its WS event schema already accommodate `OrderPlacedEvent`? If not, 438-04 still broadcasts the event, but the admin UI in Phase 13 will need to subscribe. **Recommendation:** no blocker. 438-04 broadcasts the event per design; Phase 13 consumes when it ships. Document the event shape in 438-04 SUMMARY.md for Phase 13 reference.

**OQ-8 — Cancellation reconciliation.**
If Uday cancels a HyperPure order manually in the HyperPure app (outside our flow), does Core learn about it? Not in Phase 438 scope. **Recommendation:** defer to Phase 13 admin reception view which can have a "mark cancelled" button. Flag this as a known gap in Phase 438 SUMMARY.md.

**OQ-9 — Service-key rotation.**
`[inventory_dispatch] service_key` in racecontrol.toml is the auth secret for `/api/v1/inventory/order-placed`. Rotation strategy? **Recommendation:** manual rotation on-demand (same cadence as Phase 427 rc-agent-service-key). Automated rotation out of scope.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md` Phase 10)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` HYPER-01..05
- **Phase 437 Zomato driver:** `.planning/phases/437-zomato-partner-driver/PLAN.md` — establishes AppDriver / PersistentSession / audit / flag infrastructure that Phase 438 reuses wholesale
- **Phase 429 Kotlin scaffold:** `.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md` — structural template for Android phases
- **Reference Rust depletion origin:** `crates/racecontrol/src/cafe_stock.rs`, `crates/racecontrol/src/cafe_alerts.rs`
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **ToS posture:** PROJECT.md v50.0 section — "MEDIUM risk for HyperPure"

## 11. Output (at phase close)

At the end of Plan 438-09 (E2E drill pass), create `.planning/phases/438-hyperpure-driver/SUMMARY.md` capturing:
- Which commits implemented each plan (438-01 through 438-09)
- HyperPure app version captured in selector map (for Phase 432 onAppUpdate detection)
- OQ-1..9 resolutions (real HyperPure package id, deep-link support, test account details, SKU mapping table schema, etc.)
- Actual stopwatch measurements for each success criterion in 438-09
- Real delivery-window text samples encountered (feed into regex bank)
- Screenshots: admin reception view (or SQL dump), HyperPure app order history, WhatsApp alert thread
- Cost: total ₹ spent on test orders (for ops budget tracking)
- Any risks (R-1..R-14) that materialized + how they were resolved
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked
- Handoff to Phase 439 (Blinkit driver) — what's ready, what's deferred (e.g., cancellation reconciliation left for Phase 13)

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 10 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
