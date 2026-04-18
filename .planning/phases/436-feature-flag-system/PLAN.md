---
phase: 436-feature-flag-system
phase_number: 436
milestone: v50.0 rc-agent-mobile
name: "Feature Flag System — server-side store + agent consumer + kill-switch"
status: ready-to-execute
goal: >
  Server-side per-device + per-driver feature flags for the Android fleet, re-using
  the existing v22.0 feature_flags store (SQLite + in-memory cache + WS broadcast).
  Mobile flags live under a dedicated `mobile.*` namespace so they do not collide
  with the existing pod fleet flag space. Toggling `mobile.enable_<driver>_on_<device>`
  fires the driver's install()/uninstall() lifecycle hook on the target device
  within 10s over a delta-push comms-link channel. A global
  `mobile.pause_all_drivers` kill-switch halts every driver on every device within
  10s for ToS incident response, overriding per-driver flags. Every flag change
  audit-logged (actor, timestamp, before/after, target) in a new
  `mobile_flag_audit` table surfaced through the existing admin activity feed.
  NOT IN SCOPE: admin dashboard toggle UI (Phase 442).
requirements: [FLAG-01, FLAG-02, FLAG-03, FLAG-04]
depends_on: [432]   # Driver framework — flag toggles drive LifecycleDispatcher.install()/uninstall().
wave: 5             # Wave 4 = 432 (driver framework). 433-436 are the parallel-after-432 band.
plan_count: 8
plans:
  - 436-01-PLAN: mobile.* namespace extension + mobile_flag_audit DB migration
  - 436-02-PLAN: REST API — GET /api/v1/mobile/flags/:device_id + PUT /api/v1/mobile/flags/:device_id/:flag_key
  - 436-03-PLAN: Comms-link WS flag-delta channel (server → mobile agents)
  - 436-04-PLAN: Agent-side FeatureFlagStore — boot fetch + 5min re-fetch + WS subscribe + apply-on-delta
  - 436-05-PLAN: Lifecycle dispatch — enable_<driver>_on_<device> → install()/uninstall() within 10s
  - 436-06-PLAN: Global kill-switch — pause_all_drivers overrides per-driver flags, halts all drivers within 10s
  - 436-07-PLAN: mobile_flag_audit table surfaced via existing admin activity feed
  - 436-08-PLAN: Integration test — racecontrol + Kotlin agent + mock driver end-to-end (10s propagation)
autonomous: true   # No human-verify checkpoints; all tests are automated (adb shell + curl + log assertions).
files_modified:
  # ── Rust server side ─────────────────────────────────────────────────────
  - crates/racecontrol/src/flags.rs                              # extend with mobile.* namespace handlers
  - crates/racecontrol/src/flags_mobile.rs                       # NEW — mobile-specific GET/PUT + delta builder
  - crates/racecontrol/src/db/migrate_config.rs                  # ALTER: add mobile_flag_audit table
  - crates/racecontrol/src/state.rs                              # new field: mobile_flag_senders (HashMap<device_id, mpsc::Sender>)
  - crates/racecontrol/src/state_methods.rs                      # new: broadcast_mobile_flag_delta() + resolve_for_device()
  - crates/racecontrol/src/api/routes.rs                         # register /api/v1/mobile/flags/* routes (staff JWT gated)
  - crates/racecontrol/src/ws/mobile_flag_sync.rs                # NEW — WS handler for mobile agent flag channel
  - crates/rc-common/src/protocol.rs                             # extend: add MobileFlagDelta + MobilePauseAll variants
  # ── Kotlin agent side ────────────────────────────────────────────────────
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/flags/FeatureFlagStore.kt      # NEW
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/flags/FlagFetcher.kt           # NEW — HTTP GET boot + 5min
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/flags/FlagDeltaHandler.kt      # NEW — WS delta consumer
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/flags/LifecycleDispatcher.kt   # NEW — fires install()/uninstall() within 10s
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/flags/KillSwitchGate.kt        # NEW — pause_all_drivers override
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/comms/CommsLinkClient.kt      # MODIFY — route mobile_flag_delta to FlagDeltaHandler
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/service/AgentForegroundService.kt  # MODIFY — wire FeatureFlagStore
  - rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/driver/DriverRegistry.kt     # MODIFY (from 432) — consult FeatureFlagStore before dispatch
  # ── Docs / protocol / tests ──────────────────────────────────────────────
  - rc-agent-mobile/docs/PROTOCOL.md                             # extend: mobile_flag_delta + mobile_pause_all message types
  - comms-link/shared/agent-protocol-v1.md                       # parity copy
  - crates/racecontrol/src/flags_mobile_tests.rs                 # NEW — unit tests for namespace routing + resolve_for_device
  - crates/racecontrol/src/ws/mobile_flag_sync_tests.rs          # NEW — unit tests for WS delta broadcast
  - rc-agent-mobile/app/src/test/kotlin/.../flags/FeatureFlagStoreTest.kt
  - rc-agent-mobile/app/src/test/kotlin/.../flags/LifecycleDispatcherTest.kt
  - rc-agent-mobile/app/src/test/kotlin/.../flags/KillSwitchGateTest.kt
  - tests/integration/mobile-flag-e2e.sh                         # NEW — 10s propagation end-to-end drill
  - .planning/phases/436-feature-flag-system/SUMMARY.md          # filled at end

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [racecontrol]     # Server-side namespace + REST + WS changes
  frontend_rebuild: [none]       # Admin UI is Phase 442 — out of scope
  config_change: none            # No racecontrol.toml changes (flags live in DB)
  db_migration: "mobile_flag_audit (id, actor, target_device, target_driver, flag_key, old_value, new_value, created_at)"
  infrastructure: none           # Re-uses existing comms-link relays (no new listeners, no new PSKs)
  data_files: >
    C:\RacingPoint\mobile-flags-cache.json (created at runtime on each Android device — stores last-known
    flag map for boot-before-server-up case). No file authored at deploy time — agent writes it at first sync.
  bat_file: none
  cloud_parity:
    - Bono VPS racecontrol binary must be redeployed (same build as server .23) — mobile_flag_audit migration runs on cloud DB too.
    - Cloud comms-link relay must accept mobile_flag_delta + mobile_pause_all envelope types (protocol additive — existing filter-by-type code auto-accepts if "ignore unknown" parity was preserved in phase 429-04, which it was).
  targets:
    - server      # 192.168.31.23 (racecontrol binary swap + DB migration)
    - bono_vps    # 100.70.177.44 (racecontrol binary + DB migration parity)
    - tab_plus    # rc-agent-mobile APK rebuild + ADB install
    - m07         # rc-agent-mobile APK rebuild + ADB install
  rollback:
    - "Server: revert to previous racecontrol.exe via renaming racecontrol-prev.exe (72hr window)."
    - "Android: adb install -r /sdcard/Download/rc-agent-mobile-prev.apk on each device."
    - "DB: mobile_flag_audit is additive — safe to leave table in place even if binary rolls back. No destructive schema change."
    - "If kill-switch misbehaves in production: manually set mobile.pause_all_drivers=false via curl PUT with staff JWT; agents drain within 10s."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip            # No frontend in this phase — admin toggle UI is Phase 442.
  ui_auditor: skip               # Same.
  nyquist_auditor: required      # Business logic: namespace routing, delta builder, kill-switch priority, audit log write path.
  mma_audit: required            # Kill-switch is safety-critical (ToS incident response). Cross-system bridge: Rust server ↔ Kotlin agent ↔ driver framework. Dual reasoning modes REQUIRED (abstract for flag priority correctness + trace-level for "what value does FeatureFlagStore.get() return at line X when delta arrives mid-lifecycle-dispatch?").
  integration_checker: required  # Cross-phase: 432 (driver framework) ↔ 436 (flag toggle dispatch) ↔ future 442 (admin UI). Integration check before milestone ship.
  codebase_mapper: skip          # Not a new top-level module — extends existing rc-agent-mobile + racecontrol modules.

risks_summary:
  - "Flag consistency during network partition: if WS drops after PUT but before delta arrives at device, the 5min periodic re-fetch (BOOT-02 parity) is the safety net. Worst-case stale window: 5min, not infinite."
  - "Race between boot-fetch and WS-delta: if agent fetches v=7 via HTTP and then receives delta v=6 via WS, we MUST drop the stale delta. FeatureFlagStore enforces monotonic version (drop delta where version <= current)."
  - "Kill-switch ordering across 2 devices: broadcast is fan-out, no ACK-before-proceed on server — if one device is offline the kill-switch is NOT applied to it until reconnect + re-fetch. Acceptable because offline = not running drivers = not acting on customer data. Documented in ToS playbook (Phase 443)."
  - "Driver uninstall() blocking for >10s: lifecycle hooks MUST be coroutine-cancellable with a 10s deadline. If uninstall() exceeds deadline, KillSwitchGate kills the driver's CoroutineScope forcefully (already isolated per Phase 432). State persistence is the driver's responsibility; kill-switch is fast-halt, not graceful-drain."
  - "Audit log under flood: 1000 rapid flag toggles = 1000 rows + 1000 activity feed entries. Mitigation: no rate limit in v1 (staff-only endpoint, JWT-gated), but add a `warn` log if >10 toggles/min from same actor."
  - "DB migration on cloud (Bono VPS): mobile_flag_audit is additive (CREATE TABLE IF NOT EXISTS + no ALTER to existing tables), safe to roll out ahead of binary swap window."
  - "Android agent receives delta before driver is registered: if FeatureFlagStore.apply(delta) runs before DriverRegistry has loaded manifests, the toggle has no target to fire on. Mitigation: FeatureFlagStore buffers deltas in a replay queue until DriverRegistry signals ready; LifecycleDispatcher drains the buffer on ready."

open_questions:
  - id: OQ-1
    question: "Should flags be per-device (rcm-tab-plus, rcm-m07) OR per-device-type (tablet, phone)?"
    resolution: "Per-device (exact device_id) for v1. CAPREG-04 (multi-device-type readiness) is future-proofing — mobile_flag_audit.target_device is a free-form string, so switching to device_type later requires no schema migration. Picked per-device because: (a) Tab Plus might run HyperPure + cardboard while M07 only runs Zomato (per REQUIREMENTS-v50.md Capability Registry), so device-scoped flags are already required; (b) device_type collapsing can be added later as a fallback-resolver in resolve_for_device() without breaking existing entries."
  - id: OQ-2
    question: "What is the canonical flag key format?"
    resolution: "`mobile.enable_<driver_id>_on_<device_id>` for per-driver toggles. `mobile.pause_all_drivers` for global. `mobile.enable_zomato_on_rcm_tab_plus` NOT `mobile.enable_zomato_on_tab_plus` — use the registration device_id verbatim (from Phase 429 DeviceState.deviceId) to avoid a separate lookup table. Validator in flags_mobile.rs rejects keys not matching `^mobile\\.(pause_all_drivers|enable_[a-z0-9_]+_on_[a-z0-9_]+)$`."
  - id: OQ-3
    question: "Does PUT return after DB write, or after delta broadcast acked by at least one device?"
    resolution: "Return after DB write + broadcast dispatched (fire-and-forget). Do NOT block on device ACK. Reason: if a device is offline, PUT would hang indefinitely. The 5min periodic re-fetch self-heals on reconnect. Admin UI (Phase 442) will show per-device ACK status separately via the mobile_flag_audit.pods_acked column (parity with existing config_audit_log.pods_acked pattern)."
  - id: OQ-4
    question: "How does the kill-switch interact with driver health checks (Phase 432 healthCheck@5min)?"
    resolution: "KillSwitchGate short-circuits BEFORE healthCheck dispatch. Any driver whose CoroutineScope was killed by the kill-switch is marked `paused=true` in DriverRegistry state; healthCheck skips paused drivers. Resume path: when pause_all_drivers flips back to false, LifecycleDispatcher re-invokes install() on each previously-running driver (derived from DriverRegistry's `was_running_before_pause` snapshot taken at pause time)."
  - id: OQ-5
    question: "Do we persist the kill-switch state on the device for reboot resilience?"
    resolution: "Yes — write mobile-flags-cache.json on every apply (existing pattern from v22.0 rc-agent feature_flags.rs). On agent boot, FeatureFlagStore.load_from_cache() is consulted BEFORE FlagFetcher fetches from server. If server is unreachable at boot and cache says pause_all_drivers=true, drivers do NOT start. This is the correct fail-safe: an offline agent cannot know the kill-switch has been lifted, so it stays halted until a successful sync."
---

# Phase 436 — Feature Flag System

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 436 |
| Name | Feature Flag System — server-side store + agent consumer + kill-switch |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | FLAG-01, FLAG-02, FLAG-03, FLAG-04 |
| Dependencies | Phase 432 (driver framework — LifecycleDispatcher uses Phase 432's install()/uninstall() hooks) |
| Wave | 5 |
| Status | Ready to execute |
| Autonomous | Yes — no human-verify checkpoints (automated curl + adb + log assertions) |
| Ship test | Toggle `mobile.enable_zomato_on_rcm_tab_plus=true` via curl PUT → within 10s, Tab Plus driver's install() invoked (verified by UI notification + log entry); Toggle `mobile.pause_all_drivers=true` → within 10s, ALL drivers on BOTH devices halted (uninstall() invoked or CoroutineScope cancelled); mobile_flag_audit shows row with actor + before/after per change |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 8)

1. **Toggle-to-install propagation ≤ 10s:** Toggling `mobile.enable_<driver>_on_<device>=true` fires the driver's `install()` hook within 10 seconds of the PUT request completing on the server. Toggling to `false` fires `uninstall()` within the same window.
2. **Kill-switch ≤ 10s fleet-wide:** Setting `mobile.pause_all_drivers=true` halts every driver on every connected device within 10 seconds. Per-driver flags are overridden while this is active.
3. **Audit trail:** Every flag change is persisted in `mobile_flag_audit` with actor (staff JWT subject), timestamp (ISO-8601), target_device, target_driver (nullable for global), old_value, new_value. Surfaced via the existing admin activity feed endpoint.

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)

- **T-1:** Staff issues `curl -X PUT -H "Authorization: Bearer $STAFF_JWT" http://192.168.31.23:8080/api/v1/mobile/flags/rcm-tab-plus/enable_zomato` with body `{"enabled": true}` → HTTP 200 with the new flag row. (FLAG-01, FLAG-02)
- **T-2:** Within 10s of T-1, `adb logcat | grep 'Driver install: zomato'` on Tab Plus shows the lifecycle fire. (FLAG-03)
- **T-3:** Within 10s of T-1, Tab Plus persistent notification body updates to include "zomato" in the active-drivers list (wired via Phase 429's `AgentForegroundService.updateNotification`). (FLAG-03)
- **T-4:** Toggling `mobile.pause_all_drivers=true` → within 10s, `adb logcat | grep 'KillSwitchGate: halting'` fires on BOTH devices; all running drivers' CoroutineScopes cancelled; `/capability` endpoint on each device reports `active_drivers: []`. (FLAG-04)
- **T-5:** After T-4, flipping `mobile.pause_all_drivers=false` triggers `install()` on all drivers that were running before pause (derived from DriverRegistry's snapshot). No manual intervention. (FLAG-04)
- **T-6:** `SELECT * FROM mobile_flag_audit ORDER BY id DESC LIMIT 5` shows rows with actor, target_device, flag_key, old_value, new_value, created_at for each PUT above. (FLAG-02, audit requirement)
- **T-7:** If an Android device is offline during PUT, it receives the change within 5min of reconnect via the HTTP GET periodic re-fetch (BOOT-02 parity). Cache on disk ensures reboot resilience with last-known state.
- **T-8:** Protocol version negotiation (from Phase 429) intact: if server sends a `mobile_flag_delta` with `protocol_version: 2`, agent logs WARN and keeps connection alive; no crash.

### Required artifacts (files that must exist, with minimum behavior)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `crates/racecontrol/src/flags_mobile.rs` | Mobile namespace handlers | 200 | `get_mobile_flags_for_device()`, `put_mobile_flag()`, `validate_mobile_flag_key()`, `resolve_for_device()` |
| `crates/racecontrol/src/ws/mobile_flag_sync.rs` | WS delta broadcast | 120 | `broadcast_mobile_flag_delta()`, per-device mpsc sender registry |
| `crates/racecontrol/src/db/migrate_config.rs` | `mobile_flag_audit` table | +30 | `CREATE TABLE IF NOT EXISTS mobile_flag_audit (id, actor, target_device, target_driver, flag_key, old_value, new_value, created_at)` |
| `crates/rc-common/src/protocol.rs` | Protocol envelope additions | +40 | `CoreToAgentMessage::MobileFlagDelta(MobileFlagDeltaPayload)`, `CoreToAgentMessage::MobilePauseAll(bool)` |
| `rc-agent-mobile/app/.../flags/FeatureFlagStore.kt` | Agent-side flag store | 150 | `apply_delta()`, `load_from_cache()`, `get(key): Boolean`, monotonic version check |
| `rc-agent-mobile/app/.../flags/FlagFetcher.kt` | Boot + 5min HTTP fetch | 80 | `fetch_all_for_device()`, retry w/ exp backoff, applies via `FeatureFlagStore.apply_bulk()` |
| `rc-agent-mobile/app/.../flags/FlagDeltaHandler.kt` | WS delta consumer | 60 | Receives `MobileFlagDelta` + `MobilePauseAll` envelopes, calls `FeatureFlagStore.apply_delta()` |
| `rc-agent-mobile/app/.../flags/LifecycleDispatcher.kt` | Fires install()/uninstall() on flag change | 130 | Listens to `FeatureFlagStore.change_flow`, debounces rapid toggles (100ms), dispatches to `DriverRegistry` within 10s budget |
| `rc-agent-mobile/app/.../flags/KillSwitchGate.kt` | pause_all_drivers override | 80 | Intercepts ALL lifecycle dispatches; when active, cancels all driver CoroutineScopes; on resume, replays install() |
| `rc-agent-mobile/docs/PROTOCOL.md` | Protocol spec extension | +80 | `mobile_flag_delta` + `mobile_pause_all` message schemas, bi-directional flow diagrams |
| `tests/integration/mobile-flag-e2e.sh` | 10s propagation drill | 150 | Starts racecontrol+mock agent, PUT flag, measure Δt from PUT→install(), assert ≤ 10s |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| PUT /api/v1/mobile/flags/:dev/:key → flags_mobile.rs::put_mobile_flag | mobile_flag_audit INSERT | sqlx query | grep `INSERT INTO mobile_flag_audit` in `flags_mobile.rs` |
| flags_mobile.rs::put_mobile_flag (end) | broadcast_mobile_flag_delta() | Rust call | grep `broadcast_mobile_flag_delta` in `flags_mobile.rs` |
| state_methods.rs::broadcast_mobile_flag_delta | mobile_flag_senders.iter() → mpsc::send | Rust call | grep `mobile_flag_senders.read().await` in `state_methods.rs` |
| ws/mobile_flag_sync.rs::on_agent_register | mobile_flag_senders.insert(device_id, sender) | Rust call | grep `mobile_flag_senders.write().await` in `ws/mobile_flag_sync.rs` |
| CommsLinkClient.onMessage(mobile_flag_delta) | FlagDeltaHandler.handle(delta) | Kotlin call | grep `FlagDeltaHandler.handle` in `CommsLinkClient.kt` |
| FeatureFlagStore.apply_delta(delta) | change_flow.emit(key) | Kotlin Flow | grep `change_flow.emit` in `FeatureFlagStore.kt` |
| LifecycleDispatcher (subscribes to change_flow) | DriverRegistry.driver(id).install()/uninstall() | Kotlin call | grep `.install(` or `.uninstall(` in `LifecycleDispatcher.kt` |
| KillSwitchGate.onKillSwitchActive | Cancel ALL driver CoroutineScopes | Kotlin call | grep `scope.cancel` in `KillSwitchGate.kt` |
| FeatureFlagStore.apply_delta | persist to mobile-flags-cache.json | File write | grep `writeText` or `FileOutputStream` in `FeatureFlagStore.kt` |
| AgentForegroundService.onCreate | FlagFetcher.start() + FlagFetcher.scheduleRefetch(5min) | Kotlin call | grep `FlagFetcher` in `AgentForegroundService.kt` |
| mobile_flag_audit row → admin activity feed | existing activity_log::log_pod_activity() reader | SQL JOIN or UNION | grep `mobile_flag_audit` in admin activity endpoint |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md   # Protocol envelope + CommsLinkClient pattern
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md  # DriverRegistry + LifecycleDispatcher contract
@./crates/racecontrol/src/flags.rs                                # Existing v22.0 feature_flags store — REUSE, don't rebuild
@./crates/racecontrol/src/state_methods.rs                        # broadcast_flag_sync() reference implementation
@./crates/racecontrol/src/db/migrate_config.rs                    # feature_flags + config_audit_log CREATE TABLE — copy migration pattern
@./crates/rc-common/src/protocol.rs                               # CoreToAgentMessage enum — MobileFlagDelta variant goes here
@./crates/rc-agent/src/feature_flags.rs                           # Pod-side consumer with 5min re-fetch — MIRROR pattern in Kotlin

### Interfaces executors will need

The existing racecontrol feature_flags infrastructure ALREADY provides:
- `feature_flags` SQLite table with schema `(name, enabled, default_value, overrides, version, updated_at)`
- `state.feature_flags: Arc<RwLock<HashMap<String, FeatureFlagRow>>>` in-memory cache
- `state.broadcast_flag_sync()` WS broadcast method (sends to pod WS connections only — we are ADDING a mobile variant, not replacing)
- `config_audit_log` table with schema `(action, entity_type, entity_name, old_value, new_value, pushed_by, pods_acked, created_at, seq_num)`

We are REUSING the `feature_flags` table (new rows with `mobile.*` prefix) and ADDING:
- `mobile_flag_audit` table (mobile-specific audit trail — keeps `config_audit_log` pod-focused)
- `state.mobile_flag_senders: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<CoreMessage>>>>` — per-device mobile WS senders, parallel to existing `agent_senders` (which is pod-scoped)
- `broadcast_mobile_flag_delta(device_id, flag_key, new_value)` — fan-out to a specific device (targeted) OR all mobile devices (for `pause_all_drivers`)

Key JSON envelope (extends comms-link's v:1 envelope from Phase 429-04, consistent with its protocol_version negotiation):

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "mobile_flag_delta",
  "from": "racecontrol",
  "to": "rcm-tab-plus",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "flag_key": "mobile.enable_zomato_on_rcm_tab_plus",
    "old_value": false,
    "new_value": true,
    "version": 42,
    "actor": "uday@racingpoint.in"
  }
}
```

Global kill-switch envelope:

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "mobile_pause_all",
  "from": "racecontrol",
  "to": "*",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "active": true,
    "reason": "ToS incident: Zomato account warning",
    "actor": "uday@racingpoint.in",
    "version": 43
  }
}
```

## 5. Atomic plan breakdown (8 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion.

---

### 436-01-PLAN — mobile.* namespace extension + mobile_flag_audit DB migration

**Goal:** Extend existing `feature_flags` table usage to accept `mobile.*` prefixed keys, add `mobile_flag_audit` table, and expose a `validate_mobile_flag_key()` helper.

**Covers:** FLAG-01 (namespace), FLAG-02 (audit foundation)

**Dependencies:** none (within this phase) — relies on existing v22.0 feature_flags infra.

**Type:** `auto`

#### Tasks

1. Create `crates/racecontrol/src/flags_mobile.rs`:
   - `validate_mobile_flag_key(key: &str) -> Result<(), String>` — regex `^mobile\.(pause_all_drivers|enable_[a-z0-9_]+_on_[a-z0-9_]+)$`. Rejects any other shape. Tested with 6 valid + 6 invalid inputs.
   - `parse_mobile_flag_key(key: &str) -> MobileFlagRef` — struct `{ global: bool, driver_id: Option<String>, device_id: Option<String> }`.
   - `resolve_for_device(state, device_id) -> HashMap<String, bool>` — reads all `mobile.*` flags from `state.feature_flags` cache, filters to those targeting `device_id` (driver scope) + globals. Returns the effective map the agent sees.

2. Extend `crates/racecontrol/src/db/migrate_config.rs`:
   ```sql
   CREATE TABLE IF NOT EXISTS mobile_flag_audit (
       id            INTEGER PRIMARY KEY AUTOINCREMENT,
       actor         TEXT NOT NULL,
       target_device TEXT NOT NULL,
       target_driver TEXT,                                      -- nullable for pause_all_drivers
       flag_key      TEXT NOT NULL,
       old_value     TEXT,                                      -- "true"/"false"/NULL (first write)
       new_value     TEXT NOT NULL,                             -- "true"/"false"
       version       INTEGER NOT NULL,
       created_at    TEXT NOT NULL DEFAULT (datetime('now'))
   );
   CREATE INDEX IF NOT EXISTS idx_mobile_flag_audit_device_created
       ON mobile_flag_audit(target_device, created_at DESC);
   CREATE INDEX IF NOT EXISTS idx_mobile_flag_audit_flag_key_created
       ON mobile_flag_audit(flag_key, created_at DESC);
   ```
   Place inside existing `migrate_config()` function after the `feature_flags` block (idempotent — CREATE TABLE IF NOT EXISTS is safe on re-run).

3. Add unit tests in `flags_mobile_tests.rs`:
   - `validate_mobile_flag_key_accepts_valid_shapes()` — 6 valid cases.
   - `validate_mobile_flag_key_rejects_invalid()` — 6 invalid (missing prefix, uppercase, hyphen, no driver_id, no device_id, empty).
   - `resolve_for_device_returns_globals_and_device_scoped()` — seed cache with 4 flags (1 global, 2 matching device, 1 matching different device); assert 3 returned.
   - `mobile_flag_audit_migration_creates_table_and_indexes()` — open in-memory sqlx pool, run migration, assert `SELECT count(*) FROM sqlite_master WHERE name='mobile_flag_audit'` = 1.

4. Wire `flags_mobile` module into `lib.rs` and `main.rs`.

#### Acceptance

- `cargo test -p racecontrol-crate flags_mobile_tests` passes (all 4 tests).
- `cargo test -p racecontrol-crate db::tests::migrations_are_idempotent` still passes (no regression on existing schema).
- `grep "validate_mobile_flag_key" crates/racecontrol/src/flags_mobile.rs` shows at least 1 match.
- `PRAGMA table_info(mobile_flag_audit);` on a fresh DB shows 9 columns.

#### G4 NOT TESTED list (carry into commit)

- No endpoint exposed yet (plan 436-02).
- No WS broadcast (plan 436-03).
- No Android consumer (plans 436-04 through 436-06).
- Cloud DB migration not applied (deploy step — separate from test).

#### Commit message

```
feat(436-01): mobile.* flag namespace validator + mobile_flag_audit table

Adds crates/racecontrol/src/flags_mobile.rs with key validator, parser, and
per-device resolver reusing the existing v22.0 feature_flags SQLite store.
New mobile_flag_audit table (additive, IF NOT EXISTS) with (device, created_at)
and (flag_key, created_at) indexes for admin activity feed queries.

Covers: FLAG-01 (namespace), FLAG-02 (audit foundation)
Not tested: endpoint exposure (436-02), WS broadcast (436-03), Android consumer
(436-04..06). DB migration runs on racecontrol boot — no manual step.
```

---

### 436-02-PLAN — REST API: GET/PUT /api/v1/mobile/flags/:device_id[/:flag_key]

**Goal:** Staff-JWT-gated endpoints to read the resolved flag map for a device and toggle a specific flag. PUT writes to `feature_flags` table (reusing v22.0 path), writes `mobile_flag_audit`, and fires broadcast (wired in 436-03).

**Covers:** FLAG-01, FLAG-02 (audit write)

**Dependencies:** 436-01

**Type:** `auto`

#### Tasks

1. In `crates/racecontrol/src/flags_mobile.rs`, add:

   ```rust
   // GET /api/v1/mobile/flags/:device_id
   pub async fn get_mobile_flags_for_device(
       State(state): State<Arc<AppState>>,
       Extension(_claims): Extension<StaffClaims>,
       Path(device_id): Path<String>,
   ) -> Result<Json<MobileFlagMapResponse>, (StatusCode, Json<serde_json::Value>)>;

   // PUT /api/v1/mobile/flags/:device_id/:flag_key
   pub async fn put_mobile_flag(
       State(state): State<Arc<AppState>>,
       Extension(claims): Extension<StaffClaims>,
       Path((device_id, flag_key)): Path<(String, String)>,
       Json(body): Json<MobileFlagUpdateRequest>,   // { enabled: bool }
   ) -> Result<Json<MobileFlagRow>, (StatusCode, Json<serde_json::Value>)>;
   ```

   Implementation flow for `put_mobile_flag`:
   - Construct full flag key: `format!("mobile.{}_on_{}", flag_key, device_id)` for per-driver; `"mobile.pause_all_drivers"` if `flag_key == "pause_all_drivers"` and `device_id == "*"`.
   - `validate_mobile_flag_key(&full_key)?` — reject malformed.
   - Read old value from `state.feature_flags` cache (may be None on first write).
   - INSERT OR UPDATE into `feature_flags` (same SQL as existing `update_flag` path; REUSE by factoring a helper in `flags.rs::upsert_flag_row()`).
   - INSERT into `mobile_flag_audit` with `{actor: claims.sub, target_device: device_id, target_driver: parsed.driver_id, flag_key: full_key, old_value, new_value, version}`.
   - Update in-memory cache.
   - Call `state.broadcast_mobile_flag_delta(&device_id, &full_key, new_value).await` — STUB for now (returns `()`, no-op; plan 436-03 implements the real broadcast). This is deliberate: the endpoint is fully functional against the DB/cache in plan 436-02, and 436-03 wires the delta path without changing this endpoint.
   - Return the new flag row.

2. Register routes in `crates/racecontrol/src/api/routes.rs`:
   ```rust
   .route("/api/v1/mobile/flags/:device_id",
          get(flags_mobile::get_mobile_flags_for_device))
   .route("/api/v1/mobile/flags/:device_id/:flag_key",
          put(flags_mobile::put_mobile_flag))
   ```
   Both routes MUST go behind the existing staff JWT middleware (same layer as `/api/v1/flags`). NEVER add to `public_routes` — this is a staff-only write path (CLAUDE.md Security section).

3. Route-uniqueness sanity check (CLAUDE.md standing rule):
   ```bash
   grep -n '\.route("/' crates/racecontrol/src/api/routes.rs | sed 's/.*\.route("//' | sed 's/".*//' | sort | uniq -d
   # must print nothing
   ```

4. Integration test in `crates/racecontrol/src/flags_mobile_tests.rs`:
   - Spin up Axum test server with in-memory DB.
   - Forge a staff JWT (reuse existing test helper from `auth/middleware.rs` tests).
   - PUT `/api/v1/mobile/flags/rcm-tab-plus/enable_zomato` with `{"enabled": true}` → expect 200, assert response row has `name="mobile.enable_zomato_on_rcm_tab_plus"`, `enabled=true`.
   - SELECT from `mobile_flag_audit` → assert 1 row with correct actor + device + flag_key + new_value.
   - GET `/api/v1/mobile/flags/rcm-tab-plus` → assert response includes the toggle.
   - PUT without JWT → assert 401.
   - PUT with malformed key (e.g. `ENABLE_UPPERCASE`) → assert 400.

#### Acceptance

- `cargo test -p racecontrol-crate flags_mobile_tests::put_mobile_flag_writes_audit_and_broadcasts` passes.
- `cargo test -p racecontrol-crate flags_mobile_tests::get_rejects_no_jwt` passes.
- Manual smoke: run a local racecontrol, `curl -X PUT -H "Authorization: Bearer $JWT" .../mobile/flags/rcm-tab-plus/enable_zomato -d '{"enabled":true}'` returns 200.
- `grep -n "/api/v1/mobile/flags" crates/racecontrol/src/api/routes.rs | wc -l` returns ≥ 2.
- Route uniqueness script above prints nothing.

#### G4 NOT TESTED list

- WS delta is a no-op stub (436-03 implements).
- Android consumer (436-04) — endpoint works from curl, not from agent yet.
- End-to-end 10s propagation (436-08 drill).

#### Commit message

```
feat(436-02): GET/PUT /api/v1/mobile/flags/:device_id[/:flag_key] staff JWT

Reuses v22.0 feature_flags table under mobile.* namespace. PUT writes DB,
updates in-memory cache, writes mobile_flag_audit row with staff actor.
Broadcast is a no-op stub awaiting 436-03.

Covers: FLAG-01, FLAG-02 (audit write path)
Not tested: WS delta (436-03), Android consumer (436-04+).
```

---

### 436-03-PLAN — Comms-link WS flag-delta channel (server → mobile agents)

**Goal:** Replace the stub `broadcast_mobile_flag_delta()` from 436-02 with a real WS broadcast that delivers `MobileFlagDelta` envelopes to the affected device(s) via the comms-link relay. Mobile agents register as distinct identities in a new per-device sender registry.

**Covers:** FLAG-01 (delta push), FLAG-03 (fire install()/uninstall() within 10s — server-side half: ensure delta delivered in <1s)

**Dependencies:** 436-02

**Type:** `auto`

#### Tasks

1. Extend `crates/rc-common/src/protocol.rs`:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MobileFlagDeltaPayload {
       pub flag_key: String,
       pub old_value: Option<bool>,
       pub new_value: bool,
       pub version: u64,
       pub actor: String,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MobilePauseAllPayload {
       pub active: bool,
       pub reason: Option<String>,
       pub actor: String,
       pub version: u64,
   }

   // Extend CoreToAgentMessage enum:
   //   MobileFlagDelta(MobileFlagDeltaPayload),
   //   MobilePauseAll(MobilePauseAllPayload),
   ```
   Bump `protocol_version` MINOR in docs but KEEP `protocol_version = 1` in the wire payload (backward-compatible additive change per Phase 429-04's "ignore unknown fields" rule).

2. Extend `AppState` in `crates/racecontrol/src/state.rs`:
   ```rust
   pub mobile_flag_senders: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<CoreMessage>>>>,
   ```
   Populated by the comms-link relay WS handler when a mobile agent registers (distinguish mobile from pod by `device_id` prefix `rcm-`).

3. Create `crates/racecontrol/src/ws/mobile_flag_sync.rs`:
   - `on_mobile_agent_register(state, device_id, sender)` — insert into `mobile_flag_senders`.
   - `on_mobile_agent_disconnect(state, device_id)` — remove.
   - On register, immediately send the full current flag snapshot for the device (SAME pattern as existing pod `/ws/sync` first-message) so the agent self-heals even if it missed deltas during disconnect.

4. Implement `crates/racecontrol/src/state_methods.rs::broadcast_mobile_flag_delta()`:
   ```rust
   pub async fn broadcast_mobile_flag_delta(
       &self,
       target_device: &str,           // exact device_id OR "*" for all mobile
       flag_key: &str,
       old_value: Option<bool>,
       new_value: bool,
       actor: &str,
   ) {
       // If flag_key is "mobile.pause_all_drivers" → emit MobilePauseAll to ALL mobile agents
       // Else → emit MobileFlagDelta to target_device only (exact match)
       let senders = self.mobile_flag_senders.read().await;
       let targets: Vec<_> = if target_device == "*" {
           senders.iter().collect()
       } else {
           senders.iter().filter(|(k, _)| k.as_str() == target_device).collect()
       };
       // Snapshot then release lock BEFORE awaiting (CLAUDE.md lock rule)
       let snapshot: Vec<_> = targets.iter().map(|(k, s)| (k.to_string(), s.clone())).collect();
       drop(senders);
       for (dev, sender) in snapshot {
           let msg = build_envelope(/* ... */);
           if let Err(e) = sender.send(msg) {
               tracing::warn!("MobileFlagDelta to {} failed: {}", dev, e);
           }
       }
   }
   ```
   CRITICAL: snapshot-then-drop pattern per CLAUDE.md `Never hold a lock across .await` rule.

5. Wire into `flags_mobile.rs::put_mobile_flag()`: replace the stub call with the real `broadcast_mobile_flag_delta(&target, &full_key, old_value, new_value, &claims.sub).await`. For `pause_all_drivers`, `target = "*"`.

6. Extend `rc-agent-mobile/docs/PROTOCOL.md` with the two new message types + sequence diagram (server → agent → driver).

7. Unit tests in `mobile_flag_sync_tests.rs`:
   - `broadcast_to_specific_device_does_not_leak_to_others()` — register 2 mock senders, broadcast targeted at one, assert only that mpsc received.
   - `broadcast_pause_all_reaches_every_mobile_sender()` — register 3 mock senders, broadcast with target `*`, assert all 3 received.
   - `broadcast_drops_sender_on_send_err_and_continues()` — close one mpsc, broadcast, assert remaining receive, no panic.
   - `on_register_sends_initial_snapshot()` — seed cache with 3 `mobile.*` flags for device, register sender, assert first message is a bulk snapshot containing all 3.

#### Acceptance

- `cargo test -p racecontrol-crate ws::mobile_flag_sync_tests` passes (all 4).
- `cargo test -p rc-common protocol::mobile_payloads_serialize_roundtrip` passes.
- Protocol doc `rc-agent-mobile/docs/PROTOCOL.md` has sections for `mobile_flag_delta` and `mobile_pause_all` with JSON examples (≥ 50 added lines).
- `grep -n "senders.read().await" crates/racecontrol/src/state_methods.rs | grep -v "let .* = senders"` returns nothing across `.await` in that function (lock-drop rule).

#### G4 NOT TESTED list

- Android client does not yet consume the messages (436-04).
- End-to-end live delivery (436-08 drill).

#### Commit message

```
feat(436-03): WS MobileFlagDelta + MobilePauseAll broadcast channel

Adds mobile_flag_senders registry in AppState, broadcast_mobile_flag_delta()
with snapshot-and-drop pattern (CLAUDE.md lock rule). Initial snapshot sent
on mobile agent register for disconnect self-heal. Protocol additive —
protocol_version stays at 1, new message types surface via existing
CoreToAgentMessage enum.

Covers: FLAG-01, FLAG-03 (server-side half: <1s delta dispatch)
Not tested: Android consumer (436-04), end-to-end (436-08).
```

---

### 436-04-PLAN — Agent-side FeatureFlagStore + FlagFetcher + FlagDeltaHandler

**Goal:** Kotlin agent-side equivalent of `crates/rc-agent/src/feature_flags.rs`. At boot: HTTP GET the flag snapshot from server. Every 5min: re-fetch (BOOT-02 rule). WS: subscribe to deltas, apply incrementally. Cache to disk for reboot resilience.

**Covers:** FLAG-01 (agent-side consumer), FLAG-03 (delta apply within <1s of arrival)

**Dependencies:** 436-03 (server broadcasts something) — but in practice this plan can develop against a mock server; the live integration lands in 436-08.

**Type:** `auto`

#### Tasks

1. Create `rc-agent-mobile/app/src/main/kotlin/in/racingpoint/rcagentmobile/flags/FeatureFlagStore.kt`:

   ```kotlin
   class FeatureFlagStore(
       private val cacheFile: File,
       private val json: Json = Json { ignoreUnknownKeys = true },
   ) {
       private val _state = MutableStateFlow(FlagsSnapshot(emptyMap(), version = 0L))
       val state: StateFlow<FlagsSnapshot> = _state.asStateFlow()
       val changeFlow: SharedFlow<FlagChangeEvent> = MutableSharedFlow(extraBufferCapacity = 64)

       fun get(key: String): Boolean {
           val snap = _state.value
           // Kill-switch priority: pause_all_drivers = true short-circuits to false
           if (snap.flags["mobile.pause_all_drivers"] == true && key.startsWith("mobile.enable_")) {
               return false
           }
           return snap.flags[key] ?: false  // default false for missing flags (deny-by-default for mobile)
       }

       @Synchronized
       fun applyDelta(delta: MobileFlagDeltaPayload): Boolean {
           val snap = _state.value
           if (delta.version <= snap.version) {
               Log.w(TAG, "Dropping stale delta: incoming=${delta.version}, current=${snap.version}")
               return false
           }
           val newFlags = snap.flags.toMutableMap()
           val old = newFlags[delta.flagKey]
           newFlags[delta.flagKey] = delta.newValue
           _state.value = FlagsSnapshot(newFlags, delta.version)
           persistToDisk()
           // Emit event for LifecycleDispatcher
           (changeFlow as MutableSharedFlow).tryEmit(FlagChangeEvent(delta.flagKey, old, delta.newValue))
           return true
       }

       @Synchronized
       fun applyBulk(bulk: List<FlagEntry>, version: Long) { /* used by FlagFetcher */ }

       fun loadFromCache() { /* read mobile-flags-cache.json if exists */ }
       private fun persistToDisk() { /* atomic tmp + rename, same pattern as rc-agent feature_flags.rs */ }
   }
   ```

   Defaults: unknown flag = `false` (deny-by-default for mobile — opposite of pod defaults, because we do NOT want a driver running on a device that was never explicitly enabled).

2. Create `FlagFetcher.kt`:
   - `suspend fun fetchAllForDevice(deviceId: String): Boolean` — `GET http://{serverBaseUrl}/api/v1/mobile/flags/{deviceId}` with staff JWT (device carries a long-lived device-service token, provisioned in phase 431 first-run UX; for phase 436 we use a placeholder header `X-Device-Token`).
   - Parses JSON, calls `store.applyBulk(...)`.
   - Exponential backoff on failure: 1s → 2s → 4s → ... → 60s cap.
   - `fun schedulePeriodicRefetch(intervalMs: Long = 5 * 60 * 1000L)` — `tickerFlow` coroutine.

3. Create `FlagDeltaHandler.kt`:
   - `fun handle(msg: CoreToAgentMessage)` — routes `MobileFlagDelta` → `store.applyDelta(payload)`. Routes `MobilePauseAll` → synthesizes a delta for `mobile.pause_all_drivers` and applies.

4. Modify `CommsLinkClient.kt` (from Phase 429): when parsing inbound WS messages, route `type: "mobile_flag_delta"` and `type: "mobile_pause_all"` to `FlagDeltaHandler.handle()`. Unknown types continue to log WARN per 429-04 unknown-field rule.

5. Modify `AgentForegroundService.onCreate()`:
   ```kotlin
   val flagStore = FeatureFlagStore(File(filesDir, "mobile-flags-cache.json"))
   flagStore.loadFromCache()
   val fetcher = FlagFetcher(httpClient, deviceState.deviceId, flagStore)
   serviceScope.launch { fetcher.fetchAllForDevice(deviceState.deviceId) }  // boot fetch
   fetcher.schedulePeriodicRefetch()                                         // 5min re-fetch
   commsClient.onMessage = FlagDeltaHandler(flagStore)::handle
   ```

6. Unit tests:
   - `FeatureFlagStoreTest.applyDeltaDropsStaleVersion()` — apply v=5, then v=3, assert v=3 rejected, state at v=5.
   - `FeatureFlagStoreTest.killSwitchOverridesPerDriverFlag()` — set `pause_all_drivers=true`, set `enable_zomato_on_rcm_tab_plus=true`, assert `get("mobile.enable_zomato_on_rcm_tab_plus")` returns `false`.
   - `FeatureFlagStoreTest.loadFromCacheSurvivesReboot()` — write cache file directly, construct store, assert flags present.
   - `FeatureFlagStoreTest.defaultIsFalseForUnknownMobileFlag()` — deny-by-default.
   - `FlagFetcherTest.backoffRespected()` — mock 500 response twice then 200, assert 2 retry delays observed.
   - `FlagFetcherTest.periodicRefetchFiresEvery5Min()` — virtual time coroutine test, advance clock 5min, assert fetch invoked.
   - `FlagDeltaHandlerTest.pauseAllTranslatedToMobilePauseAllDriversFlag()` — feed `MobilePauseAllPayload`, assert store has `mobile.pause_all_drivers = true`.

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*flags*'` passes (all 7 tests).
- Install APK on Tab Plus, `adb logcat | grep FeatureFlagStore` shows "Loaded cache: N flags, version=X" within 2s of app start.
- Manual curl PUT from plan 436-02 → within 1-2s, `adb logcat` shows "Applied delta: mobile.enable_zomato_on_rcm_tab_plus = true".

#### G4 NOT TESTED list

- Lifecycle dispatch not yet wired to driver install()/uninstall() (436-05).
- Kill-switch does not yet cancel driver CoroutineScopes (436-06).
- 10s end-to-end budget not yet measured (436-08).

#### Commit message

```
feat(436-04): Kotlin FeatureFlagStore + FlagFetcher + FlagDeltaHandler

Mirrors crates/rc-agent/src/feature_flags.rs pattern: boot fetch + 5min
periodic re-fetch (BOOT-02) + WS delta subscribe + disk cache with atomic
tmp+rename. Kill-switch priority baked into get(): pause_all_drivers=true
short-circuits any mobile.enable_* lookup. Deny-by-default for unknown
mobile.* keys. Monotonic version — stale deltas dropped.

Covers: FLAG-01 (agent consumer), FLAG-03 (delta apply <1s)
Not tested: lifecycle dispatch (436-05), kill-switch scope cancel (436-06),
E2E 10s budget (436-08).
```

---

### 436-05-PLAN — LifecycleDispatcher: enable_<driver>_on_<device> → install()/uninstall() ≤10s

**Goal:** Subscribe to `FeatureFlagStore.changeFlow` and dispatch the corresponding driver's `install()` / `uninstall()` hook within a 10-second budget, per the Phase 432 `AppDriver` contract.

**Covers:** FLAG-03

**Dependencies:** 432 (DriverRegistry + AppDriver interface), 436-04 (FeatureFlagStore)

**Type:** `auto`

#### Tasks

1. Create `LifecycleDispatcher.kt`:
   ```kotlin
   class LifecycleDispatcher(
       private val registry: DriverRegistry,            // Phase 432
       private val store: FeatureFlagStore,
       private val deviceId: String,
       private val scope: CoroutineScope,               // supervisor from AgentForegroundService
       private val dispatchDeadlineMs: Long = 10_000L,
       private val debounceMs: Long = 100L,
   ) {
       fun start() {
           scope.launch {
               store.changeFlow
                   .filter { it.key.startsWith("mobile.enable_") && it.key.endsWith("_on_$deviceId") }
                   .debounce(debounceMs)
                   .collect { event -> dispatch(event) }
           }
       }

       private suspend fun dispatch(event: FlagChangeEvent) {
           val driverId = parseDriverIdFromKey(event.key, deviceId) ?: return
           val driver = registry.get(driverId) ?: run {
               // Buffer for DriverRegistry-ready (see OQ race note in frontmatter)
               registry.onReady { dispatch(event) }
               return
           }
           withTimeoutOrNull(dispatchDeadlineMs) {
               if (event.newValue) {
                   Log.i(TAG, "Driver install: $driverId (from flag change)")
                   driver.install()
                   registry.markActive(driverId)
               } else {
                   Log.i(TAG, "Driver uninstall: $driverId (from flag change)")
                   driver.uninstall()
                   registry.markInactive(driverId)
               }
           } ?: Log.e(TAG, "Driver $driverId lifecycle hook exceeded ${dispatchDeadlineMs}ms")
       }
   }
   ```

2. `AgentForegroundService.onCreate()` wiring:
   ```kotlin
   val dispatcher = LifecycleDispatcher(driverRegistry, flagStore, deviceState.deviceId, serviceScope)
   dispatcher.start()
   ```

3. `DriverRegistry` extension (in Phase 432 code, additive):
   - `fun onReady(callback: suspend () -> Unit)` — registers a callback that fires when `loadManifests()` completes.
   - `fun markActive(driverId: String)`, `fun markInactive(driverId: String)` — maintain a `Set<String> activeDriverIds` for Phase 432 health checks + kill-switch resume.

4. Unit tests:
   - `LifecycleDispatcherTest.onEnableFiresInstall()` — emit `FlagChangeEvent("mobile.enable_zomato_on_rcm_tab_plus", false, true)`, assert mock driver's `install()` called within 1s.
   - `LifecycleDispatcherTest.onDisableFiresUninstall()` — ditto for `uninstall()`.
   - `LifecycleDispatcherTest.exceedsDeadlineLogsError()` — mock driver whose `install()` suspends 15s, assert timeout log fires and dispatcher returns (does not block).
   - `LifecycleDispatcherTest.debouncesRapidToggles()` — emit 5 events in 50ms, assert only 1 install() call (debounce).
   - `LifecycleDispatcherTest.dispatchBuffersUntilRegistryReady()` — emit event BEFORE registry.onReady, assert install() fires AFTER registry signals ready.
   - `LifecycleDispatcherTest.ignoresFlagsForOtherDevices()` — emit `..._on_rcm_m07` on a Tab Plus dispatcher, assert no driver call.

5. Timing measurement test:
   - `LifecycleDispatcherTimingTest.propagationUnder10sWithMockClock()` — virtual time, emit event, measure time to `install()` call, assert ≤ 10s (with generous mock delay on the driver).

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*LifecycleDispatcher*'` passes (7 tests).
- Manual integration: install APK + a mock HelloDriver (from Phase 432-09), toggle flag via curl PUT, measure wall-clock time to notification update. Should be well under 10s.

#### G4 NOT TESTED list

- Kill-switch scope cancellation (436-06).
- Live 10s budget across real comms-link relay (436-08 drill).
- DriverRegistry.onReady() path requires Phase 432-03 to be shipped.

#### Commit message

```
feat(436-05): LifecycleDispatcher fires install()/uninstall() on flag change

Subscribes to FeatureFlagStore.changeFlow, filters to this device's
mobile.enable_*_on_<deviceId> keys, debounces 100ms, dispatches to
DriverRegistry within a 10s withTimeoutOrNull budget. Events arriving
before DriverRegistry.onReady are buffered and replayed on ready.

Covers: FLAG-03
Not tested: kill-switch CoroutineScope cancel (436-06), E2E 10s budget
across live comms-link (436-08).
```

---

### 436-06-PLAN — Global kill-switch: pause_all_drivers overrides per-driver, halts all ≤10s

**Goal:** When `mobile.pause_all_drivers = true` is received, `KillSwitchGate` halts every active driver on the device within 10s by cancelling its supervised CoroutineScope (Phase 432 isolation primitive). When flipped back to false, drivers that WERE running before pause are automatically re-installed.

**Covers:** FLAG-04

**Dependencies:** 432 (per-driver supervised scopes), 436-05 (LifecycleDispatcher)

**Type:** `auto`

#### Tasks

1. Create `KillSwitchGate.kt`:
   ```kotlin
   class KillSwitchGate(
       private val registry: DriverRegistry,
       private val store: FeatureFlagStore,
       private val dispatcher: LifecycleDispatcher,
       private val scope: CoroutineScope,
       private val haltDeadlineMs: Long = 10_000L,
   ) {
       private val priorState = AtomicReference<Set<String>>(emptySet())

       fun start() {
           scope.launch {
               store.state
                   .map { it.flags["mobile.pause_all_drivers"] ?: false }
                   .distinctUntilChanged()
                   .collect { active -> if (active) halt() else resume() }
           }
       }

       private suspend fun halt() {
           val snapshot = registry.activeDriverIds()   // from 436-05
           priorState.set(snapshot)
           Log.w(TAG, "KillSwitchGate: halting ${snapshot.size} drivers")
           withTimeoutOrNull(haltDeadlineMs) {
               snapshot.map { id ->
                   async {
                       val driver = registry.get(id) ?: return@async
                       // Best-effort uninstall() with a sub-deadline; then force-cancel scope regardless
                       withTimeoutOrNull(haltDeadlineMs / 2) { driver.uninstall() }
                       registry.cancelDriverScope(id)
                       registry.markInactive(id)
                   }
               }.awaitAll()
           } ?: Log.e(TAG, "KillSwitchGate: halt exceeded ${haltDeadlineMs}ms; some drivers may still be running")
       }

       private suspend fun resume() {
           val toResume = priorState.getAndSet(emptySet())
           if (toResume.isEmpty()) return
           Log.i(TAG, "KillSwitchGate: resuming ${toResume.size} drivers")
           toResume.forEach { id ->
               val driver = registry.get(id) ?: return@forEach
               scope.launch { driver.install(); registry.markActive(id) }
           }
       }
   }
   ```

2. `AgentForegroundService.onCreate()`:
   ```kotlin
   val killSwitch = KillSwitchGate(driverRegistry, flagStore, dispatcher, serviceScope)
   killSwitch.start()
   ```

3. `DriverRegistry` extensions (additive to Phase 432):
   - `fun activeDriverIds(): Set<String>` — returns `Set<String>` of currently-active drivers.
   - `fun cancelDriverScope(id: String)` — cancels the per-driver supervised CoroutineScope.

4. Interaction with `LifecycleDispatcher.get()` (from 436-04): the existing `FeatureFlagStore.get()` already short-circuits `enable_*` lookups when `pause_all_drivers=true` (plan 436-04). LifecycleDispatcher should ALSO check this directly before firing `install()` to avoid a race where per-driver flag arrives after pause_all:
   ```kotlin
   // in LifecycleDispatcher.dispatch(event), before install():
   if (event.newValue && store.get("mobile.pause_all_drivers") == true) {
       Log.w(TAG, "Skipping install of $driverId: pause_all_drivers active")
       return
   }
   ```

5. Unit tests:
   - `KillSwitchGateTest.haltCancelsAllActiveDrivers()` — mock 3 active drivers; flip flag; assert all 3 have `cancelDriverScope` called within 1s (mock clock).
   - `KillSwitchGateTest.resumeRestoresOnlyPreviouslyActiveDrivers()` — halt (3 active), resume, assert 3 `install()` calls (not the whole registry).
   - `KillSwitchGateTest.haltRespectsDeadlineEvenIfUninstallHangs()` — one driver's `uninstall()` suspends forever; assert `cancelDriverScope` still fires for that driver within `haltDeadlineMs`.
   - `KillSwitchGateTest.dispatcherSkipsInstallWhilePauseActive()` — integration of LifecycleDispatcher + KillSwitchGate: pause active, emit `enable_zomato=true`, assert NO install() call.
   - `KillSwitchGateTest.noOpOnDuplicateHaltEvents()` — `distinctUntilChanged` gates re-emission; setting pause_all=true twice in a row triggers halt() only once.

#### Acceptance

- `./gradlew :app:testDebugUnitTest --tests '*KillSwitch*'` passes (5 tests).
- Manual: enable Zomato + HelloDriver on Tab Plus, then PUT `pause_all_drivers=true` via curl, `adb logcat` shows "halting 2 drivers" then 2x scope-cancel within 10s.
- Flip back to false: `adb logcat` shows "resuming 2 drivers" and both install() calls fire.

#### G4 NOT TESTED list

- Phase 432 scope-cancel semantics depend on 432-04 (isolation core). If 432-04 is not yet merged when 436-06 runs, use a stub DriverRegistry.cancelDriverScope().
- End-to-end 10s fleet-wide (2 devices) budget drill (436-08).

#### Commit message

```
feat(436-06): KillSwitchGate halts all drivers ≤10s on pause_all_drivers

Snapshot of active drivers taken at pause-time, restored at resume-time.
Per-driver uninstall() gets best-effort with sub-deadline, then scope is
force-cancelled regardless (Phase 432 isolation primitive). LifecycleDispatcher
also checks pause_all_drivers defensively to prevent race where an install
arrives while paused.

Covers: FLAG-04
Not tested: live cross-device 10s drill (436-08).
```

---

### 436-07-PLAN — mobile_flag_audit surfaced via admin activity feed

**Goal:** The existing admin activity feed (consumed by the current admin dashboard) must include `mobile_flag_audit` rows. Admin Phase 442 will render them; this plan exposes them on the API side so the data is available the moment 442 ships.

**Covers:** FLAG-02 (audit trail visibility)

**Dependencies:** 436-01 (table), 436-02 (rows written)

**Type:** `auto`

#### Tasks

1. Identify the existing activity feed endpoint. Grep: `grep -rn "activity_feed\|activity_log\|config_audit_log" crates/racecontrol/src/ | grep -i 'GET\|route'`.
   Expected: an endpoint like `/api/v1/admin/activity` that SELECTs from `config_audit_log` + possibly other tables.

2. Extend the endpoint handler to UNION the mobile audit rows:
   ```sql
   SELECT 'config_audit_log' AS source, action, entity_type, entity_name, pushed_by AS actor,
          old_value, new_value, created_at
   FROM config_audit_log
   UNION ALL
   SELECT 'mobile_flag_audit' AS source, 'update' AS action, 'mobile_flag' AS entity_type,
          flag_key AS entity_name, actor, old_value, new_value, created_at
   FROM mobile_flag_audit
   ORDER BY created_at DESC
   LIMIT ?;
   ```
   New columns returned: `source` (so admin UI can render differently), `target_device`, `target_driver` (nullable).

3. Add filter query parameter `?source=mobile_flag_audit` to allow Phase 442 admin UI to fetch only mobile entries.

4. Ensure the endpoint's staff-JWT gating remains intact (no auth regression).

5. Also: on EVERY PUT in `flags_mobile.rs::put_mobile_flag` (added in 436-02), call `activity_log::log_pod_activity(state, "server", "mobile_flag", "Mobile Flag Updated", &format!("{}={} device={} by={}", flag_key, new_value, device_id, actor), "staff", None)` for parity with the existing `feature_flags` update path (see `crates/racecontrol/src/flags.rs` line 284).

6. Unit tests in `flags_mobile_tests.rs`:
   - `admin_activity_feed_includes_mobile_flag_audit_rows()` — seed 2 rows in `mobile_flag_audit` + 1 in `config_audit_log`, hit endpoint, assert response has all 3 ordered by `created_at DESC`.
   - `admin_activity_feed_filter_by_source_mobile_flag_audit()` — assert `?source=mobile_flag_audit` returns only the 2 mobile rows.

#### Acceptance

- `cargo test -p racecontrol-crate flags_mobile_tests::admin_activity_feed_*` passes (2 tests).
- `curl -H "Authorization: Bearer $JWT" http://192.168.31.23:8080/api/v1/admin/activity?limit=50&source=mobile_flag_audit | jq .` returns the mobile rows (manual smoke post-deploy).
- `grep -n "mobile_flag_audit" crates/racecontrol/src/<activity_endpoint_file>.rs` returns ≥ 1 match.

#### G4 NOT TESTED list

- Admin dashboard rendering is Phase 442.
- No UI diff captured in this phase.

#### Commit message

```
feat(436-07): Surface mobile_flag_audit via admin activity feed UNION

UNION ALL mobile_flag_audit into existing activity_log endpoint with
a source column. Source=mobile_flag_audit filter for Phase 442 admin UI.
Parity call to log_pod_activity() added to put_mobile_flag so hash-chain
audit coverage is preserved (Phase 307 AUDIT-03).

Covers: FLAG-02 (audit visibility)
Not tested: admin UI rendering (Phase 442).
```

---

### 436-08-PLAN — Integration drill: racecontrol + Kotlin agent + mock driver, 10s propagation

**Goal:** End-to-end automated drill that measures wall-clock time from staff PUT → driver `install()` confirmed, across a real racecontrol binary + a real Kotlin agent (emulator) + a mock driver. Must reliably complete ≤ 10s.

**Covers:** FLAG-01, FLAG-03, FLAG-04 (E2E confirmation — ship gate)

**Dependencies:** 436-01 through 436-07 + Phase 432-09 (HelloDriver)

**Type:** `auto`

#### Tasks

1. Create `tests/integration/mobile-flag-e2e.sh`:

   ```bash
   #!/usr/bin/env bash
   # Phase 436 end-to-end propagation drill.
   set -euo pipefail

   RACECONTROL_BIN=${RACECONTROL_BIN:-target/release/racecontrol}
   DEVICE_SERIAL=${DEVICE_SERIAL:-emulator-5554}
   DEVICE_ID=${DEVICE_ID:-rcm-emu-test}
   STAFF_JWT=${STAFF_JWT:?"Set STAFF_JWT env"}

   # 1. Start racecontrol against a tmp DB
   TMPDB=$(mktemp)
   $RACECONTROL_BIN --db "$TMPDB" --port 18080 &
   RC_PID=$!
   trap "kill $RC_PID; rm -f $TMPDB" EXIT
   sleep 2  # boot

   # 2. Install + launch agent on emulator with HelloDriver enabled in drivers.json
   adb -s $DEVICE_SERIAL install -r rc-agent-mobile/app/build/outputs/apk/debug/app-debug.apk
   adb -s $DEVICE_SERIAL shell am start-foreground-service \
       -n in.racingpoint.rcagentmobile/.service.AgentForegroundService \
       --es SERVER_URL http://10.0.2.2:18080 \
       --es DEVICE_ID $DEVICE_ID

   # 3. Wait for agent to register
   for i in {1..20}; do
       resp=$(curl -s -H "Authorization: Bearer $STAFF_JWT" \
           http://localhost:18080/api/v1/mobile/flags/$DEVICE_ID || true)
       if echo "$resp" | grep -q '"flags"'; then break; fi
       sleep 1
   done

   # 4. Clear logcat to capture only the test window
   adb -s $DEVICE_SERIAL logcat -c

   # 5. TEST A — install via flag toggle (enable)
   T0=$(date +%s%3N)
   curl -s -X PUT -H "Authorization: Bearer $STAFF_JWT" \
       -H "Content-Type: application/json" \
       http://localhost:18080/api/v1/mobile/flags/$DEVICE_ID/enable_hello \
       -d '{"enabled": true}' > /dev/null

   # Wait for logcat line "Driver install: hello"
   adb -s $DEVICE_SERIAL logcat -d | grep -q 'Driver install: hello' || {
       for i in {1..30}; do
           sleep 1
           if adb -s $DEVICE_SERIAL logcat -d | grep -q 'Driver install: hello'; then break; fi
       done
   }
   T1=$(date +%s%3N)
   DELTA_A=$((T1 - T0))
   echo "TEST A (enable → install): ${DELTA_A}ms"
   [ $DELTA_A -le 10000 ] || { echo "FAIL: TEST A exceeded 10s"; exit 1; }

   # 6. TEST B — uninstall via flag toggle (disable)
   adb -s $DEVICE_SERIAL logcat -c
   T0=$(date +%s%3N)
   curl -s -X PUT -H "Authorization: Bearer $STAFF_JWT" \
       -H "Content-Type: application/json" \
       http://localhost:18080/api/v1/mobile/flags/$DEVICE_ID/enable_hello \
       -d '{"enabled": false}' > /dev/null
   for i in {1..30}; do
       sleep 1
       if adb -s $DEVICE_SERIAL logcat -d | grep -q 'Driver uninstall: hello'; then break; fi
   done
   T1=$(date +%s%3N)
   DELTA_B=$((T1 - T0))
   echo "TEST B (disable → uninstall): ${DELTA_B}ms"
   [ $DELTA_B -le 10000 ] || { echo "FAIL: TEST B exceeded 10s"; exit 1; }

   # 7. TEST C — kill-switch
   # Re-enable first
   curl -s -X PUT -H "Authorization: Bearer $STAFF_JWT" \
       http://localhost:18080/api/v1/mobile/flags/$DEVICE_ID/enable_hello \
       -d '{"enabled": true}' > /dev/null
   sleep 3
   adb -s $DEVICE_SERIAL logcat -c

   T0=$(date +%s%3N)
   curl -s -X PUT -H "Authorization: Bearer $STAFF_JWT" \
       http://localhost:18080/api/v1/mobile/flags/*/pause_all_drivers \
       -d '{"enabled": true}' > /dev/null
   for i in {1..30}; do
       sleep 1
       if adb -s $DEVICE_SERIAL logcat -d | grep -q 'KillSwitchGate: halting'; then break; fi
   done
   T1=$(date +%s%3N)
   DELTA_C=$((T1 - T0))
   echo "TEST C (pause_all → halting): ${DELTA_C}ms"
   [ $DELTA_C -le 10000 ] || { echo "FAIL: TEST C exceeded 10s"; exit 1; }

   # 8. TEST D — audit row exists
   ROWS=$(sqlite3 "$TMPDB" "SELECT count(*) FROM mobile_flag_audit;")
   [ $ROWS -ge 4 ] || { echo "FAIL: expected >=4 audit rows, got $ROWS"; exit 1; }

   echo "ALL TESTS PASSED: A=${DELTA_A}ms B=${DELTA_B}ms C=${DELTA_C}ms D=${ROWS} rows"
   ```

2. Run the drill 3 times consecutively to surface flakes. Accept variance: each run must individually pass.

3. Document failure modes in `tests/integration/mobile-flag-e2e-NOTES.md`:
   - Emulator cold-start adds ~5s to TEST A only.
   - If `ROWS < 4`, PUT path didn't write audit — check flags_mobile.rs.
   - If TEST C fails but A+B pass, KillSwitchGate or pause-broadcast routing is the suspect.

4. Make the drill part of CI (non-blocking initially, blocking after first green week):
   - Add to `.github/workflows/integration.yml` with matrix on emulator API 33, 34 (future-proofing).
   - Also add to `scripts/gate-check.sh --pre-milestone-ship` for v50.0.

#### Acceptance

- `bash tests/integration/mobile-flag-e2e.sh` passes 3/3 consecutive runs.
- Each of TEST A, B, C under 10_000ms (print exact values in the final summary line).
- TEST D confirms ≥ 4 audit rows.
- CI job green.

#### G4 NOT TESTED list

- Physical Tab Plus + M07 device drill — emulator proves the propagation; physical devices add OEM-killer variance addressed in Phase 429 risk notes. Real-device drill deferred to milestone-ship Phase 444 E2E drill (Phase 16 in ROADMAP).

#### Commit message

```
test(436-08): end-to-end mobile flag propagation drill ≤10s

Four-test drill: enable→install, disable→uninstall, pause_all→halting,
audit row count. Must pass 3/3 runs on Android emulator. Drives real
racecontrol binary, real Kotlin agent APK, real HelloDriver from 432-09.
CI matrix on API 33+34. Added to gate-check.sh --pre-milestone-ship.

Covers: FLAG-01, FLAG-03, FLAG-04 (E2E ship gate)
Not tested: physical Tab Plus + M07 drill (deferred to Phase 444 milestone
ship gate — emulator proves the protocol; real-device OEM variance is a
Phase 429/431 concern).
```

---

## 6. Phase deploy manifest

| Layer | Action |
|---|---|
| Rust binary | Rebuild racecontrol (new routes, protocol enum variants, state fields). Swap on server .23 via `deploy-staging/deploy-server.sh`. |
| DB migration | Auto-runs on racecontrol boot (idempotent CREATE TABLE + indexes). No manual step. |
| Cloud parity | Run the same binary on Bono VPS (pm2 restart after git pull + rebuild). DB migration runs same way. |
| Android APK | Rebuild `rc-agent-mobile` with plans 436-04..06 changes. ADB install -r on both devices. Previous APK kept at `/sdcard/Download/rc-agent-mobile-prev.apk` for 72hr rollback. |
| Frontend | None (admin UI is Phase 442). |
| comms-link relays | No code change required — protocol is additive; relays forward envelopes they do not introspect. Verify by sending a `mobile_flag_delta` through the relay in the drill. |

Per CLAUDE.md DMP rule: update `docs/ARCHITECTURE.md` Section 20.3 after ship, and memory (`gsd-projects.md`) with v50 Phase 8 completion entry.

## 7. Phase-level verification (post-ship)

```bash
# 1. Server-side health
curl -s http://192.168.31.23:8080/api/v1/health | jq .
curl -s -H "Authorization: Bearer $JWT" http://192.168.31.23:8080/api/v1/mobile/flags/rcm-tab-plus | jq .

# 2. DB migration present on both environments
sqlite3 C:/RacingPoint/data/racecontrol.db "SELECT count(*) FROM sqlite_master WHERE name='mobile_flag_audit';"
# → 1
# Repeat on Bono VPS via relay: curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"sqlite_schema_check"}'

# 3. Agent-side: flag cache exists post-first-sync
adb -s <tab_plus> shell cat /sdcard/Android/data/in.racingpoint.rcagentmobile/files/mobile-flags-cache.json | jq .
adb -s <m07> shell cat /sdcard/Android/data/in.racingpoint.rcagentmobile/files/mobile-flags-cache.json | jq .

# 4. Kill-switch reachable (do NOT leave on in production)
curl -s -X PUT -H "Authorization: Bearer $JWT" http://192.168.31.23:8080/api/v1/mobile/flags/*/pause_all_drivers -d '{"enabled": true}'
sleep 12
adb -s <tab_plus> logcat -d | grep 'KillSwitchGate: halting' | tail -5
curl -s -X PUT -H "Authorization: Bearer $JWT" http://192.168.31.23:8080/api/v1/mobile/flags/*/pause_all_drivers -d '{"enabled": false}'

# 5. Audit surface
curl -s -H "Authorization: Bearer $JWT" http://192.168.31.23:8080/api/v1/admin/activity?source=mobile_flag_audit&limit=10 | jq .

# 6. Route uniqueness (regression check)
grep -n '\.route("/' crates/racecontrol/src/api/routes.rs | sed 's/.*\.route("//' | sed 's/".*//' | sort | uniq -d
# → empty
```

## 8. Phase ship gate

- [ ] All 8 plans' commits landed in a single PR or linear commit chain
- [ ] `cargo test -p racecontrol-crate flags_mobile_tests` passes (all tests)
- [ ] `cargo test -p racecontrol-crate ws::mobile_flag_sync_tests` passes
- [ ] `./gradlew :app:testDebugUnitTest --tests '*flags*'` passes in rc-agent-mobile
- [ ] `bash tests/integration/mobile-flag-e2e.sh` passes 3/3 on emulator
- [ ] nyquist-audit subagent run: all 4 requirements have automated tests (per CLAUDE.md Subagent Gates)
- [ ] MMA audit (kill-switch is safety-critical — dual-reasoning-mode REQUIRED per CLAUDE.md): run full 5-model audit across DIAGNOSE+PLAN+VERIFY phases with at least one thinking model variant per step
- [ ] integration-checker subagent: confirm 432↔436 contract, confirm 442 will be able to consume audit feed (contract-readiness check)
- [ ] Deployed to server .23 (build_id recorded in LOGBOOK)
- [ ] Deployed to Bono VPS (cloud parity)
- [ ] APK deployed to Tab Plus + M07 with previous APK preserved at /sdcard/Download/
- [ ] `.planning/ROADMAP-v50.md` phase 8 checkbox flipped to [x]
- [ ] `docs/ARCHITECTURE.md` Section 20.3 updated
- [ ] `~/.claude/projects/C--Users-bono/memory/gsd-projects.md` active work entry moved to shipped

## 9. Links out

- Phase 432 PLAN (driver framework — direct dependency): `.planning/phases/432-driver-framework-capability-registry/PLAN.md`
- Phase 442 PLAN (admin toggle UI — direct consumer, ships after): to be written
- Phase 429 PLAN (protocol envelope): `.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md`
- Existing server flag infra: `crates/racecontrol/src/flags.rs`
- Existing pod flag consumer (pattern reference): `crates/rc-agent/src/feature_flags.rs`
- ROADMAP v50 Phase 8 row: `.planning/ROADMAP-v50.md` line 111
- Requirements FLAG-01..04: `.planning/REQUIREMENTS-v50.md` lines 90-95
