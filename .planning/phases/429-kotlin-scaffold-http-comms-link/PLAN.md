---
phase: 429-kotlin-scaffold-http-comms-link
phase_number: 429
milestone: v50.0 rc-agent-mobile
name: "Kotlin Scaffold + HTTP Server + Comms-link Registration"
status: ready-to-execute
goal: >
  Kotlin Android agent installs on Tab Plus (TB-351FU) + Samsung Galaxy M07, runs a
  Foreground Service with persistent notification, exposes local HTTP :8090 with /health,
  /build_id, /heartbeat, /capability, registers with the comms-link relay on startup,
  sends a 30s heartbeat with exponential-backoff reconnect, auto-starts on device
  reboot via BOOT_COMPLETED, writes structured rotating logs (50 MB cap), and negotiates
  a protocol_version so forward-incompatible messages are rejected gracefully.
requirements: [AGENT-01, AGENT-02, AGENT-03, AGENT-04, AGENT-05, AGENT-06, AGENT-07, AGENT-08]
depends_on: []
wave: 1
plan_count: 8
plans:
  - 429-01-PLAN: Repo decision + Gradle scaffold + module structure
  - 429-02-PLAN: Foreground Service + persistent notification + lifecycle
  - 429-03-PLAN: HTTP server on :8090 with /health + /build_id + /capability
  - 429-04-PLAN: Protocol spec doc (shared JSON contract with Rust rc-agent)
  - 429-05-PLAN: Comms-link registration client + heartbeat loop + backoff
  - 429-06-PLAN: BOOT_COMPLETED receiver + reboot auto-start
  - 429-07-PLAN: Rotating structured log file (50 MB cap, JSONL)
  - 429-08-PLAN: Phase 429 E2E drill (both devices register + heartbeat 5min)
autonomous: false # Plans 429-01 and 429-08 contain human-verify checkpoints (physical device).
files_modified:
  - rc-agent-mobile/                                # new module (see 429-01 for location decision)
  - rc-agent-mobile/settings.gradle.kts
  - rc-agent-mobile/build.gradle.kts
  - rc-agent-mobile/app/build.gradle.kts
  - rc-agent-mobile/app/src/main/AndroidManifest.xml
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/RcAgentApp.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/http/LocalHttpServer.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/CommsLinkClient.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/HeartbeatScheduler.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/protocol/Protocol.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/boot/BootCompletedReceiver.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/log/RotatingLog.kt
  - rc-agent-mobile/app/src/test/kotlin/...         # unit tests
  - rc-agent-mobile/docs/PROTOCOL.md                # shared JSON contract with Rust rc-agent
  - comms-link/shared/agent-protocol-v1.md          # copy for relay-side reference
  - .planning/phases/429-kotlin-scaffold-http-comms-link/SUMMARY.md   # filled at end

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: >
    Android APK installed via ADB on Tab Plus + M07.
    Device firewall rule allowing inbound TCP 8090 from reception LAN subnet.
    comms-link relay (James .27:8765 and cloud Bono VPS:8765) must accept
    two additional WS client identities ("rcm-tab-plus", "rcm-m07") with PSK.
  data_files: >
    rc-agent-mobile/app/src/main/assets/drivers.json  (empty array for phase 429;
    Phase 432 populates.  File must exist so DriverRegistry code in later phases
    does not 404 at boot.)
  bat_file: none
  cloud_parity:
    - comms-link James relay (192.168.31.27:8765) must accept Android clients.
    - comms-link cloud Bono VPS relay (100.70.177.44:8765) must also accept Android clients (DEPLOY PARITY rule).
  targets:
    - tab_plus   # Lenovo TB-351FU (Android 14)
    - m07        # Samsung Galaxy M07 (Android 14)
    - james_27   # comms-link relay config: new client identities + PSK parity
    - bono_vps   # comms-link cloud relay: same
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  rollback:
    - "Keep previous APK on device: /sdcard/Download/rc-agent-mobile-prev.apk"
    - "Rollback command: adb install -r /sdcard/Download/rc-agent-mobile-prev.apk"
    - "Staff-visible: uninstall current APK via Android Settings, reinstall prev APK via Files app"

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip           # No user-facing UI in phase 429 (persistent notification only, boilerplate Android behavior)
  ui_auditor: skip              # Same reason.
  nyquist_auditor: required     # Registration, heartbeat, and version negotiation are business logic.
  mma_audit: required           # Cross-system bridge: Kotlin agent ↔ Node.js relay (comms-link) ↔ Rust server. Dual reasoning modes REQUIRED per CLAUDE.md.
  integration_checker: required # Multi-binary, multi-language integration — must run before milestone ship.
  codebase_mapper: required     # rc-agent-mobile is a NEW top-level module in the monorepo.  Must refresh .planning/codebase/ to include it.

risks_summary:
  - "Android Doze mode can delay heartbeat and kill network calls — mitigated by Foreground Service + WakefulBroadcastReceiver."
  - "Samsung M07 OEM skin (One UI) aggressively kills background apps — mitigated by Foreground Service + user disabling battery optimization in Phase 431 first-run UX."
  - "Android 12+ Foreground Service launch restrictions require dataSync + connectedDevice types declared in manifest."
  - "comms-link PSK distribution to Android devices is NOT solved in 429 — device starts with a placeholder PSK; Phase 431 first-run UX captures the real PSK from QR or manual entry."
  - "BOOT_COMPLETED is throttled by the OS — agent may take 30-60s to register after boot."
---

# Phase 429 — Kotlin Scaffold + HTTP Server + Comms-link Registration

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 429 |
| Name | Kotlin Scaffold + HTTP Server + Comms-link Registration |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | AGENT-01, AGENT-02, AGENT-03, AGENT-04, AGENT-05, AGENT-06, AGENT-07, AGENT-08 |
| Dependencies | None (first phase) |
| Wave | 1 |
| Status | Ready to execute |
| Autonomous | No — plans 429-01 and 429-08 have human-verify checkpoints (physical devices) |
| Ship test | Both devices show up in `/fleet/health` within 30s of boot; killing agent triggers Foreground Service auto-restart within 10s; reboot → re-register without human action; v2 messages gracefully rejected |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 1)

1. **Visibility:** Both devices show up in `/fleet/health` within 30s of device boot.
2. **Foreground Service resilience:** Killing the agent process triggers Foreground Service auto-restart within 10s.
3. **Reboot survival:** Device reboot → agent re-registers without human action.
4. **Protocol forward-compat:** Protocol-version negotiation rejects v2 messages gracefully on v1 agent.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)
- T-1: Agent APK can be installed on both Tab Plus + M07 without Play Store (AGENT-01).
- T-2: After install, the notification shade shows a persistent notification titled "RC Agent Mobile — connected" within 30s (AGENT-05).
- T-3: On James's dashboard `GET /fleet/health`, both devices appear with `ws_connected: true`, `http_reachable: true`, `version: "<build_id>"`, `last_seen < 60s` (AGENT-03).
- T-4: If you force-stop the agent via `adb shell am force-stop`, within 10s the persistent notification re-appears and the device re-registers (AGENT-05).
- T-5: After physical reboot (power button hold → restart), within 90s the device re-registers automatically (AGENT-06).
- T-6: The comms-link relay log shows a heartbeat entry for each device every 30s ± 5s (AGENT-04).
- T-7: Relay sends a mock message with `v=2` — agent logs a WARN "unsupported protocol version" and continues operating (AGENT-08).
- T-8: On the device, `adb shell ls /sdcard/Android/data/in.racingpoint.rcagentmobile/files/logs/` shows a rotating JSONL log file, size under 50 MB (AGENT-07).
- T-9: `curl http://<device_lan_ip>:8090/health` returns `{ok: true, build_id: "...", protocol_version: 1, device_id: "..."}` from a machine on the same LAN (AGENT-02).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `rc-agent-mobile/settings.gradle.kts` | Gradle module registration | 10 | `include(":app")` |
| `rc-agent-mobile/app/build.gradle.kts` | App build config | 50 | `minSdk = 29`, `targetSdk = 34`, Ktor server + client deps |
| `rc-agent-mobile/app/src/main/AndroidManifest.xml` | Android permissions + service + receiver | 40 | `FOREGROUND_SERVICE`, `POST_NOTIFICATIONS`, `RECEIVE_BOOT_COMPLETED`, `INTERNET`, `AgentForegroundService`, `BootCompletedReceiver` |
| `.../service/AgentForegroundService.kt` | Foreground Service main entry | 80 | Notification builder, coroutine scope, onStartCommand → START_STICKY |
| `.../http/LocalHttpServer.kt` | Embedded HTTP server on :8090 | 60 | Ktor `embeddedServer`, routes for /health /build_id /capability /heartbeat |
| `.../comms/CommsLinkClient.kt` | WebSocket client + registration | 120 | OkHttp WS client, exponential backoff, JSON envelope per PROTOCOL.md |
| `.../comms/HeartbeatScheduler.kt` | Periodic heartbeat (30s) | 40 | tickerFlow, coroutine, structured heartbeat payload |
| `.../protocol/Protocol.kt` | Typed JSON envelope + messages | 80 | `@Serializable` data classes matching docs/PROTOCOL.md |
| `.../boot/BootCompletedReceiver.kt` | Auto-start on reboot | 30 | Action BOOT_COMPLETED → startForegroundService |
| `.../log/RotatingLog.kt` | Rotating JSONL logger | 60 | 50 MB cap, N rolled files, JSON lines |
| `rc-agent-mobile/docs/PROTOCOL.md` | Shared JSON contract | 200 | Envelope, message types, version negotiation |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| AgentForegroundService.onCreate | LocalHttpServer.start | Kotlin call | grep `LocalHttpServer.start` in `AgentForegroundService.kt` |
| AgentForegroundService.onCreate | CommsLinkClient.connect | Kotlin call | grep `CommsLinkClient.connect` in `AgentForegroundService.kt` |
| CommsLinkClient.onOpen | HeartbeatScheduler.start(30s) | Kotlin call | grep `HeartbeatScheduler.start` in `CommsLinkClient.kt` |
| BootCompletedReceiver.onReceive | AgentForegroundService (startForegroundService) | Intent | grep `startForegroundService` in `BootCompletedReceiver.kt` |
| CommsLinkClient.onMessage(v!=1) | Log WARN + ignore | match branch | grep `unsupported protocol version` in `CommsLinkClient.kt` |
| CommsLinkClient (everywhere) | RotatingLog.write | Kotlin call | grep `RotatingLog.write` in `CommsLinkClient.kt` |
| /fleet/health on server | CommsLinkClient registration payload | comms-link relay forwards to racecontrol server | Relay must know how to expose Android client identities in the fleet roster (see Open Question OQ-3) |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./comms-link/CLAUDE.md
@./comms-link/docs/PROTOCOL.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md  # v50.0 Planning Milestone section at top
@./crates/rc-common/src/protocol.rs  # reference AgentMessage enum — Kotlin must match JSON tags (Register, Heartbeat)
@./crates/rc-agent/src/main.rs  # reference registration + WS reconnect pattern (lines ~400-900)
@./crates/rc-agent/src/ws_handler.rs  # reference CoreMessage envelope + command_id dedupe
@./crates/rc-agent/src/remote_ops.rs  # reference HTTP server structure (auth middleware, endpoint shape)

### Interfaces executors will need

The Kotlin agent must produce JSON messages that the existing Rust server can parse OR we define a new v1 schema distinct from the Rust agent's enum. We are choosing the **latter — distinct v1 schema scoped to Android agents** — because:

1. The Rust `AgentMessage` enum (`crates/rc-common/src/protocol.rs`) carries pod-specific payloads (PodInfo, TelemetryFrame, LapData, AcStatus) that have no meaning for a reception tablet.
2. Forcing the Android agent into the same schema would pollute shared types with Android-only variants.
3. The comms-link relay is the correct boundary — it forwards typed envelopes; it does not care about payload shapes.

Authoritative shared schema document: `rc-agent-mobile/docs/PROTOCOL.md` (created in plan 429-04).

Key JSON envelope (mirrors comms-link's existing `v:1` envelope for stylistic consistency, but with a distinct `from` namespace):

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "register",
  "from": "rcm-tab-plus",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "device_id": "rcm-tab-plus",
    "device_model": "Lenovo TB-351FU",
    "android_version": "14",
    "agent_version": "0.1.0",
    "build_id": "abc1234",
    "capabilities": [],
    "supported_device_types": ["tablet"]
  }
}
```

## 5. Atomic plan breakdown (8 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

---

### 429-01-PLAN — Repo decision + Gradle scaffold + module structure

**Goal:** A minimal buildable Kotlin Android app that installs via ADB on both devices and launches an empty main Activity.

**Covers:** AGENT-01

**Dependencies:** none

**Type:** `checkpoint:human-verify` at end (physical install on both devices)

#### Decision: where does `rc-agent-mobile/` live?

**Recommendation: new top-level directory in the `racecontrol` monorepo** (`C:\Users\bono\racingpoint\racecontrol\rc-agent-mobile\`).

Rationale:
- **Shared protocol cadence.** The Kotlin agent's JSON protocol is a sibling of `crates/rc-common/src/protocol.rs`. Keeping them in one repo means a single git diff when the protocol evolves.
- **Pattern precedent.** The monorepo already hosts Rust (`crates/`), Next.js frontends (`kiosk/`, `apps/web/`, etc.), and shell tooling (`scripts/`). A Kotlin module at the top level is consistent.
- **Deploy hygiene.** Existing `.planning/`, `LOGBOOK.md`, `CLAUDE.md`, and MMA audit machinery work out of the box.
- **v50.0 is unambiguously a RacingPoint internal tool.** It is not open-source, not a public library, and not independently versioned.

Alternative considered and rejected: sibling repo `~/racingpoint/rc-agent-mobile/`. Rejected because:
- PROTOCOL.md would drift between repos (the exact failure mode called out in CLAUDE.md's "Cross-Process Updates — RECURSIVE cascade" rule).
- Two repos means two CI pipelines, two `LOGBOOK.md`s, two sets of hooks.
- The `comms-link/` sibling repo pattern exists for genuinely independent-lifecycle code (Bono runs it too). rc-agent-mobile runs on a single operator's devices — no such independence benefit.

**Consequence:** `.gitignore` in `racecontrol/` must be amended to ignore Android build artifacts (`rc-agent-mobile/.gradle/`, `rc-agent-mobile/app/build/`, `rc-agent-mobile/.idea/`, `*.apk`, `local.properties`).

#### Tasks

1. Decide minSdkVersion.
   - **Choice: minSdk = 29 (Android 10).**
   - Rationale: Accessibility Service improvements (`TYPE_WINDOW_CONTENT_CHANGED` event batching, `GLOBAL_ACTION_TAKE_SCREENSHOT`) stabilized in API 28-29. Foreground service types (`dataSync`, `connectedDevice`) became formal enums in API 29+. Both target devices ship with Android 13 or 14 (confirmed: M07 is Android 14; TB-351FU ships with Android 14). Setting `minSdk = 29` costs nothing — 100% of fleet is covered — and keeps the door open for future reception-floor donations of older stock (e.g., an Android 11 tablet) without a migration.
   - Rejected alternatives: `minSdk = 34` (target Android 14 exclusively) would exclude any future hardware variance and trigger excessive `@RequiresApi` annotations; `minSdk = 21` would require polyfilling too much.
   - `targetSdk = 34` (Android 14 — current at scaffold time).

2. Run `gradle init --type kotlin-application` template is wrong for Android; instead create files by hand based on standard Android project structure:
   - `rc-agent-mobile/settings.gradle.kts` (`rootProject.name = "rc-agent-mobile"`, `include(":app")`)
   - `rc-agent-mobile/build.gradle.kts` (top-level, declares `kotlin` + `android` plugin versions)
   - `rc-agent-mobile/app/build.gradle.kts` (app module)
   - `rc-agent-mobile/gradle.properties` (`org.gradle.jvmargs=-Xmx2g`, `kotlin.code.style=official`, `android.useAndroidX=true`)
   - `rc-agent-mobile/gradle/wrapper/gradle-wrapper.properties` (Gradle 8.7)

3. Dependencies in `app/build.gradle.kts`:
   - `org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1`
   - `org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.3`
   - `io.ktor:ktor-server-core:2.3.12` + `io.ktor:ktor-server-netty:2.3.12` + `io.ktor:ktor-server-content-negotiation:2.3.12` (local :8090 HTTP)
   - `io.ktor:ktor-serialization-kotlinx-json:2.3.12`
   - `com.squareup.okhttp3:okhttp:4.12.0` (WebSocket client to comms-link — lighter than Ktor client for this use)
   - `androidx.core:core-ktx:1.13.1`
   - `androidx.lifecycle:lifecycle-service:2.8.0`
   - Test: `junit:junit:4.13.2`, `org.jetbrains.kotlinx:kotlinx-coroutines-test:1.8.1`, `io.mockk:mockk:1.13.10`

4. Application ID: `in.racingpoint.rcagentmobile`. App name: "RC Agent Mobile". Version: `0.1.0`, versionCode `1`.

5. Create `RcAgentApp.kt` (empty `Application` subclass) and an empty `MainActivity.kt` that displays "RC Agent Mobile running" — this is a placeholder; the user never interacts with it after install.

6. Amend root `racecontrol/.gitignore` to exclude Android build artifacts.

7. Build locally:
   ```bash
   cd C:/Users/bono/racingpoint/racecontrol/rc-agent-mobile
   ./gradlew :app:assembleRelease
   ```
   Artifact: `app/build/outputs/apk/release/app-release-unsigned.apk`. For dev installs we sign with the Android debug keystore (which Gradle auto-generates).

8. ADB install on both devices:
   ```bash
   adb devices  # both should appear
   adb -s <tab_plus_serial> install app/build/outputs/apk/debug/app-debug.apk
   adb -s <m07_serial> install app/build/outputs/apk/debug/app-debug.apk
   ```

#### Acceptance

- `./gradlew :app:assembleDebug` completes with exit 0.
- `./gradlew :app:testDebugUnitTest` runs (zero tests, exit 0).
- `adb shell pm list packages | grep racingpoint` on both devices returns the package.
- Tapping the app icon on each device shows "RC Agent Mobile running" for at least 1 second without crashing.

#### Checkpoint (human-verify)

Install the APK on both Tab Plus and M07 physically. Tap the icon. Confirm no crash. User replies "APK installed both devices, no crash" or describes what went wrong.

#### G4 NOT TESTED list (carry into commit)

- Foreground Service (phase 429-02).
- Any networking (phases 429-03, 429-05).
- Any runtime behavior beyond "app launches".

#### Commit message

```
feat(429-01): rc-agent-mobile Gradle scaffold, minSdk 29

Creates rc-agent-mobile/ at repo root per phase 429 recommendation.
Adds Kotlin Android app scaffolding with Ktor, OkHttp, kotlinx-serialization.
App installs via ADB on Tab Plus + M07; launches empty MainActivity.

Covers: AGENT-01
Not tested: Foreground Service (429-02), networking (429-03/05).
```

---

### 429-02-PLAN — Foreground Service + persistent notification + lifecycle

**Goal:** Agent runs as a Foreground Service with a persistent notification, survives Android OS background-killing, and automatically restarts within 10s if force-stopped.

**Covers:** AGENT-05

**Dependencies:** 429-01

**Type:** `auto` (automated test via `adb shell am force-stop` + wait)

#### Tasks

1. Create `AgentForegroundService.kt` extending `LifecycleService` (from `androidx.lifecycle:lifecycle-service`):
   - `onCreate`: build notification channel `rc_agent_mobile_status` (importance LOW — no sound), build persistent notification with static title "RC Agent Mobile" and **parameterizable** body text (initial: "Starting..."). Use `NotificationCompat.Builder`. Call `startForeground(NOTIFICATION_ID, notification, FOREGROUND_SERVICE_TYPE_DATA_SYNC or FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)`.
   - `onStartCommand`: return `START_STICKY` so the OS restarts the service if killed.
   - `onDestroy`: log "Service destroyed, scheduling self-restart" + broadcast intent to `BootCompletedReceiver` with action `ACTION_AGENT_RESTART` (self-heal pathway distinct from BOOT_COMPLETED).
   - Hold a `SupervisorJob` coroutine scope; child coroutines for HTTP server and comms-link client start in `onCreate` (stubbed to no-op in this plan — wired in 429-03 and 429-05).

2. Update `MainActivity.kt` (created in 429-01) to call `startForegroundService(Intent(this, AgentForegroundService::class.java))` in `onCreate` **unconditionally**. The Activity exists only to kick off the service — it can finish immediately after.

3. `AndroidManifest.xml` additions:
   - Permissions: `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_DATA_SYNC` (API 34+), `FOREGROUND_SERVICE_CONNECTED_DEVICE` (API 34+), `POST_NOTIFICATIONS` (API 33+).
   - `<service android:name=".service.AgentForegroundService" android:foregroundServiceType="dataSync|connectedDevice" android:exported="false" />`.

4. On API 33+ (Android 13+, which both target devices are on), `POST_NOTIFICATIONS` is a runtime permission. Handle this gracefully: if the permission is denied, the Foreground Service still runs but the notification is silently dropped by the OS. We accept this — the user will grant the permission in Phase 431's first-run UX. For 429-02 smoke testing, the installer must manually approve the notification prompt.

5. Helper: `AgentForegroundService.updateNotification(text: String)` — allows Phase 432 drivers to update the notification body ("Accepting Zomato orders..."). For 429-02, only "Starting..." and "Connected — heartbeat OK" (hardcoded) are exercised.

6. Write a unit test (JVM, not instrumented) asserting the notification channel is created with the correct channel id.  Use `mockk` to stub the `NotificationManager`.

#### Acceptance

- After tap-launch, `adb shell dumpsys notification --noredact | grep rc_agent_mobile_status` shows the channel.
- `adb shell dumpsys activity services in.racingpoint.rcagentmobile` shows `AgentForegroundService` with state `STARTED` and `foreground=true`.
- `adb shell am force-stop in.racingpoint.rcagentmobile` — wait 10s — re-check `dumpsys activity services`. **Expected:** service is back up (due to `START_STICKY`). **Note:** if OEM killing is too aggressive, document the failure mode in the test output and add mitigation in Phase 431.
- Unit test `AgentForegroundServiceTest.notificationChannelCreated` passes.

#### G4 NOT TESTED list

- Long-term durability across 24+ hours (tested in 429-08 drill and then in real-world use).
- Interaction with Android Doze mode (phase 429-06 handles BOOT; Doze is implicitly tested over time).
- HTTP server inside the service (429-03).
- comms-link WS client (429-05).

#### Commit message

```
feat(429-02): AgentForegroundService with persistent notification

Agent now runs as Foreground Service (START_STICKY, type=dataSync|connectedDevice).
Persistent notification channel "rc_agent_mobile_status" at IMPORTANCE_LOW.
force-stop test: service auto-restarts within 10s.

Covers: AGENT-05
Not tested: long-term Doze, OEM killers (real-world 429-08 + Phase 431).
```

---

### 429-03-PLAN — HTTP server on :8090 with /health + /build_id + /capability

**Goal:** Ktor embedded HTTP server runs inside AgentForegroundService on port 8090, exposes JSON endpoints that match the shape of Rust rc-agent's endpoints (so the existing server-side `/fleet/health` poller works with zero code changes).

**Covers:** AGENT-02

**Dependencies:** 429-02

**Type:** `auto`

#### Tasks

1. Create `LocalHttpServer.kt`:
   ```kotlin
   class LocalHttpServer(private val port: Int = 8090, private val deviceState: DeviceState) {
       private var server: NettyApplicationEngine? = null
       fun start() { /* embeddedServer(Netty, port) { ... }.start(wait = false) */ }
       fun stop() { server?.stop(1_000, 2_000) }
   }
   ```

2. Endpoints (all respond JSON):
   - `GET /health` → `{ ok: true, device_id, build_id, agent_version, uptime_secs, protocol_version: 1, ws_connected: <bool>, last_heartbeat_age_secs: <int?> }`
   - `GET /build_id` → plain text, the build_id (matches `rc-agent` `--build-id` flag output)
   - `GET /capability` → `{ capabilities: [], supported_device_types: ["tablet"|"phone"] }` (empty capabilities — Phase 432 populates)
   - `GET /heartbeat` → `{ last_heartbeat_at: <iso8601>, next_heartbeat_in_secs: <int> }` (introspection, not an action)

3. `DeviceState` singleton (in-memory, lifecycle-tied to the service):
   - `deviceId`: "rcm-tab-plus" or "rcm-m07" — computed at boot from `Build.MODEL` + `Build.SERIAL` (fall back to `Settings.Secure.ANDROID_ID` if SERIAL unreadable on API 29+).
   - `buildId`: baked in at compile time from `BuildConfig.GIT_HASH` (Gradle build step reads `git rev-parse --short HEAD` into `buildConfigField "String", "GIT_HASH"`).
   - `agentVersion`: from `BuildConfig.VERSION_NAME`.
   - `startTime`: `System.currentTimeMillis()` at service onCreate.
   - `wsConnected`: AtomicBoolean, updated by CommsLinkClient in 429-05.
   - `lastHeartbeatAt`: AtomicLong, updated by HeartbeatScheduler in 429-05.

4. `AgentForegroundService.onCreate` now starts `LocalHttpServer(8090, deviceState)`.

5. `AndroidManifest.xml` adds `android.permission.INTERNET` (required for Ktor server socket on some OEMs, harmless on others).

6. Unit test: smoke-test the HTTP server on a random port (Ktor TestApplication), assert all four endpoints return 200 with expected keys.

#### Acceptance

- Install, launch. From a LAN machine: `curl http://<device_ip>:8090/health` returns 200 JSON with `ok: true` and a truthy `build_id`.
- `/build_id` returns `text/plain` matching the commit hash.
- `/capability` returns `{"capabilities":[],"supported_device_types":["tablet"]}` on Tab Plus, `"phone"` on M07.
- Unit test `LocalHttpServerTest.allEndpointsReturn200` passes.

#### Extensibility hooks deliberately left in place

- `LocalHttpServer` exposes a `registerRoute(path, handler)` method stub with TODO for Phase 432 drivers to attach driver-specific routes like `/driver/zomato/order` without modifying LocalHttpServer.kt itself.

#### Commit message

```
feat(429-03): local HTTP :8090 with /health /build_id /capability /heartbeat

Ktor embedded Netty server inside AgentForegroundService.
/health shape matches racecontrol's /fleet/health poller expectations.
DeviceState singleton holds device_id + build_id (BuildConfig.GIT_HASH).

Covers: AGENT-02
Not tested: registration to comms-link (429-05), BOOT auto-start (429-06).
```

---

### 429-04-PLAN — Protocol spec doc (shared JSON contract)

**Goal:** Authoritative `rc-agent-mobile/docs/PROTOCOL.md` document defining the JSON envelope + message types the Kotlin agent speaks to comms-link. This is a deliverable of this phase so Phase 432+ can reference a stable schema.

**Covers:** AGENT-08 (protocol version slot + unknown-field policy)

**Dependencies:** 429-01 (file location)

**Type:** `auto` (docs-only; diff + lint)

#### Tasks

1. Write `rc-agent-mobile/docs/PROTOCOL.md`, ~200 lines, structured as:
   - Overview (one paragraph: relationship to comms-link v1 envelope; distinct from Rust rc-agent's AgentMessage enum)
   - Envelope definition (table of fields)
   - `protocol_version` field semantics: integer, current = 1, monotonically increasing, forward-compat rule "agent ignores unknown top-level fields and unknown payload fields"
   - Message types (Phase 429 scope only — later phases extend):
     - `register` (Android → relay)
     - `register_ack` (relay → Android)
     - `heartbeat` (Android → relay)
     - `heartbeat_ack` (relay → Android)
     - `ping` / `pong` (bidirectional)
     - `version_negotiation_error` (relay → Android, payload `{ requested: N, max_supported: 1 }`)
   - Version negotiation flow (sequence diagram):
     1. Android sends `register` with `protocol_version: 1`.
     2. Relay responds `register_ack` with `accepted: true, relay_protocol_version: <n>`.
     3. If relay sends a message with `protocol_version: 2` (or higher), Android logs WARN and ignores the payload but keeps the connection alive.
     4. If relay sends `protocol_version: 0` or negative, Android disconnects and logs ERROR (defensive).
   - Reconnect semantics: exponential backoff 1s → 2s → 4s → ... → 30s cap, with 0-500ms jitter (mirrors comms-link/docs/PROTOCOL.md existing convention).
   - Heartbeat semantics: every 30s, payload `{ uptime_secs, ws_connected, battery_pct?, memory_mb, last_activity_at? }`.
   - Unknown field policy: the agent uses `kotlinx.serialization` with `ignoreUnknownKeys = true` and `encodeDefaults = true`. Forward-compat is automatic for added fields; removed fields are a breaking change requiring a `protocol_version` bump.
   - Extension points: "Phase 432 will add `driver_event`, `selector_miss`, `capability_update` message types. This doc version: v1.0 scope is agent lifecycle only."

2. Also mirror the doc to `comms-link/shared/agent-protocol-v1.md` (symlink or copy) so the relay team (future-James or Bono) can reference it without pulling the racecontrol repo.

3. Define the `ignoreUnknownKeys` and `encodeDefaults` Json configuration in the spec so Kotlin, Rust, and Node implementers all apply the same rule.

#### Acceptance

- `rc-agent-mobile/docs/PROTOCOL.md` exists, ≥ 150 lines, contains sections listed above.
- `comms-link/shared/agent-protocol-v1.md` exists and matches (by content hash or explicit copy step in commit).
- `markdownlint` (optional) passes.

#### G4 NOT TESTED list

- No code in this plan — docs only. The schema is enforced by code in 429-05 (registration) and the unit tests there.

#### Commit message

```
docs(429-04): rc-agent-mobile protocol spec v1

Authoritative JSON envelope + message schema for Android agent ↔ comms-link relay.
Defines protocol_version negotiation, unknown-field policy, reconnect backoff.
Distinct from Rust rc-agent's AgentMessage enum (different payload shapes).

Covers: AGENT-08 (schema + version slot; runtime enforcement in 429-05)
```

---

### 429-05-PLAN — Comms-link registration client + heartbeat loop + backoff + version negotiation

**Goal:** Kotlin agent connects to comms-link relay (James .27:8765 as primary; Bono VPS 100.70.177.44:8765 as failover), sends a `register` envelope per PROTOCOL.md, sends heartbeat every 30s, reconnects with exponential backoff on relay restart, and gracefully rejects incoming messages with `protocol_version > 1`.

**Covers:** AGENT-03, AGENT-04, AGENT-08 (runtime enforcement)

**Dependencies:** 429-02, 429-03, 429-04

**Type:** `auto`

#### Tasks

1. Create `protocol/Protocol.kt` with `@Serializable` data classes matching `docs/PROTOCOL.md`:
   - `Envelope<T>(v: Int, protocol_version: Int, type: String, from: String, ts: Long, id: String, payload: T)`
   - `RegisterPayload`, `HeartbeatPayload`, `PingPayload`, etc.
   - Top-level `Json { ignoreUnknownKeys = true; encodeDefaults = true }` singleton.

2. Create `comms/CommsLinkClient.kt`:
   - OkHttp WebSocket client. URL read from config — default `ws://192.168.31.27:8765` for dev; Phase 431 first-run UX stores the actual URL + PSK in EncryptedSharedPreferences. For 429-05 we hardcode the primary URL and read PSK from `BuildConfig.COMMS_PSK_DEV` (a Gradle build-time variable sourced from `local.properties` — see setup note below).
   - `Authorization: Bearer $psk` header on the upgrade request (matches comms-link/docs/PROTOCOL.md PSK auth).
   - On `onOpen`: send `register` envelope with the payload shape defined in §4 of this document.
   - On `onMessage(text)`: parse Envelope. If `protocol_version > 1`, log WARN "unsupported protocol_version=$v, ignoring" and continue.  If `== 1`, dispatch by `type` (`register_ack`, `heartbeat_ack`, `ping` → reply `pong`, unknown → log DEBUG and ignore).
   - On `onFailure` or `onClosed`: schedule reconnect with exponential backoff: `delay = min(30_000, 1_000 * 2.pow(attempt)) + Random.nextLong(0, 500)`. Reset attempt counter on successful `onOpen`.
   - Expose `AtomicBoolean isConnected` and `AtomicLong lastMessageAt` to `DeviceState` so HTTP `/health` stays accurate.

3. Create `comms/HeartbeatScheduler.kt`:
   - Uses `kotlinx.coroutines.flow.tickerFlow` (or a manual `while(isActive) { delay(30_000) }` loop) in the service's coroutine scope.
   - Every tick: build a `HeartbeatPayload` with `uptime_secs`, `memory_mb` (from `Runtime.getRuntime().totalMemory() - freeMemory()`), `battery_pct` (from `BatteryManager.BATTERY_PROPERTY_CAPACITY`), send via `CommsLinkClient.send()`.
   - If `CommsLinkClient.isConnected` is false, skip this tick (no queueing; the next reconnect will re-register and resume).

4. Wire `AgentForegroundService.onCreate` to:
   ```kotlin
   commsClient = CommsLinkClient(deviceState, serviceScope)
   commsClient.connect()
   heartbeat = HeartbeatScheduler(commsClient, deviceState, serviceScope)
   heartbeat.start()
   ```

5. Local config surface (for dev builds):
   - `local.properties` (already gitignored by Android convention): `commsPskDev=<actual_psk>`.
   - `app/build.gradle.kts` reads it: `buildConfigField("String", "COMMS_PSK_DEV", "\"${project.findProperty("commsPskDev") ?: ""}\"")`.
   - If empty, `CommsLinkClient` logs FATAL and refuses to connect — prevents shipping without a PSK.

6. Tests:
   - Unit: `CommsLinkClientTest` — stub OkHttp WebSocket, assert `register` is sent on open, assert reconnect backoff sequence.
   - Unit: `HeartbeatSchedulerTest` — virtual time (`runTest` from kotlinx-coroutines-test), advance 90s, assert 3 heartbeats sent.
   - Unit: `ProtocolVersionTest` — feed the client an `onMessage` with `protocol_version = 2`, assert WARN log was emitted and no exception thrown.

7. Comms-link relay side (cross-repo task!):
   - Register two new client identities `rcm-tab-plus` and `rcm-m07` in the relay's allowed-identities list. Location: inspect `comms-link/james/index.js` or the relay's identity registry — likely a TOML or hardcoded set.
   - **Task:** find where the relay validates the `from` field in received envelopes; add rcm-* to the allowlist.
   - **DEPLOY PARITY:** apply identically to Bono VPS relay (per CLAUDE.md DEPLOY PARITY rule).
   - If the relay currently hardcodes `{"james","bono"}` as valid `from` values, this plan must amend that set.

#### Acceptance

- Install build with a real PSK set in `local.properties`. Launch agent on Tab Plus.
- On James's comms-link relay, `tail -f ~/racingpoint/comms-link/LOGBOOK.md` (or the runtime log) shows a `register` received from `rcm-tab-plus` within 30s of device boot.
- Same on M07 → `rcm-m07`.
- For 5 minutes, the relay's log shows ~10 heartbeats per device (1 every 30s ± 5s).
- `curl http://<device_ip>:8090/health` shows `ws_connected: true` and `last_heartbeat_age_secs` between 0 and 30.
- `am force-stop` the agent, wait 15s: relay logs a disconnect + reconnect + new register.
- Send a crafted `protocol_version: 2` message via a WS debug tool: device log (logcat filter `RcAgentMobile`) shows WARN "unsupported protocol_version=2".
- Unit tests `CommsLinkClientTest`, `HeartbeatSchedulerTest`, `ProtocolVersionTest` all pass.

#### Key risks addressed

- **Lock-held-across-await** (CLAUDE.md rule): all state reads in CommsLinkClient use atomics; no mutexes held across coroutine suspend points. Unit test `NoLockAcrossAwaitTest` uses `kotlinx-coroutines-debug` to assert no suspended coroutine is holding a `Mutex.lock()`.
- **Silent registration failure:** if the relay returns 401 (bad PSK), OkHttp fires `onFailure` with HTTP 401 — CommsLinkClient must log this as ERROR (not WARN) and refuse to reconnect for 5 minutes (prevent PSK brute-force). Explicitly tested.
- **Interface-first:** Protocol.kt (data classes) is created BEFORE CommsLinkClient uses them — so each test file can be written top-down.

#### Commit message

```
feat(429-05): comms-link registration + heartbeat + protocol_version negotiation

Kotlin agent registers with comms-link relay, sends 30s heartbeat, reconnects
with exponential backoff (1s-30s cap + jitter).  Gracefully ignores messages
with protocol_version > 1 (AGENT-08).  Relay-side: allowlists rcm-tab-plus
and rcm-m07 identities on both James .27 and Bono VPS (DEPLOY PARITY).

Covers: AGENT-03, AGENT-04, AGENT-08
Not tested: reboot auto-register (429-06), long-running durability (429-08).
```

---

### 429-06-PLAN — BOOT_COMPLETED receiver + reboot auto-start

**Goal:** Physical device reboot → agent re-registers within 90s without human interaction.

**Covers:** AGENT-06

**Dependencies:** 429-02 (service), 429-05 (registration path)

**Type:** `checkpoint:human-verify` at end (physical reboot both devices)

#### Tasks

1. Create `boot/BootCompletedReceiver.kt`:
   ```kotlin
   class BootCompletedReceiver : BroadcastReceiver() {
       override fun onReceive(context: Context, intent: Intent) {
           when (intent.action) {
               Intent.ACTION_BOOT_COMPLETED,
               Intent.ACTION_LOCKED_BOOT_COMPLETED,
               "android.intent.action.QUICKBOOT_POWERON",
               "com.htc.intent.action.QUICKBOOT_POWERON" -> {
                   val svc = Intent(context, AgentForegroundService::class.java)
                   ContextCompat.startForegroundService(context, svc)
               }
           }
       }
   }
   ```

2. `AndroidManifest.xml`:
   - `<uses-permission android:name="android.permission.RECEIVE_BOOT_COMPLETED" />`
   - `<uses-permission android:name="android.permission.WAKE_LOCK" />` (to let the service finish starting before Doze kicks in)
   - `<receiver android:name=".boot.BootCompletedReceiver" android:enabled="true" android:exported="true" android:directBootAware="false">`
     - Intent filters for `android.intent.action.BOOT_COMPLETED`, `android.intent.action.LOCKED_BOOT_COMPLETED`, and OEM-specific `android.intent.action.QUICKBOOT_POWERON`.

3. First-launch caveat: Android requires the user to launch the app at least once before `RECEIVE_BOOT_COMPLETED` fires. This is baked into OS behavior, not configurable. Document this in `rc-agent-mobile/docs/INSTALL-NOTES.md` (a new file) so Phase 431 first-run UX can ensure the user taps the icon once post-install.

4. OEM-specific battery optimization: Samsung (One UI) and Lenovo's Android skin aggressively kill background apps. We cannot fix this in code — it requires Settings → App info → Battery → Unrestricted. Write this as a `USER_SETUP` manifest item that Phase 431 enforces via a first-run checklist. For 429-06, document it as a known setup requirement and verify manually during the checkpoint.

5. Tests:
   - Unit test: instantiate `BootCompletedReceiver`, send a mock `Intent(Intent.ACTION_BOOT_COMPLETED)`, mock `Context`, assert `startForegroundService` is called with the correct Intent. Use `mockk`.
   - Instrumented test (skipped on CI, run manually via `adb shell am broadcast -a android.intent.action.BOOT_COMPLETED`): verify service starts. **Caveat:** `am broadcast` from adb does NOT replicate the real BOOT_COMPLETED behavior on all OEMs — the only true test is physical reboot.

#### Acceptance

- Unit test `BootCompletedReceiverTest.startsServiceOnBoot` passes.
- `adb shell am broadcast -a android.intent.action.BOOT_COMPLETED -n in.racingpoint.rcagentmobile/.boot.BootCompletedReceiver` — check logcat for `BootCompletedReceiver: service started`. (Smoke test, not definitive.)
- **Physical reboot test (human-verify):**
  1. Unrestrict battery for the app in OS Settings on both devices.
  2. Reboot Tab Plus. Within 90s, verify `GET http://<tab_plus_ip>:8090/health` returns `ws_connected: true`.
  3. Repeat for M07.

#### Checkpoint (human-verify)

User physically reboots both devices (or the plan executor does, if present at reception). User reports:
- "Tab Plus re-registered in <N> seconds."
- "M07 re-registered in <N> seconds."
- Or: describes failure mode (stayed off, took > 90s, app crashed, etc.).

Resume signal: User reports both re-registration times or types "approved".

#### Commit message

```
feat(429-06): BOOT_COMPLETED receiver + reboot auto-start

Receiver starts AgentForegroundService on device boot.  Verified: both devices
re-register within 90s of physical reboot (after battery-unrestrict setup).
INSTALL-NOTES.md documents the "tap icon once post-install" requirement and
OEM battery-unrestrict requirement (addressed structurally in Phase 431).

Covers: AGENT-06
```

---

### 429-07-PLAN — Rotating structured log file (50 MB cap, JSONL)

**Goal:** Every lifecycle event (start, stop, crash, reconnect, registration success/failure, heartbeat send, protocol version warnings) is written to a rotating JSONL log file on device, capped at 50 MB total with automatic rotation. Format is structured so Phase 435's audit log can extend it.

**Covers:** AGENT-07

**Dependencies:** 429-02 (service lifecycle hooks), 429-05 (comms events)

**Type:** `auto`

#### Tasks

1. Create `log/RotatingLog.kt`:
   - Output directory: `context.getExternalFilesDir("logs")` (per-app, survives uninstall opt-in, accessible via adb).
   - File naming: `rc-agent-mobile.log.jsonl`, rotated to `.1.jsonl`, `.2.jsonl`, etc.
   - Total cap: 50 MB. Per-file cap: 10 MB. So up to 5 files.
   - Rotation policy: on each write, check current file size; if ≥ 10 MB, rename files down (`.1.jsonl` → `.2.jsonl` ... drop `.5.jsonl`) and open a new primary file.
   - JSONL line shape:
     ```json
     {"ts_ms": 1713440000000, "level": "INFO", "target": "comms", "event": "registered", "device_id": "rcm-tab-plus", "details": {"relay_url": "ws://192.168.31.27:8765"}}
     ```
   - Thread-safe via single-writer coroutine channel (bounded 256 messages, drop-oldest on overflow).

2. Wire existing log points:
   - `AgentForegroundService.onCreate` → `RotatingLog.info("lifecycle", "service_created")`
   - `AgentForegroundService.onDestroy` → `RotatingLog.warn("lifecycle", "service_destroyed")`
   - `CommsLinkClient.onOpen` → `RotatingLog.info("comms", "ws_opened")`
   - `CommsLinkClient.onFailure` → `RotatingLog.error("comms", "ws_failed", {throwable, response_code})`
   - `CommsLinkClient` register success → `RotatingLog.info("comms", "registered", {relay_accepted_version})`
   - `CommsLinkClient` register 401 → `RotatingLog.error("comms", "register_rejected_auth")`
   - `CommsLinkClient` protocol_version > 1 → `RotatingLog.warn("comms", "unsupported_protocol_version", {version})`
   - `HeartbeatScheduler` tick → `RotatingLog.debug("comms", "heartbeat_sent")`  (DEBUG — usually filtered out in prod).

3. Expose `/logs/tail?n=100` on the LocalHttpServer (protected by a service key from EncryptedSharedPreferences, set in Phase 431 — for 429-07, hardcode a dev key to `local.properties`). Returns the last N JSONL lines as `text/plain`. This is for on-site debugging so staff don't need ADB.

4. Tests:
   - Unit test `RotatingLogTest.rotationAt10MB` — write 11 MB of synthetic log lines, assert 2 files exist, primary is < 10 MB.
   - Unit test `RotatingLogTest.totalCapAt50MB` — write 55 MB, assert total disk usage ≤ 50 MB (oldest file dropped).
   - Unit test `RotatingLogTest.jsonlIsParseable` — write 10 lines, read file, parse each line as JSON, assert `ts_ms`, `level`, `target`, `event` keys present.

#### Extensibility hook

- `RotatingLog.write(event: LogEvent)` takes a typed `LogEvent` sealed class. Phase 435 audit log extends this with `UiAction`, `SelectorMiss`, `DriverEvent` subclasses — the writer and rotation logic are already in place.

#### Acceptance

- After 5 minutes of running (429-05 heartbeats + 429-06 reconnect), `adb shell ls -la /sdcard/Android/data/in.racingpoint.rcagentmobile/files/logs/` shows `rc-agent-mobile.log.jsonl` with size > 0.
- `adb shell cat /sdcard/Android/data/.../rc-agent-mobile.log.jsonl | head -5 | jq .` parses without error (5 valid JSONL lines).
- Lines include `service_created`, `ws_opened`, `registered`, `heartbeat_sent`.
- Unit tests all pass.

#### Commit message

```
feat(429-07): rotating structured JSONL log, 50 MB cap

5-file rotation (10 MB each), single-writer coroutine, thread-safe.
Wired to all lifecycle events in AgentForegroundService + CommsLinkClient.
/logs/tail endpoint on LocalHttpServer for on-site debug without ADB.

Covers: AGENT-07
Extensible: Phase 435 audit log extends LogEvent sealed class.
```

---

### 429-08-PLAN — Phase 429 E2E drill (both devices register + heartbeat for 5 min)

**Goal:** Full end-to-end drill validating all four Phase 1 success criteria in one uninterrupted run. This is the ship gate.

**Covers:** all of AGENT-01..08 (verification, not net-new implementation)

**Dependencies:** 429-01 through 429-07

**Type:** `checkpoint:human-verify` (physical devices + live relay)

#### Preconditions

- Both devices reachable on LAN, battery-unrestricted in OS Settings.
- comms-link relay up on James .27:8765 AND on Bono VPS 100.70.177.44:8765 (DEPLOY PARITY verified via `curl http://localhost:8766/relay/health` and equivalent on VPS).
- `local.properties` contains a valid `commsPskDev` matching the relay's PSK.
- racecontrol server on .23:8080 is up (for `/fleet/health` visibility — note: the server's poller must recognize the new client identities; this wiring is OQ-3 below and may be deferred to a subsequent server-side plan).

#### Drill script

1. Uninstall current APK on both devices: `adb uninstall in.racingpoint.rcagentmobile`.
2. Clean install of release APK: `adb install app-release.apk` on both.
3. Tap icon on each device (first-launch requirement for BOOT_COMPLETED).
4. Wait 30 seconds. Verify both devices appear at `curl http://localhost:8766/relay/health` as connected clients. **Success criterion 1 ✔.**
5. Let agents run 5 full minutes. Verify comms-link runtime log shows ~10 heartbeats per device. Verify `GET /health` on each device returns sane values (`ws_connected: true`, reasonable `last_heartbeat_age_secs`).
6. On Tab Plus: `adb shell am force-stop in.racingpoint.rcagentmobile`. Start timer. Measure: how long until `/health` endpoint responds again? **Success criterion 2:** should be < 10s.
7. On M07: physical power-button reboot. Start timer. Measure: how long until device shows up in relay again? **Success criterion 3:** should be < 90s (allowing for OS boot overhead; AGENT-06 target is re-register without human action — the 30s target in ROADMAP-v50.md refers to post-boot registration latency, not total reboot time).
8. Send a crafted message with `protocol_version: 2` via a debug WS script against the Tab Plus CommsLinkClient (tool to be written as part of this plan). Observe logcat: should contain WARN `unsupported_protocol_version=2`. Confirm connection stays alive (no disconnect, heartbeats continue). **Success criterion 4 ✔.**
9. Grab logs: `adb pull /sdcard/Android/data/in.racingpoint.rcagentmobile/files/logs ./drill-logs-tab-plus/`; same for M07. Commit to the SUMMARY.md evidence trail.

#### Acceptance (all four must pass)

- [ ] SC-1: Both devices in relay within 30s of boot ✔
- [ ] SC-2: force-stop → Foreground Service restart in < 10s ✔
- [ ] SC-3: reboot → re-register automatic, < 90s total ✔
- [ ] SC-4: v2 message gracefully ignored, connection survives ✔

#### Artifacts to save in SUMMARY.md

- `drill-logs-tab-plus/rc-agent-mobile.log.jsonl` (at least last 100 lines)
- `drill-logs-m07/rc-agent-mobile.log.jsonl` (at least last 100 lines)
- Stopwatch measurements for SC-2 and SC-3
- Screenshot of `curl http://localhost:8766/relay/health` showing both clients

#### Checkpoint (human-verify)

User runs the drill script, reports pass/fail for each SC with numeric measurements.  If any SC fails, create a gap-closure plan (429-0N or a new 430-prep plan) — do NOT mark Phase 429 complete.

#### Commit message

```
test(429-08): Phase 429 E2E drill — register + force-stop + reboot + v2 rejection

All four Phase 1 success criteria exercised on Tab Plus + M07.
Evidence: drill-logs/ + stopwatch measurements in SUMMARY.md.

Covers: full Phase 429 acceptance gate.
```

---

## 6. Risks and pitfalls (Android-specific)

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **Android Doze mode** throttles network + heartbeats in idle | Foreground Service is Doze-exempt for foreground-service-type work (`dataSync`, `connectedDevice`). Battery-unrestrict is mandatory and captured as a 429-06 user-setup item. |
| R-2 | **Samsung One UI and Lenovo skins** aggressively kill background apps | Foreground Service + battery-unrestrict. Phase 431 first-run UX codifies this. If OneUI still kills the agent, Phase 431 adds a foreground-service persistent notification tap target instructing the user to add the app to "Never sleeping apps". |
| R-3 | **Android 14 Foreground Service launch restrictions** — services cannot be started from BOOT_COMPLETED without `dataSync`/`connectedDevice` type declared | `foregroundServiceType="dataSync\|connectedDevice"` in manifest + corresponding permissions (429-02). |
| R-4 | **API 33+ POST_NOTIFICATIONS runtime permission** — FGS runs but no notification | Handle permission denial gracefully (service still runs). Phase 431 first-run UX prompts user. |
| R-5 | **App must be launched once post-install** before BOOT_COMPLETED fires | INSTALL-NOTES.md calls this out; Phase 431 enforces "tap to finish setup". |
| R-6 | **Kill-switch dependencies on OEM OS behavior** — cannot test all OEM skins exhaustively | Ship to Tab Plus + M07 ONLY in v50.0. No third device supported. Revisit if fleet expands. |
| R-7 | **PSK distribution** — hardcoding in `local.properties` for dev; production needs secure channel | Phase 431 first-run UX uses QR code scan or manual entry into EncryptedSharedPreferences. 429-05 uses dev PSK placeholder explicitly — documented in INSTALL-NOTES.md. |
| R-8 | **Silent registration failure** if comms-link relay rejects `from: rcm-*` | 429-05 amends the relay's identity allowlist as a cross-repo task. Failure mode: OkHttp `onFailure` with 401 → RotatingLog.error → visible in `/logs/tail` + in persistent notification update ("RC Agent Mobile — auth failed"). |
| R-9 | **Lock-held-across-await** (CLAUDE.md Rust rule applies here too) | Kotlin coroutines + atomic state; explicit `NoLockAcrossAwaitTest` in 429-05 using kotlinx-coroutines-debug. |
| R-10 | **APK signing** — debug keystore is local to whoever built it; reinstall fails if signed by different key | 429-01 generates/reuses a stable debug keystore committed to `rc-agent-mobile/keystores/debug.keystore` (ok because it's dev-only, not release-signing). Phase 431 or a later phase handles release keystore properly. |
| R-11 | **No Gradle Wrapper bytecode** — CI may not have Java 17 for Gradle 8.7 | `rc-agent-mobile/gradle/wrapper/gradle-wrapper.jar` is committed (standard practice). CI container must have JDK 17+ (document in a CI note for Phase 429 ship gate). |
| R-12 | **Emulator ≠ real device** | Only real-device tests count. 429-01 checkpoint, 429-06 checkpoint, 429-08 drill are all real-device. |

## 7. Test plan

### Unit tests (JVM, fast, on every build)
- `AgentForegroundServiceTest` (429-02)
- `LocalHttpServerTest` (429-03)
- `ProtocolVersionTest` (429-05)
- `CommsLinkClientTest` (429-05)
- `HeartbeatSchedulerTest` (429-05)
- `NoLockAcrossAwaitTest` (429-05)
- `BootCompletedReceiverTest` (429-06)
- `RotatingLogTest` (429-07)

All unit tests run as part of `./gradlew :app:testDebugUnitTest` on every build. Gradle task returns non-zero on any failure.

### Instrumented tests (skip on CI, run before release)
- `InstrumentedForegroundTest` — install, force-stop, wait, confirm service restart.
- `InstrumentedHttpSmokeTest` — device hits its own `/health` via loopback.

Run via `./gradlew :app:connectedDebugAndroidTest` with a connected device.

### Physical device tests (human-verify)
- 429-01 checkpoint: install + launch both devices.
- 429-06 checkpoint: physical reboot both devices.
- 429-08 drill: full E2E.

### `/fleet/health` verification

**Caveat:** The racecontrol server's `/fleet/health` is currently populated from the pods' WS connection state (`crates/racecontrol/src/...`). Extending `/fleet/health` to include Android comms-link clients is a SERVER-SIDE CHANGE that is not in Phase 429's scope — it probably belongs to a later phase or to Phase 13 (admin reception view). For 429-08, we verify registration via the comms-link relay's own `/relay/health` or runtime log, not via `/fleet/health`. See OQ-3.

## 8. Verification gates (per CLAUDE.md)

- **nyquist-audit (required):** Registration logic + heartbeat scheduler + version negotiation are business logic. Before 429-08 closes, run `gsd-nyquist-auditor` on 429-05 deliverables.
- **MMA audit (required — cross-system bridge):** Kotlin (Android) ↔ Node.js (comms-link relay) ↔ Rust (racecontrol server) is a 3-language, 2-process bridge. CLAUDE.md explicitly requires MMA for cross-system bridges **with dual reasoning modes** (abstract + trace-level). Run before Phase 429 ship gate. Budget: $5.
- **integration-checker (required — multi-phase, cross-language):** Run before the v50.0 milestone ship.
- **codebase-mapper (required):** `rc-agent-mobile/` is a new top-level module. Run `gsd-codebase-mapper` before Phase 430 begins so the map includes the new module. Defer to after 429-01 so the directory exists.
- **ui-researcher / ui-auditor:** Skip. Phase 429 has no user-facing UI (persistent notification only, standard Android).
- **SEC gate:** `node comms-link/test/security-check.js` must pass after 429-05 amends relay-side identity allowlist — extends the security-check coverage to rcm-* identities and PSK handling.
- **Deploy Manifest Protocol (DMP):** Already captured in this PLAN's frontmatter `deploy:` section. Executor must tick each item and verifier must confirm.
- **Backlog gate (CLAUDE.md CGP v4.3):** Phase 429 must reach DEPLOYED-VERIFIED (APK installed on both devices + 429-08 drill passed) before Phase 430 may begin. COMMITTED ≠ SHIPPED.

## 9. Open questions the planner cannot decide

These require a user decision before executing the flagged plans. Listed in execution-blocking order.

**OQ-1 — comms-link relay identity allowlist (BLOCKS 429-05).**
Where in the comms-link codebase is the set of allowed `from` identities? It might be (a) hardcoded in `james/index.js`, (b) a TOML config, (c) derived dynamically from the PSK allowlist. Inspecting the relay source in 429-05 will answer this, but the user may have prior knowledge that short-cuts the investigation. If not, 429-05's first task is "grep the relay for `'james'|'bono'|from:|identity|allowlist`".

**OQ-2 — do Android agent logs go to comms-link or a separate endpoint? (BLOCKS 429-07 design choice, minor impact)**
`RotatingLog` writes to device-local disk. Do we ALSO ship log lines to comms-link in real-time (as `log_event` messages)? Or does that wait for Phase 435 audit log? **Default assumption:** Phase 429 writes locally only; Phase 435 adds relay shipping. Confirm before 429-07 if this is wrong.

**OQ-3 — `/fleet/health` extension for Android agents (OUT OF SCOPE for 429 but decision needed for verification).**
Phase 1's success criterion 1 is "Both devices show up in `/fleet/health` within 30s of device boot". The racecontrol server's `/fleet/health` handler currently enumerates pods from the WS connection table (see `crates/racecontrol/src/.../fleet.rs`). Extending it to also report Android clients via the comms-link relay is a server-side change that is NOT part of Phase 429's Kotlin scope. Options:
- (a) Treat this success criterion as satisfied by the comms-link relay's own `/relay/health` (both agents visible there) — defer server `/fleet/health` extension to Phase 13.
- (b) Add a server-side subtask here in Phase 429 that teaches `/fleet/health` to query the relay.
- **Recommendation:** (a). Document in 429-08 drill: "SC-1 verified via relay `/relay/health`; server `/fleet/health` integration deferred to Phase 13 as originally planned." User to confirm.

**OQ-4 — release keystore for APK signing (BLOCKS any non-debug install).**
Phase 429 uses the auto-generated debug keystore for ADB installs, which is fine for development. A release keystore (password + key alias) will be needed before Phase 431 ships installable APKs to staff. Should the release keystore (a) live in `rc-agent-mobile/keystores/release.keystore` encrypted with `git-crypt` or similar, (b) live outside the repo in `~/.android/`, or (c) be managed by Android Studio's signing config with credentials in `local.properties` only? Default: (b) until Phase 431 decides.

**OQ-5 — Tab Plus vs. M07 capability split for `supported_device_types`.**
Phase 429 defaults Tab Plus → `"tablet"` and M07 → `"phone"` based on `Build.MODEL` heuristics. For the M07, `Build.DEVICE` might not obviously be a phone (it is Android but with tablet-like dimensions in some regions). Confirm the intended split. If unsure, ship with a config override slot in `local.properties` (`deviceTypeOverride=tablet`) so James can flip it post-install without rebuild. **Recommended default:** keep heuristic + override slot; this is what 429-03 implements.

**OQ-6 — what is the comms-link relay's `from` value for Android agents going to be, exactly?**
Proposed: `rcm-tab-plus` and `rcm-m07`. But the existing convention is `james` and `bono` (symbolic) rather than device-typed. Alternative: `rcm-<device_id>` where `device_id` is a UUID generated at first-run and stored in EncryptedSharedPreferences (collision-proof but not human-readable in relay logs). **Recommendation:** keep human-readable `rcm-tab-plus` / `rcm-m07` for v50.0 (fleet size 2 — no collision risk); switch to UUID namespace if the fleet ever exceeds ~5 Android devices.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md`
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Reference Rust agent (patterns, NOT code reuse):** `crates/rc-agent/`, `crates/rc-common/`
- **Reference relay protocol:** `comms-link/docs/PROTOCOL.md`
- **Project memory active work:** `project_v50_rc_agent_mobile.md`

## 11. Output (at phase close)

At the end of Plan 429-08 (E2E drill pass), create `.planning/phases/429-kotlin-scaffold-http-comms-link/SUMMARY.md` capturing:
- Which commits implemented each plan (429-01 through 429-08)
- Actual stopwatch measurements for success criteria SC-1..SC-4
- Log excerpts (tailed JSONL from both devices)
- Any risks encountered and how they were resolved
- Any open questions resolved during execution (update §9 state)
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked
- Handoff to Phase 430 (Accessibility Service foundation) — what's ready, what's deferred

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 1 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
