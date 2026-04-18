---
phase: 435-humanize-layer-audit-log
phase_number: 435
milestone: v50.0 rc-agent-mobile
name: "Humanize Layer + Audit Log"
status: ready-to-execute
goal: >
  Every UI action dispatched by any driver (tap, swipe, text-input, screen-read) passes
  through a shared HumanizeInterceptor chain before Accessibility dispatch. The chain
  injects randomized delays (per-action-type N(mean_ms, stddev_ms)), a business-hours
  gate (configurable window, default 08:00-23:00 IST) with per-driver policy
  (queue_until_window | drop_with_log), a token-bucket rate limiter keyed on
  (driver_id, app_package). Every action, humanize decision, and outcome emits a
  structured AuditEvent to a thread-safe JSONL writer. Events include timestamp,
  driver, screen, selector, selector match confidence, screenshot SHA256, and outcome.
  Logs rotate locally (10 MB x 50 files = 500 MB cap, oldest-first eviction) and ship
  hourly to the comms-link relay at POST /api/v1/mobile-audit/ingest. Screenshots are
  captured via AccessibilityService.takeScreenshot() (Android 11+); when FLAG_SECURE
  blocks capture the event records the sentinel "sha256:unavailable:flag_secure" and
  continues. This phase is the MANDATORY ToS-risk mitigation for Zomato, HyperPure, and
  Blinkit automation — drivers must not dispatch a single action outside the chain.
requirements: [HUMANIZE-01, HUMANIZE-02, HUMANIZE-03, HUMANIZE-04, AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04]
depends_on: [432]                    # 432 ships DriverContext; 435 wraps accessibility in interceptor chain before 437 (Zomato) consumes it
wave: 4                              # 432 is wave 3 (via 430 -> 432); 435 is wave 4
plan_count: 9
plans:
  - 435-01-PLAN: HumanizeInterceptor interface + InterceptorChain + DispatchAction + default delay impl
  - 435-02-PLAN: BusinessHoursGate interceptor (IST window, per-driver policy)
  - 435-03-PLAN: RateLimiter interceptor (token-bucket, per driver_id + app_package)
  - 435-04-PLAN: AuditEvent data model + JsonlWriter (thread-safe, append-only, single-writer coroutine)
  - 435-05-PLAN: RotationPolicy (10 MB x 50 files = 500 MB cap, oldest-first eviction)
  - 435-06-PLAN: ScreenshotCapture (AccessibilityService.takeScreenshot + SHA256 + FLAG_SECURE fallback)
  - 435-07-PLAN: HourlyShippingClient (batch last-hour JSONL, POST via comms-link, retry-with-backoff, mark-shipped state)
  - 435-08-PLAN: Server-side stub ingest endpoint (POST /api/v1/mobile-audit/ingest -> 200, no storage)
  - 435-09-PLAN: Unit tests (interceptor chain, business-hours, rate-limiter, rotation, shipping) + Tab Plus integration drill (1000 mock actions)
autonomous: false   # 435-09 contains a human-verify checkpoint on the integration drill (physical Tab Plus)

files_modified:
  # Kotlin agent — interceptor chain + audit producer
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/DispatchAction.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/ActionOutcome.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/HumanizeInterceptor.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/InterceptorChain.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/DelayInterceptor.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/BusinessHoursGate.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/RateLimiter.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/HumanizeConfig.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/humanize/HumanizeAccessibilityBridge.kt
  # Audit log
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/audit/AuditEvent.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/audit/JsonlWriter.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/audit/RotationPolicy.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/audit/AuditLog.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/audit/ShippingClient.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/audit/ShippedCursor.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/audit/ScreenshotCapture.kt
  # DriverContext extension (additive)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverContext.kt  # +dispatch: DispatchBridge, +audit: AuditLog
  # Docs
  - rc-agent-mobile/docs/HUMANIZE.md
  - rc-agent-mobile/docs/AUDIT-LOG.md
  # Tests
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/humanize/InterceptorChainTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/humanize/DelayInterceptorTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/humanize/BusinessHoursGateTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/humanize/RateLimiterTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/audit/JsonlWriterTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/audit/RotationPolicyTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/audit/ShippingClientTest.kt
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/audit/ScreenshotCaptureTest.kt
  # Server (stub)
  - crates/racecontrol/src/api/mobile_audit.rs        # POST /api/v1/mobile-audit/ingest stub
  - crates/racecontrol/src/api/mod.rs                 # route registration
  - crates/racecontrol/src/api/routes.rs              # route registration
  # Relay forwarder (if comms-link needs a route hint; stub)
  - comms-link/james/mobile-audit-forward.js
  # Phase artifacts
  - .planning/phases/435-humanize-layer-audit-log/SUMMARY.md

# DMP — Deploy Manifest Protocol
deploy:
  rust_binary: [racecontrol]         # 435-08 adds POST /api/v1/mobile-audit/ingest stub to server binary
  frontend_rebuild: [none]           # Admin dashboard log viewer is Phase 441
  config_change: >
    racecontrol.toml [mobile_audit] section (optional): max_batch_bytes=5242880, stub_mode=true.
    If omitted, server uses hardcoded defaults.  No schema migration.
  db_migration: none                 # Stub endpoint writes nothing.  Real storage = Phase 441.
  infrastructure: >
    comms-link relay (James .27:8765 AND Bono VPS:8765) must forward
    POST /api/v1/mobile-audit/ingest from Android clients rcm-tab-plus and rcm-m07
    to the venue racecontrol server at http://192.168.31.23:8080/api/v1/mobile-audit/ingest.
    For DEPLOY PARITY the cloud relay forwards to the cloud racecontrol
    (Bono VPS pm2 "racecontrol" on :8080).
  data_files: >
    rc-agent-mobile/app/src/main/assets/humanize-defaults.json
    (default per-action-type delay profiles; hot-overridable per driver at runtime).
  bat_file: none
  cloud_parity:
    - racecontrol binary (stub ingest endpoint) deploys to cloud Bono VPS.
    - comms-link cloud relay forwards /api/v1/mobile-audit/ingest same as James relay.
  targets:
    - tab_plus                       # Lenovo TB-351FU — APK reinstall
    - m07                            # Samsung Galaxy M07 — APK reinstall
    - server_23                      # venue racecontrol binary (stub endpoint)
    - bono_vps                       # cloud racecontrol (stub endpoint, DEPLOY PARITY)
    - james_27                       # comms-link relay forward config
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  rollback:
    - "APK: adb install -r /sdcard/Download/rc-agent-mobile-prev.apk on both devices"
    - "Server: rename racecontrol-prev.exe back and restart via schtasks StartRCTemp"
    - "Relay: revert comms-link/james/mobile-audit-forward.js via git checkout"

# Subagent gates (per CLAUDE.md > Subagent Gates)
gates:
  ui_researcher: skip                # No user-facing UI in 435.  Admin log viewer = Phase 441.
  ui_auditor: skip                   # Same.
  nyquist_auditor: required          # Interceptor chain, business-hours math, token-bucket math, rotation math are all business logic with defined I/O.
  mma_audit: required                # HUMANIZE is THE ToS-risk mitigation.  Cross-system: Kotlin interceptor chain -> JSONL producer -> comms-link -> Rust stub.  Dual reasoning modes REQUIRED (abstract AND trace-level — CLAUDE.md MMA rule).
  integration_checker: required      # Integration with Phase 432 DriverContext + 430 AccessibilityBridge must not regress before 437 (Zomato) depends on it.
  codebase_mapper: skip              # 435 extends existing rc-agent-mobile module; no new top-level directory.

risks_summary:
  - "AccessibilityService.takeScreenshot() blocks for ~150-300 ms on many devices and cannot be parallelised with tap dispatch (the two contend for the same AccessibilityService thread).  Mitigation: screenshots are taken AFTER the action dispatch completes, not before.  Every AuditEvent therefore carries the PRE-action screenshot only if pre_capture=true (default OFF for tap, ON for selector miss recovery).  See 435-06 for full discussion."
  - "Token-bucket concurrency: multiple drivers dispatching on separate coroutines can race on the same bucket.  Mitigation: buckets are per (driver_id, app_package), held in a ConcurrentHashMap, and each bucket's acquire() is serialised via a Mutex (NOT held across suspend — follows CLAUDE.md rule).  Unit test NoLockAcrossAwaitTest enforces this."
  - "FLAG_SECURE apps (Zomato Partner sets FLAG_SECURE during login) block screen capture entirely.  Mitigation: sentinel hash 'sha256:unavailable:flag_secure' recorded; driver continues.  Audit trail still has timestamp + selector + outcome, just no visual evidence."
  - "Hourly shipping can back up if relay is offline.  Mitigation: ShippedCursor (persisted in EncryptedSharedPreferences) tracks the last successfully-shipped line.  On next hourly tick after reconnect, agent ships backlog.  If backlog exceeds local 500 MB cap, rotation drops oldest — ToS accountability gap noted in §9 OQ-4."
  - "IST hardcoding: BusinessHoursGate defaults to 08:00-23:00 IST.  If James ever extends the fleet to a different timezone (cloud kitchen Mumbai branch?), the gate must accept a tz field in config.  For v50.0 we hardcode IST and document the assumption."
  - "Stub ingest endpoint returning 200 looks exactly like success to the agent but discards data.  Phase 441 MUST replace it before any real ToS incident occurs.  Until then, audit trail exists only on-device (500 MB = ~ 2 weeks of typical load).  Explicit WARN banner on admin dashboard Phase 441 will surface this."
  - "JsonlWriter coroutine channel overflow: if audit events burst faster than disk can flush, bounded channel (1024) drops events with ERROR log.  Mitigation: channel oversize triggers a 'HumanizeInterceptor slow-start' (50 ms back-pressure on DelayInterceptor) to naturally throttle.  See 435-04."
  - "Android 10 (minSdk=29) devices do NOT support AccessibilityService.takeScreenshot (API 30+).  Both v50.0 target devices are Android 13/14 — no impact.  But if fleet expands, 435-06 explicitly degrades to sentinel 'sha256:unavailable:api_too_low' on API < 30."
  - "Key links (wiring most likely to break): (a) DriverContext.dispatch MUST replace direct AccessibilityBridge access (432 exposed accessibility directly; 435 makes dispatch the only legal path — grep enforces in 435-01).  (b) JsonlWriter MUST be a singleton across drivers (one writer, one file) or rotation breaks — grep enforces.  (c) ShippingClient MUST NOT ship while rotation is mid-write — guarded via atomic cursor."
---

# Phase 435 — Humanize Layer + Audit Log

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 435 |
| Name | Humanize Layer + Audit Log |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | HUMANIZE-01, HUMANIZE-02, HUMANIZE-03, HUMANIZE-04, AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04 |
| Dependencies | Phase 432 (DriverContext + AccessibilityBridge injection) |
| Wave | 4 |
| Status | Ready to execute |
| Autonomous | No — plan 435-09 has a physical-device human-verify checkpoint |
| Ship test | Every tap/swipe/text event logged with timestamp + driver + selector + outcome + screenshot hash; business-hours gate drops or queues per policy outside window; rate limiter enforces per-app ceiling; logs rotate at 500 MB; hourly shipping succeeds to relay |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 7)

1. **AUDIT-01 — Every tap/swipe/text event logged** with timestamp + driver + screen + selector + selector_match_confidence + outcome + screenshot_sha256.
2. **HUMANIZE-02 — Business-hours gate configurable;** outside window, driver queues or drops per policy.
3. **HUMANIZE-03 — Rate limiter enforces per-app ceiling;** excess actions queue or drop.
4. **AUDIT-02 — Logs rotate locally at 500 MB cap;** hourly shipping to server succeeds via comms-link.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)

- T-1: When any driver calls `ctx.dispatch.tap(selector)`, a HumanizeInterceptor chain runs before the tap reaches AccessibilityBridge (HUMANIZE-01).
- T-2: After the action completes, `adb shell cat /sdcard/Android/data/in.racingpoint.rcagentmobile/files/audit/audit-000.jsonl | tail -1 | jq .` returns a JSON object with all eight audit fields (ts, driver_id, app_package, screen, selector_id, selector_match_confidence, action_type, outcome, screenshot_sha256, agent_build_id) (AUDIT-01).
- T-3: At 23:15 IST, an automated test action on a driver with policy `drop_with_log` produces an AuditEvent with `outcome="dropped_business_hours"` and ZERO Accessibility tap dispatched (HUMANIZE-02).
- T-4: At 23:15 IST, an automated test action on a driver with policy `queue_until_window` produces an AuditEvent with `outcome="queued_business_hours"` and, at 08:00 IST next morning, the queued action dispatches with `outcome="success"` (HUMANIZE-02).
- T-5: Under load of 120 actions/minute on a driver configured for 60 actions/minute, the agent logs exactly 60 dispatched + 60 `outcome="rate_limited"` in any rolling 60-second window (HUMANIZE-03).
- T-6: After the on-device audit log reaches 500 MB, the oldest rotated file is deleted on the next rotation tick; total size never exceeds 500 MB + 10 MB (one active file) (AUDIT-02 rotation).
- T-7: Once per hour (on a 60 min ticker), the agent POSTs the last hour's unsent audit batch to `POST /api/v1/mobile-audit/ingest` via comms-link; server returns 200 (stub); agent advances `shipped_cursor` (AUDIT-02 shipping).
- T-8: A driver launches Zomato Partner (which sets FLAG_SECURE in its login screen). Every AuditEvent emitted from that screen records `screenshot_sha256: "sha256:unavailable:flag_secure"` and continues (AUDIT-01 + 435-06).
- T-9: Humanize config is hot-reloadable — editing `humanize_delay_mean_ms` in a driver manifest (or pushing a new config via comms-link) takes effect within 10s without restart (HUMANIZE-04).
- T-10: Every AuditEvent's `agent_build_id` matches the currently-installed APK build_id (no drift between code and logs).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `.../humanize/DispatchAction.kt` | Sealed class describing an action request | 50 | `sealed class DispatchAction { Tap(selector, screen); Swipe(from, to, screen); TextInput(selector, text, screen); ScreenRead(screen) }` |
| `.../humanize/ActionOutcome.kt` | Enum of all possible outcomes | 40 | `enum class ActionOutcome { SUCCESS, MISS, ERROR, DROPPED_BUSINESS_HOURS, QUEUED_BUSINESS_HOURS, RATE_LIMITED, RETRIED }` |
| `.../humanize/HumanizeInterceptor.kt` | OkHttp-style interceptor contract | 40 | `interface HumanizeInterceptor { suspend fun intercept(chain: Chain): ActionResult }` |
| `.../humanize/InterceptorChain.kt` | Chain runner | 80 | Immutable list of interceptors, `suspend fun proceed(action)`, index-based next() |
| `.../humanize/DelayInterceptor.kt` | Randomized delay N(mean, stddev) per action type | 60 | `sample()` uses Java Random with per-action-type profile |
| `.../humanize/BusinessHoursGate.kt` | Window gate with queue or drop policy | 100 | TimeZone Asia/Kolkata, configurable window, policy dispatcher |
| `.../humanize/RateLimiter.kt` | Token-bucket per (driver_id, app_package) | 120 | ConcurrentHashMap<BucketKey, TokenBucket>, serialised acquire, no lock across await |
| `.../humanize/HumanizeConfig.kt` | Per-driver + per-device config | 60 | Kotlinx data class, hot-reload subscription |
| `.../humanize/HumanizeAccessibilityBridge.kt` | Wraps 430 AccessibilityBridge through the chain | 80 | The ONLY public path drivers use for UI actions post-435 |
| `.../audit/AuditEvent.kt` | Structured event record | 60 | Kotlinx @Serializable data class, 10 fields |
| `.../audit/JsonlWriter.kt` | Thread-safe append-only writer | 100 | Single-writer coroutine, bounded channel (1024), line-oriented append |
| `.../audit/RotationPolicy.kt` | 10 MB x 50 files rotation | 80 | Filesize check, rename cascade, oldest-first delete |
| `.../audit/AuditLog.kt` | Singleton façade for drivers | 60 | `suspend fun emit(event: AuditEvent)`, wraps JsonlWriter + RotationPolicy |
| `.../audit/ShippingClient.kt` | Hourly batch POST to comms-link | 120 | Ticker coroutine, reads unshipped range via ShippedCursor, POSTs, advances cursor on 200 |
| `.../audit/ShippedCursor.kt` | Persistent last-shipped pointer | 60 | `(file_name, byte_offset)` tuple persisted in EncryptedSharedPreferences |
| `.../audit/ScreenshotCapture.kt` | AccessibilityService.takeScreenshot + SHA256 | 120 | API 30+ check, FLAG_SECURE detection, sentinel fallback, MessageDigest SHA-256 |
| `crates/racecontrol/src/api/mobile_audit.rs` | Server-side stub ingest | 60 | Axum handler: POST /api/v1/mobile-audit/ingest, accept JSONL body, return 200, log count |
| `rc-agent-mobile/docs/HUMANIZE.md` | Interceptor chain architecture | 150 | Chain order, config shape, rate-limiter math, worked example |
| `rc-agent-mobile/docs/AUDIT-LOG.md` | AuditEvent schema + shipping protocol | 150 | Line format, rotation policy, shipping contract, privacy notes |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| Driver code (e.g., ZomatoDriver in Phase 437) | HumanizeAccessibilityBridge | `ctx.dispatch.tap(...)` | grep `ctx.accessibility` across all driver/ code: MUST return zero matches after 435 (all access goes via dispatch) |
| HumanizeAccessibilityBridge | InterceptorChain.proceed | Kotlin call | grep `InterceptorChain(` in HumanizeAccessibilityBridge.kt |
| InterceptorChain | DelayInterceptor -> BusinessHoursGate -> RateLimiter -> AccessibilityBridge (terminal) | ordered list | assert order in `HumanizeAccessibilityBridgeTest.chainOrderIsStable` |
| DelayInterceptor.intercept | AuditLog.emit (timing event) | suspend call | grep `AuditLog.emit` in DelayInterceptor.kt |
| BusinessHoursGate (outside window + queue_until_window) | DelayQueue (local in-memory) | Java util | assert FIFO order in `BusinessHoursGateTest.queueDelayedActionsDispatchInOrder` |
| RateLimiter.acquire | TokenBucket.refill | single-bucket mutex | grep `Mutex` and assert not held across `delay(` in `RateLimiterTest.noLockAcrossAwait` |
| AuditLog.emit | JsonlWriter.write (channel send) | `Channel.send` | grep `Channel.send` in AuditLog.kt |
| JsonlWriter writer coroutine | RotationPolicy.checkAndRotate | Kotlin call | must be called BEFORE every write; grep `checkAndRotate` in JsonlWriter.kt |
| ShippingClient | ShippedCursor + JsonlWriter read API | Kotlin calls | ShippingClient MUST NOT read a file that RotationPolicy is mid-renaming; guarded via atomic lock (see 435-07) |
| ShippingClient | comms-link relay | OkHttp POST via CommsLinkClient reverse-tunnel | see §4 for exact endpoint |
| ScreenshotCapture.captureOrSentinel | SHA256 digest | MessageDigest | on FLAG_SECURE: returns `"sha256:unavailable:flag_secure"` verbatim |
| Server /api/v1/mobile-audit/ingest (stub) | tracing::info count only | Rust log | grep `mobile_audit_batch_received` in crates/racecontrol |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./comms-link/docs/PROTOCOL.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md  # reference log file pattern (RotatingLog); 435 JsonlWriter follows same conventions
@./.planning/phases/430-accessibility-service-foundation/PLAN.md  # reference AccessibilityBridge interface — HumanizeAccessibilityBridge wraps it
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md  # DriverContext adds .dispatch + .audit slots in this phase
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverContext.kt  # extended here
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/log/RotatingLog.kt  # similar-but-separate rotation pattern (429-07)

### Interfaces executors will need

The 435 framework's public surface:

```kotlin
// humanize/DispatchAction.kt
sealed class DispatchAction {
    abstract val driverId: String
    abstract val appPackage: String
    abstract val screen: String
    abstract val actionType: ActionType           // TAP | SWIPE | TEXT_INPUT | SCREEN_READ

    data class Tap(
        override val driverId: String,
        override val appPackage: String,
        override val screen: String,
        val selectorId: String,
        val selectorMatchConfidence: Double       // 0.0 .. 1.0 — set by 433 selector resolver
    ) : DispatchAction() { override val actionType = ActionType.TAP }

    data class Swipe(
        override val driverId: String,
        override val appPackage: String,
        override val screen: String,
        val fromX: Int, val fromY: Int, val toX: Int, val toY: Int
    ) : DispatchAction() { override val actionType = ActionType.SWIPE }

    data class TextInput(
        override val driverId: String,
        override val appPackage: String,
        override val screen: String,
        val selectorId: String,
        val text: String,
        val selectorMatchConfidence: Double
    ) : DispatchAction() { override val actionType = ActionType.TEXT_INPUT }

    data class ScreenRead(
        override val driverId: String,
        override val appPackage: String,
        override val screen: String
    ) : DispatchAction() { override val actionType = ActionType.SCREEN_READ }
}

enum class ActionType { TAP, SWIPE, TEXT_INPUT, SCREEN_READ }

// humanize/HumanizeInterceptor.kt
interface HumanizeInterceptor {
    suspend fun intercept(chain: Chain): ActionResult

    interface Chain {
        val action: DispatchAction
        val config: HumanizeConfig
        suspend fun proceed(action: DispatchAction): ActionResult
    }
}

data class ActionResult(
    val outcome: ActionOutcome,
    val durationMs: Long,
    val error: Throwable? = null,
    val nodeSnapshot: String? = null        // serialised AccessibilityNodeInfo for SCREEN_READ
)

// humanize/HumanizeAccessibilityBridge.kt — THE ONLY LEGAL PATH for driver -> UI
class HumanizeAccessibilityBridge(
    private val underlying: AccessibilityBridge,          // from 430
    private val auditLog: AuditLog,
    private val screenshotCapture: ScreenshotCapture,
    private val chainFactory: (HumanizeConfig) -> InterceptorChain,
    private val configProvider: (driverId: String) -> HumanizeConfig
) {
    suspend fun tap(driverId: String, appPackage: String, screen: String, selectorId: String, confidence: Double): ActionResult
    suspend fun swipe(driverId: String, appPackage: String, screen: String, from: Pair<Int,Int>, to: Pair<Int,Int>): ActionResult
    suspend fun textInput(driverId: String, appPackage: String, screen: String, selectorId: String, text: String, confidence: Double): ActionResult
    suspend fun screenRead(driverId: String, appPackage: String, screen: String): ActionResult
}

// humanize/HumanizeConfig.kt
@Serializable
data class HumanizeConfig(
    val driver_id: String,
    val delay_profiles: Map<ActionType, DelayProfile>,    // per-action-type
    val business_hours: BusinessHoursConfig,
    val rate_limit_per_minute: Int,                       // 0 = disabled
    val device_override: DeviceOverride? = null
)

@Serializable
data class DelayProfile(val mean_ms: Int, val stddev_ms: Int)

@Serializable
data class BusinessHoursConfig(
    val enabled: Boolean = true,
    val start_hour: Int = 8, val start_minute: Int = 0,
    val end_hour: Int = 23, val end_minute: Int = 0,
    val timezone: String = "Asia/Kolkata",
    val policy: Policy = Policy.QUEUE_UNTIL_WINDOW
) {
    enum class Policy { QUEUE_UNTIL_WINDOW, DROP_WITH_LOG }
}

// audit/AuditEvent.kt
@Serializable
data class AuditEvent(
    val ts_ms: Long,
    val agent_build_id: String,
    val driver_id: String,
    val app_package: String,
    val screen: String,
    val selector_id: String?,                 // null for SWIPE / SCREEN_READ
    val selector_match_confidence: Double?,   // null for SWIPE / SCREEN_READ
    val action_type: ActionType,
    val outcome: ActionOutcome,
    val duration_ms: Long,
    val screenshot_sha256: String,            // 64-hex OR "sha256:unavailable:<reason>"
    val error_class: String? = null,
    val error_message: String? = null
)

// audit/AuditLog.kt — singleton façade
interface AuditLog {
    suspend fun emit(event: AuditEvent)
    fun readRange(from: Long, to: Long): Sequence<String>    // for ShippingClient
    companion object { val MAX_FILES: Int = 50; val MAX_BYTES_PER_FILE: Long = 10L * 1024 * 1024 }
}
```

**Rust server-side stub (435-08):**

```rust
// crates/racecontrol/src/api/mobile_audit.rs
#[derive(Deserialize)]
pub struct MobileAuditBatch {
    pub device_id: String,
    pub agent_build_id: String,
    pub event_count: usize,
    pub events_jsonl: String,      // newline-delimited AuditEvent JSON lines
}

pub async fn ingest_handler(
    State(state): State<AppState>,
    Json(batch): Json<MobileAuditBatch>,
) -> impl IntoResponse {
    tracing::info!(
        device_id = %batch.device_id,
        build_id = %batch.agent_build_id,
        count = batch.event_count,
        "mobile_audit_batch_received"
    );
    // STUB: no storage.  Phase 441 will add SQLite insert + drift detection.
    (StatusCode::OK, Json(serde_json::json!({"accepted": batch.event_count, "storage": "stub"})))
}
```

## 5. Atomic plan breakdown (9 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion. Order is strictly sequential within dependency groups.

Dependency order (parentheses = plan #):
- 435-01 defines contracts. No dependencies within phase.
- 435-02, 435-03 implement interceptors. Both depend on 435-01.
- 435-04 defines audit writer. No dependencies within phase (independent track).
- 435-05 adds rotation on top of 435-04.
- 435-06 adds screenshot capture. Depends on 430 AccessibilityBridge.
- 435-07 ships audit batches. Depends on 435-04, 435-05, 429 comms-link.
- 435-08 adds Rust server stub. Independent — can land any time before 435-09.
- 435-09 E2E drill. Depends on everything.

---

### 435-01-PLAN — HumanizeInterceptor interface + InterceptorChain + DispatchAction + DelayInterceptor

**Goal:** Lock the public contract of the interceptor chain. Interface-first (per CLAUDE.md planning philosophy) so that 435-02 (business hours), 435-03 (rate limiter), and 437+ (drivers) program against stable types. Include the simplest real interceptor (DelayInterceptor) so the chain is exercised end-to-end.

**Covers:** HUMANIZE-01, HUMANIZE-04 (config shape)

**Dependencies:** Phase 432 complete (DriverContext exists and can be extended). 430 AccessibilityBridge stubbed if not merged.

**Type:** `auto` (TDD)

**TDD:** `tdd="true"`

#### Behavior (tests written BEFORE code)

- Test 1: `InterceptorChainTest.emptyChainReachesTerminal` — a chain with zero interceptors proceeds straight to the terminal (AccessibilityBridge) and returns its result unchanged.
- Test 2: `InterceptorChainTest.chainProceedsInOrder` — three mock interceptors log their index on enter; assert `[0, 1, 2]` order, then `[2, 1, 0]` on unwind.
- Test 3: `InterceptorChainTest.interceptorCanShortCircuit` — an interceptor returns `ActionResult(outcome=RATE_LIMITED)` without calling `chain.proceed()`; assert terminal is never invoked.
- Test 4: `DelayInterceptorTest.delayDistributionIsGaussian` — given `DelayProfile(mean=800, stddev=200)`, 1000 samples: mean within 780..820, stddev within 180..220 (chi-square tolerance), no negative delays (clamp at 0).
- Test 5: `DelayInterceptorTest.perActionTypeProfileUsed` — config has different profiles for TAP vs SWIPE; assert DelayInterceptor picks the correct profile.
- Test 6: `HumanizeAccessibilityBridgeTest.chainOrderIsStable` — chain built via factory with [Delay, BusinessHours, RateLimit] returns the interceptors in that exact order.

#### Tasks

1. Create `humanize/DispatchAction.kt` with the sealed class defined in §4. Include `ActionType` enum. No logic.

2. Create `humanize/ActionOutcome.kt` — enum with SUCCESS, MISS, ERROR, DROPPED_BUSINESS_HOURS, QUEUED_BUSINESS_HOURS, RATE_LIMITED, RETRIED. Each value has a boolean `isTerminal` flag (RATE_LIMITED and DROPPED_BUSINESS_HOURS are terminal — no retry).

3. Create `humanize/HumanizeInterceptor.kt` — the interface defined in §4, plus `ActionResult` data class.

4. Create `humanize/InterceptorChain.kt`:
   ```kotlin
   class InterceptorChain(
       private val interceptors: List<HumanizeInterceptor>,
       private val terminal: suspend (DispatchAction) -> ActionResult,
       private val config: HumanizeConfig
   ) {
       suspend fun proceed(action: DispatchAction): ActionResult =
           proceedFrom(0, action)

       private suspend fun proceedFrom(index: Int, action: DispatchAction): ActionResult {
           if (index >= interceptors.size) return terminal(action)
           val nextChain = object : HumanizeInterceptor.Chain {
               override val action = action
               override val config = this@InterceptorChain.config
               override suspend fun proceed(a: DispatchAction) = proceedFrom(index + 1, a)
           }
           return interceptors[index].intercept(nextChain)
       }
   }
   ```

5. Create `humanize/DelayInterceptor.kt`:
   ```kotlin
   class DelayInterceptor(private val random: Random = Random()) : HumanizeInterceptor {
       override suspend fun intercept(chain: HumanizeInterceptor.Chain): ActionResult {
           val profile = chain.config.delay_profiles[chain.action.actionType]
               ?: DelayProfile(mean_ms = 500, stddev_ms = 100)
           val sampled = (random.nextGaussian() * profile.stddev_ms + profile.mean_ms).toLong()
           val delayMs = sampled.coerceAtLeast(0L)
           delay(delayMs)
           return chain.proceed(chain.action)
       }
   }
   ```

6. Create `humanize/HumanizeConfig.kt` with the types defined in §4 plus `DeviceOverride` for per-device tweaks. Load default from `assets/humanize-defaults.json` bundled in APK.

7. Create `humanize/HumanizeAccessibilityBridge.kt`:
   ```kotlin
   class HumanizeAccessibilityBridge(
       private val underlying: AccessibilityBridge,
       private val auditLog: AuditLog,
       private val screenshotCapture: ScreenshotCapture,
       private val chainFactory: (HumanizeConfig) -> InterceptorChain,
       private val configProvider: (driverId: String) -> HumanizeConfig
   ) {
       // see §4 for full signature
       suspend fun tap(...) : ActionResult {
           val action = DispatchAction.Tap(driverId, appPackage, screen, selectorId, confidence)
           return dispatch(action)
       }
       private suspend fun dispatch(action: DispatchAction): ActionResult {
           val cfg = configProvider(action.driverId)
           val chain = chainFactory(cfg)
           val start = System.currentTimeMillis()
           val result = chain.proceed(action)
           val screenshotHash = screenshotCapture.captureOrSentinel(action.screen)
           val event = AuditEvent(
               ts_ms = start,
               agent_build_id = BuildConfig.GIT_HASH,
               driver_id = action.driverId,
               app_package = action.appPackage,
               screen = action.screen,
               selector_id = (action as? DispatchAction.Tap)?.selectorId ?: (action as? DispatchAction.TextInput)?.selectorId,
               selector_match_confidence = (action as? DispatchAction.Tap)?.selectorMatchConfidence,
               action_type = action.actionType,
               outcome = result.outcome,
               duration_ms = result.durationMs,
               screenshot_sha256 = screenshotHash,
               error_class = result.error?.javaClass?.simpleName,
               error_message = result.error?.message
           )
           auditLog.emit(event)
           return result
       }
   }
   ```
   Stub `AuditLog`, `ScreenshotCapture`, and `AccessibilityBridge` with no-op implementations in this plan (real implementations in 435-04, 435-06, and 430 respectively). Unit tests use `mockk` for all three.

8. Extend `driver/DriverContext.kt` to include `val dispatch: HumanizeAccessibilityBridge` (additive; existing `accessibility: AccessibilityBridge` field remains for now but will be deprecated in a subsequent refactor — grep assertion in 435-09 drill ensures no driver code uses `accessibility` directly).

9. Create `assets/humanize-defaults.json`:
   ```json
   {
     "delay_profiles": {
       "TAP":         {"mean_ms": 800,  "stddev_ms": 200},
       "SWIPE":       {"mean_ms": 1200, "stddev_ms": 300},
       "TEXT_INPUT":  {"mean_ms": 150,  "stddev_ms": 50},
       "SCREEN_READ": {"mean_ms": 100,  "stddev_ms": 30}
     }
   }
   ```
   HumanizeConfig.default() reads this at boot; per-driver manifest overrides take precedence.

10. Write unit tests listed in Behavior section. Use `kotlinx-coroutines-test` `runTest` with virtual time for the DelayInterceptor tests (otherwise 1000 samples x 800 ms = 13 minutes of real time).

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*InterceptorChainTest*' --tests '*DelayInterceptorTest*' --tests '*HumanizeAccessibilityBridgeTest*'` passes.
- `grep -rn "ctx.accessibility\.\(tap\|swipe\|textInput\)" rc-agent-mobile/app/src/main/` returns zero matches (drivers must use `ctx.dispatch.*`; this is the enforced rule).
- `./gradlew :app:assembleDebug` compiles.
- DriverContext now has `dispatch: HumanizeAccessibilityBridge` field alongside existing `accessibility`.

#### G4 NOT TESTED list (carry into commit)

- Actual delay (virtual time only; real-time behaviour deferred to 435-09 drill).
- Business-hours gate (435-02).
- Rate limiter (435-03).
- AuditLog.emit actually writing to disk (435-04).
- Screenshot capture (435-06).

#### Commit message

```
feat(435-01): HumanizeInterceptor chain + DispatchAction + DelayInterceptor

OkHttp-style interceptor chain as the only legal path for drivers to dispatch
UI actions.  DispatchAction sealed class covers TAP/SWIPE/TEXT_INPUT/SCREEN_READ.
InterceptorChain runs interceptors in order; DelayInterceptor injects
N(mean, stddev) delay per action type.  HumanizeAccessibilityBridge is the
driver-facing API; wraps 430 AccessibilityBridge and wires AuditLog +
ScreenshotCapture stubs.  humanize-defaults.json ships sensible defaults.

Covers: HUMANIZE-01, HUMANIZE-04 (config shape)
Not tested: business hours (435-02), rate limit (435-03), audit write (435-04),
            screenshot (435-06).
```

---

### 435-02-PLAN — BusinessHoursGate interceptor

**Goal:** A HumanizeInterceptor that evaluates the current time against a configurable IST window. Outside the window, applies the driver's policy — drop with log, or queue until window re-opens.

**Covers:** HUMANIZE-02

**Dependencies:** 435-01

**Type:** `auto` (TDD)

**TDD:** `tdd="true"`

#### Behavior (tests BEFORE code)

- Test 1: `BusinessHoursGateTest.insideWindowPassesThrough` — at 14:30 IST, interceptor calls `chain.proceed()` with no delay.
- Test 2: `BusinessHoursGateTest.outsideWindowDropWithLogReturnsDropped` — at 02:15 IST with policy DROP_WITH_LOG, returns `ActionResult(DROPPED_BUSINESS_HOURS)`, terminal NOT invoked.
- Test 3: `BusinessHoursGateTest.outsideWindowQueueDelaysUntilOpen` — at 23:45 IST with policy QUEUE_UNTIL_WINDOW, advance virtual clock to 08:00:05, assert `chain.proceed()` invoked exactly once.
- Test 4: `BusinessHoursGateTest.queueDelayedActionsDispatchInOrder` — queue 5 actions at 23:50, advance clock to 08:00, assert proceed called 5 times in insertion order.
- Test 5: `BusinessHoursGateTest.windowCrossingMidnightIsRejected` — config with `start_hour=22, end_hour=02` logs ERROR at config load and uses fallback 08:00-23:00. (v50.0 does not support midnight-crossing windows; documented in HUMANIZE.md.)
- Test 6: `BusinessHoursGateTest.timezoneDefault` — if config omits `timezone`, uses "Asia/Kolkata".
- Test 7: `BusinessHoursGateTest.disabledGatePassesThrough` — `enabled=false` makes the gate a no-op.

#### Tasks

1. Create `humanize/BusinessHoursGate.kt`:
   ```kotlin
   class BusinessHoursGate(
       private val clock: Clock = Clock.systemUTC(),
       private val scope: CoroutineScope
   ) : HumanizeInterceptor {
       private val queue = Channel<QueuedAction>(capacity = 256, onBufferOverflow = BufferOverflow.DROP_OLDEST)

       override suspend fun intercept(chain: HumanizeInterceptor.Chain): ActionResult {
           val cfg = chain.config.business_hours
           if (!cfg.enabled) return chain.proceed(chain.action)
           val now = LocalTime.now(clock.withZone(ZoneId.of(cfg.timezone)))
           val inside = isInsideWindow(now, cfg)
           if (inside) return chain.proceed(chain.action)
           return when (cfg.policy) {
               BusinessHoursConfig.Policy.DROP_WITH_LOG -> {
                   ActionResult(ActionOutcome.DROPPED_BUSINESS_HOURS, durationMs = 0)
               }
               BusinessHoursConfig.Policy.QUEUE_UNTIL_WINDOW -> {
                   // Return immediately with QUEUED; actual dispatch happens on window-open
                   val untilOpen = millisUntilNextWindowOpen(now, cfg)
                   scope.launch {
                       delay(untilOpen)
                       chain.proceed(chain.action)   // note: drops ActionResult (queued fire-and-forget); AuditLog emit still fires from caller
                   }
                   ActionResult(ActionOutcome.QUEUED_BUSINESS_HOURS, durationMs = 0)
               }
           }
       }

       // Pure functions — unit-testable without clock
       internal fun isInsideWindow(now: LocalTime, cfg: BusinessHoursConfig): Boolean { ... }
       internal fun millisUntilNextWindowOpen(now: LocalTime, cfg: BusinessHoursConfig): Long { ... }
   }
   ```

2. Midnight-crossing window rejection: validate at config-parse time in HumanizeConfig.load(). If `start_hour * 60 + start_minute >= end_hour * 60 + end_minute`, log ERROR and use fallback (8:00, 23:00). Documented in HUMANIZE.md.

3. Queue dispatch caveat: when a queued action fires at window-open, its AuditEvent will have a LATER `ts_ms` than the original queue-time. This is intentional (reflects actual dispatch time). However, we emit a second AuditEvent at queue-time with `outcome=QUEUED_BUSINESS_HOURS` and a specific `action_id` shared with the later SUCCESS event for correlation. `AuditEvent.kt` gains an optional `correlation_id: String?` field for this.

4. Write unit tests from Behavior section. Use a fixed `Clock.fixed(Instant, ZoneId)` for deterministic time. Use `TestScope` and `advanceTimeBy` for the queue tests.

5. Wire BusinessHoursGate into the default chain order in `HumanizeAccessibilityBridge`:
   ```kotlin
   val interceptors = listOf(
       DelayInterceptor(),          // first: applies human-like pause
       BusinessHoursGate(scope = scope),   // second: drops/queues outside hours
       RateLimiter()                // third: per-app ceiling — added in 435-03
   )
   ```

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*BusinessHoursGateTest*'` passes (7 tests).
- A driver test harness driver configured with `DROP_WITH_LOG` policy at 02:15 IST (faked clock) returns `ActionResult(DROPPED_BUSINESS_HOURS)` and AuditLog receives an event with that outcome.
- HUMANIZE.md documents the midnight-crossing restriction.

#### Commit message

```
feat(435-02): BusinessHoursGate interceptor with drop/queue policies

Configurable IST window (default 08:00-23:00).  DROP_WITH_LOG returns
immediately with DROPPED_BUSINESS_HOURS outcome.  QUEUE_UNTIL_WINDOW queues
the action on a coroutine, dispatches at window open.  Midnight-crossing
windows rejected with log + fallback.  AuditEvent gains correlation_id for
queued->dispatched correlation.

Covers: HUMANIZE-02
Not tested: rate limiting (435-03), durable queue across agent restart (documented as known gap).
```

---

### 435-03-PLAN — RateLimiter interceptor (token-bucket per driver_id + app_package)

**Goal:** Enforce max N actions per minute per `(driver_id, app_package)` tuple. Excess actions return `RATE_LIMITED` (drop policy) or `delay(until next token)` (queue policy — v50.0 ships drop only; queue is deferred to phase 441 scope).

**Covers:** HUMANIZE-03

**Dependencies:** 435-01

**Type:** `auto` (TDD)

**TDD:** `tdd="true"`

#### Behavior (tests BEFORE code)

- Test 1: `RateLimiterTest.underLimitPassesThrough` — 30 actions in 60s with limit=60, all succeed.
- Test 2: `RateLimiterTest.exactLimitReached` — 60 actions in 60s with limit=60, all succeed, 61st in same minute returns RATE_LIMITED.
- Test 3: `RateLimiterTest.refillAfterMinute` — exhaust bucket, advance virtual time 60s, bucket is full again.
- Test 4: `RateLimiterTest.bucketsAreIsolatedPerKey` — (zomato, com.zomato) exhausted does not affect (zomato, com.hyperpure) or (hyperpure, com.zomato).
- Test 5: `RateLimiterTest.concurrentAcquireIsSerialised` — 10 coroutines hit the same bucket simultaneously with limit=5, exactly 5 succeed, 5 return RATE_LIMITED, no race.
- Test 6: `RateLimiterTest.noLockAcrossAwait` — instruments `kotlinx-coroutines-debug`; asserts no coroutine suspension while holding the bucket Mutex.
- Test 7: `RateLimiterTest.disabledLimitPassesThrough` — `rate_limit_per_minute = 0` makes the gate a no-op.

#### Tasks

1. Create `humanize/RateLimiter.kt`:
   ```kotlin
   class RateLimiter(private val clock: Clock = Clock.systemUTC()) : HumanizeInterceptor {
       private data class BucketKey(val driverId: String, val appPackage: String)
       private class TokenBucket(
           val capacity: Int,
           val refillPerSecond: Double,   // capacity / 60
           var tokens: Double,
           var lastRefillMs: Long,
           val mutex: Mutex = Mutex()
       )
       private val buckets = ConcurrentHashMap<BucketKey, TokenBucket>()

       override suspend fun intercept(chain: HumanizeInterceptor.Chain): ActionResult {
           val limit = chain.config.rate_limit_per_minute
           if (limit <= 0) return chain.proceed(chain.action)
           val key = BucketKey(chain.action.driverId, chain.action.appPackage)
           val bucket = buckets.computeIfAbsent(key) {
               TokenBucket(
                   capacity = limit,
                   refillPerSecond = limit / 60.0,
                   tokens = limit.toDouble(),
                   lastRefillMs = clock.millis()
               )
           }
           val now = clock.millis()
           val acquired: Boolean = bucket.mutex.withLock {       // short critical section
               val elapsedSec = (now - bucket.lastRefillMs) / 1000.0
               bucket.tokens = minOf(bucket.capacity.toDouble(), bucket.tokens + elapsedSec * bucket.refillPerSecond)
               bucket.lastRefillMs = now
               if (bucket.tokens >= 1.0) { bucket.tokens -= 1.0; true } else false
           }
           // CRITICAL: do NOT call chain.proceed() inside the mutex — that would hold the lock across await.
           return if (acquired) chain.proceed(chain.action)
                  else ActionResult(ActionOutcome.RATE_LIMITED, durationMs = 0)
       }
   }
   ```

2. Note the explicit "do NOT hold mutex across chain.proceed()" comment — this is the CLAUDE.md "Never hold a lock across .await" rule applied to Kotlin coroutines. Enforced by test 6.

3. Per-driver config override: if a driver manifest has `rate_limit_per_minute: 30`, that overrides the global default. HumanizeConfig already carries this.

4. Hot-reload: RateLimiter reads `chain.config.rate_limit_per_minute` fresh on every intercept — no cached limits. If a push reconfigures a driver from 60 to 10 mid-session, the change takes effect on the next action. Bucket capacity adjusts; if tokens > new capacity, clamped down on next refill.

5. Unit tests: use `Clock.fixed` + `Clock.offset` + virtual time for the refill test. For the concurrency test, use `coroutineScope { repeat(10) { launch { rateLimiter.intercept(chain) } } }` with `runBlocking`.

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*RateLimiterTest*'` passes (7 tests).
- `kotlinx-coroutines-debug` assertion in test 6 confirms no lock-across-await.
- Integration: 120 actions/minute on a driver configured for 60 results in 60 SUCCESS + 60 RATE_LIMITED (verified via mock AuditLog in test).

#### Commit message

```
feat(435-03): token-bucket RateLimiter per (driver_id, app_package)

Per-bucket mutex guards acquire-check-decrement; lock never held across
chain.proceed() per CLAUDE.md rule.  Refill math: limit/60 tokens per second.
Disabled (limit <= 0) = no-op.  Hot-reload: config read fresh per intercept.

Covers: HUMANIZE-03
Not tested: durable bucket state across agent restart (ephemeral — reset on reboot; accepted for v50).
```

---

### 435-04-PLAN — AuditEvent + JsonlWriter (thread-safe, append-only)

**Goal:** Structured AuditEvent record plus a thread-safe JSONL writer that accepts events from any coroutine and writes them line-by-line to disk. Single-writer coroutine pattern to avoid file-lock contention; bounded channel (1024) with drop-oldest on overflow.

**Covers:** AUDIT-01

**Dependencies:** 435-01 (AuditEvent emitted from HumanizeAccessibilityBridge)

**Type:** `auto` (TDD)

**TDD:** `tdd="true"`

#### Behavior (tests BEFORE code)

- Test 1: `JsonlWriterTest.singleEventWritten` — write one AuditEvent, read file back, parse as JSON, assert all ten fields present.
- Test 2: `JsonlWriterTest.concurrentWritersAreSerialised` — 100 coroutines each write 10 events; file has exactly 1000 lines, each valid JSON.
- Test 3: `JsonlWriterTest.channelOverflowDropsOldest` — fill channel with 1025 events before consumer starts; assert oldest is dropped (first event missing from file).
- Test 4: `JsonlWriterTest.writerSurvivesIoError` — mock FileOutputStream throwing on one write; writer logs ERROR, continues on next write (does not crash the agent).
- Test 5: `JsonlWriterTest.jsonIsOneLinePerEvent` — asserts no newlines WITHIN a serialised event (text-input events can contain user-entered newlines — these must be escaped).
- Test 6: `AuditEventSerializationTest.schemaRoundTrip` — serialise + deserialise all ActionType × ActionOutcome combinations; assert bijection.

#### Tasks

1. Create `audit/AuditEvent.kt` — @Serializable data class per §4. Include `correlation_id: String? = null` (from 435-02) and `metadata: Map<String, String> = emptyMap()` for future extensibility.

2. Create `audit/JsonlWriter.kt`:
   ```kotlin
   class JsonlWriter(
       private val baseDir: File,
       private val fileNamePrefix: String = "audit",
       private val scope: CoroutineScope,
       private val rotationPolicy: RotationPolicy    // stub from 435-04; real impl in 435-05
   ) {
       private val json = Json { encodeDefaults = true; ignoreUnknownKeys = true }
       private val channel = Channel<AuditEvent>(
           capacity = 1024,
           onBufferOverflow = BufferOverflow.DROP_OLDEST
       )
       private var writerJob: Job? = null

       fun start() {
           writerJob = scope.launch(Dispatchers.IO) {
               for (event in channel) {
                   try {
                       rotationPolicy.checkAndRotateIfNeeded(baseDir, fileNamePrefix)
                       val line = json.encodeToString(AuditEvent.serializer(), event) + "\n"
                       val current = File(baseDir, "$fileNamePrefix-000.jsonl")
                       current.appendText(line, Charsets.UTF_8)
                   } catch (t: Throwable) {
                       Log.e("JsonlWriter", "write failed", t)
                   }
               }
           }
       }

       suspend fun write(event: AuditEvent) { channel.send(event) }
       fun stop() { channel.close(); writerJob?.cancel() }
       fun readRange(fromMs: Long, toMs: Long): Sequence<String> = /* read active + rotated files, filter by ts_ms */
   }
   ```

3. `AuditLog.kt` is the singleton façade:
   ```kotlin
   class AuditLogImpl(
       private val writer: JsonlWriter,
       private val rotation: RotationPolicy
   ) : AuditLog {
       override suspend fun emit(event: AuditEvent) = writer.write(event)
       override fun readRange(from: Long, to: Long) = writer.readRange(from, to)
   }
   ```
   Stub `RotationPolicy.checkAndRotateIfNeeded()` as no-op; real impl in 435-05.

4. Directory: `context.getExternalFilesDir("audit")`. Files: `audit-000.jsonl` (active), `audit-001.jsonl` (most-recent rotated), ..., `audit-050.jsonl` (oldest before deletion).

5. Text escaping: AuditEvent.error_message and TextInput.text can contain arbitrary characters including newlines. Kotlinx Json serialiser automatically escapes `\n` as `\\n`. Test 5 asserts this by writing an event with a literal newline in error_message and asserting the line count matches the event count.

6. Wire AuditLogImpl into the service-level singleton. `AgentForegroundService.onCreate` creates one AuditLogImpl and injects it into the HumanizeAccessibilityBridge factory. Only ONE AuditLogImpl exists process-wide — enforced by grep in 435-09 (`grep -rn "AuditLogImpl(" app/src/main/` returns at most one match outside the service).

7. HTTP endpoint on LocalHttpServer (same pattern as 429-07's `/logs/tail`): `/audit/tail?n=100` returns last N lines, protected by the dev service key. Helpful for on-site debugging without ADB.

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*JsonlWriterTest*' --tests '*AuditEventSerializationTest*'` passes (6 tests).
- After 100 actions through the interceptor chain, `adb shell cat /sdcard/Android/data/.../audit/audit-000.jsonl | wc -l` returns 100.
- `adb shell curl http://<device>:8090/audit/tail?n=10` (with dev key header) returns 10 valid JSONL lines.

#### Commit message

```
feat(435-04): AuditEvent schema + JsonlWriter single-writer coroutine

@Serializable AuditEvent with ten fields: ts_ms, agent_build_id, driver_id,
app_package, screen, selector_id, selector_match_confidence, action_type,
outcome, duration_ms, screenshot_sha256 + optional error_class/message,
correlation_id, metadata.  JsonlWriter uses bounded channel (1024, DROP_OLDEST)
+ single Dispatchers.IO coroutine to serialise writes.  HTTP /audit/tail for
on-site debug.

Covers: AUDIT-01
Not tested: rotation (435-05), shipping (435-07), screenshot hash real capture (435-06).
```

---

### 435-05-PLAN — RotationPolicy (10 MB × 50 files = 500 MB cap)

**Goal:** Rotate JsonlWriter's active file when it exceeds 10 MB; maintain at most 50 rotated files on disk; oldest-first eviction. Total on-disk cap: 500 MB + 10 MB active = ~510 MB worst case (documented).

**Covers:** AUDIT-02 (the rotation half — shipping half is 435-07)

**Dependencies:** 435-04

**Type:** `auto` (TDD)

**TDD:** `tdd="true"`

#### Behavior (tests BEFORE code)

- Test 1: `RotationPolicyTest.rotationAt10MB` — write 11 MB of synthetic lines; assert `audit-000.jsonl < 10 MB`, `audit-001.jsonl` exists and contains the pre-rotation data.
- Test 2: `RotationPolicyTest.keepsAt50Files` — simulate 55 rotations; assert `audit-050.jsonl` is deleted, `audit-000.jsonl` through `audit-049.jsonl` + one newly-rotated exist.
- Test 3: `RotationPolicyTest.totalCapAt500MB` — simulate writing 600 MB total; assert total disk usage ≤ 510 MB.
- Test 4: `RotationPolicyTest.rotationIsAtomic` — kill the writer mid-rotation (simulated via thrown IOException after rename); assert no data loss — all pre-rotation lines readable after recovery.
- Test 5: `RotationPolicyTest.rotateWhileReadingShippingCursor` — ShippingClient (435-07) is reading `audit-003.jsonl` while rotation wants to rename it. Assert rotation blocks until read lock releases (or read completes then rotation proceeds).

#### Tasks

1. Create `audit/RotationPolicy.kt`:
   ```kotlin
   class RotationPolicy(
       val maxBytesPerFile: Long = 10L * 1024 * 1024,
       val maxFiles: Int = 50                  // 50 rotated + 1 active = 51; rotated are 001..050
   ) {
       private val rotationMutex = Mutex()      // serialise rotations process-wide
       private val readWriteLock = ReentrantReadWriteLock()   // ShippingClient takes read; rotation takes write

       suspend fun checkAndRotateIfNeeded(baseDir: File, prefix: String) {
           val active = File(baseDir, "$prefix-000.jsonl")
           if (!active.exists() || active.length() < maxBytesPerFile) return
           rotationMutex.withLock {
               // Double-check under lock
               if (active.length() < maxBytesPerFile) return@withLock
               readWriteLock.writeLock().lock()   // exclusive during rename cascade
               try {
                   // Rename cascade: delete 050, rename 049 -> 050, ... rename 000 -> 001, create fresh 000
                   val oldest = File(baseDir, "$prefix-${String.format("%03d", maxFiles)}.jsonl")
                   if (oldest.exists()) oldest.delete()
                   for (i in (maxFiles - 1) downTo 0) {
                       val src = File(baseDir, "$prefix-${String.format("%03d", i)}.jsonl")
                       val dst = File(baseDir, "$prefix-${String.format("%03d", i + 1)}.jsonl")
                       if (src.exists()) src.renameTo(dst)
                   }
                   File(baseDir, "$prefix-000.jsonl").createNewFile()
               } finally {
                   readWriteLock.writeLock().unlock()
               }
           }
       }

       fun acquireReadLock(): Closeable { ... }   // for ShippingClient (435-07)
   }
   ```

2. The `ReentrantReadWriteLock` is a java.util.concurrent lock — reads can proceed concurrently; writes are exclusive. ShippingClient takes a read lock while it reads a file; rotation takes write lock before renaming. This prevents ShippingClient from reading a half-renamed file.

3. Async caveat: `readWriteLock.readLock().lock()` is blocking, not suspending. ShippingClient calls this inside `withContext(Dispatchers.IO)` so the main coroutine is not blocked. Document in AUDIT-LOG.md.

4. Filename format: `audit-NNN.jsonl` with NNN zero-padded to 3 digits. 000 is active, 001..050 are rotated.

5. Failure handling: if `renameTo()` returns false (Android sometimes disallows cross-device renames even within external storage), fall back to `copyTo(dst, overwrite=true)` + `delete()`. Log WARN on fallback.

6. Unit tests from Behavior section. Use `@TempDir` JUnit 4 extension (or manual tempdir) to isolate tests.

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*RotationPolicyTest*'` passes (5 tests).
- Under stress (1 MB/s sustained write for 15 minutes via mock driver), total disk usage measured via `adb shell du -k /sdcard/Android/data/.../audit/` stays ≤ 512 MB.

#### Commit message

```
feat(435-05): RotationPolicy — 10MB x 50 files, 500MB cap, oldest-first

Rotation cascade: 050 deleted, 049 -> 050, ..., 000 -> 001, fresh 000.
ReentrantReadWriteLock coordinates with ShippingClient reads.  Mutex guards
rotation atomicity; renameTo fallback to copy+delete on cross-device issues.

Covers: AUDIT-02 (rotation)
Not tested: shipping (435-07), actual disk exhaustion (deferred to 435-09 drill).
```

---

### 435-06-PLAN — ScreenshotCapture + SHA256 + FLAG_SECURE fallback

**Goal:** Capture the current screen at action time, compute SHA256 of the image bytes, return as hex string. When FLAG_SECURE is set (login screens commonly do this), return the sentinel `sha256:unavailable:flag_secure`. When API < 30 (pre-Android 11), return `sha256:unavailable:api_too_low`. When capture times out or throws, return `sha256:unavailable:error`.

**Covers:** AUDIT-01 (screenshot hash requirement)

**Dependencies:** Phase 430 AccessibilityService bridge

**Type:** `auto` (TDD for pure functions; instrumented test for AccessibilityService path)

**TDD:** `tdd="true"` (for hashing + decision logic; AccessibilityService.takeScreenshot itself is mocked)

#### Behavior (tests BEFORE code)

- Test 1: `ScreenshotCaptureTest.sha256OfKnownBytesMatches` — feed a fixed byte array; assert hex matches known SHA256.
- Test 2: `ScreenshotCaptureTest.flagSecureReturnsSentinel` — mock AccessibilityService; simulate FLAG_SECURE blocking by throwing `SecurityException`; assert return is `sha256:unavailable:flag_secure`.
- Test 3: `ScreenshotCaptureTest.apiBelow30ReturnsSentinel` — mock Build.VERSION.SDK_INT = 29; assert return is `sha256:unavailable:api_too_low` without calling takeScreenshot.
- Test 4: `ScreenshotCaptureTest.timeoutReturnsSentinel` — mock takeScreenshot to hang > 500 ms; assert return is `sha256:unavailable:timeout`.
- Test 5: `ScreenshotCaptureTest.genericErrorReturnsSentinel` — mock takeScreenshot to throw RuntimeException; assert return is `sha256:unavailable:error`.
- Test 6: `ScreenshotCaptureTest.successfulCaptureReturnsHex` — mock takeScreenshot to return a ScreenshotResult with 100 bytes of 0xAA; assert return is 64-hex-char SHA256.

#### Tasks

1. Create `audit/ScreenshotCapture.kt`:
   ```kotlin
   class ScreenshotCapture(
       private val accessibilityService: AccessibilityService,  // singleton from 430
       private val apiLevel: Int = Build.VERSION.SDK_INT,
       private val clock: Clock = Clock.systemUTC(),
       private val timeoutMs: Long = 500L
   ) {
       suspend fun captureOrSentinel(screen: String): String {
           if (apiLevel < 30) return "sha256:unavailable:api_too_low"
           return try {
               withTimeout(timeoutMs) {
                   val bytes = captureBytes()
                   computeSha256(bytes)
               }
           } catch (e: TimeoutCancellationException) { "sha256:unavailable:timeout" }
             catch (e: SecurityException) { "sha256:unavailable:flag_secure" }
             catch (e: Throwable) {
                 Log.w("ScreenshotCapture", "capture failed: ${e.message}")
                 "sha256:unavailable:error"
             }
       }

       private suspend fun captureBytes(): ByteArray = suspendCancellableCoroutine { cont ->
           accessibilityService.takeScreenshot(
               Display.DEFAULT_DISPLAY,
               Executors.newSingleThreadExecutor(),
               object : AccessibilityService.TakeScreenshotCallback {
                   override fun onSuccess(result: AccessibilityService.ScreenshotResult) {
                       val hwBitmap = Bitmap.wrapHardwareBuffer(result.hardwareBuffer, result.colorSpace)
                       val stream = ByteArrayOutputStream()
                       hwBitmap?.compress(Bitmap.CompressFormat.PNG, 100, stream)
                       cont.resume(stream.toByteArray())
                   }
                   override fun onFailure(errorCode: Int) {
                       cont.resumeWithException(RuntimeException("takeScreenshot failed: $errorCode"))
                   }
               }
           )
       }

       internal fun computeSha256(bytes: ByteArray): String {
           val digest = MessageDigest.getInstance("SHA-256").digest(bytes)
           return digest.joinToString("") { "%02x".format(it) }
       }
   }
   ```

2. Screenshot timing: **screenshots are taken AFTER the action dispatch, not before**, for these reasons:
   - (a) AccessibilityService.takeScreenshot and tap dispatch contend for the same AccessibilityService thread; doing both in parallel serialises anyway with ~200 ms added latency.
   - (b) For audit purposes, the POST-action screen is more informative (did the tap actually change the UI?).
   - (c) For FLAG_SECURE apps, BOTH pre and post will fail identically — no information lost.
   - Exception: on `ActionOutcome.MISS` (selector not found), the POST-state shows what WAS on screen instead of what was expected. This is the forensic value of the screenshot.
   - Documented in AUDIT-LOG.md under "screenshot timing rationale".

3. Performance: takeScreenshot blocks for ~150-300 ms on real devices. With default 800 ms TAP delay + 200 ms screenshot = 1 second per action worst case. This is acceptable for reception-floor throughput (< 1 Hz typical) but documented as a ceiling in HUMANIZE.md for future high-frequency drivers.

4. The hash is computed on the PNG-compressed bytes, not the raw pixels. Two identical screens produce identical hashes only if PNG compression is deterministic (Android's Bitmap.compress(PNG, 100) IS deterministic — verified in Android docs). Test 6 asserts this.

5. Unit tests mock AccessibilityService using `mockk`. Instrumented test (skipped on CI, run manually): real capture on Tab Plus + verify hash is valid hex.

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*ScreenshotCaptureTest*'` passes (6 tests).
- Manual smoke test on Tab Plus: open a non-FLAG_SECURE app (Calculator), trigger a mock tap, AuditEvent's screenshot_sha256 is 64 hex chars.
- Manual smoke test on Zomato Partner login screen (FLAG_SECURE): screenshot_sha256 is `sha256:unavailable:flag_secure`.

#### G4 NOT TESTED list

- Real FLAG_SECURE behaviour in Zomato (depends on their current app build; verified in 435-09 drill).
- Screenshot deterministic-hash assumption across OEMs (Android Bitmap PNG compression might vary; if 435-09 finds drift, switch to raw-pixel SHA256 in a follow-up).

#### Commit message

```
feat(435-06): ScreenshotCapture with SHA256 + FLAG_SECURE/API/timeout sentinels

AccessibilityService.takeScreenshot (API 30+) wrapped in suspendCancellable
coroutine with 500ms timeout.  Sentinels for api_too_low, flag_secure,
timeout, error.  Screenshots taken POST-action per design rationale in
AUDIT-LOG.md.  SHA256 computed over PNG-compressed bytes (deterministic).

Covers: AUDIT-01 (screenshot hash requirement)
Not tested: cross-OEM PNG-compression determinism (follow-up if 435-09 drift).
```

---

### 435-07-PLAN — HourlyShippingClient (batch + POST + retry-with-backoff + cursor)

**Goal:** Every hour, agent ships the last hour's unsent AuditEvents to the server via comms-link relay. Uses a persistent ShippedCursor `(file_name, byte_offset)` to track progress. Retries with exponential backoff on HTTP failure. Never loses events unless the 500 MB rotation eats them first.

**Covers:** AUDIT-02 (shipping half)

**Dependencies:** 435-04, 435-05, 429 (comms-link connection)

**Type:** `auto` (TDD for cursor + batch logic; instrumented for real POST path)

**TDD:** `tdd="true"` (for cursor + batch + retry; real POST is integration-tested in 435-09)

#### Behavior (tests BEFORE code)

- Test 1: `ShippingClientTest.firstTickShipsAll` — ShippedCursor is fresh; 500 events on disk; tick fires; assert POST body contains all 500 events in order.
- Test 2: `ShippingClientTest.subsequentTickShipsDelta` — cursor at (audit-000, 1000); 100 new events written; tick fires; assert POST body contains only the 100 delta.
- Test 3: `ShippingClientTest.cursorAdvancesOnly200` — mock POST returns 500; cursor unchanged; next tick re-ships the same batch.
- Test 4: `ShippingClientTest.exponentialBackoffOn5xx` — mock POST returns 500; retry attempts at 1s, 2s, 4s, 8s, ..., capped at 300s.
- Test 5: `ShippingClientTest.batchSizeCapAt5MB` — 10k events generating > 5 MB of JSONL; assert the client splits into multiple POST calls, each ≤ 5 MB body.
- Test 6: `ShippingClientTest.cursorSurvivesRestart` — write cursor, simulate process kill, re-instantiate client, assert cursor is read back correctly.
- Test 7: `ShippingClientTest.rotationDuringShipDoesNotLoseEvents` — while ShippingClient reads audit-003, RotationPolicy wants to rename files. Assert ReentrantReadWriteLock blocks rotation until read completes; no events dropped.
- Test 8: `ShippingClientTest.shippedCursorJumpsAcrossRotation` — cursor at (audit-000, X); rotation happens (000 → 001); assert cursor is updated to (audit-001, X) via a rename-aware update path.

#### Tasks

1. Create `audit/ShippedCursor.kt`:
   ```kotlin
   @Serializable
   data class ShippedCursor(val fileName: String, val byteOffset: Long)

   class ShippedCursorStore(private val prefs: EncryptedSharedPreferences) {
       fun load(): ShippedCursor { ... }
       fun save(c: ShippedCursor) { ... }
   }
   ```
   Stored in EncryptedSharedPreferences for tamper resistance (per CLAUDE.md credential guidance).

2. Create `audit/ShippingClient.kt`:
   ```kotlin
   class ShippingClient(
       private val baseDir: File,
       private val cursorStore: ShippedCursorStore,
       private val rotationPolicy: RotationPolicy,
       private val httpClient: OkHttpClient,              // injected from CommsLinkClient or standalone
       private val ingestUrl: String,                      // "http://relay/api/v1/mobile-audit/ingest"
       private val agentBuildId: String,
       private val deviceId: String,
       private val scope: CoroutineScope,
       private val clock: Clock = Clock.systemUTC()
   ) {
       private val maxBatchBytes = 5L * 1024 * 1024
       private var job: Job? = null

       fun start() {
           job = scope.launch {
               while (isActive) {
                   try { tick() }
                   catch (t: Throwable) { Log.e("Shipping", "tick failed", t) }
                   delay(60L * 60L * 1000L)    // 1 hour
               }
           }
       }

       internal suspend fun tick() {
           val readLock = rotationPolicy.acquireReadLock()
           try {
               var cursor = cursorStore.load()
               while (true) {
                   val batch = readBatchFromCursor(cursor, maxBatchBytes) ?: break
                   if (batch.lines.isEmpty()) break
                   val ok = postWithRetry(batch.lines)
                   if (!ok) break    // keep cursor unchanged; retry next tick
                   cursor = batch.nextCursor
                   cursorStore.save(cursor)
               }
           } finally { readLock.close() }
       }

       private suspend fun postWithRetry(lines: List<String>): Boolean {
           var attempt = 0
           while (attempt < 6) {
               try {
                   val body = buildBatchJson(lines)
                   val req = Request.Builder().url(ingestUrl).post(body).build()
                   val resp = httpClient.newCall(req).execute()
                   if (resp.code in 200..299) return true
                   Log.w("Shipping", "POST returned ${resp.code}; retry=$attempt")
               } catch (t: Throwable) { Log.w("Shipping", "POST failed: ${t.message}") }
               val backoffMs = minOf(300_000L, 1000L * (1L shl attempt)) + Random.nextLong(0, 500)
               delay(backoffMs)
               attempt++
           }
           return false
       }
       // readBatchFromCursor + buildBatchJson helpers
   }
   ```

3. Cursor rename-handling: when RotationPolicy renames `audit-000.jsonl` to `audit-001.jsonl`, any cursor pointing at `audit-000.jsonl` is stale. RotationPolicy fires a callback (`onRotated(oldName, newName)`) that ShippingClient subscribes to; cursor is rewritten atomically. Test 8 enforces.

4. Batch body format (to server):
   ```json
   {
     "device_id": "rcm-tab-plus",
     "agent_build_id": "abc1234",
     "event_count": 500,
     "events_jsonl": "<newline-delimited AuditEvent JSON lines>"
   }
   ```
   Content-Type: `application/json`. `events_jsonl` is a single JSON string containing JSONL (yes, this double-quotes the JSONL — simplifies the stub handler; Phase 441 will switch to `Content-Type: application/x-ndjson` for efficiency).

5. Batch size cap: 5 MB per POST. Events that would exceed this are split across multiple POSTs within the same tick. Cursor advances after each successful POST.

6. Ingest URL resolution:
   - Primary: `http://192.168.31.27:8766/relay/forward?dest=audit` → comms-link relay on James → forwards to racecontrol `POST /api/v1/mobile-audit/ingest`.
   - OR direct if on venue LAN: `http://192.168.31.23:8080/api/v1/mobile-audit/ingest`.
   - Configurable in HumanizeConfig.ingest_url_override; default is the relay forward path.

7. Unit tests from Behavior section. Use `MockWebServer` (from OkHttp) for the POST tests. Use virtual time for backoff tests.

8. Failure semantics: if shipping fails for 24 consecutive hours AND on-device log fills to 500 MB, rotation WILL drop un-shipped events. This is a ToS-accountability gap. Logged as a documented limitation in AUDIT-LOG.md; mitigation is Phase 441 (real server storage + admin alert on shipping stalls).

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*ShippingClientTest*'` passes (8 tests).
- Manual smoke on Tab Plus (with 435-08 stub endpoint live): wait 1 hour + 1 minute, observe server log for `mobile_audit_batch_received` with event count matching the on-device `audit-*.jsonl | wc -l`.

#### Commit message

```
feat(435-07): hourly ShippingClient with cursor + retry-backoff

Hourly ticker reads from ShippedCursor, batches up to 5 MB per POST, retries
with exponential backoff (1s..300s) on non-2xx.  EncryptedSharedPreferences
stores cursor across restart.  Rename-aware cursor updates on rotation.
Relay-forward path as default, direct LAN path as fallback.

Covers: AUDIT-02 (shipping)
Known gap: 24h+ shipping stall + 500MB cap = event loss; Phase 441 mitigates.
Not tested: 24h sustained drift (accepted; Phase 441 will add server-side drift alarm).
```

---

### 435-08-PLAN — Server-side stub ingest endpoint

**Goal:** `POST /api/v1/mobile-audit/ingest` on the racecontrol server accepts the batch payload defined in 435-07 and returns 200 with no storage. Logs event count at INFO. Phase 441 will replace this stub with real SQLite persistence + drift detection.

**Covers:** AUDIT-02 (the "shipping succeeds" half) — the partial stub part of the cross-system pipeline

**Dependencies:** none within phase; serves 435-07

**Type:** `auto`

#### Tasks

1. Create `crates/racecontrol/src/api/mobile_audit.rs`:
   ```rust
   use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
   use serde::Deserialize;
   use serde_json::json;

   #[derive(Deserialize)]
   pub struct MobileAuditBatch {
       pub device_id: String,
       pub agent_build_id: String,
       pub event_count: usize,
       pub events_jsonl: String,
   }

   pub async fn ingest_handler(
       State(state): State<AppState>,
       Json(batch): Json<MobileAuditBatch>,
   ) -> impl IntoResponse {
       // STUB implementation.  Phase 441 will insert into SQLite.
       tracing::info!(
           target = "mobile_audit",
           device_id = %batch.device_id,
           agent_build_id = %batch.agent_build_id,
           event_count = batch.event_count,
           body_bytes = batch.events_jsonl.len(),
           "mobile_audit_batch_received"
       );
       // Light validation: ensure events_jsonl line count matches event_count.
       let actual = batch.events_jsonl.lines().count();
       if actual != batch.event_count {
           tracing::warn!(
               declared = batch.event_count,
               actual,
               "mobile_audit_event_count_mismatch"
           );
       }
       (
           StatusCode::OK,
           Json(json!({
               "accepted": batch.event_count,
               "storage": "stub",
               "note": "Phase 441 will add persistent storage"
           }))
       )
   }
   ```

2. Register the route:
   - In `crates/racecontrol/src/api/mod.rs`: `pub mod mobile_audit;`
   - In `crates/racecontrol/src/api/routes.rs`: add `.route("/api/v1/mobile-audit/ingest", post(mobile_audit::ingest_handler))` to the authenticated service-key router (same middleware as `/api/v1/fleet/exec` — PSK in `X-Service-Key` header).
   - Rationale: rc-agent-mobile is trusted (same class as rc-agent). Incoming POST must carry the service key that only racecontrol-authorised agents hold. The comms-link relay forwards this key transparently.

3. Body size limit: Axum default is 2 MB. For 5 MB batches, increase the route-specific limit:
   ```rust
   .route("/api/v1/mobile-audit/ingest", post(mobile_audit::ingest_handler))
       .layer(DefaultBodyLimit::max(8 * 1024 * 1024))    // 8 MB
   ```

4. Security-check extension: `comms-link/test/security-check.js` must learn that `/api/v1/mobile-audit/ingest` is an authenticated route (service key required). Add one new assertion to the per-route coverage map.

5. Configuration: add a `[mobile_audit]` section to `racecontrol.toml`:
   ```toml
   [mobile_audit]
   stub_mode = true      # Phase 441 flips to false when storage is added
   max_batch_bytes = 5242880
   ```
   Optional; hardcoded defaults take over if section absent.

6. Comms-link relay forwarder — create `comms-link/james/mobile-audit-forward.js`:
   - Handles `POST /relay/forward?dest=audit` from rc-agent-mobile.
   - Injects service key from env (`RELAY_SERVICE_KEY`) into `X-Service-Key` header.
   - Forwards to `http://192.168.31.23:8080/api/v1/mobile-audit/ingest`.
   - DEPLOY PARITY: same forwarder shipped to Bono VPS relay, destination `http://localhost:8080/api/v1/mobile-audit/ingest` (cloud racecontrol).

7. Unit tests (Rust): `mobile_audit::tests::{batch_accepted_returns_200, mismatched_count_logs_warning, oversized_body_returns_413}`.

8. Integration test: `curl -X POST -H "X-Service-Key: $KEY" -H "Content-Type: application/json" -d '{"device_id":"rcm-test","agent_build_id":"abc","event_count":1,"events_jsonl":"{\"ts_ms\":1,\"outcome\":\"SUCCESS\"}"}' http://192.168.31.23:8080/api/v1/mobile-audit/ingest` returns 200 with `{"accepted":1,"storage":"stub"}`.

#### Acceptance

- `cargo test -p racecontrol-crate --test mobile_audit_tests` passes.
- Deploy racecontrol to server .23 + Bono VPS (DEPLOY PARITY).
- `curl` integration test passes on both venue and cloud.
- Server log shows `mobile_audit_batch_received` at INFO level.
- `comms-link/test/security-check.js` passes with the new route assertion.

#### Commit message

```
feat(435-08): server stub POST /api/v1/mobile-audit/ingest (Phase 441 will flesh out)

Axum handler accepts MobileAuditBatch, validates event_count vs actual line
count, logs INFO, returns 200.  No storage — Phase 441.  Service-key
authenticated.  8 MB body limit.  Relay forwarder shipped to James .27 +
Bono VPS (DEPLOY PARITY).  Security-check assertion added.

Covers: AUDIT-02 (shipping endpoint stub)
Not tested: Phase 441 persistence (out of scope for 435).
```

---

### 435-09-PLAN — Unit test roll-up + Tab Plus integration drill (1000 mock actions)

**Goal:** All unit test suites pass locally on CI. Then on Tab Plus, a mock driver dispatches 1000 actions over 10 minutes; verify end-to-end that every action is delayed, business-hours-gated, rate-limited, audited, screenshot-hashed, rotated, and shipped.

**Covers:** ship gate for Phase 435 — verification-only

**Dependencies:** 435-01..08

**Type:** `checkpoint:human-verify` (physical Tab Plus + live relay + live racecontrol)

#### Preconditions

- Tab Plus has rc-agent-mobile APK built from 435-08 commit.
- Tab Plus has AccessibilityService enabled (Phase 430 requirement).
- comms-link relay on James .27 + Bono VPS is forwarding the new `/api/v1/mobile-audit/ingest` path.
- racecontrol server on .23 has the stub endpoint live.

#### Drill script

1. Install fresh APK on Tab Plus: `adb install -r app-release.apk`.
2. Deploy a test driver `MockHighVolumeDriver` (bundled in the APK behind a BuildConfig flag):
   - `driver_id = "mock-high-volume"`
   - `target_package = "com.android.settings"` (harmless target — tapping Settings is reversible)
   - `rate_limit_per_minute = 60`
   - `delay_profiles = {TAP: (mean=200, stddev=50)}` (accelerated for drill)
   - `business_hours = enabled=false` (drill runs at any time)
3. Enable the driver via local intent: `adb shell am broadcast -a in.racingpoint.rcagentmobile.ACTION_TOGGLE_DRIVER --es driver_id "mock-high-volume" --ez enabled true`.
4. Run the drill harness (new instrumented test class `MockDriverDrill`):
   - For 10 minutes, the driver dispatches `TAP` on a Settings button at 100 actions/minute. With `rate_limit_per_minute = 60`, expected outcome: 60 SUCCESS/minute + 40 RATE_LIMITED/minute = 1000 total events over 10 minutes.
5. Collect evidence:
   - `adb shell cat /sdcard/Android/data/in.racingpoint.rcagentmobile/files/audit/audit-*.jsonl | wc -l` → expect ~1000.
   - `adb shell cat /sdcard/Android/data/.../audit/audit-*.jsonl | jq -r .outcome | sort | uniq -c` → expect ~600 SUCCESS + ~400 RATE_LIMITED.
   - `adb shell du -k /sdcard/Android/data/.../audit/` → expect < 50 MB (well below the cap).
   - Server log: `grep mobile_audit_batch_received` on server .23 for the 10 minutes → expect at least one batch shipped (the hourly tick fires once; for the drill we FORCE one immediately via a debug intent `adb shell am broadcast -a in.racingpoint.rcagentmobile.DEBUG_SHIP_NOW`).
6. Business-hours gate drill (separate 5-minute test):
   - Reconfigure MockHighVolumeDriver with `business_hours = {enabled=true, start_hour=23, end_hour=23, end_minute=55, policy=DROP_WITH_LOG}` — a 0-minute window effectively closed.
   - Dispatch 100 actions.
   - Assert all 100 AuditEvents have `outcome="dropped_business_hours"`.
   - Reconfigure to `policy=QUEUE_UNTIL_WINDOW` and window `00:00-23:59` — 100 actions queued, then dispatched; assert 200 events (100 queued + 100 eventually-dispatched with matching correlation_id).
7. FLAG_SECURE drill: launch Zomato Partner manually, navigate to its login screen, drive a tap via debug intent; assert AuditEvent has `screenshot_sha256 = "sha256:unavailable:flag_secure"`.

#### Acceptance (all four success criteria)

- [ ] SC-1 (AUDIT-01): 1000 AuditEvents written, all with 10+ fields present, JSON-parseable.
- [ ] SC-2 (HUMANIZE-02): drop drill = 100/100 dropped; queue drill = 100 QUEUED + 100 matched SUCCESS with correlation_id.
- [ ] SC-3 (HUMANIZE-03): ~60 SUCCESS/min + ~40 RATE_LIMITED/min sustained over 10 min.
- [ ] SC-4 (AUDIT-02): server received at least one batch via debug-ship; cursor advanced; re-ship idempotent.

#### Artifacts to save in SUMMARY.md

- `drill-logs-tab-plus/audit-*.jsonl` (first + last 100 lines as evidence)
- Server log excerpt showing `mobile_audit_batch_received`
- Screenshot of admin dashboard — even though Phase 441 is not done, the 500-MB cap + rotation math can be visualised via `adb shell stat` output captured into SUMMARY.md
- Grep check: `grep -rn "ctx.accessibility\." rc-agent-mobile/app/src/main/` returns ZERO matches (all drivers use `ctx.dispatch`).

#### Checkpoint (human-verify)

User runs the drill script on Tab Plus + asserts each SC with numeric measurements. If any SC fails, create a gap-closure plan — do NOT mark Phase 435 complete.

#### Commit message

```
test(435-09): Phase 435 E2E drill — 1000 mock actions + business-hours + rate-limit + FLAG_SECURE

All four ROADMAP Phase 7 success criteria exercised on Tab Plus.  Evidence in
SUMMARY.md.  Grep check enforces that no driver bypasses the interceptor chain.

Covers: full Phase 435 acceptance gate.
```

---

## 6. Risks and pitfalls

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **Screenshot timing adds ~200 ms/action** | Take AFTER dispatch; accept worst-case ~1 s/action for reception throughput (< 1 Hz typical).  Documented ceiling. |
| R-2 | **Token-bucket race** on shared ConcurrentHashMap | Per-bucket Mutex + never-lock-across-await; unit test `RateLimiterTest.noLockAcrossAwait` with kotlinx-coroutines-debug. |
| R-3 | **FLAG_SECURE blocks screen capture** on login screens | Sentinel hash; audit trail remains complete on non-visual fields.  Zomato Partner session screens are non-FLAG_SECURE (verified during 435-06 smoke). |
| R-4 | **IST hardcoded** for business-hours | Configurable via `BusinessHoursConfig.timezone`; default IST; documented in HUMANIZE.md. |
| R-5 | **Shipping endpoint is a stub** — data goes to /dev/null for v50 | WARN banner in Phase 441 admin dashboard; 500 MB local buffer absorbs ~2 weeks of typical load. |
| R-6 | **JsonlWriter channel overflow** drops events under burst | Bounded channel DROP_OLDEST + back-pressure via DelayInterceptor; ERROR log emitted on drop. |
| R-7 | **Rotation during shipping read** could corrupt cursor | ReentrantReadWriteLock + rename-aware cursor update; Test 8 enforces. |
| R-8 | **Queued actions lost on agent restart** | Queue is in-memory; v50.0 accepts this (business hours apply to future actions, not backlog).  Phase 441 may add durable queue. |
| R-9 | **AccessibilityService.takeScreenshot unavailable on API < 30** | Sentinel `sha256:unavailable:api_too_low`; both target devices are API 33+ so no impact. |
| R-10 | **DriverContext.accessibility vs DriverContext.dispatch coexistence** tempts drivers to skip the chain | Grep check in 435-09 drill enforces zero `.accessibility.tap/swipe/textInput` calls.  Phase 437 (Zomato) MUST use `ctx.dispatch.*` exclusively. |
| R-11 | **Cross-OEM PNG-compression determinism** for SHA256 | If 435-09 detects drift, swap to raw-pixel SHA256 in a follow-up plan; not blocking for ship. |
| R-12 | **Battery drain from hourly ShippingClient** | 5 MB POST once/hour is negligible (~50 KB/min average); foreground service is already in the Doze whitelist from 429-05. |
| R-13 | **Privacy (audit log contents)** | Audit log records selector IDs + screens + action types.  TextInput.text MUST NOT be logged as-is for credential fields — add a `redact: Boolean` flag on TextInput (default false; drivers set true for password fields).  Documented in AUDIT-LOG.md.  Phase 439 (Zomato credentials) will set redact=true during login. |
| R-14 | **Server stub silently discarding data** masks shipping bugs | 435-08 handler validates `event_count` vs actual line count; logs WARN on mismatch.  Phase 441 will add a 24-hour "no events received" alarm. |

## 7. Test plan

### Unit tests (JVM, fast, on every build)

- `InterceptorChainTest` (435-01) — 3 tests
- `DelayInterceptorTest` (435-01) — 2 tests
- `HumanizeAccessibilityBridgeTest` (435-01) — 1 test
- `BusinessHoursGateTest` (435-02) — 7 tests
- `RateLimiterTest` (435-03) — 7 tests
- `JsonlWriterTest` (435-04) — 4 tests
- `AuditEventSerializationTest` (435-04) — 1 test
- `RotationPolicyTest` (435-05) — 5 tests
- `ScreenshotCaptureTest` (435-06) — 6 tests
- `ShippingClientTest` (435-07) — 8 tests
- Rust: `mobile_audit::tests` (435-08) — 3 tests

Total: **47 unit tests** across 11 suites. All run under `./gradlew :app:testDebugUnitTest` + `cargo test -p racecontrol-crate`.

### Instrumented tests (manual run before release)

- `HumanizeAccessibilityBridgeInstrumentedTest` — dispatches 10 real taps on Calculator app; verify AuditLog contains 10 events with valid screenshot hashes.
- `MockHighVolumeDriverDrill` — used in 435-09 for the 1000-action drill.

### Physical device tests (human-verify in 435-09)

- 1000 mock actions on Tab Plus
- Business-hours drop + queue drills
- FLAG_SECURE drill on Zomato Partner login screen

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** Interceptor chain, business-hours window math, token-bucket math, rotation cascade, shipping cursor — all business logic with defined I/O. Run `gsd-nyquist-auditor` BEFORE 435-09 lands.
- **MMA audit (required — cross-system bridge AND ToS-risk mitigation):** Kotlin interceptor chain → JSONL writer → comms-link relay → Rust stub is a 3-language bridge. HUMANIZE is THE ToS mitigation for Zomato/HyperPure/Blinkit per PROJECT.md risk register. **Dual reasoning modes REQUIRED** (abstract for architecture + trace-level for token-bucket race / rotation atomicity). Budget: $5. Run BEFORE Phase 435 ship gate. Expected findings: race conditions, lock-across-await, cursor rename corner cases, screenshot timing invariants.
- **integration-checker (required):** Must verify that 437 (Zomato), 438 (HyperPure), 439 (Blinkit) can call `ctx.dispatch.*` without bypassing the chain. Run before milestone ship.
- **codebase-mapper (skip):** 435 extends existing rc-agent-mobile module; no new top-level directory. Runtime-verified in 435-09 drill.
- **ui-researcher / ui-auditor (skip):** No user-facing UI in 435. Admin dashboard log viewer = Phase 441.
- **SEC gate (required):** `node comms-link/test/security-check.js` must pass after 435-08 adds `/api/v1/mobile-audit/ingest` route. Gate asserts the route is service-key authenticated.
- **Deploy Manifest Protocol (DMP):** Already captured in frontmatter `deploy:` section. Executor must tick each item; verifier confirms deployed state.
- **Backlog gate (CLAUDE.md CGP v4.3):** Phase 435 must reach DEPLOYED-VERIFIED (APK on both devices + 435-09 drill passed + server stub live on venue + cloud) before Phase 436 begins.

## 9. Open questions the planner cannot decide

**OQ-1 — Is a stub ingest endpoint on Bono VPS OK for 435, or should real shipping defer entirely to Phase 441?**
Plan 435-08 ships a stub that returns 200 with no storage. Drills in 435-09 will exercise the full shipping path end-to-end, but events are discarded server-side. The alternative is deferring ALL shipping code (435-07 + 435-08) to Phase 441. **Planner recommendation:** ship the stub now. Rationale:
- (a) 435-07 shipping client is non-trivial (cursor, retry, rotation coordination) and benefits from being exercised against a real endpoint immediately.
- (b) The stub's `event_count` validation catches "events lost in flight" bugs even without real storage.
- (c) Phase 441 can replace the stub in a single commit without re-testing the agent side.
- (d) The 500 MB local buffer absorbs 24h+ of outage; no data loss in normal operation.
If user disagrees, defer 435-07 + 435-08 to Phase 441 and ship 435-01..06 + 435-09 (drill replaced with on-device-only verification).

**OQ-2 — Should humanize-defaults.json be bundled in APK or fetched from comms-link at first-run?**
Current plan: bundled in APK. Alternative: fetched on first launch from comms-link (similar to rc-agent's allowlist fetch pattern). Bundling is simpler but means a config update requires APK redeploy. Per CLAUDE.md "no hardcoded config" rule (spirit), runtime fetch is preferred — but Phase 436 (feature flags) already handles the runtime-update path. **Planner recommendation:** bundle as a fallback default; let Phase 436 override at runtime. Confirm before 435-01 if this is wrong.

**OQ-3 — TextInput.text redaction policy.**
Plan mentions `redact: Boolean` flag on TextInput to prevent logging password characters. Should the default be true (safe by default, drivers opt out) or false (explicit opt-in)? CLAUDE.md security stance suggests default-deny — so `redact: true` by default with drivers opting out for order fields etc. is safer. **Planner recommendation:** `redact: true` by default, documented in AUDIT-LOG.md, explicit `redact=false` in driver manifests for non-credential fields. Confirm before 435-04.

**OQ-4 — Sustained shipping-stall behaviour: drop or halt?**
If shipping fails for 24h+ AND local buffer fills, current plan drops oldest. Alternative: HALT the agent entirely (refuse new actions) to guarantee no data loss. **Planner recommendation:** drop (current plan); Phase 441 will add a 24h shipping-stall alarm + admin dashboard WARN. Halting the agent is too aggressive for reception automation where Zomato orders pile up during any downtime. Confirm or direct.

**OQ-5 — Does the drill need a real Zomato Partner session, or is the mock driver + Settings app sufficient?**
Current plan: mock driver targets Settings. Alternative: drive Zomato Partner explicitly (requires a real Zomato account + session, which Phase 437 sets up). **Planner recommendation:** mock for 435-09 (faster, fewer dependencies); real Zomato exercised in Phase 437's drill. FLAG_SECURE behaviour IS tested against real Zomato login screen as a separate 5-minute drill (listed in 435-09 step 7). Confirm.

**OQ-6 — ShippingClient: run in agent process or in a separate WorkManager periodic job?**
WorkManager would survive app restart and respect Android's battery/Doze policy more robustly. However, it complicates ShippedCursor consistency (WorkManager can execute while the main agent is sleeping). **Planner recommendation:** run in agent process (simpler, ShippedCursor is consistent); WorkManager is a Phase 441+ optimisation if battery drain becomes a problem. Confirm.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` — HUMANIZE-01..04, AUDIT-01..04
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Prior phase:** 432 (driver framework) — DriverContext extended here with `dispatch: HumanizeAccessibilityBridge` + `audit: AuditLog`
- **Consumer phases:** 437 (Zomato), 438 (HyperPure), 439 (Blinkit) — all MUST use `ctx.dispatch.*`; grep enforced
- **Downstream phase:** 441 (admin dashboard log viewer + real server storage) — replaces stub ingest with SQLite persistence + drift detection + admin WARN banner for shipping stalls
- **Reference pattern:** `rc-agent-mobile/app/src/main/kotlin/.../log/RotatingLog.kt` (Phase 429-07) — similar-but-separate rotation; RotationPolicy follows same convention

## 11. Output (at phase close)

At the end of 435-09 (drill pass), create `.planning/phases/435-humanize-layer-audit-log/SUMMARY.md` capturing:

- Which commits implemented each plan (435-01 through 435-09)
- Numeric drill evidence: SC-1..SC-4 results with actual counts, screenshot hash samples
- `grep -rn "ctx.accessibility\.\(tap\|swipe\|textInput\)" app/src/main/` output (must be empty)
- Any risks encountered and how they were resolved
- Any open questions from §9 resolved during execution — update state
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked
- Handoff to Phase 436 (feature flags) — what's ready, what's deferred
- Outstanding item for Phase 441: replace stub ingest endpoint with real storage + admin log viewer UI + 24h shipping-stall alarm

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 7 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
