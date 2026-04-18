---
phase: 434-credential-abstraction
phase_number: 434
milestone: v50.0 rc-agent-mobile
name: "Credential Abstraction — CredentialStrategy + PersistentSession"
status: ready-to-execute
goal: >
  Define a pluggable CredentialStrategy interface with login/isSessionValid/refresh/logout
  contract; ship the PersistentSession implementation in v50.0 (human logs in once,
  agent verifies session via Accessibility-detectable indicators declared per-driver
  in selectors); define OtpFlow and OAuth as future-compat interface slots that throw
  NotImplementedError; each driver declares its credential strategy in its manifest and
  the agent core enforces the declaration at runtime; session-expiry emits a
  SessionExpiredEvent which pauses the driver and notifies staff via the admin
  dashboard (Phase 441) and WhatsApp (via Bono VPS racingpoint-whatsapp-bot).
requirements: [CRED-01, CRED-02, CRED-03, CRED-04]
depends_on: [432-driver-framework-capability-registry]
wave: 3
plan_count: 7
plans:
  - 434-01-PLAN: CredentialStrategy interface + SessionState sealed class + SessionExpiredEvent
  - 434-02-PLAN: PersistentSession implementation (Accessibility session-indicator detection)
  - 434-03-PLAN: OtpFlow future-compat interface stub (NotImplementedError)
  - 434-04-PLAN: OAuth future-compat interface stub (NotImplementedError)
  - 434-05-PLAN: Driver manifest credential-strategy declaration + runtime enforcement
  - 434-06-PLAN: Session-expiry routing (comms-link event -> admin dashboard + WhatsApp)
  - 434-07-PLAN: Unit tests (strategy swap) + integration test (simulated session expiry)
autonomous: true # All plans are code + unit/integration tests. Phase 434 has no human-verify checkpoints (physical login is exercised in Phase 439 Zomato driver E2E, not here).
files_modified:
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/CredentialStrategy.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/SessionState.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/SessionExpiredEvent.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/CredentialStrategyRegistry.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/PersistentSession.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/SessionIndicatorEvaluator.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/OtpFlow.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/credential/OAuth.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverManifest.kt       # amend — add credential_strategy field
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverLoader.kt         # amend — enforce credential strategy at install()
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverLifecycle.kt      # amend — pause driver on SessionExpiredEvent
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/alert/SessionExpiryAlerter.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/CommsLinkClient.kt        # amend — send SessionExpiredEvent upstream
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/protocol/Protocol.kt            # amend — SessionExpiredEventPayload
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/CredentialStrategyTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/PersistentSessionTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/SessionIndicatorEvaluatorTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/OtpFlowStubTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/OAuthStubTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/CredentialStrategyRegistryTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/SessionExpiryAlerterTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/driver/ManifestEnforcementTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/StrategySwapIntegrationTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/credential/SessionExpiryE2ETest.kt
  - rc-agent-mobile/docs/CREDENTIAL-STRATEGIES.md                                                   # new — architectural overview
  - rc-agent-mobile/docs/PROTOCOL.md                                                                # amend — SessionExpiredEvent message type
  - comms-link/shared/session-expiry-event-v1.md                                                    # new — cross-repo reference
  - .planning/phases/434-credential-abstraction/SUMMARY.md                                          # filled at phase close

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [none]
  frontend_rebuild: [none]               # Phase 441 (admin dashboard reception view) is a later phase; Phase 434 only DEFINES the event schema that 441 will consume. No admin rebuild in this phase.
  config_change: >
    rc-agent-mobile driver manifests gain a required "credential_strategy" field.
    drivers.json (bundled in APK from Phase 432) is extended from the empty array
    placeholder — in Phase 434 no real driver exists yet, so the manifest enforcement
    is exercised via a test-only fixture manifest. Production manifests populate
    in Phase 439 (Zomato) etc.
  db_migration: none                     # No DB schema change. SessionExpiredEvent is transient (sent via comms-link, not persisted at phase 434; Phase 435 audit log may persist).
  infrastructure: >
    Bono VPS racingpoint-whatsapp-bot must expose an endpoint for session-expiry
    alerts. Proposed path: POST http://localhost:<whatsapp-bot-port>/alerts/session-expired
    with payload {device_id, driver_id, app, expired_at, staff_message}. Reachable
    via comms-link relay on Bono VPS (100.70.177.44:8765) — Phase 434-06 defines
    the exact wire format. See OQ-1 for routing decision (direct HTTP vs relay exec).
  data_files: >
    rc-agent-mobile/app/src/androidTest/resources/fixtures/driver-manifest-persistent-session.json
    (test fixture — a minimal driver manifest declaring credential_strategy: "persistent_session"
    used by StrategySwapIntegrationTest to prove a new strategy can be plugged in
    without modifying core code).
  bat_file: none
  cloud_parity:
    - Bono VPS racingpoint-whatsapp-bot — endpoint for session-expiry alerts.
    - comms-link cloud relay (100.70.177.44:8765) — already forwards all agent events; Phase 434 adds a new message TYPE only, no new identity.
  targets:
    - tab_plus    # Lenovo TB-351FU (runs PersistentSession once Phase 439 Zomato driver lands)
    - m07         # Samsung Galaxy M07 (same)
    - bono_vps    # racingpoint-whatsapp-bot endpoint addition
    - server_23   # NO deploy impact (Phase 434 does NOT touch server racecontrol binary)
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk  # amended APK; new code under credential/*
  rollback:
    - "Keep previous APK on device: /sdcard/Download/rc-agent-mobile-prev.apk"
    - "If PersistentSession emits false-positive SessionExpiredEvents on a driver installed later, the driver can be feature-flag-disabled via Phase 438 flags without redeploy."
    - "If the WhatsApp alerting endpoint is misconfigured, SessionExpiryAlerter degrades gracefully (logs WARN, does NOT crash the driver) — see Risks R-6."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip           # Phase 434 has no user-facing UI. Admin dashboard surface lives in Phase 441.
  ui_auditor: skip              # Same.
  nyquist_auditor: required     # CredentialStrategy swap logic, session-expiry detection, and event routing are business logic AND silent-data-loss class. Per CLAUDE.md Subagent Gates: "Any phase with business logic (billing, sessions, auth, games)". Credentials = auth.
  mma_audit: required           # Session-expiry bugs are SILENT DATA LOSS class (driver runs without a valid session -> actions fail silently or succeed with wrong account context -> ToS violations / refund disputes). CLAUDE.md MMA rule: "cross-system bridge deploy (MANDATORY)" — this phase spans Kotlin agent + comms-link relay + WhatsApp bot + (future) admin dashboard. Dual reasoning modes REQUIRED per CLAUDE.md — abstract for architecture + trace-level for false-positive logic.
  integration_checker: required # Multi-phase wiring: Phase 432 driver framework + Phase 433 selectors + Phase 434 credentials + Phase 441 dashboard. Run before v50.0 ship.
  codebase_mapper: skip         # rc-agent-mobile/ module already added to map in Phase 429. No new top-level module.
  sec_gate: required            # PersistentSession reads app session cookies/indicators via Accessibility. Node comms-link/test/security-check.js must confirm no credential material is logged, no PSK leaked, no identity impersonation possible.

risks_summary:
  - "False-positive session-indicator detection -> false SessionExpiredEvent -> unnecessary staff WhatsApp spam -> alert fatigue -> real expiries ignored. Mitigation: per-driver tunable confidence threshold + min 3 consecutive failures (~15 min) before firing; staff mute-for-N-hours button in Phase 441."
  - "False-negative session-indicator -> driver believes session valid when it's not -> silent task failure, potential wrong-account action. Mitigation: on any driver-action failure, force an isSessionValid() re-check before retry; verify-before-generate standing rule applied to selector match."
  - "PersistentSession indicator selectors drift when target app updates UI -> detection breaks -> looks like session expired (false positive) OR looks valid when not (false negative). Mitigation: Phase 433 selector DSL with fallback chain + hot-reload; SelectorMissEvent (already defined in 433) emitted when no indicator matches at all.",
  - "WhatsApp bot down -> SessionExpiryAlerter HTTP call fails -> alert lost. Mitigation: retry with 3x exponential backoff, persist to RotatingLog (Phase 429-07) for staff to check manually, also broadcast to admin dashboard (dual channel)."
  - "Strategy swap 'no core code change' promise violated by hidden coupling. Mitigation: StrategySwapIntegrationTest plugs a fake TestStrategy class via manifest only; CI fails if core code is touched to add it."
  - "OtpFlow / OAuth stubs called in production because a driver manifest accidentally declares them -> NotImplementedError crashes driver. Mitigation: DriverLoader refuses to install() a driver whose credential_strategy is not in {persistent_session} in v50.0; emits clear ManifestEnforcementError with actionable message."
  - "Cross-session PII leak if SessionIndicatorEvaluator logs user names/profile text when matching 'logged-in' indicator. Mitigation: logs record selector id and match result (true/false) only, NEVER the matched text."
  - "DriverLifecycle pause semantics — does pause mean (a) stop scheduling new actions, (b) complete in-flight action then stop, (c) abort mid-action? Locked decision: (b) — per R-8 in section 6."
  - "BUG-TRACKER open item (Pattern F false-positive class from session_handoff_20260417_game_launch_poe.md): silent failures can look like session expiry but are actually driver-side bugs. Mitigation: SessionExpiredEvent must include a distinguishing signature (indicator_present: false AND last_heartbeat_age < 60s); pure driver crash is handled by DriverFramework not credential layer."
---

# Phase 434 — Credential Abstraction (CredentialStrategy + PersistentSession)

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 434 |
| Name | Credential Abstraction — CredentialStrategy + PersistentSession |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | CRED-01, CRED-02, CRED-03, CRED-04 |
| Dependencies | Phase 432 (driver framework + manifest loader) |
| Wave | 3 (parallel with Phase 433 selectors and Phase 435 humanize+audit — all depend only on Phase 432) |
| Status | Ready to execute |
| Autonomous | Yes — all plans are code + unit/integration tests. No human-verify checkpoints in this phase. |
| Ship test | (a) Driver manifest declares `credential_strategy: "persistent_session"` and DriverLoader enforces at runtime (CRED-01); (b) PersistentSession flags `isSessionValid() = false` within one healthCheck cycle (<= 5 min) on a test driver whose indicator selectors fail (CRED-02); (c) StrategySwapIntegrationTest plugs a new strategy class + manifest entry with zero changes to DriverLoader, CredentialStrategyRegistry, or any core agent file (CRED-03); (d) session-expiry -> SessionExpiredEvent -> comms-link -> (admin dashboard stub + WhatsApp alerter) -> driver paused within 10s (CRED-04). |

## 2. Success criteria (verbatim from ROADMAP-v50.md Phase 6)

1. **Strategy declaration + enforcement:** Driver declares credential strategy in manifest; agent enforces at runtime.
2. **Expiry detection latency:** `PersistentSession` detects session expiry within one health-check cycle (<= 5 min).
3. **Extensibility:** Adding a new strategy class + manifest entry does NOT require core code change.

ROADMAP Phase 6 lists 3 criteria. CRED-04 adds an implicit 4th (session-expiry alert routing). Both ship in this phase.

4. **Alerting:** SessionExpiredEvent routed to admin dashboard (message schema ready; UI landing in Phase 441) AND WhatsApp (via Bono VPS racingpoint-whatsapp-bot) within 10s of detection. Driver paused on detection.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (observable)

- T-1: A valid driver manifest with `credential_strategy: "persistent_session"` installs and operates; a manifest with `credential_strategy: "otp_flow"` or `"oauth"` in v50.0 is rejected with a clear ManifestEnforcementError at install time (CRED-01 + R-6).
- T-2: A manifest without a `credential_strategy` field at all is rejected with "missing required field" at install time (CRED-01).
- T-3: A driver in persistent_session mode, when its indicator selectors fail to match the session-valid element, produces `SessionState.Expired` from `isSessionValid()` within 5 minutes (one healthCheck cycle per DRIVER-04) of the real expiry (CRED-02).
- T-4: The same driver, when session indicators DO match, produces `SessionState.Valid` (with `last_verified_at` in the response) (CRED-02).
- T-5: Adding a new `TestStrategy : CredentialStrategy` class in `src/androidTest/.../credential/TestStrategy.kt` + bumping the manifest's `credential_strategy` to `"test_strategy"` is sufficient to make DriverLoader load it — no edits to DriverLoader.kt, CredentialStrategyRegistry.kt, DriverLifecycle.kt, or any other core agent file (CRED-03).
- T-6: On `SessionState.Expired`, the agent emits a `SessionExpiredEvent` envelope upstream via CommsLinkClient within 3 seconds (CRED-04).
- T-7: The driver's `DriverLifecycle.state` transitions to `Paused(reason = SessionExpired)` within 10 seconds of SessionExpiredEvent emission (CRED-04).
- T-8: The admin dashboard surface receives the event (for Phase 441 consumption) — verified in Phase 434 by inspecting the comms-link relay log for a `session_expired` message with the expected payload. Admin UI rendering lands in Phase 441.
- T-9: The WhatsApp alerter fires a POST to the Bono VPS racingpoint-whatsapp-bot endpoint within 10 seconds. On HTTP failure, retries 3x with 1s/2s/4s backoff, then logs ERROR and gives up (CRED-04).
- T-10: `OtpFlow` and `OAuth` interface files exist, their methods throw `NotImplementedError` with a descriptive message mentioning "future phase" (CRED-03 forward-compat).
- T-11: Unit tests `CredentialStrategyTest`, `PersistentSessionTest`, `SessionIndicatorEvaluatorTest`, `OtpFlowStubTest`, `OAuthStubTest`, `CredentialStrategyRegistryTest`, `SessionExpiryAlerterTest`, `ManifestEnforcementTest`, `StrategySwapIntegrationTest`, `SessionExpiryE2ETest` all pass on `./gradlew :app:testDebugUnitTest`.

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `.../credential/CredentialStrategy.kt` | Interface contract | 40 | `interface CredentialStrategy { suspend fun login(); suspend fun isSessionValid(): SessionState; suspend fun refresh(): SessionState; suspend fun logout() }` + KDoc with contract semantics |
| `.../credential/SessionState.kt` | Sealed class of session states | 30 | `sealed class SessionState { object NotLoggedIn; data class Valid(lastVerifiedAt: Long); data class Expired(reason: ExpiryReason); data class Unknown(error: Throwable) }` |
| `.../credential/SessionExpiredEvent.kt` | Event payload | 20 | `data class SessionExpiredEvent(deviceId, driverId, app, expiredAt, reason: ExpiryReason, indicatorDetails: Map<String,Boolean>)` |
| `.../credential/CredentialStrategyRegistry.kt` | Registry pattern for strategy lookup | 50 | Maps strategy name ("persistent_session", "otp_flow", "oauth") to a factory function; NEW strategies are registered via ServiceLoader or a companion-object `register()` call so no core file needs edit |
| `.../credential/PersistentSession.kt` | Concrete strategy impl | 120 | Implements CredentialStrategy; login() triggers staff-facing "log in manually now" prompt (notification + admin push); isSessionValid() delegates to SessionIndicatorEvaluator; refresh() re-triggers login(); logout() clears session indicators from driver state |
| `.../credential/SessionIndicatorEvaluator.kt` | Selector-driven indicator check | 80 | Reads per-driver session_indicator selectors from Phase 433 selector registry; queries AccessibilityService (Phase 430) for each; returns `Valid` if at least `min_match_count` indicators match, `Expired` if zero match, `Unknown` if AccessibilityService unavailable |
| `.../credential/OtpFlow.kt` | Future-compat interface stub | 30 | `interface OtpFlow : CredentialStrategy { suspend fun requestOtp(): String; suspend fun submitOtp(code: String): SessionState }` + default throws NotImplementedError |
| `.../credential/OAuth.kt` | Future-compat interface stub | 30 | `interface OAuth : CredentialStrategy { suspend fun startAuthFlow(activity: Activity): Intent; suspend fun handleCallback(intent: Intent): SessionState }` + default throws NotImplementedError |
| `.../driver/DriverManifest.kt` | Amended data class | +5 | Adds `val credentialStrategy: String` field (required) |
| `.../driver/DriverLoader.kt` | Amended enforcement | +40 | At install(): read credentialStrategy, look up in CredentialStrategyRegistry, instantiate; fail loudly if not found; in v50.0 reject "otp_flow" and "oauth" (R-6) |
| `.../driver/DriverLifecycle.kt` | Amended pause path | +30 | On SessionExpiredEvent for this driver, transition state to Paused; expose resume() that re-runs login() |
| `.../alert/SessionExpiryAlerter.kt` | WhatsApp alert sender | 80 | On SessionExpiredEvent: (a) sends via CommsLinkClient for admin dashboard consumption; (b) HTTP POSTs to Bono VPS racingpoint-whatsapp-bot with 3x exp backoff |
| `.../comms/CommsLinkClient.kt` | Amended | +20 | `sendEvent(SessionExpiredEvent)` serializes to PROTOCOL.md envelope and sends over WS |
| `.../protocol/Protocol.kt` | Amended | +30 | Adds `SessionExpiredEventPayload` @Serializable |
| `rc-agent-mobile/docs/CREDENTIAL-STRATEGIES.md` | Architectural doc | 150 | Explains the CredentialStrategy pattern, lifecycle hooks, how to add a new strategy (pointed to by StrategySwapIntegrationTest), expiry semantics |
| `rc-agent-mobile/docs/PROTOCOL.md` | Amended | +40 | Documents `session_expired` message type, payload schema, relay routing expectations |
| `comms-link/shared/session-expiry-event-v1.md` | Cross-repo reference | 60 | Mirror copy for Bono's whatsapp-bot team |
| `.../androidTest/resources/fixtures/driver-manifest-persistent-session.json` | Test fixture | 15 | Minimal valid manifest declaring persistent_session strategy |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| DriverLoader.install(manifest) | CredentialStrategyRegistry.resolve(manifest.credentialStrategy) | Kotlin call | grep `CredentialStrategyRegistry.resolve` in `DriverLoader.kt` |
| Driver.healthCheck() (Phase 432) | strategy.isSessionValid() | Kotlin call | grep `strategy.isSessionValid` in `Driver.kt` (Phase 432 code amended here) |
| PersistentSession.isSessionValid() | SessionIndicatorEvaluator.evaluate() | Kotlin call | grep `SessionIndicatorEvaluator` in `PersistentSession.kt` |
| SessionIndicatorEvaluator | SelectorRegistry (Phase 433) | Kotlin call | grep `SelectorRegistry` in `SessionIndicatorEvaluator.kt` |
| SessionIndicatorEvaluator | AccessibilityService (Phase 430) | Kotlin call | grep `AccessibilityService` or `screenTree` in `SessionIndicatorEvaluator.kt` |
| PersistentSession on Expired | SessionExpiredEvent emitter | event bus | grep `SessionExpiredEvent(` in `PersistentSession.kt` |
| SessionExpiredEvent | CommsLinkClient.sendEvent | Kotlin call | grep `sendEvent(.*SessionExpiredEvent` in `SessionExpiryAlerter.kt` |
| SessionExpiredEvent | DriverLifecycle.pause(reason=SessionExpired) | Kotlin call | grep `DriverLifecycle.pause` in `SessionExpiryAlerter.kt` |
| SessionExpiredEvent | WhatsApp POST | OkHttp | grep `racingpoint-whatsapp-bot` in `SessionExpiryAlerter.kt` |
| CredentialStrategyRegistry.register | (companion object self-register) | Kotlin call | grep `CredentialStrategyRegistry.register` in each strategy file (PersistentSession, OtpFlow, OAuth) — proves no central edit needed |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./comms-link/CLAUDE.md
@./comms-link/docs/PROTOCOL.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md  # v50.0 Planning Milestone — extensibility feature #4: credential abstraction
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md  # structural template + Protocol.kt shape
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md  # Driver + DriverManifest + DriverLoader + DriverLifecycle contracts (dependency)
@./.planning/phases/433-selector-dsl-hot-reload/PLAN.md  # SelectorRegistry + selector YAML schema (consumer of indicators)
@./.planning/phases/430-accessibility-service-foundation/PLAN.md  # AccessibilityService.screenTree() API (PersistentSession consumes it)

### Interfaces executors will need

Extracted contract surface from the dependency phases. Executor should use these directly — no codebase exploration needed.

**From Phase 432 (DriverManifest.kt):**

```kotlin
// BEFORE Phase 434:
@Serializable
data class DriverManifest(
  val id: String,
  val name: String,
  val version: String,
  val supportedDeviceTypes: List<String>,
  val appPackage: String,
  // ... other Phase 432 fields
)

// AFTER Phase 434 (amended in plan 434-05):
@Serializable
data class DriverManifest(
  val id: String,
  val name: String,
  val version: String,
  val supportedDeviceTypes: List<String>,
  val appPackage: String,
  val credentialStrategy: String,           // NEW — required. One of: persistent_session | otp_flow | oauth
  val sessionIndicators: List<String>? = null,  // NEW — optional, refs selectors in Phase 433 YAML by name. Only read for persistent_session.
)
```

**From Phase 432 (Driver.kt lifecycle):**

```kotlin
interface Driver {
  suspend fun install()         // Called when driver is enabled (feature flag on). Phase 434 enforces credential_strategy here.
  suspend fun onAppUpdate()     // Called when target app package version changes
  suspend fun healthCheck()     // Called every 5 min by DriverScheduler. Phase 434 adds isSessionValid() check inside this.
  suspend fun uninstall()       // Called when driver disabled
}
```

**From Phase 432 (DriverLifecycle.kt):**

```kotlin
sealed class DriverLifecycleState {
  object Installed        // Installed and idle
  object Running          // Processing events
  data class Paused(val reason: PauseReason)   // Phase 434 adds PauseReason.SessionExpired
  object Uninstalled
}

sealed class PauseReason {
  object FeatureFlagOff
  object OutsideBusinessHours
  object SessionExpired   // NEW in Phase 434
  // ... other reasons
}
```

**From Phase 433 (SelectorRegistry.kt):**

```kotlin
interface SelectorRegistry {
  /** Resolve a named selector (e.g., "zomato.logged_in_avatar") to a concrete selector chain. */
  fun resolve(driverId: String, name: String): SelectorChain?
  fun hotReload()
}
```

**From Phase 430 (AccessibilityService.kt):**

```kotlin
interface AccessibilityBridge {
  /** Returns the current foreground app's AccessibilityNodeInfo tree. */
  suspend fun screenTree(appPackage: String): AccessibilityNode?
  /** Query a single selector against current tree. Returns matched node or null. */
  suspend fun findNode(selector: SelectorChain): AccessibilityNode?
}
```

### New interfaces THIS phase creates (consumed by Phase 435+, 439+)

```kotlin
// Public API of Phase 434, finalized in plan 434-01:

interface CredentialStrategy {
  /** Non-interactive when possible. PersistentSession: triggers staff manual-login prompt (first run or admin-dash action). */
  suspend fun login(): SessionState

  /** Cheap health check. PersistentSession: evaluates session-indicator selectors. MUST return within 5s. */
  suspend fun isSessionValid(): SessionState

  /** Attempt to renew without full login. PersistentSession: same as login() for v50.0; OAuth: uses refresh token. */
  suspend fun refresh(): SessionState

  /** Tear down session state. */
  suspend fun logout()
}

sealed class SessionState {
  object NotLoggedIn : SessionState()
  data class Valid(val lastVerifiedAt: Long, val indicatorsMatched: List<String>) : SessionState()
  data class Expired(val reason: ExpiryReason, val indicatorsChecked: Map<String, Boolean>) : SessionState()
  data class Unknown(val error: Throwable) : SessionState()
}

sealed class ExpiryReason {
  object NoIndicatorsMatched : ExpiryReason()
  object ExplicitLogoutDetected : ExpiryReason()
  data class Other(val detail: String) : ExpiryReason()
}

data class SessionExpiredEvent(
  val deviceId: String,
  val driverId: String,
  val app: String,
  val expiredAt: Long,
  val reason: ExpiryReason,
  val indicatorDetails: Map<String, Boolean>
)
```

### JSON wire format (session_expired message — added to PROTOCOL.md)

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "session_expired",
  "from": "rcm-tab-plus",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "device_id": "rcm-tab-plus",
    "driver_id": "zomato-partner",
    "app": "com.zomato.partner",
    "expired_at": 1713440000000,
    "reason": "no_indicators_matched",
    "indicator_details": {
      "logged_in_avatar": false,
      "profile_name_visible": false,
      "orders_tab_present": false
    },
    "staff_message": "Zomato Partner session expired on Tab Plus. Please open the app and log in."
  }
}
```

## 5. Atomic plan breakdown (7 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

---

### 434-01-PLAN — CredentialStrategy interface + SessionState sealed class + SessionExpiredEvent

**Goal:** Define the full contract surface of Phase 434 in types — nothing implemented yet. Downstream plans implement against these types. Interface-first ordering per CLAUDE.md.

**Covers:** CRED-01 (interface definition portion)

**Dependencies:** Phase 432 (DriverManifest + Driver contracts exist in the repo)

**Type:** `auto`

#### Tasks

1. Create `credential/CredentialStrategy.kt`:
   - Kotlin `interface CredentialStrategy` with 4 `suspend fun`s: `login()`, `isSessionValid()`, `refresh()`, `logout()`.
   - KDoc on each method defining exact contract semantics:
     - `login`: MAY be interactive (staff prompt). Returns the resulting SessionState. MUST be idempotent if already valid.
     - `isSessionValid`: MUST be cheap (< 5s). MUST NOT itself mutate state. Called every health-check cycle.
     - `refresh`: Best-effort non-interactive renewal. MAY fall back to triggering a login() prompt.
     - `logout`: Tear down local session state. Does NOT guarantee remote session revocation (some apps don't support it).

2. Create `credential/SessionState.kt`:
   - Kotlin `sealed class SessionState` with subtypes: `NotLoggedIn`, `Valid(lastVerifiedAt: Long, indicatorsMatched: List<String>)`, `Expired(reason: ExpiryReason, indicatorsChecked: Map<String, Boolean>)`, `Unknown(error: Throwable)`.
   - **Decision LOCKED: sealed class, NOT enum.** Rationale: each state needs payload (lastVerifiedAt, reason, error). An enum cannot carry per-variant data without the `enum class X(val f: Type)` trick, which forces all variants to share the same shape. Sealed class is the idiomatic Kotlin analog to Rust's `enum` with payloads — which is what the data model genuinely is. This is mechanical: the moment you need `Expired(reason)` vs `Valid(timestamp)`, enum is wrong.

3. Create `credential/SessionExpiredEvent.kt`:
   - `data class SessionExpiredEvent(deviceId, driverId, app, expiredAt, reason: ExpiryReason, indicatorDetails: Map<String,Boolean>)`.
   - `@Serializable` so it goes over the wire directly.
   - Companion `fun toProtocolEnvelope(): Envelope<SessionExpiredEventPayload>` method.

4. Create `credential/CredentialStrategyRegistry.kt`:
   - `object CredentialStrategyRegistry { private val factories = ConcurrentHashMap<String, (DriverManifest) -> CredentialStrategy>(); fun register(name: String, factory: (DriverManifest) -> CredentialStrategy); fun resolve(name: String, manifest: DriverManifest): CredentialStrategy }`.
   - Registration happens via each strategy class's companion-object `init { CredentialStrategyRegistry.register("persistent_session", ::PersistentSession) }`. This is the "adding a strategy requires no core code change" mechanism — the strategy file itself announces its presence.
   - Alternative considered: Java `ServiceLoader`. Rejected because ProGuard/R8 on Android release builds sometimes strips service manifests silently; Kotlin companion-init is deterministic.

5. Unit tests `CredentialStrategyTest`:
   - Test: sealed class exhaustiveness — `when(state)` over SessionState compiles without `else` branch.
   - Test: SessionExpiredEvent round-trips JSON (`@Serializable` works on all fields).
   - Test: CredentialStrategyRegistry.register + resolve is thread-safe (concurrent registrations don't lose entries).
   - Test: Resolving an unregistered name throws `CredentialStrategyNotFoundException` with the name in the message.

6. Write `rc-agent-mobile/docs/CREDENTIAL-STRATEGIES.md` skeleton (the full fill lands in 434-02 after PersistentSession exists).

#### Acceptance

- `./gradlew :app:compileDebugKotlin` succeeds.
- `./gradlew :app:testDebugUnitTest --tests 'CredentialStrategyTest'` — all green.
- `grep -rn "sealed class SessionState" rc-agent-mobile/app/src/main/` matches exactly one file.

#### G4 NOT TESTED list

- No runtime behavior yet — types only.
- No strategy implementation (PersistentSession is 434-02).
- No driver integration (434-05).

#### Commit message

```
feat(434-01): CredentialStrategy interface + SessionState + SessionExpiredEvent

Defines Phase 434 contract surface before any implementation. Sealed SessionState
carries per-variant payloads (Valid.lastVerifiedAt, Expired.reason, Unknown.error)
so callers cannot confuse states. CredentialStrategyRegistry uses companion-init
self-registration so adding a strategy does not require touching core files
(CRED-03 extensibility promise — enforced structurally in 434-07).

Covers: CRED-01 (interface portion)
Not tested: runtime semantics (434-02+), driver integration (434-05).
```

---

### 434-02-PLAN — PersistentSession implementation + SessionIndicatorEvaluator

**Goal:** Concrete `PersistentSession` strategy + `SessionIndicatorEvaluator` that queries per-driver selectors via the Phase 433 SelectorRegistry and Phase 430 AccessibilityBridge. This is the meat of v50.0 credential handling.

**Covers:** CRED-02

**Dependencies:** 434-01, Phase 430 (AccessibilityBridge), Phase 433 (SelectorRegistry)

**Type:** `auto`

#### Tasks

1. Create `credential/SessionIndicatorEvaluator.kt`:
   - Class depends on `SelectorRegistry` (Phase 433) and `AccessibilityBridge` (Phase 430), injected via constructor.
   - Method: `suspend fun evaluate(driverId: String, appPackage: String, indicatorNames: List<String>): IndicatorResult` where `IndicatorResult = { matched: Map<String,Boolean>, checkedAt: Long, accessibilityAvailable: Boolean }`.
   - Algorithm:
     1. If AccessibilityBridge reports service unavailable → return IndicatorResult with `accessibilityAvailable = false` (caller maps this to `SessionState.Unknown`, NOT Expired).
     2. Fetch screen tree for `appPackage` (may require opening/focusing the app — decision: if app not foreground, attempt a quiet `am start` via Accessibility, but DO NOT steal focus during business hours; this is handled by the humanize layer in Phase 435, for 434-02 we just query whatever tree is available and treat "no tree" as Unknown).
     3. For each indicator name: `SelectorRegistry.resolve(driverId, indicatorName) → SelectorChain`. If selector missing, log WARN + skip. `AccessibilityBridge.findNode(chain) → bool`. Record matched/not.
     4. Aggregate: if >= 1 indicator matched → call site interprets as Valid; if 0 matched AND all checks ran → Expired; if AccessibilityService unavailable → Unknown.
   - **Decision LOCKED: min_match_count = 1 by default, per-driver override.** Rationale: robust against indicator drift. A driver can declare in its manifest `session_indicators: ["logged_in_avatar", "profile_name"]` AND `min_indicator_matches: 2` if it wants stricter.

2. Create `credential/PersistentSession.kt`:
   - Class `PersistentSession(private val driverId: String, private val manifest: DriverManifest, private val evaluator: SessionIndicatorEvaluator, private val alerter: SessionExpiryAlerter, private val clock: Clock)` : `CredentialStrategy`.
   - `login()`:
     - On first run (no previous session indicator record in local storage): fire a "log in manually" prompt — persistent notification update + an admin-dashboard push via `alerter.sendStaffLoginPrompt(driverId, manifest.app)` (dashboard UI in Phase 441, but the comms event is sent now).
     - Poll `isSessionValid()` every 30s for up to 10 min (staff login window). Return `Valid` on success, `NotLoggedIn` on timeout.
     - Idempotency: if already valid, return immediately.
   - `isSessionValid()`:
     - Call `evaluator.evaluate(driverId, manifest.appPackage, manifest.sessionIndicators ?: emptyList())`.
     - `accessibilityAvailable = false` → `SessionState.Unknown(AccessibilityUnavailableException)`.
     - `matched.values.any { it }` → `SessionState.Valid(clock.now(), matched.filterValues{it}.keys.toList())`.
     - else → `SessionState.Expired(ExpiryReason.NoIndicatorsMatched, matched)`.
   - `refresh()`: for v50.0 PersistentSession, refresh = login (persistent session means the app has a durable cookie; we just re-prompt the staff to log back in). Log INFO "refresh() requesting full login via staff prompt".
   - `logout()`: clear local indicator cache; do NOT attempt to log out inside the target app (outside our control and unneeded — staff physically logs out in the target app).
   - Consecutive-failure gate (R-1 mitigation): keep an AtomicInteger `consecutiveExpiredCount`. Only *emit* `SessionExpiredEvent` when count reaches 3 consecutive failures (~15 min at 5-min healthCheck cadence). The strategy itself still returns Expired immediately — but the *alerting* side waits. This prevents the false-positive class (momentary UI lag, app foreground change during check, etc.). **The 3x gate is enforced in SessionExpiryAlerter (plan 434-06), NOT in PersistentSession** — per "separation of concerns: detection vs alerting".

3. Register PersistentSession:
   ```kotlin
   class PersistentSession(...) : CredentialStrategy {
       companion object {
           init { CredentialStrategyRegistry.register("persistent_session") { manifest -> PersistentSession(driverId = manifest.id, manifest = manifest, ...) } }
       }
   }
   ```
   - Caveat: Kotlin `companion object { init { ... } }` only runs when the class is first referenced. On Android, that happens at DriverLoader.install() time when the loader does `Class.forName("...PersistentSession").kotlin.companionObject`. This is fine for PersistentSession (always referenced in v50.0), but means OtpFlow/OAuth don't self-register until their first use. Document this in CREDENTIAL-STRATEGIES.md.
   - Alternative: put `CredentialStrategyRegistry.register(...)` calls in a dedicated `CredentialStrategyBootstrap.kt` that IS always loaded (from AgentForegroundService.onCreate). Downside: this DOES touch a central file. **Decision LOCKED: companion-init, self-announcing.** Matches Phase 432 DriverRegistry precedent. Phase 434-07 test proves this is genuinely decoupled.

4. Unit tests `PersistentSessionTest`:
   - Test: indicator match → `Valid(lastVerifiedAt, matched)`.
   - Test: no indicators match → `Expired(NoIndicatorsMatched, allFalseMap)`.
   - Test: AccessibilityService unavailable → `Unknown(AccessibilityUnavailableException)`.
   - Test: `login()` idempotent when already valid.
   - Test: `login()` fires staff prompt exactly once, not on every poll.

5. Unit tests `SessionIndicatorEvaluatorTest`:
   - Test: single indicator, matches → `matched["x"] = true`.
   - Test: single indicator, selector missing (SelectorRegistry returns null) → that indicator is skipped with WARN log, NOT treated as "no match".
   - Test: multiple indicators, partial match → reports partial map faithfully.
   - Test: min_indicator_matches = 2, only 1 matched → caller interprets as Expired (test asserts the raw matched map; the interpretation lives in PersistentSession, so add a PersistentSession test for the threshold).

6. Fill `rc-agent-mobile/docs/CREDENTIAL-STRATEGIES.md`:
   - Lifecycle diagram (NotLoggedIn → login() → polling → Valid → ... → indicator drift → Expired → alert → Paused → staff login → Valid).
   - "How to add a new strategy" guide (referenced by 434-07 test).
   - FAQ on false-positives, indicator drift, humanize layer interaction, consecutive-failure gate.

#### Acceptance

- `PersistentSessionTest` + `SessionIndicatorEvaluatorTest` all green.
- `./gradlew :app:assembleDebug` succeeds — no missing imports from Phase 430 or 433 interfaces.
- Manual inspection: CREDENTIAL-STRATEGIES.md renders correctly (`markdownlint` optional).

#### Risks addressed

- **R-1 False positives:** The `consecutiveExpiredCount=3` gate lives in SessionExpiryAlerter (434-06). PersistentSession itself returns Expired immediately — because Driver.healthCheck() needs to know. The *user-visible alert* is gated.
- **R-2 False negatives:** A `force re-check on action failure` hook is defined in the Driver.kt contract (Phase 432 amended in 434-05 task 2).
- **R-7 PII leak:** `IndicatorResult.matched` only carries indicator NAMES and booleans, NEVER the matched `AccessibilityNode.text` content. Test `SessionIndicatorEvaluatorTest.neverLogsNodeText` verifies no node text appears in log output.

#### G4 NOT TESTED list

- Not tested: live AccessibilityService on a real device (Phase 430 territory).
- Not tested: integration with a real target app (Phase 439 Zomato driver E2E).
- Not tested: event routing (434-06).

#### Commit message

```
feat(434-02): PersistentSession + SessionIndicatorEvaluator

PersistentSession strategy queries per-driver session indicator selectors
via SelectorRegistry (Phase 433) + AccessibilityBridge (Phase 430).
Returns Valid / Expired / Unknown with full indicator-level evidence.
Self-registers in CredentialStrategyRegistry via companion-object init.
Never logs matched node text (R-7 PII mitigation).

Covers: CRED-02
Not tested: live Accessibility (Phase 430), target app (Phase 439).
```

---

### 434-03-PLAN — OtpFlow future-compat interface stub

**Goal:** Define `OtpFlow` as a future-compatible interface slot. Methods throw `NotImplementedError` in v50.0. Manifest declaring `credential_strategy: "otp_flow"` is rejected at install() in v50.0 with a clear error. Forward-compat: a future phase (CRED-OTP-*) implements `OtpFlow` in a single new file with zero edits to Phase 434 code.

**Covers:** CRED-03 (OTP slot portion)

**Dependencies:** 434-01

**Type:** `auto`

#### Tasks

1. Create `credential/OtpFlow.kt`:
   ```kotlin
   /**
    * Future-compat slot for SMS/email OTP flows.
    *
    * NOT IMPLEMENTED in v50.0. Manifests declaring credential_strategy: "otp_flow"
    * are rejected by DriverLoader (see ManifestEnforcementTest). When implementing
    * in a future phase (CRED-OTP-*): create a concrete class implementing this
    * interface + register it with CredentialStrategyRegistry. No changes needed
    * to DriverLoader, CredentialStrategyRegistry, or any other Phase 434 file.
    */
   interface OtpFlow : CredentialStrategy {
       /** Triggers the target app to send an OTP. Must not block > 30s. */
       suspend fun requestOtp(): Result<Unit>

       /** Submits the received OTP code. Returns resulting SessionState. */
       suspend fun submitOtp(code: String): SessionState
   }

   /**
    * Default implementation that every method throws NotImplementedError.
    * This exists so tests can verify the slot is defined, not so production uses it.
    */
   class OtpFlowNotImplemented : OtpFlow {
       override suspend fun login() = throw NotImplementedError("OtpFlow.login() is a future phase (CRED-OTP-*). Not supported in v50.0.")
       override suspend fun isSessionValid() = throw NotImplementedError("OtpFlow.isSessionValid() is a future phase.")
       override suspend fun refresh() = throw NotImplementedError("OtpFlow.refresh() is a future phase.")
       override suspend fun logout() = throw NotImplementedError("OtpFlow.logout() is a future phase.")
       override suspend fun requestOtp() = throw NotImplementedError("OtpFlow.requestOtp() is a future phase.")
       override suspend fun submitOtp(code: String) = throw NotImplementedError("OtpFlow.submitOtp() is a future phase.")
   }
   ```

2. Do NOT register `OtpFlowNotImplemented` in CredentialStrategyRegistry. This is by design: registering it would allow a manifest to "succeed" at install but explode at first use. Better to fail loudly at install time (ManifestEnforcementError in 434-05).

3. Unit tests `OtpFlowStubTest`:
   - Test: `OtpFlowNotImplemented().login()` throws `NotImplementedError` with message mentioning "future phase".
   - Test: Every method throws `NotImplementedError`.
   - Test: The interface file can be found at the expected path (crude protection against file deletion).
   - Test: The type `OtpFlow` extends `CredentialStrategy` (compiles, so the interface slot is truly a strategy).

#### Acceptance

- `OtpFlowStubTest` passes.
- `grep -rn "class OtpFlow" rc-agent-mobile/app/src/main/` returns the intended file.

#### Commit message

```
feat(434-03): OtpFlow future-compat interface stub

Defines OtpFlow : CredentialStrategy with requestOtp() + submitOtp() methods.
v50.0: all methods throw NotImplementedError. Manifests declaring "otp_flow"
rejected at DriverLoader (enforced in 434-05). Future CRED-OTP-* phase
implements concrete class in a single new file — no edits to Phase 434 code.

Covers: CRED-03 (OTP slot)
```

---

### 434-04-PLAN — OAuth future-compat interface stub

**Goal:** Same shape as 434-03 but for OAuth flows. Interface slot defined, v50.0 implementation throws NotImplementedError, manifest rejection enforced by 434-05.

**Covers:** CRED-03 (OAuth slot portion)

**Dependencies:** 434-01

**Type:** `auto`

#### Tasks

1. Create `credential/OAuth.kt`:
   ```kotlin
   /**
    * Future-compat slot for OAuth 2.0 / OIDC flows.
    *
    * NOT IMPLEMENTED in v50.0. See OtpFlow.kt for rationale and migration path.
    */
   interface OAuth : CredentialStrategy {
       /** Returns an Intent to launch the authorization flow (browser custom tab). */
       suspend fun startAuthFlow(activity: android.app.Activity): android.content.Intent

       /** Handles the redirect callback. Returns resulting SessionState. */
       suspend fun handleCallback(intent: android.content.Intent): SessionState
   }

   class OAuthNotImplemented : OAuth {
       override suspend fun login() = throw NotImplementedError("OAuth.login() is a future phase (CRED-OAUTH-*). Not supported in v50.0.")
       override suspend fun isSessionValid() = throw NotImplementedError("OAuth.isSessionValid() is a future phase.")
       override suspend fun refresh() = throw NotImplementedError("OAuth.refresh() is a future phase. (OAuth refresh token flow.)")
       override suspend fun logout() = throw NotImplementedError("OAuth.logout() is a future phase.")
       override suspend fun startAuthFlow(activity: android.app.Activity) = throw NotImplementedError("OAuth.startAuthFlow() is a future phase.")
       override suspend fun handleCallback(intent: android.content.Intent) = throw NotImplementedError("OAuth.handleCallback() is a future phase.")
   }
   ```

2. Unit tests `OAuthStubTest`: mirror OtpFlowStubTest exactly.

3. Do NOT register in CredentialStrategyRegistry (same reasoning as 434-03).

#### Acceptance

- `OAuthStubTest` passes.
- `grep -rn "interface OAuth " rc-agent-mobile/app/src/main/` returns the file.

#### Commit message

```
feat(434-04): OAuth future-compat interface stub

Mirror of OtpFlow: interface slot + NotImplementedError stub + manifest rejection.
Includes Activity Intent method for Custom Tabs OAuth flow when a future phase
implements it.

Covers: CRED-03 (OAuth slot)
```

---

### 434-05-PLAN — Driver manifest credential-strategy declaration + runtime enforcement

**Goal:** Every driver manifest must declare a `credential_strategy` field (required). DriverLoader resolves it against `CredentialStrategyRegistry`, instantiates the strategy, and attaches it to the Driver. In v50.0, only `"persistent_session"` is accepted; `"otp_flow"` and `"oauth"` are rejected at install() with a clear `ManifestEnforcementError` pointing to the future phase. Also: amend `Driver.healthCheck()` (Phase 432) to call `strategy.isSessionValid()` each cycle and emit `SessionExpiredEvent` on `Expired`.

**Covers:** CRED-01 (enforcement), CRED-04 (healthCheck integration)

**Dependencies:** 434-01, 434-02, 434-03, 434-04, Phase 432 (DriverLoader + DriverLifecycle)

**Type:** `auto`

#### Tasks

1. Amend `driver/DriverManifest.kt` (from Phase 432):
   ```kotlin
   @Serializable
   data class DriverManifest(
       // ... Phase 432 fields
       val credentialStrategy: String,                        // NEW, required
       val sessionIndicators: List<String>? = null,           // NEW, optional (only read if credentialStrategy == "persistent_session")
       val minIndicatorMatches: Int = 1,                      // NEW, optional with default
   ) {
       init {
           require(credentialStrategy.isNotBlank()) { "credential_strategy is required in driver manifest" }
       }
   }
   ```

2. Amend `driver/DriverLoader.kt` (Phase 432):
   - In `install(manifest)`:
     1. Validate manifest schema (Phase 432 already does basic checks).
     2. `val strategy = CredentialStrategyRegistry.resolve(manifest.credentialStrategy, manifest)` — throws `CredentialStrategyNotFoundException` if not registered.
     3. **v50.0 allowlist check:** `if (manifest.credentialStrategy !in setOf("persistent_session")) throw ManifestEnforcementError("credential_strategy '${manifest.credentialStrategy}' is a future phase (CRED-OTP-* / CRED-OAUTH-*) and is not supported in v50.0. Only 'persistent_session' is supported.")`. This allowlist is a named constant `SUPPORTED_STRATEGIES_V50_0` so a future phase just appends to it.
     4. Attach `strategy` to the Driver instance (`Driver.credentialStrategy = strategy`).
     5. Call `strategy.login()` as part of install — first-run prompt fires here.

3. Amend `driver/Driver.kt` interface (Phase 432) — add field:
   ```kotlin
   interface Driver {
       var credentialStrategy: CredentialStrategy?   // Set by DriverLoader at install(). Null before install.
       // ... Phase 432 methods
   }
   ```

4. Amend the default `healthCheck()` implementation (in Phase 432's `AbstractDriver`):
   ```kotlin
   override suspend fun healthCheck() {
       val strategy = credentialStrategy ?: run {
           log.warn("healthCheck called before install — no strategy")
           return
       }
       val state = strategy.isSessionValid()
       when (state) {
           is SessionState.Valid -> { /* ok */ }
           is SessionState.Expired -> sessionExpiryAlerter.onExpired(this, state)
           is SessionState.Unknown -> log.warn("Session state unknown for $driverId: ${state.error.message}")
           is SessionState.NotLoggedIn -> sessionExpiryAlerter.onNotLoggedIn(this)
       }
   }
   ```

5. Amend `driver/DriverLifecycle.kt` (Phase 432):
   - Add `PauseReason.SessionExpired` to the sealed class.
   - Expose `fun pause(reason: PauseReason)` that is idempotent (pausing an already-paused driver is a no-op).
   - Expose `fun resume()` that re-runs `credentialStrategy!!.login()` before returning to Running.

6. Add `onActionFailure()` hook (R-2 mitigation): in `AbstractDriver`, if any driver action throws, call `credentialStrategy?.isSessionValid()` before the normal retry logic. If `Expired`, skip retry and let the expiry path fire. If `Valid`, the failure is NOT a session issue — fall through to normal retry. This is the "force re-check on action failure" hook.

7. Unit tests `ManifestEnforcementTest`:
   - Test: manifest with `credential_strategy: "persistent_session"` installs successfully.
   - Test: manifest with `credential_strategy: "otp_flow"` throws `ManifestEnforcementError` with message mentioning "future phase".
   - Test: manifest with `credential_strategy: "oauth"` throws `ManifestEnforcementError`.
   - Test: manifest with `credential_strategy: "nonsense_xyz"` throws `CredentialStrategyNotFoundException`.
   - Test: manifest missing `credential_strategy` field entirely — `@Serializable` fails to deserialize with a clear field-missing message.
   - Test: after install, `Driver.credentialStrategy` is non-null and `is PersistentSession`.
   - Test: `healthCheck()` with `Expired` state calls `SessionExpiryAlerter.onExpired()` exactly once.
   - Test: `healthCheck()` with `Unknown` state logs WARN and does NOT call alerter.

#### Acceptance

- `ManifestEnforcementTest` all green.
- `./gradlew :app:testDebugUnitTest` passes across all Phase 434 tests so far.
- Grep check: `grep -rn "SUPPORTED_STRATEGIES_V50_0" rc-agent-mobile/` returns the DriverLoader constant.

#### Risks addressed

- **R-6 OtpFlow/OAuth accidentally called in production:** the v50.0 allowlist in DriverLoader catches misconfigured manifests at install time, not first use.
- **R-8 pause semantics:** Decision LOCKED (option b): DriverLifecycle.pause() is idempotent, completes any in-flight action, then stops scheduling new actions. Mid-action abortion (option c) is rejected because Zomato/HyperPure UI actions are multi-step; aborting mid-step can leave the target app in a weird state (e.g., order accepted but confirmation dialog still open).

#### G4 NOT TESTED list

- Live integration with Phase 432 driver scheduling (tested in Phase 432's own tests).
- Event routing to admin dashboard + WhatsApp (plan 434-06).
- End-to-end with simulated driver (plan 434-07).

#### Commit message

```
feat(434-05): driver manifest credential-strategy + DriverLoader enforcement

DriverManifest gains required credential_strategy + optional session_indicators.
DriverLoader.install() resolves strategy via CredentialStrategyRegistry,
rejects otp_flow/oauth in v50.0 with ManifestEnforcementError.
Driver.healthCheck() calls strategy.isSessionValid() every cycle,
pipes Expired to SessionExpiryAlerter (defined in 434-06).

Covers: CRED-01 (enforcement), CRED-04 (healthCheck wiring)
Not tested: alerter routing (434-06), E2E (434-07).
```

---

### 434-06-PLAN — Session-expiry routing (comms-link event + admin dashboard + WhatsApp)

**Goal:** `SessionExpiryAlerter` receives `onExpired(driver, state)` calls, applies the 3-consecutive-failures gate (R-1), emits `SessionExpiredEvent` to comms-link for admin dashboard consumption, POSTs to Bono VPS racingpoint-whatsapp-bot for staff notification, and transitions the driver to `Paused(SessionExpired)` within 10s.

**Covers:** CRED-04

**Dependencies:** 434-05, Phase 429 (CommsLinkClient), Phase 432 (DriverLifecycle.pause)

**Type:** `auto`

#### Tasks

1. Create `alert/SessionExpiryAlerter.kt`:
   - Constructor: `(commsLinkClient: CommsLinkClient, httpClient: OkHttpClient, driverLifecycle: DriverLifecycle, config: AlerterConfig)`.
   - `AlerterConfig`: `whatsappBotUrl: String` (from `BuildConfig.WHATSAPP_BOT_URL` — dev default "http://100.70.177.44:8767/alerts/session-expired", prod configured via Phase 438 feature flag or EncryptedSharedPreferences). `consecutiveFailureThreshold: Int = 3`. `whatsappRetries: Int = 3`. `whatsappBackoffMs: LongArray = [1000, 2000, 4000]`.
   - Internal state: `ConcurrentHashMap<driverId, AtomicInteger>` tracking consecutive Expired callbacks per driver.

2. Method `suspend fun onExpired(driver: Driver, state: SessionState.Expired)`:
   1. Increment the driver's counter.
   2. If counter < threshold → log INFO "Expired detected, counter=$N/$threshold, not yet alerting". Return. (Gate enforcement.)
   3. Reset counter (prevent re-firing on every subsequent cycle — re-fires only if we see Valid then Expired again).
   4. Build `SessionExpiredEvent(deviceId, driver.id, driver.manifest.appPackage, clock.now(), state.reason, state.indicatorsChecked)`.
   5. Fire-and-forget via structured concurrency:
      - Emit to CommsLinkClient (non-blocking — relay handles delivery to admin dashboard in Phase 441).
      - Schedule WhatsApp HTTP POST (see step 3).
      - Transition `DriverLifecycle.pause(PauseReason.SessionExpired)` — this is the MOST important action, must complete even if the other two fail.

3. WhatsApp POST logic:
   - Build JSON: `{device_id, driver_id, app, expired_at, reason, indicator_details, staff_message: "{app_name} session expired on {device_id}. Please open the app and log in."}`.
   - `httpClient.newCall(...).enqueue` with retry. On any 2xx → log INFO "WhatsApp alert delivered". On 4xx → log ERROR, DO NOT retry (client error means the endpoint doesn't know about us — retry won't help). On 5xx or network error → backoff and retry up to 3x. After exhaustion → log ERROR "WhatsApp alert FAILED after 3 retries — staff must be notified via dashboard (Phase 441) or RotatingLog tail".

4. Method `onNotLoggedIn(driver: Driver)`:
   - Fire the "log in manually" staff prompt (same payload shape as SessionExpiredEvent but `type: "session_login_required"` and `reason: "not_logged_in"`). This is what `PersistentSession.login()` calls on first run.
   - Does NOT increment the consecutive-failure counter (first login is not an expiry).

5. Amend `comms/CommsLinkClient.kt` (Phase 429):
   - `fun sendEvent(event: SessionExpiredEvent)` — serializes via `Protocol.kt`, sends over WS with envelope `type: "session_expired"`.
   - Mirror method for `session_login_required` type.
   - On WS disconnected, drop the event (do NOT queue — the next reconnect re-registers and the next healthCheck will detect expiry again anyway). Add a RotatingLog WARN on drop for audit.

6. Amend `protocol/Protocol.kt` (Phase 429):
   - Add `@Serializable data class SessionExpiredEventPayload` matching the JSON wire format in section 4 of this document.
   - Add `@Serializable data class SessionLoginRequiredPayload` (same shape minus `reason`).

7. Amend `docs/PROTOCOL.md` (Phase 429 doc) to include the new message types.

8. Copy `comms-link/shared/session-expiry-event-v1.md` (cross-repo reference for Bono's whatsapp-bot team).

9. Unit tests `SessionExpiryAlerterTest`:
   - Test: counter increments on successive Expired calls; alerter does NOT fire at count < threshold.
   - Test: counter resets after alert fires; re-triggering requires 3 more Expired in a row.
   - Test: WhatsApp POST 200 → logged INFO.
   - Test: WhatsApp POST 400 (client error) → logged ERROR, NO retry.
   - Test: WhatsApp POST network error → 3 retries with backoff, then logged ERROR.
   - Test: Driver paused via `DriverLifecycle.pause(SessionExpired)` exactly once per alert.
   - Test: CommsLink disconnect does NOT prevent driver pause (most important side effect).
   - Test: `onNotLoggedIn` does NOT increment counter.

#### Acceptance

- `SessionExpiryAlerterTest` all green.
- Integration test in 434-07 exercises the end-to-end.
- `docs/PROTOCOL.md` + cross-repo copy updated and committed.

#### Risks addressed

- **R-1 False positives:** 3-consecutive gate.
- **R-4 WhatsApp bot down:** 3x retry + dual-channel (dashboard + WhatsApp). Driver pause happens regardless of WhatsApp delivery status.
- Independence from comms/WhatsApp: driver pause happens on best-effort basis, independent of external delivery. The system's core behavior (stop driving the app with a bad session) is robust.

#### Open question

**OQ-1:** Does the WhatsApp POST go (a) directly via OkHttp to `http://100.70.177.44:8767/alerts/session-expired` (a new endpoint on Bono's racingpoint-whatsapp-bot), or (b) via comms-link relay exec (`POST http://<relay>/relay/exec/run` with command `send_whatsapp_session_expired`)? Option (a) is simpler and keeps the WS channel for events only. Option (b) is consistent with the existing "all Bono ops go via relay" convention. **Recommendation: option (a), direct HTTP to racingpoint-whatsapp-bot with a new endpoint.** The relay is for op commands, not structured alerts. But this adds a new endpoint to Bono's bot — user to confirm and coordinate with Bono.

#### G4 NOT TESTED list

- Not tested: real WhatsApp end-to-end (requires Bono VPS bot endpoint to be live — will be exercised in Phase 434-07 integration test against a mock, and live-verified during Phase 439 Zomato driver E2E).
- Not tested: admin dashboard rendering (Phase 441).

#### Commit message

```
feat(434-06): SessionExpiryAlerter + WhatsApp routing + driver pause

3-consecutive-failures gate suppresses false positives. On expiry: emit
session_expired event via comms-link (for Phase 441 dashboard), POST to
Bono VPS racingpoint-whatsapp-bot with 3x backoff retry, pause driver via
DriverLifecycle.pause(SessionExpired). Driver pause is independent of
external delivery — robust even if both WhatsApp and comms-link are down.

Covers: CRED-04
Not tested: live WhatsApp delivery (Phase 439), admin UI (Phase 441).
OQ-1: HTTP direct vs relay exec for WhatsApp alert — went with option (a).
```

---

### 434-07-PLAN — Unit test coverage + integration test (strategy swap + session expiry E2E)

**Goal:** Two high-value tests that prove the Phase 434 success criteria structurally:
1. `StrategySwapIntegrationTest` — plugs a new `TestStrategy : CredentialStrategy` class via manifest only; proves CRED-03 "adding a new strategy does not require core code change".
2. `SessionExpiryE2ETest` — simulates full pipeline: test driver with `persistent_session` strategy + mocked AccessibilityBridge that returns "no indicators match" → within <= 5 min (simulated via virtual time) the Driver state is `Paused(SessionExpired)`, the CommsLinkClient received a `session_expired` envelope, and a mock WhatsApp HTTP server received a POST. Covers CRED-02 + CRED-04 as an integration.

**Covers:** CRED-02 (E2E verification), CRED-03 (extensibility verification), CRED-04 (routing E2E)

**Dependencies:** all prior 434-0N plans

**Type:** `auto`

#### Tasks

1. Add test fixture `app/src/androidTest/resources/fixtures/driver-manifest-test-strategy.json`:
   ```json
   {
     "id": "test-driver",
     "name": "Test Driver",
     "version": "0.0.1",
     "supportedDeviceTypes": ["tablet", "phone"],
     "appPackage": "com.example.test",
     "credentialStrategy": "test_strategy",
     "sessionIndicators": ["foo"]
   }
   ```
   Note: this manifest would FAIL DriverLoader's v50.0 allowlist. The test temporarily extends `SUPPORTED_STRATEGIES_V50_0` via a test-only override hook (exposed as `@VisibleForTesting`). The override proves that adding a strategy to the allowlist is a one-line constant change, not a code change scattered across files.

2. Create `StrategySwapIntegrationTest.kt` in `app/src/test/kotlin/.../credential/`:
   - Define a `TestStrategy : CredentialStrategy` class in the TEST SOURCE SET (not production). Register it via `CredentialStrategyRegistry.register("test_strategy") { TestStrategy() }` in the test's `@BeforeClass`.
   - Load the fixture manifest, call `DriverLoader.install(manifest)`.
   - Assert: `Driver.credentialStrategy is TestStrategy`.
   - **Critical assertion (the CRED-03 promise):** The test uses git/diff tooling to assert that no file in `rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/` was modified to make the test pass. The production source code must be identical before and after adding TestStrategy. Implementation: the test runs `git diff --name-only HEAD -- rc-agent-mobile/app/src/main/kotlin` in a `@BeforeClass` checkpoint and again in `@AfterClass`; if the list of modified files differs, the test fails with a clear "CRED-03 violation — $file was modified to enable strategy swap" message.
   - Alternative implementation if shelling out is fragile in CI: a compile-time annotation `@PhaseLocked(phase = "434")` on DriverLoader + companion test using Kotlin reflection to verify no method signatures changed. **Decision LOCKED: git-diff approach** — simpler and catches new files as well as modifications.

3. Create `SessionExpiryE2ETest.kt`:
   - Wire up: `MockAccessibilityBridge` (always returns empty matched map), real `SessionIndicatorEvaluator`, real `PersistentSession`, `MockCommsLinkClient` (captures sent envelopes in-memory), `MockHttpServer` (captures POSTs to /alerts/session-expired), real `SessionExpiryAlerter`, real `DriverLifecycle`, `VirtualClock` + `TestCoroutineScheduler`.
   - Install a test driver via DriverLoader with `credentialStrategy: "persistent_session"`.
   - Advance virtual clock by 5 minutes (one healthCheck cycle) three times (15 minutes total — 3 consecutive Expired = alert).
   - Assert:
     - `MockCommsLinkClient.sentEvents` contains exactly 1 `session_expired` envelope with expected payload.
     - `MockHttpServer.receivedPosts` contains exactly 1 entry to `/alerts/session-expired` with expected JSON.
     - `DriverLifecycle.state` for the test driver is `Paused(SessionExpired)`.
     - Total elapsed virtual time is <= 15 min, not less than 15 min — proves the gate worked and the alert didn't fire prematurely.

4. Bonus test `SessionExpiryE2ETest.flappyExpiryDoesNotAlert`:
   - Scenario: driver alternates Valid → Expired → Valid → Expired for 10 cycles.
   - Assert: counter resets on each Valid; alerter never fires (counter never reaches 3).

5. Bonus test `SessionExpiryE2ETest.unknownStateDoesNotIncrementCounter`:
   - Scenario: AccessibilityBridge reports unavailable (e.g., service briefly disabled during a system update).
   - Assert: `SessionState.Unknown` is logged WARN but counter is NOT incremented, alerter is NOT called.

6. Run the full Phase 434 test suite: `./gradlew :app:testDebugUnitTest`. All 10 test classes must pass.

#### Acceptance

- All tests green on `./gradlew :app:testDebugUnitTest`.
- `StrategySwapIntegrationTest` passes with zero production file modifications — structural proof of CRED-03.
- `SessionExpiryE2ETest` reports virtual-time elapsed = exactly 15 min between driver install and alert fire.
- `SessionExpiryE2ETest.flappyExpiryDoesNotAlert` + `unknownStateDoesNotIncrementCounter` both green.
- `./gradlew :app:assembleRelease` succeeds — type-check across whole Phase 434 module.

#### Phase ship gate (runs after 434-07)

- [ ] All Phase 434 unit + integration tests green.
- [ ] `gsd-nyquist-auditor` on 434-02 + 434-06 deliverables (business logic + silent-data-loss class).
- [ ] MMA audit on cross-system wire format (Kotlin JSON ↔ comms-link Node.js forwarder ↔ Rust server passthrough ↔ WhatsApp bot HTTP). Dual reasoning modes REQUIRED.
- [ ] Security-check (`node comms-link/test/security-check.js`) passes — confirm no credential material logged; new envelope types security-reviewed.
- [ ] `docs/CREDENTIAL-STRATEGIES.md` + `docs/PROTOCOL.md` updated and committed.
- [ ] Cross-repo file `comms-link/shared/session-expiry-event-v1.md` committed.
- [ ] Phase 434 SUMMARY.md written.

#### G4 NOT TESTED list

- NOT tested: live target app (Zomato Partner — Phase 439 integration).
- NOT tested: real AccessibilityService detecting real indicator drift — requires hardware (Phase 439 + staff walkthrough).
- NOT tested: Bono VPS racingpoint-whatsapp-bot live endpoint — needs Bono to add the endpoint. Mocked here; live-tested in Phase 439 E2E drill.
- NOT tested: admin dashboard rendering — Phase 441 territory.

#### Commit message

```
test(434-07): StrategySwapIntegrationTest + SessionExpiryE2ETest

Proves CRED-03 structurally: adding a new CredentialStrategy class + manifest
entry requires zero modifications to files under app/src/main/. Proves CRED-02
+ CRED-04 integration: 15 min of consecutive Expired → alert fires exactly once,
driver paused, envelope sent to comms-link, WhatsApp POST delivered. Flappy
expiry does NOT alert. Unknown state does NOT increment counter.

Covers: CRED-02, CRED-03, CRED-04 (verification)
Phase 434 ship gate open — MMA + nyquist next.
```

---

## 6. Risks and pitfalls

| # | Risk | Severity | Mitigation |
|---|------|----------|------------|
| R-1 | **Selector drift → false-positive SessionExpiredEvent → WhatsApp alert spam → staff ignores alerts → real expiries missed.** | HIGH (alert fatigue) | 3-consecutive-failures gate in SessionExpiryAlerter (~15 min). Staff mute-for-N-hours button in Phase 441. Per-driver confidence threshold tunable via manifest (min_indicator_matches). |
| R-2 | **False negative — session expired but indicators still match → silent driver failure.** | HIGH (silent data loss — Uday's ToS risk) | `onActionFailure()` hook in AbstractDriver forces `strategy.isSessionValid()` re-check before retry. All driver-action failures caught by the framework (Phase 432) trigger this. |
| R-3 | **PersistentSession indicator selectors drift when target app UI updates.** | HIGH | Phase 433 selector DSL with fallback chain + hot-reload handles this. `SelectorMissEvent` emitted when NO indicator matches at all — distinguishes from "session expired" (some indicators checked, none matched vs zero indicators resolvable). |
| R-4 | **WhatsApp bot endpoint down → alert lost.** | MEDIUM | 3x exponential backoff. Dual-channel (comms-link → dashboard + WhatsApp). Driver pause happens regardless. RotatingLog ERROR on final failure — staff can check `/logs/tail`. |
| R-5 | **CRED-03 "no core code change" promise quietly violated by hidden coupling.** | MEDIUM | StrategySwapIntegrationTest enforces via git-diff assertion — fails if any file under `src/main/` was modified to plug a new strategy. |
| R-6 | **OtpFlow or OAuth accidentally invoked in production → NotImplementedError crashes driver.** | MEDIUM | DriverLoader v50.0 allowlist rejects `otp_flow` + `oauth` at install() with a clear ManifestEnforcementError mentioning the future phase. Crash impossible because install fails first. |
| R-7 | **PII leak via log files.** | MEDIUM (legal — DPDP) | SessionIndicatorEvaluator logs selector names and booleans only, NEVER matched AccessibilityNode text. Unit test `SessionIndicatorEvaluatorTest.neverLogsNodeText` enforces. Cross-check by nyquist audit. |
| R-8 | **DriverLifecycle.pause() mid-action semantics ambiguity.** | LOW (was HIGH pre-decision) | DECISION LOCKED: option (b) — pause is idempotent, completes in-flight action, stops scheduling new ones. Aborting mid-step can leave the target app in a bad state. Documented in CREDENTIAL-STRATEGIES.md. |
| R-9 | **Accessibility foreground-app ambiguity — session check triggers when a different app is foreground.** | MEDIUM | SessionIndicatorEvaluator reads `AccessibilityBridge.screenTree(appPackage)` — passes the TARGET app package, not "whatever's foreground". If the target app isn't running, `screenTree` returns null → `SessionState.Unknown` (not Expired). Humanize layer in Phase 435 may be asked to foreground the app for a check; in Phase 434 we just treat "not foreground" as Unknown. |
| R-10 | **Companion-object `init { register() }` doesn't fire if class never referenced.** | LOW | PersistentSession IS always referenced in v50.0 (it's the only non-stub strategy). A test in `CredentialStrategyRegistryTest.persistentSessionIsRegistered` asserts this by name. |
| R-11 | **Circular dependency: PersistentSession depends on SessionExpiryAlerter for the staff-prompt; Alerter depends on Driver for pause().** | LOW | PersistentSession.login() only NEEDS to fire the prompt — that is `alerter.onNotLoggedIn(driver)` which does NOT call back into PersistentSession. Driver is a Driver reference, not PersistentSession — one-way dependency. Verified by compile (circular deps would fail at module boundaries). |
| R-12 | **BUG-TRACKER "silent failures look like session expiry" class (per Pattern F in session_handoff_20260417_game_launch_poe.md).** | MEDIUM | SessionExpiredEvent payload includes `indicator_details` (which indicators checked + which matched). Silent driver crashes don't produce indicator data. Phase 441 dashboard differentiates `session_expired (indicators checked)` from `driver_crashed (no indicator data)`. |

## 7. Test plan

### Unit tests (JVM, fast, run on every build)

| Test class | Plan | What it proves |
|------------|------|----------------|
| `CredentialStrategyTest` | 434-01 | Types + registry thread-safety |
| `PersistentSessionTest` | 434-02 | Strategy state machine correctness |
| `SessionIndicatorEvaluatorTest` | 434-02 | Selector → boolean map; no node-text in logs |
| `OtpFlowStubTest` | 434-03 | Slot defined, throws NotImplementedError |
| `OAuthStubTest` | 434-04 | Same for OAuth |
| `CredentialStrategyRegistryTest` | 434-01 | resolve/register thread-safe, unknown name throws |
| `ManifestEnforcementTest` | 434-05 | v50.0 allowlist rejection + healthCheck integration |
| `SessionExpiryAlerterTest` | 434-06 | Gate + retries + pause-is-robust |
| `StrategySwapIntegrationTest` | 434-07 | CRED-03 structural proof |
| `SessionExpiryE2ETest` | 434-07 | CRED-02 + CRED-04 full pipeline |

All unit tests run as part of `./gradlew :app:testDebugUnitTest`. Non-zero on any failure.

### Instrumented tests

None for Phase 434 — all tests are JVM unit tests (using mocked AccessibilityBridge, CommsLinkClient, HttpClient). Live AccessibilityService exercised in Phase 430's instrumented tests and Phase 439 E2E drill.

### Physical device tests

None for Phase 434. The strategy layer is device-independent. Physical device tests come in Phase 439 when a real Zomato Partner driver runs against the live app on Tab Plus.

### `/fleet/health` verification

Same caveat as Phase 429 §7: server `/fleet/health` extension for Android is out of scope until Phase 441 (admin dashboard). For Phase 434, `session_expired` events verified by reading the comms-link relay log.

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** 434-02 (PersistentSession state machine + SessionIndicatorEvaluator) + 434-06 (Alerter gate logic) are business logic AND silent-data-loss class. Must run before 434-07 closes.
- **MMA audit (required — cross-system bridge, silent-data-loss class):** Kotlin agent ↔ comms-link Node relay ↔ (future) admin dashboard Next.js ↔ Bono VPS racingpoint-whatsapp-bot. CLAUDE.md MMA rule mandates for cross-system bridges with dual reasoning modes. Session-expiry bugs are silent data-loss class — architecture-level review will miss trace-level bugs (e.g., counter race condition under concurrent healthChecks). Budget: $5.
- **integration-checker (required — multi-phase, cross-language):** Before v50.0 milestone ship — must run after Phase 441 (dashboard) so the full event path is testable E2E.
- **codebase-mapper:** skip — no new top-level module (rc-agent-mobile/ mapped in Phase 429).
- **ui-researcher / ui-auditor:** skip — no UI in Phase 434.
- **SEC gate (required):** Session indicators could be interpreted as credentials; `node comms-link/test/security-check.js` must be extended to confirm (a) no credential material leaked in logs; (b) SessionExpiredEvent payload contains no secret; (c) new `session_expired` WS message type follows existing auth rules on the comms-link side.
- **DMP:** see frontmatter `deploy:` — executor ticks each item.
- **Backlog gate (CLAUDE.md CGP v4.3):** Phase 434 must reach DEPLOYED-VERIFIED (APK built, unit tests green, MMA + nyquist both cleared) before Phase 439 (Zomato driver) may begin — Zomato driver is the first real consumer. COMMITTED ≠ SHIPPED.

## 9. Open questions the planner cannot decide

Listed in execution-blocking order.

**OQ-1 — WhatsApp alert routing: direct HTTP vs relay exec (BLOCKS 434-06 implementation detail).**
Does the session-expiry WhatsApp alert go (a) directly from Android agent via OkHttp to `http://100.70.177.44:8767/alerts/session-expired` on Bono VPS racingpoint-whatsapp-bot, or (b) via comms-link relay exec (`POST /relay/exec/run` with command `send_whatsapp_session_expired`)? Option (a) requires Bono to expose a new endpoint; option (b) reuses the existing exec channel but mixes structured events into an ops channel. **Recommendation: option (a).** User to confirm AND user to ping Bono with the exact endpoint spec (`POST /alerts/session-expired`, JSON body shape in section 4 of this document) so Bono can add it. If Bono objects, fall back to (b).

**OQ-2 — WhatsApp staff target: group chat vs individual (SHIP-BLOCKING for CRED-04 E2E).**
Which WhatsApp number/group receives the session-expiry alert? Uday? A venue-operations WhatsApp group? James's WhatsApp? The existing racingpoint-whatsapp-bot probably already has a concept of "ops channel" — confirm the ID to use. **Recommendation: venue-operations WhatsApp group if one exists; fall back to Uday if not.** Configurable per-deploy so test APKs can route alerts to James only.

**OQ-3 — PersistentSession staff-login prompt delivery channel.**
When `strategy.login()` fires because the driver was just installed OR because a `SessionExpired` was confirmed, the staff needs to be told "open the app and log in". Delivery channels:
(a) Persistent notification on the Android device itself (visible to whoever is holding the Tab Plus / M07).
(b) WhatsApp alert to staff (reuses 434-06 infrastructure).
(c) Admin dashboard push (Phase 441).
**Recommendation: all three, but the persistent notification is the authoritative one (the device knows it needs login; the user physically at the device sees it).** Low effort. Confirm before 434-02 implements staff-prompt semantics.

**OQ-4 — session-indicator selector ownership (Phase 433 coordination).**
Phase 434 defines HOW indicators are evaluated; Phase 433 owns the selector DSL that defines them. The driver manifest field `session_indicators: ["logged_in_avatar", "profile_name"]` — these names MUST match keys in the Phase 433 YAML (e.g., `zomato-partner/v3.14.2/selectors.yaml` has a `session_indicators:` block). Confirm with Phase 433 planner (if separate) that the YAML schema includes this block. **Recommendation: add `session_indicators:` as a first-class top-level key in the Phase 433 YAML schema, parallel to `screens:` and `elements:`.** This keeps indicator selectors discoverable and separable from action selectors. User to sign off or route back to Phase 433 author.

**OQ-5 — Does PersistentSession need to know how to open the target app?**
If the app isn't foreground when `isSessionValid()` runs, AccessibilityBridge returns null screen tree → `SessionState.Unknown`. Option (a): PersistentSession foregrounds the app quietly via `am start -n <package>/<activity>`. Option (b): rely on the humanize layer (Phase 435) to schedule the check when the app IS foreground. Option (c): treat "not foreground" as Unknown and let the driver's normal action lifecycle (which WILL foreground the app) be the only check path. **Recommendation: option (c) for v50.0.** Simpler, and matches the "isSessionValid is cheap" contract (no side effects). Downside: healthCheck cycles when the app isn't foreground produce Unknown (not Expired, not Valid), which doesn't decrement or increment the consecutive-failure counter, effectively pausing expiry detection while the app is backgrounded. If a real expiry occurs during this window, it's detected on the next driver action via `onActionFailure()`. Accept this trade-off for v50.0.

**OQ-6 — Concurrency: what if two healthChecks run simultaneously on the same driver?**
If Driver.healthCheck() is triggered concurrently (scheduled by DriverScheduler and on-demand by admin dashboard), PersistentSession.isSessionValid() runs twice in parallel, each hits AccessibilityBridge, each gets a result, each increments the consecutive-failure counter. Counter could reach 3 in half the expected time (false early alert). **Mitigation: wrap isSessionValid() calls in a per-driver Mutex** (not held across scope — held only for the check duration, which is < 5s by contract). Alternative: use a single-flight pattern (if a check is already in progress, return the cached result). **Recommendation: single-flight with 30s TTL.** Implementation: `ConcurrentHashMap<driverId, Deferred<SessionState>>` — simultaneous callers await the same Deferred. Confirm before 434-02 implements.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` CRED-01..04
- **Dependency phases:** 429 (agent scaffold), 430 (Accessibility), 432 (driver framework), 433 (selector DSL)
- **Consumer phases:** 439 (Zomato driver — first real PersistentSession user), 440 (HyperPure), 441 (admin dashboard reception view — consumes `session_expired` events)
- **Reference spec:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Reference relay protocol:** `comms-link/docs/PROTOCOL.md`
- **Future phases:** CRED-OTP-* (OtpFlow implementation), CRED-OAUTH-* (OAuth implementation) — both listed as "Future Requirements" in REQUIREMENTS-v50.md

## 11. Output (at phase close)

At the end of plan 434-07 (integration tests pass + MMA + nyquist clear), create `.planning/phases/434-credential-abstraction/SUMMARY.md` capturing:

- Which commits implemented each plan (434-01 through 434-07)
- Unit + integration test results (pass counts, virtual-time measurements for SessionExpiryE2ETest)
- nyquist-audit findings summary (pass/fail, issues closed)
- MMA audit findings summary (consensus, bugs found + fixed, budget used)
- Any risks encountered and how they were resolved
- Resolutions to open questions (update §9 state: OQ-1 through OQ-6)
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked (especially: Bono VPS whatsapp-bot endpoint verified live if OQ-1 resolved as option (a))
- Handoff to Phase 439 (Zomato driver) — what's ready, what PersistentSession hooks the driver should use, cheat-sheet for `session_indicators` manifest field

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 6 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
