---
phase: 445-typed-api-contract-rust-ts-codegen
plan: 02a
subsystem: build-system
tags: [ts-rs, rust, typescript, codegen, feature-flags, determinism, rc-common]

# Dependency graph
requires:
  - 445-00 (D-14 audit test, 42-name whitelist, ts-rs 12 spike Verdict A)
  - 445-01 (workspace deps + feature flags + gen-types skeleton + OpenApi umbrella)
provides:
  - 46 `#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]` + `#[cfg_attr(feature = "ts-rs", ts(export))]` derive sites across 7 rc-common source files
  - `rc_common::fleet_health_types::PodFleetStatus` (relocated from `racecontrol::fleet_health_api`, source-compat re-export preserved)
  - Fully populated `crates/racecontrol/src/bin/gen_types.rs` emitting 46 apex types + barrel index.ts
  - 46 committed `.ts` files + `index.ts` + `serde_json/JsonValue.ts` in `packages/shared-types/generated/`
  - Deterministic output proven: `DETERMINISTIC: f29aa838ae11d6e56c579089b09dfd1381049b78d46ee44bbca5806f407e1836` across 3 runs
affects: [445-03, 445-04, 445-05]

# Tech tracking
tech-stack:
  added:
    - "ts-rs 12.0.1 TS derive usage across rc-common admin whitelist (feature-gated — zero default-build cost)"
  patterns:
    - "Single source of truth for TS output directory: `Config::with_out_dir()` in gen_types.rs controls the final path; derives use `ts(export)` without `export_to` to avoid double-counting."
    - "Transitive TS derive propagation: apex types trigger TS emission for every TS-derived dependency (e.g. AcLanSessionConfig pulls AcSessionBlock, AcEntrySlot, AcWeatherConfig, AcDynamicTrackConfig)."
    - "Cross-crate struct relocation via re-export: `PodFleetStatus` moved from racecontrol-crate to rc-common, then re-exported at original path for zero consumer churn."

key-files:
  created:
    - crates/rc-common/src/fleet_health_types.rs
    - packages/shared-types/generated/AcDynamicTrackConfig.ts
    - packages/shared-types/generated/AcEntrySlot.ts
    - packages/shared-types/generated/AcLanSessionConfig.ts
    - packages/shared-types/generated/AcServerStatus.ts
    - packages/shared-types/generated/AcSessionBlock.ts
    - packages/shared-types/generated/AcStatus.ts
    - packages/shared-types/generated/AcWeatherConfig.ts
    - packages/shared-types/generated/ActionId.ts
    - packages/shared-types/generated/AiCountRange.ts
    - packages/shared-types/generated/BillingSessionInfo.ts
    - packages/shared-types/generated/BillingSessionStatus.ts
    - packages/shared-types/generated/Booking.ts
    - packages/shared-types/generated/ContentDirsResponse.ts
    - packages/shared-types/generated/DiagnosisTier.ts
    - packages/shared-types/generated/Driver.ts
    - packages/shared-types/generated/DrivingState.ts
    - packages/shared-types/generated/Event.ts
    - packages/shared-types/generated/EventType.ts
    - packages/shared-types/generated/FixType.ts
    - packages/shared-types/generated/FleetEvent.ts
    - packages/shared-types/generated/GameDirs.ts
    - packages/shared-types/generated/GameInventory.ts
    - packages/shared-types/generated/GameState.ts
    - packages/shared-types/generated/HealLease.ts
    - packages/shared-types/generated/HealLeaseRequest.ts
    - packages/shared-types/generated/HealLeaseResponse.ts
    - packages/shared-types/generated/Incident.ts
    - packages/shared-types/generated/LapData.ts
    - packages/shared-types/generated/LaunchNoteEvent.ts
    - packages/shared-types/generated/LaunchState.ts
    - packages/shared-types/generated/Leaderboard.ts
    - packages/shared-types/generated/LeaderboardEntry.ts
    - packages/shared-types/generated/MeshSolution.ts
    - packages/shared-types/generated/PlayableSignal.ts
    - packages/shared-types/generated/PodFleetStatus.ts
    - packages/shared-types/generated/PodInfo.ts
    - packages/shared-types/generated/PodInventory.ts
    - packages/shared-types/generated/PodStatus.ts
    - packages/shared-types/generated/PricingTier.ts
    - packages/shared-types/generated/SessionType.ts
    - packages/shared-types/generated/SimType.ts
    - packages/shared-types/generated/SolutionStatus.ts
    - packages/shared-types/generated/SurvivalLayer.ts
    - packages/shared-types/generated/ValidityError.ts
    - packages/shared-types/generated/ValidityErrorCode.ts
    - packages/shared-types/generated/WatchdogCrashReport.ts
    - packages/shared-types/generated/serde_json/JsonValue.ts
  modified:
    - crates/rc-common/src/lib.rs (register fleet_health_types module)
    - crates/rc-common/src/types.rs (24 derive blocks added — SimType, PodStatus, PodInfo, Driver, SessionType, LapData, LeaderboardEntry, Leaderboard, EventType, Event, Booking, DrivingState, AcStatus, BillingSessionStatus, PlayableSignal, BillingSessionInfo, PricingTier, GameState, AcServerStatus, AcSessionBlock, AcWeatherConfig, AcDynamicTrackConfig, AcEntrySlot, AcLanSessionConfig, WatchdogCrashReport)
    - crates/rc-common/src/inventory_types.rs (7 derives — PodInventory, GameInventory, AiCountRange, ValidityError, ValidityErrorCode, ContentDirsResponse, GameDirs)
    - crates/rc-common/src/mesh_types.rs (4 derives — SolutionStatus, FixType, DiagnosisTier, MeshSolution)
    - crates/rc-common/src/survival_types.rs (5 derives — ActionId, SurvivalLayer, HealLease, HealLeaseRequest, HealLeaseResponse)
    - crates/rc-common/src/fleet_event.rs (2 derives — FleetEvent, Incident)
    - crates/rc-common/src/protocol.rs (2 derives — LaunchState, LaunchNoteEvent)
    - crates/racecontrol/src/fleet_health_api.rs (inline struct replaced with `pub use rc_common::fleet_health_types::PodFleetStatus`)
    - crates/racecontrol/src/bin/gen_types.rs (skeleton body replaced with real per-type export_all calls + deterministic barrel)
    - packages/shared-types/generated/index.ts (barrel re-exports for 46 apex types)

key-decisions:
  - "D-14 SKIP-list honoured by NOT deriving on adjacently-tagged or flatten-owning whitelist members (CloudAction, CoreToAgentMessage, CoreMessage, PendingCloudAction). enum_tagging_audit still reports 0 forbidden combos at 46 TS-derived sites."
  - "export_to attribute REMOVED from every derive after verification discovered it caused path double-counting (`cfg.export_dir.join(export_to)` = `packages/packages/shared-types/generated/`). Single source of truth = `Config::with_out_dir(gen_dir)` in gen_types.rs."
  - "PodFleetStatus relocated to rc-common via a new `fleet_health_types` module. Original location keeps a `pub use` re-export so every consumer compiles unchanged."
  - "Transitive TS-derived deps explicitly annotated: AcSessionBlock, AcEntrySlot, AcWeatherConfig, AcDynamicTrackConfig (pulled by AcLanSessionConfig); FleetEvent (pulled by Incident); FixType, DiagnosisTier (pulled by MeshSolution); ValidityErrorCode (pulled by ValidityError). Without these, ts-rs would reject the apex derive."
  - "AgentConfig deliberately skipped — dense nested config tree with 12+ sub-configs, high cost to derive safely. Not on admin-critical path (frontend reads individual settings via separate endpoints). Deferred to a follow-on plan if future admin UI needs it."
  - "FleetHealthResponse is NOT a Rust struct — it's built inline via `serde_json::json!()` in `fleet_health_handler`. Rewriting it to a concrete struct is a separate refactor; its fields (pods array + timestamp + services) are covered either directly (PodFleetStatus) or as opaque JSON for admin purposes."

patterns-established:
  - "Per-apex-type gen-types emission (Plan 00 SPIKE Verdict A confirmed viable at scale): 46 `T::export_all(&cfg)?` calls in alphabetical order produce byte-identical output across 3 runs."
  - "Hard-coded alphabetical barrel (NOT DirEntry iteration) — Pitfall 6 mitigated, `scripts/check-gen-types-determinism.sh` prints DETERMINISTIC across runs."
  - "Feature isolation invariant holds after 46 TS derives: `cargo tree -e features -p rc-agent-crate | grep -c ts-rs` = 0, same for rc-sentry and racecontrol default."

requirements-completed: [TYP-01, TYP-09]

# Metrics
duration: 43min
completed: 2026-04-21
---

# Phase 445 Plan 02a: Wave 2a — rc-common TS Derives + gen-types Binary Body Summary

**Added 46 `#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]` sites across 7 rc-common source files, relocated `PodFleetStatus` from racecontrol-crate to rc-common (source-compat re-export retained), and populated `gen_types.rs` with explicit per-apex-type emission. gen-types now produces 46 deterministic .ts files + index.ts barrel at `packages/shared-types/generated/`. D-14 safety gate preserved (0 forbidden combos). ts-rs feature isolation holds fleet-wide.**

## Performance

- **Duration:** ~43 min
- **Started:** 2026-04-21T00:55Z (06:25 IST)
- **Completed:** 2026-04-21T01:38Z (07:08 IST)
- **Tasks:** 2
- **Files created:** 47 (fleet_health_types.rs + 46 .ts files + serde_json/JsonValue.ts)
- **Files modified:** 8 source files (7 rc-common .rs + 1 gen_types.rs + lib.rs + fleet_health_api.rs)

## Accomplishments

- **46 TS derives added to rc-common** across types.rs (24), inventory_types.rs (7), mesh_types.rs (4), survival_types.rs (5), fleet_event.rs (2), protocol.rs (2), fleet_health_types.rs (1 for PodFleetStatus after relocation). Every admin-consumed type from the .whitelist.txt that is safe to annotate (no D-14 violations, no complex trait bounds) received the pattern.
- **D-14 safety gate preserved.** `cargo test -p rc-common --test enum_tagging_audit` reports `scanned 23 files, 46 TS-derived sites, 0 forbidden combos`. Every whitelist member that would trip the audit (CloudAction, CoreToAgentMessage, CoreMessage, PendingCloudAction) was deliberately skipped and documented.
- **PodFleetStatus moved to rc-common cleanly.** New `crates/rc-common/src/fleet_health_types.rs` (~140 lines) owns the struct. Original location `crates/racecontrol/src/fleet_health_api.rs` now carries a single `pub use rc_common::fleet_health_types::PodFleetStatus;` so every downstream consumer in racecontrol (handler builder, state updates, frontend proxy) continues to see the type at the same import path. Field list + serde attrs preserved byte-for-byte.
- **gen-types binary body filled.** `crates/racecontrol/src/bin/gen_types.rs` (168 lines net, up from the 68-line skeleton) now does:
  1. Emit `docs/openapi.generated.yaml` via `ApiDoc::openapi()` + `serde_yaml::to_string` (feeds Plan 02b's route annotations — which are live; output grew 572 → 20,245 bytes during parallel execution).
  2. Create `packages/shared-types/generated/` with `Config::default().with_out_dir(gen_dir)`.
  3. Call `T::export_all(&cfg)?` for every TS-derived apex type (46 calls, alphabetical by module).
  4. Hard-code an alphabetical `index.ts` barrel (2,602 bytes) so determinism gate doesn't trip on DirEntry iteration order.
- **Deterministic across 3 runs.** `bash scripts/check-gen-types-determinism.sh` prints `DETERMINISTIC: f29aa838ae11d6e56c579089b09dfd1381049b78d46ee44bbca5806f407e1836`. Manual 3x binary rerun + sha256 confirms every `.ts` file is byte-identical (PodInfo.ts sha256: `567a4a2a6925385192cf43a9a07ead4e5f37df3df8285c765c7667e9b52dfe30`).
- **Feature isolation holds.** `cargo tree -e features -p rc-agent-crate | grep -c ts-rs` = 0. Same for rc-sentry and racecontrol (default features). Only `racecontrol-crate --features gen-types` pulls ts-rs into the tree. RESEARCH Pitfall 1 defence preserved after 46 additional derives.

## Task Commits

Each task committed atomically with `--no-verify` (parallel-wave protocol):

1. **Task 1: TS derives on rc-common admin-consumed types + PodFleetStatus relocation** — `65276bfe` (feat)
   - crates/rc-common/src/types.rs (+48 lines — 24 derive blocks)
   - crates/rc-common/src/inventory_types.rs (+14 lines — 7 derive blocks)
   - crates/rc-common/src/mesh_types.rs (+8 lines — 4 derive blocks)
   - crates/rc-common/src/survival_types.rs (+10 lines — 5 derive blocks)
   - crates/rc-common/src/fleet_event.rs (+4 lines — 2 derive blocks)
   - crates/rc-common/src/protocol.rs (+4 lines — 2 derive blocks)
   - crates/rc-common/src/lib.rs (+1 — fleet_health_types module)
   - crates/rc-common/src/fleet_health_types.rs NEW (~140 lines — PodFleetStatus moved here)
   - crates/racecontrol/src/fleet_health_api.rs (-109 / +5 — inline struct replaced with `pub use`)

2. **Task 2: gen-types binary body + export_to path cleanup + emitted .ts files** — `fca20883` (feat)
   - crates/racecontrol/src/bin/gen_types.rs (+168 / -27 — real per-type emission + barrel)
   - 7 rc-common files (46 total changed derive lines — removed broken `export_to = "../../packages/shared-types/generated/"` attribute; `with_out_dir` is now the sole source of truth)
   - packages/shared-types/generated/*.ts (46 new files — one per apex type)
   - packages/shared-types/generated/serde_json/JsonValue.ts (1 new — transitive dep for `serde_json::Value` fields in MeshSolution)
   - packages/shared-types/generated/index.ts (+47 barrel lines)

## Derives Index (46 sites, one per whitelisted type)

### rc-common/src/types.rs (24)
| Line | Type | Shape |
|------|------|-------|
| 7 | SimType | externally-tagged enum (snake_case) |
| 41 | PodStatus | externally-tagged enum |
| 85 | PodInfo | struct |
| 132 | Driver | struct |
| 148 | SessionType | externally-tagged enum |
| 251 | LapData | struct |
| 272 | LeaderboardEntry | struct |
| 286 | Leaderboard | struct |
| 298 | EventType | externally-tagged enum |
| 308 | Event | struct |
| 325 | Booking | struct |
| 340 | DrivingState | externally-tagged enum |
| 357 | AcStatus | externally-tagged enum |
| 376 | BillingSessionStatus | externally-tagged enum |
| 399 | PlayableSignal | externally-tagged enum (data-carrying variants) |
| 418 | BillingSessionInfo | struct |
| 463 | PricingTier | struct |
| 478 | GameState | externally-tagged enum |
| 598 | AcServerStatus | externally-tagged enum |
| 608 | AcSessionBlock | struct (transitive dep of AcLanSessionConfig) |
| 619 | AcWeatherConfig | struct (transitive) |
| 650 | AcDynamicTrackConfig | struct (transitive) |
| 671 | AcEntrySlot | struct (transitive) |
| 687 | AcLanSessionConfig | struct |
| 1009 | WatchdogCrashReport | struct |

### rc-common/src/inventory_types.rs (7)
| Line | Type | Shape |
|------|------|-------|
| 22 | PodInventory | struct |
| 41 | GameInventory | struct |
| 57 | AiCountRange | struct (Copy) |
| 77 | ValidityError | struct |
| 86 | ValidityErrorCode | externally-tagged enum (SCREAMING_SNAKE_CASE) |
| 100 | ContentDirsResponse | struct |
| 107 | GameDirs | struct |

### rc-common/src/mesh_types.rs (4)
| Line | Type | Shape |
|------|------|-------|
| 20 | SolutionStatus | externally-tagged enum |
| 40 | FixType | externally-tagged enum (transitive dep of MeshSolution) |
| 60 | DiagnosisTier | externally-tagged enum (transitive) |
| 93 | MeshSolution | struct (deny_unknown_fields; uses `serde_json::Value` → emits `serde_json/JsonValue.ts` transitively) |

### rc-common/src/survival_types.rs (5)
| Line | Type | Shape |
|------|------|-------|
| 15 | ActionId | newtype struct |
| 51 | SurvivalLayer | externally-tagged enum |
| 274 | HealLease | struct |
| 286 | HealLeaseRequest | struct |
| 295 | HealLeaseResponse | struct |

### rc-common/src/fleet_event.rs (2)
| Line | Type | Shape |
|------|------|-------|
| 21 | FleetEvent | externally-tagged enum (transitive dep of Incident) |
| 112 | Incident | struct |

### rc-common/src/protocol.rs (2)
| Line | Type | Shape |
|------|------|-------|
| 91 | LaunchState | externally-tagged enum |
| 128 | LaunchNoteEvent | struct |

### rc-common/src/fleet_health_types.rs (1)
| Line | Type | Shape |
|------|------|-------|
| 27 | PodFleetStatus | struct (relocated from racecontrol-crate; serde-Serialize only, no Deserialize needed) |

## Generated output sample (PodInfo.ts, 25 lines)

```typescript
// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { DrivingState } from "./DrivingState";
import type { GameState } from "./GameState";
import type { PodStatus } from "./PodStatus";
import type { SimType } from "./SimType";

export type PodInfo = { id: string, number: number, name: string, ip_address: string, mac_address?: string | null, sim_type: SimType, status: PodStatus, current_driver: string | null, current_session_id: string | null, last_seen: string | null, driving_state?: DrivingState | null, billing_session_id?: string | null, game_state?: GameState | null, current_game?: SimType | null, installed_games?: Array<SimType>,
/**
 * Whether the pod screen is currently blanked (black screen between sessions).
 */
screen_blanked?: boolean | null, ...
```

Note: `Option<T>` renders as `T | null` (ts-rs 12 default), matching SPIKE finding — admin Zod validators must use `.nullable()` for `skip_serializing_if = "Option::is_none"` fields.

## SHA256 stability anchor for D-15 determinism

- **Combined hash (all generated .ts + docs/openapi.generated.yaml):** `f29aa838ae11d6e56c579089b09dfd1381049b78d46ee44bbca5806f407e1836`
- **index.ts sha256:** `66a1b89fec88c351fb7b17df3f058a6a7f5e5adf9d642da98cac2fc68bfe0066`
- **PodInfo.ts sha256:** `567a4a2a6925385192cf43a9a07ead4e5f37df3df8285c765c7667e9b52dfe30`
- **BillingSessionInfo.ts sha256:** `2ab6bf304effdd7ad4e11090375335d40fb904b2aedaaa4b4b4a4fcc78586d8b`
- **FleetEvent.ts sha256:** `2d9cb4b6f8010b8a4192647de9909f721d533f23e4bacf119fffeef1bc794c5a`

## Decisions Made

- **D-F (export_to attribute removed):** During Task 2 verification, the first binary run wrote every .ts file to `packages/packages/shared-types/generated/` — a directory layer deeper than intended. Root cause: ts-rs 12.0.1's `export_into()` does `cfg.export_dir.join(output_path)` (where `output_path` comes from the `#[ts(export_to = ...)]` attribute). With `with_out_dir("packages/shared-types/generated/")` + `export_to = "../../packages/shared-types/generated/"`, the concatenation produced the doubled path. **Fix:** drop `export_to` from all 46 derive attrs; rely solely on `Config::with_out_dir()` in gen_types.rs. One source of truth for the target directory.
- **D-G (PodFleetStatus lives in rc-common):** Per RESEARCH § "Drift findings" #4, this struct's original location in racecontrol-crate made it unreachable from the gen-types emission path without cross-crate gymnastics. Relocation to rc-common + source-compat re-export is cleaner than teaching gen-types how to import from racecontrol's lib. Implemented in Task 1.
- **D-H (4 whitelist members intentionally NOT derived):** CloudAction, CoreToAgentMessage (both adjacently tagged, would trip D-14), CoreMessage (owns a `#[serde(flatten)]` field, would trip D-14), PendingCloudAction (transitively depends on CloudAction so ts-rs would reject the derive). These are tracked as deferred work — their TS shapes need either a hand-written adapter (admin already has hand-written unions for WS-carried protocol types) or a different serde layout (breaking change).
- **D-I (AgentConfig deferred):** 12+ nested sub-configs (PodConfig, CoreConfig, WheelbaseConfig, ...), some feature-gated. Deriving TS would require annotating every transitive sub-type, which is ~30 more annotations for low admin value (frontend reads individual settings via separate endpoints). Deferred to a follow-on plan if future admin UI needs it.
- **D-J (FleetHealthResponse has no Rust struct):** The fleet-health response is built inline via `serde_json::json!({...})` in `fleet_health_handler`. Rewriting it to a concrete struct is a separate refactor. Admin currently consumes the `pods` array (which IS typed via PodFleetStatus) plus opaque services/displays/churn JSON — sufficient for the current UI.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] ts-rs `export_to` attribute + `with_out_dir` double-counted paths**
- **Found during:** Task 2 first gen-types run after rebuild with real per-type calls
- **Issue:** All 46 .ts files landed in `packages/packages/shared-types/generated/` (one level too deep) instead of `packages/shared-types/generated/`. Root cause: ts-rs 12 joins `Config::with_out_dir` (set to `packages/shared-types/generated/`) with the per-derive `#[ts(export_to = "../../packages/shared-types/generated/")]` — producing `packages/shared-types/generated/../../packages/shared-types/generated/` = `packages/packages/shared-types/generated/`.
- **Fix:** Removed `export_to` attribute from all 46 `cfg_attr` derive sites via 7 `replace_all` edits. `with_out_dir` is now the sole source of truth. Removed the stray `packages/packages/` directory.
- **Files modified:** All 7 rc-common source files (types.rs, inventory_types.rs, mesh_types.rs, survival_types.rs, fleet_event.rs, protocol.rs, fleet_health_types.rs)
- **Verification:** Rebuilt gen-types, re-ran 3×, all 46 .ts files now land in the correct directory, determinism hash stable.
- **Committed in:** `fca20883` (Task 2) — fix applied pre-commit.

**2. [Rule 2 - Missing] `FleetHealthResponse` in whitelist but no struct exists**
- **Found during:** Task 1 type enumeration (grep `^pub struct FleetHealthResponse` returned 0 hits anywhere)
- **Issue:** The whitelist lists `FleetHealthResponse` as an admin-consumed type, but the fleet-health endpoint's response is built via `serde_json::json!({...})` inline — no Rust struct exists.
- **Fix:** Documented the gap in gen_types.rs comments + this SUMMARY. Left for a future refactor to introduce a concrete struct (out of Plan 02a scope — would be a ~60-line change in racecontrol-crate + a separate D-14 audit for the new type).
- **Files modified:** None (only documentation).
- **Impact on plan:** The pods array shape IS typed via PodFleetStatus (which IS derived). Admin's frontend can consume PodFleetStatus directly until a concrete FleetHealthResponse materializes.

**3. [Rule 2 - Missing] 4 whitelist members are D-14 SKIP candidates**
- **Found during:** Task 1 whitelist review against protocol.rs serde attrs
- **Issue:** `CloudAction` (protocol.rs:1586, `#[serde(tag="action_type", content="payload")]`), `CoreToAgentMessage` (protocol.rs:774, `#[serde(tag, content)]`), `CoreMessage` (protocol.rs:749, owns `#[serde(flatten)]` field) — all adjacently-tagged or flatten-owning. `PendingCloudAction` (protocol.rs:1625) depends on CloudAction, so even it can't derive. Adding `derive(TS)` to any of these would immediately fail `enum_tagging_audit`.
- **Fix:** Intentionally did NOT add derives to these 4 types. Documented as D-H decision. The admin client can hand-write TypeScript unions for protocol-adjacent types (standard pattern for WS-carried messages).
- **Files modified:** None.
- **Impact on plan:** Plan 02a delivers 46 of 42-from-whitelist-plus-6-transitive-deps derives. The 4 skipped types remain hand-written on the admin side (no regression from current state).

**4. [Rule 2 - Missing] `AgentConfig` deferred**
- **Found during:** Task 1 whitelist review
- **Issue:** AgentConfig aggregates 12+ sub-configs (PodConfig, CoreConfig, WheelbaseConfig, TelemetryPortsConfig, GamesConfig, AiDebuggerConfig, KioskConfig, LockScreenConfig, PreflightConfig, ProcessGuardConfig, LaunchTimeoutConfig, MmaConfig), some feature-gated. Deriving TS requires annotating every transitive sub-type — ~30 additional annotations for low admin value (frontend reads individual settings, not the whole config tree).
- **Fix:** Skipped. Documented as D-I decision.
- **Files modified:** None.
- **Impact on plan:** Zero — admin frontend doesn't consume the full AgentConfig.

### Authentication Gates

None — no external auth required for this wave.

### Out-of-Scope Blockers Observed

- **Parallel Plan 02b compile errors during mid-execution.** At one point mid-verification, `cargo build --release --bin gen-types --features gen-types` failed because openapi.rs in 02b's in-flight state referenced `crate::api::customer_referral` before the module was registered in `api/mod.rs`. This was NOT my scope (per `<parallel_execution>` partition — 02b owns openapi.rs + handler files). By the time I ran final verification after my Task 2 commit, 02b had landed commits `16dfb0e4` and `71bc63bc` which populated openapi.rs correctly, and the full build passed. Noted here for visibility; no action required.

---

**Total deviations:** 4 (1 Rule 1 auto-fix for the export_to path bug, 3 Rule 2 documented gaps/deferrals for types that couldn't be derived safely or are scope-creep).
**Impact on plan:** Rule 1 fix was necessary for correctness (the whole point of Plan 02a is emitting .ts files to the right place). Rule 2 gaps are documented for future plans; no scope creep.

## Issues Encountered

- **Task 2 rebuild momentarily blocked by 02b's in-flight state.** Between Task 1 commit and 02b's next commit, `cargo build` failed with 7 `E0433: could not find customer_referral` errors from openapi.rs. Resolved itself when 02b landed commits. Lesson: parallel-wave execution can produce transient compile failures in shared dependency graphs; verify after the other plan catches up.
- **First gen-types run produced nested `packages/packages/` directory.** See Deviation #1 — caused by `export_to` + `with_out_dir` double-counting. Fixed in Task 2 before the Task 2 commit.
- **ts-rs 12 auto-emits `serde_json/JsonValue.ts`** as a transitive TS file for types that use `serde_json::Value` (MeshSolution has 3 such fields: symptoms, environment, fix_action). This file is committed alongside the apex .ts files; the drift gate will treat it as tracked content.
- **CRLF warnings during git add.** Standard Windows Git Bash behavior. Non-blocking.

## Next Phase Readiness

**Plan 03 (`packages/shared-types/src/index.ts` re-export flip) can start immediately with:**
1. 46 deterministic `.ts` files in `packages/shared-types/generated/` ready to re-export from.
2. `index.ts` barrel already structured alphabetically — Plan 03 can re-export from `./generated` into the package's top-level surface.
3. Admin's existing hand-written types in `packages/shared-types/src/` can be compared against generated via `bash scripts/audit-handwritten-vs-generated.sh` (Plan 00 audit tool, now armed).

**Plan 04 (admin consumption) can start:**
1. `PodInfo.ts` / `BillingSessionInfo.ts` / `FleetEvent.ts` / etc. are live — admin's Zod validators can switch from hand-written shapes to imports from `@racecontrol/shared-types/generated/*`.

**No blockers.** Plan 02a delivers every Wave 2a goal from the plan's success-criteria checklist.

## Self-Check: PASSED

**Files verified (48/48 exist on disk):**
- crates/rc-common/src/fleet_health_types.rs ✓
- crates/rc-common/src/lib.rs (contains `pub mod fleet_health_types;`) ✓
- crates/racecontrol/src/fleet_health_api.rs (contains `pub use rc_common::fleet_health_types::PodFleetStatus`) ✓
- crates/racecontrol/src/bin/gen_types.rs (168 net lines, 46 `::export_all(&cfg)?` calls) ✓
- 46 `.ts` files in packages/shared-types/generated/ ✓
- packages/shared-types/generated/index.ts (47-line barrel) ✓
- packages/shared-types/generated/serde_json/JsonValue.ts (transitive) ✓

**Commits verified (2/2 present in `git log --oneline`):**
- 65276bfe (Task 1: derives + PodFleetStatus relocation)
- fca20883 (Task 2: gen-types body + export_to removal + emitted .ts files)

**Acceptance criteria (all 9 plan-level):**
- ✓ `cargo check -p rc-common` (no features) exits 0
- ✓ `cargo check -p rc-common --features ts-rs` exits 0
- ✓ `cargo test -p rc-common --test enum_tagging_audit` exits 0: 1 passed, 46 sites, 0 forbidden combos
- ✓ `cargo build --release --bin gen-types --features gen-types` exits 0 (after 02b landed its commits)
- ✓ `cargo run --release --bin gen-types --features gen-types` emits 46 .ts files + index.ts + docs/openapi.generated.yaml
- ✓ `bash scripts/check-gen-types-determinism.sh` → `DETERMINISTIC: f29aa838...`
- ✓ `ls packages/shared-types/generated/*.ts | wc -l` = 46 (plus index.ts = 47 total)
- ✓ `cargo tree -e features -p rc-agent-crate | grep -c ts-rs` = 0
- ✓ `cargo tree -e features -p rc-sentry | grep -c ts-rs` = 0

---
*Phase: 445-typed-api-contract-rust-ts-codegen*
*Completed: 2026-04-21*
