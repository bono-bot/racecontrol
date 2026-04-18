---
phase: 432-driver-framework-capability-registry
phase_number: 432
milestone: v50.0 rc-agent-mobile
name: "Pluggable Driver Framework + Capability Registry"
status: ready-to-execute
goal: >
  Ship the architectural keystone of v50.0 — a pluggable driver framework where
  drivers are plugins loaded via JSON manifest (not hardcoded if/else), each
  driver runs in its own supervised CoroutineScope with a CoroutineExceptionHandler
  (isolation — one driver crashing never kills the agent or siblings), lifecycle
  hooks (install/onAppUpdate/healthCheck/uninstall) fire deterministically, each
  device declares its capability list in registration + heartbeat, and
  supported_device_types enforcement refuses to install a tablet-only driver on
  a phone. A sample HelloDriver proves the contract end-to-end and a deliberately
  crashing CrashDriver proves isolation end-to-end.
requirements: [DRIVER-01, DRIVER-02, DRIVER-03, DRIVER-04, DRIVER-05, CAPREG-01, CAPREG-02, CAPREG-03, CAPREG-04]
depends_on: [430]  # Phase 430 ships AccessibilityBridge (screen/tree, ui/tap, text input) — DriverContext injects it.
wave: 4            # Wave 1 = 429, Wave 2 = 430, Wave 3 = 431, Wave 4 = 432. 433-436 run parallel AFTER 432 lands.
plan_count: 9
plans:
  - 432-01-PLAN: AppDriver interface + DriverContext (DI) + HealthStatus type
  - 432-02-PLAN: Driver manifest spec + schema validation (drivers.json)
  - 432-03-PLAN: DriverRegistry — discover + load bundled manifests at boot
  - 432-04-PLAN: Per-driver supervised CoroutineScope + ExceptionHandler (isolation core)
  - 432-05-PLAN: LifecycleDispatcher — install / onAppUpdate / healthCheck@5min / uninstall
  - 432-06-PLAN: CapabilityRegistry — publish in registration + heartbeat + /capability endpoint
  - 432-07-PLAN: supported_device_types enforcement (refuse wrong-type install)
  - 432-08-PLAN: CrashDriver isolation test (verified sibling + core survive)
  - 432-09-PLAN: HelloDriver sample — end-to-end plug-in contract proof
autonomous: true   # No physical-device human-verify gates. All acceptance is automated on device via adb + unit tests. Phase-level MMA audit IS mandatory but is a gate, not a checkpoint task.
files_modified:
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/AppDriver.kt                        # interface (contract)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverContext.kt                    # DI container for drivers
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/HealthStatus.kt                     # sealed class Healthy/Degraded/Unhealthy
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverManifest.kt                   # @Serializable manifest data class
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverManifestLoader.kt             # loads + validates drivers.json
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverRegistry.kt                   # discover/load/track all drivers
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverScope.kt                      # SupervisorJob + ExceptionHandler per driver
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/LifecycleDispatcher.kt              # install/update/healthCheck/uninstall hooks
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/CapabilityRegistry.kt               # device_id -> [driver_id] persistent map
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DeviceType.kt                       # enum Tablet, Phone, SmartDisplay (reserved), Ps5Tablet (reserved), KioskPhone (reserved)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/PackageMonitor.kt                   # watches PackageManager for app-update events
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/sample/HelloDriver.kt               # no-op sample driver
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/sample/CrashDriver.kt               # deliberately-crashing test driver
  - rc-agent-mobile/app/src/main/assets/drivers.json                                                             # canonical bundled manifest list
  - rc-agent-mobile/app/src/main/assets/drivers/hello-driver.json                                                # per-driver manifest
  - rc-agent-mobile/app/src/main/assets/drivers/crash-driver.json                                                # per-driver manifest
  - rc-agent-mobile/app/src/main/AndroidManifest.xml                                                             # amend: PackageMonitor broadcast receiver
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt           # wire DriverRegistry + LifecycleDispatcher into service lifecycle
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/LocalHttpServer.kt                     # extend /capability to return live registry contents
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/CommsLinkClient.kt                    # include capabilities in register payload
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/HeartbeatScheduler.kt                 # include capabilities diff in heartbeat
  - rc-agent-mobile/docs/DRIVER-FRAMEWORK.md                                                                     # architectural reference doc
  - rc-agent-mobile/docs/MANIFEST-SCHEMA.md                                                                      # drivers.json schema spec
  - rc-agent-mobile/app/src/test/kotlin/.../driver/                                                              # unit tests (one per plan)
  - .planning/phases/432-driver-framework-capability-registry/SUMMARY.md                                         # written at phase close

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: >
    Android APK rebuilt and installed via ADB on Tab Plus (TB-351FU) + M07.
    No new firewall rules (drivers run inside existing agent process on :8090).
    No new comms-link routes (register + heartbeat payloads gain a `capabilities`
    field — forward-compat because 429-04's PROTOCOL.md uses ignoreUnknownKeys=true).
  data_files: >
    rc-agent-mobile/app/src/main/assets/drivers.json       (canonical manifest list)
    rc-agent-mobile/app/src/main/assets/drivers/*.json      (per-driver manifests)
    Both ship inside the APK; no on-device generation.  Remote push (Phase 436/443)
    overrides these at runtime but 432 ships with bundled-only loading.
  bat_file: none
  cloud_parity: [none]   # No cloud-side changes in 432.  Admin dashboard capability view is Phase 13/14.
  targets:
    - tab_plus   # Lenovo TB-351FU
    - m07        # Samsung Galaxy M07
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  rollback:
    - "Keep prior APK at /sdcard/Download/rc-agent-mobile-prev.apk"
    - "Uninstall current: adb uninstall in.racingpoint.rcagentmobile"
    - "Reinstall prev: adb install /sdcard/Download/rc-agent-mobile-prev.apk"
    - "On rollback, CapabilityRegistry file at /data/data/.../files/capability-registry.json is preserved — next install re-reads it."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip             # No user-facing UI in 432.  Persistent notification text update only (already built in 429-02).
  ui_auditor: skip                # Same reason.
  nyquist_auditor: required       # Lifecycle dispatcher, registration payload wiring, isolation semantics are all business logic with defined I/O.
  mma_audit: required             # THIS IS THE ARCHITECTURAL KEYSTONE OF v50.0.  Every subsequent driver (Zomato, HyperPure, Blinkit, cardboard, future smart-display drivers) is built on this framework.  A wrong abstraction here calcifies into every future phase.  Budget: $5 unless Uday approves more.  Dual reasoning modes REQUIRED (abstract for interface shape + trace-level for coroutine scope / lifecycle edge cases).
  integration_checker: required   # Touches service lifecycle + HTTP server + comms client + package monitor.  Must run before v50.0 milestone ship.
  codebase_mapper: required       # New subsystem (rc-agent-mobile/driver/*).  Refresh .planning/codebase/ after 432-09 lands so 433+ planners see the driver module.

risks_summary:
  - "Wrong interface shape calcifies — every future driver inherits the mistake.  Mitigated by MMA dual reasoning mode audit BEFORE merging 432-01."
  - "SupervisorJob vs CoroutineScope semantics are easy to get wrong — a child job's uncaught exception in a regular Job() cancels the parent.  We explicitly use SupervisorJob + per-driver CoroutineExceptionHandler and write a dedicated isolation test (432-08)."
  - "Drivers holding strong Context refs leak memory.  DriverContext passes an `applicationContext` (not Activity) and never exposes the Service directly — drivers receive narrow callable interfaces, not the container."
  - "Lifecycle hook ordering bugs — onAppUpdate firing before install() on a fresh install would be a silent correctness bug.  LifecycleDispatcher has deterministic sequencing encoded in a state machine + unit test matrix."
  - "healthCheck every 5min × N drivers = N coroutines on a 5min ticker.  Fine for N < 20; document ceiling in DRIVER-FRAMEWORK.md.  Drivers whose healthCheck blocks > 30s will be hard-timed-out by the dispatcher."
  - "Manifest validation must be strict at LOAD TIME.  A silently-ignored typo in supported_device_types turns tablet-only into run-anywhere.  Validator fails loudly and writes a SelectorMissEvent-class log entry (reusing 429-07 RotatingLog)."
  - "Remote-pushed manifest trust model is OUT OF SCOPE for 432 (Phase 436 + 443 own this) — 432 only loads from APK assets.  Document this boundary clearly so a future planner does not assume remote push is already secure."
  - "Package version polling could fire spuriously on APK reinstall.  PackageMonitor dedupes on (package, versionCode) tuple with a 30s cooldown."
---

# Phase 432 — Pluggable Driver Framework + Capability Registry

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 432 |
| Name | Pluggable Driver Framework + Capability Registry |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | DRIVER-01, DRIVER-02, DRIVER-03, DRIVER-04, DRIVER-05, CAPREG-01, CAPREG-02, CAPREG-03, CAPREG-04 |
| Dependencies | Phase 430 (Accessibility primitives — DriverContext wraps them) |
| Wave | 4 |
| Status | Ready to execute (after 430 lands) |
| Autonomous | Yes — all acceptance is automated (unit + instrumented + adb). MMA audit is a phase gate, not a task checkpoint. |
| Ship test | Adding a new driver = drop a module + manifest entry, zero core changes (proved by HelloDriver); CrashDriver does not kill agent or sibling drivers (proved by CrashDriver test); tablet-only driver refuses to install on phone (proved by supported_device_types enforcement test); capability list visible in `GET /capability` + comms-link register payload. |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 4)

1. **Pluggable — zero core code changes to add a driver.** Adding a new driver = drop a module dir + manifest entry; zero core-agent code changes.
2. **Registration carries capabilities.** Device registration payload includes capability list; visible in admin (proxied via comms-link relay for 432; admin UI is Phase 14).
3. **Crash isolation.** Crashing a driver does not kill the agent or sibling drivers (verified by a deliberately-crashing test driver).
4. **Deterministic lifecycle.** Lifecycle hooks fire deterministically — install on enable, onAppUpdate on package change, healthCheck every 5 min, uninstall on disable.
5. **Device-type enforcement.** Manifest `supported_device_types` blocks installing a tablet-only driver on a phone.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable / test-observable)

- T-1: Dropping a new module directory `driver/sample/HelloDriver.kt` + a manifest entry in `assets/drivers/hello-driver.json` causes the driver to load at next boot with NO changes to `AgentForegroundService.kt`, `DriverRegistry.kt`, `LifecycleDispatcher.kt`, or any other core file. (DRIVER-02, DRIVER-03)
- T-2: `GET http://<device_ip>:8090/capability` returns a JSON body that lists every driver whose manifest loaded successfully, with `driver_id`, `version`, `enabled_at`, `health_status`. (CAPREG-02)
- T-3: The comms-link relay log shows a `register` envelope whose `payload.capabilities` is a non-empty JSON array matching the output of T-2. (CAPREG-01)
- T-4: Running `CrashDriver` (throws in its action handler) does NOT crash the agent process — `adb shell ps | grep in.racingpoint.rcagentmobile` still shows the process running, AND sibling `HelloDriver` still responds to its own health check. (DRIVER-05)
- T-5: Installing APK with a tablet-only driver manifest on M07 (phone) causes `DriverRegistry.install` to SKIP that driver with a log entry `driver_skipped_unsupported_device_type` — the driver is NOT present in `/capability`. (CAPREG-03, DRIVER-04)
- T-6: Reinstalling a target-app APK whose version changed triggers exactly one `onAppUpdate(old, new)` invocation on drivers whose manifest declares `target_package`. (DRIVER-04)
- T-7: Exactly every 300 ± 30 seconds, every installed driver receives exactly one `healthCheck()` invocation. Timings verified via log event counts over a 20-minute adb logcat capture. (DRIVER-04)
- T-8: Toggling the driver off (via a local dev trigger — real flag push is Phase 436) invokes `uninstall()` within 10 seconds, and the driver disappears from `/capability`. (DRIVER-04)
- T-9: Manifests with unknown `supported_device_types` values (e.g., `smart_display`) LOAD (reserved enum slots are accepted by schema) but are SKIPPED on today's Tab Plus + M07 — they appear in `/capability` with `status: reserved_type_not_installed` so future phases know the reservation is honored. (CAPREG-04)

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/app/src/main/kotlin/.../driver/AppDriver.kt` | Driver contract interface | 40 | `interface AppDriver { fun install(ctx); fun onAppUpdate(old,new,ctx); suspend fun healthCheck(): HealthStatus; fun uninstall() }` + typed action extension point |
| `.../driver/DriverContext.kt` | DI container passed to every driver | 60 | Accessibility bridge (from 430), HTTP client, RotatingLog, application Context (never Service), Json serializer, DriverIdentity |
| `.../driver/HealthStatus.kt` | Healthy/Degraded/Unhealthy sealed class | 30 | `sealed class HealthStatus { object Healthy; data class Degraded(msg); data class Unhealthy(cause, ts) }` |
| `.../driver/DriverManifest.kt` | `@Serializable` manifest data class | 80 | All 12 manifest fields (see 432-02 schema) |
| `.../driver/DriverManifestLoader.kt` | Parse + validate manifest JSON | 120 | `fun loadAll(assets): List<DriverManifest>`, strict validator, load-time logs for every failure |
| `.../driver/DriverRegistry.kt` | Track every loaded driver | 150 | Map driver_id → DriverHandle (manifest + driver-class + scope + lastHealth), `install(manifest)`, `uninstall(id)`, `all()`, `byId(id)` |
| `.../driver/DriverScope.kt` | Per-driver coroutine scope | 80 | `fun createScope(driverId): CoroutineScope` with SupervisorJob + named CoroutineExceptionHandler + structured name for thread dumps |
| `.../driver/LifecycleDispatcher.kt` | Runs install/update/healthCheck/uninstall | 200 | State machine: NotInstalled → Installing → Installed → Updating → Installed → Uninstalling → NotInstalled |
| `.../driver/CapabilityRegistry.kt` | device_id → [driver_id] persistent map | 100 | In-memory map + JSON file at `getFilesDir()/capability-registry.json`, broadcasts change events via Flow |
| `.../driver/DeviceType.kt` | Type enum | 25 | `enum class DeviceType { Tablet, Phone, SmartDisplay, Ps5Tablet, KioskPhone, Unknown }` |
| `.../driver/PackageMonitor.kt` | Observe package-added/replaced | 80 | BroadcastReceiver on `android.intent.action.PACKAGE_REPLACED`, `PACKAGE_ADDED`; dedupe 30s; route to LifecycleDispatcher |
| `.../driver/sample/HelloDriver.kt` | No-op sample driver | 50 | Implements AppDriver; returns Healthy; logs "hello from HelloDriver" in install/healthCheck/uninstall |
| `.../driver/sample/CrashDriver.kt` | Deliberately-crashing test driver | 40 | Implements AppDriver; `healthCheck()` throws `IllegalStateException("intentional crash for isolation test")` |
| `rc-agent-mobile/app/src/main/assets/drivers.json` | Canonical bundled manifest list | 10 | `{ "v": 1, "drivers": ["drivers/hello-driver.json", "drivers/crash-driver.json"] }` |
| `rc-agent-mobile/app/src/main/assets/drivers/hello-driver.json` | HelloDriver manifest | 15 | See 432-02 schema |
| `rc-agent-mobile/app/src/main/assets/drivers/crash-driver.json` | CrashDriver manifest | 15 | Same schema; enabled=false by default (test-only) |
| `rc-agent-mobile/docs/DRIVER-FRAMEWORK.md` | Architectural reference | 250 | Rationale, lifecycle diagram, isolation model, extension cookbook |
| `rc-agent-mobile/docs/MANIFEST-SCHEMA.md` | drivers.json schema spec | 150 | Field table, examples, forward-compat rules |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| `AgentForegroundService.onCreate` | `DriverRegistry.start(context)` | Kotlin call | `grep DriverRegistry.start` in `AgentForegroundService.kt` |
| `DriverRegistry.start` | `DriverManifestLoader.loadAll(assets)` | Kotlin call | `grep DriverManifestLoader.loadAll` in `DriverRegistry.kt` |
| `DriverRegistry.install(manifest)` | `DriverScope.createScope(driverId)` | Kotlin call | `grep DriverScope.createScope` in `DriverRegistry.kt` |
| `DriverRegistry.install` → driver.install | Runs INSIDE the per-driver scope | Kotlin call | `grep launch(driverScope)` + `driver.install(context)` in `LifecycleDispatcher.kt` |
| `PackageMonitor.onReceive(PACKAGE_REPLACED)` | `LifecycleDispatcher.handleAppUpdate(pkg, oldVer, newVer)` | broadcast intent → function call | `grep handleAppUpdate` in `PackageMonitor.kt` |
| Ticker every 5 min | `LifecycleDispatcher.runHealthChecks()` | kotlinx coroutine ticker | `grep delay(300_000` OR `fixedPeriod = 300` in `LifecycleDispatcher.kt` |
| `CommsLinkClient.register()` payload build | `CapabilityRegistry.snapshot()` | Kotlin call | `grep CapabilityRegistry.snapshot` in `CommsLinkClient.kt` |
| `HeartbeatScheduler.tick()` payload build | `CapabilityRegistry.snapshot()` | Kotlin call | `grep CapabilityRegistry.snapshot` in `HeartbeatScheduler.kt` |
| `LocalHttpServer /capability handler` | `CapabilityRegistry.snapshot()` | Ktor route | `grep CapabilityRegistry.snapshot` in `LocalHttpServer.kt` |
| `DriverRegistry.install` | Rejects when `supported_device_types` mismatches device type | branch + log | `grep driver_skipped_unsupported_device_type` in `DriverRegistry.kt` |
| Exception anywhere in a driver's coroutine | Handled by `CoroutineExceptionHandler` → marks driver `Unhealthy` → agent + sibling drivers unaffected | exception handler | `grep CoroutineExceptionHandler` in `DriverScope.kt` AND isolation test asserting sibling survives |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md
@./.planning/phases/430-accessibility-service-foundation/PLAN.md
@./rc-agent-mobile/docs/PROTOCOL.md                                                    # from 429-04 — register/heartbeat envelope
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/protocol/Protocol.kt        # envelope data classes (432 extends RegisterPayload/HeartbeatPayload with `capabilities`)
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt    # where DriverRegistry boots
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/LocalHttpServer.kt     # where `/capability` route was stubbed empty in 429-03 — 432 extends it
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/log/RotatingLog.kt          # 429-07 — reuse for driver lifecycle event logging

### Interfaces executors will need (extracted so executors do not scavenger-hunt)

#### From 429-03 — `LocalHttpServer.kt` (already exists):

```kotlin
class LocalHttpServer(private val port: Int, private val deviceState: DeviceState) {
    fun start()
    fun stop()
    // 432 adds: capabilityRegistry: CapabilityRegistry property, /capability route handler reads from it
}
```

#### From 429-05 — `Protocol.kt` (already exists):

```kotlin
@Serializable data class Envelope<T>(
    val v: Int = 1,
    val protocol_version: Int = 1,
    val type: String,
    val from: String,
    val ts: Long,
    val id: String,
    val payload: T
)
@Serializable data class RegisterPayload(
    val device_id: String,
    val device_model: String,
    val android_version: String,
    val agent_version: String,
    val build_id: String,
    val capabilities: List<CapabilityEntry> = emptyList(),   // 432 populates this
    val supported_device_types: List<String>
)
@Serializable data class HeartbeatPayload(
    val uptime_secs: Long,
    val memory_mb: Long,
    val battery_pct: Int?,
    val capabilities_digest: String = ""                      // 432 populates this (SHA256 of capabilities) — relay detects change cheaply
)
```

#### From 429-07 — `RotatingLog.kt` (already exists):

```kotlin
object RotatingLog {
    fun info(target: String, event: String, details: Map<String, Any?> = emptyMap())
    fun warn(target: String, event: String, details: Map<String, Any?> = emptyMap())
    fun error(target: String, event: String, details: Map<String, Any?> = emptyMap())
    fun debug(target: String, event: String, details: Map<String, Any?> = emptyMap())
}
```

#### From Phase 430 — `AccessibilityBridge.kt` (to be built; 432 depends only on the interface):

```kotlin
interface AccessibilityBridge {
    suspend fun screenTree(): AccessibilityNodeSnapshot
    suspend fun tap(selector: Selector): TapResult
    suspend fun swipe(from: Point, to: Point, durationMs: Long): SwipeResult
    suspend fun inputText(selector: Selector, text: String): InputResult
    fun isAccessibilityEnabled(): Boolean
}
```

(If Phase 430 does not land before 432 execution begins, 432-01 still creates a stub interface `AccessibilityBridge` so drivers can be coded against the contract. 430 will then replace the stub with the real implementation.)

### New interfaces THIS phase creates (consumed by 433 onward)

```kotlin
// 432-01 defines ALL of these as the FIRST commit — every subsequent plan imports from here.

interface AppDriver {
    /** Called once when driver is enabled. Must be idempotent (may be called after a crash + restart). */
    fun install(ctx: DriverContext)

    /** Called when the target app (declared via manifest.target_package) changes version. */
    fun onAppUpdate(oldVersionCode: Long, newVersionCode: Long, ctx: DriverContext)

    /** Called every 5 minutes. Must return within 30s or will be considered Unhealthy. */
    suspend fun healthCheck(): HealthStatus

    /** Called when the driver is disabled. Must release resources. */
    fun uninstall()
}

sealed class HealthStatus {
    object Healthy : HealthStatus()
    data class Degraded(val reason: String) : HealthStatus()
    data class Unhealthy(val cause: String, val sinceMs: Long) : HealthStatus()
}

data class DriverContext(
    val driverId: String,
    val applicationContext: android.content.Context,            // NEVER the Service — prevents leaks
    val accessibility: AccessibilityBridge,                     // from 430
    val httpClient: okhttp3.OkHttpClient,
    val log: DriverLog,                                         // thin per-driver wrapper around RotatingLog
    val json: kotlinx.serialization.json.Json,
    val scope: CoroutineScope                                   // per-driver supervised scope; do not create your own
)

@Serializable data class DriverManifest(
    val driver_id: String,                                       // "zomato-partner"
    val driver_class: String,                                    // "in.racingpoint.drivers.zomato.ZomatoDriver"
    val version: String,                                         // "1.0.0"
    val target_package: String?,                                 // "com.application.zomato.partner" (nullable — HelloDriver has none)
    val supported_device_types: List<String>,                    // ["tablet","phone"]
    val requires_accessibility: Boolean,                         // true for all real drivers
    val requires_credentials: String?,                           // "PersistentSession" — Phase 434 owns strategies; 432 stores the string
    val rate_limit_per_minute: Int = 60,                         // Phase 435 humanize consumer
    val humanize_delay_mean_ms: Int = 800,                       // Phase 435 humanize consumer
    val humanize_delay_stddev_ms: Int = 200,                     // Phase 435 humanize consumer
    val enabled_by_default: Boolean = false,                     // Phase 436 feature-flag consumer
    val manifest_schema_version: Int = 1                         // For forward-compat
)

class DriverRegistry {
    fun start(context: android.content.Context)
    suspend fun install(manifest: DriverManifest): InstallResult
    suspend fun uninstall(driverId: String)
    fun all(): List<DriverHandle>
    fun byId(id: String): DriverHandle?
    fun observe(): Flow<RegistryEvent>                           // for CapabilityRegistry + HeartbeatScheduler reactivity
}

sealed class InstallResult {
    data class Installed(val handle: DriverHandle) : InstallResult()
    data class SkippedUnsupportedDeviceType(val supported: List<String>, val actual: DeviceType) : InstallResult()
    data class SkippedReservedType(val reservedType: DeviceType) : InstallResult()
    data class Failed(val driverId: String, val cause: Throwable) : InstallResult()
}

data class DriverHandle(
    val manifest: DriverManifest,
    val driver: AppDriver,
    val scope: CoroutineScope,
    val state: StateFlow<DriverState>
)

sealed class DriverState {
    object NotInstalled : DriverState()
    object Installing : DriverState()
    data class Installed(val since: Long, val lastHealth: HealthStatus) : DriverState()
    data class Updating(val oldVer: Long, val newVer: Long) : DriverState()
    object Uninstalling : DriverState()
    data class Crashed(val cause: Throwable, val at: Long) : DriverState()
}

class CapabilityRegistry(
    private val filesDir: File,
    private val registry: DriverRegistry
) {
    fun snapshot(): List<CapabilityEntry>                        // for /capability + register + heartbeat
    fun digest(): String                                         // SHA256 of snapshot — cheap change detection in heartbeat
    fun observe(): Flow<List<CapabilityEntry>>
}

@Serializable data class CapabilityEntry(
    val driver_id: String,
    val version: String,
    val enabled_at: Long,
    val state: String,                                           // "installed" | "crashed" | "reserved_type_not_installed" | "unsupported_device_type"
    val health_status: String,                                   // "healthy" | "degraded" | "unhealthy" | "unknown"
    val last_health_at: Long?
)
```

These interfaces are the **entire public surface** of the 432 framework. Phases 433-440 program against these exclusively — they do not touch `AgentForegroundService`, `CommsLinkClient`, or `LocalHttpServer` directly.

## 5. Atomic plan breakdown (9 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion. Order is strictly sequential — 432-01 MUST land first (defines all contracts), 432-02 next (manifest format the rest depends on), then the rest in order.

---

### 432-01-PLAN — AppDriver interface + DriverContext + HealthStatus

**Goal:** Lock the public contract of the driver framework before any implementation exists. Every plan that follows programs against these types. Interface-first per CLAUDE.md planning philosophy.

**Covers:** DRIVER-01 (contract shape)

**Dependencies:** 429 complete (types extended in 432-06); 430 provides `AccessibilityBridge` interface (or 432-01 stubs it if 430 has not merged yet — the stub is replaced by 430 without changes to 432 code).

**Type:** `auto` (interface + unit tests)

**TDD:** `tdd="true"` — tests written BEFORE code.

#### Behavior

- Given an implementer writes `class MyDriver : AppDriver { ... }`, the compiler forces them to provide `install`, `onAppUpdate`, `healthCheck`, and `uninstall`.
- `HealthStatus.Healthy` compares equal regardless of construction site.
- `HealthStatus.Unhealthy("x", 123L)` and `HealthStatus.Unhealthy("x", 123L)` are equal (data class).
- `DriverContext.applicationContext` cannot be cast to a `Service` or `Activity` at construction (defensive: fail-fast if the caller passes anything but `applicationContext`).

#### Tasks

1. Create `driver/AppDriver.kt` with the interface defined in §4 above. No implementation.
2. Create `driver/HealthStatus.kt` as a sealed class with `Healthy` (object), `Degraded(reason: String)` (data class), `Unhealthy(cause: String, sinceMs: Long)` (data class).
3. Create `driver/DriverContext.kt` as a data class. In `init { }`, assert `applicationContext === applicationContext.applicationContext` (Android idiom — an Activity/Service's applicationContext is idempotent; a wrongly-passed Activity would fail this assertion).
4. Create `driver/DriverLog.kt` — a thin wrapper around `RotatingLog` that automatically scopes every call to `target = "driver.$driverId"`. This is a per-driver log surface so every log line is attributable.
5. Create `driver/DeviceType.kt` enum with `Tablet, Phone, SmartDisplay, Ps5Tablet, KioskPhone, Unknown` (CAPREG-04 reserves future types — they compile today, even though no device returns them from `DeviceTypeResolver` yet).
6. Create a `DeviceTypeResolver` object with `fun resolve(context): DeviceType` that:
   - Reads `context.resources.configuration.smallestScreenWidthDp`
   - ≥ 600 dp → `Tablet`
   - < 600 dp → `Phone`
   - `BuildConfig` override (`deviceTypeOverride` set in `local.properties`) always wins if present (supports OQ-5 from 429).
7. If Phase 430 has NOT yet landed, stub `driver/AccessibilityBridge.kt` with the interface signature only + a `TODO()` implementation. Phase 430 will swap in the real impl. If 430 HAS landed, import from its package.
8. Unit tests in `driver/AppDriverContractTest.kt`:
   - Write a fake driver; confirm it compiles against the interface.
   - Assert `HealthStatus.Healthy == HealthStatus.Healthy`.
   - Assert `HealthStatus.Unhealthy("x", 0) == HealthStatus.Unhealthy("x", 0)`.
   - Assert `HealthStatus.Unhealthy("x", 0) != HealthStatus.Healthy`.
9. Unit test in `driver/DeviceTypeResolverTest.kt`:
   - Fake configurations for phone (360 dp) and tablet (800 dp); assert resolver outputs `Phone` vs `Tablet`.
   - Override slot test: set `BuildConfig.DEVICE_TYPE_OVERRIDE = "tablet"`, assert even on 360 dp it returns `Tablet`.

#### Acceptance

- `./gradlew :app:compileDebugKotlin` succeeds.
- `./gradlew :app:testDebugUnitTest --tests '*AppDriverContractTest*'` passes.
- `./gradlew :app:testDebugUnitTest --tests '*DeviceTypeResolverTest*'` passes.
- No implementation of `AppDriver` exists yet (this plan creates contracts only) — compile succeeds because `HelloDriver` and `CrashDriver` are not in this commit.
- Grep check: `grep -rn "class.*AppDriver" app/src/main/` returns nothing besides the interface declaration and the fake in tests.

#### G4 NOT TESTED list

- Driver registry (432-03).
- Per-driver scopes (432-04).
- Lifecycle dispatcher (432-05).
- Manifest parsing (432-02).

#### Commit message

```
feat(432-01): AppDriver interface + DriverContext + HealthStatus (contract)

Defines the public surface of the pluggable driver framework before any
implementation exists.  HealthStatus is a sealed class (Healthy/Degraded/
Unhealthy).  DriverContext injects ApplicationContext (never Service —
prevents leaks), AccessibilityBridge (from Phase 430 or stubbed),
OkHttpClient, DriverLog, Json, and the per-driver coroutine scope.
DeviceType enum reserves SmartDisplay/Ps5Tablet/KioskPhone for CAPREG-04.

Covers: DRIVER-01 (contract shape), CAPREG-04 (reserved enum slots)
Not tested: runtime behavior (deferred to 432-03..09).
```

---

### 432-02-PLAN — Driver manifest spec + JSON schema + loader + validator

**Goal:** Lock `drivers.json` manifest format and ship a strict load-time validator that fails loudly on typos. Document schema in `MANIFEST-SCHEMA.md` so future drivers (433+) have a stable reference.

**Covers:** DRIVER-02 (manifest-driven registration), DRIVER-03 (new driver = drop manifest, no core changes)

**Dependencies:** 432-01 (`DeviceType` enum used by manifest schema)

**Type:** `auto` + `tdd="true"` (schema is testable — property-based)

#### Behavior

- A well-formed manifest JSON parses into `DriverManifest`.
- An empty `driver_id` fails validation with log `manifest_invalid: driver_id empty`.
- An unknown `supported_device_types` value that is NOT in the reserved set fails with `manifest_invalid: supported_device_types contains unknown <val>`.
- Reserved types (`smart_display`, `ps5_tablet`, `kiosk_phone`) LOAD successfully (CAPREG-04 reservation honored).
- A duplicate `driver_id` across two manifest files fails with `manifest_invalid: duplicate driver_id`.
- An unknown top-level field is ignored with a DEBUG log (forward-compat).
- `manifest_schema_version > 1` causes the loader to SKIP the manifest with a WARN (forward guard — future framework version).

#### Tasks

1. Create `driver/DriverManifest.kt` — `@Serializable` data class with all fields listed in §4 above. Use `kotlinx.serialization.json` with `ignoreUnknownKeys = true`.
2. Create `driver/DriverManifestLoader.kt`:
   - `fun loadAll(assetManager: AssetManager): List<DriverManifest>`
   - Reads `drivers.json` (index file) from `assets/drivers.json`.
   - Expected shape: `{"v": 1, "drivers": ["drivers/hello-driver.json", "drivers/crash-driver.json"]}`
   - For each path, reads + validates the per-driver manifest.
   - Returns only valid manifests; logs every failure via `RotatingLog.error("driver", "manifest_invalid", {...})`.
3. Validation rules (all implemented with targeted error messages):
   - `driver_id` non-empty, matches `^[a-z0-9-]{3,40}$`
   - `driver_class` non-empty, looks like a JVM FQCN (`^[a-z][a-z0-9_]*(\.[a-zA-Z][a-zA-Z0-9_]*)+$`)
   - `version` matches semver lite `^\d+\.\d+\.\d+$`
   - `supported_device_types` non-empty; every element ∈ `{"tablet","phone","smart_display","ps5_tablet","kiosk_phone"}` (string form matches `DeviceType` enum names in snake_case)
   - `target_package` either null or matches `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$`
   - `rate_limit_per_minute` in `[1, 600]`
   - `humanize_delay_mean_ms` in `[0, 10_000]`
   - `humanize_delay_stddev_ms` in `[0, 5_000]`
   - `manifest_schema_version == 1` (currently)
4. Global cross-manifest check: duplicate `driver_id` across manifests → LOAD FAILS for the later one with an explicit log.
5. Create `rc-agent-mobile/app/src/main/assets/drivers.json`:
   ```json
   { "v": 1, "drivers": ["drivers/hello-driver.json", "drivers/crash-driver.json"] }
   ```
6. Create `rc-agent-mobile/app/src/main/assets/drivers/hello-driver.json`:
   ```json
   {
     "driver_id": "hello",
     "driver_class": "in.racingpoint.rcagentmobile.driver.sample.HelloDriver",
     "version": "1.0.0",
     "target_package": null,
     "supported_device_types": ["tablet","phone"],
     "requires_accessibility": false,
     "requires_credentials": null,
     "rate_limit_per_minute": 10,
     "humanize_delay_mean_ms": 0,
     "humanize_delay_stddev_ms": 0,
     "enabled_by_default": true,
     "manifest_schema_version": 1
   }
   ```
7. Create `rc-agent-mobile/app/src/main/assets/drivers/crash-driver.json` — same shape, `driver_id: "crash-test"`, `enabled_by_default: false`, `driver_class: ...sample.CrashDriver`.
8. Write `rc-agent-mobile/docs/MANIFEST-SCHEMA.md` (~150 lines): field table, examples, forward-compat rules, reserved `supported_device_types` list, sample full manifest, schema-evolution rules.
9. Unit tests in `driver/DriverManifestLoaderTest.kt`:
   - Valid manifest loads.
   - Each validation rule has a dedicated negative test (empty id, wrong semver, unknown device type, bad FQCN, etc.) — ~12 tests total.
   - Duplicate driver_id across two manifests: only the first loads; second is rejected with correct error.
   - Unknown top-level field is accepted (forward-compat) but logged.
   - `manifest_schema_version = 2` → SKIPPED with WARN.
   - Reserved device type (`smart_display`) loads.

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*DriverManifestLoaderTest*'` passes all 14+ tests.
- Running a smoke harness (created in 432-03) with only HelloDriver + CrashDriver manifests returns a list of 2 valid `DriverManifest` instances.
- `MANIFEST-SCHEMA.md` exists, ≥ 150 lines.

#### G4 NOT TESTED list

- Actual installation (432-03).
- Lifecycle dispatch (432-05).
- supported_device_types enforcement against live device (432-07).

#### Commit message

```
feat(432-02): driver manifest schema + strict load-time validator

Defines DriverManifest with 12 fields.  JSON schema loader validates every
rule before install: driver_id pattern, semver, supported_device_types
enum, FQCN class path, rate limit bounds, humanize bounds.  Duplicate
driver_id across two manifests causes the second to be rejected.
Unknown fields = forward-compat (log DEBUG, accept).
manifest_schema_version > 1 = skip with WARN.
Bundles HelloDriver + CrashDriver manifests under assets/drivers/.

Covers: DRIVER-02 (manifest registration), DRIVER-03 (no core changes)
Not tested: install/uninstall/lifecycle (deferred to 432-03..05).
```

---

### 432-03-PLAN — DriverRegistry: discover and load at boot

**Goal:** On `AgentForegroundService.onCreate`, the registry scans bundled manifests, instantiates driver classes via reflection, and holds handles. No lifecycle hooks fire yet (432-05). No isolation scope yet (432-04). This plan is purely registration + handle-holding.

**Covers:** DRIVER-02, DRIVER-03

**Dependencies:** 432-01, 432-02

**Type:** `auto`

#### Behavior

- At service boot, `DriverRegistry.start(context)` produces one `DriverHandle` per valid manifest.
- Reflection instantiation failures (class not found, no-arg constructor missing) log ERROR and skip the driver without killing the service.
- `registry.all()` returns a consistent snapshot.
- `registry.byId("hello")` returns the HelloDriver handle; `registry.byId("nonsense")` returns null.

#### Tasks

1. Create `driver/DriverRegistry.kt`:
   - Internal state: `private val handles = ConcurrentHashMap<String, DriverHandle>()`
   - `fun start(context)`: calls `DriverManifestLoader.loadAll`, for each manifest calls `instantiateDriver(manifest)`, builds a `DriverHandle`, adds to map. Emits `RegistryEvent.Added(handle)` on the event flow.
   - `private fun instantiateDriver(m: DriverManifest): AppDriver?`: uses `Class.forName(m.driver_class).getDeclaredConstructor().newInstance() as AppDriver`. Catches `ClassNotFoundException`, `NoSuchMethodException`, `InstantiationException`, `ClassCastException` — logs each distinctly.
   - `fun all(): List<DriverHandle>` returns `handles.values.toList()`.
   - `fun byId(id): DriverHandle?` returns `handles[id]`.
   - `fun observe(): Flow<RegistryEvent>` — `MutableSharedFlow<RegistryEvent>(replay=0, extraBufferCapacity=32)` shared for CapabilityRegistry + HeartbeatScheduler.
2. Create `RegistryEvent` sealed class: `Added(handle)`, `Removed(driverId)`, `StateChanged(driverId, from, to)`.
3. Wire `AgentForegroundService.onCreate` to:
   ```kotlin
   driverRegistry = DriverRegistry().also { it.start(applicationContext) }
   ```
   Place AFTER `LocalHttpServer.start()` and `CommsLinkClient.connect()` so the HTTP `/capability` endpoint is ready to serve the registry contents immediately.
4. 432-03 deliberately SKIPS calling `install()` on drivers. That happens in 432-05. For 432-03, the DriverHandle's `state` starts as `DriverState.NotInstalled` and stays there.
5. Unit tests in `driver/DriverRegistryTest.kt`:
   - Registry with 0 manifests starts with `all().isEmpty()`.
   - Registry with 2 valid manifests produces 2 handles in `all()`.
   - Registry with a manifest whose class does not exist logs ERROR and produces a 1-handle result (not 2) — the other driver loads successfully (partial failure is survivable).
   - `byId` returns handle for existing, null for missing.
   - Event flow emits `Added` per handle.

#### Acceptance

- Unit tests pass.
- `adb install` the APK, launch, then `curl http://<device_ip>:8090/health` — observe no crash from registry boot (even though /capability is not yet wired up).
- `adb logcat | grep RcAgent` shows `driver_registered driver_id=hello` and `driver_registered driver_id=crash-test` during boot.
- `adb shell dumpsys activity services in.racingpoint.rcagentmobile` confirms service still foreground + started.

#### G4 NOT TESTED list

- `/capability` endpoint body (wired in 432-06).
- Driver install lifecycle (432-05).
- Per-driver scope isolation (432-04).

#### Commit message

```
feat(432-03): DriverRegistry — boot-time manifest load + reflective instantiation

AgentForegroundService.onCreate now starts the DriverRegistry.  Registry
scans bundled manifests, instantiates each driver via reflection
(no-arg constructor), holds handles in a concurrent map.  Partial-failure
tolerant: a missing driver class does not stop sibling drivers from loading.
RegistryEvent SharedFlow feeds future CapabilityRegistry + HeartbeatScheduler
reactivity (wired in 432-06).  Does NOT call install() yet — that is 432-05.

Covers: DRIVER-02, DRIVER-03 (partial — no lifecycle yet)
Not tested: lifecycle hooks (432-05), /capability body (432-06).
```

---

### 432-04-PLAN — Per-driver supervised CoroutineScope + ExceptionHandler

**Goal:** Isolate driver execution so an uncaught exception in driver A never propagates to the agent or driver B. This is the correctness heart of DRIVER-05.

**Covers:** DRIVER-05 (isolation)

**Dependencies:** 432-03

**Type:** `auto` + `tdd="true"`

#### Behavior

- Every driver runs inside its own `CoroutineScope(SupervisorJob() + Dispatchers.Default + CoroutineExceptionHandler { _, t -> ... })`.
- When a driver throws an uncaught exception, the handler logs `driver_crashed` and updates `DriverHandle.state = DriverState.Crashed(t, now)`.
- The agent's main scope (service scope) is UNAFFECTED — agent HTTP server, comms-link client, heartbeat scheduler keep running.
- Sibling drivers' scopes are UNAFFECTED — they continue to receive healthCheck ticks.
- A crashed driver is NOT automatically restarted by 432 — recovery is Phase 436's feature-flag-triggered re-install. 432-04 just ensures isolation; restart is out of scope.

#### Why SupervisorJob + CoroutineExceptionHandler (both are needed)

- A plain `Job()` propagates child cancellations UP the tree. An uncaught exception in a child would cancel siblings too.
- `SupervisorJob()` blocks upward propagation but does NOT swallow the exception — without a `CoroutineExceptionHandler`, the exception goes to `Thread.UncaughtExceptionHandler` which on Android crashes the process.
- Together: upward propagation blocked + handler swallows the exception = true isolation.

#### Tasks

1. Create `driver/DriverScope.kt`:
   ```kotlin
   object DriverScope {
       fun create(driverId: String, onCrash: (Throwable) -> Unit): CoroutineScope {
           val exceptionHandler = CoroutineExceptionHandler { _, t ->
               RotatingLog.error("driver", "driver_crashed",
                   mapOf("driver_id" to driverId, "exception" to t.toString(), "stack" to t.stackTraceToString()))
               onCrash(t)
           }
           return CoroutineScope(
               SupervisorJob() +
               Dispatchers.Default +
               exceptionHandler +
               CoroutineName("driver-$driverId")
           )
       }
   }
   ```
2. `DriverRegistry.instantiateDriver` now also calls `DriverScope.create(id) { t -> handles[id]?.markCrashed(t) }` and stores the scope in the handle.
3. `DriverHandle` exposes `suspend fun launch(block: suspend CoroutineScope.() -> Unit)` — shorthand for `scope.launch { block() }` — this is how LifecycleDispatcher (432-05) fires hooks.
4. Add `DriverHandle.markCrashed(t)` — updates internal `MutableStateFlow<DriverState>` to `Crashed(t, System.currentTimeMillis())` and cancels any remaining children in that scope.
5. Clean shutdown: `DriverRegistry.close()` calls `scope.cancel()` on every handle — reached in `AgentForegroundService.onDestroy`.
6. Instrumented-style unit test (JVM with `runTest` + `StandardTestDispatcher`) in `driver/DriverScopeIsolationTest.kt`:
   - Test A: a driver launches a coroutine that throws. Assert:
     - the driver's state becomes `Crashed`
     - a SIBLING driver's scope is still active (`scope.isActive == true`)
     - the parent service scope (a third scope in the test) is still active
   - Test B: 3 drivers. Drivers 1 and 3 run looping coroutines that never throw. Driver 2 throws on iteration 5. Advance virtual time. Assert drivers 1 and 3 complete their loops, driver 2 is `Crashed`.
   - Test C: `DriverRegistry.close()` cancels all driver scopes (assert `isActive == false` for each) and the service scope is still active.

#### Acceptance

- `DriverScopeIsolationTest` all three tests pass.
- `./gradlew :app:testDebugUnitTest --tests '*DriverScopeIsolationTest*'` exits 0.
- `adb install` the APK, launch, then force an exception in a test path (see 432-08 for the real adb-based verification). The process does not crash.

#### G4 NOT TESTED list

- Full crash-driver isolation on a physical device (432-08 exercises this).
- Automatic restart of crashed drivers (OUT OF SCOPE for v50.0; Phase 436 decides policy).

#### Risks

- **Risk (memory leak if driver holds strong reference to Context):** Drivers receive only `applicationContext` (not Service), so cancelling the scope releases all driver-owned references. Documented in DRIVER-FRAMEWORK.md.
- **Risk (CoroutineExceptionHandler only catches TOP-LEVEL children):** If a driver internally does `withContext(Dispatchers.IO) { throw ... }` INSIDE a `try { } catch { }`, the exception is caught there; otherwise it bubbles to the handler. If a driver uses `async { }` without `.await()`, the exception is swallowed until await — this is a driver-author mistake, not a framework bug. Call out in DRIVER-FRAMEWORK.md.

#### Commit message

```
feat(432-04): per-driver SupervisorJob + CoroutineExceptionHandler

Every driver runs in its own CoroutineScope(SupervisorJob() + Default +
CoroutineExceptionHandler + CoroutineName).  Uncaught exceptions in driver A
are caught by A's handler, state flipped to Crashed, but agent core and
siblings unaffected.  Clean shutdown via DriverRegistry.close().

Tested: 3-driver isolation scenario with a deliberate throw in middle driver.

Covers: DRIVER-05 (isolation — unit-level)
Not tested: adb physical crash-driver test (432-08).
```

---

### 432-05-PLAN — LifecycleDispatcher: install / onAppUpdate / healthCheck / uninstall

**Goal:** Fire driver lifecycle hooks deterministically. State machine governs transitions; timer runs healthCheck every 5 minutes per installed driver; PackageMonitor routes package-update broadcasts to `onAppUpdate`; feature-flag-off (Phase 436 hook, stubbed in 432 via a local dev API) triggers `uninstall`.

**Covers:** DRIVER-04 (lifecycle hooks deterministic)

**Dependencies:** 432-01, 432-02, 432-03, 432-04

**Type:** `auto` + `tdd="true"`

#### Behavior

- On first boot, every `enabled_by_default: true` driver's state goes `NotInstalled → Installing → Installed` via `driver.install(ctx)`.
- Every 300 seconds (±30s jitter), for every `Installed` driver, `healthCheck()` is called with a 30s timeout. Result updates `DriverState.Installed.lastHealth`.
- When `android.intent.action.PACKAGE_REPLACED` fires with a package matching any driver's `target_package`, `onAppUpdate(oldVer, newVer, ctx)` is invoked on that driver. Version codes are read via `PackageManager.getPackageInfo`. Dedupe window 30s prevents double-fire on rapid reinstalls.
- A local dev intent `in.racingpoint.rcagentmobile.ACTION_TOGGLE_DRIVER` with extras `{driver_id, enabled}` flips the driver — on enabled=false, transitions `Installed → Uninstalling → NotInstalled` calling `driver.uninstall()`. Phase 436 will replace this intent with real flag push; for 432 the dev intent is enough.

#### State machine (explicit)

```
        NotInstalled
            | install()
            v
        Installing  --(driver.install ok)-->  Installed
            |                                    |
            | install fails                      | PACKAGE_REPLACED
            v                                    v
          Crashed                              Updating
                                                 |
                                                 | driver.onAppUpdate ok
                                                 v
                                              Installed
                                                 |
                                                 | toggle off
                                                 v
                                             Uninstalling
                                                 |
                                                 | driver.uninstall ok
                                                 v
                                           NotInstalled
```

Every transition is logged via `RotatingLog.info("driver", "state_transition", {driver_id, from, to})`.

#### Tasks

1. Create `driver/LifecycleDispatcher.kt`:
   - Constructor: `DriverRegistry`, `DriverContext` factory (builds per-driver DriverContext with shared AccessibilityBridge, httpClient, json, log), `CoroutineScope` (the SERVICE scope, NOT a driver scope — the dispatcher itself is service-level so it can orchestrate across drivers).
   - `fun installAllEnabled()`: iterates registry, for each handle whose manifest has `enabled_by_default = true`, calls `install(handle)`.
   - `suspend fun install(handle)`: checks device-type compatibility first (calls 432-07 check); if incompatible → state `NotInstalled` + log SkippedUnsupportedDeviceType; else sets state `Installing`, launches `handle.launch { driver.install(ctx) }`, on success sets `Installed(since=now, lastHealth=Unknown)`.
   - `suspend fun uninstall(driverId)`: similar flow to Uninstalling → NotInstalled.
   - `fun startHealthCheckTicker()`: launches on service scope a `while(isActive) { delay(300_000 + Random.nextLong(0, 30_000)); runHealthChecks() }` loop.
   - `private suspend fun runHealthChecks()`: for each Installed handle, `withTimeoutOrNull(30_000) { handle.driver.healthCheck() } ?: HealthStatus.Unhealthy("healthCheck timed out", now)`. Update state in place. Emit `RegistryEvent.StateChanged`.
   - `fun handleAppUpdate(pkgName, oldVer, newVer)`: find handles with matching `target_package`; for each Installed, transition to Updating, call `onAppUpdate`, back to Installed on success.
2. Create `driver/PackageMonitor.kt`:
   - `class PackageMonitor(private val dispatcher: LifecycleDispatcher) : BroadcastReceiver()`
   - `onReceive(ctx, intent)`: filter by action `PACKAGE_REPLACED` or `PACKAGE_ADDED`, extract package name from `intent.data?.schemeSpecificPart`, read current + previous version codes via `PackageManager`, dedupe on `(pkg, newVer)` with 30s TTL, call `dispatcher.handleAppUpdate(pkg, old, new)`.
   - Dedupe cache: simple `LinkedHashMap<Pair<String, Long>, Long>` with 30s expiry.
3. `AndroidManifest.xml` additions:
   - `<receiver android:name=".driver.PackageMonitor" android:exported="false">`
     - Intent filter: `android.intent.action.PACKAGE_REPLACED`, `android.intent.action.PACKAGE_ADDED`, with `<data android:scheme="package" />`.
   - Register dynamically in `AgentForegroundService.onCreate` (Android 8+ restricts implicit PACKAGE_* broadcasts to dynamic registrations for unprivileged apps — this is the only correct path).
4. Local dev toggle intent: `AgentForegroundService` registers a `LocalBroadcastManager` listener for `in.racingpoint.rcagentmobile.ACTION_TOGGLE_DRIVER` with extras `{driver_id, enabled}`. Calls `dispatcher.install(...)` or `dispatcher.uninstall(...)`. Command-line invocation for testing:
   ```
   adb shell am broadcast -a in.racingpoint.rcagentmobile.ACTION_TOGGLE_DRIVER --es driver_id hello --ez enabled false
   ```
5. Wire `AgentForegroundService.onCreate`:
   ```kotlin
   lifecycleDispatcher = LifecycleDispatcher(driverRegistry, driverContextFactory, serviceScope)
   lifecycleDispatcher.installAllEnabled()
   lifecycleDispatcher.startHealthCheckTicker()
   registerReceiver(PackageMonitor(lifecycleDispatcher), packageFilter)
   ```
6. Unit tests in `driver/LifecycleDispatcherTest.kt`:
   - TDD matrix (state transitions):
     - NotInstalled → Installing → Installed (happy path)
     - NotInstalled → Installing → Crashed (when driver.install throws)
     - Installed → Updating → Installed (onAppUpdate)
     - Installed → Uninstalling → NotInstalled
     - Installed → (timeout in healthCheck) → Installed with `lastHealth = Unhealthy("timeout")`
   - Ticker fires exactly once per `delay(300_000)` using virtual time.
   - 3 Installed drivers → each receives exactly 1 healthCheck call per tick.
   - PackageMonitor dedupe: two `PACKAGE_REPLACED` broadcasts within 30s for the same pkg+ver trigger ONE onAppUpdate.

#### Acceptance

- All lifecycle unit tests pass.
- `./gradlew :app:testDebugUnitTest --tests '*LifecycleDispatcher*'` exits 0.
- adb smoke test: `adb install` APK, wait 10s, `adb logcat -d | grep state_transition` shows `hello: NotInstalled -> Installing -> Installed`.
- adb toggle test: `adb shell am broadcast -a in.racingpoint.rcagentmobile.ACTION_TOGGLE_DRIVER --es driver_id hello --ez enabled false`. Within 10s, logcat shows `hello: Installed -> Uninstalling -> NotInstalled`.
- 20-minute adb logcat capture (run script): asserts ~4 healthCheck ticks for `hello` (1 per 5 minutes).

#### Risks

- **Ticker drift over long runs:** `delay(300_000)` on a coroutine scheduler is accurate enough for healthCheck (seconds, not subseconds). `Dispatchers.Default` is sufficient.
- **PACKAGE_REPLACED on the agent's OWN package:** filter out in PackageMonitor (compare `pkgName != context.packageName`). Otherwise an agent self-update would fire onAppUpdate on every driver with a matching target_package — not what we want.
- **healthCheck() blocks forever:** `withTimeoutOrNull(30_000)` forces a result. The coroutine is cancelled cooperatively — drivers must respect cancellation. Documented in DRIVER-FRAMEWORK.md.

#### Commit message

```
feat(432-05): LifecycleDispatcher — deterministic install/update/healthCheck/uninstall

State machine: NotInstalled <-> Installing <-> Installed <-> Updating
                                                   <-> Uninstalling <-> NotInstalled
                                                   (+ Crashed terminal with recovery in Phase 436)
Ticker runs healthCheck every 300s +/- 30s jitter with 30s timeout per driver.
PackageMonitor dispatches PACKAGE_REPLACED to onAppUpdate with 30s dedupe.
Local dev toggle intent triggers install/uninstall for testing (Phase 436
replaces with real flag push).

Covers: DRIVER-04
Not tested: crash-driver isolation E2E on device (432-08); sample driver
(432-09); supported_device_types enforcement (432-07).
```

---

### 432-06-PLAN — CapabilityRegistry: publish in register + heartbeat + /capability

**Goal:** Every device's driver list is published in its comms-link registration, included in heartbeat (digest-form for cheap change detection), exposed via `/capability` HTTP endpoint, and persisted to disk so it survives agent restart.

**Covers:** CAPREG-01 (in registration), CAPREG-02 (queryable from admin via relay forwarding)

**Dependencies:** 432-01, 432-03, 432-05, 429-03 (HTTP server), 429-05 (CommsLinkClient)

**Type:** `auto`

#### Behavior

- `CapabilityRegistry.snapshot()` returns a JSON-serializable list of `CapabilityEntry` reflecting the current `DriverRegistry` state.
- The list is persisted to `context.filesDir/capability-registry.json` on every change (atomic write via temp+rename).
- `GET /capability` returns the current snapshot as `application/json`.
- The comms-link `register` envelope includes the full snapshot in `payload.capabilities`.
- Every heartbeat includes `capabilities_digest = sha256(snapshot_json)`. If the relay sees a digest change, Phase 13/14 (admin) can re-fetch the full list via a new relay-routed `GET /capability` probe.
- When `registry.observe()` emits a `RegistryEvent` (Added, Removed, StateChanged), the CapabilityRegistry recomputes snapshot and persists.

#### Tasks

1. Create `driver/CapabilityRegistry.kt`:
   - Constructor: `DriverRegistry`, `filesDir: File`, `scope: CoroutineScope`.
   - On init: load `capability-registry.json` if exists (for diagnostics only — the live registry is authoritative).
   - Subscribe to `registry.observe()` in a coroutine on `scope`; on each event, recompute + persist + emit on own `MutableSharedFlow<List<CapabilityEntry>>`.
   - `fun snapshot(): List<CapabilityEntry>` — maps every `DriverHandle` to a `CapabilityEntry`, reflecting current `DriverState`.
   - `fun digest(): String` — SHA256 of `Json.encodeToString(snapshot())`.
   - `private fun persist(list)` — writes `{filesDir}/capability-registry.json.tmp` then renames to `capability-registry.json` atomically.
2. `CapabilityEntry.state` string mapping:
   - `DriverState.NotInstalled` → `"not_installed"`
   - `DriverState.Installing` → `"installing"`
   - `DriverState.Installed` → `"installed"`
   - `DriverState.Updating` → `"updating"`
   - `DriverState.Uninstalling` → `"uninstalling"`
   - `DriverState.Crashed` → `"crashed"`
   - Special case: driver skipped because device type unsupported → `"unsupported_device_type"` (state never enters the state machine)
   - Special case: driver skipped because reserved type → `"reserved_type_not_installed"` (CAPREG-04 honoring)
3. `LocalHttpServer.kt` extension:
   - Inject `CapabilityRegistry` via constructor.
   - `get("/capability") { call.respond(capabilityRegistry.snapshot()) }`.
   - Remove the stub from 429-03 that returned `{"capabilities":[],"supported_device_types":["tablet"]}` — replaced by live registry.
4. `CommsLinkClient.kt` extension:
   - `register()` payload now: `RegisterPayload(..., capabilities = capabilityRegistry.snapshot())`.
   - On `RegistryEvent`, queue a re-`register` envelope (or a dedicated `capability_update` message type — introduced in PROTOCOL.md amendment below).
5. `HeartbeatScheduler.kt` extension:
   - Every 30s tick: `HeartbeatPayload(..., capabilities_digest = capabilityRegistry.digest())`.
6. `docs/PROTOCOL.md` amendment:
   - New message type `capability_update` (Android → relay) with payload `{capabilities: [...CapabilityEntry]}`. Sent on RegistryEvent.
   - Heartbeat schema update: add `capabilities_digest` field.
   - Bump doc minor version, increment note "Phase 432 added capability payload to register + heartbeat (forward-compat — relays ignoring the field still work)".
7. `AgentForegroundService.onCreate`:
   ```kotlin
   capabilityRegistry = CapabilityRegistry(driverRegistry, applicationContext.filesDir, serviceScope)
   localHttpServer = LocalHttpServer(8090, deviceState, capabilityRegistry)
   commsClient = CommsLinkClient(deviceState, capabilityRegistry, serviceScope)
   heartbeatScheduler = HeartbeatScheduler(commsClient, deviceState, capabilityRegistry, serviceScope)
   ```
8. Unit tests in `driver/CapabilityRegistryTest.kt`:
   - Snapshot reflects registry state.
   - State change in registry triggers snapshot update + persistence.
   - `digest()` stable when state stable.
   - `digest()` changes after RegistryEvent.
   - Persist-and-reload round-trip matches.

#### Acceptance

- Unit tests pass.
- `adb install`, launch, `curl http://<device_ip>:8090/capability` returns a JSON array containing `{driver_id:"hello", state:"installed", health_status:"unknown", ...}`.
- comms-link relay log (James .27 + Bono VPS) shows a `register` with non-empty `capabilities`.
- Toggle hello off via adb: within 10s, `/capability` reflects `state:"not_installed"`, relay receives `capability_update` message (or re-register), heartbeat digest changes.
- File `/data/data/in.racingpoint.rcagentmobile/files/capability-registry.json` exists and matches `/capability` output after install.

#### G4 NOT TESTED list

- Admin dashboard display of capabilities (Phase 14).
- Relay forwarding of `capability_update` (relay-side, Phase 13/14 subtask).
- `supported_device_types` enforcement under test (432-07).

#### Commit message

```
feat(432-06): CapabilityRegistry — publish in register + heartbeat + /capability

Every RegistryEvent recomputes a snapshot of driver state, persists to
filesDir/capability-registry.json (atomic write), and emits to subscribers.
LocalHttpServer.capability route returns the live snapshot.
CommsLinkClient.register payload includes full capabilities list.
HeartbeatScheduler includes capabilities_digest (SHA256) for cheap
change detection.  PROTOCOL.md amended with capability_update message type.

Covers: CAPREG-01, CAPREG-02
Not tested: admin dashboard view (Phase 14), device-type enforcement (432-07).
```

---

### 432-07-PLAN — supported_device_types enforcement

**Goal:** A tablet-only driver installed on a phone is REFUSED at install time and shows `unsupported_device_type` in `/capability`. Reserved types (`smart_display`, `ps5_tablet`, `kiosk_phone`) are also refused on current devices BUT as `reserved_type_not_installed` (CAPREG-04 honor).

**Covers:** CAPREG-03 (enforcement), CAPREG-04 (reserved slot honor)

**Dependencies:** 432-01, 432-02, 432-05

**Type:** `auto` + `tdd="true"`

#### Behavior

- `LifecycleDispatcher.install(handle)` first resolves the device type via `DeviceTypeResolver.resolve(ctx)`.
- If the resolved type is NOT in `manifest.supported_device_types`:
  - If the intersection is among `{smart_display, ps5_tablet, kiosk_phone}` (all reserved) → state `reserved_type_not_installed` + log WARN `driver_skipped_reserved_type`.
  - Else → state `unsupported_device_type` + log WARN `driver_skipped_unsupported_device_type`.
- Either way, `InstallResult` returns a distinct Skipped variant so CapabilityRegistry can surface the right state string.

#### Tasks

1. Amend `LifecycleDispatcher.install(handle)`:
   ```kotlin
   val deviceType = deviceTypeResolver.resolve(ctx)
   val supportedLC = manifest.supported_device_types
       .mapNotNull { parseDeviceType(it) }
   if (deviceType !in supportedLC) {
       val allReserved = supportedLC.all { it in reservedTypes }
       if (allReserved) {
           handle.markReservedTypeNotInstalled(supportedLC)
           return InstallResult.SkippedReservedType(supportedLC.first())
       }
       handle.markUnsupportedDeviceType(supportedLC)
       return InstallResult.SkippedUnsupportedDeviceType(supportedLC, deviceType)
   }
   // ... existing install path
   ```
2. `DriverHandle` gains two states that are NOT in the state machine proper — they are terminal diagnostic states used only for reporting:
   - `markUnsupportedDeviceType(types)` sets a private flag; `CapabilityRegistry` checks this flag and emits `state: "unsupported_device_type"`.
   - `markReservedTypeNotInstalled(types)` similar, emits `state: "reserved_type_not_installed"`.
3. `CapabilityEntry` JSON includes `supported_device_types` so the admin dashboard can show "this driver needs a smart_display" without fetching the manifest.
4. Add a third manifest for testing: `assets/drivers/tablet-only-test.json` with `supported_device_types: ["tablet"]` and `driver_class: ...sample.HelloDriver` (reuses HelloDriver class, just a different manifest to avoid duplicating code).
5. Add a fourth manifest for testing: `assets/drivers/smart-display-only-test.json` with `supported_device_types: ["smart_display"]` (reserved; both Tab Plus and M07 should skip it).
6. Update `assets/drivers.json` to include the two test manifests.
7. Unit tests in `driver/DeviceTypeEnforcementTest.kt`:
   - Fake DeviceTypeResolver returning `Phone`, install `tablet-only-test` manifest → `SkippedUnsupportedDeviceType`.
   - Fake DeviceTypeResolver returning `Tablet`, install `tablet-only-test` → `Installed`.
   - Fake DeviceTypeResolver returning `Tablet`, install `smart-display-only-test` → `SkippedReservedType`.
   - Fake DeviceTypeResolver returning `Phone`, install `smart-display-only-test` → `SkippedReservedType` (device type is phone but the reserved check takes precedence because the supported list contains ONLY reserved types).

#### Acceptance

- Unit tests all pass.
- Build APK, install on Tab Plus (tablet): `/capability` shows `tablet-only-test: installed`, `smart-display-only-test: reserved_type_not_installed`.
- Install same APK on M07 (phone): `/capability` shows `tablet-only-test: unsupported_device_type`, `smart-display-only-test: reserved_type_not_installed`.
- Logcat shows correct WARN messages.

#### G4 NOT TESTED list

- Per-manifest field `requires_accessibility` = true blocks install if Accessibility not enabled (Phase 430's job; 432 doesn't add this).

#### Commit message

```
feat(432-07): supported_device_types enforcement + reserved-type honor

LifecycleDispatcher.install resolves device type first; if not in manifest's
supported_device_types, returns SkippedUnsupportedDeviceType or
SkippedReservedType (when the manifest's types are all reserved like
smart_display/ps5_tablet/kiosk_phone).  CapabilityRegistry surfaces these
states so admin UI (Phase 14) can show them.

Covers: CAPREG-03, CAPREG-04
Not tested: Accessibility-required enforcement (belongs to Phase 430 + 432
integration, deferred to Phase 433 gap if missing).
```

---

### 432-08-PLAN — CrashDriver isolation test (physical device, E2E)

**Goal:** Prove on a real device that when a driver deliberately crashes in its `healthCheck()`, (a) the agent process stays up, (b) sibling drivers continue to respond, (c) the crashed driver's state is visible in `/capability` as `crashed`. This is the DRIVER-05 ship test.

**Covers:** DRIVER-05 (isolation — E2E verification)

**Dependencies:** 432-04, 432-05, 432-06, 432-07, 432-01..02 (CrashDriver manifest)

**Type:** `auto` (automated via adb script) — NO human-verify checkpoint because the whole test is scripted; the pass/fail is determinate.

#### Behavior

- `CrashDriver.healthCheck()` throws `IllegalStateException("intentional crash for isolation test")`.
- At the next healthCheck tick (up to 5 minutes), CrashDriver's state flips to `Crashed`.
- Agent process PID is unchanged (process did not restart).
- HelloDriver's state remains `Installed`, `healthCheck()` keeps returning `Healthy`.
- HTTP `/capability` shows both drivers: hello installed+healthy, crash-test crashed.
- adb logcat shows the CoroutineExceptionHandler log with the right `driver_id`.

#### Tasks

1. `driver/sample/CrashDriver.kt`:
   ```kotlin
   class CrashDriver : AppDriver {
       override fun install(ctx: DriverContext) {
           ctx.log.info("install", mapOf("note" to "CrashDriver installed; will crash on first healthCheck"))
       }
       override fun onAppUpdate(oldVersionCode: Long, newVersionCode: Long, ctx: DriverContext) {
           /* no-op */
       }
       override suspend fun healthCheck(): HealthStatus {
           throw IllegalStateException("intentional crash for isolation test")
       }
       override fun uninstall() { /* no-op */ }
   }
   ```
2. Update `assets/drivers/crash-driver.json` with `enabled_by_default: true` so it gets installed at boot (so the test requires no manual toggle).
3. Script `scripts/test-432-08-isolation.sh` (bash, run on dev machine after `adb install`):
   - Step 1: `adb shell pidof in.racingpoint.rcagentmobile` — record PID as $PID_BEFORE.
   - Step 2: Wait 360 seconds (one full healthCheck tick + buffer).
   - Step 3: `adb shell pidof in.racingpoint.rcagentmobile` — record PID as $PID_AFTER. Assert $PID_BEFORE == $PID_AFTER (no process restart).
   - Step 4: `curl http://<device_ip>:8090/capability > cap.json`.
   - Step 5: `jq -e '.[] | select(.driver_id=="hello") | .state=="installed" and .health_status=="healthy"' cap.json`. Expect exit 0.
   - Step 6: `jq -e '.[] | select(.driver_id=="crash-test") | .state=="crashed"' cap.json`. Expect exit 0.
   - Step 7: `adb logcat -d | grep 'driver_crashed' | grep 'crash-test'` — expect ≥1 match.
   - Step 8: `adb logcat -d | grep 'driver_crashed' | grep 'hello'` — expect 0 matches (HelloDriver must NOT have crashed).
4. Run the script as part of 432-08 acceptance. Save output to `.planning/phases/432-driver-framework-capability-registry/isolation-test-evidence.log` for the SUMMARY.

#### Acceptance

- All 8 assertions in `test-432-08-isolation.sh` pass.
- Evidence log committed.

#### G4 NOT TESTED list

- Recovery from crashed state (restart policy is Phase 436 + 443 territory).
- Long-running stability across hours of crashes (deferred to v50.0 E2E phase 444).

#### Commit message

```
test(432-08): CrashDriver E2E isolation proof on device

Deliberately-crashing driver's healthCheck throws IllegalStateException.
After one 5-min healthCheck tick: agent process PID unchanged, HelloDriver
remains Installed+Healthy, crash-test in Crashed state, CoroutineExceptionHandler
log present, no spillover to sibling.  Evidence log in phase dir.

Covers: DRIVER-05 (E2E verification)
```

---

### 432-09-PLAN — HelloDriver end-to-end plug-in contract proof

**Goal:** Prove the claim "adding a new driver = drop a module + manifest entry, zero core changes" by doing exactly that with HelloDriver. Executor must NOT touch any `driver/*.kt` file except to ADD `sample/HelloDriver.kt`. All other files in `files_modified` are from prior plans.

**Covers:** DRIVER-03 (E2E demonstration), DRIVER-01 (contract adhered-to)

**Dependencies:** 432-01..07

**Type:** `auto`

#### Behavior

- HelloDriver implements AppDriver with minimal no-op logic that logs each lifecycle event.
- `GET /capability` shows HelloDriver with `state: "installed"`, `health_status: "healthy"`.
- After toggle off: state `not_installed`, `/capability` reflects this within 10s.

#### Tasks

1. `driver/sample/HelloDriver.kt`:
   ```kotlin
   class HelloDriver : AppDriver {
       private var installedAt: Long = 0

       override fun install(ctx: DriverContext) {
           installedAt = System.currentTimeMillis()
           ctx.log.info("install", mapOf("message" to "hello from HelloDriver"))
       }
       override fun onAppUpdate(oldVersionCode: Long, newVersionCode: Long, ctx: DriverContext) {
           ctx.log.info("onAppUpdate", mapOf("old" to oldVersionCode, "new" to newVersionCode))
       }
       override suspend fun healthCheck(): HealthStatus {
           return HealthStatus.Healthy
       }
       override fun uninstall() {
           // no-op
       }
   }
   ```
2. Self-audit script `scripts/audit-432-09-no-core-changes.sh`:
   - `git diff HEAD~1 --name-only -- 'rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/*.kt'` — must return ONLY `sample/HelloDriver.kt` and NOT `AppDriver.kt`, `DriverRegistry.kt`, `LifecycleDispatcher.kt`, etc.
   - `git diff HEAD~1 --name-only -- 'rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/*.kt'` — must return nothing.
   - `git diff HEAD~1 --name-only -- 'rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/*.kt'` — must return nothing.
   - `git diff HEAD~1 --name-only -- 'rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/*.kt'` — must return nothing.
3. Run the script during 432-09 acceptance. If it fails, the abstraction is leaking and 432 is not ready to ship — treat as a design regression.
4. Runtime test: `adb install`, wait 10s, `curl http://<device_ip>:8090/capability | jq '.[] | select(.driver_id=="hello") | .state'` → `"installed"`.
5. Toggle test: `adb shell am broadcast -a in.racingpoint.rcagentmobile.ACTION_TOGGLE_DRIVER --es driver_id hello --ez enabled false`. Within 10s, `/capability` shows `state: "not_installed"`.
6. Re-enable: same broadcast with `--ez enabled true`. Within 10s, `/capability` shows `state: "installed"` again.

#### Acceptance

- `scripts/audit-432-09-no-core-changes.sh` exits 0.
- HelloDriver shows up in `/capability` after fresh install.
- Toggle off/on cycle completes in < 10s each direction.

#### G4 NOT TESTED list

- Long-running stability of HelloDriver (covered by v50.0 Phase 444 E2E).
- Real-world driver behavior (HelloDriver is a no-op; Zomato driver 437 is where real logic appears).

#### Commit message

```
test(432-09): HelloDriver — end-to-end plug-in contract proof

Adds sample/HelloDriver.kt (50 LOC no-op).  No changes to any core file
(verified by audit-432-09-no-core-changes.sh).  Driver loads at boot,
appears in /capability, toggles off/on via adb broadcast.  Proves the
claim "new driver = new module + manifest, zero core changes" (DRIVER-03).

Covers: DRIVER-01 (contract adhered-to), DRIVER-03 (E2E demo)
```

---

## 6. Risks and pitfalls (framework-specific)

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **Wrong interface shape calcifies across every future driver.** A regrettable choice in `AppDriver` or `DriverContext` becomes hard to change after 5+ drivers implement it. | Interface-first (432-01) lands BEFORE any implementation. MMA audit with dual reasoning modes (required gate, §8) runs AFTER 432-01 lands and BEFORE 432-03+ so corrections are cheap. |
| R-2 | **Coroutine scope semantics.** Plain `Job()` propagates child cancellation up the tree — one exception kills siblings. | 432-04 explicitly uses `SupervisorJob() + CoroutineExceptionHandler`. Isolation test (432-08) is the ship gate. |
| R-3 | **Memory leak if drivers hold strong refs to Activity/Service Context.** | `DriverContext.applicationContext` only. Documented in DRIVER-FRAMEWORK.md + asserted in 432-01 init block. |
| R-4 | **Lifecycle ordering bugs.** onAppUpdate firing before install() on a fresh install. | State machine (432-05) has explicit transitions with unit-test coverage for every edge. |
| R-5 | **healthCheck blocks indefinitely.** A driver's healthCheck() that never returns stalls the ticker for all drivers. | `withTimeoutOrNull(30_000)` per driver. Drivers that don't respect cancellation are a driver-author bug, not a framework bug — but still each healthCheck runs in the driver's scope, so a hung one does not block siblings. |
| R-6 | **Manifest typo silently changes semantics.** `"supported_device_types": ["tablt"]` parses and runs "anywhere". | Strict validator (432-02) with enum whitelist. Unknown values are REJECTED at load time. |
| R-7 | **Remote-pushed manifest trust boundary.** Phase 436/443 adds remote push; signature verification is their job. | 432 loads ONLY from APK assets. Documented boundary in DRIVER-FRAMEWORK.md §7 "What 432 does NOT own". |
| R-8 | **PackageMonitor spurious fires on agent self-update.** | Filter `pkgName != context.packageName` in PackageMonitor.onReceive. |
| R-9 | **Reflection instantiation fails at runtime** (ProGuard strips, class not found). | (a) Add `-keep class in.racingpoint.rcagentmobile.driver.sample.** { *; }` to `proguard-rules.pro` so driver classes survive minification. (b) Fail gracefully in `DriverRegistry.instantiateDriver` with distinct log lines per exception class. (c) 432-09 audit script catches regressions. |
| R-10 | **/capability endpoint leaks internal state.** An attacker on LAN could enumerate capabilities to identify pod types. | /capability is currently PUBLIC — consistent with 429-03's /health + /build_id. If later deemed too revealing, move behind service-key header (pattern from CLAUDE.md "Pod HTTP endpoints default to protected"). Flag for Phase 436 review. |
| R-11 | **Heartbeat capability digest implementation** — SHA256 of unsorted JSON → non-stable digest. | Use `Json { encodeDefaults = true; prettyPrint = false }` with deterministic field ordering (kotlinx-serialization preserves declaration order). Unit test asserts same snapshot → same digest across 100 iterations. |
| R-12 | **reserved_type_not_installed handling.** Future-type manifests MUST NOT break current devices. | 432-07 tests both paths (unsupported vs reserved) explicitly. Honor preserves CAPREG-04 semantics. |
| R-13 | **Ticker jitter accumulation.** Over 24h, does `delay(300_000 + jitter)` drift significantly? | No — jitter is ±30s per tick, not cumulative. Total drift over 24h is O(jitter), not O(N*jitter). |

## 7. Test plan

### Unit tests (JVM, fast, on every build)

- `AppDriverContractTest` (432-01) — interface compile + HealthStatus equality
- `DeviceTypeResolverTest` (432-01) — phone vs tablet + override slot
- `DriverManifestLoaderTest` (432-02) — 14+ validation rule tests
- `DriverRegistryTest` (432-03) — happy path + partial failure
- `DriverScopeIsolationTest` (432-04) — 3-driver isolation matrix
- `LifecycleDispatcherTest` (432-05) — all state transitions + healthCheck timeout + PackageMonitor dedupe
- `CapabilityRegistryTest` (432-06) — snapshot + digest stability + persist-and-reload
- `DeviceTypeEnforcementTest` (432-07) — 4 combinations (phone/tablet × supported/unsupported/reserved)

All unit tests run as part of `./gradlew :app:testDebugUnitTest`. Gradle task returns non-zero on any failure. Total count: ~50 tests.

### Instrumented tests (Android device, pre-release)

- `InstrumentedDriverLifecycleTest` — full boot-to-toggle cycle on emulator
- `InstrumentedIsolationTest` — CrashDriver in emulator environment

### adb-scripted tests (physical device)

- `scripts/test-432-08-isolation.sh` — 8-assertion E2E isolation drill
- `scripts/audit-432-09-no-core-changes.sh` — diff-gate proving no core files touched for new driver
- `scripts/test-432-health-check-timing.sh` — 20-min logcat capture, assert ~4 healthCheck ticks per driver

### MMA audit (gate)

- After 432-01 + 432-02 merge, run MMA audit on the interface design (not the full phase). Budget: $5. Dual reasoning modes required per CLAUDE.md. Focus areas: AppDriver method signatures, DriverContext injection, manifest schema completeness, supervisor scope correctness.

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** Lifecycle dispatcher state machine + capability registry wiring are business logic with defined I/O. Run `gsd-nyquist-auditor` before 432-09 lands.
- **MMA audit (required — this is the architectural keystone):** Covers the pluggable framework design before it calcifies into every driver phase that follows. **Run AFTER 432-02 lands and BEFORE 432-03 starts.** Dual reasoning modes REQUIRED. Budget $5. Explicit ask to the audit models: "Find architecture bugs in AppDriver + DriverContext + DriverManifest that will be expensive to fix after 10 drivers ship against them."
- **integration-checker (required):** Run before v50.0 milestone ship.
- **codebase-mapper (required):** Run AFTER 432-09 so the map includes `driver/` as a subsystem.
- **ui-researcher / ui-auditor:** Skip. No user-facing UI in 432.
- **SEC gate:** `node comms-link/test/security-check.js` must pass after 432-06 amends the PROTOCOL (new capability_update message type). Audit should verify no new auth bypass or info-disclosure vector.
- **DMP:** Captured in `deploy:` frontmatter. Executor ticks each item; verifier confirms.
- **Backlog gate:** 432 must reach DEPLOYED-VERIFIED (APK on both devices + 432-08 + 432-09 scripts pass) before 433-440 may begin. COMMITTED ≠ SHIPPED.

## 9. Open questions the planner cannot decide

**OQ-1 — Where do driver manifests live in the APK: `assets/` or `res/raw/`?**
Decision needed before 432-02 execution. `assets/` allows subfolder structure and filename-based discovery via `AssetManager.list("drivers")` — preferred for our use case. `res/raw/` is flatter and compiled into resource table (faster access, but no subfolder hierarchy). **Recommendation: `assets/drivers/*.json` with an index file `assets/drivers.json`.** This is what 432-02 implements unless user overrides.

**OQ-2 — Should bundled manifests be remotely overridable at boot?**
Phase 436 (feature flags) + Phase 443 (remote manifest push) own remote override. 432 MUST load from APK assets only. But question: should 432 load `filesDir/override-drivers.json` if it exists, as a forward-compat hook for 443? **Recommendation: NO — leave remote loading entirely to 443. A boot-time override channel here creates attack surface before the trust model is designed. 432 stays strict-APK-only.**

**OQ-3 — Reflective instantiation vs ServiceLoader vs compile-time registration?**
`Class.forName(manifest.driver_class).newInstance()` is reflection — simple, manifest-declarative, but ProGuard needs explicit keep rules. Alternative 1: Java's `ServiceLoader` — requires annotation-processing/KSP setup. Alternative 2: Compile-time registration via a generated `DriverRegistry.kt` — loses the "drop a file, zero core code changes" property. **Recommendation: reflection with ProGuard keep rules. Simplest correct answer for our fleet size. Document in DRIVER-FRAMEWORK.md.**

**OQ-4 — HealthCheck cadence: 5 minutes per requirement, but is it synchronous (one driver at a time) or parallel (all at once)?**
Sequential is simpler but a slow driver delays others. Parallel (launch per driver) is faster but risks resource contention. **Recommendation: parallel via `coroutineScope { drivers.forEach { launch { it.healthCheck() } } }` — each driver's healthCheck runs in its own scope anyway (per 432-04), and `withTimeoutOrNull(30_000)` bounds the worst case. Total tick duration = max(driver healthCheck duration), which is what we want.**

**OQ-5 — Should CrashDriver ship in release APK, or only in a `debug` build variant?**
Shipping in release means the isolation test can be run against production APKs (high confidence). But an unused crashing driver in production is a smell. **Recommendation: ship CrashDriver in ALL variants but with `enabled_by_default: false` by default. The 432-08 isolation test flips it on via the dev toggle broadcast for the test, then flips it off. For production, it never runs. Revisit in 432-08 if this feels wrong.**

**OQ-6 — `DriverContext.scope` vs driver creating its own scope.**
Drivers should use the injected `ctx.scope` for all coroutine work so cancellation on uninstall is automatic. Should we enforce this structurally or just document it? **Recommendation: document strongly in DRIVER-FRAMEWORK.md + add a lint rule (Detekt custom rule) that flags `GlobalScope.launch` in any `driver/*/` file. Structural enforcement is hard without a DSL — documentation + lint is pragmatic.**

**OQ-7 — Manifest pushed without a `version` field — fail closed (reject) or fail open (default "1.0.0")?**
**Recommendation: fail CLOSED. Missing required fields are a manifest author bug. Loud failure is better than silent wrong-version.**

**OQ-8 — Should `DriverState.Crashed` trigger an immediate `capability_update` message to comms-link, or only surface on the next 30s heartbeat?**
Immediate is more responsive (admin sees crashes within seconds). 30s heartbeat is simpler. **Recommendation: immediate via `capability_update` for state transitions `Installed → Crashed`; rely on heartbeat digest otherwise. One extra WS frame per crash, negligible cost, high observability value.**

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md`
- **Prior phase (interfaces):** `.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md` (Protocol envelope, RotatingLog, LocalHttpServer, CommsLinkClient, HeartbeatScheduler)
- **Prior phase (AccessibilityBridge contract):** `.planning/phases/430-accessibility-service-foundation/PLAN.md` — if not yet landed, 432-01 stubs the interface
- **Downstream consumers:** 433 (selectors DSL), 434 (credential strategies), 435 (humanize layer), 436 (feature flags), 437 (Zomato), 438 (HyperPure), 439 (Blinkit), 443 (remote push)
- **Reference: Rust trait-based plugin pattern for interface-shape validation only:** `crates/rc-agent/src/tier_engine.rs` (review IDEAS, do NOT import Rust patterns into Kotlin)
- **Project memory active work:** `project_v50_rc_agent_mobile.md`

## 11. Output (at phase close)

At the end of Plan 432-09 (HelloDriver E2E proof pass), create `.planning/phases/432-driver-framework-capability-registry/SUMMARY.md` capturing:

- Which commits implemented each plan (432-01 through 432-09)
- MMA audit findings + resolution log (required gate)
- nyquist-audit findings + resolution log
- Evidence artifacts from 432-08 (isolation-test-evidence.log) and 432-09 (audit-432-09-no-core-changes output)
- Actual `/capability` body from both Tab Plus and M07
- Test counts (unit + instrumented + adb-scripted), all green
- Any risks encountered and how they were resolved
- Any open questions resolved during execution (update §9 state)
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked
- Handoff to Phase 433 (Selector DSL) — driver contract is locked, selectors attach via Phase 433's DSL loaded in DriverContext

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 4 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule). Refresh `.planning/codebase/` via `gsd-codebase-mapper` as the final step before closing.
