---
phase: 431-bootstrap-install-first-run-ux
phase_number: 431
milestone: v50.0 rc-agent-mobile
name: "Bootstrap Install + First-run UX"
status: ready-to-execute
goal: >
  Non-technical venue staff can install rc-agent-mobile onto a Lenovo Tab Plus
  (TB-351FU) or Samsung Galaxy M07 in under 5 minutes using only a printed
  one-page step-by-step guide — no James involvement. On first launch, the
  agent presents a single-screen Activity that walks the user through four
  Android permissions (Accessibility enable, "display over other apps",
  "install unknown apps" per-source, disable battery optimization), with live
  status indicators per item, resume-on-return detection, and one-tap intent
  launchers. Once shipped, the agent self-updates from a comms-link-hosted
  APK via user-confirmed PackageInstaller intent (Android forbids silent
  install without device-owner provisioning, which we deliberately do NOT use
  in v50.0).
requirements: [INSTALL-01, INSTALL-02, INSTALL-03]
depends_on: [429-kotlin-scaffold-http-comms-link]
wave: 3
plan_count: 6
plans:
  - 431-01-PLAN: First-run Activity UI — single screen, 4-item permission checklist
  - 431-02-PLAN: Permission intent launchers (4 Android system intents)
  - 431-03-PLAN: Per-permission detection + onResume re-check loop
  - 431-04-PLAN: APK self-update — manifest poll + download + install-intent trigger
  - 431-05-PLAN: Printed one-page install guide (PDF, pictures, < 1 page)
  - 431-06-PLAN: Dry-run on both devices + staff usability test (Uday or Vishal)
autonomous: false # 431-01 (UI-SPEC review), 431-05 (staff review), 431-06 (live staff walk-through) are human-verify.
files_modified:
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/firstrun/FirstRunActivity.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/firstrun/PermissionChecklistViewModel.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/firstrun/PermissionItem.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/firstrun/PermissionStatus.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/firstrun/PermissionIntentLauncher.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/firstrun/PermissionDetector.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/update/UpdateManifest.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/update/UpdatePoller.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/update/ApkDownloader.kt
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/update/ApkInstallIntent.kt
  - rc-agent-mobile/app/src/main/res/layout/activity_first_run.xml
  - rc-agent-mobile/app/src/main/res/values/strings.xml
  - rc-agent-mobile/app/src/main/res/values/colors.xml
  - rc-agent-mobile/app/src/main/res/drawable/ic_status_pending.xml
  - rc-agent-mobile/app/src/main/res/drawable/ic_status_done.xml
  - rc-agent-mobile/app/src/main/res/drawable/ic_status_error.xml
  - rc-agent-mobile/app/src/main/AndroidManifest.xml                 # add FirstRunActivity, REQUEST_INSTALL_PACKAGES, QUERY_ALL_PACKAGES
  - rc-agent-mobile/app/src/test/kotlin/...                          # unit tests
  - rc-agent-mobile/docs/UI-SPEC-firstrun.md                         # first-run Activity specification (gsd-ui-researcher output)
  - rc-agent-mobile/docs/UI-REVIEW-firstrun.md                       # first-run Activity review (gsd-ui-auditor output)
  - rc-agent-mobile/docs/INSTALL-NOTES.md                            # extends the one created in 429-06
  - docs/rc-agent-mobile-install-guide.pdf                           # printed one-pager for staff
  - docs/rc-agent-mobile-install-guide.md                            # source markdown (PDF generated from this)
  - .planning/phases/431-bootstrap-install-first-run-ux/SUMMARY.md   # filled at end

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: >
    comms-link relay must host APK artifacts at a stable URL
    (proposed: https://api.racingpoint.cloud/mobile-agent/manifest.json
    and ...mobile-agent/<version>/app-release.apk). Bono VPS serves the
    manifest + APKs. James's relay at .27:8765 mirrors for LAN-fast path.
    File hosting permission on the relay must not require staff JWT — the
    manifest is public (read-only, SHA256-verified on device). See OQ-2.
  data_files: >
    rc-agent-mobile/app/src/main/res/raw/install_guide_thumbnail.png
    (a 400x600 thumbnail of the printed guide, shown on the first-run screen
    "Need help? Ask your manager for the printed guide").
  bat_file: none
  cloud_parity:
    - comms-link Bono VPS must serve the APK manifest JSON + APK binary.
    - comms-link James .27 must mirror the APK for LAN-fast updates.
    - Both must publish the same manifest version + SHA256 (identical hashes).
  targets:
    - tab_plus   # Lenovo TB-351FU
    - m07        # Samsung Galaxy M07
    - bono_vps   # manifest + APK hosting
    - james_27   # LAN APK mirror
  apk_artifact: rc-agent-mobile/app/build/outputs/apk/release/app-release.apk
  rollback:
    - "Previous APK preserved on device at /sdcard/Download/rc-agent-mobile-prev.apk"
    - "Rollback command (user-facing): open Files app, tap rc-agent-mobile-prev.apk, confirm install"
    - "Rollback manifest: relay serves manifest.json with previous version to roll fleet back in one push"

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: required       # First-run Activity IS a staff-facing UI surface. UI-SPEC.md produced BEFORE 431-01 execution.
  ui_auditor: required          # UI-REVIEW.md produced AFTER 431-01 execution, BEFORE 431-06 live staff test.
  nyquist_auditor: required     # Permission detection + manifest polling + version comparison are business logic.
  mma_audit: required           # Cross-system: Android install-intent -> PackageInstaller -> APK signing. OEM skin divergence is a known ToS-adjacent failure surface. Run before 431-06. Budget: $5.
  integration_checker: skip     # Single-phase scope; integration-checker runs at v50.0 milestone ship, not here.
  codebase_mapper: skip         # Phase 429 already mapped rc-agent-mobile/; 431 does not create new top-level directories.

risks_summary:
  - "OEM skin divergence — Samsung One UI (M07) and Lenovo's Android skin (Tab Plus) place battery-optimization and accessibility settings in different menu paths; intent launchers must handle ActivityNotFoundException with a graceful text fallback."
  - "Android 13+ POST_NOTIFICATIONS is a runtime permission — first-run must request it as a 5th step (or as a hidden prerequisite inside the checklist) or the Foreground Service notification is silently dropped."
  - "ACTION_MANAGE_UNKNOWN_APP_SOURCES is per-source on API 26+ — granting 'allow from Files' does not grant 'allow from Chrome'. Self-update intent source is the rc-agent-mobile app itself (via FileProvider), so we must verify this specific source is allowed."
  - "PackageInstaller.Session API vs. ACTION_INSTALL_PACKAGE intent — intent-based install still shows the system confirmation dialog on non-device-owner devices; Session API needs REQUEST_INSTALL_PACKAGES + same OS dialog. We cannot skip the confirmation. Documented in INSTALL-NOTES.md."
  - "Staff usability — a one-page printed guide with pictures only works if the photos match what the staff actually sees on the device; One UI + Lenovo skin photos will diverge. Print two variants OR use screenshots annotated with generic arrows."
  - "TM-T82 printer setup (project_tm_t82_tablet_setup.md) found that developer options on the Tab Plus could not be unlocked — not a blocker for 431 (we deliberately avoid ADB for staff install) but a reminder that the staff device is locked down harder than expected; plan for NO reliance on dev features."
  - "APK signing key MUST be stable across self-updates — if the release keystore rotates, self-update fails with INSTALL_FAILED_UPDATE_INCOMPATIBLE and the user sees a cryptic error. Release keystore handling is OQ-4 from Phase 429; 431-04 explicitly surfaces it."
---

# Phase 431 — Bootstrap Install + First-run UX

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 431 |
| Name | Bootstrap Install + First-run UX |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | INSTALL-01, INSTALL-02, INSTALL-03 |
| Dependencies | Phase 429 (Kotlin scaffold + HTTP + comms-link registration) |
| Wave | 3 (runs after 429 ship-gate + 430 Accessibility foundation) |
| Status | Ready to execute |
| Autonomous | No — 431-01 (UI-SPEC review), 431-05 (guide review), 431-06 (live staff test) have human-verify checkpoints |
| Ship test | Uday or Vishal installs on a wiped device using only the printed guide in < 5 min, completes the 4-step checklist without asking James |

## 2. Success criteria (verbatim from ROADMAP-v50.md Phase 3)

1. **Printed guide install:** Staff can install the agent with a printed 5-step guide (no James involvement).
2. **Single-screen checklist:** First-run Activity guides through Accessibility + overlay + install-unknown-apps + battery-optimization-off in one screen.
3. **User-confirmed self-update:** Agent self-update accepts APK from comms-link with a single user-confirmation tap.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above, from the STAFF perspective?"

### Truths (user-observable)

- T-1: A non-technical staff member with ONLY the printed one-page guide and a Windows PC can get the APK from Windows onto the Android device via MTP file transfer in under 5 minutes (INSTALL-01).
- T-2: Tapping the copied APK in the Android Files app opens the system installer, which offers an "install unknown apps" toggle for Files as the source; staff flips the toggle once, taps Install, returns to Files, taps Install again — install succeeds (INSTALL-01).
- T-3: On first launch of the installed app, the FIRST screen the user sees is the FirstRunActivity checklist — not MainActivity, not a blank screen, not a crash. Checklist shows 4 items with clear titles, one-line descriptions, and status indicators in three visual states: pending (grey), done (green check), error (red with text) (INSTALL-02).
- T-4: Tapping any of the 4 checklist items opens the correct Android system Settings page for that permission via an Intent; when the user taps Back, the Activity re-checks that item's status and updates the indicator without requiring a restart (INSTALL-02).
- T-5: Accessibility toggle in Settings → Accessibility → Services → "RC Agent Mobile" → On → returns to FirstRunActivity → item 1 now shows a green check (INSTALL-02, integration with ACCESS-04 from Phase 430).
- T-6: Overlay permission Settings → Apps → Special access → Display over other apps → RC Agent Mobile → On → returns → item 2 green check (INSTALL-02).
- T-7: Install unknown apps Settings → Apps → Special access → Install unknown apps → RC Agent Mobile → Allow → returns → item 3 green check (INSTALL-02).
- T-8: Disable battery optimization Settings → Apps → RC Agent Mobile → Battery → Unrestricted → returns → item 4 green check (INSTALL-02).
- T-9: Once all 4 items are green, a single "Finish Setup" primary button enables; tapping it dismisses FirstRunActivity, starts AgentForegroundService, and the staff member can close the app — it runs headless from now on (INSTALL-02).
- T-10: After a new APK is published to the comms-link manifest URL, within one UpdatePoller cycle (default 4 hours, configurable) the running agent shows a persistent-notification action "Tap to install update v0.2.0" OR launches a small in-app dialog on next foreground; tapping it downloads the APK, then launches the system install intent; user confirms once; app restarts on the new version (INSTALL-03).
- T-11: The printed one-page install guide (PDF) fits on A4 or Letter, under 1 page, has ≥ 3 annotated pictures showing (a) MTP file copy from Windows Explorer, (b) Android Files app tap-to-install, (c) "install unknown apps" toggle location. Guide includes the exact manifest URL / APK source for James or Bono to reissue in future (INSTALL-01).

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `.../firstrun/FirstRunActivity.kt` | Single-screen checklist UI | 120 | ViewBinding, 4 item views, onResume detection loop, Finish button |
| `.../firstrun/PermissionChecklistViewModel.kt` | State holder + detection logic wiring | 80 | `StateFlow<List<PermissionItem>>`, re-check on resume, Finish-enabled flag |
| `.../firstrun/PermissionItem.kt` | Data class for one checklist row | 30 | id, title, description, status, intentBuilder |
| `.../firstrun/PermissionStatus.kt` | Sealed class | 20 | Pending, Granted, Denied(reason) |
| `.../firstrun/PermissionIntentLauncher.kt` | 4 intent builders with OEM fallbacks | 100 | accessibility, overlay, unknownSources, batteryOpt |
| `.../firstrun/PermissionDetector.kt` | 4 detection functions | 80 | pure functions returning PermissionStatus |
| `.../update/UpdateManifest.kt` | @Serializable schema | 40 | version, url, sha256, min_agent_version, release_notes |
| `.../update/UpdatePoller.kt` | Periodic manifest poll | 80 | 4h default, respect manifest min_agent_version |
| `.../update/ApkDownloader.kt` | HTTPS download + SHA256 verify | 100 | streaming download, resume on fail, Mb-progress callback |
| `.../update/ApkInstallIntent.kt` | Launch system installer | 60 | FileProvider URI, ACTION_VIEW install intent, fall back to PackageInstaller.Session |
| `res/layout/activity_first_run.xml` | UI layout | 80 | Header, 4 ChecklistItemView, Finish button |
| `res/values/strings.xml` | Copy (English only for v50.0) | 40 | Titles + descriptions matching UI-SPEC |
| `docs/rc-agent-mobile-install-guide.md` | Source for printed one-pager | 50 | numbered steps + picture refs |
| `docs/rc-agent-mobile-install-guide.pdf` | Printed deliverable | N/A | ≤ 1 page A4/Letter |
| `rc-agent-mobile/docs/UI-SPEC-firstrun.md` | UI researcher output | ≥ 150 | Layout, copy, state diagram, fallback text |
| `rc-agent-mobile/docs/UI-REVIEW-firstrun.md` | UI auditor output | ≥ 100 | Accessibility audit, i18n readiness, OEM variance notes |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| App launch (MainActivity) | FirstRunActivity.start() | Kotlin call, conditional on "any item not Granted" | grep `startActivity(.*FirstRunActivity` in `MainActivity.kt` |
| FirstRunActivity.onResume | ViewModel.recheckAll() | Kotlin call | grep `recheckAll\|viewModel.refresh` in `FirstRunActivity.kt` |
| Checklist item tap | PermissionIntentLauncher.launch(item.id) | Kotlin call | grep `intentLauncher.launch` in `FirstRunActivity.kt` |
| PermissionIntentLauncher.accessibility() | ACTION_ACCESSIBILITY_SETTINGS intent | Android Intent | grep `Settings.ACTION_ACCESSIBILITY_SETTINGS` in `PermissionIntentLauncher.kt` |
| PermissionIntentLauncher.overlay() | ACTION_MANAGE_OVERLAY_PERMISSION + package URI | Android Intent | grep `ACTION_MANAGE_OVERLAY_PERMISSION` + `package:` in `PermissionIntentLauncher.kt` |
| PermissionIntentLauncher.unknownSources() | ACTION_MANAGE_UNKNOWN_APP_SOURCES + package URI (API 26+) | Android Intent | grep `ACTION_MANAGE_UNKNOWN_APP_SOURCES` in `PermissionIntentLauncher.kt` |
| PermissionIntentLauncher.batteryOpt() | ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS OR fallback to ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS | Android Intent | grep both in `PermissionIntentLauncher.kt` (must have fallback branch) |
| Finish button tap | AgentForegroundService.start + Activity.finish | Kotlin call | grep `startForegroundService\(.*AgentForegroundService` in `FirstRunActivity.kt` |
| UpdatePoller.tick | ApkDownloader.downloadIfNewer | Kotlin call | grep `ApkDownloader(` in `UpdatePoller.kt` |
| ApkDownloader.onComplete | ApkInstallIntent.trigger + user-visible notification | Kotlin call | grep `ApkInstallIntent\|notify\(UPDATE_AVAILABLE` in `ApkDownloader.kt` |
| ApkInstallIntent.trigger | FileProvider + ACTION_VIEW + application/vnd.android.package-archive | Android Intent | grep `FileProvider.getUriForFile` + `setDataAndType` in `ApkInstallIntent.kt` |
| AndroidManifest.xml | `.firstrun.FirstRunActivity` (not exported, launcher alias?) | XML | grep `<activity.*FirstRunActivity` |
| AndroidManifest.xml | `REQUEST_INSTALL_PACKAGES` permission + `QUERY_ALL_PACKAGES` on API 30+ | XML | grep `REQUEST_INSTALL_PACKAGES` |
| AndroidManifest.xml | FileProvider declaration for update APKs | XML | grep `FileProvider` + `file_paths` |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md        # scaffolding patterns, package layout, PSK handling, INSTALL-NOTES.md origin
@./.planning/phases/430-accessibility-service-foundation/PLAN.md       # Accessibility enable path that Phase 431's item 1 wires into
@./rc-agent-mobile/app/src/main/AndroidManifest.xml                    # from Phase 429 — add to, do not replace
@./rc-agent-mobile/docs/PROTOCOL.md                                    # from 429-04 — protocol version 1 semantics
@./rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt   # from 429-02 — Finish Setup hands off here

### Interfaces executors will need

**Pre-existing from Phase 429 (read-only — do not modify shape):**

```kotlin
// From 429-02
class AgentForegroundService : LifecycleService {
    companion object {
        fun startIntent(context: Context): Intent = Intent(context, AgentForegroundService::class.java)
    }
    // FirstRunActivity Finish button calls: ContextCompat.startForegroundService(ctx, AgentForegroundService.startIntent(ctx))
}

// From 429-03
object DeviceState {
    val deviceId: String
    val buildId: String
    val agentVersion: String   // "0.1.0"
    // 431-04 UpdateManifest compares agent_version to manifest.version using semver
}
```

**Pre-existing from Phase 430 (read-only):**

```kotlin
// From 430-N (Accessibility foundation)
class RcAccessibilityService : AccessibilityService {
    // AndroidManifest.xml already declares this service with the right intent filter.
    // Phase 431's accessibility intent must open Settings.ACTION_ACCESSIBILITY_SETTINGS;
    // the user scrolls to "RC Agent Mobile" and toggles on.  This is OS behavior —
    // we cannot deep-link to our specific service row on all OEMs (One UI blocks it).
}
```

**New, defined in this phase:**

```kotlin
// 431-01
sealed class PermissionStatus {
    object Pending : PermissionStatus()
    object Granted : PermissionStatus()
    data class Denied(val reason: String) : PermissionStatus()
}

data class PermissionItem(
    val id: PermissionId,          // ACCESSIBILITY | OVERLAY | UNKNOWN_SOURCES | BATTERY_OPT
    val title: String,             // e.g., "1. Enable Accessibility Service"
    val description: String,       // e.g., "Lets RC Agent read app screens to automate Zomato orders"
    val status: PermissionStatus,
)

enum class PermissionId { ACCESSIBILITY, OVERLAY, UNKNOWN_SOURCES, BATTERY_OPT, NOTIFICATIONS }
// NOTIFICATIONS included for Android 13+ POST_NOTIFICATIONS, either as a 5th item or as a
// pre-checklist runtime permission request.  Decision in 431-01 UI-SPEC.

// 431-04
@Serializable
data class UpdateManifest(
    val version: String,           // "0.2.0" semver
    val url: String,               // https://api.racingpoint.cloud/mobile-agent/v0.2.0/app-release.apk
    val sha256: String,            // hex lowercase
    val min_agent_version: String, // refuse update if current < this
    val release_notes: String,     // shown in install prompt
    val published_at: String       // ISO8601
)
```

## 5. Atomic plan breakdown (6 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

---

### 431-01-PLAN — First-run Activity UI (single screen, 4-item checklist)

**Goal:** A single-screen FirstRunActivity with a vertical list of 4 permission-checklist items, each showing title + one-line description + status indicator, plus a primary "Finish Setup" button at the bottom that is disabled until all 4 items are Granted. Activity launches automatically on first run (when any item != Granted) and does not launch again once all are Granted.

**Covers:** INSTALL-02 (UI shell only — detection logic is 431-03, intents are 431-02)

**Dependencies:** Phase 429 complete (MainActivity + AgentForegroundService exist)

**Type:** `checkpoint:human-verify` at end (UI-SPEC + UI-REVIEW per gates)

#### MANDATORY pre-task: gsd-ui-researcher

Before writing ANY code in 431-01, invoke `gsd-ui-researcher` to produce `rc-agent-mobile/docs/UI-SPEC-firstrun.md`. The spec must cover:

- Layout: ConstraintLayout vs. LinearLayout vs. Jetpack Compose (recommendation: ViewBinding + ConstraintLayout for minimum Compose runtime overhead on the M07; Compose is overkill for 4 static rows).
- Typography: Montserrat (matches RacingPoint brand) vs. system default. Recommendation: system default — brand fonts must ship in the APK, adding ~200 KB; the first-run screen is seen once per device and not customer-facing.
- Color scheme: Racing Red (#E10600) for the Finish button when enabled, Asphalt Black (#1A1A1A) for text on white background (not full dark mode — first-run is clearer in light mode, avoids OEM-specific dark-mode issues).
- Status indicators: grey dot (pending), green check (granted), red exclamation (denied) — use Material Icons via `androidx.core:core-ktx` (already included in 429-01 deps).
- Copy: exact English strings for each item's title + description. Must be ≤ 8 words for title, ≤ 25 words for description. Draft inline in the UI-SPEC, final in `strings.xml`.
- State diagram: (INITIAL, all pending) → (SOME_GRANTED) → (ALL_GRANTED, finish enabled) → (FINISH_TAPPED, service starts) → (ACTIVITY_FINISHES).
- Fallback text: what the user sees if `PermissionIntentLauncher.launch()` throws ActivityNotFoundException (rare but possible on heavily customized Lenovo skins). Recommendation: toast "Settings app unavailable — open Settings manually and search for '<permission name>'".
- Notifications item (Android 13+ POST_NOTIFICATIONS): decision gate — include as 5th checklist item OR request via `requestPermissions()` before FirstRunActivity loads. UI researcher picks the best UX and documents the tradeoff. Default recommendation: request before FirstRunActivity (simpler flow — user sees one system prompt, then the checklist).

UI-SPEC output lives at `rc-agent-mobile/docs/UI-SPEC-firstrun.md`. Commit it BEFORE writing code.

#### Tasks

1. Create `res/layout/activity_first_run.xml` matching UI-SPEC:
   - Header: "Welcome to RC Agent Mobile" + one-line subhead "Complete 4 quick permissions to get started"
   - 4 × ChecklistItemView (custom compound view or inline `<include>`). Each has: row number, title, description, status icon, tap target covering the whole row.
   - Primary button: "Finish Setup" — disabled until all 4 items Granted, Racing Red when enabled.

2. Create `firstrun/PermissionItem.kt`, `firstrun/PermissionStatus.kt` (data/sealed classes per §4).

3. Create `firstrun/PermissionChecklistViewModel.kt` (AndroidX ViewModel):
   - Holds `MutableStateFlow<List<PermissionItem>>` of 4 items (5 if UI-SPEC picks inline-checklist for notifications).
   - Exposes `val items: StateFlow<List<PermissionItem>>` for the Activity to collect.
   - Exposes `val allGranted: StateFlow<Boolean>` derived from items.
   - Exposes `fun recheckAll()` — calls PermissionDetector (defined in 431-03) for each item and updates flow.
   - **In 431-01 the detector returns stub values** (`Pending` for all) — 431-03 replaces the stub with real checks. This is interface-first: UI works end-to-end with stubs, detection slots in without UI changes.

4. Create `firstrun/FirstRunActivity.kt`:
   - `onCreate`: setContentView, wire ViewModel via `viewModels { factory }`, collect flows, wire item click listeners (stub — launcher in 431-02 replaces).
   - `onResume`: `viewModel.recheckAll()` — this is the critical UX — returning from Settings must update status.
   - Finish button: `startForegroundService(AgentForegroundService.startIntent(this))` + `finish()`.
   - **No launcher logic in 431-01** — item taps route through a stub `onItemTap(id: PermissionId)` that logs which item was tapped. 431-02 wires the real launcher.

5. Wire launch decision in `MainActivity.kt` (from 429-01):
   - `onCreate`: if any permission is not Granted (detect via 431-03's PermissionDetector — in 431-01, since detector is stubbed to Pending, launch FirstRunActivity unconditionally), `startActivity(Intent(this, FirstRunActivity::class.java))` then `finish()`.
   - After 431-03 ships, the check becomes real and MainActivity finishes silently when all permissions are granted.

6. Unit tests:
   - `PermissionChecklistViewModelTest` — provide a fake detector returning mixed statuses, assert `allGranted` is false when any is not Granted, true when all are Granted.
   - Snapshot test of Activity layout via Espresso or Robolectric — optional, gate by CI capacity.

#### Acceptance

- `./gradlew :app:assembleDebug` succeeds.
- Install, launch app. FirstRunActivity appears with 4 items all showing "pending" grey dots. "Finish Setup" is disabled (grey).
- Tap each item → logcat shows `FirstRunActivity: item tapped: ACCESSIBILITY` etc (stub).
- Back from a no-op settings page → onResume fires → status re-check runs (still Pending, since detector is stub).
- Unit test `PermissionChecklistViewModelTest` passes.

#### MANDATORY post-task: gsd-ui-auditor

After 431-01 acceptance passes, invoke `gsd-ui-auditor` to produce `rc-agent-mobile/docs/UI-REVIEW-firstrun.md`. Review covers:

- Accessibility: TalkBack support, minimum tap target size (48dp), contrast ratios, content descriptions on icons.
- i18n readiness: no hardcoded strings in layout XML (all in `strings.xml`), no right-to-left layout assumptions.
- OEM variance: review screenshots from both Tab Plus and M07 (emulator if device unavailable); note any rendering differences.
- Brand alignment: Racing Red button matches the venue's kiosk/web admin brand.
- Edge cases: extreme font scale (150%), narrow split-screen, landscape orientation (FirstRunActivity should be locked to portrait — Activity tag `android:screenOrientation="portrait"`).

UI-REVIEW output must be committed BEFORE 431-02 begins.

#### Checkpoint (human-verify)

User approves the UI by seeing a screenshot of FirstRunActivity on one physical device. Replies "UI approved" or describes issues. Resume signal: "UI approved".

#### Commit message

```
feat(431-01): FirstRunActivity single-screen permission checklist

4-item (or 5 with POST_NOTIFICATIONS) checklist, stub detection (431-03
replaces), stub intent launchers (431-02 replaces). Finish Setup button
hands off to AgentForegroundService and finishes the Activity.

UI-SPEC (gsd-ui-researcher): rc-agent-mobile/docs/UI-SPEC-firstrun.md
UI-REVIEW (gsd-ui-auditor):  rc-agent-mobile/docs/UI-REVIEW-firstrun.md

Covers: INSTALL-02 (UI shell)
Not tested: real permission detection (431-03), real intent launches (431-02).
```

---

### 431-02-PLAN — Permission intent launchers (4 Android system intents)

**Goal:** Tapping any checklist item launches the correct Android system Settings page via an Intent, with OEM-fallback handling and ActivityNotFoundException safety.

**Covers:** INSTALL-02 (intent wiring)

**Dependencies:** 431-01

**Type:** `auto`

#### Tasks

1. Create `firstrun/PermissionIntentLauncher.kt`:

   ```kotlin
   class PermissionIntentLauncher(private val context: Context, private val packageName: String) {
       fun launch(id: PermissionId): LaunchResult = when (id) {
           PermissionId.ACCESSIBILITY    -> tryLaunch(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS))
           PermissionId.OVERLAY          -> tryLaunch(
               Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName"))
           )
           PermissionId.UNKNOWN_SOURCES  -> tryLaunch(
               // API 26+ has per-source toggle; earlier APIs have a global toggle at ACTION_SECURITY_SETTINGS
               if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O)
                   Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:$packageName"))
               else
                   Intent(Settings.ACTION_SECURITY_SETTINGS)
           )
           PermissionId.BATTERY_OPT      -> tryLaunchBatteryOpt()
           PermissionId.NOTIFICATIONS    -> tryLaunch(
               Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                   putExtra(Settings.EXTRA_APP_PACKAGE, packageName)
               }
           )
       }

       private fun tryLaunchBatteryOpt(): LaunchResult {
           // Primary: per-app dialog on API 23+
           val primary = Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS,
               Uri.parse("package:$packageName"))
           // Fallback: global battery-opt settings list (if primary fails, or on OEM skins that block it)
           val fallback = Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)
           return tryLaunch(primary).ifFailed { tryLaunch(fallback) }
       }

       private fun tryLaunch(intent: Intent): LaunchResult = runCatching {
           intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
           context.startActivity(intent)
           LaunchResult.Success
       }.getOrElse { LaunchResult.Failed(it) }
   }

   sealed class LaunchResult {
       object Success : LaunchResult()
       data class Failed(val cause: Throwable) : LaunchResult()
       inline fun ifFailed(block: () -> LaunchResult): LaunchResult =
           if (this is Failed) block() else this
   }
   ```

2. Wire `FirstRunActivity.onItemTap(id)` to call `launcher.launch(id)`; on `LaunchResult.Failed`, show a toast with the fallback instruction from UI-SPEC (e.g., "Open Settings → Apps → RC Agent Mobile → Permissions and enable <name>").

3. AndroidManifest.xml additions:
   - `<uses-permission android:name="android.permission.SYSTEM_ALERT_WINDOW" />` (required for overlay; checking if it's active also requires this permission to be declared)
   - `<uses-permission android:name="android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS" />` (required by ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS on API 23+)
   - `<uses-permission android:name="android.permission.REQUEST_INSTALL_PACKAGES" />` (actually not required for the launcher — for 431-04 self-update — but declare here so the same manifest covers both)
   - `<queries>` block (API 30+) listing intent actions we launch: `ACCESSIBILITY_SETTINGS`, `MANAGE_OVERLAY_PERMISSION`, etc. Without `<queries>`, `resolveActivity` can return null on API 30+ even when the Activity exists (package-visibility restrictions).

4. Unit tests:
   - `PermissionIntentLauncherTest` — mock `Context.startActivity`, assert correct Intent action + data URI for each PermissionId. Use `mockk` + `verify { context.startActivity(match { ... }) }`.
   - `BatteryOptFallbackTest` — first mock throws ActivityNotFoundException, assert fallback Intent is tried.

#### OEM fallback matrix (document in INSTALL-NOTES.md)

| OEM Skin | Accessibility | Overlay | Unknown sources | Battery opt |
|----------|---------------|---------|-----------------|-------------|
| Stock Android (Pixel-like) | ACTION_ACCESSIBILITY_SETTINGS lands on list | ACTION_MANAGE_OVERLAY_PERMISSION lands on app page | ACTION_MANAGE_UNKNOWN_APP_SOURCES lands on app page | ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS shows dialog |
| Samsung One UI (M07) | Lands on list, user scrolls | Lands on app-specific page | Lands on app-specific page | Dialog may be blocked; fallback opens global list, user finds app |
| Lenovo / ZUI (Tab Plus) | Lands on list | Lands on app page | Lands on app page | Dialog usually works; fallback rarely needed |

#### Acceptance

- Install, launch, tap each of the 4 items — the correct Settings page opens on both Tab Plus and M07.
- Tap Back on each page — FirstRunActivity re-appears. Status indicators are still Pending (detection is 431-03).
- Unit tests pass.
- AndroidManifest `<queries>` verified via `adb shell dumpsys package in.racingpoint.rcagentmobile | grep queries`.

#### Commit message

```
feat(431-02): permission intent launchers for 4 Android system intents

ACTION_ACCESSIBILITY_SETTINGS, ACTION_MANAGE_OVERLAY_PERMISSION,
ACTION_MANAGE_UNKNOWN_APP_SOURCES, ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS
with fallback to ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS.
<queries> block declared for API 30+ package visibility.
Graceful ActivityNotFoundException handling via toast fallback.

Covers: INSTALL-02 (intent wiring)
Not tested: status detection on return (431-03).
```

---

### 431-03-PLAN — Per-permission detection + onResume re-check

**Goal:** Each checklist item's status updates correctly after the user returns from the Settings page. Detection must be pure functions (testable, no Activity coupling) and fast (onResume must not block the UI thread).

**Covers:** INSTALL-02 (detection closes the feedback loop started in 431-01 and 431-02)

**Dependencies:** 431-01, 431-02

**Type:** `auto`

#### Tasks

1. Create `firstrun/PermissionDetector.kt`:

   ```kotlin
   class PermissionDetector(private val context: Context) {
       fun detect(id: PermissionId): PermissionStatus = when (id) {
           PermissionId.ACCESSIBILITY    -> detectAccessibility()
           PermissionId.OVERLAY          -> detectOverlay()
           PermissionId.UNKNOWN_SOURCES  -> detectUnknownSources()
           PermissionId.BATTERY_OPT      -> detectBatteryOpt()
           PermissionId.NOTIFICATIONS    -> detectNotifications()
       }

       private fun detectAccessibility(): PermissionStatus {
           // Read Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES, split on ':' and '/',
           // check if our RcAccessibilityService component name is present.
           val enabled = Settings.Secure.getString(
               context.contentResolver,
               Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
           ) ?: return PermissionStatus.Denied("accessibility service list unreadable")
           val target = ComponentName(context, RcAccessibilityService::class.java).flattenToString()
           return if (enabled.split(":").any { it.equals(target, ignoreCase = true) })
               PermissionStatus.Granted
           else PermissionStatus.Denied("accessibility service not enabled")
       }

       private fun detectOverlay(): PermissionStatus =
           if (Settings.canDrawOverlays(context)) PermissionStatus.Granted
           else PermissionStatus.Denied("overlay permission not granted")

       private fun detectUnknownSources(): PermissionStatus {
           // API 26+ per-source
           if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
               return if (context.packageManager.canRequestPackageInstalls())
                   PermissionStatus.Granted
               else PermissionStatus.Denied("app cannot request installs")
           }
           // Pre-26: global setting
           @Suppress("DEPRECATION")
           val allowed = Settings.Secure.getInt(
               context.contentResolver, Settings.Secure.INSTALL_NON_MARKET_APPS, 0
           ) == 1
           return if (allowed) PermissionStatus.Granted else PermissionStatus.Denied("global unknown sources off")
       }

       private fun detectBatteryOpt(): PermissionStatus {
           val pm = context.getSystemService(Context.POWER_SERVICE) as PowerManager
           return if (pm.isIgnoringBatteryOptimizations(context.packageName))
               PermissionStatus.Granted
           else PermissionStatus.Denied("battery optimization not disabled")
       }

       private fun detectNotifications(): PermissionStatus {
           if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return PermissionStatus.Granted
           return if (ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS)
                       == PackageManager.PERMISSION_GRANTED)
               PermissionStatus.Granted
           else PermissionStatus.Denied("notification permission not granted")
       }
   }
   ```

2. Replace the stub detector in `PermissionChecklistViewModel` with `PermissionDetector`. Call `detect()` off the main thread using `viewModelScope.launch(Dispatchers.IO)` — though the detect functions are fast (content-resolver reads), offloading is safer for onResume UI responsiveness.

3. `FirstRunActivity.onResume` already calls `viewModel.recheckAll()` from 431-01 — this now does real work.

4. Edge case: detecting Accessibility enables → user goes to Settings → toggles on → returns; but the ViewModel's `recheckAll` fires on Activity resume which happens BEFORE the user sees the UI. Verify no race: the Settings Activity fully returns control before onResume fires, so `Settings.Secure.getString` reads the updated value. This is guaranteed by Android lifecycle contract.

5. Unit tests:
   - `PermissionDetectorTest` — use Robolectric (or pure mockk shadowing of Settings.Secure) to assert each detect function returns Granted when the underlying state is present, Denied otherwise. 5 tests, one per PermissionId.
   - `PermissionChecklistViewModelTest` (extended from 431-01) — inject real PermissionDetector mocked through Robolectric, assert onResume recheck updates StateFlow.

6. Live test script (for 431-06 drill):
   - Install → launch → FirstRunActivity shown, all 4 items Pending.
   - Tap item 1 → Settings opens → user toggles Accessibility on → Back → item 1 shows green check.
   - Repeat for items 2, 3, 4. Finish button becomes enabled after item 4 turns green.
   - Tap Finish → AgentForegroundService starts → Activity finishes → persistent notification appears.
   - Relaunch app → MainActivity detects all permissions Granted → skips FirstRunActivity → shows a "Setup complete, RC Agent running in background" brief screen and finishes. (Or on subsequent launches, MainActivity is never seen at all because the user never re-opens the app — the launcher icon can be hidden via `activity-alias` once setup is complete; defer to a later phase if desired.)

#### Acceptance

- Live test: all 4 items individually turn green as their underlying permissions are granted. Finish button enables once all 4 green. Tapping Finish starts the service.
- Unit tests pass.
- Edge case verified: returning from Settings with no change keeps the item Pending (no false green check).

#### Commit message

```
feat(431-03): permission detection closes first-run feedback loop

PermissionDetector reads Settings.Secure, Settings.canDrawOverlays,
PackageManager.canRequestPackageInstalls, PowerManager.isIgnoringBatteryOptimizations,
and POST_NOTIFICATIONS runtime grant. ViewModel onResume re-check now
produces real status — stub replaced. Finish button enables when all green.

Covers: INSTALL-02 (detection closes the loop)
Not tested: self-update (431-04), printed guide (431-05), staff walk-through (431-06).
```

---

### 431-04-PLAN — APK self-update (manifest poll + download + install-intent)

**Goal:** Agent checks a comms-link-hosted manifest URL every N hours; when a newer version appears, downloads the APK, verifies SHA256, and launches the system install intent. User taps confirm once; the new APK installs, the OS restarts the app on the new version.

**Covers:** INSTALL-03

**Dependencies:** 431-03 (detector also confirms INSTALL_UNKNOWN_APP_SOURCES is Granted — self-update fails silently otherwise)

**Type:** `auto`

#### Tasks

1. Create `update/UpdateManifest.kt`:
   - `@Serializable data class UpdateManifest` as defined in §4.
   - Companion: `fun parse(jsonString: String): UpdateManifest` using `Json { ignoreUnknownKeys = true }` (future-compat per PROTOCOL.md policy).

2. Create `update/UpdatePoller.kt`:
   - Coroutine running in `AgentForegroundService.serviceScope`.
   - Polls the manifest URL every 4 hours (configurable via `BuildConfig.UPDATE_POLL_INTERVAL_MIN`, default 240).
   - Manifest URL: `https://api.racingpoint.cloud/mobile-agent/manifest.json` (cloud primary), with LAN fast-path `http://192.168.31.27:18890/manifest.json` when reachable. Try LAN first (2s timeout), fall through to cloud (10s timeout).
   - Compares `manifest.version` to `DeviceState.agentVersion` via semver (`com.vdurmont:semver4j:3.1.0` — add to deps; tiny).
   - On newer version AND `current >= manifest.min_agent_version`: emit `UpdateAvailable(manifest)` to a flow consumed by the service, which shows an in-app dialog OR updates the persistent notification with a "Tap to install update" action button.
   - On `current < min_agent_version`: refuse the update (would fail at install time with INSTALL_FAILED_VERSION_DOWNGRADE); emit an `IncompatibleUpdate` event for the log + notification.
   - Use OkHttp (already in 429-01 deps). No WebSocket — HTTP GET.

3. Create `update/ApkDownloader.kt`:
   - Downloads the APK from `manifest.url` to `context.getExternalFilesDir("updates")/rc-agent-mobile-<version>.apk`.
   - Streams bytes to disk (no loading the full file into memory — APK can be 30-50 MB).
   - Verifies SHA256 of the downloaded file against `manifest.sha256`; on mismatch, delete the file, log ERROR, emit `UpdateCorrupt` event.
   - Keeps a "previous" APK slot: before writing the new APK, move the existing `app-release.apk` (if any) to `/sdcard/Download/rc-agent-mobile-prev.apk` for rollback per DMP rollback spec.
   - Emits `DownloadProgress(bytesDone, bytesTotal)` for the persistent notification progress bar.

4. Create `update/ApkInstallIntent.kt`:
   - Uses `FileProvider.getUriForFile(ctx, "<authority>", apkFile)` — authority declared in AndroidManifest.xml as `in.racingpoint.rcagentmobile.fileprovider` with `res/xml/file_paths.xml` mapping `external-files-path/updates`.
   - Launches `Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)`.
   - User sees the standard Android install confirmation dialog — exactly one tap.
   - Fallback: if ACTION_VIEW fails (rare), use `PackageInstaller.Session` API (more code, same UX result). Document the fallback in INSTALL-NOTES.md but prefer the simpler intent.

5. UX wiring:
   - Persistent notification (from 429-02 + Phase 432 updates) gains a new action: "Update v0.2.0 available — tap to install". Tapping it calls `ApkInstallIntent.trigger(manifestEntry)`.
   - Alternatively, if the agent is foreground (user just opened the app), show an AlertDialog via a `UpdatePromptActivity` (small, one-screen) with "Install now" and "Later" buttons. "Later" defers for 24 hours.

6. AndroidManifest.xml additions:
   - `<uses-permission android:name="android.permission.REQUEST_INSTALL_PACKAGES" />` (already declared in 431-02 for consistency).
   - `<provider>` element for `androidx.core.content.FileProvider` with authority `in.racingpoint.rcagentmobile.fileprovider` and meta-data pointing to `@xml/file_paths`.

7. Create `res/xml/file_paths.xml`:
   ```xml
   <paths>
       <external-files-path name="updates" path="updates/" />
   </paths>
   ```

8. Release keystore handling (OQ-4 from Phase 429):
   - Self-update ONLY works if the new APK is signed with the SAME key as the installed APK (INSTALL_FAILED_UPDATE_INCOMPATIBLE otherwise).
   - For v50.0 development: reuse the debug keystore committed in 429-01 (`rc-agent-mobile/keystores/debug.keystore`) for both local dev AND the self-update test — the keystore is NOT a secret (it's already in git) and the debug signing config applies.
   - For production: commit a release keystore + password to a secure location outside the repo. Since v50.0 is not public-facing and only runs on 2 devices, the simplification stands: **use the same debug keystore for all signs**. Document in INSTALL-NOTES.md and flag for a future hardening phase.

9. Unit tests:
   - `UpdateManifestTest` — parse sample JSON, assert fields.
   - `UpdatePollerTest` — fake OkHttp responses for (a) newer version, (b) same version, (c) older version, (d) incompatible min_agent_version; assert emitted events.
   - `ApkDownloaderTest` — verify SHA256 match accepts file, mismatch deletes file + logs error.

10. Cloud side (cross-repo task for Bono relay):
   - Create directory `/var/www/racingpoint.cloud/mobile-agent/` on Bono VPS.
   - Write `manifest.json` containing the current release metadata.
   - Upload `app-release.apk` to the version-tagged URL.
   - Ensure nginx serves `manifest.json` with `Cache-Control: no-cache` and APKs with `Content-Type: application/vnd.android.package-archive`.
   - Mirror on James .27 via comms-link relay's static-file hosting OR an additional http.server on :18890 (matching the staging-server pattern from deploy-staging/).
   - **DEPLOY PARITY:** both must publish identical manifest + APK. CI verifies SHA256 match before ship.

#### Acceptance

- On Tab Plus (running v0.1.0): place a manifest.json at `http://192.168.31.27:18890/manifest.json` declaring v0.2.0 with a valid SHA256 of a signed v0.2.0 APK. Within 4h (or force a poll via a debug-menu tap), persistent notification action "Update v0.2.0 available" appears.
- Tap the notification action → system install dialog → tap Install → app updates.
- After update, `GET http://<device_ip>:8090/build_id` returns the v0.2.0 build_id.
- SHA256 mismatch: corrupt the APK on the server, agent downloads it, detects mismatch, logs ERROR, does not prompt the user.
- Version downgrade refused: set manifest.version to 0.0.1, poller ignores it.
- Unit tests pass.

#### Commit message

```
feat(431-04): APK self-update via comms-link manifest + install intent

UpdatePoller hits LAN fast-path (.27:18890) then cloud (api.racingpoint.cloud)
every 4h.  ApkDownloader streams + SHA256-verifies + keeps .prev APK for rollback.
ApkInstallIntent triggers Android system installer — user confirms once.
Release keystore reuses debug keystore for v50.0 (simplification — no public
distribution). Documented in INSTALL-NOTES.md.

Covers: INSTALL-03
Not tested: staff walk-through (431-06).
```

---

### 431-05-PLAN — Printed one-page install guide (PDF, pictures)

**Goal:** A single printable A4 (or Letter) sheet, ≤ 1 page, that walks a non-technical staff member through: MTP copy from Windows → Android Files app install → enable "install unknown apps" → open the app → complete the 4-step checklist. Guide includes ≥ 3 annotated screenshots.

**Covers:** INSTALL-01 (printed guide is the deliverable)

**Dependencies:** 431-01 (first-run UI must be final before screenshots)

**Type:** `checkpoint:human-verify` (guide review before print)

#### Tasks

1. Write `docs/rc-agent-mobile-install-guide.md` as the source:
   - Title: "RC Agent Mobile — 5-Minute Install Guide"
   - Step 1: "Connect the phone/tablet to the staff PC with a USB cable. When Windows asks, choose **File transfer** on the device."
   - Step 2: "Open **This PC** in Windows Explorer. Find the device. Copy `rc-agent-mobile.apk` from `C:\Users\staff\Desktop\` to `Internal shared storage\Download` on the device."
   - Step 3: "On the device, open the **Files** app. Tap **Downloads**. Tap `rc-agent-mobile.apk`."
   - Step 4: "A dialog says 'Files can't install unknown apps'. Tap **Settings**, then toggle **Allow from this source** on. Press Back."
   - Step 5: "Tap **Install**. After it finishes, tap **Open**."
   - Step 6: "Follow the 4 items on the first screen. Each tap opens a Settings page — turn the switch on, press Back, the checkmark appears. When all 4 are green, tap **Finish Setup**."
   - Footer: "If stuck, WhatsApp James at +91-XXXX or scan QR to open this guide digitally."
   - QR code at the footer linking to the markdown source on GitHub so the guide is self-updating.

2. Annotated screenshots (take on both Tab Plus AND M07; pick the most generic-looking one per step for the printed version):
   - Screenshot A: Windows Explorer showing "Internal shared storage\Download" with the APK copied.
   - Screenshot B: Android Files app showing the APK in Downloads.
   - Screenshot C: Install unknown apps toggle page.
   - Screenshot D: Install confirmation dialog.
   - Screenshot E: FirstRunActivity with all 4 items (take after 431-03 is done and one item is green, one pending, etc. — shows all states).

3. Layout the guide in a print-friendly format. Options:
   - **Option A — Markdown + pandoc → PDF:** `pandoc install-guide.md -o install-guide.pdf --pdf-engine=xelatex --variable=geometry:a4paper,margin=1.5cm`. Reproducible from source; CI can regenerate on every edit.
   - **Option B — Google Docs/Canva:** More design-friendly but not in git, not version-controlled.
   - **Recommendation: Option A.** Markdown is the source of truth; PDF is a build artifact. Add a Makefile target `make install-guide` that runs pandoc. Commit both .md and .pdf so staff can grab either without building.

4. Verify: print it on A4. Does it fit on one page? Are the screenshots legible at print size? Revise until it does.

5. Place a shortcut on the Windows Desktop of the install-admin PC (likely the POS PC .130 or a staff laptop) linking to the markdown source and the PDF, so the next person who installs has it at hand.

6. Include the QR code (printed at footer) linking to the GitHub raw URL of the markdown. Tools: `qrencode -o qr.png "https://raw.githubusercontent.com/..."`.

#### Acceptance

- `docs/rc-agent-mobile-install-guide.md` exists and is ≥ 40 lines.
- `docs/rc-agent-mobile-install-guide.pdf` exists; `pdfinfo install-guide.pdf` reports `Pages: 1`.
- Printing test: print on a real printer (or the TM-T82 if shared printer is unavailable — the TM-T82 is 80mm receipt paper which is NOT suitable for A4; use a regular office printer at the venue). Verify readability.
- James or a proxy (Bono, since Uday is the real validator) reviews the draft and replies "guide approved" or notes revisions needed.

#### Checkpoint (human-verify)

User reads the printed guide and confirms:
- "Could a new hire follow this without help?" (Yes/No + notes)
- "Do the screenshots match the actual screens staff will see?" (Yes/No)

Resume signal: "guide approved" or list of revisions.

#### Commit message

```
docs(431-05): rc-agent-mobile printed install guide (PDF + MD source)

One-page A4 guide with 5 numbered steps + QR code to digital version.
Includes annotated screenshots for MTP copy, Android Files install,
unknown-sources toggle, and FirstRunActivity checklist.
Generated via pandoc from MD source — Makefile target `make install-guide`.

Covers: INSTALL-01
Not tested: staff can actually follow it (431-06).
```

---

### 431-06-PLAN — Dry-run on both devices + staff usability test

**Goal:** Prove INSTALL-01, INSTALL-02, INSTALL-03 all work on both physical devices, with a real non-technical human (Uday or Vishal) doing the install using only the printed guide. Time the install; target < 5 minutes.

**Covers:** all of INSTALL-01, INSTALL-02, INSTALL-03 (verification, not net-new implementation)

**Dependencies:** 431-01 through 431-05

**Type:** `checkpoint:human-verify` (physical devices + live staff member)

#### Preconditions

- A clean, uninstalled state on both devices: `adb uninstall in.racingpoint.rcagentmobile` on both (or factory-reset-equivalent for a true blind test).
- Printed copy of the install guide in hand.
- Signed release APK at `rc-agent-mobile/app/build/outputs/apk/release/app-release.apk` copied to the staff PC's Desktop as `rc-agent-mobile.apk`.
- Comms-link relay up on James .27:8765 AND Bono VPS 100.70.177.44:8765 (so the agent's registration test works after install).
- Manifest.json on Bono VPS declaring v0.1.0 as current (so self-update test in step 5 of the drill can trigger a forced upgrade).

#### Drill script

1. **Tester:** Uday or Vishal (must NOT be James). Hand them the device, a USB cable, the staff PC, and the printed guide.
2. Say exactly: "Install this app following the guide. If you're stuck for more than 30 seconds on any step, say so but don't ask me how."
3. Start a stopwatch.
4. Tester completes steps 1-6 of the guide.
5. Stop the stopwatch when the 4th green check appears and they tap Finish Setup.
6. Record: total time, any stumbling points, any guide revisions needed.
7. Verify the agent is running: `curl http://<device_ip>:8090/health` from a LAN machine returns `{ok: true, ws_connected: true, ...}`.
8. Verify on comms-link relay: `curl http://localhost:8766/relay/health` shows the new client.

**Repeat for the second device.**

**Then run the self-update test (INSTALL-03):**
9. On Bono VPS, update `manifest.json` to advertise v0.1.1 with a fresh APK.
10. Force a UpdatePoller tick via a debug intent: `adb shell am broadcast -a in.racingpoint.rcagentmobile.DEBUG_FORCE_UPDATE_POLL` (debug-only broadcast receiver, gated on BuildConfig.DEBUG; remove before release).
11. Observe: persistent notification action "Update v0.1.1 available — tap to install" appears on both devices within 60s.
12. Tap the notification → system install dialog → tap Install.
13. Verify: `GET /build_id` now returns v0.1.1's build_id. Install took ≤ 30s plus the user's one-tap confirmation.

#### Acceptance (all four must pass)

- [ ] SC-1 **Printed guide install:** Tester completes full install using only the printed guide, without asking James, in ≤ 5 minutes on both devices.
- [ ] SC-2 **Single-screen checklist:** All 4 items visible on one screen without scrolling on both Tab Plus (large screen) and M07 (smaller phone screen). All 4 turn green with correct taps.
- [ ] SC-3 **User-confirmed self-update:** v0.1.1 installs within 60s of manifest push, via single user confirmation tap.
- [ ] SC-4 **Headless operation:** After FirstRunActivity.finish(), user closes the app; agent continues to run (persistent notification visible); force-stop test still shows auto-restart (regression test of 429-02).

#### Failure mode handling

If any SC fails, file a gap-closure plan (431-0N) within the same phase directory. Do NOT mark Phase 431 complete. Common failure modes to watch for:

- **SC-1 fail:** Screenshots in the guide don't match reality on one of the two devices. Fix: add a conditional step or an OEM-specific addendum page.
- **SC-2 fail:** M07 screen height crops the 4th item below the fold. Fix: reduce item padding or use a scrollable layout; re-test.
- **SC-3 fail:** INSTALL_FAILED_UPDATE_INCOMPATIBLE — signing key mismatch. Fix: ensure keystore consistency per 431-04 task 8.
- **SC-4 fail:** Samsung One UI kills the agent after Finish. Fix: add "add to Never sleeping apps" as an explicit optional checklist item or an addendum in INSTALL-NOTES.md.

#### Artifacts to save in SUMMARY.md

- Stopwatch measurement for Tab Plus (install time)
- Stopwatch measurement for M07 (install time)
- Screenshot of FirstRunActivity with all 4 green
- Screenshot of `/relay/health` showing both devices
- Screenshot of v0.1.1 `/build_id` post self-update
- Tester's verbal feedback transcript (1 paragraph)

#### Checkpoint (human-verify)

User reports PASS/FAIL per SC with numeric measurements and feedback transcript. Resume signal: "all SCs passed, Phase 431 ship approved" or lists gaps.

#### Commit message

```
test(431-06): Phase 431 E2E drill — staff install + checklist + self-update

Tester (Uday or Vishal) completed install using only the printed guide.
Measured times, feedback transcript, screenshots in SUMMARY.md.
All 4 SCs passed. Phase 431 ready to ship.

Covers: full Phase 431 acceptance gate (INSTALL-01, INSTALL-02, INSTALL-03).
```

---

## 6. Risks and pitfalls (Android + OEM + UX)

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **Samsung One UI (M07) vs. Lenovo skin (Tab Plus) settings-menu divergence** | `PermissionIntentLauncher` has per-intent fallback chain + `ActivityNotFoundException` toast fallback. INSTALL-NOTES.md carries the OEM matrix. Printed guide uses the most generic screenshots. |
| R-2 | **Android 13+ POST_NOTIFICATIONS runtime permission** | UI-SPEC decides whether to include as 5th checklist item or request before FirstRunActivity. Default: request before (simpler UX). Detector in 431-03 handles both paths. |
| R-3 | **ACTION_MANAGE_UNKNOWN_APP_SOURCES is per-source on API 26+** | For self-update in 431-04, the source is rc-agent-mobile itself (via FileProvider install intent), which means `packageManager.canRequestPackageInstalls()` must return true for rc-agent-mobile's own package. Detector checks this correctly — Granted = our package can install. If false, self-update prompts via persistent notification "Enable 'install unknown apps' to update RC Agent Mobile" with a deep link to Settings. |
| R-4 | **APK signing key mismatch breaks self-update** | Reuse the debug keystore from 429-01 for all signs in v50.0 (simplification: 2-device fleet, internal tool, no public distribution). Document in INSTALL-NOTES.md + flag for hardening phase. |
| R-5 | **Printed guide screenshots age out as Android evolves** | Regenerate screenshots at every major Android OS update on the devices. QR code on the guide points to the always-current markdown source on GitHub. |
| R-6 | **Staff cannot use MTP file transfer** (device shows up but refuses writes, or Windows doesn't show it) | Guide's Step 1 explicitly says "choose File transfer on the device" — Android defaults to "Charging only" which makes the device invisible in Explorer. If MTP is truly broken on a unit (very rare), fallback: email APK to staff Gmail, download on device, open from Downloads. Document as a tertiary fallback. |
| R-7 | **Self-update downgrade attack** | Manifest's `min_agent_version` refuses installs from versions below it; `version` must be > current for an update to proceed. SHA256 verification prevents tampered APKs. |
| R-8 | **ToS-adjacent: app running without visible UI confuses staff** | Persistent notification from 429-02 is the visual indicator. Phase 432 adds driver status text ("Accepting Zomato orders..."). After first-run, the app ONLY exists in the notification shade — verified in SC-4 of the drill. |
| R-9 | **Uday's handoff model** | The goal of v50.0 is to free Uday's time. 431-06 deliberately puts Uday (or Vishal) as the tester — if THEY can install without James, the system passes. If not, the phase doesn't ship. |
| R-10 | **AI assistant / TalkBack / accessibility collisions** | The Accessibility Service we ENABLE (for automation) is different from the Accessibility Settings the user VISITS in 431-02. UI-SPEC copy must avoid confusion: item 1 says "Enable Accessibility Service" (capital S) with description "Lets RC Agent automate Zomato Partner". The printed guide reinforces this. |
| R-11 | **The TM-T82 tablet-setup history showed developer-options were not unlockable on TB-351FU** | Phase 431 deliberately avoids any ADB-dependent step in the user-facing install path. All install actions are through the Android Files app + Settings UI. Staff never open dev-options. |
| R-12 | **Lenovo's Smart Connect / Lenovo Vantage may pop up during MTP connect** | Step 1 of the guide says "ignore any pop-ups from Lenovo" — they don't block the copy. Observed in TM-T82 setup project. |
| R-13 | **Cloud APK hosting on Bono VPS — credentials + availability** | Manifest + APKs served via nginx on Bono VPS; credentials for upload follow the existing comms-link deploy pattern (no new secrets). Mirror on James .27 covers LAN-fast path AND serves as fallback if Bono VPS is down. |

## 7. Test plan

### Unit tests (JVM, fast, on every build)
- `PermissionChecklistViewModelTest` (431-01, extended in 431-03)
- `PermissionIntentLauncherTest` (431-02)
- `BatteryOptFallbackTest` (431-02)
- `PermissionDetectorTest` (431-03) — 5 detect functions × at least 2 paths each
- `UpdateManifestTest` (431-04)
- `UpdatePollerTest` (431-04) — 4 manifest scenarios
- `ApkDownloaderTest` (431-04) — SHA256 match and mismatch

All run as part of `./gradlew :app:testDebugUnitTest`. Gradle task returns non-zero on any failure.

### Instrumented tests (skip on CI, run before release)
- `FirstRunActivityEspressoTest` — launch Activity, verify 4 items render, tap each, verify stub launcher called.
- `FirstRunFlowTest` — mock-enable each permission programmatically (where possible via Settings.Secure.putString — requires WRITE_SECURE_SETTINGS, only available via adb shell), assert items turn green.

Run via `./gradlew :app:connectedDebugAndroidTest` with a connected device.

### Physical device tests (human-verify)
- 431-01 checkpoint: UI-SPEC review + UI-REVIEW on both devices.
- 431-05 checkpoint: printed-guide readability and accuracy review.
- 431-06 drill: full E2E with Uday or Vishal as tester.

### UI-SPEC and UI-REVIEW (subagent gates)

- `gsd-ui-researcher` runs BEFORE 431-01 code → produces `UI-SPEC-firstrun.md`.
- `gsd-ui-auditor` runs AFTER 431-01 code + BEFORE 431-02 → produces `UI-REVIEW-firstrun.md`.
- Both are committed to `rc-agent-mobile/docs/`.

## 8. Verification gates (per CLAUDE.md)

- **ui-researcher (required):** UI-SPEC-firstrun.md produced before 431-01 execution. **This is the first checklist item in 431-01's tasks.**
- **ui-auditor (required):** UI-REVIEW-firstrun.md produced after 431-01 execution, before 431-02 begins. Accessibility + i18n + OEM variance coverage.
- **nyquist-audit (required):** Permission detection functions + manifest polling + semver comparison are business logic. Run `gsd-nyquist-auditor` on 431-03 and 431-04 deliverables before 431-06.
- **MMA audit (required):** Android install-intent pathway + OEM skin variance + APK signing + PackageInstaller fallback is a cross-system + adversarial surface. CLAUDE.md requires MMA for cross-system bridges. Dual reasoning modes per CLAUDE.md (abstract for architecture, trace-level for intent action values). Run before 431-06. Budget: $5.
- **integration-checker:** Skip this phase; runs at v50.0 milestone ship.
- **codebase-mapper:** Skip this phase; rc-agent-mobile/ already mapped in Phase 429.
- **SEC gate:** `node comms-link/test/security-check.js` must pass after 431-04 adds new relay-hosted manifest endpoint. Verify the manifest is served read-only to the public (intentional — no staff JWT required) AND that the APK URL is SHA256-verified by the agent client (prevents MitM).
- **Deploy Manifest Protocol (DMP):** Captured in frontmatter `deploy:` section. Executor ticks each item; verifier confirms deployed state matches manifest.
- **Backlog gate (CLAUDE.md CGP v4.3):** Phase 431 must reach DEPLOYED-VERIFIED (both devices run self-updated agent + checklist completed by Uday or Vishal) before Phase 432 begins. COMMITTED ≠ SHIPPED.

## 9. Open questions the planner cannot decide

These require a user decision before executing the flagged plans. Listed in execution-blocking order.

**OQ-1 — POST_NOTIFICATIONS placement in the first-run flow (BLOCKS 431-01 UI-SPEC).**
Android 13+ requires a runtime permission grant for notifications. Two options:
- (a) Request `POST_NOTIFICATIONS` via `requestPermissions()` BEFORE FirstRunActivity loads. User sees one system prompt immediately, then the clean 4-item checklist. Simpler UX.
- (b) Add POST_NOTIFICATIONS as a 5th checklist item. Consistent with the other 4 items' pattern but makes the screen taller and possibly scrollable on M07.

**Recommendation:** (a). Document in UI-SPEC. User override at UI-SPEC review.

**OQ-2 — manifest hosting and APK URL authority (BLOCKS 431-04 cloud-side task).**
The agent polls a URL for the manifest. Candidates:
- (a) `https://api.racingpoint.cloud/mobile-agent/manifest.json` — needs the domain, TLS, and nginx config on Bono VPS. Production-ready.
- (b) `https://github.com/racingpoint/rc-agent-mobile/releases/latest/download/manifest.json` — free, versioned, public — but exposes the Kotlin agent publicly. Given this is an internal tool, not recommended.
- (c) A comms-link-relay HTTP endpoint `http://localhost:8766/mobile-agent/manifest` (LAN only) — secure by default but requires the agent to be on LAN to update. Bad for a tablet that might travel outside the venue briefly.

**Recommendation:** (a) + LAN fast-path on James .27:18890. User to confirm the domain is set up and Bono will host.

**OQ-3 — Keystore handling for v50.0 (INHERITED from Phase 429 OQ-4, BLOCKS 431-04 acceptance).**
Self-update requires keystore consistency. Proposed simplification: reuse the debug keystore committed at `rc-agent-mobile/keystores/debug.keystore` for all v50.0 signs. Trade-off: the keystore is NOT secret (it's in git) — acceptable given v50.0 is internal-only and not distributed outside the venue. If the user wants a proper release keystore, that's a pre-431-04 blocker.

**Recommendation:** stick with debug keystore for v50.0. Open a post-v50.0 phase for release keystore + Play Console enrollment if the fleet ever goes external.

**OQ-4 — Does the agent need to uninstall itself on factory reset / explicit uninstall? (LOW-priority, non-blocking)**
If the agent runs an Accessibility Service and the user deletes the app via Settings, the Accessibility Service is automatically removed. No action required from our side. If user re-installs later, they re-complete the first-run checklist. Confirmed as a no-op — surfaced here for closure.

**OQ-5 — "Finish Setup" button text (minor UX call, non-blocking).**
UI-SPEC can decide between: "Finish Setup", "Start RC Agent", "Let's Go", "Done". Default: "Finish Setup" (matches the checklist metaphor, no ambiguity).

**OQ-6 — Update frequency (TUNEABLE, not blocking).**
4-hour polling is the default. Too frequent burns battery; too rare leaves security patches pending. Alternatives: 1h (aggressive), 12h (conservative), on-demand + daily fallback. Default: 4h with a BuildConfig override. User can tune via `local.properties`.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` INSTALL-01, INSTALL-02, INSTALL-03
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Dependent phase:** 429 (scaffold) — 431 consumes MainActivity + AgentForegroundService from it
- **Sibling phase:** 430 (Accessibility Service) — 431's checklist item 1 launches Settings for the service 430 defines
- **Tab Plus hardware context:** `~/.claude/projects/C--Users-bono/memory/project_tm_t82_tablet_setup.md` — documents the TB-351FU's locked-down state and why ADB-dependent install paths are non-viable for staff
- **CLAUDE.md gates:** UI-SPEC + UI-REVIEW for any staff-facing UI; MMA required for cross-system bridges

## 11. Output (at phase close)

At the end of Plan 431-06 (E2E drill pass), create `.planning/phases/431-bootstrap-install-first-run-ux/SUMMARY.md` capturing:
- Which commits implemented each plan (431-01 through 431-06)
- Stopwatch measurements for SC-1 (install time) on both devices
- UI-SPEC and UI-REVIEW document references
- Screenshot evidence: FirstRunActivity all-green, relay /health, self-update build_id change
- Tester feedback transcript (Uday or Vishal)
- OEM-specific quirks encountered (M07 vs. Tab Plus)
- Any OQs resolved during execution (update §9 state)
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked
- Printed guide artifact location (PDF + source MD)
- Handoff to Phase 432 (Driver framework) — what's ready, what's deferred

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 3 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
