# Architecture Research: v41.0 Game Intelligence System

**Domain:** Per-pod game inventory, proactive combo validation, launch timeline tracing, crash loop detection, reliability dashboard — integrated into existing Meshed Intelligence Rust/Axum monorepo
**Researched:** 2026-04-03 IST
**Confidence:** HIGH — based on direct inspection of all relevant source files in the deployed codebase

---

> Note: This file supersedes the v31.0 architecture for this milestone. The v31.0 survival architecture (survival_coordinator, rc-guardian, smart_watchdog) is already shipped. This document focuses exclusively on v41.0 integration points.

---

## Standard Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ADMIN DASHBOARD (:3201)                   KIOSK (:3300)                    │
│  /reliability page (NEW)                   game selection panel (MODIFIED)  │
│  - Fleet game matrix                       - Filter by installed_games       │
│  - Flagged combo list                      - Show flagged combo badges        │
│  - Launch timeline viewer                                                   │
│  - Per-combo success rates                                                  │
└─────────────────────────────────┬───────────────────────┬───────────────────┘
                                  │ HTTP REST             │ WebSocket
┌─────────────────────────────────▼───────────────────────▼───────────────────┐
│  RACECONTROL SERVER (:8080)                                                  │
│                                                                              │
│  ws/mod.rs (MODIFIED)           state.rs (MODIFIED)                         │
│  - Handle ContentManifest       - pod_manifests: HashMap<pod_id, ExtManifest>│
│  - Write pod_game_inventory     - combo_validation_flags: in-memory cache    │
│                                                                              │
│  NEW endpoints:                 EXTENDED endpoints:                         │
│  GET /api/v1/fleet/game-matrix  GET /games/catalog (already exists)         │
│  GET /api/v1/presets/{id}/valid GET /games/alternatives (already exists)    │
│  GET /api/v1/launch-timeline/*  GET /api/v1/metrics/launch-stats (exists)   │
│  POST /api/v1/fleet/combo-scan                                               │
│                                                                              │
│  NEW modules:                   EXTENDED modules:                            │
│  combo_validator.rs             preset_library.rs (add per-pod queries)     │
│  game_inventory.rs              api/metrics.rs (add timeline endpoints)     │
│                                 ws/mod.rs (handle new WS messages)          │
│                                                                              │
│  NEW DB tables:                 EXISTING DB tables:                          │
│  pod_game_inventory             combo_reliability (extended)                 │
│  launch_timeline_spans          launch_events (existing)                     │
│  combo_validation_flags         game_presets (existing)                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                  │ WebSocket (LAN)
┌─────────────────────────────────▼───────────────────────────────────────────┐
│  RC-AGENT (each pod, :8090)                                                  │
│                                                                              │
│  content_scanner.rs (EXTENDED)  game_doctor.rs (EXTENDED)                   │
│  - scan_ac_content (exists)     - diagnose_and_fix (exists, reactive)       │
│  - scan_steam_library (NEW)     - run_boot_validation (NEW, proactive)       │
│  - scan_non_steam (NEW)         - validate_combo_filesystem (NEW)            │
│  - build_full_manifest (NEW)                                                 │
│                                                                              │
│  tier_engine.rs (EXTENDED)      game_launch_retry.rs (EXTENDED)             │
│  - GameLaunchFail (exists)      - retry_game_launch (exists, 60s limit)     │
│  - GameLaunchTimeout (NEW)      - timeout_watchdog wraps launch (NEW)       │
│  - CrashLoop (NEW)                                                           │
│                                                                              │
│  NEW modules:                   EXISTING passing through:                    │
│  launch_timeline.rs             diagnostic_engine.rs (add CrashLoop emit)   │
│  crash_loop_detector.rs         failure_monitor.rs (feeds crash_loop count) │
│                                                                              │
│  NEW WS messages (AgentMessage variants):                                    │
│  GameInventoryUpdate { installed_games, manifest }                           │
│  LaunchTimelineReport { launch_id, spans }                                   │
│  ComboValidationResult { preset_id, flags }                                  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Location |
|-----------|----------------|----------|
| `content_scanner.rs` (extended) | Filesystem scan — AC + Steam + non-Steam. Produces `ExtendedManifest`. | `crates/rc-agent/src/` |
| `game_inventory.rs` (new, server) | Persists `pod_game_inventory` table; answers fleet matrix queries | `crates/racecontrol/src/` |
| `combo_validator.rs` (new, server) | Crosses presets vs content manifest at boot; writes `combo_validation_flags` | `crates/racecontrol/src/` |
| `launch_timeline.rs` (new, agent) | Captures per-phase launch spans; INSERT INTO launch_timeline_spans | `crates/rc-agent/src/` |
| `crash_loop_detector.rs` (new, agent) | Rolling-window failure counter; emits `CrashLoop` DiagnosticTrigger | `crates/rc-agent/src/` |
| `tier_engine.rs` (extended) | Handles `GameLaunchTimeout` and `CrashLoop` triggers with Tier 0-5 escalation | `crates/rc-agent/src/` |
| `game_doctor.rs` (extended) | `run_boot_validation()` — proactive filesystem check at startup (not reactive) | `crates/rc-agent/src/` |
| `ws/mod.rs` (extended, server) | Handles new `GameInventoryUpdate`, `LaunchTimelineReport`, `ComboValidationResult` messages | `crates/racecontrol/src/ws/` |
| `preset_library.rs` (extended) | Per-pod reliability query + auto-disable when `combo_validation_flags` exists | `crates/racecontrol/src/` |
| `/reliability` admin page (new) | Fleet game matrix + flagged combos + launch timeline viewer | `racingpoint-admin/src/app/` |
| kiosk game selection (modified) | Filter `GAME_DISPLAY` by `pod.installed_games`; show flagged badge on preset | `kiosk/src/components/` |

---

## New vs Modified: Explicit Breakdown

### New Modules

| File | Crate | What It Does |
|------|-------|-------------|
| `crates/rc-agent/src/launch_timeline.rs` | rc-agent | `LaunchTimeline` struct + `record_span()`. Wraps game launch to checkpoint each phase (`process_start`, `steam_overlay_dismissed`, `first_telemetry_frame`, `billing_playable`). Persists to pod-local rusqlite and emits `AgentMessage::LaunchTimelineReport` at end. |
| `crates/rc-agent/src/crash_loop_detector.rs` | rc-agent | Rolling 10-minute window counter per `sim_type`. On `N >= 3` failures, emits `DiagnosticTrigger::CrashLoop { sim_type, fail_count, window_secs }`. State persisted to `C:\RacingPoint\crash_loop_state.json` (survives rc-agent restart). |
| `crates/racecontrol/src/game_inventory.rs` | racecontrol | Persists `pod_game_inventory` from incoming `GameInventoryUpdate` WS message. Answers `GET /api/v1/fleet/game-matrix` with pod × game matrix + install status. |
| `crates/racecontrol/src/combo_validator.rs` | racecontrol | Called at server boot and on each `GameInventoryUpdate`. Crosses `game_presets` against `pod_manifests` in AppState. Writes `combo_validation_flags` rows. Auto-sets `enabled = false` on presets with missing car/track/ai_lines. |

### Modified Modules

| File | Crate | What Changes |
|------|-------|-------------|
| `crates/rc-agent/src/content_scanner.rs` | rc-agent | Add `scan_steam_library()`, `scan_non_steam_games()`, `build_full_manifest()`. AC scan stays unchanged. New `ExtendedManifest` struct wraps `ContentManifest` + `Vec<InstalledGame>`. |
| `crates/rc-agent/src/game_doctor.rs` | rc-agent | Add `run_boot_validation(manifest: &ExtendedManifest) -> Vec<ComboFlag>`. Called at startup after content scan. Checks AC combos for ai_lines, pit stalls, surfaces.ini. Non-AC games checked for exe existence only. |
| `crates/rc-agent/src/tier_engine.rs` | rc-agent | Add match arms for `DiagnosticTrigger::GameLaunchTimeout` (Tier 1: kill stale process + retry once, Tier 2: KB lookup, Tier 3+: MMA) and `DiagnosticTrigger::CrashLoop` (Tier 0: disable combo + WhatsApp alert, skip retry). |
| `crates/rc-agent/src/game_launch_retry.rs` | rc-agent | Wrap `retry_game_launch()` with `tokio::time::timeout`. On timeout: emit `GameLaunchTimeout` trigger to diagnostic_engine channel. |
| `crates/rc-agent/src/diagnostic_engine.rs` | rc-agent | Add `CrashLoop` and `GameLaunchTimeout` to `DiagnosticTrigger` enum. Add emission path in the diagnostic loop for crash_loop_detector output. |
| `crates/rc-agent/src/main.rs` | rc-agent | At boot after `scan_ac_content()`: call `build_full_manifest()`, run `game_doctor::run_boot_validation()`, send `AgentMessage::GameInventoryUpdate`. |
| `crates/racecontrol/src/ws/mod.rs` | racecontrol | Handle `GameInventoryUpdate` → call `game_inventory::upsert_pod_inventory()` + `combo_validator::run_for_pod()`. Handle `LaunchTimelineReport` → insert into `launch_timeline_spans`. Handle `ComboValidationResult` → upsert `combo_validation_flags`. |
| `crates/racecontrol/src/preset_library.rs` | racecontrol | In `list_presets_with_reliability()`: JOIN with `combo_validation_flags` to add `is_flagged` and `flag_reason` to `GamePresetWithReliability`. Add per-pod variant `list_presets_for_pod(pod_id)` that additionally filters by `pod_game_inventory`. |
| `crates/racecontrol/src/state.rs` | racecontrol | `pod_manifests` type changes from `HashMap<String, ContentManifest>` to `HashMap<String, ExtendedManifest>` (backward compatible — `ContentManifest` is embedded in `ExtendedManifest`). |
| `crates/rc-common/src/types.rs` | rc-common | Add `ExtendedManifest { ac_manifest: ContentManifest, installed_games: Vec<InstalledGame> }`. Add `InstalledGame { sim_type: SimType, install_path: String, verified_at: String }`. Extend `GamePresetWithReliability` with `is_flagged: bool, flag_reason: Option<String>`. Add new `AgentMessage` variants: `GameInventoryUpdate`, `LaunchTimelineReport`, `ComboValidationResult`. Add `DiagnosticTrigger::CrashLoop` and `DiagnosticTrigger::GameLaunchTimeout`. |

---

## Recommended Project Structure

```
crates/
├── rc-common/src/
│   └── types.rs                     MODIFIED — ExtendedManifest, InstalledGame,
│                                               new AgentMessage variants,
│                                               CrashLoop + GameLaunchTimeout triggers
│
├── rc-agent/src/
│   ├── content_scanner.rs           MODIFIED — add Steam + non-Steam scan
│   ├── game_doctor.rs               MODIFIED — add run_boot_validation()
│   ├── game_launch_retry.rs         MODIFIED — add timeout wrapper
│   ├── tier_engine.rs               MODIFIED — add CrashLoop + GameLaunchTimeout arms
│   ├── diagnostic_engine.rs         MODIFIED — add new trigger variants + emission
│   ├── main.rs                      MODIFIED — boot scan + GameInventoryUpdate send
│   ├── launch_timeline.rs           NEW — LaunchTimeline, record_span, WS emit
│   └── crash_loop_detector.rs       NEW — rolling window, CrashLoop emit, state persist
│
└── racecontrol/src/
    ├── game_inventory.rs             NEW — pod_game_inventory table, fleet matrix query
    ├── combo_validator.rs            NEW — preset x manifest cross-ref, flag writes
    ├── ws/mod.rs                     MODIFIED — handle 3 new AgentMessage variants
    ├── preset_library.rs             MODIFIED — add is_flagged, list_presets_for_pod
    ├── state.rs                      MODIFIED — pod_manifests type to ExtendedManifest
    └── api/
        ├── routes.rs                 MODIFIED — register new endpoints
        └── metrics.rs                MODIFIED — add launch_timeline_spans endpoints

Frontend:
racingpoint-admin/src/app/
└── reliability/
    └── page.tsx                      NEW — fleet game matrix + flagged combos + timeline
kiosk/src/components/
├── GamePickerPanel.tsx               MODIFIED — filter by installedGames (already has prop)
└── GamePresetCard.tsx (or equivalent) MODIFIED — show flagged badge from is_flagged
```

---

## Architectural Patterns

### Pattern 1: Boot-Time Proactive Check via Existing Startup Sequence

**What:** Extend the existing boot scan path (`scan_ac_content()` → `AgentMessage::ContentManifest`) to call `build_full_manifest()` and `game_doctor::run_boot_validation()` in the same startup function, then emit `AgentMessage::GameInventoryUpdate` instead of (or in addition to) `ContentManifest`.

**When to use:** When the new check shares the same trigger point (WS connect/reconnect) and same data source (filesystem) as the existing check. No new scheduling or polling needed.

**Why not a separate timer:** `scan_ac_content()` already runs at startup AND on WS reconnect (line 1950 in main.rs). Piggybacking onto this call site means the inventory is always fresh when the server gets it. A separate 5-minute timer would leave the server with stale data after a pod reconnect.

**Example (pseudocode):**
```rust
// In main.rs, ws_connect path (existing call site at line ~1950)
let ac_manifest = content_scanner::scan_ac_content();
let steam_games = content_scanner::scan_steam_library();
let non_steam = content_scanner::scan_non_steam_games(&config);
let extended = ExtendedManifest { ac_manifest, installed_games: [steam_games, non_steam].concat() };

// Boot validation: deterministic filesystem check, no AI
let flags = game_doctor::run_boot_validation(&extended);

let msg = AgentMessage::GameInventoryUpdate { manifest: extended, combo_flags: flags };
ws_send(msg);
```

### Pattern 2: Server-Side Cross-Reference on Inventory Arrival

**What:** When `GameInventoryUpdate` arrives at the server's WS handler, immediately run `combo_validator::run_for_pod(pod_id, &manifest, &state.db)`. This computes which presets are broken for THAT pod and writes `combo_validation_flags` rows. No polling, no scheduled job.

**When to use:** When the validation is O(preset_count × filesystem checks) and the dataset is small (Racing Point has ~20-50 presets). The event-driven approach means validation happens exactly when new inventory data arrives — not on a timer that may race with the WS message.

**Trade-off:** If a pod sends `GameInventoryUpdate` on every reconnect, combo_validator runs each time. This is acceptable: validation is a pure DB write (idempotent UPSERT), and pods reconnect rarely (boot, WS drop recovery).

### Pattern 3: New DiagnosticTrigger Arms in Existing Tier Engine

**What:** Add `GameLaunchTimeout` and `CrashLoop` to the `DiagnosticTrigger` enum and add match arms in `tier_engine.rs` using the SAME structure as existing arms.

**When to use:** The tier engine's match arms are already the canonical integration point for any new diagnostic scenario. Adding a new trigger arm is two changes: (1) extend the enum in rc-common/types.rs, (2) add a match arm in tier_engine.rs. No new execution paths, no new channels.

**Critical constraint — CrashLoop arm must be Tier 0, not Tier 1:** A crash loop means the game is persistently broken on this pod. Tier 1 (diagnose and retry) would cause infinite retries. Tier 0 (hardened response, no AI, no cost) should: disable the combo via `combo_validation_flags`, send `WhatsApp` alert via Tier 5 path, and return `TierResult::Fixed` to stop the loop. The WhatsApp path already exists in the EscalationRequest WS message.

**Example:**
```rust
// In tier_engine.rs run_tier1() match
DiagnosticTrigger::GameLaunchTimeout => {
    // Kill orphan processes (same as GameLaunchFail Tier 1)
    let killed = kill_orphan_game_processes();
    if killed > 0 {
        TierResult::Fixed { tier: 1, action: format!("killed {} orphan processes", killed) }
    } else {
        TierResult::FailedToFix { tier: 1, reason: "no orphan processes found".into() }
    }
}
DiagnosticTrigger::CrashLoop { sim_type, fail_count, .. } => {
    // Tier 0: disable combo, do NOT retry
    disable_combo_local(sim_type);
    emit_escalation_request(format!("CrashLoop: {} failed {} times", sim_type, fail_count));
    TierResult::Fixed { tier: 0, action: "combo disabled, staff alerted".into() }
}
```

### Pattern 4: Launch Timeline as Inline Instrumentation

**What:** `launch_timeline.rs` provides a `LaunchTimeline` struct that wraps game launch execution. Each phase in the existing launch code calls `timeline.record("label")` which captures elapsed millis since launch start. No new threads, no new channels — purely additive to the existing launch function body.

**When to use:** When the cost of instrumentation must be near-zero (no allocation on the hot path, no IO until launch completes). Timeline data is written to rusqlite only after the launch succeeds or fails — not during.

**Critical:** Do not instrument the timeout watchdog itself. If the timeout fires, the timeline's final span is `timeout_fired`. The incomplete timeline is still written to rusqlite (partial data is useful for diagnosis).

---

## Data Flow

### Flow 1: Boot Inventory Scan

```
[rc-agent starts / WS reconnects]
    ↓
content_scanner::build_full_manifest()
    ├── scan_ac_content_at(AC_CONTENT_PATH)          [existing]
    ├── scan_steam_library(STEAM_PATH)               [new]
    └── scan_non_steam_games(&config.game_paths)     [new]
    ↓
game_doctor::run_boot_validation(&manifest)           [new, sync, filesystem-only]
    ├── For each AC preset: check car folder, track folder, ai_lines, pit stalls
    └── For each non-AC game: check exe exists at install_path
    ↓
AgentMessage::GameInventoryUpdate { manifest, combo_flags }  [new WS message]
    ↓ (WS to server)
ws/mod.rs: handle GameInventoryUpdate
    ├── game_inventory::upsert_pod_inventory(pod_id, &manifest, &db)
    │     → INSERT OR REPLACE INTO pod_game_inventory
    ├── state.pod_manifests.write().insert(pod_id, manifest)
    └── combo_validator::run_for_pod(pod_id, &manifest, &state.db)
          → UPSERT INTO combo_validation_flags
          → auto-disable: UPDATE game_presets SET enabled=0 WHERE preset has critical flag
```

### Flow 2: Game Launch Timeout

```
[kiosk triggers /games/launch]
    ↓
game_launcher.rs: send LaunchGame WS command to agent
    ↓
rc-agent: game_launch_retry::retry_game_launch() wrapped in tokio::time::timeout(90s)
    ├── OK path: game launches → LaunchTimeline records spans → emit LaunchTimelineReport
    │
    └── TIMEOUT path:
          ↓
        game_launch_retry: emit DiagnosticTrigger::GameLaunchTimeout to diagnostic_engine channel
          ↓
        tier_engine: match GameLaunchTimeout
          - Tier 1: kill_orphan_game_processes()
          - Tier 2: KB lookup for known fixes
          - Tier 3/4: MMA (if Tier 1-2 fail)
          - Tier 5: EscalationRequest WhatsApp
          ↓
        LaunchTimeline records "timeout_fired" span → emit partial LaunchTimelineReport
```

### Flow 3: Crash Loop Detection

```
[rc-agent: game launch fails (any cause)]
    ↓
crash_loop_detector::record_failure(sim_type)
    ├── Append to rolling window (in-memory + crash_loop_state.json)
    └── fail_count < 3: return None (no action)
        fail_count >= 3 within 10 minutes:
          ↓
        Emit DiagnosticTrigger::CrashLoop { sim_type, fail_count, window_secs }
          ↓
        tier_engine: match CrashLoop
          - Tier 0: disable combo locally
          - Tier 5: EscalationRequest { reason: "crash_loop", sim_type, fail_count }
          ↓
        ws: AgentMessage::EscalationRequest (existing message type)
          ↓
        Server: forward to Tier 5 WhatsApp via Bono relay (existing path)
```

### Flow 4: Kiosk Showing Per-Pod Games

```
[customer opens kiosk booking wizard for Pod N]
    ↓
kiosk fetches pod state (existing WS or REST)
    ↓
pod.installed_games: Vec<SimType> (populated from GameInventoryUpdate, persisted in pod_game_inventory)
    ↓
GamePickerPanel: filter GAME_DISPLAY keys by installedGames prop  [already has prop, already passes it]
    ↓
Only installed games shown. AC launches preset wizard. Non-AC launches directly.
    ↓
Preset selection: GET /api/v1/presets?pod_id=N
    ↓
preset_library::list_presets_for_pod(pod_id) — filters by pod inventory + JOIN combo_validation_flags
    ↓
Presets with is_flagged=true show warning badge. Auto-disabled presets not returned.
```

---

## Integration Points: New vs Modified Summary

### What is Purely New (no existing code touched)

| Component | Crate | Why New |
|-----------|-------|---------|
| `launch_timeline.rs` | rc-agent | No existing timeline tracing exists anywhere |
| `crash_loop_detector.rs` | rc-agent | No existing crash loop detection; failure_monitor tracks state but does not count consecutive launch failures per sim_type |
| `game_inventory.rs` | racecontrol | `pod_game_inventory` table and fleet matrix query have no existing counterpart |
| `combo_validator.rs` | racecontrol | No existing proactive cross-ref of presets vs manifest; `game_doctor.diagnose_and_fix()` is reactive-only |
| `/reliability` admin page | racingpoint-admin | No existing reliability dashboard page |
| `pod_game_inventory` table | racecontrol SQLite | New DB table |
| `launch_timeline_spans` table | rc-agent + racecontrol SQLite | New DB table (both sides) |
| `combo_validation_flags` table | racecontrol SQLite | New DB table |

### What is Extended (existing code touched, additive)

| Component | Crate | Extension Point |
|-----------|-------|-----------------|
| `content_scanner.rs` | rc-agent | Add 2 new scan functions; `build_full_manifest()` wraps existing `scan_ac_content_at()` unchanged |
| `game_doctor.rs` | rc-agent | Add `run_boot_validation()` as a new public function; existing `diagnose_and_fix()` unchanged |
| `game_launch_retry.rs` | rc-agent | Wrap existing `retry_game_launch()` call site with timeout; retry logic unchanged |
| `tier_engine.rs` | rc-agent | Add 2 match arms to existing match block; all other arms unchanged |
| `diagnostic_engine.rs` | rc-agent | Extend enum with 2 new variants; add 2 emission paths in detector loop |
| `main.rs` | rc-agent | Replace `scan_ac_content()` call with `build_full_manifest()` call at same call site (line ~1950) |
| `ws/mod.rs` | racecontrol | Add 3 new match arms to the `AgentMessage` match block; existing arms unchanged |
| `preset_library.rs` | racecontrol | Add `list_presets_for_pod(pod_id)` as new function; `list_presets_with_reliability()` gets JOIN extension |
| `state.rs` | racecontrol | `pod_manifests` type change from `HashMap<String, ContentManifest>` to `HashMap<String, ExtendedManifest>`. Backward compatible because `ExtendedManifest.ac_manifest` is the original `ContentManifest`. |
| `types.rs` | rc-common | Additive struct/enum extensions; no existing fields removed or renamed |
| `GamePickerPanel.tsx` | kiosk | Already receives `installedGames` prop (line 53); currently shows all games that have a GAME_DISPLAY entry. Change: filter is already implemented at line 58 (`installedGames.filter(g => GAME_DISPLAY[g] !== undefined)`). No structural change needed — the `installedGames` field just needs to be populated from `pod.installed_games` (which is already in `PodInfo`). |

### What Must NOT Change (risk of regression)

| Component | Risk if touched | Safe boundary |
|-----------|----------------|---------------|
| `combo_reliability` table schema | `preset_library.rs` and `api/metrics.rs` both query it with specific column names | Add new tables alongside it; do not ALTER this table |
| `AgentMessage::ContentManifest` variant | Already has tests (rc-common/src/protocol.rs:2046-2218); kiosk book page may reference it | Keep `ContentManifest` variant for backward compat with old agents. `GameInventoryUpdate` is additive — old servers that don't know `GameInventoryUpdate` will ignore it. |
| `GamePresetWithReliability` existing fields | `PresetPushPayload` is sent over WS to agents; kiosk renders `reliability_score` + `flagged_unreliable` | Only ADD fields (`is_flagged`, `flag_reason`) with `#[serde(default)]` |
| `game_launch_retry::retry_game_launch()` signature | Called from tier_engine.rs; caller uses `RetryResult` enum | Do not change function signature; wrap the call site instead |
| `DiagnosticTrigger` existing variants | tier_engine match arms already handle them; changing variants breaks exhaustive match | Only ADD new variants; Rust compiler enforces this |

---

## Build Order (Considers Dependencies)

Dependencies drive this order: rc-common is the leaf crate — everything depends on it. Agent changes must compile before deploy. Server endpoint must exist before agent can send to it. Frontend is always last.

```
Phase 1 — rc-common Types Foundation (UNBLOCKS ALL)
    Files: crates/rc-common/src/types.rs
    Add: ExtendedManifest, InstalledGame
         GameInventoryUpdate / LaunchTimelineReport / ComboValidationResult AgentMessage variants
         DiagnosticTrigger::CrashLoop, DiagnosticTrigger::GameLaunchTimeout
         is_flagged + flag_reason fields on GamePresetWithReliability (#[serde(default)])
    Verify: cargo check -p rc-common (must compile clean)
    Deploy: None (lib-only change)
    Rationale: Everything else imports rc-common types. Doing this first means
               all downstream cargo check passes from the start.

Phase 2 — Agent Content Scanner Extension
    Files: crates/rc-agent/src/content_scanner.rs
           crates/rc-agent/src/game_doctor.rs (add run_boot_validation)
    Add: scan_steam_library(), scan_non_steam_games(), build_full_manifest()
         game_doctor::run_boot_validation()
    Verify: cargo test -p rc-agent -- content_scanner (all existing tests still pass)
    Deploy: Pod 8 canary — verify GameInventoryUpdate received by server via server logs
    Rationale: Must be before Phase 3 (server endpoints) because Phase 3 uses the
               same WS message type defined here.

Phase 3 — Server Inventory + Combo Validation
    Files: crates/racecontrol/src/game_inventory.rs (NEW)
           crates/racecontrol/src/combo_validator.rs (NEW)
           crates/racecontrol/src/ws/mod.rs (add 3 new message handlers)
           crates/racecontrol/src/preset_library.rs (add list_presets_for_pod, JOIN flags)
           crates/racecontrol/src/state.rs (pod_manifests type)
           DB migration: pod_game_inventory, combo_validation_flags
    Verify: cargo test -p racecontrol -- game_inventory combo_validator
    Deploy: Server only. No pod changes needed yet.
    Rationale: Server endpoints must exist before agents send GameInventoryUpdate at scale.
               Deploying server first means even if old agents don't send new messages,
               the server handles them correctly when agents are updated.

Phase 4 — Agent rc-agent main.rs + Retry Timeout + Tier Engine
    Files: crates/rc-agent/src/main.rs (replace scan_ac_content with build_full_manifest)
           crates/rc-agent/src/game_launch_retry.rs (wrap with timeout)
           crates/rc-agent/src/crash_loop_detector.rs (NEW)
           crates/rc-agent/src/launch_timeline.rs (NEW)
           crates/rc-agent/src/diagnostic_engine.rs (add new trigger emission)
           crates/rc-agent/src/tier_engine.rs (add CrashLoop + GameLaunchTimeout arms)
    Verify: cargo test -p rc-agent (all existing tests pass)
    Deploy: Pod 8 canary first. Verify:
            - Server receives GameInventoryUpdate with non-empty installed_games
            - Server logs show combo_validator ran
            - game_launch triggers timeout on forced-slow launch (manual test)
    Rationale: This is the most change-dense phase in rc-agent. Canary-first is critical.
               Phase 3 (server) must be live first — if old agents still on Phase 2 code
               continue running, no WS messages are lost or rejected.

Phase 5 — Fleet-Wide Agent Deploy
    Files: None new — Phase 4 binary to all pods
    Deploy: All 8 pods + POS
    Verify: Fleet health check — all pods show GameInventoryUpdate received
            GET /api/v1/fleet/game-matrix returns populated matrix
    Rationale: Fleet deploy after canary validation is standing rule (Pod 8 first).

Phase 6 — Frontend: Admin Reliability Dashboard
    Files: racingpoint-admin/src/app/reliability/page.tsx (NEW)
           Extend /api/v1/fleet/game-matrix endpoint if not yet done
           GET /api/v1/launch-timeline endpoint in api/metrics.rs
    Verify: UI review gate (gsd-ui-auditor per standing rules)
            fleet matrix renders all 8 pods × 8 games
            flagged combos list shows entries from combo_validation_flags
    Rationale: Frontend last — depends on all backend endpoints being live.

Phase 7 — Frontend: Kiosk Game Filtering
    Files: kiosk/src/components/GamePickerPanel.tsx (minor)
           kiosk/src/app/book/page.tsx (pass installed_games from pod state)
    Verify: UI review gate
            Book a pod — only games installed on that pod show in picker
            Flagged preset shows warning badge
    Rationale: Kiosk changes are highest customer-impact — validate last with full
               backend data in place.
```

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Rewriting ContentManifest Instead of Extending

**What people do:** Replace `AgentMessage::ContentManifest(ContentManifest)` with a new `GameInventoryUpdate` and stop sending `ContentManifest`.

**Why it's wrong:** The server's `ws/mod.rs` already handles `ContentManifest` (line 920), stores it in `pod_manifests`, and calls `log_pod_activity`. Old rc-agent binaries on pods that haven't been updated yet still send `ContentManifest`. Removing the variant breaks backward compatibility for a mixed fleet during rolling deploy.

**Do this instead:** Keep `ContentManifest` handling unchanged. Add `GameInventoryUpdate` as a NEW variant. The server handles both — old agents get `ContentManifest` handling, new agents get `GameInventoryUpdate` handling. After all pods are on the new binary, `ContentManifest` can be deprecated (but not removed until next major milestone).

### Anti-Pattern 2: Running Boot Validation as a Blocking Call in the WS Reconnect Path

**What people do:** Call `game_doctor::run_boot_validation(&manifest)` synchronously in the WS reconnect handler and wait for it to complete before sending the manifest.

**Why it's wrong:** Boot validation walks the filesystem for every AC preset (car folder, track folder, ai_lines, pit stalls). With 50+ AC presets, this could take 500ms-2s. The WS reconnect must be fast — the server considers a pod offline until it receives `Register`. Blocking here delays all other pod-to-server sync.

**Do this instead:** Run `build_full_manifest()` synchronously (fast, just `read_dir` calls). Send `GameInventoryUpdate` with `combo_flags: vec![]` (empty). Then spawn `tokio::task::spawn_blocking(|| game_doctor::run_boot_validation(&manifest))` in a background task. When it completes, send a second `AgentMessage::ComboValidationResult { pod_id, flags }`. The server handles both messages independently.

### Anti-Pattern 3: Crash Loop State in Memory Only

**What people do:** Store crash loop counts in a `RwLock<HashMap<SimType, u32>>` in rc-agent's AppState.

**Why it's wrong:** rc-agent restarts frequently (crash recovery, OTA deploy, MAINTENANCE_MODE clear). In-memory state is reset on every restart. A crash loop that crosses a restart boundary (game crashes 2x, rc-agent restarts, game crashes 1x more) is NOT detected because the counter resets.

**Do this instead:** Persist crash loop state to `C:\RacingPoint\crash_loop_state.json` (same pattern as `watchdog-survival.json`). Read on startup, write on every failure. Clear the rolling window entries older than 10 minutes on read (lazy expiry). This is what `crash_loop_detector.rs` implements.

### Anti-Pattern 4: Per-Pod Preset Filtering in the Kiosk API Call

**What people do:** Kiosk calls `/api/v1/presets` and then client-side filters by `pod.installed_games`.

**Why it's wrong:** The kiosk already receives `installed_games` from pod state. But the filtering also needs to respect `combo_validation_flags` (auto-disabled presets should not appear). Client-side filtering requires the kiosk to know the flag logic. Two sources of truth for what is "launchable."

**Do this instead:** Server-side `list_presets_for_pod(pod_id)` does BOTH filters: (1) sim_type must be in `pod_game_inventory` for that pod, (2) preset must not have an active `combo_validation_flags.resolved_at IS NULL AND auto_disabled=1` row. The kiosk just shows what the server returns. No client-side filter logic.

### Anti-Pattern 5: Adding GameLaunchTimeout as a Timer in game_launcher.rs (Server Side)

**What people do:** Server-side `game_launcher.rs` starts a 90-second timer when it sends `LaunchGame` to the agent. If no `GameStateUpdate::Launched` comes back, server declares timeout.

**Why it's wrong:** The server already has a `GameTracker` state machine stuck-in-Launching problem (v40.0 bug). Adding ANOTHER timer on the server adds a second overlapping timeout. The agent is closer to the actual launch — it knows when `acs.exe` is expected to appear, not just when the WS command was sent.

**Do this instead:** Agent-side timeout (Phase 4 above). The agent wraps `retry_game_launch()` with `tokio::time::timeout(90s)`. On timeout: emits `GameLaunchTimeout` trigger locally, sends `GameStateUpdate::Failed` to server (existing message). Server's GameTracker transitions from `Launching` to `Failed` on this message — no new server-side timer needed.

---

## Scaling Considerations

Racing Point is a fixed fleet (8 pods, 1 server). These considerations are for operational scale only.

| Concern | At 8 pods | At 3 venues (24 pods) |
|---------|-----------|----------------------|
| `pod_game_inventory` table size | 8 × 8 = 64 rows max | 24 × 8 = 192 rows — trivially small |
| `combo_validation_flags` write rate | Only on WS connect/reconnect (rare) | Same rate per pod; no fan-out issue |
| `launch_timeline_spans` write rate | ~10 spans × N launches/day | Add `venue_id` column when multi-venue ships |
| Boot validation filesystem cost | 50 presets × 4 checks = 200 `metadata()` calls, ~50-100ms | Same cost per pod; no server load |
| Fleet game matrix query | 1 query spanning 64 rows | Add index on `(pod_id, sim_type)` when >8 pods |

---

## Sources

- Direct code inspection: `crates/rc-agent/src/content_scanner.rs` — AC-only scope, std::fs pattern, existing call site in main.rs:1950
- Direct code inspection: `crates/rc-agent/src/game_doctor.rs` — diagnose_and_fix(), reactive-only, AC_CONTENT_PATH constant
- Direct code inspection: `crates/rc-agent/src/tier_engine.rs` — DiagnosticTrigger enum, existing match arms, GameLaunchFail arm
- Direct code inspection: `crates/rc-agent/src/game_launch_retry.rs` — retry structure, TOTAL_TIMEOUT_SECS=60, MAX_RETRY_ATTEMPTS=2
- Direct code inspection: `crates/rc-agent/src/diagnostic_engine.rs` — full DiagnosticTrigger enum (lines 49-108), emission paths
- Direct code inspection: `crates/racecontrol/src/ws/mod.rs:920-929` — ContentManifest handler, pod_manifests.write()
- Direct code inspection: `crates/racecontrol/src/api/routes.rs:5402-5445` — games_catalog, installed_games usage
- Direct code inspection: `crates/racecontrol/src/api/metrics.rs:427-490` — query_launch_matrix, combo_reliability table schema
- Direct code inspection: `crates/racecontrol/src/api/metrics.rs:595-641` — launch_events schema, combo_reliability schema
- Direct code inspection: `crates/racecontrol/src/preset_library.rs:23,38,82-107` — list_presets_with_reliability, reliability JOIN
- Direct code inspection: `crates/racecontrol/src/state.rs:135,189` — AppState, pod_manifests field type
- Direct code inspection: `crates/rc-common/src/types.rs:83-122,897-1031` — PodInfo.installed_games, ContentManifest, GamePresetWithReliability
- Direct code inspection: `crates/rc-common/src/protocol.rs:83-147,628,881` — AgentMessage enum, CoreToAgentMessage, ContentManifest variant
- Direct code inspection: `kiosk/src/components/GamePickerPanel.tsx:53-58` — installedGames prop, GAME_DISPLAY filter
- Direct code inspection: `kiosk/src/components/GameCatalogLoader.tsx` — loadGameCatalog pattern
- `.planning/PROJECT.md` — v41.0 constraints, target features, existing architecture description
- `.planning/research/STACK-v41.md` — stack decisions for this milestone (no new deps)

---
*Architecture research for: v41.0 Game Intelligence System*
*Researched: 2026-04-03 IST*
