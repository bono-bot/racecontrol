# Phase 368: Research — Live Launch Status with Autonomous Debug

**Researched:** 2026-04-11
**Domain:** Real-time WS event bridge (rc-agent ↔ racecontrol server ↔ kiosk) + launch state machine + staff notes CRUD + feature-flagged frontend rollout
**Confidence:** HIGH (all evidence drawn from direct file reads in this repo; no speculation on external libraries)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **D-01:** A launch is identified by a server-minted `launch_id` (UUID v4) created when `/games/launch` is accepted by the server. `launch_id` is reused as the primary key for `launch_timeline_spans` (existing table already has this column — aligns with `db/mod.rs:650`).
- **D-02:** New DB table `launch_notes` is the only new schema. Columns: `id TEXT PRIMARY KEY, launch_id TEXT NOT NULL, pod_id TEXT NOT NULL, staff_id TEXT, staff_name TEXT, body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now'))`. Indexed on `launch_id`. No edit/delete. Replicated via cloud_sync (Phase 301 dual-write pattern).
- **D-03:** No new table for launch state. State lives in-memory in a new `LaunchStateMachine` in `crates/racecontrol/src/game_launcher.rs` (or an adjacent module decided in plan-phase). On transitions it emits a WS event AND optionally appends to `launch_timeline_spans.events_json` for durable history (reusing the existing events_json column).
- **D-04:** Two new WS event types, both tagged under the existing kiosk WS stream (same channel `useKioskSocket` already uses):
  1. `launch_status_changed` — payload: `{ launch_id, pod_id, sim_type, state: "launch_started"|"ai_analysis_requested"|"issue_being_fixed"|"issue_fixed"|"needs_manual_intervention", detail, timestamp, origin: "customer"|"staff"|"auto_token"|"retry", ai_tier: 1|2|3|null, fix_action }`
  2. `launch_note_added` — payload: `{ launch_id, pod_id, note_id, staff_id, staff_name, body, created_at }`
- **D-05:** State transitions are hybrid server+rc-agent:
  - Server emits `launch_started` and `issue_fixed` (Phase 362 playable signal).
  - rc-agent emits `ai_analysis_requested` (game_doctor entry) and `issue_being_fixed` (tier_engine start).
  - rc-agent emits `needs_manual_intervention` when tier_engine exhausts Tier 1 options.
- **D-06:** Events broadcast to all staff-authenticated WS clients venue-wide. No per-pod filtering.
- **D-07:** Tier 1 deterministic fixes auto-apply (existing Phase 275 path).
- **D-08:** Tier 2+ fixes wait for staff click → `POST /api/v1/debug/launches/{launch_id}/approve-fix`.
- **D-09:** New `kiosk/src/components/LaunchCard.tsx` — pod badge, sim name, 4-dot timeline, inline notes, composer, dismiss button.
- **D-10:** Newest on top, same-pod attempts stack with expand-to-view.
- **D-11:** `issue_fixed` auto-dismiss after 5min; `needs_manual_intervention` persists until staff dismisses via `POST /api/v1/debug/launches/{launch_id}/dismiss`.
- **D-12:** Empty state: `"No active launches — waiting for next game start"` row with WS-connection pulsing dot.
- **D-13:** New boolean feature flag `kiosk_launch_cards_enabled` in feature_flags table, default `false`. Periodic re-fetch via Phase 177+ infrastructure.
- **D-14:** REMOVE `setInterval(loadData, 30000)` from `kiosk/src/app/debug/page.tsx:122` only when flag=true AND WS connected. Retain as fallback.
- **D-15:** Billing state NEVER rendered. Rejections use card text `"Launch blocked — billing not ready"` only.
- **D-16:** Launch card lifetime ends at "playable". No mid-session tracking.

### Claude's Discretion

- Exact module layout for `LaunchStateMachine` (separate file vs inside `game_launcher.rs`)
- Internal struct field order / derive macros
- Tailwind class choices (within rp-card, rp-border, rp-black, rp-grey variables)
- 4-dot state timeline horizontal vs vertical
- Animation timings for state transitions
- SQL index strategy beyond the required `idx_launch_notes_launch_id`

### Deferred Ideas (OUT OF SCOPE)

- Sim-type filter bar
- Error taxonomy view
- Launch timeline drawer with stage breakdown (events_json is durable but not surfaced in this phase)
- Orphan/ghost billing detector
- Comment threads on incidents and activity events (only `launch_notes` in scope)
- Gameplay diagnostics post-launch
- Filter-by-pod card view
- Fix auto-apply override for staff on `needs_manual_intervention` cards
</user_constraints>

<phase_requirements>
## Phase Requirements

No pre-existing REQ IDs were assigned to this phase in REQUIREMENTS.md. Derived here as `LLS-01..LLS-12` (Live Launch Status). Planner should copy these into REQUIREMENTS.md under v47.0 or a Phase 368 sub-section before plan-phase executes.

| ID | Description | Research Support |
|----|-------------|------------------|
| LLS-01 | Server mints `launch_id` at `/games/launch` entry, threads it through the launch pipeline, and reuses it as `launch_timeline_spans.launch_id` | Integration Map §1; today server+agent both mint independent launch_ids that never meet |
| LLS-02 | New `launch_status_changed` and `launch_note_added` DashboardEvent variants added to `crates/rc-common/src/protocol.rs` with full Phase 62 enum-value contract tests (string-tagged state enum) | Integration Map §2 + §8 |
| LLS-03 | In-memory `LaunchStateMachine` emits `launch_started` on server-side launch_game entry and `issue_fixed` on Phase 282 playable transition (game_launcher.rs:920) | Integration Map §3 + §4 |
| LLS-04 | rc-agent emits `ai_analysis_requested` when game_launch_retry::retry_game_launch() begins, `issue_being_fixed` when a diagnosis produces a fix, `needs_manual_intervention` when RetryResult::EscalateToMma fires | Integration Map §5 |
| LLS-05 | New `launch_notes` table created idempotently in db/mod.rs, replicated via cloud_sync SYNC_TABLES | Integration Map §6 |
| LLS-06 | New endpoints under `/api/v1/debug/launches/*` require staff JWT via existing `require_staff_jwt` middleware (proxied into the existing staff_routes branch in api/routes.rs:326) | Integration Map §7 |
| LLS-07 | Feature flag `kiosk_launch_cards_enabled` (default false) added to feature_flags table seed path; broadcast via existing FlagSync infrastructure; kiosk reads via `GET /api/v1/flags` and listens for WS FlagSync | Integration Map §8 |
| LLS-08 | Kiosk `LaunchCard` component + new handlers in `useKioskSocket.ts` switch block (after line 328); new state `launches: Map<launch_id, LaunchStatusCard>` with setter; dismiss + note composer UI | Integration Map §9 |
| LLS-09 | Kiosk `/debug` page 30s poll removal is CONDITIONAL on `flag_enabled && connected` — retain fallback otherwise | D-14 + Integration Map §9 |
| LLS-10 | Launch card for a rejected `/games/launch` (FSM-03/billing-paused/concurrent) emits a `needs_manual_intervention` state with detail `"Launch blocked — billing not ready"` regardless of which specific billing error was returned; NO customer/tier/balance field ever populated | D-15 + Integration Map §3 |
| LLS-11 | Contract test: TypeScript `LaunchStatusCard.state` union type MUST match the Rust enum-value string set; test file added to `crates/rc-common/src/protocol.rs` test module asserting each variant's JSON string form (Phase 62 pattern) | Validation Architecture §Contract Tests |
| LLS-12 | MMA audit executed before binary deploy — cross-system bridge (rc-agent → server → kiosk), both non-thinking AND thinking reasoning modes per CLAUDE.md §Subagent Gates | Validation Architecture §MMA Gate |
</phase_requirements>

## Project Constraints (from CLAUDE.md)

Relevant directives — planner must honor verbatim:

1. **§Cross-Boundary Serialization:** Every kiosk/frontend field MUST have a matching Rust struct field. Before shipping, grep `LaunchStatusCard` fields in `kiosk/src/lib/types.ts` against the Rust `DashboardEvent::LaunchStatusChanged` payload. Serde silently drops unknown fields — a mismatch means staff see stale cards with missing detail text and zero error.
2. **§Never hold a lock across `.await`:** The new `LaunchStateMachine` must snapshot (clone + drop guard) before any WS broadcast or DB write. Apply the `let snapshot = { let guard = lock.read(); guard.clone() }; drop(guard); do_async().await;` pattern already used in `billing.rs` Phase 311 game-snapshot work.
3. **§Subagent Gates:** This phase is (a) a frontend phase (kiosk LaunchCard component) → REQUIRES `gsd-ui-researcher` (UI-SPEC.md) + `gsd-ui-auditor` (UI-REVIEW.md); (b) a business-logic phase (new state machine) → REQUIRES `gsd-nyquist-auditor` (test coverage); (c) a cross-system bridge (new rc-agent → server → kiosk WS path) → REQUIRES MMA audit in BOTH reasoning modes per v27.0 rule.
4. **§Deploy Parity:** Venue .23 racecontrol + kiosk same commit as cloud Bono VPS. Feature flag default false means shadow deploy is safe.
5. **§No `.unwrap()` in production Rust, no `any` in TypeScript** — state machine transitions handled via `?` / `match`.
6. **§Route Uniqueness:** The 5 new `/debug/launches/*` routes MUST NOT collide with existing routes (none do today — verified in routes.rs grep).
7. **§v27.0 Staff-triggered fleet broadcast authority rule:** Tier 2+ fix approval (D-08) must gate behind `tier >= 2 && confidence >= 0.8` before fleet cascade; Tier 1 stays pod-local.
8. **§Smallest Reversible Fix First:** Default feature flag to false; ship backend + frontend to both venue + cloud with flag off; smoke-test shadow mode; toggle to true after MMA audit.
9. **§CGP H3 evidence rule:** Verification = EXACT behavior observed, not proxies. Post-deploy verify must trigger a simulated `GameLaunchFail` on Pod 8 and read a screenshot of the kiosk /debug page showing all 4 card states transition.
10. **§Summary Fidelity:** Plan-phase must not invent scope beyond D-01..D-16. Do NOT bundle in sim-type filter, error taxonomy view, or any deferred idea.

## Summary

Phase 368 is a **thin bridge**, not new machinery. Every autonomous behavior it "surfaces" already exists: `game_doctor::diagnose_and_fix()` runs 12 checks, `tier_engine` has a 5-tier decision tree, `game_launch_retry` retries with backoff, `knowledge_base` records resolutions, `mesh_gossip` propagates fixes to the fleet. The problem is visibility — today this machinery produces log lines and internal `FleetEvent` broadcasts that never reach the kiosk `/debug` page. The existing kiosk UI subscribes to a flat `pod_activity` event stream that conflates game launches with unrelated pod events, and polls `/api/v1/debug/activity` every 30s for refresh.

The phase adds **four wire-level boundaries**: (1) the server mints a `launch_id` at the `/games/launch` entry point and threads it through the existing WS relay so transitions on the pod can correlate back to the originating request; (2) two new `DashboardEvent` variants carry the 4-state transitions to the kiosk without disturbing the existing `game_state_changed` path; (3) a new `launch_notes` table provides an append-only audit trail of staff observations per card; (4) five new REST endpoints under `/api/v1/debug/launches/*` handle notes CRUD + staff approval of Tier 2+ fixes + dismissal of manual-intervention cards. The kiosk `LaunchCard` component replaces the flat Live Activity panel contents, and all of this is behind a feature flag (`kiosk_launch_cards_enabled`, default false) that honors Phase 177+ periodic re-fetch.

**Primary recommendation:** Add new DashboardEvent variants (NOT new FleetEvent variants). FleetEvent is internal to rc-agent's anomaly→tier engine bus and never reaches the server's WS relay directly; DashboardEvent is the channel already carrying pod→server→kiosk messages for every UI-visible state today. `launch_id` correlation should happen by threading a new `launch_id: String` field through `CoreToAgentMessage::LaunchGame` (additive, #[serde(default)] for backward compat), copied into GameTracker at game_launcher.rs:460, and recorded in `conn.current_launch_id` at ws_handler.rs:536 instead of the current agent-minted UUID. This is a single additive field change that unifies what are currently two unrelated launch_ids.

## Architecture Analysis

### Existing WS event path (traced)

For `game_state_changed` — the closest existing analog:

1. **rc-agent side:** `event_loop.rs` or sim-specific adapters emit state transitions via `ws_msg_tx.send(AgentMessage::GameStateUpdate(info))`. The info carries `GameLaunchInfo { pod_id, sim_type, game_state, pid, launched_at, error_message, diagnostics, ... }`. (See the `playable_at:` initializers at `ws_handler.rs:511, 597, 621, 676, 715, 783, ...`.)
2. **Transport:** the agent's outbound WebSocket client serializes `AgentMessage` as JSON and sends it to the server via the agent↔server WS. This is the same channel used for Heartbeat, Telemetry, LapCompleted, etc.
3. **Server receive:** `crates/racecontrol/src/ws/mod.rs` deserializes incoming frames into `AgentMessage`, matches on variant, and for `GameStateUpdate` routes into `game_launcher::handle_game_state_update(state, info)` which updates the in-memory GameTracker and at `game_launcher.rs:920` records `playable_at` on the first Running transition.
4. **Server re-broadcast:** `state.dashboard_tx.send(DashboardEvent::GameStateChanged(info))` (game_launcher.rs:528, 568). `dashboard_tx` is a `tokio::sync::broadcast::Sender<DashboardEvent>` — fan-out to all connected kiosk/web dashboard WS clients via subscribers spawned in `ws/mod.rs` around line 2530.
5. **Kiosk receive:** `useKioskSocket.ts:106` parses the `{event, data}` envelope and the switch at line 216 dispatches on `"game_state_changed"`, updating `gameStates: Map<string, GameLaunchInfo>`.

**Key observation for Phase 368:** The path `AgentMessage → dashboard_tx → DashboardEvent → kiosk` is the cleanly additive insertion point. Adding a new DashboardEvent variant costs: 1 variant + 1 handler in ws/mod.rs + 1 switch case in useKioskSocket.ts. Nothing else moves.

### FleetEvent enum current state

`crates/rc-common/src/fleet_event.rs:20-106` defines 9 variants:

1. `AnomalyDetected { trigger, severity, node_id, timestamp, pod_state_snapshot }`
2. `PredictiveAlert { alert_type, severity, message, metric_value, threshold, node_id, timestamp }`
3. `FixApplied { node_id, tier, action, trigger, timestamp }`
4. `FixFailed { node_id, tier, reason, trigger, timestamp }`
5. `Escalated { node_id, tier, reason, timestamp }`
6. `GameLaunchRetryResult { node_id, attempt, success, cause, fix_applied, timestamp }` — Phase 275 shared variant
7. `ExperienceScoreUpdate { node_id, total_score, status, timestamp }`
8. `RevenueAnomaly { anomaly_type, detail, node_id, timestamp }`
9. `ModelReputationChange { model_id, action, accuracy, total_runs, timestamp }`

**Critical architectural finding:** FleetEvent is consumed ONLY inside rc-agent's in-process broadcast bus (`FleetEventBus` at line 143, capacity 256). Grepping the racecontrol server for `FleetEvent` or `fleet_event`/`fleet_bus` returns **zero matches** — the server does not import `rc_common::fleet_event` at all. FleetEvents exist only inside one pod and drive that pod's local tier_engine, experience scorer, and fleet coordinator.

**Implication for D-04:** The CONTEXT.md phrase "Extend FleetEvent with LaunchStatusChanged" is architecturally wrong. The right move is a new `DashboardEvent` variant. This is consistent with D-04 ("both tagged under the existing kiosk WS stream") but contradicts the way CONTEXT.md's canonical_refs line 117 ("extend FleetEvent per D-04 or convert in WS relay layer") presents the option. **Research recommendation: NO FleetEvent change. Add DashboardEvent variants only.**

(Phase 275's retrospective summary already confirms this — see its "Deviations from Plan > FleetEvent::GameLaunchRetryResult does not exist" note at the time. The variant was later added but remains unused by the server.)

### Phase 275 machinery hook points

All needed code is in rc-agent:

| File | Line | What happens today | Phase 368 hook |
|------|------|---------------------|-----------------|
| `crates/rc-agent/src/game_launch_retry.rs` | 44 | `pub fn retry_game_launch() -> RetryResult` — synchronous, called from tier_engine::run_tiers on `DiagnosticTrigger::GameLaunchFail` via `spawn_blocking` | **Emit `ai_analysis_requested`** before line 44's `let start = Instant::now();`. Pass a `ws_msg_tx: &UnboundedSender<AgentMessage>` handle in as a parameter, or widen the return to carry a pre-emission callback. |
| `crates/rc-agent/src/game_launch_retry.rs` | 74 | `let diagnosis: GameDiagnosis = game_doctor::diagnose_and_fix();` | Optionally emit a second `ai_analysis_requested` per retry attempt with `detail: "retry attempt N/2"` — OR keep it to 1 event per RetryResult (simpler, per D-04's "1 card per launch_id"). Recommend the latter. |
| `crates/rc-agent/src/game_launch_retry.rs` | 77 | `if diagnosis.fixed { ... return RetryResult::Fixed { ... }; }` | **Emit `issue_being_fixed`** with `ai_tier: 1, fix_action: fix_str.clone()` right before the return. |
| `crates/rc-agent/src/game_launch_retry.rs` | 100-110 | `if *hint == RetryHint::NoRetry { ... return RetryResult::EscalateToMma { ... }; }` | **Emit `needs_manual_intervention`** with `detail: format!("No retry possible for {:?}", diagnosis.cause)`. |
| `crates/rc-agent/src/game_launch_retry.rs` | 140 | Final `RetryResult::EscalateToMma { attempts: MAX_RETRY_ATTEMPTS, causes }` (exhausted all retries) | **Emit `needs_manual_intervention`** with `detail: format!("Tier 1 exhausted after {} attempts", MAX_RETRY_ATTEMPTS)`. |
| `crates/rc-agent/src/tier_engine.rs` | 2125-2135 | `DiagnosticTrigger::GameLaunchFail => { ... let retry_result = game_launch_retry::retry_game_launch(); ... }` | This is the **call site** — plumb `ws_msg_tx` + `launch_id` through here. `launch_id` comes from `conn.current_launch_id` (see ws_handler.rs:536). |
| `crates/rc-agent/src/tier_engine.rs` | 722-788 | Existing `fleet_bus_tx.send(FleetEvent::FixApplied { ... })` emissions for non-game triggers | Keep untouched — the tier engine already has the right broadcast pattern for generic fixes. |

**Important constraint: game_doctor itself must NOT emit events.** `diagnose_and_fix()` is a pure synchronous 12-check function — adding a WS send inside would require threading a channel through a sync function that has no async context. Keep all emissions in `game_launch_retry.rs` and `tier_engine.rs` where the async `ws_msg_tx` is already in scope.

### Phase 362 playable signal

Phase 362 Stage 5 `verify_launch_config()` in `crates/rc-agent/src/launch_verifier.rs` is the final check before a launch is declared successful. It runs:
1. Process alive
2. Main window present
3. Game state = InRace/Practice/Hotlap
4. Telemetry feed active
5. **(NEW in 362)** SessionConfig matches AcLaunchParams

When Stage 5 passes, the agent's event loop emits `AgentMessage::GameStateUpdate(GameLaunchInfo { game_state: GameState::Running, ... })`. The server receives this in `ws/mod.rs` and calls `game_launcher::handle_game_state_update()`, which at **game_launcher.rs:920** runs:

```rust
if info.game_state == GameState::Running && tracker.playable_at.is_none() {
    let now = Utc::now();
    tracker.playable_at = Some(now);
    ...
}
```

**This is the server-side "playable" hook point for emitting `issue_fixed`.** The new LaunchStateMachine observes the `tracker.playable_at.is_none() → Some(now)` transition and fires `DashboardEvent::LaunchStatusChanged { state: "issue_fixed", launch_id: tracker.launch_id.clone(), ... }`. Simple and deterministic.

Note: "issue_fixed" is only emitted if there was a PRIOR `ai_analysis_requested` for this launch_id (i.e. a Tier 1 retry actually ran). A plain happy-path launch without any retry simply skips the whole card animation and the card's final state is `launch_started → issue_fixed` in one jump. The state machine needs to track "has this launch_id ever been in ai_analysis_requested?" to decide whether to fire `issue_fixed` at all — or simpler: always fire it, and let the kiosk UI decide what to show.

### launch_timeline_spans as state store

`db/mod.rs:650-675`:

```sql
CREATE TABLE IF NOT EXISTS launch_timeline_spans (
    launch_id   TEXT PRIMARY KEY,
    pod_id      TEXT NOT NULL,
    sim_type    TEXT NOT NULL,
    preset_id   TEXT,
    billing_session_id TEXT,
    outcome     TEXT NOT NULL,
    total_duration_ms INTEGER NOT NULL,
    started_at  TEXT NOT NULL,
    events_json TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
)
```

**Current usage:** LAUNCH-05 (Phase 318 launch intelligence). `launch_id` is currently **agent-minted** at `ws_handler.rs:536` (`conn.current_launch_id = Some(uuid::Uuid::new_v4().to_string());`). The agent emits `AgentMessage::LaunchTimelineReport(LaunchTimeline)` at the end of a launch (see rc-common/src/protocol.rs:635), which the server persists into this table.

**Phase 368 constraint from D-01:** "Server-minted launch_id UUID reused as launch_timeline_spans.launch_id." This is a contract change. There are two viable paths:

1. **Thread launch_id from server → agent in the launch message.** Add `launch_id: Option<String>` (with `#[serde(default)]` for backward compat) to `CoreToAgentMessage::LaunchGame` (rc-common/src/protocol.rs:944 area). Server mints the UUID at game_launcher.rs:460 (already does today for GameTracker), puts it in the launch message, and the agent stores it at ws_handler.rs:536 INSTEAD of minting its own. Old agents (backward compat) fall back to minting their own — logs a deprecation warning.
2. **Agent-minted launch_id flows back to server via GameStateUpdate.** Add `launch_id: Option<String>` field to `GameLaunchInfo` struct (rc-common/src/protocol.rs:131 area). Agent includes its minted UUID on every GameStateUpdate, server stores it in the GameTracker on first update, and uses that as the card's launch_id going forward.

**Recommendation: Path 1.** It's cleaner and matches D-01 literally. Path 2 delays card emission by one round-trip (the server can't emit `launch_started` until it receives the first state update from the agent, which is ~200ms+ later). Path 1 lets the server emit `launch_started` immediately on the `/games/launch` HTTP response path before the agent has even started.

### Feature flag infrastructure

Two layers exist:

1. **Server-side registry:** `crates/racecontrol/src/flags.rs` — `feature_flags` SQLite table with columns `(name, enabled, default_value, overrides, version, updated_at)`. In-memory cache at `state.feature_flags: RwLock<HashMap<String, FeatureFlagRow>>`. CRUD via `GET/POST/PUT /api/v1/flags` endpoints. Flag changes broadcast via `CoreToAgentMessage::FlagSync(FlagSyncPayload)` to all connected pods (see rc-common/src/protocol.rs:944).

2. **Kiosk consumption:** The kiosk is NOT a pod agent — it does NOT receive FlagSync messages via WS today. The kiosk MUST fetch flags over HTTP from `GET /api/v1/flags` at mount and periodically re-fetch (Phase 177+ periodic re-fetch pattern mirrors the pod-side `spawn_periodic_refetch()` in `crates/rc-common/src/boot_resilience.rs`).

**For `kiosk_launch_cards_enabled`:**

- Add a seed row to the `feature_flags` table creation path in `db/mod.rs` (search for existing seeded flags to find the exact location — look near where `game_launch` flag is seeded; `grep -n 'game_launch' crates/racecontrol/src/db/mod.rs`).
- Kiosk reads via `fetchApi<FeatureFlagRow[]>('/flags')` in `kiosk/src/lib/api.ts` (add a new `listFlags()` helper).
- Kiosk `DebugPage` calls `api.listFlags()` on mount + every 60s, reads `kiosk_launch_cards_enabled`, conditionally renders either the new card view or the old flat activity feed.
- No pod-side change needed — this flag never reaches rc-agent.

**NOTE: D-14 "REMOVE poll only when flag true AND WS connected" is a user-review item.** Current poll at `debug/page.tsx:122` hits server endpoints `/debug/activity`, `/debug/playbooks`, `/debug/incidents` — none of these hit pods directly. The anti-cheat risk cited in the DESCRIPTION.md is minimal here. Flag this for plan-phase: the fallback logic (keep poll when flag=false) is almost certainly unnecessary, but the user has asked for it in D-14 so preserve it. Mark it as a follow-up cleanup task for a future phase.

### Cloud sync patterns

`crates/racecontrol/src/cloud_sync.rs:31`:

```rust
pub const SYNC_TABLES: &str = "drivers,wallets,pricing_tiers,pricing_rules,billing_rates,kiosk_experiences,kiosk_settings,auth_tokens,reservations,debit_intents,staff_members,driver_ratings,fleet_solutions,model_evaluations";
```

**For launch_notes replication:** add `launch_notes` to this string. Syncing is bidirectional — cloud admins (reviewing post-mortems) will get venue data within 30s (HTTP fallback) or 2s (relay mode).

**Schema idempotency pattern:** `db/mod.rs` uses `CREATE TABLE IF NOT EXISTS ...` + trailing `ALTER TABLE ... ADD COLUMN ...` (wrapped in `let _ =` to ignore failures when column already exists — see lines 287-296, 714-767 for examples). This is the pattern to copy for `launch_notes` creation (there are no `ALTER` needs since the table is brand new).

**Phase 368 tables that need replication:**
- `launch_notes` — MUST be added to SYNC_TABLES per D-02
- `launch_timeline_spans` — already exists but is NOT in SYNC_TABLES today. **Finding:** launch_timeline_spans currently does NOT replicate to cloud. For Phase 368 the card view is in-memory only — not affected — but if a future phase wants the timeline drawer (deferred per DESCRIPTION.md), that phase will need to add launch_timeline_spans to SYNC_TABLES.

### Existing /debug auth pattern

`crates/racecontrol/src/api/routes.rs:326-340`:

```rust
fn staff_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        ...
        .route("/debug/db-stats", get(debug_db_stats))
        .route("/debug/activity", get(debug_activity))
        .route("/debug/playbooks", get(debug_playbooks))
        .route("/debug/incidents", get(list_debug_incidents).post(create_debug_incident))
        .route("/debug/incidents/{id}", put(update_debug_incident))
        .route("/debug/incidents/{id}/apply-fix", post(debug_apply_fix))
        .route("/debug/diagnose", post(debug_diagnose))
        .route("/debug/pod-events/{pod_id}", get(debug_pod_events))
        ...
        .layer(axum::middleware::from_fn_with_state(state, require_staff_jwt))
}
```

**Pattern for Phase 368's 5 new routes:**

```rust
.route("/debug/launches/active", get(debug_launches_active))
.route("/debug/launches/{launch_id}/notes", get(debug_launches_get_notes).post(debug_launches_post_note))
.route("/debug/launches/{launch_id}/approve-fix", post(debug_launches_approve_fix))
.route("/debug/launches/{launch_id}/dismiss", post(debug_launches_dismiss))
```

All five go into the `staff_routes()` router (same block as existing debug routes). They inherit `require_staff_jwt` from the `.layer(...)` on the staff router. The `kiosk_staff_token` from sessionStorage is sent as `Authorization: Bearer <token>` via the existing `fetchApi()` helper (`kiosk/src/lib/api.ts:16`).

**No new middleware needed.** No Phase 348 AUTH-01..07 lockout concerns here — these are read endpoints + low-frequency staff writes (notes are typed by humans, approve-fix + dismiss are single clicks).

## Integration Map (file + line for every hook point)

### 1. Server-side launch_id generation and propagation

| # | File | Line | Current code | Change |
|---|------|------|--------------|--------|
| 1a | `crates/rc-common/src/protocol.rs` | ~944 (CoreToAgentMessage::LaunchGame area) | `LaunchGame { sim_type, launch_args, force_clean, duration_minutes }` | Add `launch_id: String` field (NOT optional — server always generates). Backward compat with older agents: agents that don't know the field still parse OK via serde's default-on-missing behavior IF we wrap with `#[serde(default)]`; BUT D-01 says server-minted so the field must be present in all new messages. Safer: make it `Option<String>` with `#[serde(default)]` and have the agent fall back to minting its own if missing (warns in log) |
| 1b | `crates/racecontrol/src/game_launcher.rs` | 460 | `launch_id: uuid::Uuid::new_v4().to_string(),` | Extract to `let launch_id = uuid::Uuid::new_v4().to_string();` BEFORE the GameTracker struct init, and reuse it on the next line in the struct AND in the LaunchGame message at line 478 |
| 1c | `crates/racecontrol/src/game_launcher.rs` | 478 | `let launch_inner = launcher.make_launch_message(sim_type, launch_args, duration_minutes);` | Pass `launch_id` through. Update the `GameLauncherImpl::make_launch_message` trait signature (line 106, 123, 135, 147, 159) to accept `launch_id: String` and forward it into CoreToAgentMessage::LaunchGame |
| 1d | `crates/rc-agent/src/ws_handler.rs` | 536 | `conn.current_launch_id = Some(uuid::Uuid::new_v4().to_string());` | Replace with `conn.current_launch_id = launch_id_from_msg.or_else(|| { tracing::warn!("LaunchGame without launch_id — minting locally (old server?)"); Some(uuid::Uuid::new_v4().to_string()) });` where `launch_id_from_msg` is the deserialized field from the CoreToAgentMessage::LaunchGame payload |

### 2. New DashboardEvent variants

| # | File | Line | Change |
|---|------|------|--------|
| 2a | `crates/rc-common/src/protocol.rs` | After line 1156 (GameStateChanged variant) | Add two new variants: `LaunchStatusChanged(LaunchStatusCard)` and `LaunchNoteAdded(LaunchNoteEvent)` |
| 2b | `crates/rc-common/src/protocol.rs` | Near line 1103 (DashboardEvent definition) or separate type section | Define new structs: `LaunchStatusCard { launch_id, pod_id, sim_type, state: LaunchState, detail: Option<String>, timestamp: DateTime<Utc>, origin: LaunchOrigin, ai_tier: Option<u8>, fix_action: Option<String> }` and `LaunchNoteEvent { launch_id, pod_id, note_id, staff_id, staff_name, body, created_at: DateTime<Utc> }` |
| 2c | `crates/rc-common/src/protocol.rs` | Same region | Define `pub enum LaunchState { LaunchStarted, AiAnalysisRequested, IssueBeingFixed, IssueFixed, NeedsManualIntervention }` with `#[serde(rename_all = "snake_case")]` so JSON = `"launch_started"` etc. |
| 2d | `crates/rc-common/src/protocol.rs` | Same region | Define `pub enum LaunchOrigin { Customer, Staff, AutoToken, Retry }` with `#[serde(rename_all = "snake_case")]` |
| 2e | `crates/rc-common/src/protocol.rs` | Test module (bottom of file, see existing `test_pod_restarting_json_roundtrip` around line 1807) | Add Phase 62 enum-value contract tests: `assert_eq!(serde_json::to_string(&LaunchState::LaunchStarted).unwrap(), "\"launch_started\"");` × 5 states, same for LaunchOrigin |

### 3. Server-side LaunchStateMachine

| # | File | Line | Change |
|---|------|------|--------|
| 3a | `crates/racecontrol/src/game_launcher.rs` | Top of file or new sibling module `launch_state.rs` | Define `pub struct LaunchStateMachine { states: RwLock<HashMap<String, LaunchStatusCard>> }` (key = launch_id). Methods: `start_launch(launch_id, pod_id, sim_type, origin) -> LaunchStatusCard`, `transition(launch_id, new_state, detail, ai_tier, fix_action)`, `get_active() -> Vec<LaunchStatusCard>`, `dismiss(launch_id)` |
| 3b | `crates/racecontrol/src/state.rs` | AppState struct | Add `pub launch_state_machine: Arc<LaunchStateMachine>` field |
| 3c | `crates/racecontrol/src/game_launcher.rs` | 428 (right before the GameTracker creation block) | Emit `launch_started`: call `state.launch_state_machine.start_launch(launch_id.clone(), pod_id.to_string(), sim_type, origin)` then `state.dashboard_tx.send(DashboardEvent::LaunchStatusChanged(card))`. `origin` derived from the launch caller context (HTTP path, WS message, auth token consume — needs inspection of the 6 call sites of `launch_game`) |
| 3d | `crates/racecontrol/src/game_launcher.rs` | ~305 (after billing gate rejection `return Err(...)` at line 324/334) | On billing-rejection error returns (FSM-03, paused, TOCTOU expired), emit `needs_manual_intervention` with `detail: "Launch blocked — billing not ready"`. Thread launch_id generation above the billing gate check so it exists on the error path too. **NOTE:** This requires moving `let launch_id = uuid::Uuid::new_v4().to_string();` from line 460 to just after the feature-flag check at line 312. D-15 hard constraint: do NOT include the specific billing failure reason in `detail` — always the same generic string |
| 3e | `crates/racecontrol/src/game_launcher.rs` | 920 (playable_at transition) | After `tracker.playable_at = Some(now);`, call `state.launch_state_machine.transition(tracker.launch_id.clone(), LaunchState::IssueFixed, None, None, None)` and broadcast |
| 3f | `crates/racecontrol/src/game_launcher.rs` | Same 920 block | Schedule auto-dismiss 5 min later: `tokio::spawn(async move { tokio::time::sleep(Duration::from_secs(300)).await; state.launch_state_machine.dismiss(launch_id).await; })` (D-11) |

### 4. rc-agent event emissions

| # | File | Line | Change |
|---|------|------|--------|
| 4a | `crates/rc-agent/src/game_launch_retry.rs` | 44 (top of retry_game_launch) | Convert from sync `pub fn retry_game_launch() -> RetryResult` to sync with side effects OR async wrapper. Recommended: keep sync, add a `&UnboundedSender<AgentMessage>` parameter + `launch_id: String` parameter. Emit `AgentMessage::LaunchStatusUpdate(...)` at each state boundary. **Blocking/async impedance:** `ws_msg_tx` is tokio async; use `.try_send()` from sync context (already done elsewhere in this file for telemetry drops) |
| 4b | `crates/rc-common/src/protocol.rs` | AgentMessage enum ~line 87 | Add `LaunchStatusUpdate { launch_id, state: LaunchState, detail: Option<String>, ai_tier: Option<u8>, fix_action: Option<String>, origin: LaunchOrigin }` variant |
| 4c | `crates/racecontrol/src/ws/mod.rs` | Inside the AgentMessage match block (same section that handles GameStateUpdate around line 527-528) | Add `AgentMessage::LaunchStatusUpdate { .. } => { state.launch_state_machine.transition(...); state.dashboard_tx.send(DashboardEvent::LaunchStatusChanged(card)); }` |
| 4d | `crates/rc-agent/src/tier_engine.rs` | 2125-2135 (`DiagnosticTrigger::GameLaunchFail` branch) | Fetch current launch_id from conn state (requires passing through — see tier_engine's existing `node_id` and `ws_msg_tx` clones). Pass into `game_launch_retry::retry_game_launch(&ws_msg_tx, launch_id)` |

### 5. launch_notes table + REST endpoints

| # | File | Line | Change |
|---|------|------|--------|
| 5a | `crates/racecontrol/src/db/mod.rs` | Near line 675 (right after `launch_timeline_spans` block) | Add `CREATE TABLE IF NOT EXISTS launch_notes (id TEXT PRIMARY KEY, launch_id TEXT NOT NULL, pod_id TEXT NOT NULL, staff_id TEXT, staff_name TEXT, body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now')))` + `CREATE INDEX IF NOT EXISTS idx_launch_notes_launch_id ON launch_notes(launch_id)` |
| 5b | `crates/racecontrol/src/cloud_sync.rs` | 31 (SYNC_TABLES constant) | Append `,launch_notes` |
| 5c | `crates/racecontrol/src/api/routes.rs` | 340 (after `debug_pod_events` route in staff_routes) | Register 5 new routes (see §Kiosk staff auth above). Handler functions live in a new `crates/racecontrol/src/api/debug_launches.rs` module |
| 5d | `crates/racecontrol/src/api/` | New file `debug_launches.rs` | Implement 5 handlers: `debug_launches_active`, `debug_launches_get_notes`, `debug_launches_post_note`, `debug_launches_approve_fix`, `debug_launches_dismiss`. All take `Extension<StaffClaims>` for the staff_id + staff_name |

### 6. Feature flag wiring

| # | File | Line | Change |
|---|------|------|--------|
| 6a | `crates/racecontrol/src/db/mod.rs` | Search for existing seeded flags (grep `game_launch` in this file) | Add seed row `INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides, version, updated_at) VALUES ('kiosk_launch_cards_enabled', 0, 0, '{}', 1, datetime('now'))` |
| 6b | `kiosk/src/lib/api.ts` | After line ~88 (fleetHealth) | Add `listFlags: () => fetchApi<FeatureFlagRow[]>("/flags")` |
| 6c | `kiosk/src/lib/types.ts` | End of file | Add `export interface FeatureFlagRow { name: string; enabled: boolean; default_value: boolean; overrides: string; version: number; updated_at?: string; }` |
| 6d | `kiosk/src/app/debug/page.tsx` | After line 99 (staffName auth effect) | Add flag fetch + 60s re-fetch: `useEffect(() => { const load = async () => { const flags = await api.listFlags(); const flag = flags.find(f => f.name === 'kiosk_launch_cards_enabled'); setLaunchCardsEnabled(flag?.enabled ?? false); }; load(); const i = setInterval(load, 60000); return () => clearInterval(i); }, []);` |
| 6e | `kiosk/src/app/debug/page.tsx` | 122 | Change `const interval = setInterval(loadData, 30000);` to `const interval = (launchCardsEnabled && connected) ? null : setInterval(loadData, 30000);` with null-check on cleanup |

### 7. Kiosk frontend (LaunchCard component)

| # | File | Line | Change |
|---|------|------|--------|
| 7a | `kiosk/src/lib/types.ts` | After GameLaunchInfo block at line 131-147 | Add `LaunchStatusCard` and `LaunchNoteEvent` interfaces matching the Rust structs. String-tagged union `"launch_started" \| "ai_analysis_requested" \| "issue_being_fixed" \| "issue_fixed" \| "needs_manual_intervention"` for `state`. **Phase 62 cross-boundary rule applies here — field names and enum strings MUST match Rust.** |
| 7b | `kiosk/src/hooks/useKioskSocket.ts` | After the `switch` block at line 328 | Add two new cases: `case "launch_status_changed": { const card = msg.data as LaunchStatusCard; setLaunches(prev => { const next = new Map(prev); next.set(card.launch_id, card); return next; }); break; }` and `case "launch_note_added": { const note = msg.data as LaunchNoteEvent; setLaunchNotes(prev => { const next = new Map(prev); const existing = next.get(note.launch_id) ?? []; next.set(note.launch_id, [...existing, note]); return next; }); break; }` |
| 7c | `kiosk/src/hooks/useKioskSocket.ts` | Line 73 area (state declarations) | Add `const [launches, setLaunches] = useState<Map<string, LaunchStatusCard>>(new Map());` and `const [launchNotes, setLaunchNotes] = useState<Map<string, LaunchNoteEvent[]>>(new Map());` |
| 7d | `kiosk/src/hooks/useKioskSocket.ts` | Line 398-420 (return object) | Export `launches`, `launchNotes`, `dismissLaunch` |
| 7e | `kiosk/src/components/LaunchCard.tsx` | New file | New component. Props: `{ card: LaunchStatusCard, notes: LaunchNoteEvent[], onAddNote: (body: string) => void, onApproveFix: () => void, onDismiss: () => void }`. Renders pod badge, sim name (via gameDisplayInfo lookup), 4-dot state timeline, inline notes, composer (textarea + submit), action buttons (approve-fix when state=issue_being_fixed && ai_tier >= 2; dismiss when state=needs_manual_intervention or issue_fixed). Use existing `rp-card`, `rp-border`, `rp-black`, `rp-grey` Tailwind variables from KioskHeader.tsx |
| 7f | `kiosk/src/app/debug/page.tsx` | Around line 340-400 (Live Activity panel) | If `launchCardsEnabled && connected`, replace the flat activity feed with a vertical list of `<LaunchCard ... />` components sorted newest-first by `card.timestamp` descending, grouped by `pod_id` per D-10. Otherwise fall through to existing flat activity feed |
| 7g | `kiosk/src/lib/api.ts` | New methods | Add: `postLaunchNote(launchId, body) → POST /debug/launches/{id}/notes`, `getLaunchNotes(launchId)`, `approveLaunchFix(launchId)`, `dismissLaunch(launchId)`, `listActiveLaunches() → GET /debug/launches/active`. All use the existing `fetchApi` helper which already handles the staff token |

### 8. Cloud parity

| # | File / Target | Change |
|---|---------------|--------|
| 8a | Venue .23 racecontrol.exe | Standard 7-step server deploy per CLAUDE.md (`deploy-server.sh v3.0`) |
| 8b | Venue .23 kiosk :3300 | Standard Next.js rebuild + tar + SCP + extract + schtask restart |
| 8c | Pod 1-8 rc-agent.exe | Standard fleet deploy per CLAUDE.md §Deploy (stage-release → deploy-pod.sh) — required for the new AgentMessage::LaunchStatusUpdate variant + CoreToAgentMessage::LaunchGame launch_id field |
| 8d | Cloud Bono VPS racecontrol | Same build via git_pull + `cargo build --release --bin racecontrol` + pm2 restart |
| 8e | Cloud Bono VPS kiosk | Next.js rebuild in cloud directory + pm2 restart |
| 8f | Deploy parity check | All 4 targets (venue binary, venue kiosk, cloud binary, cloud kiosk) on same git commit. Flag remains `false` in feature_flags table for shadow deploy. Toggle to `true` ONLY after MMA audit + Pod 8 canary visual verification |

## Risks + Constraints

| Risk | Class | Mitigation |
|------|-------|------------|
| **Two launch_ids today** — agent mints its own at ws_handler.rs:536, server mints its own at game_launcher.rs:460. D-01 requires unification. Risk of partial deploy causing launch_timeline_spans rows without matching launch_status_changed events | Structural | Ship the protocol change (CoreToAgentMessage::LaunchGame.launch_id) FIRST as a backward-compat additive field with `Option<String>`. Deploy racecontrol FIRST with the server end that always sends. Deploy rc-agent SECOND with the client end that reads it if present. Old rc-agents ignore the new field silently (serde skip_unknown). New rc-agents on new server use the unified ID. Old rc-agents on new server log a warning and fall back to local mint. Order matters for parity. |
| **`dashboard_tx` broadcast backpressure** — tokio broadcast channel capacity limits; slow subscribers drop events. If 10 kiosk clients are watching the debug page and one lags, events can be missed | Operational | Snapshot-before-broadcast pattern is already standard; the new LaunchStateMachine in-memory map gives the initial-load endpoint `/debug/launches/active` a full list on page load, so missed WS events can be recovered by manual refresh. Flag this as a known limitation of the in-memory model (D-03) |
| **Lock held across await** — the new `LaunchStateMachine.transition()` must not hold the `states` RwLock while the WS broadcast runs | Correctness (CLAUDE.md hard rule) | Use clone-and-drop pattern: `let card = { let mut states = self.states.write().await; let mut c = states.get(&launch_id).cloned()?; c.state = new_state; states.insert(launch_id.clone(), c.clone()); c }; dashboard_tx.send(DashboardEvent::LaunchStatusChanged(card))` |
| **Billing-rejection path exits early before launch_id generation** — today `/games/launch` rejects billing at lines 322-336 BEFORE the GameTracker is created at line 444. To emit a `needs_manual_intervention` card on rejection, launch_id must be minted earlier | Refactoring | Move `let launch_id = uuid::Uuid::new_v4().to_string();` to just after the feature-flag check at line 312 so it exists on every error path out of launch_game |
| **Broadcast origin inference for `origin` field** — `launch_game` is called from 6+ places (direct REST, WS command, auth token consume, retry, split continuation, staff terminal). Telling them apart requires a new enum parameter | Moderate scope creep | Add `pub async fn launch_game(state, pod_id, sim_type, launch_args, origin: LaunchOrigin)` — force all call sites to specify. Plan-phase should enumerate all 6 call sites and assign correct origin values |
| **Cross-boundary serialization drift** — TypeScript LaunchStatusCard struct fields out-of-sync with Rust LaunchStatusCard fields. Serde silently drops unknowns — Phase 62 exact failure mode (Pod 8 pitlane incident 2026-04-08) | Known historical (CLAUDE.md rule) | Add a contract test in `crates/rc-common/src/protocol.rs` tests module asserting each LaunchState variant serializes to the exact expected string. Also add a TypeScript type check (e.g. a runtime assertion in tests/page-crawler that every received `launch_status_changed` message matches a typed schema) |
| **Feature flag seeding race** — first-boot of new racecontrol binary reads feature_flags table which doesn't yet contain `kiosk_launch_cards_enabled` | Predictable | DB migration inserts row with `INSERT OR IGNORE`. If flag cache reads before migration, it gets empty map and falls back to `default_value = false` via the `unwrap_or(false)` pattern already used at game_launcher.rs:307 |
| **D-14 conditional poll removal is structurally odd** — the described fallback (poll when flag=false) is strictly worse than removing the poll entirely. The F1 25 anti-cheat argument doesn't apply because the poll hits server endpoints, not pod endpoints | Scope challenge | Preserve the conditional logic per user directive, but flag as user-review item in plan-phase. The cleanup to fully remove the poll should be a follow-up (Phase 369 or deferred cleanup) |
| **No `NX/NM` cardinality limit on active launches** — if a pod crashes 100 times in a row, 100 launch_ids stack up | Minor | Cap the in-memory map to 100 most recent active launches. Stale entries (no transition in 10 min) auto-expire. Document in LaunchStateMachine docstring |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework (Rust) | cargo test (rustc 1.93.1, workspace packages: `rc-common`, `rc-agent-crate`, `racecontrol-crate`) |
| Framework (Kiosk TS) | Playwright + existing `tests/page-crawler` + Phase 67 `tests/contract/pod-inventory.test.ts` pattern |
| Config files | `Cargo.toml` (workspace), `tests/page-crawler/playwright.config.ts` |
| Quick run (Rust) | `cargo test -p rc-common --test protocol` (new tests for enum-value contracts) |
| Quick run (backend integration) | `cargo test -p racecontrol-crate launch_state_machine` |
| Full suite (Rust) | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| Full suite (frontend) | `cd kiosk && npm run test && cd tests/page-crawler && npx playwright test` |
| Phase gate | Full suite green + MMA audit PASS before `/gsd:verify-work` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| LLS-01 | Server-minted launch_id propagates through LaunchGame message → rc-agent ws_handler stores it | unit (rust) | `cargo test -p racecontrol-crate launch_id_propagation` | ❌ Wave 0 |
| LLS-02 | Phase 62 enum-value contract test: LaunchState → JSON string set | unit (rust) | `cargo test -p rc-common test_launch_state_snake_case` | ❌ Wave 0 |
| LLS-03 | LaunchStateMachine.transition() emits DashboardEvent without holding lock across await | unit (rust) | `cargo test -p racecontrol-crate test_launch_state_no_lock_held_across_await` | ❌ Wave 0 |
| LLS-03 | launch_started emitted on /games/launch HTTP path entry | integration (rust) | `cargo test -p racecontrol-crate test_launch_started_emitted_on_accept` | ❌ Wave 0 |
| LLS-03 | issue_fixed emitted at game_launcher.rs:920 playable transition | integration (rust) | `cargo test -p racecontrol-crate test_issue_fixed_on_playable` | ❌ Wave 0 |
| LLS-04 | game_launch_retry emits ai_analysis_requested at start, issue_being_fixed on fix, needs_manual_intervention on exhaust | unit (rust) | `cargo test -p rc-agent-crate test_launch_retry_emits_status` | ❌ Wave 0 |
| LLS-05 | launch_notes table CREATE IF NOT EXISTS idempotency + cloud_sync SYNC_TABLES inclusion | unit (rust) | `cargo test -p racecontrol-crate test_launch_notes_schema_idempotent` | ❌ Wave 0 |
| LLS-06 | POST /debug/launches/{id}/notes requires staff JWT (401 without, 200 with) | integration (rust) | `cargo test -p racecontrol-crate test_launch_notes_auth_required` | ❌ Wave 0 |
| LLS-07 | kiosk_launch_cards_enabled flag read at mount; 60s periodic re-fetch | playwright | `npx playwright test tests/page-crawler/launch-cards-flag.spec.ts` | ❌ Wave 0 |
| LLS-08 | kiosk /debug renders LaunchCard on launch_status_changed WS message; all 4 states produce distinct visual states (screenshot) | playwright + visual regression | `npx playwright test tests/venue-e2e/launch-card-states.spec.ts --update-snapshots` (Phase 1) then assertion pass (Phase 2) | ❌ Wave 0 |
| LLS-09 | 30s poll removed when flag=true && ws connected; retained otherwise | playwright | `npx playwright test tests/page-crawler/launch-cards-poll-gate.spec.ts` | ❌ Wave 0 |
| LLS-10 | Billing-rejection path emits needs_manual_intervention with detail="Launch blocked — billing not ready" (no customer data) | integration (rust) | `cargo test -p racecontrol-crate test_billing_rejection_card_text_sanitized` | ❌ Wave 0 |
| LLS-11 | TS LaunchStatusCard.state union matches Rust LaunchState enum string set via generated shared-types package OR explicit contract test | contract (TS) | `cd kiosk && npx tsc --noEmit && cd tests/contract && npx vitest run launch-status.test.ts` | ❌ Wave 0 |
| LLS-12 | MMA audit v3.0 DEFAULT — 5 model consensus over adversarial verify, dual reasoning modes | manual (command ready) | `node scripts/multi-model-audit.js` | ✅ (script exists) |

### Sampling Rate
- **Per task commit:** `cargo test -p <affected_crate>` (under 20s for rc-common, under 60s for racecontrol-crate full suite)
- **Per wave merge:** Full workspace `cargo test` + kiosk `npm run test` + Playwright smoke
- **Phase gate:** Full suite green + MMA audit PASS + Pod 8 canary E2E (deliberate GameLaunchFail → screenshot 4 card states) before `/gsd:verify-work`

### Wave 0 Gaps

Before execution begins, Wave 0 must create the test infrastructure:

- [ ] `crates/rc-common/src/protocol.rs` test module — add Phase 62 contract tests (5x LaunchState + 4x LaunchOrigin serde assertions). Pattern: copy from the existing `test_pod_restarting_json_roundtrip` at line ~1807
- [ ] `crates/racecontrol/src/game_launcher.rs` test module — add `#[tokio::test]` async tests for launch_state_machine interactions (mock dashboard_tx receiver, verify transitions). Pattern: copy from existing tests in the same file (search for `#[cfg(test)]`)
- [ ] `crates/rc-agent/src/game_launch_retry.rs` test module — add tests for the 4 emission points. Pattern: copy from existing `test_hint_for_*` tests at line 147-184
- [ ] `tests/venue-e2e/launch-card-states.spec.ts` — new Playwright spec. Use existing `auth-setup.ts` pattern (note: auth-setup.ts writes to localStorage but should write to sessionStorage per prior session side-finding; fix separately)
- [ ] `tests/page-crawler/launch-cards-flag.spec.ts` — new spec for feature flag gating
- [ ] `tests/contract/launch-status.test.ts` — new contract test asserting TS types match Rust enum strings
- [ ] No new framework install required — pytest/playwright/cargo test already present

### MMA Gate (LLS-12)

Per CLAUDE.md §Subagent Gates, this phase MUST run MMA audit before deploy because it is a **new cross-system bridge** (rc-agent → server → kiosk WS path with new event types). The audit must:
1. Run in BOTH reasoning modes (non-thinking for architecture; thinking for execution-path / state machine invariants)
2. Minimum 5 models per iteration, ≥3 vendor families
3. Full 4-step convergence engine (DIAGNOSE → PLAN → EXECUTE → VERIFY)
4. Adversarial verify step with 3 different models from Step 1-3
5. Cost ≤$5 per session per CLAUDE.md budget rule

The audit prompt should specifically challenge:
- Cross-boundary enum-value drift (Phase 62 class)
- Lock ordering across async boundaries (the new LaunchStateMachine)
- Auth boundary gaps on the 5 new endpoints
- Billing state leakage on the `needs_manual_intervention` card (D-15 hard constraint)
- Backpressure on broadcast channel at 10 concurrent dashboard clients

Command (from CLAUDE.md §Unified MMA Protocol v3.0 Operational Reference):
```bash
export OPENROUTER_KEY="..."  # from openrouter.ai/settings/keys — NEVER hardcode
node scripts/multi-model-audit.js
```

## Open Questions for Planner

1. **Backward compatibility scope for old rc-agents:** Should rc-agents that don't yet have the new `LaunchStatusUpdate` message still work on a new server? Recommendation: yes, `#[serde(default)]` on the new launch_id field in CoreToAgentMessage::LaunchGame, and on receive the agent falls back to minting its own launch_id with a deprecation log. Backward compat for one deploy cycle is cheap insurance.

2. **Origin enum completeness:** 6 launch call sites have been identified (REST /games/launch, dashboard WS LaunchGame command, auth token consume, split continuation, staff terminal manual, retry-after-crash). Does the planner want to merge any of these, or are all 6 distinct origin values needed? Suggested minimum set per D-04: `Customer` (REST/WS from kiosk), `Staff` (dashboard command, staff terminal), `AutoToken` (auth consume), `Retry` (post-crash relaunch).

3. **Card for plain happy-path launch:** If a launch succeeds without any Tier 1 retry (no GameLaunchFail trigger fires), does a card still render? Recommended: yes, the card appears in `launch_started` state and transitions directly to `issue_fixed` on playable. "Fixed" is a misnomer in this case but matches the 4-state model. Plan-phase should add a `severity` derived field to LaunchStatusCard for CSS color-coding: green for clean launches, amber for Tier 1-fixed, red for needs_manual_intervention.

4. **launch_timeline_spans cloud_sync:** Currently NOT in SYNC_TABLES. Phase 368 does not need it (card state is in-memory), but the deferred "timeline drawer" idea in DESCRIPTION.md will need it. Is this out of scope for Phase 368? Recommend: yes — deferred.

5. **Integration with Phase 318 launch intelligence:** Phase 318 already has server-side aggregation (`crates/racecontrol/src/error_aggregator.rs`). Does the new launch state machine feed into that aggregation or stay parallel? Recommend parallel — Phase 318 consumes `launch_events` table; Phase 368 cards are purely ephemeral UI state. Double-check with plan-phase.

6. **Mesh gossip interaction with Tier 2+ approval flow:** Existing tier_engine.rs:731-745 broadcasts Tier 1 game fixes to the fleet via `mesh_gossip::build_game_fix_announce()`. For Tier 2+ fixes gated behind staff approval (D-08), the broadcast must happen AFTER the staff click, not during the initial `issue_being_fixed` emit. The current code path fires the broadcast at verification pass. Does Phase 368 need to change this? Recommend: NO change to mesh_gossip. The D-08 staff-click only gates WHETHER to apply the fix at all. If staff clicks approve, the existing tier_engine path runs and the existing mesh gossip broadcast happens naturally.

7. **Per-pod card stacking (D-10):** "Older attempts for that pod collapse behind the newest. Click to expand." — UI decision. Recommend: simple `.slice(0, 1)` per pod shown by default with an expand button revealing the rest. Plan-phase finalizes with ui-researcher.

8. **Dismiss endpoint semantics:** POST /debug/launches/{id}/dismiss per D-11 "marks launch as staff-acknowledged in launch_timeline_spans (new column staff_dismissed_at, lightweight ALTER)." Does the planner want to add this ALTER or keep dismiss purely in-memory (drop from LaunchStateMachine map)? Recommend: both — the in-memory map entry is removed AND a row in launch_timeline_spans gets `staff_dismissed_at` updated (idempotent ALTER TABLE with `let _ =` wrap, fire-and-forget UPDATE). This gives post-mortem auditability without depending on the ephemeral state machine.

## Deploy Notes

**Deploy order (critical for backward compat):**

1. **Build racecontrol.exe from the new code.** Feature flag defaults false; no visible change.
2. **Deploy racecontrol server to venue .23** via `deploy-staging/deploy-server.sh <hash>` (the v3.0 hardened script). Standard 7-step + verify. New AgentMessage::LaunchStatusUpdate variants are ignored by old rc-agents (serde skip unknown). New CoreToAgentMessage::LaunchGame.launch_id field is sent but ignored by old rc-agents.
3. **Rebuild kiosk Next.js app** with new LaunchCard + types. SCP tar to .23, extract, schtask restart kiosk on port 3300. Feature flag false means old view renders — shadow deploy confirmed safe.
4. **Deploy rc-agent.exe to Pod 8 canary** via `deploy-pod.sh`. Verify build_id match + run one synthetic GameLaunchFail → observe card state transitions in kiosk /debug. Feature flag still false, so observation is via structured logs + network tap only. Keep Pod 8 on new binary for 30 min of normal traffic.
5. **Pod 8 canary passes → deploy rc-agent.exe to remaining 7 pods** via fleet deploy.
6. **Cloud Bono VPS parity:** git_pull + `cargo build --release --bin racecontrol` + pm2 restart + Next.js rebuild in cloud kiosk directory.
7. **Verify all 4 targets (venue racecontrol, venue kiosk, cloud racecontrol, cloud kiosk) on same commit.** Per CLAUDE.md §Deploy Parity.
8. **Toggle feature flag to true via admin API** `PUT /api/v1/flags/kiosk_launch_cards_enabled` body `{"enabled": true}`. Flag change broadcasts to all pods via FlagSync (pods don't care for this flag — kiosk picks it up on next 60s re-fetch or on page reload).
9. **Visual verification per CLAUDE.md §Visual verification rule:** take a screenshot of /kiosk/debug page with the new card view active, confirm layout + no regressions on incidents sidebar / playbooks panel.
10. **MMA audit MANDATORY before step 8** — toggle flag ONLY after MMA audit PASS (LLS-12).

**Rollback plan:**

- **Flag off:** `PUT /api/v1/flags/kiosk_launch_cards_enabled {"enabled": false}` — instant revert to old view. No binary swap required.
- **Binary rollback:** `del racecontrol.exe && ren racecontrol-prev.exe racecontrol.exe && schtasks /Run /TN StartRCDirect` — venue only. Cloud uses pm2's previous build.
- **Fleet rollback:** rc-agents retain `rc-agent-prev.exe`; `ren` swap back via `scripts/deploy/deploy-pod.sh --rollback <pod>` (if script exists, else manual SSH).

**72-hour binary retention rule:** `racecontrol-prev.exe` and `rc-agent-prev.exe` kept on disk ≥72h per CLAUDE.md §Rollback window.

## Sources

### Primary (HIGH confidence — direct file reads)

- `C:/Users/bono/racingpoint/racecontrol/crates/rc-common/src/fleet_event.rs` (lines 1-165, all 9 enum variants + FleetEventBus)
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-common/src/protocol.rs` (lines 30-430 AgentMessage, lines 1090-1350 DashboardEvent, lines 940+ CoreToAgentMessage::FlagSync)
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-common/src/types.rs` (lines 1-45 SimType enum — 8 variants)
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/game_launcher.rs` (lines 100-560 launch_game flow, lines 900-950 playable_at transition, line 460 current server-minted launch_id)
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/db/mod.rs` (lines 400-676 launch_events + launch_timeline_spans schemas, lines 287-767 ALTER patterns)
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs` (lines 1-120 SYNC_TABLES definition)
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/flags.rs` (lines 1-120 feature_flags schema + list_flags handler)
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/api/routes.rs` (lines 95-375 public_routes, staff_routes, require_staff_jwt middleware layer, lines 333-340 existing /debug/* routes)
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/ws/mod.rs` (lines 1-200 auth + DASHBOARD_CLIENT_COUNT, line 527-528 + 568 dashboard_tx.send broadcast pattern)
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/game_doctor.rs` (full file: 629 lines — pure sync diagnose_and_fix function)
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/game_launch_retry.rs` (full file: 185 lines — retry_game_launch with 3 return paths)
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/tier_engine.rs` (lines 1-300 + 680-815 FleetEvent emission pattern + 2125-2135 GameLaunchFail hook)
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/ws_handler.rs` (lines 520-540 current agent-minted launch_id, function around line 2043 LaunchTimelineReport construction)
- `C:/Users/bono/racingpoint/racecontrol/kiosk/src/hooks/useKioskSocket.ts` (full file: 421 lines — full WS subscription hook)
- `C:/Users/bono/racingpoint/racecontrol/kiosk/src/app/debug/page.tsx` (lines 1-250 — imports, hooks, loadData poll at line 122)
- `C:/Users/bono/racingpoint/racecontrol/kiosk/src/lib/api.ts` (lines 1-200 fetchApi staff token pattern)
- `C:/Users/bono/racingpoint/racecontrol/kiosk/src/lib/types.ts` (lines 115-160 GameLaunchInfo + LaunchDiagnostics)
- `C:/Users/bono/racingpoint/racecontrol/.planning/phases/275-autonomous-game-launch-fix/275-01-SUMMARY.md`
- `C:/Users/bono/racingpoint/racecontrol/.planning/phases/362-post-launch-config-verification/362-01-SUMMARY.md`
- `C:/Users/bono/racingpoint/racecontrol/.planning/phases/311-launch-billing-coordination-guard/311-01-SUMMARY.md`
- `C:/Users/bono/racingpoint/racecontrol/.planning/phases/368-live-launch-status-with-autonomous-debug/368-CONTEXT.md`
- `C:/Users/bono/racingpoint/racecontrol/.planning/phases/368-live-launch-status-with-autonomous-debug/DESCRIPTION.md`
- `C:/Users/bono/racingpoint/racecontrol/.planning/STATE.md`
- `C:/Users/bono/racingpoint/racecontrol/.planning/REQUIREMENTS.md` (Theme 1-6, ADMIN-01..28, AUTH-01..07)
- `C:/Users/bono/racingpoint/racecontrol/.planning/config.json` (nyquist_validation: true)
- Inline CLAUDE.md delivered via system-reminder

### Secondary (MEDIUM confidence)

- None — all findings traced to primary sources in this repo.

### Tertiary (LOW confidence — flag for validation)

- None — research did not rely on WebSearch or external documentation.

## Metadata

**Confidence breakdown:**
- Standard stack (none — all existing code): HIGH — zero new libraries, all changes operate within existing Rust/Next.js stack
- Architecture (hook points + state machine placement): HIGH — every hook point verified by direct line read in this session
- Pitfalls (Phase 62 drift, lock across await, dual launch_ids): HIGH — all three are documented CLAUDE.md rules with prior incident history

**Research date:** 2026-04-11
**Valid until:** 2026-04-18 (code churn in this repo averages ~5-10 commits/day; lines and patterns identified above may shift slightly. Plan-phase should re-grep line numbers before authoring PLAN.md)
