# Phase 445 Plan 03 — D-12 Drift Audit Report

**Date:** 2026-04-21 07:19 IST (Tuesday)
**Audit tool:** `bash scripts/audit-handwritten-vs-generated.sh` (exit code 1, see caveat below)
**Whitelist source:** `packages/shared-types/generated/.whitelist.txt` (42 entries)
**Generated files count:** 46 in `packages/shared-types/generated/*.ts` + 1 barrel `index.ts` + 1 transitive `serde_json/JsonValue.ts` (see Plan 02a SUMMARY)
**Pairs audited by script:** 11 (only types that have BOTH a generated file AND a hand-written occurrence)
**Parity OK per script:** 3 (GameState, PodStatus, SimType)
**Drift flagged per script:** 8 (reviewed manually below — many are script artifacts, not real drift)

## Script caveat

The script's `grep -l -E "\\b$TYPE_NAME\\b" src/*.ts | head -1` pattern finds the FIRST `src/*.ts` file that mentions the type anywhere (import, re-export, or doc comment). For types referenced inside `billing.ts` (DrivingState imported there), the grep points at `billing.ts` — the field-normalization then compares `BillingSession` struct fields vs. the target enum, producing spurious DRIFT for DrivingState, PlayableSignal, etc.

To correct for this, I inspected each `<src-file, generated-file>` pair manually by reading the canonical hand-written definition (searched `src/*.ts` for `export type $TYPE_NAME` or `export interface $TYPE_NAME`).

## Per-Type Audit

### PodInfo

- **Parity status:** MINOR DIFF
- **Hand-written source:** `packages/shared-types/src/pod.ts` lines 27-47 (exported as `Pod` — note: name mismatch)
- **Generated source:** `packages/shared-types/generated/PodInfo.ts`
- **Field-level diff:**
  - Hand-written field name is `Pod` (not `PodInfo`); the `<interfaces>` section of the plan explicitly lists `PodInfo` as the target migrated name.
  - Hand-written `mac_address?: string` vs generated `mac_address?: string | null` (Pitfall 2 class — admin must accept both).
  - Hand-written `current_driver?: string; current_session_id?: string; last_seen?: string` all optional/undefined; generated makes `current_driver: string | null, current_session_id: string | null, last_seen: string | null` REQUIRED with null.
  - Generated adds `agent_timestamp?: string | null` (new field RESIL-08).
  - Generated lacks nothing admin depends on (spot-check vs admin code below).
- **Verdict:** ACCEPTED — flip re-export. The admin hand-written type is `Pod`; the generated type is `PodInfo`. Task 2 must re-export `PodInfo` from `../generated/index` AND retain `Pod` alias (for backward-compat) until admin code switches to `PodInfo`. **Action in Task 2:** add `export type { PodInfo } from '../generated/index'` AND keep `export type { Pod } from './pod'`.

### PodStatus

- **Parity status:** PARITY OK (script confirmed)
- **Hand-written source:** `packages/shared-types/src/pod.ts` line 21
- **Generated source:** `packages/shared-types/generated/PodStatus.ts`
- **Field-level diff:** Both are `"offline" | "idle" | "in_session" | "error" | "disabled"` — identical string union.
- **Verdict:** OK — flip re-export to `../generated/index`.

### SimType

- **Parity status:** PARITY OK (script confirmed)
- **Hand-written source:** `packages/shared-types/src/pod.ts` lines 3-11
- **Generated source:** `packages/shared-types/generated/SimType.ts`
- **Field-level diff:** Both: `"assetto_corsa" | "assetto_corsa_evo" | "assetto_corsa_rally" | "iracing" | "le_mans_ultimate" | "f1_25" | "forza" | "forza_horizon_5"` — identical 8-variant union (Rust `#[serde(rename = ...)]` overrides for `iracing` + `f1_25` produce identical wire format).
- **Verdict:** OK — flip re-export to `../generated/index`.

### DrivingState

- **Parity status:** PARITY OK (manual review — script false-flagged DRIFT due to grep pointing at billing.ts)
- **Hand-written source:** `packages/shared-types/src/pod.ts` line 23
- **Generated source:** `packages/shared-types/generated/DrivingState.ts`
- **Field-level diff:** Both: `"active" | "idle" | "no_device"` — identical.
- **Verdict:** OK — flip re-export to `../generated/index`.

### GameState

- **Parity status:** MINOR DIFF (generated has 1 extra variant)
- **Hand-written source:** `packages/shared-types/src/pod.ts` line 25
- **Generated source:** `packages/shared-types/generated/GameState.ts`
- **Field-level diff:**
  - Hand-written: `"idle" | "launching" | "loading" | "running" | "stopping" | "error"` (6 variants)
  - Generated: `"idle" | "launching" | "loading" | "running" | "stopping" | "error" | "in_lobby"` (7 variants — adds `in_lobby` for MP sessions)
- **Impact:** Generated is SUPERSET of hand-written. Admin code that `switch`es on GameState will have a new variant to handle, but TypeScript allows superset widening in reading position. Admin CURRENTLY doesn't read `in_lobby` (pre-existing gap; post-Wave-3 admin work can switch on it).
- **Verdict:** ACCEPTED — flip re-export. Admin switch-statements may now have non-exhaustive warnings on `in_lobby`; `tsc --noEmit` will reveal which ones in Task 2. If any fail, fall back to BREAKING for GameState.

### BillingSessionInfo (new) / BillingSession (hand-written)

- **Parity status:** BREAKING DIFF at type-name level — distinct TS identifiers, cannot be a drop-in.
- **Hand-written source:** `packages/shared-types/src/billing.ts` lines 22-45 (exports `BillingSession`)
- **Generated source:** `packages/shared-types/generated/BillingSessionInfo.ts` (exports `BillingSessionInfo`)
- **Field-level diff:** Field list is near-identical (both have id, driver_id, driver_name, pod_id, pricing_tier_name, allocated_seconds, driving_seconds, remaining_seconds, status, driving_state, started_at, split_count, split_duration_minutes, current_split_number, elapsed_seconds, cost_paise, rate_per_min_paise, billing_mode, recovery_pause_seconds, between_games_idle_seconds). Pitfall 2 applies:
  - Hand-written `started_at?: string` (optional undefined) vs generated `started_at: string | null` (required null).
  - Hand-written `split_duration_minutes?: number` vs generated `split_duration_minutes: number | null` (required null).
  - Hand-written `cost_paise?: number` vs generated `cost_paise?: bigint | null` — **BIGINT drift**: Rust `u64` serializes as JSON number, but ts-rs 12 emits `bigint` for `u64`. Admin likely uses plain `number`; if any code does arithmetic on `cost_paise`, `number + bigint` is a type error.
  - Hand-written `rate_per_min_paise?: number` vs generated `rate_per_min_paise?: bigint | null` — same bigint issue.
- **Verdict:** HELD hand-written. Flipping would (a) change the exported NAME (`BillingSession` → `BillingSessionInfo`) and require admin code to rename every `BillingSession` reference — OUT OF SCOPE for 445 (Plan 03 is type-source swap only, no admin code changes per D-04); (b) bigint drift would break any arithmetic on cost_paise / rate_per_min_paise. **Migration deferred to a follow-on phase that also refactors admin to use `BillingSessionInfo` + handle bigint.**
- **Task 2 action:** ALSO export `BillingSessionInfo` from generated (for admin code that wants to opt-in early), but keep `BillingSession` re-export from `./billing`. This is pure additive — zero risk to admin tsc.

### BillingSessionStatus

- **Parity status:** BREAKING DIFF — variant count mismatch
- **Hand-written source:** `packages/shared-types/src/billing.ts` lines 9-20 (11 variants)
- **Generated source:** `packages/shared-types/generated/BillingSessionStatus.ts` (11 variants — see decoded union below)
- **Field-level diff:**
  - Hand-written (11): `"pending" | "waiting_for_game" | "active" | "paused_manual" | "paused_disconnect" | "paused_game_pause" | "completed" | "ended_early" | "cancelled" | "cancelled_no_playable" | "paused_crash_recovery"`
  - Generated (11): `"pending" | "waiting_for_game" | "active" | "paused_manual" | "paused_disconnect" | "paused_game_pause" | "completed" | "ended_early" | "cancelled" | "cancelled_no_playable" | "paused_crash_recovery"`
- **Variants:** IDENTICAL — 11/11 match, same order. Plan target noted "11 variants in both" — verified.
- **Verdict:** OK — flip re-export to `../generated/index`. Rust `#[serde(rename = ...)]` attrs produce exactly these snake_case wire values, verified byte-identical.

### PricingTier

- **Parity status:** ACCEPTED DRIFT — hand-written has extra admin-only field
- **Hand-written source:** `packages/shared-types/src/billing.ts` lines 47-56
- **Generated source:** `packages/shared-types/generated/PricingTier.ts`
- **Field-level diff:**
  - Hand-written: `id, name, duration_minutes, price_paise, is_trial, is_active, sort_order?`
  - Generated: `id, name, duration_minutes, price_paise, is_trial, is_active` (NO `sort_order`)
- **Impact:** Admin code that reads `.sort_order` on a PricingTier WOULD get `tsc` error "property 'sort_order' does not exist". This is a real regression vector. Checked admin usage via grep (see Task 2 verification): if zero admin code uses `sort_order`, flip is safe.
- **Verdict:** PENDING admin grep. **Task 2 action:** grep `racingpoint-admin/src/**/*.{ts,tsx}` for `sort_order` — if zero hits on a `PricingTier`-typed value, flip to generated. If hits found, HOLD hand-written for PricingTier and document for follow-on.

### Driver

- **Parity status:** ACCEPTED DRIFT — hand-written has extra admin-only field; bigint drift on totals
- **Hand-written source:** `packages/shared-types/src/driver.ts` lines 2-14
- **Generated source:** `packages/shared-types/generated/Driver.ts`
- **Field-level diff:**
  - Hand-written: `id, name, email?, phone?, steam_guid?, iracing_id?, total_laps: number, total_time_ms: number, created_at?, has_used_trial?`
  - Generated: `id, name, email: string | null, phone: string | null, steam_guid: string | null, iracing_id: string | null, total_laps: bigint, total_time_ms: bigint, created_at: string` (REQUIRED; not optional)
- **Missing from generated:** `has_used_trial?: boolean` (admin-computed field, not in Rust struct — kiosk API adds it at the handler layer).
- **Bigint drift:** `total_laps` and `total_time_ms` are `u64` in Rust → `bigint` in TS. Same class as BillingSessionInfo cost_paise. Any admin code doing `driver.total_laps + 1` will `tsc`-fail.
- **Verdict:** HELD hand-written. Admin relies on `has_used_trial` field and likely uses `total_laps` / `total_time_ms` as `number`. Flipping would break Zod validation AND arithmetic. Deferred to follow-on that handles bigint + adds `has_used_trial` to Rust struct.

### PodFleetStatus

- **Parity status:** ACCEPTED DRIFT — generated is SUPERSET
- **Hand-written source:** `packages/shared-types/src/fleet.ts` lines 2-28 (~20 fields)
- **Generated source:** `packages/shared-types/generated/PodFleetStatus.ts` (~31 fields)
- **Field-level diff:** Generated ADDS ~11 fields the hand-written doesn't have: `active_sentinels`, `bat_sha256`, `crash_loop`, `maintenance_flag`, `crashes_last_hour`, `clock_drift_secs` (bigint), `experience_score`, `experience_status`, `avg_ready_delay_ms`, `crash_recovery_count` (bigint), `windows_session_id`, `ws_reconnects_5m`, `ws_reconnect_count`, `silent_reconnect_suspected`, `active_session_id`, `stuck_session_candidate`. These are all ADDITIONS — no field removed.
- **Optionality drift:** hand-written has many `field?: T` (optional undefined); generated has `field: T | null` (required null). Pitfall 2.
- **Bigint drift on:** `uptime_secs`, `clock_drift_secs`, `crash_recovery_count`.
- **Impact:** Admin code reading existing fields: widened type is safe (all hand-written fields present in generated). Admin code doing arithmetic on `uptime_secs` (possible — "uptime in hours" calc) would break with bigint.
- **Verdict:** PENDING admin grep for uptime_secs arithmetic. If none found, flip. If found, HELD.

### FleetHealthResponse

- **Parity status:** NOT MIGRATED (no Rust struct)
- **Hand-written source:** `packages/shared-types/src/fleet.ts` lines 30-33 (exports `FleetHealthResponse`)
- **Generated source:** NONE. `generated/FleetHealthResponse.ts` does NOT exist. Per Plan 02a SUMMARY D-J decision: "FleetHealthResponse is NOT a Rust struct — it's built inline via `serde_json::json!()` in `fleet_health_handler`. Rewriting it to a concrete struct is a separate refactor."
- **Verdict:** HELD hand-written. No generated equivalent to flip to.

### ConfigMismatchDetected

- **Parity status:** NOT IN WHITELIST (hand-written only)
- **Hand-written source:** `packages/shared-types/src/fleet.ts` lines 40-47
- **Generated source:** NONE — `ConfigMismatchDetected` is a WS message type (per comment "Maps to Rust AgentMessage::ConfigMismatchDetected in rc-common/src/protocol.rs") and per D-19 **WS message types stay hand-written permanently**.
- **Verdict:** HELD hand-written (D-19 permanent).

### PodInventory

- **Parity status:** PARITY OK (verified manually vs generated)
- **Hand-written source:** NONE in `src/*.ts` (not previously hand-written; new type introduced by Plan 02a).
- **Generated source:** `packages/shared-types/generated/PodInventory.ts`
- **Verdict:** NEW — add export from `../generated/index`. No hand-written collision.

### GameInventory

- **Parity status:** PARITY OK (new generated)
- **Hand-written source:** NONE
- **Generated source:** `packages/shared-types/generated/GameInventory.ts`
- **Verdict:** NEW — add export from `../generated/index`.

### ContentDirsResponse

- **Parity status:** PARITY OK (new generated)
- **Hand-written source:** NONE
- **Generated source:** `packages/shared-types/generated/ContentDirsResponse.ts`
- **Verdict:** NEW — add export from `../generated/index`.

### GameDirs

- **Parity status:** PARITY OK (new generated)
- **Hand-written source:** NONE
- **Generated source:** `packages/shared-types/generated/GameDirs.ts`
- **Verdict:** NEW — add export from `../generated/index`.

### PlayableSignal

- **Parity status:** PARITY OK (new generated, not previously hand-written)
- **Hand-written source:** NONE (script false-flagged DRIFT due to grep pointing at billing.ts)
- **Generated source:** `packages/shared-types/generated/PlayableSignal.ts`
- **Verdict:** NEW — add export from `../generated/index`.

### AcLanSessionConfig / AcServerStatus / AcSessionBlock / AcWeatherConfig / AcDynamicTrackConfig / AcEntrySlot / AcStatus

- **Parity status:** NEW — no hand-written equivalents in `src/*.ts`
- **Generated source:** 7 files in `packages/shared-types/generated/Ac*.ts`
- **Admin consumption:** Unlikely — these are MP/LAN server config types rarely touched by admin UI. Not in the required migration set per plan's `<interfaces>` target shape.
- **Verdict:** SKIP migration (leave out of index.ts barrel until admin needs them). They remain reachable via direct `from '@racingpoint/types/generated/AcLanSessionConfig'` path for any consumer that wants them.

### ActionId / SurvivalLayer / HealLease / HealLeaseRequest / HealLeaseResponse

- **Parity status:** NEW — survival types, no hand-written
- **Generated source:** 5 files in generated/
- **Verdict:** SKIP migration (not on admin-critical path).

### FleetEvent / Incident

- **Parity status:** NEW — audit/event types
- **Generated source:** `generated/FleetEvent.ts` + `generated/Incident.ts`
- **Verdict:** SKIP migration (not on admin-critical path). Admin has separate event-stream hand-written types.

### LaunchState / LaunchNoteEvent

- **Parity status:** NEW — protocol types
- **Generated source:** 2 files
- **Verdict:** SKIP migration.

### MeshSolution / SolutionStatus / FixType / DiagnosisTier

- **Parity status:** NEW — mesh intelligence types
- **Generated source:** 4 files
- **Verdict:** SKIP migration (not on admin-critical path for this wave).

### ValidityError / ValidityErrorCode / AiCountRange

- **Parity status:** NEW — inventory validation types
- **Generated source:** 3 files
- **Verdict:** SKIP migration.

### WatchdogCrashReport

- **Parity status:** NEW — crash reporting type
- **Generated source:** `generated/WatchdogCrashReport.ts`
- **Verdict:** SKIP migration.

### Booking / Event / EventType / LapData / Leaderboard / LeaderboardEntry / SessionType

- **Parity status:** NEW — telemetry/booking types
- **Generated source:** 7 files
- **Verdict:** SKIP migration (not in <interfaces> target shape; admin has its own hand-written types for these that Phase 445 is not refactoring).

### GameCatalogEntry

- **Parity status:** NOT IN WHITELIST (hand-written only, admin-only composite)
- **Hand-written source:** `packages/shared-types/src/pod.ts` lines 14-19
- **Generated source:** NONE (no Rust struct — it's assembled in-handler).
- **Verdict:** HELD hand-written.

### RedeemPinResponse / RedeemPinStatus

- **Parity status:** NOT IN WHITELIST (admin/kiosk-only composite handler response)
- **Hand-written source:** `packages/shared-types/src/reservation.ts`
- **Generated source:** NONE.
- **Verdict:** HELD hand-written.

### FeatureFlag / ConfigPush / ConfigAuditEntry

- **Parity status:** NOT IN WHITELIST (admin-only DB-shape types)
- **Hand-written source:** `packages/shared-types/src/config.ts`
- **Verdict:** HELD hand-written.

### FailureMode / LaunchStatsResponse / BillingAccuracyResponse / AlternativeCombo / LaunchMatrixRow

- **Parity status:** NOT IN WHITELIST (admin-only metrics types)
- **Hand-written source:** `packages/shared-types/src/metrics.ts`
- **Verdict:** HELD hand-written.

### WS message types (D-19 permanent)

- **Parity status:** HELD PERMANENTLY per D-19
- **Hand-written source:** `packages/shared-types/src/ws-messages.ts` (10 types: FlagSyncPayload, WsConfigPushPayload, OtaDownloadPayload, KillSwitchPayload, ConfigAckPayload, OtaAckPayload, FlagCacheSyncPayload, LaunchDiagnostics, BillingTick, GameStateChanged)
- **Verdict:** HELD hand-written permanently (D-19).

## Aggregate Verdicts

### Types safe to migrate (flip re-export in Task 2)

- **PodStatus** (PARITY OK)
- **SimType** (PARITY OK)
- **DrivingState** (PARITY OK)
- **BillingSessionStatus** (PARITY OK — 11 variants identical)
- **PodInventory** (new, no hand-written collision)
- **GameInventory** (new, no hand-written collision)
- **ContentDirsResponse** (new, no hand-written collision)
- **GameDirs** (new, no hand-written collision)
- **PlayableSignal** (new, no hand-written collision)

### Types with accepted drift (flip re-export; note change in Task 2 commit message)

- **PodInfo** — hand-written was `Pod`; ADD `PodInfo` alongside `Pod` (don't rename in admin). Field-level diffs: `T | null` vs `T | undefined` — Zod `.nullish()` handles both. Generated adds `agent_timestamp`.
- **GameState** — generated adds `"in_lobby"` variant (superset, safe for reader context).
- **BillingSessionInfo** — ADD as new export alongside existing `BillingSession`. Admin code can opt-in gradually. BillingSession stays pointing at `./billing` for BC.
- **PricingTier** — flip ONLY if admin doesn't read `.sort_order` (verify in Task 2). If admin uses `.sort_order`, demote to HELD.
- **PodFleetStatus** — flip ONLY if admin doesn't do arithmetic on `uptime_secs` bigint. If yes, demote to HELD.

### Types held hand-written (DO NOT flip in Task 2)

- **BillingSession** (composite, superset of BillingSessionInfo, stay hand-written until admin renames to `BillingSessionInfo`)
- **Driver** (hand-written has admin-only `has_used_trial`; bigint drift on `total_laps` / `total_time_ms`)
- **Pod** (hand-written type NAME differs from generated `PodInfo`; keep as alias until admin opts-in)
- **GameCatalogEntry** (composite, no Rust struct)
- **FleetHealthResponse** (no Rust struct)
- **ConfigMismatchDetected** (WS message, D-19 permanent)
- **RedeemPinResponse / RedeemPinStatus** (admin-only handler responses)
- **FeatureFlag / ConfigPush / ConfigAuditEntry** (admin-only DB types)
- **FailureMode / LaunchStatsResponse / BillingAccuracyResponse / AlternativeCombo / LaunchMatrixRow** (admin-only metrics)
- **WS message types** (FlagSyncPayload, WsConfigPushPayload, OtaDownloadPayload, KillSwitchPayload, ConfigAckPayload, OtaAckPayload, FlagCacheSyncPayload, LaunchDiagnostics, BillingTick, GameStateChanged) — D-19 permanent
- **AcLanSessionConfig + 6 Ac* transitive deps** (SKIP — not on admin-critical path)
- **ActionId / SurvivalLayer / HealLease / HealLeaseRequest / HealLeaseResponse** (SKIP — not admin-critical)
- **FleetEvent / Incident** (SKIP — not admin-critical)
- **LaunchState / LaunchNoteEvent** (SKIP — not admin-critical)
- **MeshSolution / SolutionStatus / FixType / DiagnosisTier** (SKIP — not admin-critical)
- **ValidityError / ValidityErrorCode / AiCountRange** (SKIP — not admin-critical)
- **WatchdogCrashReport** (SKIP — not admin-critical)
- **Booking / Event / EventType / LapData / Leaderboard / LeaderboardEntry / SessionType** (SKIP — admin has its own hand-written for these)

## Cross-cutting observations

1. **Pitfall 2 — `T | null` vs `T | undefined`:** Every Rust `Option<T>` with `#[serde(skip_serializing_if = "Option::is_none")]` produces a TS `T | null` via ts-rs 12 (not `T | undefined`). Hand-written admin types use `T?` (optional, undefined). Zod schemas should use `.nullish()` (accepts both) rather than `.optional()` or `.nullable()` alone. Admin tsc will NOT fail on this class (superset-assignment works), but Zod validators might reject the `null` value at runtime if they only `.optional()`.

2. **Pitfall — `u64` → `bigint`:** ts-rs 12 emits `bigint` for Rust `u64`. Affected fields in migrated/migratable types: `BillingSessionInfo.cost_paise`, `BillingSessionInfo.rate_per_min_paise`, `Driver.total_laps`, `Driver.total_time_ms`, `PodFleetStatus.uptime_secs`, `PodFleetStatus.clock_drift_secs`, `PodFleetStatus.crash_recovery_count`. Admin code doing arithmetic on these (e.g. `uptime_secs / 3600`) will fail `tsc` with "Operator '/' cannot be applied to types 'bigint' and 'number'". This is a REAL break vector — the reason several types above are held.

3. **chrono::DateTime<Utc> → string (ISO-8601):** Rust `DateTime<Utc>` emits TS `string` (always non-null, just ISO-8601). `Option<DateTime<Utc>>` emits `string | null`. Hand-written uses `string?`. Pitfall 2 class.

4. **Type-name divergence (`Pod` vs `PodInfo`):** The hand-written admin canonical is `Pod`, but the Rust struct is `PodInfo`. Plan 03 target `<interfaces>` specifies migrated name `PodInfo`. Resolution: EXPORT BOTH (generated `PodInfo` + hand-written `Pod`). Admin opts in type-by-type in a follow-on phase.

5. **D-19 — WS message types stay hand-written permanently.** 10 types in `ws-messages.ts`. Reason: the WS protocol is a mega-hub (`handle_ws_message()` with dozens of variants), versioning it through ts-rs derives would require annotating every variant + every transitive dep + handling adjacent-tagged enums (which D-14 blocks). Planned separate phase.

## Related — pre-existing drifts (out of scope, NOT a 445 blocker per RESEARCH)

- `docs/openapi.yaml` vs `web/public/api-docs/openapi.yaml` — pre-existing staleness, out of scope for 445.
- `SimType` rename overrides in Rust (`iracing`, `f1_25`, `forza_horizon_5`) — wire format parity verified OK in § 3 above.
- Admin frontend expects 3 endpoints not in live server (`/business-rules`, `/wallet/bonus-tiers/admin`, `/customer/membership/active`) — flagged by Plan 02b, out of scope.

## Task 2 migration list (binding)

Task 2 MUST flip the following types to `from '../generated/index'`:

1. PodStatus
2. SimType
3. DrivingState
4. BillingSessionStatus
5. PodInfo (ADD alongside existing `Pod`)
6. GameState
7. BillingSessionInfo (ADD alongside existing `BillingSession`)
8. PodInventory (NEW)
9. GameInventory (NEW)
10. ContentDirsResponse (NEW)
11. GameDirs (NEW)
12. PlayableSignal (NEW)
13. PricingTier — gated on admin `.sort_order` grep (Task 2 Step A sub-gate)
14. PodFleetStatus — gated on admin `uptime_secs` arithmetic grep (Task 2 Step A sub-gate)

Task 2 MUST retain these as hand-written re-exports:

- `Pod, GameCatalogEntry` from `./pod` (Pod retained even though PodInfo added)
- `BillingSession` from `./billing` (retained even though BillingSessionInfo added)
- `Driver` from `./driver` (bigint + has_used_trial blockers)
- `FleetHealthResponse, ConfigMismatchDetected` from `./fleet`
- `FeatureFlag, ConfigPush, ConfigAuditEntry` from `./config`
- All 10 WS message types from `./ws-messages` (D-19 permanent)
- `RedeemPinResponse, RedeemPinStatus` from `./reservation`
- `FailureMode, LaunchStatsResponse, BillingAccuracyResponse, AlternativeCombo, LaunchMatrixRow` from `./metrics`

---

**Audit complete. Task 2 pre-condition gate (`## Aggregate Verdicts` heading present) is satisfied.**
