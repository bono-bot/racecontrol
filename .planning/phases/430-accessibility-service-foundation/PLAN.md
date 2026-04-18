---
phase: 430-accessibility-service-foundation
phase_number: 430
milestone: v50.0 rc-agent-mobile
name: "Accessibility Service Foundation"
status: ready-to-execute
goal: >
  Kotlin Android agent runs an Accessibility Service that reads the full AccessibilityNodeInfo
  hierarchy of the foreground app and exposes it over HTTP as /screen/tree. The agent also
  dispatches tap, swipe, and text-input primitives by selector (resource-id, content-description,
  text, xpath) with 100ms retry on miss. When Accessibility Service is disabled, all UI action
  endpoints return HTTP 503 with a human-readable message and the persistent notification shows
  a warning state. A first-run Activity detects the disabled state, opens Android Settings ->
  Accessibility page, and polls until the user toggles the service on.
requirements: [ACCESS-01, ACCESS-02, ACCESS-03, ACCESS-04, ACCESS-05]
depends_on: [429-kotlin-scaffold-http-comms-link]
wave: 2
plan_count: 7
plans:
  - 430-01-PLAN: AccessibilityService subclass + manifest + lifecycle
  - 430-02-PLAN: Screen-tree reader + /screen/tree endpoint
  - 430-03-PLAN: Tap primitive via GestureDescription + selector engine + 100ms retry
  - 430-04-PLAN: Swipe + text-input primitives
  - 430-05-PLAN: 503 gate for disabled-Accessibility state + notification warning
  - 430-06-PLAN: First-run Activity + Settings deep-link + toggle poll
  - 430-07-PLAN: Unit + instrumented tests + Tab Plus + M07 E2E verification
autonomous: false # Plans 430-06 and 430-07 contain human-verify checkpoints (physical devices + Settings toggle)
files_modified:
  - rc-agent-mobile/app/src/main/AndroidManifest.xml
  - rc-agent-mobile/app/src/main/res/xml/accessibility_service_config.xml                           # new
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/RcAccessibilityService.kt # new
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/A11yBridge.kt              # new (singleton hand-off)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/ScreenTreeReader.kt        # new
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/ScreenNode.kt              # new (DTO)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/Selector.kt                # new (sealed + strategy enum)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/SelectorResolver.kt        # new (match + retry)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/GestureDispatcher.kt       # new
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/TextInputDispatcher.kt     # new
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/A11yStateMonitor.kt        # new (enabled/disabled detection)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/UiRoutes.kt                # new (Ktor route module)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/LocalHttpServer.kt         # MODIFIED (mount UiRoutes)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt # MODIFIED (notification state + listener)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/firstrun/AccessibilitySetupActivity.kt # new
  - rc-agent-mobile/app/src/main/res/layout/activity_accessibility_setup.xml                        # new
  - rc-agent-mobile/app/src/main/res/values/strings.xml                                             # MODIFIED (setup copy)
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/a11y/ScreenTreeReaderTest.kt   # new
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/a11y/SelectorResolverTest.kt   # new
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/a11y/GestureDispatcherTest.kt  # new
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/a11y/A11yStateMonitorTest.kt   # new
  - rc-agent-mobile/app/src/androidTest/kotlin/in/racingpoint/rcagentmobile/a11y/AccessibilityInstrumentedTest.kt # new
  - rc-agent-mobile/docs/PROTOCOL.md                                                                # MODIFIED (add UI action schema)
  - rc-agent-mobile/docs/ACCESSIBILITY-NOTES.md                                                     # new
  - .planning/phases/430-accessibility-service-foundation/SUMMARY.md                                # filled at end

# DMP - Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: >
    APK rebuilt and sideloaded via ADB to Tab Plus + M07 (same install path as Phase 429).
    Android Accessibility Service must be ENABLED per device in Settings ->
    Accessibility -> Installed services -> "RC Agent Mobile" -> toggle ON.
    This toggle is user-action-only (Android forbids programmatic enable without device-owner
    privileges, which require ADB dpm set-device-owner at install time — not planned for v50.0).
    First-run Activity automates the deep-link to the correct Settings page.
    Device firewall: no new ports opened (reuses :8090 from Phase 429).
  data_files: >
    rc-agent-mobile/app/src/main/res/xml/accessibility_service_config.xml
    (static XML declaring event types, feedback type, and flags - bundled in APK).
  bat_file: none
  cloud_parity: [none]  # Phase 430 is device-local only; no cloud or server-side changes.
  targets:
    - tab_plus   # Lenovo TB-351FU (Android 14)
    - m07        # Samsung Galaxy M07 (Android 14)
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  rollback:
    - "Keep previous APK (Phase 429 build) on device as /sdcard/Download/rc-agent-mobile-429.apk"
    - "Rollback: adb uninstall in.racingpoint.rcagentmobile && adb install -r /sdcard/Download/rc-agent-mobile-429.apk"
    - "Accessibility toggle is preserved across APK reinstall only if the signing key is unchanged; otherwise user must re-enable the toggle after rollback."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip           # Accessibility setup Activity is a transient first-run screen, not a customer-facing product surface. A single screen with one button + one status line is below UI-SPEC threshold.
  ui_auditor: skip              # Same reason.
  nyquist_auditor: required     # Selector-resolution + 100ms-retry + gesture-dispatch + 503-gate are business logic. Precise input-output contracts.
  mma_audit: required           # Accessibility Service is a HIGH-risk Android surface (Android 13+ restricted service access rules). Cross-boundary: user-controlled toggle <-> agent runtime behavior. MMA with dual reasoning modes per CLAUDE.md.
  integration_checker: deferred # Defer to milestone-close; Phase 430 is a single-device phase with no cross-system bridges beyond Phase 429's already-tested WS.
  codebase_mapper: optional     # rc-agent-mobile/ already on the map from Phase 429. Only needed if top-level module structure changes (it does not).

risks_summary:
  - "Android 13+ (API 33) restricted-settings — OEM may block Accessibility toggle for sideloaded APKs with a 'Restricted settings' warning. User must long-press the app entry in Settings -> Apps and tap 'Allow restricted settings' BEFORE the Accessibility toggle becomes tappable. 430-06 documents and links to this flow."
  - "Android 14 (API 34) adds stricter AccessibilityService lifecycle — if the service is slow to return from onAccessibilityEvent, the OS may temporarily disconnect. All node-tree work runs on a dedicated coroutine dispatcher; the callback returns in <5ms."
  - "getRootInActiveWindow() can return null when focus just shifted (during an animation). Screen-tree endpoint must retry up to 100ms + return a structured error, not null."
  - "GestureDescription dispatching can fail silently if the service loses connection mid-gesture (onGestureCancelled). All dispatchers use callbacks + timeout, not fire-and-forget."
  - "Samsung One UI and Lenovo skins may add extra permission prompts on first Accessibility enable. 430-06 handles the deep-link but cannot automate the 'long-press -> Allow restricted settings' step (Android forbids it)."
  - "Screen tree can exceed 1 MB on complex apps (Instagram, Zomato Partner dashboard). 430-02 caps tree depth and total-node-count with explicit values (max_depth=30, max_nodes=5000)."
  - "xpath selector strategy is a PLACEHOLDER in 430-03 (Phase 433 implements the full DSL). 430-03 ships a stub that accepts xpath strings but rejects all non-trivial ones with a structured 'not yet supported' error."
  - "A crashing AccessibilityService causes Android to silently disable it with no user notification. A11yStateMonitor polls Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES every 10s and logs + updates notification + returns 503 if we were enabled and are now not."
  - "Running Accessibility Service drains battery faster (~3-5%/h idle overhead). Expected cost; document in ACCESSIBILITY-NOTES.md."
---

# Phase 430 — Accessibility Service Foundation

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 430 |
| Name | Accessibility Service Foundation |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | ACCESS-01, ACCESS-02, ACCESS-03, ACCESS-04, ACCESS-05 |
| Dependencies | Phase 429 (scaffold + HTTP :8090 + comms-link registration) |
| Wave | 2 (after Phase 429; parallel with Phase 3 Bootstrap per ROADMAP-v50.md Phase-3 entry — but Phase 3 is fed by the 430-06 first-run Activity, so for practical sequencing 430 precedes 3) |
| Status | Ready to execute |
| Autonomous | No — 430-06 and 430-07 require physical-device Accessibility toggling and E2E verification |
| Ship test | /screen/tree returns full node hierarchy in <500ms; /ui/tap by resource-id >=95% success on test harness; 503 returned when Accessibility disabled with human-readable message; first-run setup opens Settings and waits for toggle |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 2)

1. **SC-1 — Screen tree latency & completeness:** `GET /screen/tree` returns the full `AccessibilityNodeInfo` hierarchy of the foreground app in **under 500ms**, serialized as JSON, with all non-private fields (className, text, contentDescription, viewIdResourceName, bounds, clickable, focusable, children).
2. **SC-2 — Tap primitive reliability:** `POST /ui/tap` with `{ strategy: "resource-id", value: "in.zomato.partner:id/btn_accept" }` hits the target element with **>=95% success** over a 20-trial test-harness run (test harness is a controlled fixture app bundled with the instrumented test suite; see 430-07).
3. **SC-3 — Disabled-state refusal:** When Accessibility Service is disabled, **every** UI action endpoint returns **HTTP 503** with body `{ error: "accessibility_service_disabled", message: "<human readable>", settings_deep_link: "<uri>" }` and the persistent notification title changes to "RC Agent Mobile — Accessibility OFF".
4. **SC-4 — First-run setup:** Launching the app on a device with Accessibility disabled opens the first-run Activity, which opens `Settings.ACTION_ACCESSIBILITY_SETTINGS` within 2 seconds of a single button tap, and polls every 500ms until the service reports enabled — then auto-dismisses.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)
- T-1: On a device with Accessibility ENABLED, `curl http://<device_ip>:8090/screen/tree` from a LAN machine returns JSON with `root` + `children[]` fields in <500ms — foreground-app-name is reflected in the top-level `package_name` field (ACCESS-01, ACCESS-02).
- T-2: On a device with Accessibility ENABLED, while Zomato Partner (or any test fixture) is foregrounded, `POST /ui/tap` with a resource-id pointing at a visible button causes the button to visually trigger (observable via a debounced screenshot or via the fixture app's log) (ACCESS-03).
- T-3: `POST /ui/swipe` with {start, end, duration_ms} produces a scroll or swipe gesture in the foreground app (ACCESS-03).
- T-4: `POST /ui/text` with {strategy, value, text} focuses the target text field and types the string (ACCESS-03).
- T-5: On a device with Accessibility DISABLED, `POST /ui/tap` returns HTTP 503 with a body containing `"error": "accessibility_service_disabled"` and a `"settings_deep_link"` field pointing at `android.settings.ACCESSIBILITY_SETTINGS` (ACCESS-05).
- T-6: On a device with Accessibility DISABLED, the persistent notification body text says "Accessibility OFF - tap to enable" and tapping it opens the first-run Activity (ACCESS-05).
- T-7: Launching the first-run Activity when Accessibility is disabled shows a single "Open Accessibility Settings" button; tapping it deep-links to `Settings.ACTION_ACCESSIBILITY_SETTINGS` (ACCESS-04).
- T-8: After enabling the toggle in Settings and returning to the app (back button), within 2 seconds the Activity auto-dismisses and the notification flips to green/"connected" state (ACCESS-04, ACCESS-05).
- T-9: A selector miss (element not found for 100ms worth of retries) returns HTTP 404 with a structured `{ error: "selector_miss", strategy, value, elapsed_ms, matched_roots[] }` body — so Phase 433 SelectorMissEvent can be emitted cleanly (ACCESS-03).
- T-10: `curl http://<device_ip>:8090/ui/xpath_test` with an xpath selector returns HTTP 501 `{ error: "xpath_not_yet_supported", implemented_in_phase: 433 }` — intentional placeholder so the strategy enum is closed at compile time but xpath logic is deferred (per extensibility requirement).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/app/src/main/res/xml/accessibility_service_config.xml` | Service declaration | 20 | `accessibilityEventTypes="typeWindowContentChanged\|typeViewClicked\|typeViewFocused"`, `accessibilityFeedbackType="feedbackGeneric"`, `accessibilityFlags="flagReportViewIds\|flagRequestTouchExplorationMode\|flagIncludeNotImportantViews"`, `canRetrieveWindowContent="true"`, `canPerformGestures="true"` |
| `.../a11y/RcAccessibilityService.kt` | Service class + event handler | 80 | extends `AccessibilityService`, `onAccessibilityEvent` routes to `A11yBridge.updateLastEvent()`, `onServiceConnected` registers with A11yBridge |
| `.../a11y/A11yBridge.kt` | Service <-> rest-of-app hand-off singleton | 50 | `AtomicReference<RcAccessibilityService>`, `fun tryGetService(): RcAccessibilityService?`, `fun isConnected(): Boolean` |
| `.../a11y/ScreenNode.kt` | Serializable DTO | 40 | `@Serializable data class ScreenNode(class_name, text, content_description, view_id_resource_name, bounds, clickable, focusable, children)` |
| `.../a11y/ScreenTreeReader.kt` | Traverse active window | 60 | `fun readTree(max_depth=30, max_nodes=5000): ScreenNode`, retries `getRootInActiveWindow` up to 100ms if null, recycles nodes |
| `.../a11y/Selector.kt` | Selector types | 40 | `sealed class Selector { ResourceId, ContentDescription, Text, Xpath }`, `enum class SelectorStrategy { RESOURCE_ID, CONTENT_DESC, TEXT, XPATH }`, JSON serializers |
| `.../a11y/SelectorResolver.kt` | Match + retry | 100 | `suspend fun resolve(sel: Selector, timeout_ms: Long = 100): Result<AccessibilityNodeInfo>`, BFS traversal, structured error on miss |
| `.../a11y/GestureDispatcher.kt` | Tap + swipe via GestureDescription | 80 | `suspend fun tap(node) -> Boolean`, `suspend fun swipe(start, end, duration_ms)`, uses callback-based completion |
| `.../a11y/TextInputDispatcher.kt` | Type into node | 40 | `fun inputText(node, text): Boolean` via `ACTION_SET_TEXT` bundle |
| `.../a11y/A11yStateMonitor.kt` | Enabled/disabled detection | 60 | polls `Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES` every 10s, exposes `StateFlow<A11yState>`, logs transitions via `RotatingLog` |
| `.../http/UiRoutes.kt` | Ktor routes for /screen/tree + /ui/* | 120 | `Route.ui()` extension, endpoint handlers, 503 middleware that checks A11yBridge.isConnected() |
| `.../firstrun/AccessibilitySetupActivity.kt` | First-run setup | 60 | Checks A11yStateMonitor, deep-links to Settings, polls state, auto-finishes |
| `.../res/layout/activity_accessibility_setup.xml` | Simple layout | 20 | One TextView + one Button + one progress spinner |
| `rc-agent-mobile/docs/ACCESSIBILITY-NOTES.md` | Operator notes | 100 | Setup sequence, restricted-settings workaround, OEM-specific tips, battery impact |
| `rc-agent-mobile/docs/PROTOCOL.md` (MODIFIED) | Add UI action schema | +40 | New section "Phase 430: UI Actions" with `/screen/tree`, `/ui/tap`, `/ui/swipe`, `/ui/text` request/response shapes |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| `RcAccessibilityService.onServiceConnected` | `A11yBridge.register(this)` | direct call | grep `A11yBridge.register` in `RcAccessibilityService.kt` |
| `RcAccessibilityService.onUnbind` | `A11yBridge.unregister()` | direct call | grep `A11yBridge.unregister` in `RcAccessibilityService.kt` |
| `UiRoutes` (every UI endpoint) | `A11yBridge.tryGetService()` | 503 gate | grep `A11yBridge.tryGetService` in `UiRoutes.kt`; must be FIRST line of every handler |
| `ScreenTreeReader.readTree` | `service.rootInActiveWindow` | direct field | grep `rootInActiveWindow` in `ScreenTreeReader.kt` |
| `SelectorResolver.resolve` | `ScreenTreeReader` or direct BFS | traversal | grep `findAccessibilityNodeInfosByViewId\|findAccessibilityNodeInfosByText` in `SelectorResolver.kt` |
| `GestureDispatcher.tap` | `service.dispatchGesture(GestureDescription, callback)` | Android API | grep `dispatchGesture` in `GestureDispatcher.kt` |
| `TextInputDispatcher.inputText` | `node.performAction(ACTION_SET_TEXT, bundle)` | Android API | grep `ACTION_SET_TEXT` in `TextInputDispatcher.kt` |
| `A11yStateMonitor` | `Settings.Secure.getString(ENABLED_ACCESSIBILITY_SERVICES)` | Android settings | grep `ENABLED_ACCESSIBILITY_SERVICES` in `A11yStateMonitor.kt` |
| `AgentForegroundService` | `A11yStateMonitor.state` (observed via Flow) | collect | grep `a11yStateMonitor.state.collect` in `AgentForegroundService.kt` |
| `AgentForegroundService.updateNotification` | notification text change on state flip | direct call | grep `updateNotification("Accessibility OFF"` in `AgentForegroundService.kt` |
| `AccessibilitySetupActivity` | `Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)` | Android intent | grep `ACTION_ACCESSIBILITY_SETTINGS` in `AccessibilitySetupActivity.kt` |
| `AccessibilitySetupActivity.onResume` | polls `A11yStateMonitor.state` every 500ms | Handler or lifecycle scope | grep `state.collect` or `postDelayed` in `AccessibilitySetupActivity.kt` |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md   # v50.0 section top — locked decisions + extensibility features
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md  # Phase 429 shape + artifacts
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt  # Where A11yStateMonitor plugs in
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/LocalHttpServer.kt  # Where UiRoutes mounts
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/log/RotatingLog.kt  # For state transitions
@./rc-agent-mobile/docs/PROTOCOL.md  # Extend with UI action message types

### Interfaces executors will need (extracted ahead to avoid scavenger hunt)

**Phase 429 exports the executor depends on:**
```kotlin
// From rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt
class AgentForegroundService : LifecycleService() {
    val serviceScope: CoroutineScope                        // reuse for A11yStateMonitor collection
    fun updateNotification(title: String, body: String)     // existing from 429-02 (rename/extend if single-arg)
    val deviceState: DeviceState                            // exposes ws_connected etc.
}

// From rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/LocalHttpServer.kt
class LocalHttpServer(port: Int, deviceState: DeviceState) {
    fun registerRoute(installer: Route.() -> Unit)         // existing extensibility hook from 429-03
    // Phase 430 calls: server.registerRoute { ui(a11yBridge, ...) }
}

// From rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/log/RotatingLog.kt
object RotatingLog {
    fun info(target: String, event: String, details: Map<String, Any?> = emptyMap())
    fun warn(target: String, event: String, details: Map<String, Any?> = emptyMap())
    fun error(target: String, event: String, details: Map<String, Any?> = emptyMap())
}
```

**New interfaces Phase 430 defines (will be consumed by Phase 432+ drivers):**
```kotlin
// rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/Selector.kt
enum class SelectorStrategy { RESOURCE_ID, CONTENT_DESC, TEXT, XPATH }

@Serializable
sealed class Selector {
    abstract val strategy: SelectorStrategy
    @Serializable data class ResourceId(val value: String) : Selector() { override val strategy = SelectorStrategy.RESOURCE_ID }
    @Serializable data class ContentDesc(val value: String) : Selector() { override val strategy = SelectorStrategy.CONTENT_DESC }
    @Serializable data class Text(val value: String) : Selector() { override val strategy = SelectorStrategy.TEXT }
    @Serializable data class Xpath(val value: String) : Selector() { override val strategy = SelectorStrategy.XPATH }
}

// rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/SelectorResolver.kt
class SelectorResolver(private val bridge: A11yBridge) {
    suspend fun resolve(sel: Selector, timeoutMs: Long = 100): Result<AccessibilityNodeInfo>
    // Result.failure carries a ResolutionError with strategy + elapsed_ms + matched_roots_count
}

// rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/GestureDispatcher.kt
class GestureDispatcher(private val service: AccessibilityService) {
    suspend fun tap(node: AccessibilityNodeInfo): Boolean
    suspend fun swipe(x1: Int, y1: Int, x2: Int, y2: Int, durationMs: Long): Boolean
}

// rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/a11y/A11yStateMonitor.kt
sealed class A11yState {
    object Enabled : A11yState()
    data class Disabled(val reason: String) : A11yState()       // "never_enabled" | "user_disabled" | "service_crashed"
    object Unknown : A11yState()                                // startup transient
}
class A11yStateMonitor(context: Context) {
    val state: StateFlow<A11yState>
    suspend fun start()    // launches polling coroutine
}
```

**JSON envelopes added to `docs/PROTOCOL.md` (schema additions for Phase 430):**
```json
// POST /ui/tap request
{ "selector": { "strategy": "resource_id", "value": "in.zomato.partner:id/btn_accept" },
  "timeout_ms": 100 }
// Response 200:
{ "ok": true, "matched_node": { "class_name":"...", "bounds":[x,y,w,h] }, "elapsed_ms": 42 }
// Response 404 (selector miss):
{ "error": "selector_miss", "strategy":"resource_id", "value":"...", "elapsed_ms": 100, "matched_roots": [] }
// Response 503 (Accessibility disabled):
{ "error": "accessibility_service_disabled",
  "message": "Accessibility Service is OFF. Open Settings -> Accessibility -> RC Agent Mobile.",
  "settings_deep_link": "android.settings.ACCESSIBILITY_SETTINGS" }
// Response 501 (xpath placeholder):
{ "error": "xpath_not_yet_supported", "implemented_in_phase": 433 }
```

## 5. Atomic plan breakdown (7 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

---

### 430-01-PLAN — AccessibilityService subclass + manifest + lifecycle

**Goal:** `RcAccessibilityService` class registered in manifest, connects on user toggle, exposes itself via `A11yBridge` singleton to the rest of the app. Phase 430-02+ all depend on this.

**Covers:** ACCESS-01

**Dependencies:** 429 (scaffold + Foreground Service exists)

**Type:** `auto` (unit test + manual toggle in Settings, verified by logcat)

#### Tasks

1. Create `res/xml/accessibility_service_config.xml`:
   ```xml
   <?xml version="1.0" encoding="utf-8"?>
   <accessibility-service xmlns:android="http://schemas.android.com/apk/res/android"
       android:accessibilityEventTypes="typeWindowContentChanged|typeViewClicked|typeViewFocused"
       android:accessibilityFeedbackType="feedbackGeneric"
       android:accessibilityFlags="flagReportViewIds|flagRequestTouchExplorationMode|flagIncludeNotImportantViews"
       android:canRetrieveWindowContent="true"
       android:canPerformGestures="true"
       android:notificationTimeout="100"
       android:settingsActivity="in.racingpoint.rcagentmobile.firstrun.AccessibilitySetupActivity"
       android:description="@string/accessibility_service_description" />
   ```
   - Event types are the minimum set from ACCESS-01 — no `typeAll` (noisy, battery-cost).
   - `flagIncludeNotImportantViews` is required to read the full tree (important for selector robustness — some apps mark buttons as not-important-for-accessibility).
   - `notificationTimeout=100` batches rapid events to reduce callback storm.
   - `settingsActivity` deep-links from the Settings app back to our first-run flow (430-06).

2. Create `strings.xml` entries:
   - `accessibility_service_description`: "RC Agent Mobile uses Accessibility to automate Zomato Partner, HyperPure, and other reception apps. It only reads screen content and dispatches taps/swipes on behalf of staff."
   - `accessibility_setup_title`: "Enable RC Agent Accessibility"
   - `accessibility_setup_body`: "This service must stay on for Zomato Partner, HyperPure, and Blinkit automation to work."
   - `accessibility_setup_button`: "Open Accessibility Settings"
   - `accessibility_setup_waiting`: "Waiting for you to enable RC Agent Mobile..."
   - `notification_accessibility_off`: "Accessibility OFF - tap to enable"
   - `notification_accessibility_on`: "Connected - automation ready"

3. Create `A11yBridge.kt`:
   ```kotlin
   package in.racingpoint.rcagentmobile.a11y
   import android.accessibilityservice.AccessibilityService
   import java.util.concurrent.atomic.AtomicReference

   object A11yBridge {
       private val ref = AtomicReference<RcAccessibilityService?>(null)
       fun register(s: RcAccessibilityService) { ref.set(s) }
       fun unregister() { ref.set(null) }
       fun tryGetService(): AccessibilityService? = ref.get()
       fun isConnected(): Boolean = ref.get() != null
   }
   ```
   - Singleton because only ONE AccessibilityService instance can exist per app; the bridge is the standard pattern for connecting it to non-service code (HTTP routes, coroutines).
   - AtomicReference avoids lock-across-await (CLAUDE.md rule).

4. Create `RcAccessibilityService.kt`:
   ```kotlin
   package in.racingpoint.rcagentmobile.a11y
   import android.accessibilityservice.AccessibilityService
   import android.view.accessibility.AccessibilityEvent

   class RcAccessibilityService : AccessibilityService() {
       override fun onServiceConnected() {
           super.onServiceConnected()
           A11yBridge.register(this)
           RotatingLog.info("a11y", "service_connected")
       }
       override fun onAccessibilityEvent(event: AccessibilityEvent?) {
           // Phase 430: no-op. Phase 433+ selector-miss detection may consume these.
           // Return fast (<5ms) — OS disconnects slow services.
       }
       override fun onInterrupt() {
           RotatingLog.warn("a11y", "service_interrupted")
       }
       override fun onUnbind(intent: android.content.Intent?): Boolean {
           A11yBridge.unregister()
           RotatingLog.warn("a11y", "service_unbound")
           return super.onUnbind(intent)
       }
       override fun onDestroy() {
           A11yBridge.unregister()
           RotatingLog.warn("a11y", "service_destroyed")
           super.onDestroy()
       }
   }
   ```

5. `AndroidManifest.xml` additions:
   ```xml
   <service
       android:name=".a11y.RcAccessibilityService"
       android:label="@string/app_name"
       android:permission="android.permission.BIND_ACCESSIBILITY_SERVICE"
       android:exported="true">
       <intent-filter>
           <action android:name="android.accessibilityservice.AccessibilityService" />
       </intent-filter>
       <meta-data
           android:name="android.accessibilityservice"
           android:resource="@xml/accessibility_service_config" />
   </service>
   ```
   - `exported="true"` is required for AccessibilityService (Android OS binds it).
   - `BIND_ACCESSIBILITY_SERVICE` permission is how Android verifies only the system can bind us.

6. Unit test `RcAccessibilityServiceTest.kt`:
   - MockK-stub `AccessibilityService` base behaviour.
   - Call `onServiceConnected()` — assert `A11yBridge.isConnected()` is true.
   - Call `onUnbind(null)` — assert `A11yBridge.isConnected()` is false.
   - Call `onDestroy()` — assert logger called with `service_destroyed`.

#### Acceptance

- Build compiles: `./gradlew :app:assembleDebug`.
- Unit test `RcAccessibilityServiceTest` passes (3 cases).
- Install APK on Tab Plus, open Settings -> Accessibility -> Installed services: "RC Agent Mobile" appears in the list with the description from strings.xml.
- Toggle ON: logcat `RcAgentMobile` tag shows `service_connected`. `adb shell dumpsys accessibility | grep in.racingpoint` shows the service as bound.
- Toggle OFF: logcat shows `service_unbound` + `service_destroyed`.

#### G4 NOT TESTED list

- Screen-tree reading (430-02).
- Gesture dispatch (430-03, 430-04).
- 503 gate (430-05).
- First-run setup Activity (430-06).

#### Commit message

```
feat(430-01): RcAccessibilityService + A11yBridge singleton + manifest

Registers Accessibility Service with TYPE_WINDOW_CONTENT_CHANGED |
TYPE_VIEW_CLICKED | TYPE_VIEW_FOCUSED events (ACCESS-01).  A11yBridge
AtomicReference singleton is the hand-off between the OS-bound service
and the rest of the app (HTTP routes, coroutines).  Manifest declares
BIND_ACCESSIBILITY_SERVICE permission and exports the service for OS binding.

Covers: ACCESS-01
Not tested: screen-tree reading, gesture dispatch, 503 gate, first-run UX.
```

---

### 430-02-PLAN — Screen-tree reader + `/screen/tree` endpoint

**Goal:** Given a foreground app, traverse its `AccessibilityNodeInfo` hierarchy and return a serializable JSON tree via `GET /screen/tree`. P99 latency under 500ms.

**Covers:** ACCESS-02

**Dependencies:** 430-01

**Type:** `auto`

#### Tasks

1. Create `ScreenNode.kt` (serializable DTO):
   ```kotlin
   @Serializable
   data class ScreenNode(
       val class_name: String,
       val package_name: String,
       val text: String? = null,
       val content_description: String? = null,
       val view_id_resource_name: String? = null,
       val bounds: List<Int>, // [left, top, right, bottom]
       val clickable: Boolean,
       val focusable: Boolean,
       val enabled: Boolean,
       val visible_to_user: Boolean,
       val children: List<ScreenNode>
   )

   @Serializable
   data class ScreenTreeResponse(
       val package_name: String,
       val class_name: String,
       val ts_ms: Long,
       val capture_ms: Long,
       val truncated: Boolean,          // true if max_depth or max_nodes hit
       val root: ScreenNode
   )
   ```
   - Only non-PII fields (no bitmap, no content from `hashCode`).
   - `bounds` flattens `android.graphics.Rect` to a 4-int list so Phase 433 selector DSL can match by bounds without Android types leaking into the protocol.

2. Create `ScreenTreeReader.kt`:
   ```kotlin
   class ScreenTreeReader(private val bridge: A11yBridge) {
       suspend fun readTree(maxDepth: Int = 30, maxNodes: Int = 5000): Result<ScreenTreeResponse> {
           val start = System.currentTimeMillis()
           val svc = bridge.tryGetService() ?: return Result.failure(A11yDisabledException())
           // Retry getRootInActiveWindow for up to 100ms — null during focus transitions
           var root: AccessibilityNodeInfo? = null
           val deadline = start + 100
           while (root == null && System.currentTimeMillis() < deadline) {
               root = svc.rootInActiveWindow
               if (root == null) delay(10)
           }
           if (root == null) return Result.failure(NoActiveWindowException())
           val counter = AtomicInteger(0)
           val truncated = AtomicBoolean(false)
           val tree = try {
               convert(root, depth = 0, maxDepth, maxNodes, counter, truncated)
           } finally {
               root.recycle()
           }
           return Result.success(ScreenTreeResponse(
               package_name = tree.package_name,
               class_name = tree.class_name,
               ts_ms = start,
               capture_ms = System.currentTimeMillis() - start,
               truncated = truncated.get(),
               root = tree
           ))
       }

       private fun convert(node: AccessibilityNodeInfo, depth: Int, maxDepth: Int, maxNodes: Int,
                           counter: AtomicInteger, truncated: AtomicBoolean): ScreenNode {
           if (counter.incrementAndGet() > maxNodes || depth >= maxDepth) {
               truncated.set(true)
               return ScreenNode( /* leaf-stub */ )
           }
           val children = (0 until node.childCount).mapNotNull { i ->
               node.getChild(i)?.let { child ->
                   val n = convert(child, depth + 1, maxDepth, maxNodes, counter, truncated)
                   child.recycle()
                   n
               }
           }
           val rect = android.graphics.Rect().also { node.getBoundsInScreen(it) }
           return ScreenNode(
               class_name = node.className?.toString() ?: "",
               package_name = node.packageName?.toString() ?: "",
               text = node.text?.toString(),
               content_description = node.contentDescription?.toString(),
               view_id_resource_name = node.viewIdResourceName,
               bounds = listOf(rect.left, rect.top, rect.right, rect.bottom),
               clickable = node.isClickable,
               focusable = node.isFocusable,
               enabled = node.isEnabled,
               visible_to_user = node.isVisibleToUser,
               children = children
           )
       }
   }
   ```
   - **Recycling is mandatory:** `AccessibilityNodeInfo` is pooled by the OS; leaking it causes the system to degrade. Recycle after reading children.
   - `maxDepth=30` covers deepest real-world trees (nested ScrollView + RecyclerView ~12-15 deep); 30 is safety margin.
   - `maxNodes=5000` caps against pathological trees (Instagram, YouTube). Zomato Partner's dashboard is ~400 nodes.
   - Runs on `Dispatchers.IO` (reading AccessibilityNodeInfo touches system IPC).

3. Create `UiRoutes.kt` (new file — Ktor route module):
   ```kotlin
   fun Route.ui(bridge: A11yBridge, reader: ScreenTreeReader) {
       // 503 gate — checked FIRST in every handler
       suspend fun ApplicationCall.ensureEnabled(): Boolean {
           if (!bridge.isConnected()) {
               respond(HttpStatusCode.ServiceUnavailable, mapOf(
                   "error" to "accessibility_service_disabled",
                   "message" to "Accessibility Service is OFF. Open Settings -> Accessibility -> RC Agent Mobile.",
                   "settings_deep_link" to "android.settings.ACCESSIBILITY_SETTINGS"
               ))
               return false
           }
           return true
       }
       get("/screen/tree") {
           if (!call.ensureEnabled()) return@get
           val result = reader.readTree()
           result.fold(
               onSuccess = { call.respond(it) },
               onFailure = { ex ->
                   call.respond(HttpStatusCode.InternalServerError,
                       mapOf("error" to "screen_tree_failure", "message" to (ex.message ?: "unknown")))
               }
           )
       }
       // /ui/tap, /ui/swipe, /ui/text stubs added in 430-03/04 — they extend THIS block
   }
   ```

4. Modify `LocalHttpServer.kt` to mount the UI routes:
   ```kotlin
   fun start() {
       server = embeddedServer(Netty, port) {
           install(ContentNegotiation) { json() }
           routing {
               // ... existing /health, /build_id, /capability, /heartbeat from 429 ...
               ui(A11yBridge, screenTreeReader)
           }
       }.start(wait = false)
   }
   ```
   Note: 430-02 only implements `/screen/tree` inside `ui()`. Subsequent plans add more routes to the same `Route.ui` extension.

5. Unit test `ScreenTreeReaderTest.kt`:
   - MockK-stub `AccessibilityNodeInfo` with nested children (3 levels, 8 nodes total).
   - Stub `A11yBridge.tryGetService()` to return a service whose `rootInActiveWindow` returns the mock.
   - Call `readTree()`, assert returned `ScreenTreeResponse.root.children.size == 2`, total count == 8, `truncated == false`.
   - Second test: pass `maxNodes=3`, assert `truncated == true`.
   - Third test: stub bridge returning null service, assert `Result.failure(A11yDisabledException)`.

6. Update `PROTOCOL.md` with the `/screen/tree` response schema (200, 500, 503 shapes).

#### Acceptance

- Unit tests pass (3 cases).
- On Tab Plus with Accessibility ENABLED, with the system Settings app foregrounded, `curl http://<tab_plus_ip>:8090/screen/tree` returns 200 JSON with `package_name: "com.android.settings"` and a non-empty `children` array, in under 500ms (measure 5 times, all under 500ms).
- With Accessibility DISABLED, returns 503 with the expected body (full 503 coverage in 430-05; this plan already wires the gate).
- `curl http://<tab_plus_ip>:8090/screen/tree | jq '.root.children | length'` returns >0 and `jq '.capture_ms'` returns <500.
- `adb shell logcat -d | grep RotatingLog` shows no AccessibilityNodeInfo pool warnings (recycling verified).

#### G4 NOT TESTED list

- Tap / swipe / text dispatch (430-03, 430-04).
- Selector resolution (430-03).
- First-run activity polling (430-06).
- Long-running stability (430-07 instrumented tests + E2E).

#### Commit message

```
feat(430-02): ScreenTreeReader + GET /screen/tree endpoint

Serializes AccessibilityNodeInfo hierarchy to JSON via Ktor route.
max_depth=30, max_nodes=5000 caps with structured truncation flag.
Retries rootInActiveWindow for 100ms during focus transitions.
Recycles all nodes after conversion (AccessibilityNodeInfo pool hygiene).

Covers: ACCESS-02
Not tested: UI dispatch primitives, 503 disabled-state (gate already wired, full coverage in 430-05).
```

---

### 430-03-PLAN — Tap primitive + SelectorResolver + 100ms retry

**Goal:** `POST /ui/tap` with a selector (resource-id, content-desc, text, or xpath-stub) finds the target node and dispatches a tap gesture. Structured 404 on miss, 501 on xpath.

**Covers:** ACCESS-03 (tap portion)

**Dependencies:** 430-02

**Type:** `auto` (unit tests + instrumented test)

#### Tasks

1. Create `Selector.kt` with the sealed hierarchy + enum defined in §4 interfaces. Include JSON polymorphic serializers so `POST /ui/tap { selector: { strategy, value } }` deserializes cleanly.

2. Create `SelectorResolver.kt`:
   ```kotlin
   class SelectorResolver(private val bridge: A11yBridge) {
       suspend fun resolve(sel: Selector, timeoutMs: Long = 100): Result<AccessibilityNodeInfo> {
           val svc = bridge.tryGetService() ?: return Result.failure(A11yDisabledException())
           val start = System.currentTimeMillis()
           val deadline = start + timeoutMs
           var lastRoots = 0
           while (System.currentTimeMillis() < deadline) {
               val root = svc.rootInActiveWindow
               if (root != null) {
                   val matches: List<AccessibilityNodeInfo> = when (sel) {
                       is Selector.ResourceId -> root.findAccessibilityNodeInfosByViewId(sel.value)
                       is Selector.Text -> root.findAccessibilityNodeInfosByText(sel.value)
                       is Selector.ContentDesc -> bfsContentDesc(root, sel.value)
                       is Selector.Xpath -> return Result.failure(XpathNotSupportedException())
                   }
                   lastRoots++
                   if (matches.isNotEmpty()) {
                       // Return first visible, clickable match; recycle others
                       val target = matches.firstOrNull { it.isVisibleToUser && it.isClickable }
                           ?: matches.first()
                       matches.filter { it !== target }.forEach { it.recycle() }
                       root.recycle()
                       return Result.success(target)
                   }
                   root.recycle()
               }
               delay(20) // re-poll every 20ms within the 100ms budget
           }
           return Result.failure(SelectorMissException(sel, System.currentTimeMillis() - start, lastRoots))
       }

       private fun bfsContentDesc(root: AccessibilityNodeInfo, query: String): List<AccessibilityNodeInfo> {
           val found = mutableListOf<AccessibilityNodeInfo>()
           val queue = ArrayDeque<AccessibilityNodeInfo>()
           queue.add(root)
           while (queue.isNotEmpty()) {
               val n = queue.removeFirst()
               if (n.contentDescription?.toString() == query) found.add(n)
               for (i in 0 until n.childCount) n.getChild(i)?.let { queue.add(it) }
           }
           return found
       }
   }
   ```
   - **100ms budget** with 20ms poll = up to 5 retries. Matches ACCESS-03.
   - `findAccessibilityNodeInfosByViewId` is Android's built-in — fast and memory-efficient.
   - Content-desc lacks a built-in, so BFS manually. Recycling is handled by caller (see tap dispatcher below).

3. Create `GestureDispatcher.kt` (tap portion only — swipe in 430-04):
   ```kotlin
   class GestureDispatcher(private val bridge: A11yBridge) {
       suspend fun tap(node: AccessibilityNodeInfo): Boolean = suspendCancellableCoroutine { cont ->
           val svc = bridge.tryGetService() ?: run { cont.resume(false); return@suspendCancellableCoroutine }
           val rect = android.graphics.Rect().also { node.getBoundsInScreen(it) }
           val cx = rect.exactCenterX()
           val cy = rect.exactCenterY()
           val path = android.graphics.Path().apply { moveTo(cx, cy) }
           val gesture = GestureDescription.Builder()
               .addStroke(GestureDescription.StrokeDescription(path, 0, 50))
               .build()
           val timeoutJob = serviceScope.launch {
               delay(1000) // 1s gesture timeout
               if (!cont.isCompleted) cont.resume(false)
           }
           svc.dispatchGesture(gesture, object : AccessibilityService.GestureResultCallback() {
               override fun onCompleted(g: GestureDescription?) { timeoutJob.cancel(); if (!cont.isCompleted) cont.resume(true) }
               override fun onCancelled(g: GestureDescription?) { timeoutJob.cancel(); if (!cont.isCompleted) cont.resume(false) }
           }, null)
       }
   }
   ```
   - `suspendCancellableCoroutine` bridges Android's callback API into coroutines cleanly.
   - 1s gesture timeout — if `dispatchGesture` never fires a callback (service disconnected mid-flight), we resume `false` rather than hang.
   - 50ms stroke duration is Android-standard for tap (<80ms = tap, >80ms = long-press).

4. Extend `UiRoutes.kt` with `/ui/tap`:
   ```kotlin
   post("/ui/tap") {
       if (!call.ensureEnabled()) return@post
       val req = call.receive<TapRequest>()
       val start = System.currentTimeMillis()
       resolver.resolve(req.selector, req.timeout_ms ?: 100).fold(
           onSuccess = { node ->
               val ok = dispatcher.tap(node)
               node.recycle()
               if (ok) call.respond(TapResponse(ok=true, elapsed_ms = System.currentTimeMillis() - start))
               else call.respond(HttpStatusCode.InternalServerError, mapOf("error" to "gesture_failed"))
           },
           onFailure = { ex -> when (ex) {
               is SelectorMissException -> call.respond(HttpStatusCode.NotFound,
                   mapOf("error" to "selector_miss", "strategy" to ex.strategy.name,
                         "value" to ex.value, "elapsed_ms" to ex.elapsedMs))
               is XpathNotSupportedException -> call.respond(HttpStatusCode.NotImplemented,
                   mapOf("error" to "xpath_not_yet_supported", "implemented_in_phase" to 433))
               is A11yDisabledException -> call.respond(HttpStatusCode.ServiceUnavailable,
                   mapOf("error" to "accessibility_service_disabled"))
               else -> call.respond(HttpStatusCode.InternalServerError,
                   mapOf("error" to "unknown", "message" to (ex.message ?: "")))
           }}
       )
   }
   ```

5. Unit test `SelectorResolverTest.kt`:
   - Stub AccessibilityNodeInfo with children having resource-ids; assert ResourceId selector returns the correct node.
   - Stub service whose `rootInActiveWindow` returns null for 60ms then returns a populated tree; assert the retry path succeeds within the 100ms budget.
   - Assert Xpath selector returns `XpathNotSupportedException` immediately (no retry burn).
   - Assert a miss after 100ms returns `SelectorMissException` with `elapsedMs >= 100`.

6. Unit test `GestureDispatcherTest.kt`:
   - MockK-stub `AccessibilityService.dispatchGesture` to immediately invoke `onCompleted`. Assert `tap()` returns true.
   - Stub to invoke `onCancelled`. Assert returns false.
   - Stub to never invoke callback. Assert returns false after the 1s internal timeout.

7. Update `PROTOCOL.md` with `/ui/tap` request/response schema + 200/404/501/503 bodies.

#### Acceptance

- Unit tests pass (4 SelectorResolver cases + 3 GestureDispatcher cases).
- Instrumented test (to be run on a connected Tab Plus in 430-07, stubbed here): `POST /ui/tap { "selector": { "strategy": "resource_id", "value": "com.android.settings:id/search_bar" } }` from a LAN machine while Settings app is foregrounded produces a visible tap on the search bar on the Tab Plus.
- `POST /ui/tap { "selector": { "strategy": "xpath", "value": "//*[@id='foo']" } }` returns HTTP 501.
- `POST /ui/tap` with a non-existent resource-id returns HTTP 404 with `{ error: "selector_miss", elapsed_ms: >= 100 }`.

#### G4 NOT TESTED list

- Swipe + text dispatch (430-04).
- First-run Activity polling (430-06).
- 95% success rate on a fixture app (verified in 430-07 E2E with the test-harness app).

#### Commit message

```
feat(430-03): tap primitive + SelectorResolver with 100ms retry

Sealed Selector hierarchy (ResourceId, ContentDesc, Text, Xpath).
SelectorResolver uses findAccessibilityNodeInfosByViewId/ByText + BFS
for content-desc; 100ms budget with 20ms polls.  GestureDispatcher wraps
dispatchGesture in suspendCancellableCoroutine with 1s hard timeout.
/ui/tap route returns 200/404/501/503 per PROTOCOL.md schema.  Xpath
is a structured 501 placeholder (full DSL in Phase 433).

Covers: ACCESS-03 (tap portion)
Not tested: swipe, text input, 95% success rate on harness (430-04, 430-07).
```

---

### 430-04-PLAN — Swipe + text-input primitives

**Goal:** `POST /ui/swipe` and `POST /ui/text` round out the primitive set. Both reuse SelectorResolver + GestureDispatcher where applicable.

**Covers:** ACCESS-03 (swipe + text portions)

**Dependencies:** 430-03

**Type:** `auto`

#### Tasks

1. Extend `GestureDispatcher.kt`:
   ```kotlin
   suspend fun swipe(x1: Int, y1: Int, x2: Int, y2: Int, durationMs: Long): Boolean = suspendCancellableCoroutine { cont ->
       val svc = bridge.tryGetService() ?: run { cont.resume(false); return@suspendCancellableCoroutine }
       val path = android.graphics.Path().apply { moveTo(x1.toFloat(), y1.toFloat()); lineTo(x2.toFloat(), y2.toFloat()) }
       val gesture = GestureDescription.Builder()
           .addStroke(GestureDescription.StrokeDescription(path, 0, durationMs))
           .build()
       val timeoutJob = serviceScope.launch {
           delay(durationMs + 2000)
           if (!cont.isCompleted) cont.resume(false)
       }
       svc.dispatchGesture(gesture, object : AccessibilityService.GestureResultCallback() {
           override fun onCompleted(g: GestureDescription?) { timeoutJob.cancel(); if (!cont.isCompleted) cont.resume(true) }
           override fun onCancelled(g: GestureDescription?) { timeoutJob.cancel(); if (!cont.isCompleted) cont.resume(false) }
       }, null)
   }
   ```
   - `durationMs` clamped between 50 (fast flick) and 3000 (long drag) with 400 default; enforce in route handler.
   - Timeout = durationMs + 2000 to account for OS scheduling jitter.

2. Create `TextInputDispatcher.kt`:
   ```kotlin
   class TextInputDispatcher {
       fun inputText(node: AccessibilityNodeInfo, text: String): Boolean {
           if (!node.isEditable) return false
           // Focus the field first
           node.performAction(AccessibilityNodeInfo.ACTION_FOCUS)
           // Set text via the standard ACTION_SET_TEXT bundle
           val args = android.os.Bundle().apply {
               putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text)
           }
           return node.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, args)
       }
   }
   ```
   - Uses `ACTION_SET_TEXT` (API 21+) — replaces the entire field content.
   - `isEditable` guard prevents silent failure on non-text targets (e.g., button mistakenly typed into).
   - No selector extension here — the caller resolves the selector first (same flow as tap).

3. Extend `UiRoutes.kt`:
   ```kotlin
   post("/ui/swipe") {
       if (!call.ensureEnabled()) return@post
       val req = call.receive<SwipeRequest>()
       val duration = req.duration_ms.coerceIn(50, 3000)
       val ok = dispatcher.swipe(req.x1, req.y1, req.x2, req.y2, duration)
       call.respond(if (ok) HttpStatusCode.OK else HttpStatusCode.InternalServerError,
           mapOf("ok" to ok, "duration_ms" to duration))
   }
   post("/ui/text") {
       if (!call.ensureEnabled()) return@post
       val req = call.receive<TextInputRequest>()
       resolver.resolve(req.selector, req.timeout_ms ?: 100).fold(
           onSuccess = { node ->
               val ok = textDispatcher.inputText(node, req.text)
               node.recycle()
               call.respond(if (ok) HttpStatusCode.OK else HttpStatusCode.UnprocessableEntity,
                   mapOf("ok" to ok))
           },
           onFailure = { ex -> /* same dispatch as /ui/tap */ }
       )
   }
   ```

4. Unit tests:
   - `GestureDispatcherTest.swipeCompletes` — as with tap, stub callbacks.
   - `TextInputDispatcherTest.setsTextOnEditable` — stub AccessibilityNodeInfo with `isEditable=true`; assert `performAction(ACTION_SET_TEXT, args)` invoked with correct bundle.
   - `TextInputDispatcherTest.rejectsNonEditable` — stub `isEditable=false`; assert returns false without calling performAction.
   - Instrumented (430-07 scope, listed here for completeness): real tap on a button in a fixture app; real type into a `<EditText>`.

5. Update `PROTOCOL.md` with `/ui/swipe` + `/ui/text` schemas.

#### Acceptance

- Unit tests pass (3 new cases).
- On Tab Plus with the Settings app foregrounded and the search bar visible: `POST /ui/text { "selector": { "strategy": "resource_id", "value": "com.android.settings:id/search_view" }, "text": "Wi-Fi" }` types "Wi-Fi" into the search bar, observable on the device (visual confirmation in 430-07 checkpoint).
- `POST /ui/swipe` with from bottom-center to top-center causes scroll.
- Out-of-range `duration_ms` values are clamped to [50, 3000].

#### G4 NOT TESTED list

- Disabled-state 503 full coverage (430-05 closes).
- First-run setup Activity (430-06).
- 95% tap success rate on fixture (430-07 E2E).

#### Commit message

```
feat(430-04): swipe + text-input primitives via GestureDescription + ACTION_SET_TEXT

/ui/swipe uses Path.moveTo + lineTo with GestureDescription.StrokeDescription;
duration clamped 50-3000ms.  /ui/text uses AccessibilityNodeInfo.ACTION_SET_TEXT
(requires isEditable).  Both endpoints share the SelectorResolver from 430-03
and the 503 gate from the UiRoutes block.

Covers: ACCESS-03 (full)
Not tested: 503 gate full scenarios, first-run UX, E2E harness (430-05/06/07).
```

---

### 430-05-PLAN — 503 gate + notification warning on disabled state

**Goal:** When Accessibility Service is disabled (never enabled, user toggled off, or service crashed), every UI action endpoint returns 503 with a structured body. Persistent notification updates to warn the user. State detection is continuous, not just at startup.

**Covers:** ACCESS-05

**Dependencies:** 430-01, 430-02 (UiRoutes scaffolding)

**Type:** `auto`

#### Tasks

1. Create `A11yStateMonitor.kt`:
   ```kotlin
   sealed class A11yState {
       object Enabled : A11yState()
       data class Disabled(val reason: String) : A11yState()  // "never_enabled" | "user_disabled" | "service_crashed"
       object Unknown : A11yState()
   }

   class A11yStateMonitor(private val context: Context, private val scope: CoroutineScope) {
       private val _state = MutableStateFlow<A11yState>(A11yState.Unknown)
       val state: StateFlow<A11yState> = _state.asStateFlow()
       private var job: Job? = null

       fun start() {
           job = scope.launch {
               while (isActive) {
                   val enabled = isServiceEnabledInSettings() && A11yBridge.isConnected()
                   val prev = _state.value
                   _state.value = when {
                       enabled -> A11yState.Enabled
                       isServiceEnabledInSettings() && !A11yBridge.isConnected() ->
                           A11yState.Disabled("service_crashed") // settings say on, but bridge disconnected
                       else ->
                           A11yState.Disabled(if (prev is A11yState.Enabled) "user_disabled" else "never_enabled")
                   }
                   if (prev != _state.value) {
                       RotatingLog.info("a11y", "state_transition",
                           mapOf("from" to prev.toString(), "to" to _state.value.toString()))
                   }
                   delay(10_000) // 10s poll
               }
           }
       }

       fun stop() { job?.cancel() }

       private fun isServiceEnabledInSettings(): Boolean {
           val enabled = Settings.Secure.getString(context.contentResolver,
               Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES) ?: return false
           return enabled.contains("${context.packageName}/.a11y.RcAccessibilityService")
       }
   }
   ```
   - **Two-source-of-truth check:** `ENABLED_ACCESSIBILITY_SERVICES` + `A11yBridge.isConnected()`. If the first says yes but the second says no, the service crashed — a known Android failure mode.
   - 10s poll interval is the standard compromise: responsive enough for UX, light enough for battery. Battery impact <0.5%/h.

2. Wire into `AgentForegroundService.kt`:
   ```kotlin
   override fun onCreate() {
       super.onCreate()
       // ... existing Phase 429 setup ...
       val monitor = A11yStateMonitor(this, serviceScope)
       monitor.start()
       serviceScope.launch {
           monitor.state.collect { s ->
               when (s) {
                   is A11yState.Enabled -> updateNotification(getString(R.string.app_name),
                       getString(R.string.notification_accessibility_on))
                   is A11yState.Disabled -> updateNotification(getString(R.string.app_name),
                       getString(R.string.notification_accessibility_off))
                   A11yState.Unknown -> { /* keep previous state */ }
               }
           }
       }
   }
   ```
   - Make the notification tap open `AccessibilitySetupActivity` (PendingIntent on the notification, configured in 430-06).

3. Verify all UI routes go through the 503 gate. In `UiRoutes.kt`, the `ensureEnabled()` suspend function (already added in 430-02) is the single gate. Audit pattern — `grep -n 'ensureEnabled' UiRoutes.kt` must return a hit on the FIRST line of every `get`/`post` UI route body.

4. Update `/screen/tree` to differentiate Unknown vs Disabled: if monitor reports `Unknown` (within first 10s of service start), return 503 with `error: "a11y_state_initializing"` — prevents false negatives at boot.

5. Unit test `A11yStateMonitorTest.kt`:
   - Stub `Settings.Secure.getString` to return a non-matching string; assert state is `Disabled("never_enabled")`.
   - Stub to return matching string AND stub `A11yBridge.isConnected()` = true; assert state flips to `Enabled`.
   - Stub matching + bridge disconnected; assert `Disabled("service_crashed")`.
   - Start -> stop; assert job cancelled and no further polls.

6. Update `PROTOCOL.md` with the 503 response body shape + state transition log format.

#### Acceptance

- Unit tests pass (4 cases).
- On a device with Accessibility DISABLED:
  - `POST /ui/tap`, `/ui/swipe`, `/ui/text`, `GET /screen/tree` all return 503 with `error: "accessibility_service_disabled"` body.
  - Persistent notification title+body reads "RC Agent Mobile - Accessibility OFF - tap to enable".
- Enable Accessibility in Settings, wait 10 seconds (one poll cycle).
  - Notification flips to "Connected - automation ready".
  - `POST /ui/tap` now returns 200 (or 404 if selector misses).
- Disable Accessibility in Settings.
  - Within 10s, all endpoints 503 again.
- Simulate service crash: `adb shell am force-stop in.racingpoint.rcagentmobile`. On next launch with toggle still on, monitor detects `service_crashed` (reason in log).

#### G4 NOT TESTED list

- First-run setup Activity flow (430-06).
- Long-term monitor drift (week-long test — 430-07 drill + production).

#### Commit message

```
feat(430-05): 503 gate + A11yStateMonitor + notification warning state

A11yStateMonitor polls Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES + 
A11yBridge.isConnected() every 10s with a 3-state StateFlow
(Enabled, Disabled[reason], Unknown).  UiRoutes.ensureEnabled() is the
single 503 gate, called first in every UI handler.  Persistent notification
flips between "Accessibility OFF" and "Connected - automation ready" on
state transition.  service_crashed vs user_disabled reasons logged.

Covers: ACCESS-05
Not tested: first-run Activity (430-06), long-term drift (430-07 + prod).
```

---

### 430-06-PLAN — First-run Activity + Settings deep-link + toggle poll

**Goal:** A single-screen Activity launched when Accessibility is disabled that:
1. Explains why Accessibility is needed,
2. Opens the Android Settings -> Accessibility page on button tap,
3. Polls the state and auto-dismisses when the user enables the toggle.

**Covers:** ACCESS-04

**Dependencies:** 430-05 (A11yStateMonitor)

**Type:** `checkpoint:human-verify` at end (physical Settings toggle on both devices)

#### Tasks

1. Create `res/layout/activity_accessibility_setup.xml`:
   ```xml
   <?xml version="1.0" encoding="utf-8"?>
   <LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"
       android:orientation="vertical"
       android:padding="24dp"
       android:layout_width="match_parent"
       android:layout_height="match_parent">
       <TextView
           android:id="@+id/title"
           android:text="@string/accessibility_setup_title"
           android:textSize="24sp"
           android:layout_marginBottom="16dp"
           android:layout_width="match_parent"
           android:layout_height="wrap_content" />
       <TextView
           android:id="@+id/body"
           android:text="@string/accessibility_setup_body"
           android:textSize="16sp"
           android:layout_marginBottom="24dp"
           android:layout_width="match_parent"
           android:layout_height="wrap_content" />
       <Button
           android:id="@+id/btn_open_settings"
           android:text="@string/accessibility_setup_button"
           android:layout_width="match_parent"
           android:layout_height="wrap_content" />
       <TextView
           android:id="@+id/status"
           android:text="@string/accessibility_setup_waiting"
           android:textSize="14sp"
           android:visibility="gone"
           android:layout_marginTop="16dp"
           android:layout_width="match_parent"
           android:layout_height="wrap_content" />
   </LinearLayout>
   ```
   - Zero customer-facing aesthetic concerns — this is a staff-only transient screen. Default Material theme, no branding. This is why gates `ui_researcher` and `ui_auditor` are skipped.

2. Create `AccessibilitySetupActivity.kt`:
   ```kotlin
   class AccessibilitySetupActivity : AppCompatActivity() {
       private lateinit var monitor: A11yStateMonitor
       override fun onCreate(savedInstanceState: Bundle?) {
           super.onCreate(savedInstanceState)
           setContentView(R.layout.activity_accessibility_setup)
           monitor = A11yStateMonitor(this, lifecycleScope)
           findViewById<Button>(R.id.btn_open_settings).setOnClickListener {
               startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
                   addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
               })
               findViewById<View>(R.id.status).visibility = View.VISIBLE
           }
           monitor.start()
           lifecycleScope.launch {
               monitor.state.collect { s ->
                   if (s is A11yState.Enabled) {
                       RotatingLog.info("a11y", "first_run_enabled")
                       Toast.makeText(this@AccessibilitySetupActivity,
                           "RC Agent Mobile is now active.", Toast.LENGTH_SHORT).show()
                       finish()
                   }
               }
           }
       }
       override fun onDestroy() {
           monitor.stop()
           super.onDestroy()
       }
   }
   ```
   - `lifecycleScope` (from androidx.lifecycle) ties the polling coroutine to the Activity lifecycle — no leaks on rotation/finish.
   - Auto-dismisses by calling `finish()` as soon as state flips to Enabled. User returns to whatever launched the Activity (first-run flow, notification tap).
   - `Toast` gives a positive confirmation — subtle, not intrusive.

3. Manifest — register Activity:
   ```xml
   <activity
       android:name=".firstrun.AccessibilitySetupActivity"
       android:exported="true"
       android:theme="@style/Theme.MaterialComponents.Light.NoActionBar">
       <intent-filter>
           <action android:name="android.intent.action.VIEW" />
           <action android:name="in.racingpoint.rcagentmobile.SETUP_ACCESSIBILITY" />
           <category android:name="android.intent.category.DEFAULT" />
       </intent-filter>
   </activity>
   ```

4. Wire notification tap to this Activity in `AgentForegroundService.updateNotification`:
   ```kotlin
   val openSetup = Intent(this, AccessibilitySetupActivity::class.java).apply {
       addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
   }
   val pi = PendingIntent.getActivity(this, 0, openSetup, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
   val builder = NotificationCompat.Builder(this, CHANNEL_ID)
       .setContentTitle(title)
       .setContentText(body)
       .setContentIntent(pi)
       // ...
   ```
   - Tapping the "Accessibility OFF" notification goes directly to the setup Activity.

5. MainActivity (from Phase 429) — on launch, if A11y is disabled, redirect to `AccessibilitySetupActivity`; otherwise just start the Foreground Service and finish:
   ```kotlin
   override fun onCreate(savedInstanceState: Bundle?) {
       super.onCreate(savedInstanceState)
       startForegroundService(Intent(this, AgentForegroundService::class.java))
       val enabled = /* synchronous Settings.Secure check */
       if (!enabled) {
           startActivity(Intent(this, AccessibilitySetupActivity::class.java))
       }
       finish()
   }
   ```

6. Write `docs/ACCESSIBILITY-NOTES.md`:
   - Step-by-step for staff: install APK, tap icon once, follow AccessibilitySetupActivity prompt, toggle on.
   - **Android 13+ restricted-settings workaround:** Some OEMs (esp. OneUI on Samsung M07) block the Accessibility toggle for sideloaded APKs. Workaround: Settings -> Apps -> RC Agent Mobile -> three-dot menu -> "Allow restricted settings" -> then return to Accessibility and toggle on. Include screenshots (filename stubs — actual screenshots captured during 430-07 checkpoint).
   - OEM-specific notes for Tab Plus (Lenovo) and M07 (Samsung).
   - Battery impact note: +3-5%/h overhead from running Accessibility; document so staff can plan charging.

7. Unit tests are minimal here (Activity testing is instrumented territory):
   - `AccessibilitySetupActivityTest` (JVM-local, using Robolectric or MockK): assert button click fires an Intent with action `Settings.ACTION_ACCESSIBILITY_SETTINGS`.

#### Acceptance (with human-verify checkpoint)

- Install the updated APK on Tab Plus with Accessibility previously disabled.
- Tap the app icon — `AccessibilitySetupActivity` appears within 2 seconds.
- Tap "Open Accessibility Settings" — Settings app opens directly on the Accessibility page.
- Toggle "RC Agent Mobile" on, press Back. Within 2 seconds (one poll of the Activity + one state-flow emit), the Activity auto-dismisses with Toast "RC Agent Mobile is now active."
- Persistent notification now reads "Connected - automation ready".
- Repeat on M07 (expect the Android 13+ restricted-settings prompt on sideloaded APKs — document the workaround).

#### Checkpoint (human-verify)

User installs the APK on both devices, runs through the first-run flow, and reports:
- "Tab Plus first-run: opened setup screen, tapped button, enabled toggle, auto-dismissed in N seconds."
- "M07 first-run: same — OR describes if restricted-settings workaround was needed."
- Screenshots of the restricted-settings screen (if encountered) added to ACCESSIBILITY-NOTES.md.

Resume signal: user reports both devices through the flow successfully (or reports the failure mode — e.g., restricted-settings block).

#### G4 NOT TESTED list

- Long-running durability of the polling Activity across rotation / process death (one-off transient Activity — low risk).
- 95% tap success rate on the Zomato Partner app (430-07 + Phase 432).

#### Commit message

```
feat(430-06): AccessibilitySetupActivity + Settings deep-link + toggle poll

Single-screen Activity reachable via MainActivity (on a disabled-state launch)
or persistent-notification tap.  Opens Settings.ACTION_ACCESSIBILITY_SETTINGS
on button tap; polls A11yStateMonitor.state; auto-dismisses within 2s of
user toggling Accessibility on.  ACCESSIBILITY-NOTES.md documents the
Android 13+ restricted-settings workaround for sideloaded APKs.

Covers: ACCESS-04
Not tested: 95% tap success rate on production apps (430-07 + Phase 432).
```

---

### 430-07-PLAN — Unit + instrumented tests + Tab Plus + M07 E2E verification

**Goal:** Bring the whole Phase 430 surface up to the CLAUDE.md nyquist bar: every primitive has deterministic instrumented coverage, SC-2's 95% success figure is measured, and both physical devices are verified end-to-end.

**Covers:** ACCESS-01..05 (verification, no net-new implementation)

**Dependencies:** 430-01 through 430-06

**Type:** `checkpoint:human-verify` (physical devices + harness)

#### Tasks

1. Create a minimal test fixture app — `rc-agent-mobile/fixture/` (new Gradle module):
   - 4 screens: Home, ButtonList (20 uniquely-resource-id'd buttons), ScrollableList, TextInputForm.
   - Each button logs a clearly-identifiable message to logcat when tapped (so the instrumented test can assert which button was triggered).
   - Target package: `in.racingpoint.rcagentmobilefixture` (distinct so the agent can't accidentally match its own UI).
   - Installed alongside the main APK on both devices during the drill.

2. Instrumented test `AccessibilityInstrumentedTest.kt` (in `app/src/androidTest/`):
   - **SC-2 measurement (95% tap success rate):**
     ```kotlin
     @Test fun tapSuccessRateAtLeast95Percent() = runBlocking {
         val results = (1..20).map {
             val r = client.post("http://127.0.0.1:8090/ui/tap") {
                 setBody(mapOf("selector" to mapOf("strategy" to "resource_id",
                     "value" to "in.racingpoint.rcagentmobilefixture:id/btn_$it")))
             }
             r.status == HttpStatusCode.OK && logcatContainsWithin(1000L, "FixtureButton:$it:tapped")
         }
         val success = results.count { it }
         assertTrue("Tap success rate was $success/20, need >= 19", success >= 19)
     }
     ```
   - `/screen/tree` latency test — 10 calls, assert p99 < 500ms.
   - Disabled-state test — turn off toggle via `adb shell settings put secure enabled_accessibility_services ""`, assert all UI endpoints 503.
   - Re-enable test — turn toggle back on, poll until endpoints 200.
   - Swipe test — swipe up in ScrollableList, assert fixture logcat shows `ScrollEvent: position_changed`.
   - Text-input test — type "hello" into TextInputForm field, assert `/screen/tree` on the form shows the field text == "hello".

3. `./gradlew :app:connectedAndroidTest` runs the instrumented suite on whichever device is connected. Run twice (once per device).

4. Physical drill script (operator runs this):
   1. Uninstall current APKs (main + fixture) on both devices: `adb uninstall in.racingpoint.rcagentmobile; adb uninstall in.racingpoint.rcagentmobilefixture`.
   2. Clean install main APK + fixture APK on Tab Plus.
   3. Tap RC Agent Mobile icon — go through 430-06 first-run flow, enable Accessibility toggle.
   4. Foreground the fixture app on-device.
   5. From James .27 laptop: `./tests/a11y-drill.sh tab_plus` which runs:
      - 20 tap trials via `/ui/tap` against each of the 20 fixture buttons, count successes.
      - 5 `/screen/tree` calls, measure latencies.
      - 1 text-input round-trip into TextInputForm.
      - 1 swipe.
      - 1 disable/re-enable cycle via adb.
   6. Repeat on M07.
   7. Save drill output + logcat + `/screen/tree` JSON samples to `SUMMARY.md` evidence section.

5. Nyquist audit handoff: before the drill is marked complete, run `gsd-nyquist-auditor` on the Phase 430 deliverables (as declared in gates). Fix any P1/P0 findings before close.

6. MMA audit: run `node scripts/multi-model-audit.js` with the Phase 430 PLAN + diff bundle as input. Dual-mode required (abstract + trace-level). Budget: $5. Address consensus findings.

#### Acceptance (all of the below)

- [ ] Unit tests pass on CI: `./gradlew :app:testDebugUnitTest` exit 0 (all 430-01..05 tests).
- [ ] Instrumented tests pass on Tab Plus: `./gradlew :app:connectedAndroidTest` exit 0.
- [ ] Instrumented tests pass on M07: same, device connected.
- [ ] Physical drill script — Tab Plus: 20-trial tap success >= 19 (>= 95%). `/screen/tree` p99 < 500ms. Disable -> 503. Re-enable -> 200.
- [ ] Physical drill script — M07: same.
- [ ] `gsd-nyquist-auditor` report: 0 P0, 0 P1 findings (or all addressed).
- [ ] MMA audit: no consensus-blocker findings (or all addressed).
- [ ] First-run flow verified on both devices (430-06 checkpoint closes here as part of the drill).
- [ ] SUMMARY.md filled with all evidence.

#### Checkpoint (human-verify)

User runs the drill script on both devices, reports pass/fail for each acceptance bullet with exact numbers. If any bullet fails, create a gap-closure plan (430-08 or 431-prep) — do NOT mark Phase 430 complete.

#### G4 NOT TESTED list

- Real-world 7-day runtime stability — only caught by Phase 432+ in production.
- ToS risk of Zomato Partner automation — Phase 16 incident playbook.

#### Commit message

```
test(430-07): Phase 430 E2E drill + instrumented tests + nyquist + MMA

Fixture app + /ui/tap 20-trial success-rate harness (>=95% achieved).
Instrumented tests cover screen-tree latency, tap, swipe, text-input,
and full disable/re-enable cycle.  Nyquist audit + MMA audit both clean.
Drill passed on Tab Plus + M07.  SUMMARY.md has all evidence.

Covers: verification of ACCESS-01..05
```

---

## 6. Risks and pitfalls (Android-specific)

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **Android 13+ restricted settings** block Accessibility toggle for sideloaded APKs. User must "Allow restricted settings" first. | 430-06 documents the workaround in ACCESSIBILITY-NOTES.md with screenshots; Phase 431 first-run UX may automate the detection. For Phase 430: checkpoint captures the workaround on video. |
| R-2 | **Android 14 AccessibilityService lifecycle restrictions** — OS disconnects slow services. `onAccessibilityEvent` must return in <5ms. | 430-01's event handler is a no-op pass-through. Heavy work (tree reads, gesture dispatch) runs on a separate Dispatchers.IO coroutine. Nyquist audit flagged to verify. |
| R-3 | **`getRootInActiveWindow` returns null during focus transitions** (animations, IME show/hide). | 430-02 retries up to 100ms with 10ms polls before giving up with a structured error. |
| R-4 | **GestureDescription callbacks can be dropped** if service disconnects mid-gesture. | 430-03 wraps dispatchGesture in `suspendCancellableCoroutine` with a 1s hard timeout. Returns `false` cleanly, never hangs. |
| R-5 | **Samsung One UI + Lenovo skins** may add extra permission prompts on first Accessibility enable. | Cannot fully automate in code (Android forbids it). 430-06 deep-links to the correct Settings page; first-run UX documents the manual steps. |
| R-6 | **Screen tree exceeds 1 MB on complex apps** (Instagram, YouTube). | 430-02 caps `max_depth=30` and `max_nodes=5000`. Truncation is a first-class response field so callers know the tree was cut. |
| R-7 | **xpath selector is a placeholder in 430-03** — compile-time closed, runtime rejected. | Phase 433 implements the full DSL. 430-03 returns HTTP 501 with `implemented_in_phase: 433`. Selector enum is future-compatible. |
| R-8 | **Crashed AccessibilityService silently disables itself** — OS does not notify the user. | 430-05 `A11yStateMonitor` polls `ENABLED_ACCESSIBILITY_SERVICES` AND `A11yBridge.isConnected()`. If settings say on but bridge is disconnected → `Disabled("service_crashed")` logged + notification + 503. |
| R-9 | **AccessibilityNodeInfo pool leak** — forgetting to recycle nodes degrades the OS. | 430-02 recycles explicitly in `convert()` and after caller uses. 430-03 recycles in SelectorResolver and at the route handler level. Runtime audit via `adb shell dumpsys accessibility`. |
| R-10 | **Battery drain** — running AccessibilityService adds 3-5%/h idle overhead. | Documented in ACCESSIBILITY-NOTES.md. Expected cost; cannot be mitigated by code. |
| R-11 | **Lock-held-across-await** (CLAUDE.md rule ported from Rust). | All state reads use atomics (AtomicReference in A11yBridge, StateFlow in A11yStateMonitor). Nyquist audit flagged. |
| R-12 | **Emulator ≠ real device** — AccessibilityService behavior differs subtly. | Only real-device tests count. 430-06 checkpoint + 430-07 drill are all real-device. Instrumented tests fail loudly if device not connected. |
| R-13 | **Fixture app's resource-ids drift** if the fixture is rebuilt independently. | Fixture lives in `rc-agent-mobile/fixture/` (same monorepo), built from the same commit, same minSdk. Resource-ids are stable. |

## 7. Test plan

### Unit tests (JVM, fast, on every build)
- `RcAccessibilityServiceTest` (430-01)
- `ScreenTreeReaderTest` (430-02)
- `SelectorResolverTest` (430-03)
- `GestureDispatcherTest` (430-03 + 430-04)
- `TextInputDispatcherTest` (430-04)
- `A11yStateMonitorTest` (430-05)
- `AccessibilitySetupActivityTest` (430-06, Robolectric)

All run via `./gradlew :app:testDebugUnitTest`.

### Instrumented tests (run with a connected device)
- `AccessibilityInstrumentedTest` (430-07):
  - `screenTreeLatencyP99Under500ms`
  - `tapSuccessRateAtLeast95Percent` (20 trials)
  - `swipeTriggersScroll`
  - `textInputSetsFieldContent`
  - `disabledStateReturns503`
  - `reEnableFlipsTo200`

### Physical device tests (human-verify checkpoints)
- 430-06 checkpoint: first-run flow on Tab Plus + M07.
- 430-07 drill: full 20-trial harness on both devices + nyquist + MMA.

### Drill tooling
- `rc-agent-mobile/tests/a11y-drill.sh <device>` — runs 20 taps, 5 screen-tree calls, 1 swipe, 1 text-input, 1 disable/re-enable cycle. Outputs JSON report for SUMMARY.md.

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** Selector resolution, 100ms retry budget, gesture dispatch, 503 gate — all business logic with crisp input/output contracts. Run before 430-07 closes.
- **MMA audit (required):** Accessibility Service is a HIGH-risk Android surface (Android 13+ restricted-settings rules, crashed-service silent failure mode, OEM skins). Cross-boundary: user-controlled toggle <-> agent runtime behavior. Dual reasoning modes per CLAUDE.md: abstract (find architecture bugs like "what if the monitor's poll lands between user-toggle-off and next HTTP call?") + trace-level (find execution bugs like "what is the value of lastHeartbeatAt when A11yBridge unregisters mid-request?"). Budget: $5.
- **integration-checker (deferred):** Phase 430 is single-device. Defer to v50.0 milestone ship.
- **codebase-mapper (optional):** Only needed if `rc-agent-mobile/` top-level structure changes. It does not in Phase 430.
- **ui-researcher / ui-auditor (skipped):** `AccessibilitySetupActivity` is a single-screen staff-only transient setup surface, not a customer-facing product surface. Default Material theme, no branding decisions. If Phase 431 later absorbs this into a multi-step Bootstrap flow, UI-SPEC becomes mandatory there.
- **SEC gate:** Phase 430 adds no new HTTP auth surface (UI endpoints inherit Phase 429's auth middleware — confirm in 430-07). `node comms-link/test/security-check.js` must pass after the phase.
- **Deploy Manifest Protocol (DMP):** Tick every `deploy:` frontmatter item during 430-07.
- **Backlog gate (CLAUDE.md CGP v4.3):** Phase 430 must reach DEPLOYED-VERIFIED (APK installed on both devices + 430-07 drill passed) before Phase 431 may begin. COMMITTED != SHIPPED.

## 9. Open questions the planner cannot decide

These require a user decision before executing the flagged plans. Listed in execution-blocking order.

**OQ-1 — test fixture app: bundled monorepo module or separate APK project? (BLOCKS 430-07)**
Recommendation: new Gradle module `rc-agent-mobile/fixture/` in the same monorepo, with its own `app/build.gradle.kts` producing a distinct APK (`in.racingpoint.rcagentmobilefixture`). Rationale: single git repo, same commit = same fixture, no drift. Alternative: external fixture (e.g., BBC iPlayer for public apps) — rejected because we can't control UI changes. Confirm "yes, module" before 430-07 begins.

**OQ-2 — 95% success rate threshold: is 19/20 acceptable, or do we want 19/20 with exactly-which-failure triage? (BLOCKS 430-07 acceptance)**
ROADMAP-v50.md says ">= 95%". 19/20 passes. But the 1 failure should be understood, not ignored. Proposal: drill script records WHICH trial failed + full `/screen/tree` JSON at failure time + logcat — even if 19 pass, triage the 1 failure and log in SUMMARY.md. Does not block accept, but ensures signal quality.

**OQ-3 — should `A11yStateMonitor` also publish its state via the `/health` endpoint? (BLOCKS 430-05 API surface)**
Proposal: extend Phase 429's `/health` response with `"a11y_state": "enabled" | "disabled" | "unknown"`. Useful for fleet health at the server level (Phase 13 admin view). Low-cost extension. Recommend yes. Confirm in 430-05 scope or punt to Phase 3.

**OQ-4 — the notification tap on "Accessibility OFF" goes to `AccessibilitySetupActivity`. What about "Connected - automation ready"? (COSMETIC, non-blocking)**
Options: (a) tap does nothing (OS dismisses notification tap, reopens app), (b) tap opens a diagnostic screen with device_id / build_id / last-heartbeat (useful for James at the venue), (c) tap opens MainActivity which finishes immediately. Recommend (b) for operator ergonomics, but acceptable to defer to Phase 431.

**OQ-5 — xpath placeholder: accept ANY xpath string and reject with 501, or validate the syntax first? (MINOR, 430-03 scope)**
Recommendation: accept any string, return 501 with `implemented_in_phase: 433`. Reason: validating xpath without implementing it is wasted effort; the 501 response is the contract. Confirm for 430-03.

**OQ-6 — Android 13+ restricted-settings workaround: document-only, or attempt automation? (BLOCKS 430-06 scope decision)**
Android officially provides no programmatic way to bypass restricted-settings for sideloaded apps. The only published workarounds are either (a) ADB install with `-g` flag (grants runtime permissions — does NOT cover restricted-settings), (b) device-owner provisioning via `dpm set-device-owner` (requires factory reset — not practical). Recommendation: document-only in ACCESSIBILITY-NOTES.md; add screenshots. Alternative: Phase 431 adds a runtime detection + a banner pointing the user at the app-info three-dot menu. Confirm scope for 430-06.

**OQ-7 — `/ui/swipe` selector-based overload: worth adding in 430-04, or coordinate-only? (SCOPE DECISION)**
Current plan: 430-04 ships coordinate-based `/ui/swipe {x1, y1, x2, y2, duration_ms}`. Selector-based swipe (swipe-on-list-item) is a 433 concern. Recommend keep 430-04 coordinate-only for scope discipline. Confirm.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md` Phase 2)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` ACCESS-01..05
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Prior phase:** `.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md`
- **Downstream:** Phase 432 (Driver framework — consumes Selector + GestureDispatcher + ScreenTreeReader), Phase 433 (Selector DSL — replaces xpath placeholder with full engine), Phase 435 (Audit log — extends LogEvent with UiAction, SelectorMiss subclasses)

## 11. Output (at phase close)

At the end of Plan 430-07 (E2E drill pass + audits clean), create `.planning/phases/430-accessibility-service-foundation/SUMMARY.md` capturing:
- Which commits implemented each plan (430-01 through 430-07)
- Stopwatch measurements for SC-1 (screen-tree p99 latency), SC-2 (tap success rate), SC-4 (Settings deep-link -> toggle -> auto-dismiss time)
- Log excerpts (tailed JSONL from both devices showing a11y state transitions + selector resolutions)
- Nyquist audit report summary
- MMA audit report summary + cost
- Any risks encountered and how they were resolved (update §6 state)
- Any open questions resolved during execution (update §9 state)
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked, especially the "user must enable Accessibility toggle" operator step
- ACCESSIBILITY-NOTES.md final version with real screenshots of the restricted-settings workaround (if encountered on M07)
- Handoff to Phase 431 (Bootstrap install + first-run UX) — what's ready (A11yStateMonitor, AccessibilitySetupActivity deep-link, restricted-settings doc), what Phase 431 still owns (the full permissions checklist: battery unrestricted + install-unknown-apps + overlay + notifications)

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 2 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
