---
phase: 433-selector-dsl-hot-reload
phase_number: 433
milestone: v50.0 rc-agent-mobile
name: "Selector DSL + Hot-Reload + Local Remote-Push"
status: ready-to-execute
goal: >
  YAML selector files become the source of truth for every UI action the agent takes.
  Editing `selectors.yaml` on the device takes effect within 10 seconds without
  restarting the agent. Selector maps are versioned per target-app version
  (`<app_package>/<app_version>/selectors.yaml`) with a fallback chain:
  current-version → previous-version → fail loudly with a structured
  SelectorMissEvent. James can author selectors via debug mode that captures
  the current-screen Accessibility tree into a commit-ready YAML stub.
  Agent accepts signed selector-patch payloads via comms-link and applies
  them atomically with rollback on parse failure. Remote-push ADMIN UI is
  explicitly deferred to Phase 443.
requirements: [SELECTOR-01, SELECTOR-02, SELECTOR-03, SELECTOR-04, SELECTOR-05, SELECTOR-06]
depends_on: [432-driver-framework-capability-registry]
wave: 5
plan_count: 8
plans:
  - 433-01-PLAN: YAML schema + SelectorMap Kotlin data model + parser
  - 433-02-PLAN: Selector matching engine (node-tree walk + confidence scoring)
  - 433-03-PLAN: FileObserver-based hot-reload (≤ 10s latency)
  - 433-04-PLAN: Per-app-version resolution + fallback chain + app-update hook
  - 433-05-PLAN: SelectorMissEvent emission (screenshot hash + last-known-good + version)
  - 433-06-PLAN: Debug capture mode (Accessibility tree → YAML stub endpoint)
  - 433-07-PLAN: Remote push endpoint (signature verify + write + hot-reload + rollback)
  - 433-08-PLAN: Unit tests + Tab Plus integration test (parse, fallback, signature, E2E)
autonomous: false   # Plan 433-08 contains a human-verify checkpoint (physical Tab Plus integration).
files_modified:
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorSchema.kt        # 433-01
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorMap.kt           # 433-01
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorParser.kt        # 433-01
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorMatcher.kt       # 433-02
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/MatchResult.kt           # 433-02
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorStore.kt         # 433-03
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorFileWatcher.kt   # 433-03
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorResolver.kt      # 433-04
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorMissEvent.kt     # 433-05
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorEventBus.kt      # 433-05
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/debug/SelectorCaptureEndpoint.kt  # 433-06
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/debug/NodeTreeToYamlStub.kt       # 433-06
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/SelectorPushHandler.kt   # 433-07
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/selector/PatchSignatureVerifier.kt # 433-07
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/LocalHttpServer.kt           # 433-06 + 433-07 (add routes)
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/CommsLinkClient.kt          # 433-07 (wire push handler)
  - rc-agent-mobile/app/src/test/kotlin/in/racingpoint/rcagentmobile/selector/                        # unit tests
  - rc-agent-mobile/app/build.gradle.kts                                                              # YAML + crypto deps
  - rc-agent-mobile/docs/SELECTORS.md                                                                 # authoring + versioning guide
  - rc-agent-mobile/docs/PROTOCOL.md                                                                  # amended: selector-push envelope + selector_miss event
  - comms-link/shared/agent-protocol-v1.md                                                            # mirror of PROTOCOL.md amendment
  - rc-agent-mobile/app/src/main/assets/selectors-sample/                                             # v3.14.2/selectors.yaml template
  - .planning/phases/433-selector-dsl-hot-reload/SUMMARY.md                                           # filled at close

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: >
    New APK rolled to Tab Plus + M07 (adb install -r).
    On-device directory created at filesDir/selectors/ (app-private).
    No server-side changes in Phase 433 — push flows land in Phase 443
    when the ADMIN dashboard learns to call the comms-link selector_push
    envelope. The agent-side handler IS ready in Phase 433 and can be
    exercised via a one-liner CLI for integration testing.
  data_files: >
    rc-agent-mobile/app/src/main/assets/selectors-sample/zomato-partner/v3.14.2/selectors.yaml
    (packaged with the APK; copied to filesDir/selectors/ on first run if
    no selectors are present — bootstraps an empty device.)
    Also: rc-agent-mobile/keystores/signing-pubkey.pem (Ed25519 public key
    baked into BuildConfig — see OQ-1).
  bat_file: none
  cloud_parity:
    - "No comms-link relay protocol changes in 433 beyond SELECTOR envelope shape definition. The envelope is a PASSTHROUGH — relay already forwards arbitrary typed envelopes between clients (per comms-link/docs/PROTOCOL.md). The ONLY relay-side cascade: document the new `type: selector_push` and `type: selector_miss` shapes in comms-link/shared/agent-protocol-v1.md so Phase 443 admin UI implementer has the contract."
    - "Bono VPS comms-link — doc mirror only (DEPLOY PARITY rule)."
  targets:
    - tab_plus   # Lenovo TB-351FU
    - m07        # Samsung Galaxy M07
    - james_27   # comms-link doc mirror
    - bono_vps   # comms-link doc mirror
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  rollback:
    - "Previous APK preserved at /sdcard/Download/rc-agent-mobile-prev.apk."
    - "Selector rollback is DATA not BINARY: `adb shell rm -rf filesDir/selectors && adb shell cp -r filesDir/selectors.backup filesDir/selectors` — SelectorStore auto-rotates .backup/ before any remote push (see 433-07)."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip           # No customer UI in 433. Debug-mode endpoint output is JSON / YAML, consumed by James.
  ui_auditor: skip              # Same.
  nyquist_auditor: required     # Selector resolution is business-critical: every UI action the agent takes flows through the matcher. Version-fallback logic + signature verification are pure business logic — must have exhaustive test coverage.
  mma_audit: required           # Signature verification, key-handling, hot-reload concurrency, and selector-miss event fan-out are all non-local reasoning. CLAUDE.md: any cross-trust-boundary flow (signed patch arrives from admin → written to disk → hot-reloaded → next UI action uses it) requires dual-mode MMA (abstract + trace-level).
  integration_checker: required # Integrates with Phase 432 driver framework (DriverContext consumes selectors) and Phase 435 audit log (SelectorMissEvent is its first customer). Run before milestone ship.
  codebase_mapper: skip         # Module rc-agent-mobile/ already mapped by Phase 429-08 gate; new files are additions inside the existing module.

risks_summary:
  - "XPath evaluation against AccessibilityNodeInfo has no native Android support — implementing XPath adds ~400 lines and a runtime cost of O(nodes × predicates). Mitigation: ship only `resource_id`, `content_desc`, and `text` strategies in 433-02 runtime; XPath grammar is reserved in the schema but the matcher throws `UnsupportedStrategyException` for v1. Document the deferral in SELECTORS.md."
  - "Hot-reload concurrency: a selector map swap mid-action could produce undefined behavior (driver reads map A at step 1, map B at step 2). Mitigation: AtomicReference<SelectorMap> + each driver call snapshots the reference at action start; the file watcher only swaps the reference between actions, never mid-walk."
  - "Signed-patch signature scheme is an OPEN QUESTION (OQ-1) — Ed25519 via libsodium vs BouncyCastle vs platform KeyPairGenerator. Different choices have different APK-size and dependency-audit implications. Default selected in 433-07; flagged for user review."
  - "YAML parser choice (SnakeYAML-Engine vs Jackson YAML vs kotlinx-serialization-yaml) has APK-size and CVE-surface-area tradeoffs. Default: SnakeYAML-Engine (widely used on Android, MIT-licensed, ~400KB). Alternatives flagged in OQ-2."
  - "SelectorMissEvent → audit log is Phase 435's responsibility; in 433 the event goes into an in-memory bounded bus (SelectorEventBus) with a 100-event ring buffer. If Phase 435 ships later, events beyond the buffer are dropped. Mitigation: 433-05 also writes events to RotatingLog (Phase 429-07) so there is a persistent trail even before Phase 435."
  - "App-version detection relies on PackageManager.getPackageInfo(target_app_package).versionName, which returns null when the app is not installed. The resolver must treat null as `v-unknown` and select the newest selector map available, logging WARN. Covered by 433-04 unit test."
  - "Debug capture on a live screen may fire during a live customer action (Zomato order rendering). Mitigation: 433-06 endpoint is behind a service-key header (same key as /logs/tail in 429-07); admin intent is explicit."
  - "Selector miss storms — a broken selector on a high-frequency screen could fire 1000+ events/minute. Mitigation: 433-05 deduplicates within a 60s window by (driver, screen, element_name, app_version). Dedup key is stable — aligned with CLAUDE.md ErrorSpike dedup-key lesson from v27.0 MMA audit."
---

# Phase 433 — Selector DSL + Hot-Reload + Local Remote-Push

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 433 |
| Name | Selector DSL + Hot-Reload + Local Remote-Push |
| Milestone | v50.0 rc-agent-mobile |
| REQ-IDs covered | SELECTOR-01, SELECTOR-02, SELECTOR-03, SELECTOR-04, SELECTOR-05, SELECTOR-06 |
| Dependencies | Phase 432 (driver framework — DriverContext consumes selectors) |
| Wave | 5 (parallelizable with 434, 435, 436 after 432 closes, per ROADMAP-v50.md dependency graph) |
| Status | Ready to execute |
| Autonomous | No — 433-08 human-verify on physical Tab Plus |
| Ship test | (1) edit selectors.yaml on device, effect < 10s; (2) bump app version, old selectors still fall back; (3) break a selector → structured SelectorMissEvent with screenshot hash + last-known-good + app version; (4) `/debug/capture_stub` returns commit-ready YAML; (5) unsigned selector_push rejected; valid signed push applies + rolls back on parse fail |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 5)

1. **Hot-reload ≤ 10s.** Editing `selectors.yaml` on device takes effect within 10s without agent restart.
2. **Version fallback.** App version change triggers matching selector-map selection; old version remains as fallback until a newer map exists.
3. **Selector miss = structured event.** Selector miss emits `SelectorMissEvent` with screenshot hash + last-known-good selector + app version.
4. **Debug capture.** James can capture current-screen YAML stub via debug mode — a single command produces a commit-ready file.

## 3. Goal-backward must-haves

Derived from "what must be TRUE for each success criterion?"

### Truths (user-observable)

- T-1: A human editing `/data/data/in.racingpoint.rcagentmobile/files/selectors/<app>/<version>/selectors.yaml` via adb push (or via remote push) sees the change reflected in the next driver action within 10 seconds (SELECTOR-01, SELECTOR-03).
- T-2: When the target app is at v3.14.2 and only v3.14.2 selectors exist, the matcher uses v3.14.2. When the app auto-updates to v3.15.0 and no v3.15.0 selectors exist yet, the matcher logs WARN "no map for v3.15.0, falling back to v3.14.2" and continues operating until a v3.15.0 map arrives (SELECTOR-02).
- T-3: When the app auto-updates to v3.15.0 and the fallback map is also missing, the matcher emits a SelectorMissEvent AND throws a typed `NoSelectorMapException` back to the driver — the driver's exception handler is expected to pause the driver and alert (consistent with DRIVER-05 isolation). No silent failure (SELECTOR-02 + SELECTOR-05).
- T-4: When any selector fails to match an element, the matcher emits `SelectorMissEvent` containing: `{driver_id, screen, element_name, app_package, app_version, screenshot_hash, last_known_good: {strategy, value, last_matched_at_ms}, attempt_chain: [...]}` (SELECTOR-05).
- T-5: `POST http://<device_ip>:8090/debug/capture_stub` (with service-key header) returns a YAML payload that, if dropped into `selectors.yaml`, produces valid selectors for every element visible on the current screen — no manual cleanup needed beyond renaming elements (SELECTOR-06).
- T-6: A `selector_push` envelope arriving via comms-link with a valid Ed25519 signature causes the YAML on disk to update atomically; within 10s the new map is active. An invalid signature is rejected and logged with `register_rejected_signature` level ERROR (SELECTOR-04).
- T-7: A `selector_push` whose YAML fails to parse causes rollback to the pre-push file + a WARN log entry `selector_push_parse_failed_rolled_back` + an ACK to the sender with `{accepted: false, reason: "parse_error", detail: "..."}` (SELECTOR-04).
- T-8: `adb shell ls filesDir/selectors/zomato-partner/` shows at least one versioned directory (seed: `v3.14.2/selectors.yaml` copied from APK assets on first run).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/app/src/main/kotlin/.../selector/SelectorSchema.kt` | Enum of strategies + sealed classes | 60 | `SelectorStrategy` (RESOURCE_ID, CONTENT_DESC, TEXT, XPATH), `SelectorAttempt(strategy, value, timeout_ms)` |
| `.../selector/SelectorMap.kt` | Typed map: app → version → screen → element → [attempts] | 80 | `data class SelectorMap(val appPackage: String, val appVersion: String, val screens: Map<String, Map<String, List<SelectorAttempt>>>)` |
| `.../selector/SelectorParser.kt` | YAML → SelectorMap | 120 | SnakeYAML-Engine parse, validation (required fields, duplicate element names), throws `SelectorParseException` with line/col |
| `.../selector/SelectorMatcher.kt` | NodeInfo tree walk | 180 | Strategy dispatcher, depth-limited BFS, confidence score 0.0-1.0 |
| `.../selector/MatchResult.kt` | Typed match outcome | 40 | `sealed class MatchResult { data class Hit(node, strategy, confidence); data object Miss }` |
| `.../selector/SelectorStore.kt` | In-memory AtomicReference<SelectorMap-catalog> | 100 | Loads from filesDir/selectors at boot; atomic swap on reload |
| `.../selector/SelectorFileWatcher.kt` | Android FileObserver | 80 | Watches filesDir/selectors/ recursively, coalesces rapid events (300ms debounce), calls SelectorStore.reload() |
| `.../selector/SelectorResolver.kt` | Version-aware lookup + fallback chain | 90 | `resolve(appPackage, appVersion, screen, element) → SelectorAttempt[]`, falls back to newest-older-version, emits WARN |
| `.../selector/SelectorMissEvent.kt` | Typed event data class | 30 | Fields per T-4; serializable |
| `.../selector/SelectorEventBus.kt` | In-memory bounded event bus | 60 | MutableSharedFlow with replay=100, subscribers = 0..N |
| `.../debug/SelectorCaptureEndpoint.kt` | HTTP handler for /debug/capture_stub | 80 | Pulls current AccessibilityNodeInfo tree from Phase 430's service, calls NodeTreeToYamlStub.generate() |
| `.../debug/NodeTreeToYamlStub.kt` | Tree → YAML generator | 150 | For each interactable node, emits a `<element_name>: [{resource_id: ...}, {content_desc: ...}, {text: ...}]` tuple |
| `.../selector/SelectorPushHandler.kt` | comms-link push consumer | 140 | Signature verify → write .tmp → atomic rename → SelectorStore.reload() → ACK. On parse fail: rollback from .backup |
| `.../selector/PatchSignatureVerifier.kt` | Ed25519 signature check | 60 | Detached signature over canonical YAML bytes; public key from BuildConfig |
| `rc-agent-mobile/docs/SELECTORS.md` | Authoring guide | 200 | YAML schema spec, versioning rules, fallback chain, signing workflow for Phase 443 |
| `rc-agent-mobile/docs/PROTOCOL.md` (amend) | Envelope shapes for selector_push + selector_miss | +80 | Amends §"Message types" with the two new types |
| `rc-agent-mobile/app/src/main/assets/selectors-sample/zomato-partner/v3.14.2/selectors.yaml` | Seed / bootstrap map | 40 | A handful of placeholder selectors (login_screen, dashboard, order_row) — concrete values filled by Phase 437 |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| `DriverContext.findSelector(element)` (defined in 432) | `SelectorResolver.resolve(...)` | direct Kotlin call | grep `SelectorResolver` in `DriverContext.kt` (Phase 432) |
| `SelectorResolver.resolve` | `SelectorStore.getCatalog()` | direct Kotlin call | grep `SelectorStore.getCatalog` in `SelectorResolver.kt` |
| `SelectorStore.load` | `SelectorParser.parse` | direct Kotlin call | grep `SelectorParser.parse` in `SelectorStore.kt` |
| `SelectorFileWatcher.onEvent` | `SelectorStore.reload` | Android FileObserver callback | grep `SelectorStore.reload` in `SelectorFileWatcher.kt` |
| `SelectorMatcher.match(NO_HIT)` | `SelectorEventBus.emit(SelectorMissEvent)` | suspend function | grep `SelectorEventBus` in `SelectorMatcher.kt` |
| `SelectorEventBus` | `RotatingLog.error("selector", "miss", ...)` | subscriber coroutine | grep `RotatingLog` in `SelectorEventBus.kt` |
| `CommsLinkClient.onMessage(type=selector_push)` | `SelectorPushHandler.handle(envelope)` | dispatch by type | grep `SelectorPushHandler` in `CommsLinkClient.kt` |
| `SelectorPushHandler.handle` | `PatchSignatureVerifier.verify(yaml_bytes, sig)` | direct Kotlin call | grep `PatchSignatureVerifier.verify` |
| `SelectorPushHandler.handle (verify ok)` | `SelectorStore.writeAtomic` + `reload` | direct Kotlin call | grep `writeAtomic` + `reload` |
| `LocalHttpServer` | `SelectorCaptureEndpoint` | Ktor route registration | grep `registerRoute.*debug/capture_stub` in `LocalHttpServer.kt` |
| `AccessibilityService (Phase 430)` | `SelectorCaptureEndpoint` | snapshot source | grep `accessibilityService.rootNode` in `SelectorCaptureEndpoint.kt` |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md  # for patterns: RotatingLog, service-key HTTP, envelope shape, CommsLinkClient dispatch
@./.planning/phases/430-accessibility-service-foundation/PLAN.md  # for AccessibilityService API used by 433-06
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md  # for DriverContext interface — 433 implements its selector lookup surface
@./comms-link/docs/PROTOCOL.md  # for envelope shape inheritance

### Interfaces executors will need

#### A. DriverContext.findSelector (consumed from Phase 432)

Phase 432 defines the DriverContext interface; this phase provides its selector backend. The assumed contract (verified in Plan 433-01 Task 0):

```kotlin
// From Phase 432 (assumed — to be cross-checked at 433-01 kickoff)
interface DriverContext {
    val appPackage: String                  // e.g. "com.zomato.partner"
    val currentAppVersion: String?          // null if not installed

    // Phase 433 fills these:
    suspend fun findSelector(
        screen: String,                      // e.g. "login_screen"
        element: String                      // e.g. "username_field"
    ): MatchResult                           // sealed class from 433-02

    fun captureStub(screen: String): String // YAML text from 433-06, optional
}
```

If Phase 432's actual DriverContext differs, Task 0 of plan 433-01 resolves the discrepancy. **The selector API shape (`findSelector(screen, element)` returning a sealed MatchResult) is non-negotiable — anything else leaks matcher internals into every driver.**

#### B. YAML schema (authoritative for 433-01)

```yaml
# rc-agent-mobile/app/src/main/assets/selectors-sample/zomato-partner/v3.14.2/selectors.yaml
app_package: com.zomato.partner
app_version: "3.14.2"
generated_at: "2026-04-18T14:30:00+05:30"
generated_by: "james@racingpoint.in"
schema_version: 1

screens:
  login_screen:
    username_field:
      - strategy: resource_id
        value: com.zomato.partner:id/edt_username
        timeout_ms: 5000
      - strategy: content_desc
        value: "Username"
        timeout_ms: 5000
      - strategy: text
        value: "Enter your username"
        timeout_ms: 5000

    password_field:
      - strategy: resource_id
        value: com.zomato.partner:id/edt_password

    submit_button:
      - strategy: resource_id
        value: com.zomato.partner:id/btn_login
      - strategy: text
        value: "LOGIN"

  dashboard:
    pending_orders_count:
      - strategy: resource_id
        value: com.zomato.partner:id/txt_pending_count

    order_row:
      # order_row is a repeater — the driver iterates match_all()
      - strategy: resource_id
        value: com.zomato.partner:id/row_order
        all_matches: true

    accept_button_in_row:
      - strategy: content_desc
        value: "Accept order"
      - strategy: text
        value: "ACCEPT"
```

#### C. Selector-push envelope (authoritative for 433-07)

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "selector_push",
  "from": "admin-dashboard",
  "to": "rcm-tab-plus",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "app_package": "com.zomato.partner",
    "app_version": "3.14.2",
    "yaml_canonical": "<YAML bytes as UTF-8 string — see canonicalization note below>",
    "signature_ed25519": "<base64 — detached signature over yaml_canonical>",
    "signed_by_key_id": "admin-v1-2026-04-18",
    "patch_version": 3,
    "supersedes_patch_version": 2
  }
}
```

**Canonicalization:** the signed bytes are the YAML file content AS WRITTEN TO DISK — LF line endings, no BOM, no trailing whitespace. `PatchSignatureVerifier` normalizes to LF + trims trailing whitespace per line before verify. Identical rule applied by Phase 443 signer. (Documented in docs/SELECTORS.md §Signing.)

#### D. SelectorMissEvent envelope (authoritative for 433-05)

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "selector_miss",
  "from": "rcm-tab-plus",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "driver_id": "zomato-partner",
    "screen": "dashboard",
    "element_name": "accept_button_in_row",
    "app_package": "com.zomato.partner",
    "app_version": "3.14.2",
    "screenshot_hash": "sha256:1a2b3c...",
    "last_known_good": {
      "strategy": "content_desc",
      "value": "Accept order",
      "last_matched_at_ms": 1713439000000
    },
    "attempt_chain": [
      {"strategy": "content_desc", "value": "Accept order", "outcome": "no_match"},
      {"strategy": "text", "value": "ACCEPT", "outcome": "no_match"}
    ],
    "dedup_window_ms": 60000
  }
}
```

Note: phase 433 EMITS this shape; the audit log in Phase 435 CONSUMES it. In 433 the event lands in `SelectorEventBus` (in-memory) + `RotatingLog` (on-disk). Relay shipping is Phase 435's job.

## 5. Atomic plan breakdown (8 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.
Plans follow interface-first order: 433-01 defines data model, 433-02 uses it, 433-03/04 orchestrate it, 433-05 observes it, 433-06 generates it, 433-07 pushes it, 433-08 verifies end-to-end.

---

### 433-01-PLAN — YAML schema + SelectorMap data model + parser

**Goal:** A canonical YAML schema, matching Kotlin data model, and a parser that validates structure and throws typed errors.

**Covers:** SELECTOR-01 (DSL shape + file layout)

**Dependencies:** 432 (DriverContext surface — Task 0 cross-checks)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: Given a valid YAML fixture, `SelectorParser.parse(bytes)` returns a `SelectorMap` with the expected number of screens and attempts.
- Test 2: Given a YAML with missing `app_package`, parse throws `SelectorParseException` containing "app_package is required".
- Test 3: Given a YAML with a strategy not in the enum, parse throws `SelectorParseException` with the offending strategy name.
- Test 4: Given a YAML with duplicate element names within a screen, parse throws `SelectorParseException` "duplicate element name".
- Test 5: Given a YAML with an `all_matches: true` attempt, the parsed `SelectorAttempt.allMatches` is true.
- Test 6: Round-trip — serialize a `SelectorMap` back to YAML and re-parse; equal.

**Tasks:**

0. Cross-check Phase 432's `DriverContext` — grep the Phase 432 PLAN and any source. If `findSelector(screen, element) -> MatchResult` exists, proceed. If different, raise an amendment proposal to Phase 432 in the commit message and adapt 433-02's signatures accordingly.

1. Add dependency to `rc-agent-mobile/app/build.gradle.kts`:
   ```kotlin
   implementation("org.snakeyaml:snakeyaml-engine:2.7")   // ~380 KB, MIT, Android-safe
   ```
   Rejected alternatives: Jackson YAML (+Jackson Core = 2 MB cost), kotlinx-serialization-yaml (pre-1.0, API unstable). Noted in OQ-2.

2. Create `SelectorSchema.kt`:
   ```kotlin
   enum class SelectorStrategy { RESOURCE_ID, CONTENT_DESC, TEXT, XPATH }

   data class SelectorAttempt(
       val strategy: SelectorStrategy,
       val value: String,
       val timeoutMs: Long = 2000,
       val allMatches: Boolean = false
   )
   ```

3. Create `SelectorMap.kt`:
   ```kotlin
   data class SelectorMap(
       val appPackage: String,
       val appVersion: String,
       val schemaVersion: Int,
       val generatedAt: String?,      // ISO-8601
       val generatedBy: String?,
       val screens: Map<String, Map<String, List<SelectorAttempt>>>
   ) {
       fun attempts(screen: String, element: String): List<SelectorAttempt>? =
           screens[screen]?.get(element)
   }
   ```

4. Create `SelectorParser.kt`:
   - Uses SnakeYAML-Engine's `Load(LoadSettings.builder().setAllowDuplicateKeys(false).build())`.
   - Walks the resulting `Map<String, Any?>`, builds `SelectorMap`.
   - Validation:
     - `app_package` required, String.
     - `app_version` required, String (no semver enforcement — apps use non-semver versions freely).
     - `schema_version` required, must be 1.
     - For each screen → each element → each attempt: `strategy` required and in enum; `value` required and non-empty.
     - Element name must match regex `^[a-z_][a-z0-9_]*$` (snake_case) — prevents accidental quoting bugs.
     - Duplicate element name within a screen → throw.
   - `SelectorParseException(val line: Int?, val col: Int?, message: String)` extends `RuntimeException`. SnakeYAML-Engine's Mark object provides line/col for mapping nodes.

5. Create test fixtures in `app/src/test/resources/selectors/`:
   - `valid.yaml` (the sample above)
   - `missing_app_package.yaml`
   - `unknown_strategy.yaml`
   - `duplicate_element.yaml`
   - `all_matches.yaml`

6. Write unit tests listed in tdd.behavior above.

7. **Write `docs/SELECTORS.md`** (authoring guide — the spec doc): schema, versioning rules, strategy semantics, capture-mode workflow, signing workflow. ~200 lines. This is the file James will read when authoring selectors for each new Zomato/HyperPure/Blinkit update.

**Acceptance:**
- `./gradlew :app:testDebugUnitTest --tests '*SelectorParserTest'` — all 6 tests pass.
- `SelectorParser.parse(validFixture).screens["login_screen"]!!.size == 3` evaluates true.
- `docs/SELECTORS.md` exists, ≥ 150 lines, covers: schema, versioning, strategies, capture, signing.

**G4 NOT TESTED list:**
- Matcher integration (433-02).
- File-watcher triggered reloads (433-03).
- Hot-reload latency (433-08).

**Commit message:**
```
feat(433-01): selector YAML schema + SelectorMap data model + parser

Adds SelectorStrategy enum (resource_id, content_desc, text, xpath),
SelectorMap data class, and SelectorParser (SnakeYAML-Engine 2.7 backend).
Parser validates required fields, enum values, snake_case element names,
and duplicates; errors include line/col from YAML mark.
docs/SELECTORS.md authored as canonical reference for selector authors.

Covers: SELECTOR-01 (DSL shape)
Not tested: runtime matching (433-02), hot-reload (433-03), remote push (433-07).
```

---

### 433-02-PLAN — Selector matching engine

**Goal:** Given a SelectorMap, a (screen, element) lookup, and an AccessibilityNodeInfo root, return the first matching node with a confidence score, or MISS.

**Covers:** SELECTOR-01 (runtime use) + precondition for SELECTOR-05

**Dependencies:** 433-01, Phase 430 (AccessibilityNodeInfo API from AccessibilityService)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: Node tree contains a node with `viewIdResourceName = "com.zomato.partner:id/btn_login"`; matcher with a `resource_id` attempt for that value returns `Hit(node, RESOURCE_ID, confidence=1.0)`.
- Test 2: Node tree has no matching resource_id but has `contentDescription = "LOGIN"`; attempt chain `[resource_id→mismatch, content_desc→"LOGIN"]`; matcher returns `Hit(node, CONTENT_DESC, confidence=1.0)`.
- Test 3: No attempt matches — matcher returns `Miss`.
- Test 4: `text` strategy with case-insensitive fallback: node has `text = "Accept"`, attempt value `"ACCEPT"` → matches with confidence=0.9 (case-insensitive reduces confidence).
- Test 5: XPath attempt throws `UnsupportedStrategyException` (deferred to post-v50 per risk R-1).
- Test 6: `all_matches=true` returns a list of hits (matcher exposes both `matchFirst(...)` and `matchAll(...)` entry points).
- Test 7: Large tree (simulate 500 nodes) — matcher completes in < 100ms on JVM.

**Tasks:**

1. Create `MatchResult.kt`:
   ```kotlin
   sealed class MatchResult {
       data class Hit(val node: AccessibilityNodeInfoLike, val strategy: SelectorStrategy, val confidence: Double) : MatchResult()
       data object Miss : MatchResult()
   }
   ```
   `AccessibilityNodeInfoLike` is a test-friendly abstraction: an interface with `viewIdResourceName`, `contentDescription`, `text`, `children` — so unit tests can provide fakes without an Android runtime. Adapter wraps the real `AccessibilityNodeInfo` in production.

2. Create `SelectorMatcher.kt`:
   - `fun matchFirst(root: AccessibilityNodeInfoLike, attempts: List<SelectorAttempt>): MatchResult`
   - `fun matchAll(root: AccessibilityNodeInfoLike, attempts: List<SelectorAttempt>): List<MatchResult.Hit>`
   - BFS with max depth 12 (configurable via `SelectorMatcher(maxDepth = 12)`). Real-world Accessibility trees rarely exceed depth 8; 12 is a safety margin.
   - For each attempt in order: walk the tree; first strategy that matches wins for `matchFirst`.
   - Confidence rules:
     - resource_id exact: 1.0
     - content_desc exact: 1.0
     - text exact case-sensitive: 1.0
     - text case-insensitive: 0.9
     - text contains (substring, fallback): 0.7
     - XPath: 1.0 (deferred; throws)
   - Optimization: cache `viewIdResourceName` lookups in a HashMap<String, Node> built on first access per `matchFirst` call (amortizes repeated resource_id lookups across multiple attempts within one call).

3. Wire consumption from `DriverContext`:
   ```kotlin
   // In DriverContext impl (Phase 432 territory but this hooks there)
   override suspend fun findSelector(screen: String, element: String): MatchResult {
       val attempts = selectorResolver.resolve(appPackage, currentAppVersion, screen, element)
           ?: return MatchResult.Miss.also { emitMissEvent(...) }
       val root = accessibilityService.rootNode()
           ?: return MatchResult.Miss
       return selectorMatcher.matchFirst(root, attempts).also {
           if (it is MatchResult.Miss) emitMissEvent(...)
       }
   }
   ```
   Note: `emitMissEvent` is implemented in 433-05. In 433-02 we add a TODO and a stub that logs to RotatingLog.

4. Unit tests per tdd behavior, using an in-memory `AccessibilityNodeInfoLike` tree builder helper.

5. `UnsupportedStrategyException(strategy: SelectorStrategy)` for XPath — ship but document as future work in docs/SELECTORS.md §"Future strategies".

**Acceptance:**
- All 7 unit tests pass.
- `./gradlew :app:testDebugUnitTest --tests '*SelectorMatcher*'` exit 0.
- Manual: plug `SelectorMatcher` into a driver stub from Phase 432; driver can call `findSelector("login_screen", "username_field")` and get a `Hit`.

**G4 NOT TESTED list:**
- Real AccessibilityNodeInfo from a live app (tested in 433-08 on Tab Plus).
- Fallback chain (433-04).
- Miss event emission shape (433-05).

**Commit message:**
```
feat(433-02): selector matching engine (BFS, confidence scoring)

SelectorMatcher.matchFirst / matchAll walk the AccessibilityNodeInfo tree
(max depth 12). Strategy order honored; first match wins. Confidence
scores expose uncertainty to the audit log (resource_id=1.0,
text-case-insensitive=0.9, substring-fallback=0.7). XPath strategy
parsed but throws UnsupportedStrategyException — deferred to post-v50.
AccessibilityNodeInfoLike interface enables JVM-only unit tests.

Covers: SELECTOR-01 runtime
Not tested: live-app match (433-08), fallback chain (433-04), miss events (433-05).
```

---

### 433-03-PLAN — FileObserver-based hot-reload (≤ 10s latency)

**Goal:** Edit `selectors.yaml` on device → new map is live within 10 seconds.

**Covers:** SELECTOR-03

**Dependencies:** 433-01

**Type:** `auto`

**tdd behavior:** (applies to SelectorStore, test via deterministic callback)
- Test 1: `SelectorStore.load()` with no files returns empty catalog and logs INFO.
- Test 2: After `SelectorStore.load()` on a populated dir, `getCatalog().maps` size matches directory contents.
- Test 3: A mock `FileChangedCallback` fired twice in 50ms results in one `reload()` call (300ms debounce).
- Test 4: Reload is atomic: a test that captures the catalog reference before and after verifies the reference swapped; old reference is still usable (no null deref).

**Tasks:**

1. Create `SelectorStore.kt`:
   - Holds `private val catalogRef = AtomicReference<SelectorCatalog>(SelectorCatalog.empty())`.
   - `SelectorCatalog` is a nested map: `Map<app_package, Map<app_version, SelectorMap>>`.
   - `load()`: scans `filesDir/selectors/`, for each `<app>/<version>/selectors.yaml` calls `SelectorParser.parse` and assembles catalog. Parse failures are LOGGED and skipped — do not fail the whole store (one bad file should not brick all drivers). Logs via RotatingLog with target=selector, event=parse_failed.
   - `reload()`: re-runs `load()` into a new catalog, atomically swaps `catalogRef`.
   - `getCatalog(): SelectorCatalog`.
   - `getMap(appPackage, appVersion): SelectorMap?`.
   - `writeAtomic(appPackage, appVersion, yamlBytes)` — writes to `<dir>.tmp` then `File.renameTo(<dir>)` for atomic swap. Backs up previous via `.backup`. Used by 433-07 remote-push.

2. Create `SelectorFileWatcher.kt`:
   - Uses `android.os.FileObserver` (recursive variant — API 29+) on `filesDir/selectors/`.
   - Listens for events: `MODIFY`, `MOVED_TO`, `CREATE`, `DELETE`.
   - Debounces: on any event, cancel any pending reload job, schedule `reload()` in 300ms using the service's coroutine scope. The 300ms absorbs multi-file writes (adb push may produce several events in rapid succession).
   - On reload completion, emits a system event `selector_catalog_reloaded` to RotatingLog with the new catalog's map count.
   - Shuts down cleanly on service stop.

3. Wire into `AgentForegroundService.onCreate` (appending to Phase 429's onCreate):
   ```kotlin
   selectorStore = SelectorStore(filesDir)
   selectorStore.load()  // initial population
   selectorWatcher = SelectorFileWatcher(filesDir, serviceScope, selectorStore)
   selectorWatcher.start()
   ```

4. First-boot bootstrap: if `filesDir/selectors/` is empty, copy contents of APK asset `selectors-sample/` to `filesDir/selectors/`. This ensures a fresh install has at least the seed Zomato v3.14.2 map (placeholder values; Phase 437 replaces them).

5. Latency test (instrumented, in 433-08, preview here):
   - Baseline: modify a file via `adb push`, start stopwatch, poll `GET /health` for a new field `selector_catalog_last_reloaded_at_ms` (added to Phase 429-03's `DeviceState` in this plan). Verify delta < 10s.

6. Unit tests per tdd.behavior using temp directories.

**Acceptance:**
- Unit tests pass.
- Instrumented smoke test (run locally, not on CI): `adb push selectors.yaml filesDir/selectors/zomato-partner/v3.14.2/` → wait 2 seconds → `GET /health` shows `selector_catalog_last_reloaded_at_ms` within the last 2 seconds.
- `adb logcat | grep SelectorFileWatcher` shows `selector_catalog_reloaded map_count=<N>` line.

**Key concurrency risks addressed:**
- AtomicReference swap is always-correct for in-flight readers (they finish with old map, next call picks up new).
- No lock held across await — the reload task reads files + parses in the IO dispatcher, builds the new catalog as a local, then does a single `catalogRef.set(new)` without locking.
- FileObserver-backed callback runs on a binder thread pool; reload work is dispatched to `Dispatchers.IO`.

**G4 NOT TESTED list:**
- Per-version fallback (433-04).
- Miss events (433-05).
- Remote-push-triggered reload (433-07).

**Commit message:**
```
feat(433-03): FileObserver hot-reload ≤ 10s, atomic catalog swap

SelectorStore holds AtomicReference<SelectorCatalog>. SelectorFileWatcher
uses FileObserver recursive variant on filesDir/selectors/ with 300ms
debounce. Parse failures are logged and skipped — one bad file does not
brick other drivers. On first boot, APK asset selectors-sample/ is
copied to filesDir/selectors/. /health endpoint gains
selector_catalog_last_reloaded_at_ms field.

Covers: SELECTOR-03
Not tested: version fallback (433-04), miss events (433-05).
```

---

### 433-04-PLAN — Per-app-version resolution + fallback chain

**Goal:** Given (appPackage, currentAppVersion, screen, element), return the best available attempt list; fall back to the newest-older version when the current version's map is missing.

**Covers:** SELECTOR-02

**Dependencies:** 433-03

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: Catalog has v3.14.2; resolve for v3.14.2 → returns that map's attempts, no WARN.
- Test 2: Catalog has v3.14.1; resolve for v3.14.2 → returns v3.14.1's attempts, logs WARN `selector_map_fallback from=v3.14.2 to=v3.14.1`.
- Test 3: Catalog has v3.14.1 AND v3.13.0; resolve for v3.14.2 → picks v3.14.1 (newest older), not v3.13.0. Tie-breaker: string version ordering via a version comparator that handles Zomato-style "3.14.2" (three dots = major.minor.patch) AND HyperPure-style "23.10.0" and Blinkit "2.0.44" — defer exact comparator to a helper `VersionComparator.compare(a, b)` in this plan's scope.
- Test 4: Catalog has NO maps for `appPackage` → throws `NoSelectorMapException(app=..., requested=..., available=[])`.
- Test 5: Catalog has v3.14.2 but the requested screen/element is missing → returns null (distinct from "no map" — caller's matcher returns Miss and emits event).
- Test 6: App version is null (app not installed) → resolver picks newest available, logs WARN.
- Test 7: Fallback chain caps at 1 step — resolver DOES NOT walk indefinitely older. If v3.14.1's selectors also miss at runtime, that's a miss event, not a deeper chain fallback. Rationale: deeper fallback hides drift and makes bug bisection impossible.

**Tasks:**

1. Create `VersionComparator.kt` — compares dotted version strings by tokenized numeric-or-lex fields. Handles:
   - `3.14.2` vs `3.14.1` → +1 (a > b).
   - `3.14.2` vs `3.15.0` → -1.
   - `3.14.2-beta` vs `3.14.2` → -1 (suffix = older/newer? Lex order — beta < final).
   - Doc-comment: if a target app's versioning is fundamentally different (e.g. date-based), author a dedicated comparator; for v50.0 the three supported apps use dotted numeric.

2. Create `SelectorResolver.kt`:
   ```kotlin
   class SelectorResolver(
       private val store: SelectorStore,
       private val rotatingLog: RotatingLog,
       private val eventBus: SelectorEventBus   // injected here but fully used in 433-05
   ) {
       suspend fun resolve(
           appPackage: String,
           appVersion: String?,
           screen: String,
           element: String
       ): ResolveOutcome { ... }
   }

   sealed class ResolveOutcome {
       data class Hit(val attempts: List<SelectorAttempt>, val mapVersion: String) : ResolveOutcome()
       data object ElementMissing : ResolveOutcome()
       data class Fallback(val attempts: List<SelectorAttempt>, val fromVersion: String, val toVersion: String) : ResolveOutcome()
       data class NoMap(val appPackage: String, val requested: String?, val available: List<String>) : ResolveOutcome()
   }
   ```
   Logic:
   - Fetch catalog via `store.getCatalog()`.
   - `maps = catalog.maps[appPackage]` — if null/empty → `NoMap`.
   - If `appVersion != null` AND `maps[appVersion] != null` → look up screen/element. If present → `Hit`. If absent → `ElementMissing`.
   - Else (version missing in catalog OR appVersion null): select newest strictly-older version via `VersionComparator`. If found → `Fallback`. If none found → `NoMap(available = maps.keys)`.

3. Wire from `DriverContext.findSelector`:
   ```kotlin
   when (val out = selectorResolver.resolve(appPackage, currentAppVersion, screen, element)) {
       is ResolveOutcome.Hit -> matcher.matchFirst(rootNode, out.attempts)
       is ResolveOutcome.Fallback -> {
           rotatingLog.warn("selector", "map_fallback", mapOf("from" to out.fromVersion, "to" to out.toVersion))
           matcher.matchFirst(rootNode, out.attempts)
       }
       is ResolveOutcome.ElementMissing, is ResolveOutcome.NoMap -> {
           eventBus.emit(SelectorMissEvent(..., reason = out::class.simpleName))
           MatchResult.Miss
       }
   }
   ```
   (Note: `eventBus.emit` body is 433-05's work; in 433-04 the call is present but the subscriber set is stubbed.)

4. Hook up `DriverContext.onAppUpdate(oldVersion, newVersion)` (Phase 432-defined):
   - On app update, the resolver automatically picks the new version's map on the next `resolve()` call — no proactive work needed. However, we log an INFO `app_updated old=X new=Y maps_available=[...]` so the audit trail shows it.
   - If the new version has no map, the next `resolve()` returns `Fallback` or `NoMap` — same path as any other missing-version case.

5. Unit tests per tdd.behavior using a fake SelectorStore with controllable catalogs.

**Acceptance:**
- All 7 unit tests pass.
- Manual: with a catalog containing only v3.14.1, call `resolver.resolve("com.zomato.partner", "3.14.2", "login_screen", "username_field")` → returns `Fallback(attempts=[...], from="3.14.2", to="3.14.1")`.
- `logcat | grep selector_map_fallback` shows the WARN line.

**G4 NOT TESTED list:**
- Fallback with a real app update on Tab Plus (instrumented test in 433-08 — harder to simulate without actually updating Zomato).
- Miss events fully (433-05).

**Commit message:**
```
feat(433-04): version-aware selector resolution + single-step fallback

SelectorResolver returns Hit/Fallback/ElementMissing/NoMap. Fallback
picks newest strictly-older version via VersionComparator (dotted
numeric). Fallback caps at 1 step — deeper fallback would hide drift.
onAppUpdate hook logs INFO; next resolve() naturally picks the new
map without proactive work.

Covers: SELECTOR-02
Not tested: real app update on device (433-08), miss events (433-05).
```

---

### 433-05-PLAN — SelectorMissEvent emission

**Goal:** Every selector miss produces a structured event with screenshot hash + last-known-good + app version. Events land in an in-memory bus, on-disk RotatingLog, and (if the comms-link WS is up) ship to the relay as a `selector_miss` envelope.

**Covers:** SELECTOR-05

**Dependencies:** 433-02, 433-04, 429-07 (RotatingLog), 430 (screenshot capability)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: When `emit(event)` is called, the event appears in `eventBus.replay(100)` flow.
- Test 2: Two emits with the same (driver, screen, element, app_version) within 60s produce ONE emitted event (deduped); the second emit bumps a `suppressed_count` counter on the first event.
- Test 3: Two emits across a 61s gap produce two distinct events.
- Test 4: Each emitted event is written to RotatingLog via `RotatingLog.error("selector", "miss", payload)`.
- Test 5: If CommsLinkClient is connected, the event is sent as `type: selector_miss` envelope.
- Test 6: If CommsLinkClient is not connected, emit still succeeds — event lands in bus + log but is not shipped. No retry queue (Phase 435 adds durable shipping).
- Test 7: Last-known-good lookup returns the most recent `(strategy, value, last_matched_at_ms)` for that (app, screen, element) tuple. If never matched, returns null.

**Tasks:**

1. Create `SelectorMissEvent.kt` — @Serializable data class matching envelope §D above.

2. Create `SelectorEventBus.kt`:
   ```kotlin
   class SelectorEventBus(scope: CoroutineScope, bufferSize: Int = 100) {
       private val flow = MutableSharedFlow<SelectorMissEvent>(
           replay = bufferSize,
           extraBufferCapacity = 64,
           onBufferOverflow = BufferOverflow.DROP_OLDEST
       )
       private val dedupWindow = ConcurrentHashMap<String, Long>()  // key → last_emit_ts_ms

       fun emit(ev: SelectorMissEvent) { ... }   // with 60s dedup
       fun subscribe(): SharedFlow<SelectorMissEvent> = flow.asSharedFlow()
   }
   ```
   Dedup key: `"$driverId:$screen:$elementName:$appVersion"`. Dedup TTL: 60 000 ms. Suppressed counter is tracked in a `ConcurrentHashMap<String, AtomicInteger>` and attached to the NEXT non-suppressed emit of the same key.

3. Wire `SelectorMatcher.matchFirst(root, attempts) == Miss` in 433-02's consumer (DriverContext) to emit via `eventBus`.

4. Last-known-good tracker: `LastKnownGood` singleton-ish class (scoped to the service) records every successful match:
   ```kotlin
   class LastKnownGood {
       private val entries = ConcurrentHashMap<String, Entry>()   // key as above
       fun record(appPackage, appVersion, screen, element, strategy, value) { ... }
       fun lookup(...): Entry? { ... }
   }
   ```
   Invoked by SelectorMatcher on every `Hit`. Also loaded from `filesDir/last-known-good.json` at boot (best-effort persistence; deleted if corrupt).

5. Screenshot hash: 430's AccessibilityService provides `GLOBAL_ACTION_TAKE_SCREENSHOT`. In 433-05, `emit(event)` first calls a `screenshotProvider.hashCurrentScreen()` coroutine that takes a screenshot and returns its SHA-256 hash. Screenshot BYTES are NOT kept — only the hash. This gives James a forensic fingerprint without bloating logs with image data. If screenshot capture fails (permission not granted, hardware hiccup), the hash field is `"sha256:unavailable:<reason>"` and the event is still emitted.

6. Subscriber wiring:
   - RotatingLog subscriber: on every event, `log.error("selector", "miss", event.toMap())`.
   - CommsLinkClient subscriber: if `isConnected.get()`, send envelope `{type: "selector_miss", payload: event}`. Errors during send → log WARN, no retry.
   - Both subscribers attach in `AgentForegroundService.onCreate` (appending to the existing boot sequence).

7. Tests per tdd.behavior. Screenshot capture is stubbed via `FakeScreenshotProvider` returning a known hash.

**Acceptance:**
- All 7 unit tests pass.
- Manual on Tab Plus: install a selector map with a deliberately-wrong resource_id for `login_screen.username_field`. Trigger the driver to call `findSelector("login_screen", "username_field")` — verify `adb shell cat filesDir/logs/rc-agent-mobile.log.jsonl | tail -5` shows a line with `event: miss` and a screenshot_hash.
- Comms-link relay log shows a `selector_miss` envelope from `rcm-tab-plus` (verified manually — Phase 435 formalizes this subscriber).

**G4 NOT TESTED list:**
- Durable relay shipping (Phase 435).
- Event viewer in admin dashboard (Phase 443).
- Real-world dedup under 1000+ events/min load (deferred to load test).

**Commit message:**
```
feat(433-05): SelectorMissEvent emission with screenshot hash + last-known-good

SelectorEventBus: in-memory bounded flow with 60s per-key dedup.
Events carry driver, screen, element, app_version, screenshot_hash
(sha256 of current screen — bytes discarded), last_known_good
(from LastKnownGood tracker), and attempt_chain. Subscribers:
RotatingLog (on-disk) + CommsLinkClient (best-effort ship, no retry).
LastKnownGood persisted to filesDir/last-known-good.json.

Covers: SELECTOR-05
Not tested: durable relay shipping (Phase 435), admin viewer (Phase 443).
```

---

### 433-06-PLAN — Debug capture mode (Accessibility tree → YAML stub)

**Goal:** James hits an HTTP endpoint on the device; agent captures the current AccessibilityNodeInfo tree and returns a YAML stub with proposed selectors for every interactable element — ready to copy into `selectors.yaml` with minimal editing.

**Covers:** SELECTOR-06

**Dependencies:** 430 (AccessibilityService rootNode + interactable detection), 433-01 (schema shape)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: Given a fake tree with 3 interactable nodes (click/long-click/text-input), the YAML output contains 3 elements.
- Test 2: Each emitted element proposes up to 3 attempt candidates: resource_id (if non-null), content_desc (if non-null), text (if non-null). Nulls skipped.
- Test 3: Element names are auto-generated as snake_case from content_desc > resource_id leaf > text > "element_<counter>". Duplicates get a numeric suffix (`submit_button`, `submit_button_2`).
- Test 4: Non-interactable nodes (pure layout containers with no click/text handler) are omitted.
- Test 5: The output parses cleanly via `SelectorParser.parse` — output is self-consistent.
- Test 6: The YAML top-matter includes `app_package`, `app_version`, `generated_at`, `generated_by: "debug_capture"`, and a TODO comment block instructing James to review + rename elements.

**Tasks:**

1. Create `NodeTreeToYamlStub.kt`:
   - Walks the node tree BFS, max depth 12.
   - For each node where `isClickable || isLongClickable || isEditable || isCheckable`:
     - Build attempt list from non-null identifiers (resource_id, content_desc, text) in confidence order.
     - Auto-name per Test 3.
   - For nodes with `className in ("android.widget.TextView", "android.widget.ImageView")` that have a non-null resource_id OR content_desc, include them too (display-only but often needed for assertions).
   - Determine current screen name heuristically: the top-most node with resource_id matching `*(fragment|activity|screen)*` gets that name; otherwise `"unknown_screen_<ts>"`. James will rename.
   - Emit YAML via SnakeYAML-Engine Dump API with block style. Wrap in top-matter comment block.

2. Create `SelectorCaptureEndpoint.kt` — Ktor route handler:
   ```
   GET /debug/capture_stub  (requires X-Service-Key header, same key as /logs/tail)
   Query params:
     ?screen=<name>    # optional; overrides heuristic
     ?package=<pkg>    # optional; defaults to currently-foreground app (queried from AccessibilityService)
   Response: text/yaml, 200.
   ```
   Error cases:
   - Accessibility Service not enabled → 503 with JSON `{error: "accessibility_disabled"}`.
   - No foreground app detectable → 400 with `{error: "no_foreground_app"}`.
   - Service key missing/wrong → 401.

3. Add to `LocalHttpServer.kt` (from 429-03) via the `registerRoute` hook.

4. CLI helper for James, `rc-agent-mobile/scripts/capture-selectors.sh`:
   ```bash
   #!/usr/bin/env bash
   # Usage: ./capture-selectors.sh <device_ip> <screen_name> > selectors.yaml
   curl -s -H "X-Service-Key: $RCM_SERVICE_KEY" \
     "http://$1:8090/debug/capture_stub?screen=$2"
   ```

5. Tests per tdd.behavior using a fake node tree. Add one Ktor test for the endpoint (auth OK / auth failed / AX-disabled).

**Acceptance:**
- All 6 unit tests pass.
- Manual: on Tab Plus with Zomato open at the login screen, run `./scripts/capture-selectors.sh <tab_plus_ip> login_screen > /tmp/cap.yaml`. Verify `cat /tmp/cap.yaml | yq '.screens.login_screen | keys'` lists at least `username_field_or_similar`, `password_field`, and `submit_button`. Copy into the Phase 437 working file with minor renaming.

**G4 NOT TESTED list:**
- Multi-screen capture (one request, one screen — the current behavior is deliberate).
- Localized apps with RTL text (Accessibility node text may come in the device's locale — defer to Phase 443 when localization matters).

**Commit message:**
```
feat(433-06): debug capture mode — Accessibility tree → YAML stub

GET /debug/capture_stub on LocalHttpServer returns a parseable YAML
stub of all interactable nodes on the current screen. Element names
auto-generated from content_desc > resource_id leaf > text.
Output round-trips through SelectorParser — no invalid YAML.
Authenticated via X-Service-Key. scripts/capture-selectors.sh
wraps the curl for one-liner capture.

Covers: SELECTOR-06
Not tested: localized (non-English) apps, multi-screen capture.
```

---

### 433-07-PLAN — Remote push endpoint (agent-side) with signature verify + rollback

**Goal:** A `selector_push` envelope arriving via comms-link is verified against the Ed25519 public key, written atomically to disk, hot-reloaded, and ACK'd. Invalid signatures or parse failures roll back and ACK with failure reason. **Admin-side UI to construct the push is Phase 443 and NOT in this plan.**

**Covers:** SELECTOR-04

**Dependencies:** 433-03 (atomic write + hot-reload), 433-05 (parse failure event surface), 429-05 (CommsLinkClient dispatch)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: Envelope with valid signature and parseable YAML → handler writes to `filesDir/selectors/<app>/<version>/selectors.yaml`, calls `SelectorStore.reload()`, returns ACK `{accepted: true, patch_version: N}`. A `.backup` file is created atomically before the swap.
- Test 2: Envelope with tampered YAML (signature doesn't verify) → handler rejects, returns ACK `{accepted: false, reason: "signature_invalid"}`, RotatingLog.error. No file touched.
- Test 3: Envelope with valid signature but YAML fails to parse → handler writes .tmp, fails validation, deletes .tmp, returns ACK `{accepted: false, reason: "parse_error", detail: "<first parse error line>"}`, RotatingLog.error. No reload triggered. Pre-existing selectors.yaml untouched.
- Test 4: Envelope with valid signature + parseable YAML, but at the final file-rename step a simulated IOException → handler restores `.backup`, returns ACK `{accepted: false, reason: "write_failed"}`. Catalog reference unchanged (old map still live).
- Test 5: Two concurrent pushes for the same app — the second push serializes behind the first via a `Mutex` on `SelectorPushHandler.pushLock`. No interleaving.
- Test 6: `supersedes_patch_version` older than current stored patch_version → handler ACKs `{accepted: false, reason: "stale_patch", current_patch_version: N}`. This prevents a replayed old push from overwriting a newer one.
- Test 7: `signed_by_key_id` not in the known-keys list → reject with `{accepted: false, reason: "unknown_signing_key"}`.

**Tasks:**

1. Generate Ed25519 keypair for the admin signer:
   - `openssl genpkey -algorithm ed25519 -out admin-ed25519.pem` (private, NEVER ship to device).
   - `openssl pkey -in admin-ed25519.pem -pubout -out admin-ed25519.pub.pem` (public, ship to device).
   - Store private key in `~/.racingpoint-secrets/admin-ed25519.pem` on James's machine. (OQ-4 handles production key rotation.)
   - Public key goes into `rc-agent-mobile/keystores/signing-pubkey-v1.pem`, loaded via `BuildConfig.SIGNING_PUBKEY_PEM` (read from the file at Gradle build time).
   - Key ID scheme: `admin-v1-<date-created>` (e.g. `admin-v1-2026-04-18`). Keys rotate when a new public key is committed; the old key is kept for ~6 months to accept in-flight pushes during rollover.
   - Supported key IDs are declared in `BuildConfig.TRUSTED_SIGNING_KEY_IDS` = `["admin-v1-2026-04-18"]`.

2. Choose signature backend: **platform's `java.security.Signature` with `"Ed25519"` algorithm** (API 24+).
   - Rejected: libsodium (`com.goterl:lazysodium-android`, +2.5 MB APK + JNI complexity).
   - Rejected: BouncyCastle (BC is powerful but adds ~4 MB APK; Ed25519 support in BC requires an extra provider registration which has been a source of bugs).
   - Platform's built-in Ed25519 is available on API 24+ (minSdk is 29 from Phase 429-01) — zero extra dependencies.
   - Flagged in OQ-1 for user confirmation — if Uday prefers libsodium for audit reasons, we can swap.

3. Create `PatchSignatureVerifier.kt`:
   ```kotlin
   class PatchSignatureVerifier(private val trustedKeys: Map<String, PublicKey>) {
       fun verify(canonicalYamlUtf8: ByteArray, signatureBase64: String, keyId: String): Boolean { ... }
   }
   ```
   Canonicalization: normalize line endings to LF, strip trailing whitespace per line, no BOM, UTF-8. Applied identically by the signer (documented in docs/SELECTORS.md §Signing).

4. Create `SelectorPushHandler.kt`:
   - `suspend fun handle(envelope: Envelope<SelectorPushPayload>): PushAck`
   - Serializes concurrent pushes via `Mutex`.
   - Steps in order:
     1. Verify `signed_by_key_id` ∈ trusted keys → else reject `unknown_signing_key`.
     2. Verify signature on canonicalized bytes → else reject `signature_invalid`.
     3. Check `supersedes_patch_version` ≥ current stored → else reject `stale_patch`.
     4. Read existing selectors.yaml into `.backup`.
     5. Write new bytes to `<target>.tmp`.
     6. Run `SelectorParser.parse` on the tmp file → if fail, delete tmp, reject `parse_error`, restore no-op.
     7. `File.renameTo` tmp → target (atomic on same filesystem).
     8. Call `selectorStore.reload()`.
     9. Record new patch_version in `filesDir/selectors/<app>/<version>/.patch_version`.
     10. Return `PushAck(accepted=true, patch_version=N)`.
   - On ANY IOException during steps 4-9: restore .backup → reload → return `write_failed`.

5. Wire into `CommsLinkClient.onMessage` (Phase 429-05):
   ```kotlin
   "selector_push" -> {
       val ack = selectorPushHandler.handle(envelope)
       send(ackEnvelope(ack))
   }
   ```

6. Add a `scripts/push-selectors.sh` CLI (James-side, for integration testing until Phase 443):
   ```bash
   #!/usr/bin/env bash
   # Usage: ./push-selectors.sh <device_id> <selectors.yaml> <patch_version>
   # Signs locally with ~/.racingpoint-secrets/admin-ed25519.pem and sends
   # via comms-link.
   # Implementation: Python helper that uses PyNaCl for signing and
   # curl to POST to http://localhost:8766/relay/message
   ```
   (Bash + python3 + PyNaCl — James's workstation already has both. Script lives in rc-agent-mobile/scripts/.)

7. Extend `docs/PROTOCOL.md` with `selector_push` + `selector_push_ack` envelope schemas. Mirror to `comms-link/shared/agent-protocol-v1.md` (DEPLOY PARITY).

8. Tests per tdd.behavior. Key test helper: a `TestSigner` that signs with a known private key; pair with a `PatchSignatureVerifier` bootstrapped with the matching public key.

**Acceptance:**
- All 7 unit tests pass.
- Manual: from James's workstation, `./push-selectors.sh rcm-tab-plus /tmp/test-selectors.yaml 1` → within 10 seconds the device's `/health` shows `selector_catalog_last_reloaded_at_ms` fresh; `adb shell cat filesDir/selectors/zomato-partner/v3.14.2/selectors.yaml` matches the pushed file.
- Manual tamper test: modify one byte of the YAML, re-run the push → device logs `register_rejected_signature` error; no file change.

**Key risks addressed:**
- **Concurrency:** Mutex serializes pushes; file ops + atomic rename ensure no partial writes visible to readers.
- **Rollback correctness:** `.backup` is created BEFORE tmp write; any failure path after tmp-exists restores from .backup — guaranteeing reader-visible state never diverges.
- **Replay protection:** `supersedes_patch_version` prevents a replayed old envelope from downgrading a device.
- **Key rotation:** trusted key IDs are a list in BuildConfig; ship new pubkeys alongside the old ones for the overlap window (Phase 443 coordinates this with the admin UI).

**G4 NOT TESTED list:**
- Admin UI constructing + signing + sending the envelope (Phase 443).
- Key rotation flow under operational load (deferred to Phase 443).
- Load test of 100 pushes/min (not a real-world scenario — pushes are manual ~1/week).

**Commit message:**
```
feat(433-07): remote selector push with Ed25519 signature + rollback

SelectorPushHandler: verify envelope.signature via platform java.security
Ed25519, write .tmp, parse, atomic renameTo, reload, ACK. On signature
invalid / parse fail / IO error: restore .backup, ACK with reason.
Replay protection via supersedes_patch_version. Concurrent pushes
serialize via Mutex. Trusted public key list in BuildConfig.
docs/PROTOCOL.md amended with selector_push + selector_push_ack shapes;
mirrored to comms-link/shared/agent-protocol-v1.md.

Covers: SELECTOR-04 (agent-side)
Not tested: admin-side UI (Phase 443), key rotation operational load.
```

---

### 433-08-PLAN — Unit tests + Tab Plus integration test + hot-reload stopwatch

**Goal:** All unit tests green; on Tab Plus, demonstrate all four Phase 5 success criteria with stopwatch evidence.

**Covers:** all of SELECTOR-01..06 (verification, not net-new)

**Dependencies:** 433-01 through 433-07

**Type:** `checkpoint:human-verify` (physical Tab Plus)

**Preconditions:**
- Phase 432 driver framework operational; at least a `NoOpDriver` exists for Zomato package that calls `findSelector("login_screen", "submit_button")` every 20 seconds.
- Tab Plus has `filesDir/selectors/zomato-partner/v3.14.2/selectors.yaml` (seeded from APK assets).
- comms-link relay up (James .27).
- `~/.racingpoint-secrets/admin-ed25519.pem` present on James's workstation.

**Drill script:**

1. **SC-1 (hot-reload ≤ 10s).**
   - On Tab Plus: `adb pull filesDir/selectors/zomato-partner/v3.14.2/selectors.yaml /tmp/current.yaml`.
   - Edit a benign field (e.g. change a timeout_ms from 5000 to 4999).
   - Start stopwatch. `adb push /tmp/current.yaml <same location>`.
   - Poll `GET http://<tab_plus_ip>:8090/health` every 500ms; record the timestamp at which `selector_catalog_last_reloaded_at_ms` updates past the push time.
   - Stop stopwatch. **Target: < 10 000 ms.**

2. **SC-2 (version fallback).**
   - Add a map at `filesDir/selectors/zomato-partner/v9.9.9/selectors.yaml`. Do NOT add a v3.14.2 map (rename the existing v3.14.2 directory to v9.9.9 temporarily).
   - Force the driver to call `findSelector("login_screen", "submit_button")` (e.g. via a debug endpoint that triggers a NoOpDriver run).
   - Verify `adb logcat | grep selector_map_fallback` shows `from=<app_current_version> to=9.9.9` (or the reverse if current is older).
   - Verify the matcher still returned a Hit (fallback map worked).

3. **SC-3 (miss event shape).**
   - Edit `v3.14.2/selectors.yaml` and change the `submit_button` resource_id to a deliberately invalid one.
   - Trigger the driver run.
   - Check `adb shell cat filesDir/logs/rc-agent-mobile.log.jsonl | grep '"event":"miss"' | tail -1 | jq .`.
   - **Assert:** the event JSON contains `driver_id`, `screen`, `element_name`, `app_package`, `app_version`, `screenshot_hash` (non-empty), `last_known_good`, `attempt_chain`. **Non-negotiable: every field present.**

4. **SC-4 (debug capture).**
   - Open Zomato on the login screen manually.
   - Run `./scripts/capture-selectors.sh <tab_plus_ip> login_screen > /tmp/captured.yaml`.
   - Verify `yq '.screens.login_screen | keys' /tmp/captured.yaml` lists at least two elements (likely the username and password fields).
   - Verify `cat /tmp/captured.yaml | SelectorParser` round-trip parses (via a small JVM test harness OR `./gradlew :app:runCaptureValidator /tmp/captured.yaml`).

5. **SC-bonus (remote push + rollback).**
   - `./scripts/push-selectors.sh rcm-tab-plus /tmp/captured.yaml 2` — verify ACK `accepted: true`.
   - Tamper (`sed -i 's/resource_id/resouce_id/' /tmp/captured.yaml`) and push with patch_version 3. Verify ACK `accepted: false, reason: "parse_error"`; on-device `selectors.yaml` unchanged.

**Artifacts to save in SUMMARY.md:**
- Stopwatch measurement for SC-1.
- JSON dump of the SC-3 miss event.
- Full output of capture-selectors.sh run.
- Push ACK responses for both successful and rollback-triggering cases.
- Copy of the final selectors.yaml on device (post-drill).

**Checkpoint (human-verify):**
James runs the 5-step drill on Tab Plus, reports numeric pass/fail with the artifacts above. If any step fails, do NOT mark Phase 433 complete — create a gap-closure plan per CLAUDE.md backlog gate.

**Resume signal:** James reports SC-1..SC-4 + SC-bonus all pass with artifacts, or describes failures.

**Commit message:**
```
test(433-08): Phase 433 E2E drill — hot-reload + fallback + miss + capture + push

Verified on Tab Plus (TB-351FU):
- Hot-reload: <X>ms from adb push to /health freshness (target <10000ms)
- Version fallback: v3.14.2 → v<older> with WARN in logs
- Miss event: all fields populated, screenshot hash present
- Debug capture: N elements captured, round-trips through parser
- Remote push + rollback: signed push accepted; tampered push rejected

Artifacts in .planning/phases/433-selector-dsl-hot-reload/SUMMARY.md.

Covers: Phase 5 acceptance gate (SELECTOR-01..06)
```

---

## 6. Risks and pitfalls (selector-specific)

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **XPath on AccessibilityNodeInfo is O(nodes × predicates)** and has no native Android API | Ship schema support only; matcher throws `UnsupportedStrategyException`. Re-evaluate post-v50 based on real-world selector author demand. |
| R-2 | **FileObserver misses events on some OEMs** (Samsung One UI has documented quirks) | Fallback: periodic reconciliation scan every 60s in SelectorFileWatcher (compare file mtimes vs cached). If FileObserver is silent but mtimes diverged, reload. Noted as Phase 443 bonus — not blocking for 433. |
| R-3 | **Hot-reload race: driver reads map A during action, watcher swaps to map B mid-action** | AtomicReference snapshot at action start (DriverContext.findSelector reads `store.getCatalog()` once per call). No mid-action swap possible. |
| R-4 | **YAML parser CVEs** (SnakeYAML has a history of deserialization gadgets) | SnakeYAML-Engine (NOT SnakeYAML) is the safer modern fork — no unchecked deserialization, no arbitrary-class-loading. We do NOT use type tags — schema is map-of-map-of-map of primitives. Review CVEs at 433-01 kickoff. |
| R-5 | **Screenshot capture may fail if foreground app has FLAG_SECURE** (some banking apps) | Emit event with `screenshot_hash: "sha256:unavailable:flag_secure"`. Covered by 433-05 tests. |
| R-6 | **Ed25519 signature library choice — platform, BouncyCastle, or libsodium?** (OQ-1) | Default: platform java.security. Flagged for user confirmation. Swappable via a single `PatchSignatureVerifier` impl swap. |
| R-7 | **Dedup window too short (60s) → noise floods admin** | Tunable via `BuildConfig.SELECTOR_MISS_DEDUP_MS`. Default 60s based on CLAUDE.md ErrorSpike dedup analogue. 433-05 exposes the field. |
| R-8 | **Dedup key stability** — include app_version so app updates don't silently suppress fresh misses | Dedup key = `"$driver:$screen:$element:$appVersion"`. Upgrading app version breaks the key → fresh event. Intentional per CLAUDE.md ErrorSpike lesson. |
| R-9 | **File-watcher leak after service destroy** — orphan FileObservers leak handles | `SelectorFileWatcher.stop()` in service onDestroy releases via `FileObserver.stopWatching()`. Covered by 433-03 smoke test. |
| R-10 | **Selector-push timestamp check** — network latency could cause `supersedes_patch_version` ordering issues | `patch_version` is admin-authoritative — admin increments monotonically. Device-side check is `supersedes >= current`. Two admins pushing simultaneously is a Phase 443 problem, not 433's. |
| R-11 | **Capture mode on a screen with 200+ nodes produces an enormous YAML stub** | 433-06 filters to interactable nodes + 2 display-only classes. Typical screen: 5-20 elements emitted. Screens with more are unusual (app renders 200 list rows). Acceptable. |
| R-12 | **Key compromise** — if the admin private key leaks, signed bad selectors can brick drivers | Key rotation via BuildConfig list of trusted keys. Old key kept for overlap; new APK release removes it. Rollback = manually adb push a known-good selectors.yaml AND remove the compromised key from the trusted list (requires APK rebuild — documented as CLAUDE.md "have a rollback plan" compliance). |

## 7. Test plan

### Unit tests (JVM, fast, on every build)

- `SelectorParserTest` (433-01) — 6 tests
- `SelectorMatcherTest` (433-02) — 7 tests
- `SelectorStoreTest` (433-03) — 4 tests
- `SelectorFileWatcherDebounceTest` (433-03) — test the 300ms debounce logic in isolation
- `SelectorResolverTest` (433-04) — 7 tests
- `VersionComparatorTest` (433-04) — ~10 edge cases
- `SelectorEventBusTest` (433-05) — 7 tests (dedup, subscribe, overflow)
- `LastKnownGoodTest` (433-05) — 3 tests (record/lookup/persist)
- `NodeTreeToYamlStubTest` (433-06) — 6 tests
- `SelectorPushHandlerTest` (433-07) — 7 tests
- `PatchSignatureVerifierTest` (433-07) — 4 tests (valid, tampered, wrong key, unknown key id)

Total: **~60 unit tests**. All run via `./gradlew :app:testDebugUnitTest`. Gradle fails build on any test failure.

### Instrumented tests (optional, skip on CI, run before release)

- `InstrumentedFileObserverTest` — writes a file via `File.writeText`, waits up to 2s, asserts `SelectorStore.reload` was invoked.
- `InstrumentedSelectorResolveTest` — seeds `filesDir/selectors/`, triggers a `findSelector` call, asserts the right path was exercised.

### Physical device tests (human-verify)

- **433-08 drill** — all 5 success criteria on Tab Plus. See 433-08 plan for script.

### Nyquist gate

Before 433-08 closes, run `gsd-nyquist-auditor` with focus on:
- `SelectorResolver.resolve` — branch coverage (Hit, Fallback, ElementMissing, NoMap).
- `SelectorPushHandler.handle` — all 7 failure modes each covered.
- `SelectorMatcher.matchFirst` — strategy order, confidence tiers, miss path.
- `PatchSignatureVerifier.verify` — valid, tampered bytes, tampered signature, unknown key.

### MMA gate

Run before merge (dual reasoning modes per CLAUDE.md):
- **Abstract (non-thinking models):** ask whether the signature-verify → write → reload chain has a TOCTOU or rollback gap.
- **Trace-level (thinking models):** ask whether the AtomicReference swap in SelectorStore can lose events under contention; whether the SelectorEventBus DROP_OLDEST overflow can mask a selector storm.

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** selector resolution is business-critical (every UI action). See §7.
- **MMA audit (required):** signature verification, atomic swap, rollback — cross-trust-boundary logic. Dual reasoning modes.
- **integration-checker (required):** 433 is consumed by 432 (DriverContext) and feeds 435 (audit log). Check at milestone ship.
- **codebase-mapper:** skip (module already mapped; only additions within the module).
- **SEC gate:** `node comms-link/test/security-check.js` must pass AFTER 433-07 adds the `selector_push` envelope to comms-link/shared/agent-protocol-v1.md (static assertions cover new envelope types).
- **Deploy Manifest Protocol (DMP):** captured in frontmatter `deploy:`. Executor ticks each item; verifier confirms.
- **Backlog gate:** 433 must be DEPLOYED-VERIFIED (APK on both devices + 433-08 drill passed) before 437 (Zomato driver) can ship — 437 consumes actual selectors authored via 433's capture mode.

## 9. Open questions the planner cannot decide

Listed in execution-blocking order.

**OQ-1 — Ed25519 signature backend (BLOCKS 433-07).**
**Recommendation:** platform `java.security.Signature` with `"Ed25519"` algorithm (API 24+, available on minSdk 29). Zero extra dependency, zero APK size cost. If Uday prefers audit-reviewed libsodium (`com.goterl:lazysodium-android`, +2.5 MB APK + JNI), swap is a ~20-line change in `PatchSignatureVerifier`. Decide before 433-07 kickoff. **Default: platform.**

**OQ-2 — YAML library (BLOCKS 433-01).**
**Recommendation:** SnakeYAML-Engine 2.7 (~380 KB, MIT, active maintenance, no unchecked deserialization). Alternatives:
- Jackson YAML — +2 MB, richer type system we don't need.
- kotlinx-serialization-yaml — pre-1.0, API unstable, missing Mark API for line/col errors.
- Hand-rolled parser — ~300 lines, maintenance burden, poor error messages.
Decide before 433-01 kickoff. **Default: SnakeYAML-Engine.**

**OQ-3 — Last-known-good persistence format.**
Option A (recommended): JSON file at `filesDir/last-known-good.json` with a rolling cap of 500 entries. Option B: SQLite. A is simpler and 500 entries × ~200 bytes = 100 KB, well within reason. **Default: A.**

**OQ-4 — Signing key storage on James's workstation.**
**Recommendation:** `~/.racingpoint-secrets/admin-ed25519.pem` (mode 0600), added to James's `.gitignore` globally. Pair the private key fingerprint with a README in the repo listing the matching public key ID and creation date. Phase 443 graduates this to a proper secret-management system. **Default: file on disk, 0600, gitignored.**

**OQ-5 — SelectorMiss dedup window duration.**
**Recommendation:** 60s (matches CLAUDE.md ErrorSpike convention). Tunable via `BuildConfig.SELECTOR_MISS_DEDUP_MS`. Revisit if Phase 435 audit log shows floods.

**OQ-6 — Capture mode — should it auto-open Accessibility Settings if the service is disabled?**
Phase 430's first-run UX covers enabling Accessibility. 433-06 returns 503 if disabled. Opening Settings from a debug endpoint is unusual (debug-mode implies a power user). **Recommendation:** return 503 only; James knows what to do. Do NOT auto-open.

**OQ-7 — Selector map size cap per file.**
A pathological YAML could be arbitrarily large (DoS on debug capture). **Recommendation:** 256 KB per file, enforced by SelectorParser (reject with `SelectorParseException("file too large")`). Real-world selectors.yaml for a complex app: ~20 KB.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` (SELECTOR-01..06)
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Prior phase plan patterns:** `.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md`
- **Driver framework (consumer):** `.planning/phases/432-driver-framework-capability-registry/PLAN.md`
- **Audit log (next consumer of SelectorMissEvent):** `.planning/phases/435-humanize-layer-audit-log/PLAN.md`
- **Remote push UI (deferred):** `.planning/phases/443-selector-map-remote-push-ui/PLAN.md`
- **Relay protocol baseline:** `comms-link/docs/PROTOCOL.md`

## 11. Output (at phase close)

At the end of Plan 433-08 (E2E drill pass), create `.planning/phases/433-selector-dsl-hot-reload/SUMMARY.md` capturing:
- Commits implementing each plan (433-01 through 433-08)
- Stopwatch data for hot-reload SC-1
- JSON dump of a real SelectorMissEvent from SC-3
- Output sample from capture-selectors.sh (SC-4)
- Push ACKs (accepted + rejected) from SC-bonus
- Open questions resolved during execution (update §9 state)
- Deploy manifest checklist ticked
- Handoff to Phase 434 (credentials), Phase 435 (audit log — CONSUMES SelectorMissEvent), Phase 437 (Zomato driver — first real consumer of selectors)

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 5 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
