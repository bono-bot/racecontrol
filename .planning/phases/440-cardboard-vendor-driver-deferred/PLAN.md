---
phase: 440-cardboard-vendor-driver-deferred
phase_number: 440
milestone: v50.0 rc-agent-mobile
name: "Cardboard Vendor Driver (P4) — Deferred-Conditional Pluggability Proof"
status: deferred-conditional               # NOT ready-to-execute. This phase is intentionally inert.
autonomous: false                          # Activation requires Uday sign-off; E2E + live-test require James physical presence.
activation_gate: "Q2 resolved AND Uday approves"
goal: >
  PRIMARY purpose of this phase is NOT to ship a cardboard-vendor driver. It is to
  PROVE the driver-framework pluggability contract (Phase 432) by reserving a named
  slot for a future driver whose vendor app is currently unknown (open question Q2
  from v50.0 planning — "which app does the cardboard / cup / disposables vendor
  actually use?"). If Q2 is resolved before milestone ship, this plan becomes the
  drop-in template for the <vendor> driver implementation — every atomic plan is
  parameterized by a `<vendor>` placeholder and can be filled in mechanically. If
  Q2 remains unresolved at milestone close, this phase auto-skips the ship gate
  and falls back to the HelloDriver sample (Phase 432, plan 432-09) as the
  pluggability-demonstration artifact. Either way, adding the cardboard driver
  when ready requires ZERO core-agent code changes — this phase's existence
  enforces that constraint by shape.
requirements: [CARDBOARD-01, CARDBOARD-02]
depends_on:
  - 437                                    # driver-as-production-plugin pattern (ToS gates, PersistentSession, audit log reuse)
  - 432                                    # AppDriver framework + HelloDriver sample (fallback pluggability proof)
  - Q2_RESOLVED                            # non-phase dependency — Q2 = vendor app identified (see activation_checklist)
wave: conditional                          # Only enters the wave graph if activated. Otherwise skipped at milestone close.
plan_count: 6                              # Template plans — parameterised by <vendor>. Not executed until activation.
plans:
  - 440-01-PLAN: "<vendor> selector-map authoring (TEMPLATE — James, Phase 433 debug capture)"
  - 440-02-PLAN: "<Vendor>Driver AppDriver impl (TEMPLATE — scaffold + manifest entry)"
  - 440-03-PLAN: "Trigger integration (TEMPLATE — admin dashboard button or Core inventory alert)"
  - 440-04-PLAN: "Navigate + order flow (TEMPLATE — search/browse/add-to-cart/checkout)"
  - 440-05-PLAN: "Confirmation capture + log-back-to-Core (TEMPLATE — order number + ETA or delivery window)"
  - 440-06-PLAN: "Integration test (TEMPLATE — physical device E2E drill)"

files_modified:
  # ALL paths parameterised by <vendor>. None of these files are created until activation.
  - "rc-agent-mobile/app-drivers/<vendor>/"                                                 # new driver module directory
  - "rc-agent-mobile/app-drivers/<vendor>/manifest.json"                                    # driver metadata
  - "rc-agent-mobile/app-drivers/<vendor>/v<vendor_version>/selectors.yaml"                 # authored by James in 440-01
  - "rc-agent-mobile/app-drivers/<vendor>/README.md"                                        # authoring notes
  - "rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/<vendor>/<Vendor>Driver.kt"
  - "rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/<vendor>/<Vendor>TriggerConsumer.kt"
  - "rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/<vendor>/<Vendor>CheckoutFlow.kt"
  - "rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/drivers/<vendor>/<Vendor>ConfirmationParser.kt"
  - "rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/drivers/<vendor>/"    # unit tests
  - "rc-agent-mobile/drivers.json"                                                          # add <vendor> entry
  # Core side — only if trigger integration needs a new route (see 440-03 Option B)
  - "crates/racecontrol/src/api/reception/<vendor>.rs"                                      # optional — only if new route
  - "crates/racecontrol/src/db/migrations/NNN_<vendor>_orders.sql"                          # optional — only if new orders table
  - ".planning/phases/440-cardboard-vendor-driver-deferred/SUMMARY.md"                      # filled at phase close OR at auto-skip

# DMP — Deploy Manifest Protocol (MANDATORY, parameterised)
deploy:
  rust_binary: [conditional]                # racecontrol ONLY if 440-03 Option B (new /reception/<vendor>/ route)
  frontend_rebuild: [conditional]           # admin ONLY if 440-03 Option A (new admin dashboard trigger button)
  config_change: >
    rc-agent-mobile drivers.json — add <vendor> entry (required on activation).
    rc-agent-mobile humanize.toml — add [<vendor>] section (per-app delay + rate limit, required on activation).
    comms-link james/index.js — register new message type `<vendor>_trigger` (required on activation).
    racecontrol.toml [<vendor>] section — enabled=false (opt-in), max_orders_per_day=<TBD>,
    business_hours_start="08:00", business_hours_end="23:00", target_device_id=<TBD — tab_plus
    or m07 based on CAPREG capacity at activation time>, staff_alert_phone=<Uday's number>.
  db_migration: >
    CONDITIONAL. If <vendor> uses the same schema pattern as HyperPure (Phase 438) —
    orders table with (id, triggered_at, order_number, eta_text, eta_parsed, status,
    device_id, raw_receipt_text, driver_version) — create `<vendor>_orders` table.
    If <vendor> reuses the existing hyperpure_orders or blinkit_orders shape, add a
    `source` column instead of a new table. Decided in 440-04 at activation time.
  infrastructure: >
    <vendor> Android app installed on the chosen device (Tab Plus or M07).
    James logged in with a PersistentSession valid for the test account.
    Vendor account provisioned (delivery address = venue saved as default).
    Payment method saved (UPI / card / COD per vendor support).
    Device firewall allows inbound from comms-link relay (port 8090).
  data_files: "rc-agent-mobile/app-drivers/<vendor>/v<detected_version>/selectors.yaml — captured in 440-01."
  bat_file: none
  cloud_parity:
    - "racecontrol binary (venue .23) + cloud racecontrol (Bono VPS) — DEPLOY PARITY rule, IF a new /reception/<vendor>/ route is added."
    - "admin dashboard (venue .23:3201) + cloud admin dashboard — IF a new dashboard trigger button is added (440-03 Option A)."
    - "comms-link relay James .27 + comms-link relay Bono VPS — `<vendor>_trigger` message type both sides."
    - "<vendor>_orders migration on venue DB AND cloud DB (Phase 301 cloud_data_sync_v2 replicates going forward) — only if a new table is added."
    - "NO cloud_parity for the Android APK — single physical device, venue-only."
  targets:
    - "tab_plus OR m07 (decided at activation per CAPREG-01 capacity)"
    - "server_23 (conditional — only if new Core route)"
    - "bono_vps (conditional — only if new Core route, per DEPLOY PARITY)"
    - "admin_frontend (conditional — only if new admin button)"
  rollback:
    - "Feature flag `enable_<vendor>_on_<device_id>=false` halts driver within 10s (Phase 436 FLAG-03)."
    - "Global `pause_all_drivers=true` halts fleet-wide within 10s (FLAG-04 kill-switch)."
    - "Driver is plugin — uninstall via flag toggle leaves core agent untouched."
    - "Previous APK retained at /sdcard/Download/rc-agent-mobile-prev.apk; `adb install -r <prev>` rolls back driver code."
    - "If <vendor> app places a wrong order: cancel via vendor app manually (Uday), mark row status='cancelled_manual', disable feature flag, investigate via audit log + selector-miss events."

# Subagent gates (per CLAUDE.md > Subagent Gates) — MINIMAL unless activated
gates:
  ui_researcher: "conditional-on-activation"       # If 440-03 Option A (new admin button), UI-SPEC.md REQUIRED per CLAUDE.md frontend rule. Otherwise skip.
  ui_auditor: "conditional-on-activation"          # Same — only if admin UI change ships.
  nyquist_auditor: "conditional-on-activation"     # Required at activation: ConfirmationParser + RateLimiter are business logic.
  mma_audit: "conditional-on-activation"           # Required at activation: cross-system bridge (Kotlin -> Node -> Rust -> third-party Android -> Rust). Dual reasoning modes REQUIRED per CLAUDE.md v27.0.
  integration_checker: "conditional-on-activation" # Required at activation — integrates with DriverRegistry (432), humanize (435), feature flags (436), audit log (435).
  codebase_mapper: "conditional-on-activation"     # Not required at activation since no new top-level module (drivers live under rc-agent-mobile/app-drivers/).
  pre_execution_checkpoint: >
    BEFORE any atomic plan executes: (1) Q2 resolved (vendor app name + package ID known),
    (2) Uday signs off on activation (financial exposure + ToS posture accepted),
    (3) vendor test account provisioned, (4) selector-map authored via Phase 433 debug mode,
    (5) Core SKU -> vendor catalogue mapping defined. This is a BLOCKING checkpoint —
    do not start 440-01 until ALL five items are present.

risks_summary:
  - "R-1 Q2 NEVER RESOLVES (acceptable, expected path) — if at milestone close the vendor app is still unknown, this phase auto-skips and the framework pluggability claim is discharged via HelloDriver sample (Phase 432, plan 432-09). Detailed auto-skip procedure in §5 below."
  - "R-2 Weak selectors in vendor app (unknown until capture) — many small-vendor Android apps have poor accessibility labels, custom canvas rendering, or resource-id churn between versions. Mitigation at activation: require James's 440-01 debug capture to produce >=3 fallback strategies per element; if fewer than 3 are available for any critical element, 440-01 FAILS and activation is blocked until vendor ships a better UI or an alternative vendor is chosen. No workaround — we do NOT ship a driver on fragile selectors (ToS risk + billing loss risk)."
  - "R-3 Vendor ToS unknown — some vendor apps explicitly forbid automation. Mitigation at activation: Uday verifies ToS during activation checklist item (3); if automation is forbidden, phase remains deferred indefinitely and falls back to manual order placement with audit-log reminder."
  - "R-4 SKU mapping incomplete (open question cascading from Q2) — if the Core inventory has 'cardboard-cup-120ml' but the vendor catalogue uses 'Plastic Dispo Cup 120 ml (Pack of 50)', the driver cannot match. Mitigation: 440-01 produces a `sku_catalogue_mapping.yaml` mapping each Core cafe_items.id to vendor SKU + pack-size — authored by James alongside selectors."
  - "R-5 Activation race: concurrent driver deploys (this + HyperPure retuning + Blinkit) could contend for the same Tab Plus. Mitigation: at activation, CAPREG-01 query resolves device assignment BEFORE 440-02 runs; driver is rejected at manifest load if device capacity is exhausted."
  - "R-6 Silent-success anti-pattern (same as HyperPure R-10) — vendor may show a fake 'order placed' animation even when backend failed. Mitigation: ConfirmationParser MUST capture BOTH order number AND delivery window/ETA before status='placed'; missing either -> status='placed_unverified' + staff alert. No exceptions."
  - "R-7 Payment friction — small vendors may require UPI PIN entry per order (cannot be bypassed via PersistentSession). If UPI PIN is required, driver architecture forbids entering PINs via Accessibility (ToS red line — same rule as Blinkit R-6). Mitigation: at activation, prefer vendor accounts with auto-pay / saved card + 2FA-less checkout. If only UPI-PIN is available, phase remains deferred."
  - "R-8 Vendor app updates break selectors — same as all drivers (Phase 438 R-9 / Phase 439 R-1). Phase 432 onAppUpdate hook + Phase 433 selector-version-matching handle most cases; Phase 434 selector-miss events surface drift early."
  - "R-9 Test account cost — unlike HyperPure (B2B, invoice-based), this vendor may require live card charges to test. Uday must pre-approve a test budget (default: Rs. 500 cap for activation E2E)."
  - "R-10 The framework-proof ALREADY EXISTS via HelloDriver (Phase 432, plan 432-09). If R-1 through R-9 make activation uneconomical, Uday and James should CLOSE this phase with a SUMMARY documenting why activation was declined. Proving pluggability a SECOND time (with a real vendor) is nice-to-have, not required — CARDBOARD-02 requirement is structurally satisfied by the HelloDriver sample."

open_questions:
  - id: Q2
    from: ".planning/PROJECT.md > Open Questions (2026-04-18)"
    question: "What Android app does the cardboard / cup / disposables vendor actually use? Do they even have an app? Is ordering done over WhatsApp instead?"
    owner: Uday
    blocks: ["phase 440 activation (entire phase)"]
    resolution_options:
      - "Vendor has a branded Android app (e.g., com.<vendor>.partner) -> proceed with full activation, fill all <vendor> placeholders."
      - "Vendor uses an aggregator app (IndiaMart, Udaan, Amazon Business) -> driver targets the aggregator package; SKU mapping covers vendor's listings on that aggregator."
      - "Vendor uses WhatsApp / phone / paper orders -> NO Android driver possible; phase REMAINS DEFERRED; fallback to manual + Core inventory depletion WhatsApp reminder to staff. CARDBOARD-02 satisfied via HelloDriver."
      - "Vendor has a web portal only (no app) -> out of scope for v50.0 (mobile agent); defer to v5X future browser-automation milestone."
  - id: Q2.1
    cascades_from: Q2
    question: "If Q2 resolves to 'aggregator app' — do we have an account on that aggregator? Does it support bulk ordering at our scale (tens of Rs. per order, not lakhs)?"
    owner: Uday
  - id: Q2.2
    cascades_from: Q2
    question: "Does the vendor offer a test account or does activation require a production account? If production, what is the test-budget cap (see R-9)?"
    owner: Uday
  - id: Q2.3
    cascades_from: Q2
    question: "Does the vendor support COD / pay-on-delivery? If not, payment friction may block automation (see R-7)."
    owner: Uday
  - id: Q2.4
    cascades_from: Q2
    question: "What is the trigger model — Core inventory depletion (like HyperPure, infrequent + bulk) or staff manual (like Blinkit, ad-hoc + small)? Default assumption: staff manual, since cardboard depletion is less frequent and less time-sensitive than food."
    owner: James
  - id: Q2.5
    cascades_from: Q2
    question: "Is Tab Plus or M07 the target device? Default recommendation at activation: whichever has free capacity per CAPREG-01 query."
    owner: James
---

# Phase 440 — Cardboard Vendor Driver (P4) — Deferred-Conditional Pluggability Proof

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 440 |
| Name | Cardboard Vendor Driver (P4) — Deferred-Conditional Pluggability Proof |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | CARDBOARD-01, CARDBOARD-02 |
| Dependencies | Phase 437 (driver-as-production-plugin pattern), Phase 432 (AppDriver framework + HelloDriver fallback sample), **Q2 resolved** (vendor app identified) |
| Wave | conditional — not in the wave graph until activated |
| Status | **deferred-conditional** — NOT ready to execute. Intentionally inert. |
| Autonomous | No — activation requires Uday sign-off; all execution plans include human-verify checkpoints (physical device + real account) |
| Ship test (deferred path) | HelloDriver (Phase 432, plan 432-09) demonstrably loads + runs + shuts down on the agent with zero core-agent code changes. CARDBOARD-02 discharged via this sample. |
| Ship test (activated path) | (1) Trigger (staff or Core inventory) fires -> <vendor> app opens, items added, checkout completes, confirmation number + delivery window captured + logged back to Core. (2) Rate limit + business hours enforced. (3) Feature flag kill-switch halts driver within 10s. (4) Zero changes required in `AgentForegroundService.kt`, `DriverRegistry.kt`, `LifecycleDispatcher.kt`, or any core module — proves DRIVER-02, DRIVER-03, CARDBOARD-02. |

## 2. Activation checklist — what unlocks this phase

This phase DOES NOT ENTER THE EXECUTION GRAPH until **every item below is checked**. The checklist is the gate. Any single missing item = phase stays deferred.

- [ ] **Q2 resolved** — vendor app name + package ID known (or confirmed absence of app). See `open_questions.Q2` in frontmatter for resolution branches.
- [ ] **Uday approves activation** — in writing (INBOX.md entry or WhatsApp screenshot committed to repo). Specifically: Uday accepts (a) financial exposure (R-9 test budget), (b) ToS posture after reading vendor terms (R-3), (c) activation timing relative to other v50.0 work.
- [ ] **Vendor test account provisioned** — username/phone + saved payment method + venue delivery address set as default. Staff account is acceptable; personal-account usage is NOT.
- [ ] **Selector map authored** — James has run Phase 433's debug-capture mode against the vendor app on the chosen device and produced `rc-agent-mobile/app-drivers/<vendor>/v<version>/selectors.yaml` with >=3 fallback strategies per critical element (see R-2).
- [ ] **Core SKU -> vendor catalogue mapping** — James has produced `rc-agent-mobile/app-drivers/<vendor>/sku_catalogue_mapping.yaml` mapping each cafe_items.id that this driver might order to the vendor's SKU + pack size. Missing SKUs = blocker until the mapping is complete for every orderable item.
- [ ] **Device capacity confirmed** — CAPREG-01 query shows target device (Tab Plus or M07) has capacity for this driver (not overloaded with HyperPure + Blinkit + Zomato).
- [ ] **Target trigger decided** — 440-03 Option A (admin dashboard button, staff-manual trigger like Blinkit) OR Option B (Core inventory-depletion alert, auto trigger like HyperPure). See Q2.4.

When all boxes are checked, edit the frontmatter: `status: ready-to-execute`, `autonomous: false`, remove `conditional` from `wave` (assign a real wave number in the v50.0 graph — likely wave 4 since 437/438/439 occupy wave 3). Replace every `<vendor>` token in this PLAN.md with the actual vendor identifier. Then execute 440-01 through 440-06.

## 3. Goal-backward must-haves

### Truths (deferred path — the path we expect)

- **T-Deferred-1:** At milestone close, the v50.0 ship gate evaluates phase 440 as **auto-skipped** with verification artifact = Phase 432 plan 432-09 HelloDriver SUMMARY.md. Evidence format (see §5): one paragraph + link to HelloDriver SUMMARY + confirmation that DRIVER-02, DRIVER-03, CARDBOARD-02 are satisfied by HelloDriver.
- **T-Deferred-2:** `.planning/ROADMAP-v50.md` phase 12 checkbox is marked `- [skipped]` (not `- [x]` and not `- [ ]`) with a comment: `# auto-skipped per CARDBOARD-02 — Q2 unresolved at milestone close, pluggability proven via HelloDriver (Phase 432)`.
- **T-Deferred-3:** This PLAN.md remains in the repo — adding a real driver later = someone pulls this template, answers the activation checklist, fills in the `<vendor>` placeholders, and executes. The plan's existence is the enforcement mechanism for DRIVER-03 ("adding a driver = new module + manifest entry, zero core changes").

### Truths (activated path — the conditional path)

- **T-Activated-1 (CARDBOARD-01):** <Vendor>Driver exists under `rc-agent-mobile/app-drivers/<vendor>/` with an `AppDriver` implementation, a `manifest.json` entry, and a `drivers.json` registration entry. Zero lines of code in `AgentForegroundService.kt`, `DriverRegistry.kt`, `LifecycleDispatcher.kt`, or any core module are changed (observable: `git diff --stat <pre-440>..<post-440>` shows zero modified files under `rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/`, `driver/` root, or `http/`).
- **T-Activated-2 (CARDBOARD-01):** On the chosen device (Tab Plus or M07), toggling `enable_<vendor>_on_<device>=true` fires `<Vendor>Driver.install()` within 10s per FLAG-03; toggling back to false fires `uninstall()` within 10s.
- **T-Activated-3 (CARDBOARD-02):** Framework pluggability claim verified by this phase's SUMMARY.md. Evidence: before/after file list proving no core file touched + `<Vendor>Driver` functional on the real app.
- **T-Activated-4 (Trigger):** Staff trigger (admin dashboard) OR Core inventory depletion fires -> within humanize-delay window, the <vendor> app opens, items are added to cart, checkout completes.
- **T-Activated-5 (Confirmation):** Order number + delivery window / ETA captured from the vendor's confirmation screen and POSTed to Core via `/api/v1/reception/<vendor>/order-placed` (or equivalent). Missing either field -> `status='placed_unverified'` + staff alert.
- **T-Activated-6 (ToS):** Humanize delays applied at every UI action (N(mean,stddev) drawn); business-hours gate enforced; max orders/day enforced; kill-switch halts within 10s.

### Required artifacts (deferred path)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `.planning/phases/432-driver-framework-capability-registry/432-09-PLAN.md` | HelloDriver plan (ALREADY EXISTS as fallback proof) | n/a | Existing |
| `.planning/phases/440-cardboard-vendor-driver-deferred/SUMMARY.md` | Deferred-closure summary, written at milestone close | 40 | "Phase 440 auto-skipped per CARDBOARD-02 — Q2 unresolved at {date}. Framework pluggability discharged via HelloDriver (link to 432-09 SUMMARY). Activation checklist preserved for future." |
| `.planning/ROADMAP-v50.md` (phase 12 row) | Marked `- [skipped]` | 1 line | See T-Deferred-2 |

### Required artifacts (activated path)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/app-drivers/<vendor>/manifest.json` | Driver metadata | 20 | `{driver_id: "<vendor>", package: "<vendor package>", supported_device_types: ["tablet"|"phone"], credential_strategy: "PersistentSession", business_hours_gated: true, rate_limited: true, max_orders_per_day_default: <TBD from Q2.1>}` |
| `rc-agent-mobile/app-drivers/<vendor>/v<version>/selectors.yaml` | Selector map | 80 | Screens: home, search, sku_detail, cart, checkout, confirmation. Each critical element >=3 fallback strategies (R-2). |
| `rc-agent-mobile/app-drivers/<vendor>/sku_catalogue_mapping.yaml` | Core -> vendor SKU map | 20 | For each cafe_items.id this driver might order: `vendor_sku`, `pack_size`, `pack_unit`, `notes` |
| `rc-agent-mobile/app-drivers/<vendor>/README.md` | Authoring notes | 40 | HyperPure-style — which app version captured, what wasn't capturable and why, how to re-capture |
| `.../drivers/<vendor>/<Vendor>Driver.kt` | AppDriver impl | 120 | `install()`, `onAppUpdate()`, `healthCheck()`, `uninstall()`, `onTriggerReceived()`, delegates to TriggerConsumer + CheckoutFlow + ConfirmationParser |
| `.../drivers/<vendor>/<Vendor>TriggerConsumer.kt` | Listens for `<vendor>_trigger` from comms-link | 60 | Single-threaded coroutine scope, bounded channel, serializes trigger processing |
| `.../drivers/<vendor>/<Vendor>CheckoutFlow.kt` | Navigate + cart + checkout | 100 | Per-screen flow using selector map + humanize delays |
| `.../drivers/<vendor>/<Vendor>ConfirmationParser.kt` | Parse order number + ETA/delivery window | 60 | Returns `{order_number, eta_or_window, status}`; on parse failure returns `placed_unverified` with `raw_receipt_text` |
| `rc-agent-mobile/drivers.json` (amendment) | <vendor> driver registration | 1 entry | `{driver_id, class, enabled_by_default: false, required_feature_flag}` |
| `.planning/phases/440-cardboard-vendor-driver-deferred/SUMMARY.md` | Activated-closure summary | 60 | Before/after file list, HelloDriver sample still present, activation checklist satisfied, E2E drill evidence |

### Key links (wiring — activated path only)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| Trigger source (admin button OR Core inventory alert) | comms-link relay | WS message `type: "<vendor>_trigger"` | grep `<vendor>_trigger` in `comms-link/james/index.js` |
| comms-link relay | `<Vendor>TriggerConsumer.onMessage` | WS forward | grep `<vendor>_trigger` in both Kotlin + Node (SERDE TAG PARITY per CLAUDE.md) |
| `<Vendor>TriggerConsumer` | `OrderRateLimiter.admit(trigger)` | Kotlin call | grep `OrderRateLimiter.admit` in TriggerConsumer |
| `OrderRateLimiter.admit` | `<Vendor>Driver.onTriggerReceived` | Kotlin call (after admit) | grep `onTriggerReceived` in rate limiter |
| `<Vendor>Driver.onTriggerReceived` | `<Vendor>CheckoutFlow.run(trigger)` | Kotlin call | grep `CheckoutFlow.run` in Driver |
| `<Vendor>CheckoutFlow.captureConfirmation` | `POST /api/v1/reception/<vendor>/order-placed` | OkHttp POST | grep `/api/v1/reception/<vendor>` in CheckoutFlow |
| `POST /order-placed` handler | `<vendor>_orders` UPDATE | sqlx query | grep `UPDATE <vendor>_orders` in api/reception/<vendor>.rs |
| feature flag `enable_<vendor>_on_<device>=false` | `<Vendor>Driver.uninstall()` | Phase 436 dispatcher | verify via FLAG-03 test from Phase 436 |
| feature flag `pause_all_drivers=true` | ALL drivers halt | Phase 436 kill-switch | MUST be covered by Phase 436 FLAG-04 test (not re-implemented) |

## 4. Context — files to read before executing any plan (on activation)

@./CLAUDE.md
@./comms-link/CLAUDE.md
@./comms-link/docs/PROTOCOL.md
@./.planning/REQUIREMENTS-v50.md                              # CARDBOARD-01, CARDBOARD-02
@./.planning/ROADMAP-v50.md                                   # Phase 12 entry (this phase's source-of-truth row)
@./.planning/PROJECT.md                                       # v50.0 section: open question Q2
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md  # AppDriver contract + HelloDriver sample (fallback proof)
@./.planning/phases/433-selector-dsl-hot-reload/PLAN.md       # debug capture mode used in 440-01
@./.planning/phases/435-humanize-layer-audit-log/PLAN.md      # humanize + audit reused
@./.planning/phases/436-feature-flag-system/PLAN.md           # flag + kill-switch reused
@./.planning/phases/437-zomato-partner-driver/PLAN.md         # driver-as-production-plugin reference
@./.planning/phases/438-hyperpure-driver/PLAN.md              # closest shape (bulk/inventory-trigger — Option B)
@./.planning/phases/439-blinkit-driver/PLAN.md                # alternate shape (staff-manual trigger — Option A)
@./crates/rc-common/src/protocol.rs                           # types to extend with <Vendor>Trigger + OrderPlacedReceipt

## 5. Ship-gate auto-skip logic (THE CORE OF THIS PHASE)

This is the procedure executed at v50.0 milestone close if the activation checklist is NOT satisfied.

### Preconditions for auto-skip

- Milestone close has been called (all other phases shipped or explicitly deferred).
- The activation checklist at the top of this PLAN.md is NOT fully checked — specifically, Q2 remains unresolved OR Uday has not approved.

### Auto-skip procedure (manual — executed by James or Bono at milestone close)

1. **Verify HelloDriver is functional on the running agent.** On both Tab Plus and M07:
   - `curl http://<device_lan_ip>:8090/capability` returns a JSON array that includes `{"driver_id": "hello", ...}` (or whatever HelloDriver's id is per Phase 432 plan 432-02).
   - `adb shell cat /sdcard/Android/data/in.racingpoint.rcagentmobile/files/logs/rc-agent-mobile.log.jsonl | grep '"driver":"hello"'` shows at least one install + healthCheck + uninstall log line from a prior run.
   - If HelloDriver is not functional, DO NOT auto-skip — first repair HelloDriver (that is a Phase 432 regression, not a Phase 440 task), then retry this step.
2. **Confirm no vendor app is identified.** Re-read `.planning/PROJECT.md` Open Questions section. If Q2 still lists the vendor-app resolution branches unresolved, proceed. If Q2 was quietly answered in session notes but not propagated here, STOP and propagate the answer first — then re-evaluate the activation checklist.
3. **Write the deferred-closure SUMMARY.md.** Create `.planning/phases/440-cardboard-vendor-driver-deferred/SUMMARY.md` with the content:

   ```markdown
   # Phase 440 — Cardboard Vendor Driver — AUTO-SKIPPED

   **Milestone close date:** <YYYY-MM-DD>
   **Disposition:** auto-skipped per CARDBOARD-02

   ## Why skipped

   Q2 ("what Android app does the cardboard / cup / disposables vendor use?") was
   not resolved at milestone close. Per the phase's activation gate
   (`Q2 resolved AND Uday approves`), execution did not begin.

   ## How CARDBOARD-02 is discharged

   CARDBOARD-02 requires: "Phase auto-skips milestone-close gate if Q2 remains
   unresolved at ship time (driver framework is pluggable — this is a drop-in
   when ready)."

   Framework pluggability is demonstrably proven by:
   - Phase 432 plan 432-09 (HelloDriver sample). See that SUMMARY for evidence.
   - This PLAN.md itself, which is a drop-in template parameterized by
     `<vendor>`. When Q2 resolves, the activation checklist at the top of
     440-PLAN.md unlocks execution — zero core-agent code changes required.

   ## What remains

   - Phase 440 PLAN.md is PRESERVED in the repo as the activation template.
   - Activation checklist (items 1-7) captures exactly what must happen to
     de-defer this phase.
   - No code paths reference Phase 440 artifacts (`app-drivers/<vendor>/` is
     never loaded because `drivers.json` does not list it). Zero runtime risk.

   ## References

   - Phase 432 plan 432-09 HelloDriver SUMMARY.md (fallback pluggability proof)
   - .planning/REQUIREMENTS-v50.md CARDBOARD-02
   - .planning/PROJECT.md Open Questions Q2
   ```

4. **Update ROADMAP-v50.md Phase 12 row.** Mark the checkbox `- [skipped]` with a footnote line under the table (or inline as a parenthetical) referencing this SUMMARY.md. Do NOT mark `- [x]` — that would falsely claim the driver shipped.
5. **Close CARDBOARD-01 and CARDBOARD-02 in the traceability table** (REQUIREMENTS-v50.md section "Traceability"). Status for both becomes `Skipped-DeferredConditional` with phase = 440 and a link to this SUMMARY.
6. **Commit with message:** `docs(440): auto-skip phase 440 per CARDBOARD-02, pluggability discharged via HelloDriver (Phase 432)`.
7. **Notify Bono** via comms-link INBOX.md (auto-push rule) that Phase 440 is skipped-not-shipped and that if Q2 resolves later, this PLAN.md is the re-entry point.

### Why this is acceptable (not technical debt)

- The framework pluggability claim (DRIVER-02, DRIVER-03, CARDBOARD-02) is structurally satisfied by HelloDriver (Phase 432, plan 432-09). A second driver is nice-to-have, not required.
- The activation checklist at the top of this PLAN.md IS the enforcement that "adding a driver requires no core changes" — by making the template exhaustive, we've ensured that when Q2 resolves, the team cannot accidentally introduce core-agent changes (the diff would be unreviewable against a PLAN.md that explicitly forbids it).
- Deferring a driver for an unidentified vendor is better than shipping a broken driver against a guessed vendor app. CLAUDE.md rule: "Smallest reversible fix first" — doing nothing is the smallest reversible action when the input is unknown.

## 6. Template atomic plans (PARAMETERISED by <vendor>)

Each plan below is a DROP-IN TEMPLATE. At activation, every `<vendor>` / `<Vendor>` / `<vendor_version>` / `<vendor_package>` token is replaced with the actual vendor identifier. Structure mirrors Phase 438 (HyperPure) for bulk-order flows and Phase 439 (Blinkit) for staff-trigger flows — the activation checklist item 7 picks which shape to use (Option A = Blinkit-like, Option B = HyperPure-like).

---

### 440-01-PLAN (TEMPLATE) — <vendor> selector-map authoring

**Goal:** James uses Phase 433 debug-capture mode on the target device (Tab Plus or M07) to produce `rc-agent-mobile/app-drivers/<vendor>/v<vendor_version>/selectors.yaml`, committed before 440-02 begins.

**Covers:** CARDBOARD-01 partial (selectors are the substrate)

**Dependencies:** Activation checklist complete, Phase 433 debug mode available, vendor app installed + logged in

**Type:** `checkpoint:human-verify`

**Parameterization:**
- Replace `<vendor>` with actual vendor identifier (e.g., `cardboard_shop`)
- Replace `<Vendor>` with PascalCase version (e.g., `CardboardShop`)
- Replace `<vendor_package>` with Play Store package ID (e.g., `com.example.cardboardshop`)
- Replace `<vendor_version>` with detected app version (e.g., `2.1.4`)

**James's capture sequence:** Same six-screen sequence as HyperPure 438-01 (home, search, sku_detail, cart, checkout, confirmation). Each critical element must have >=3 fallback strategies (R-2 mitigation — non-negotiable; if the app UI does not expose >=3 stable selectors per element, STOP and escalate to Uday per R-2 procedure).

**Output:** `rc-agent-mobile/app-drivers/<vendor>/v<vendor_version>/selectors.yaml` + companion `README.md` + `sku_catalogue_mapping.yaml` (activation checklist item 5).

**Commit:** `feat(440-01): <vendor> v<vendor_version> selector map (6 screens, ~30 elements, >=3 fallback variants)`

**Checkpoint (human-verify):** James confirms all 6 screens captured, >=3 fallback variants on critical elements, SKU mapping complete for every orderable cafe_item, README.md notes what was not capturable.

**G4 NOT TESTED:** Runtime stability verified in 440-06. Selector drift after app update verified on next Phase 432 onAppUpdate fire.

---

### 440-02-PLAN (TEMPLATE) — <Vendor>Driver AppDriver impl + manifest entry

**Goal:** Empty `<Vendor>Driver` class implements `AppDriver` lifecycle with no-op bodies + manifest entry in `drivers.json` enables it under a feature flag (default OFF). Agent loads the driver on boot without crashing. ZERO CORE-AGENT CODE CHANGES — this is the pluggability proof.

**Covers:** CARDBOARD-01 partial, CARDBOARD-02 (framework pluggability verification), DRIVER-01..05 reuse verification

**Dependencies:** 440-01, Phase 432 (driver framework), Phase 436 (feature flags)

**Type:** `auto`

**Scaffold:** Mirror HyperPure 438-02 structure. `<Vendor>Driver.kt` implements `AppDriver` with `install/uninstall/healthCheck/onAppUpdate` + a placeholder `onTriggerReceived` (wired in 440-04/05). `drivers.json` entry: `{driver_id, class, package, supported_device_types, credential_strategy: "PersistentSession", business_hours_gated: true, rate_limited: true, max_orders_per_day_default: <TBD Q2.1>, selectors_path, enabled_by_default: false, required_feature_flag: "enable_<vendor>_on_<device_id>"}`.

**Pluggability verification (CARDBOARD-02 evidence):** Before committing, run `git diff --stat` and confirm zero modified lines in `rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/`, `driver/` root (only subdirs under `drivers/<vendor>/`), `http/`, `comms/core/`, `boot/`. Any hit = pluggability broken, fail the plan, investigate why Phase 432 framework is leaking.

**Unit tests:** `<Vendor>DriverTest` — installRegistersConsumer, uninstallStopsConsumer, healthyWhenSelectorMapLoaded, unhealthyWhenSessionExpired. Same shape as HyperPure 438-02.

**Acceptance:** Tests pass. APK builds. On install, driver logs install event. Toggling feature flag fires install/uninstall within 10s. `git diff --stat` proof of zero core changes committed as evidence file `.planning/phases/440-cardboard-vendor-driver-deferred/CARDBOARD-02-EVIDENCE.txt`.

**Commit:** `feat(440-02): <Vendor>Driver scaffold + drivers.json entry (CARDBOARD-02 pluggability verified, 0 core changes)`

---

### 440-03-PLAN (TEMPLATE) — Trigger integration

**Goal:** Wire the trigger source (admin dashboard button OR Core inventory alert) into a comms-link message that reaches the <Vendor>TriggerConsumer.

**Covers:** CARDBOARD-01 (core of "driver accepts triggers")

**Dependencies:** 440-02, Phase 429 (comms-link client), activation checklist item 7 (Option A vs B decided)

**Type:** `auto` (plus `checkpoint:decision` at the start if Option A/B was ambiguous at activation)

**Option A — Staff-manual trigger (Blinkit shape):** New admin dashboard page `/reception/<vendor>/` with a trigger form (SKU autocomplete + quantity + confirm dialog). Form POSTs to new Core route `POST /api/v1/reception/<vendor>/trigger` which sends `<vendor>_trigger` via comms-link to target device. Requires admin frontend rebuild + new Core route + UI-SPEC.md + UI-REVIEW.md (CLAUDE.md frontend gate).

**Option B — Core inventory-depletion auto-trigger (HyperPure shape):** Amend `cafe_stock.rs` to include <vendor>-managed cafe_items in the depletion dispatcher. `inventory_dispatch::enqueue_depletion` routes to `target_device_id` based on item category (food -> HyperPure target device; cardboard/disposables -> <vendor> target device). Debounce (5 min default). No admin UI change required.

**Decision:** Made at activation per Q2.4 (default: Option A since cardboard depletion is less time-sensitive and staff visibility matters more than latency).

**Unit tests:** `<Vendor>TriggerConsumerTest` verifies message deserialization + bounded-channel serialization (R-5 from HyperPure applies here too).

**Commit:** `feat(440-03): <vendor> trigger integration (Option {A|B})`

---

### 440-04-PLAN (TEMPLATE) — Navigate + order flow

**Goal:** <Vendor>CheckoutFlow implements the per-screen navigation: open app -> navigate to each SKU -> detect OOS -> add to cart -> proceed to checkout.

**Covers:** CARDBOARD-01 (core of "driver navigates app")

**Dependencies:** 440-03, 440-01 selectors

**Type:** `auto`

**Structure:** Mirror HyperPure 438-05 (CartPopulator) + 438-06 (OutOfStockHandler). For each SKU in the trigger's item list: search or direct-nav to SKU, check OOS via selector map (>=4 variants per R-2 inherited from HyperPure), if available tap add-to-cart, verify cart-count increment via accessibility node-info query, emit audit event per SKU. Humanize delays at every action (Phase 435 interceptor — already applied at the AppDriver level, no additional wiring).

**Unit tests:** `<Vendor>CheckoutFlowTest` with mock AccessibilityService — verifies OOS skip path, cart-count verification, abort-on-selector-miss path.

**Commit:** `feat(440-04): <vendor> checkout-flow (navigate + cart + OOS skip)`

---

### 440-05-PLAN (TEMPLATE) — Confirmation capture + log-back-to-Core

**Goal:** After checkout, parse order number + ETA/delivery window from the confirmation screen and POST to Core. Handle parse failures deterministically (`placed_unverified` + staff alert).

**Covers:** CARDBOARD-01 (core of "log back to Core")

**Dependencies:** 440-04

**Type:** `auto`

**Structure:** Mirror HyperPure 438-07 (CheckoutFlow confirmation capture). `<Vendor>ConfirmationParser.kt` reads confirmation number via selector (fallback OCR via Phase 433 screen capture if selector misses), reads delivery window / ETA text, parses to ISO-8601 or minutes-based field. On parse failure: status='placed_unverified' + staff WhatsApp alert + full `raw_receipt_text` preserved. POST to `/api/v1/reception/<vendor>/order-placed` (new route created in 440-03 Option A, or amended in Option B).

**R-6 mitigation (silent-success):** BOTH order number AND (delivery window OR ETA) MUST be captured. Missing either -> `placed_unverified`. No exceptions.

**Unit tests:** `<Vendor>ConfirmationParserTest` — exhaustive text-format test set (relative time, absolute time, range, malformed), includes `raw_text_preserved_on_failure` test.

**Commit:** `feat(440-05): <vendor> confirmation parser + log-back-to-Core`

---

### 440-06-PLAN (TEMPLATE) — Integration test

**Goal:** Physical-device E2E drill. Real vendor account + real trigger -> real order placed -> real confirmation captured -> logged to Core DB + visible in admin reception view.

**Covers:** All of CARDBOARD-01, CARDBOARD-02 activated path

**Dependencies:** 440-02..05, Uday sign-off (activation item 2), test budget (R-9)

**Type:** `checkpoint:human-verify`

**Drill sequence:**
1. James: fire the trigger (admin dashboard click OR simulate Core inventory depletion)
2. Verify: driver opens <vendor> app, items added, checkout completes — observable via physical screen + audit log screenshot hashes
3. Verify: order actually places (vendor sends confirmation email / SMS to account)
4. Verify: Core DB `<vendor>_orders` row has status='placed' with order_number + delivery_window/ETA
5. Verify: admin dashboard reception view shows the order
6. Verify: rate-limit — fire trigger Nth+1 times, confirm rejection with `rate_limited` audit event
7. Verify: business-hours gate — fire trigger outside window, confirm queue behavior
8. Verify: kill-switch — toggle feature flag to false, confirm halt within 10s
9. Verify: ZERO CORE CHANGES — re-run `git diff --stat <pre-440>..<post-440>` and confirm no lines modified under core directories (CARDBOARD-02 evidence)

**Rollback:** If any step fails, disable feature flag + investigate via audit log. Phase 440 remains `activated-but-unshipped`; fixes are new commits under 440-NN-PLAN.

**Commit:** `feat(440-06): <vendor> E2E drill passed — CARDBOARD-01 + CARDBOARD-02 shipped`

---

## 7. Gates (minimal in deferred state)

| Gate | Deferred path | Activated path |
|------|---------------|----------------|
| Live test account | NOT required | REQUIRED before 440-06 |
| MMA audit | NOT required | REQUIRED at 440-05 (cross-system bridge, dual reasoning modes per CLAUDE.md v27.0) |
| Uday sign-off | NOT required (until re-evaluation) | REQUIRED at activation (activation checklist item 2) |
| UI-SPEC + UI-REVIEW | NOT required | REQUIRED if 440-03 Option A (admin dashboard changes) |
| nyquist-audit | NOT required | REQUIRED at 440-05 (confirmation parser is business logic) |
| integration-checker | NOT required | REQUIRED before 440-06 (multi-phase driver integration) |
| codebase-mapper | NOT required | NOT required (no new top-level module; lives under existing rc-agent-mobile/) |

## 8. Risks — phase-specific

See `risks_summary` in frontmatter. Highlights:

- **R-1 Q2 never resolves (acceptable):** the expected path. Framework is proven via HelloDriver. Auto-skip procedure in §5.
- **R-2 Weak selectors (open-ended):** small-vendor apps often have poor accessibility labels. Mitigation = >=3 fallback strategies per critical element, enforced at 440-01. If the app UI does not support this, activation is BLOCKED indefinitely (no workaround — shipping on fragile selectors would cause silent billing / ordering errors).
- **R-3 Vendor ToS unknown:** verified by Uday at activation item 2.
- **R-7 UPI PIN friction:** if vendor requires UPI PIN per order, phase remains deferred (cannot be automated safely).

## 9. Open questions

See `open_questions` in frontmatter. All cascade from Q2 (owned by Uday or James). Summary:

- **Q2 (root):** vendor app identity — Uday.
- **Q2.1:** account availability + scale support — Uday.
- **Q2.2:** test account vs production + test budget cap — Uday.
- **Q2.3:** payment method friction (UPI PIN, COD) — Uday.
- **Q2.4:** trigger model (staff manual vs Core auto) — James (default: staff manual).
- **Q2.5:** target device (Tab Plus vs M07) — James (default: per CAPREG-01 capacity).

Until Q2 resolves, Q2.1-Q2.5 are structurally unanswerable.

## 10. How to pick this phase up later (future-agent notes)

If you are a future Claude / James / Bono session and are asked to activate Phase 440:

1. **Read this PLAN.md first.** Do not re-research the framework from scratch — Phase 432 already proved pluggability and Phase 437/438/439 already established the driver-as-production-plugin pattern.
2. **Walk the activation checklist (§2) top to bottom.** Do not skip items. If any item is unclear, stop and ask Uday.
3. **If all items are checked, update the frontmatter** per §2 final paragraph: `status: ready-to-execute`, remove `conditional` from `wave`, assign a real wave number.
4. **Globally replace `<vendor>`, `<Vendor>`, `<vendor_package>`, `<vendor_version>`** throughout this PLAN.md with the actual vendor values. Use search-and-replace. Commit the replaced PLAN.md as a separate commit before starting 440-01.
5. **Execute 440-01 through 440-06 in order.** Each plan references HyperPure 438-XX or Blinkit 439-XX as the closest shape — consult those SUMMARYs for lived-experience notes.
6. **Enforce the zero-core-changes rule mechanically.** Before committing 440-02, run `git diff --stat` and fail the plan if any core directory is touched. Commit the diff-stat output as CARDBOARD-02 evidence.
7. **If at any point Q2 resolves to "no vendor app" (e.g., vendor uses WhatsApp only),** revert frontmatter to deferred state and run the auto-skip procedure (§5) — this is a valid outcome, not a failure.
8. **Do NOT shortcut the activation checklist** to rush a ship. A broken cardboard driver would cause silent ordering errors (wrong SKU, wrong quantity, wrong delivery address) with direct financial impact. The framework-proof claim (CARDBOARD-02) is already satisfied by HelloDriver — you have NO time pressure.

## 11. Deployment

See `deploy:` block in frontmatter. All fields are `conditional` until activation. At activation, Option A (admin UI) vs Option B (auto-trigger) determines which targets are included. DEPLOY PARITY rule applies (server + cloud) if any server-side code is added. The APK rebuild is venue-only (single physical device).

## 12. SUMMARY.md template hints

Two SUMMARY templates, one per path. See §5 for the deferred-path template. For the activated path:

```markdown
# Phase 440 — Cardboard Vendor Driver — SHIPPED (<vendor>)

**Ship date:** <YYYY-MM-DD>
**Vendor:** <vendor name>
**Device:** <tab_plus | m07>

## CARDBOARD-01 evidence

- <Vendor>Driver plugs in with zero core-agent changes (diff-stat proof: `CARDBOARD-02-EVIDENCE.txt`)
- E2E drill passed on <YYYY-MM-DD> — order #<X> for Rs. <Y> placed, ETA <Z>, confirmed in admin reception view

## CARDBOARD-02 evidence

- Framework pluggability ALSO proven by HelloDriver (Phase 432, plan 432-09) — double coverage
- Adding this driver required 0 lines changed in core directories (see diff-stat)

## Artifacts created

<list all files under rc-agent-mobile/app-drivers/<vendor>/ and any new Rust files>

## Activation checklist (satisfied)

<paste filled checklist from §2>

## Lessons learned

<what surprised us, what the selector capture missed, what the next driver should do differently>
```

---

**END OF PLAN 440 — cardboard vendor driver (deferred-conditional pluggability proof)**
