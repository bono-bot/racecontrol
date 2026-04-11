# Phase 368: Live Launch Status with Autonomous Debug — Context

**Gathered:** 2026-04-11
**Status:** Ready for planning
**Mode:** `--auto` (recommended defaults auto-selected; user may revisit any D-xx before plan-phase commits)

<domain>
## Phase Boundary

**What this phase delivers:** A real-time, condensed-card view of game launches on the kiosk `/debug` page. Each launch gets one card that progresses through 4 states (`launch_started` → `ai_analysis_requested` → `issue_being_fixed` → `issue_fixed` | `needs_manual_intervention`). Staff can add append-only notes to each card. The feature exposes Phase 275's existing autonomous rc-agent fix machinery through a new WS channel and a new UI component. It replaces the current flat activity feed in the "Live Activity" panel of `kiosk/src/app/debug/page.tsx`.

**Hard scope limits:**
- Launch phase only — from `/games/launch` request to "playable" (game window focused + telemetry flowing). Gameplay and mid-session issues are explicitly **out of scope**.
- Billing is internal and invisible. Do not surface customer names, tiers, balances, or wallet state. If a launch is blocked by billing, card text is `"Launch blocked — billing not ready"` with no detail.
- All launch origins: customer kiosk flow, staff terminal manual launch, auto-launch post auth token consume, retry-after-crash.
- All sim types that `sim_type` writers populate today — enumerated from code in plan-phase, not hardcoded.

**New capabilities must be rejected** and routed to other phases: sim-type filter, error-taxonomy view, billing coordination detector, comment threads on non-launch entities, gameplay diagnostics.

</domain>

<decisions>
## Implementation Decisions

### Data model

- **D-01:** A launch is identified by a server-minted `launch_id` (UUID v4) created when `/games/launch` is accepted by the server. `launch_id` is reused as the primary key for `launch_timeline_spans` (existing table already has this column — aligns with `db/mod.rs:650`).
  - **Why:** Stable across retries, joinable with existing timeline table, decouples card lifecycle from `pod_id` (important because one pod can retry a failed launch and both attempts deserve distinct cards).
  - **Alternatives considered:** (pod_id, started_at) composite — rejected because it fragments the join story with `launch_timeline_spans`. pod_id alone — rejected because retry-after-crash would clobber the prior attempt's card.

- **D-02:** New DB table `launch_notes` is the only new schema. Columns: `id TEXT PRIMARY KEY, launch_id TEXT NOT NULL, pod_id TEXT NOT NULL, staff_id TEXT, staff_name TEXT, body TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (datetime('now'))`. Indexed on `launch_id`. No edit/delete. Replicated via cloud_sync (Phase 301 dual-write pattern).
  - **Why:** Append-only audit trail for post-mortems. DB over in-memory so notes survive server restart and replicate to cloud for cross-session review.

- **D-03:** No new table for launch state. State lives in-memory in a new `LaunchStateMachine` in `crates/racecontrol/src/game_launcher.rs` (or an adjacent module decided in plan-phase). On transitions it emits a WS event AND optionally appends to `launch_timeline_spans.events_json` for durable history (reusing the existing events_json column).
  - **Why:** Avoid DB contention during fast-fire launches (customer flows multiple launches per minute). `launch_timeline_spans` already persists outcome — in-memory state machine just feeds it.

### WS event protocol

- **D-04:** Two new WS event types, both tagged under the existing kiosk WS stream (same channel `useKioskSocket` already uses):
  1. **`launch_status_changed`** — payload: `{ launch_id: string, pod_id: string, sim_type: string, state: "launch_started"|"ai_analysis_requested"|"issue_being_fixed"|"issue_fixed"|"needs_manual_intervention", detail: string|null, timestamp: string, origin: "customer"|"staff"|"auto_token"|"retry", ai_tier: 1|2|3|null, fix_action: string|null }`
  2. **`launch_note_added`** — payload: `{ launch_id: string, pod_id: string, note_id: string, staff_id: string, staff_name: string, body: string, created_at: string }`
  - **Why:** Decoupled from the legacy `game_state_changed` event, which is already consumed by other UI components with different semantics. New event types are additive — no breaking changes.

- **D-05:** State transitions are hybrid server+rc-agent:
  - **Server emits** `launch_started` (has authoritative `/games/launch` receipt) and `issue_fixed` (sees the game process reach "playable" via the existing post-launch config verification path from Phase 362).
  - **rc-agent emits** `ai_analysis_requested` (when `game_doctor.rs` starts analysis) and `issue_being_fixed` (when `tier_engine.rs` begins applying a fix). These relay through the existing rc-agent → server → kiosk WS pipeline (no new channel).
  - **rc-agent emits** `needs_manual_intervention` when `tier_engine` exhausts Tier 1 options AND confidence for Tier 2+ fixes is < 0.8 OR requires broadcast approval.

- **D-06:** `launch_status_changed` events are broadcast to all connected staff-authenticated WS clients in the venue. No filtering by pod ownership — any staff watching the debug page sees all launches.
  - **Why:** Staff triage is venue-wide. Scoping adds complexity with no benefit.

### Autonomous fix authority

- **D-07:** Tier 1 deterministic fixes (fleet_kb confidence >= 0.8, pod-local effect, reversible) apply **automatically without staff click**. Existing Phase 275 `game_launch_retry.rs` already implements this — Phase 368 only surfaces its existing events, does not change authority rules.

- **D-08:** Tier 2+ fixes (KB-sourced, confidence < 0.8, or broadcast-requiring) emit `issue_being_fixed` with `ai_tier: 2` or `3` and `fix_action: "<description>"`, but **wait for staff click** on the card's "Apply fix" button. Staff click POSTs to a new endpoint `POST /api/v1/debug/launches/{launch_id}/approve-fix` which calls into the existing tier_engine approval path.
  - **Why:** Matches v27.0 MMA standing rule — staff-triggered fleet broadcast is only allowed for Tier 2+ KB-sourced solutions with explicit approval. Applying brake calibration from Pod 3 to Pod 7 without staff consent is a physical safety issue.

### UI behavior

- **D-09:** Cards replace the existing "Live Activity" panel contents but preserve the surrounding layout (Pods sidebar + Incidents panel stay as-is). A new React component `LaunchCard` under `kiosk/src/components/LaunchCard.tsx` renders: pod number badge, sim name, 4-dot state timeline with current state highlighted, inline notes thread, "Add note" composer (staff-only, uses `kiosk_staff_token`), dismiss button (shown when state is `issue_fixed` or `needs_manual_intervention`).

- **D-10:** Card ordering: newest launch on top. Cards for the same pod stack in a "recent launches" sub-group — older attempts for that pod collapse behind the newest. Click to expand.
  - **Why:** Matches how staff mentally model "what's happening right now" — pod-first, chronology-second.

- **D-11:** Card dismissal policy:
  - `issue_fixed` state: auto-dismiss **5 minutes** after transition. Hidden but retrievable from a "Recently resolved" accordion for 24h.
  - `needs_manual_intervention`: **stays until staff explicitly dismisses** via the card button. No auto-dismiss. POSTs to `POST /api/v1/debug/launches/{launch_id}/dismiss` which marks the launch as staff-acknowledged in `launch_timeline_spans` (new column `staff_dismissed_at`, lightweight ALTER).

- **D-12:** Empty state: when no active launches exist, render a single row `"No active launches — waiting for next game start"` with a pulsing dot matching current WS connection state.

### Feature flag + rollout

- **D-13:** New boolean feature flag `kiosk_launch_cards_enabled` in `racecontrol.toml` under `[kiosk]` section, default `false`. Kiosk debug page checks the flag via an existing feature-flag API endpoint at mount time AND listens for WS flag sync events (Phase 177+ feature flag re-fetch). When `false`, the old "Live Activity" flat feed renders unchanged.
  - **Why:** Kill switch for live rollout. Enables shadow deploy — backend emits events, frontend ignores them, smoke tests can check both paths. Toggle to `true` after MMA audit + first-use verification.

- **D-14:** REMOVE `setInterval(loadData, 30000)` poll from `kiosk/src/app/debug/page.tsx` **only when feature flag is true AND WS is connected**. Retain poll as fallback when flag is false OR WS disconnected for >30s. Rationale: anti-cheat risk comes from kiosk hitting **pods** repeatedly, not from kiosk hitting the server. The server poll is relatively safe, but WS is still preferred for latency. Fallback preserves behavior for flag-off mode.
  - **Clarification of earlier user intent:** The user said "if we do a poll, it could affect F125 and other anti-cheats". On audit, the current 30s poll hits `http://192.168.31.23:8080/api/v1/debug/*` — server endpoints, not pods. No direct anti-cheat exposure. But the principle (prefer WS) stands. Plan-phase will verify this claim against network graph and may propose fully removing the poll if the fallback proves unnecessary.

### Scope boundary enforcement

- **D-15:** Billing state is NEVER rendered on a launch card. Not even badge form. If `/games/launch` is rejected because billing is not ready, server emits `launch_status_changed { state: "needs_manual_intervention", detail: "Launch blocked — billing not ready" }`. No customer name, no tier, no remaining minutes.
  - **Why:** User directive. Staff triage on debug page is technical, not financial. Financial state has other surfaces (POS, admin).

- **D-16:** Launch card lifetime ends at "playable". No tracking of in-game events, telemetry, or session progression — those are owned by other systems (Phase 363, Phase 364, Phase 366). If a running game crashes, that's a NEW launch attempt when it retries — not a continuation of the old card.

### Claude's Discretion

The following are delegated to the planner/executor within the decisions above:
- Exact module layout for the `LaunchStateMachine` (separate file vs inside `game_launcher.rs`)
- Internal struct field order / derive macros
- Tailwind class choices for card visual design (within the existing kiosk design system — rp-card, rp-border, rp-black, rp-grey variables)
- Whether the 4-dot state timeline is horizontal or vertical
- Exact animation timings for state transitions
- SQL index strategy beyond the required `idx_launch_notes_launch_id`

### Folded Todos

No todos were matched for Phase 368 (`todo match-phase 368` returned empty).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 275 — autonomous fix machinery (the dependency being surfaced)
- `.planning/phases/275-autonomous-game-launch-fix/275-01-SUMMARY.md` — what Phase 275 built, what's already in rc-agent
- `crates/rc-agent/src/game_launch_retry.rs` — retry + hint classification
- `crates/rc-agent/src/game_doctor.rs` — diagnosis entry point (where `ai_analysis_requested` emission hooks in)
- `crates/rc-agent/src/tier_engine.rs` — Tier 1/2/3 fix application (where `issue_being_fixed` emission hooks in)
- `crates/rc-agent/src/knowledge_base.rs` — KB recording (confidence source for Tier 2+ gating)
- `crates/rc-agent/src/mesh_gossip.rs` — fleet-wide fix propagation (v27.0 safety gate applies here)

### FleetEvent + WS relay
- `crates/rc-common/src/fleet_event.rs` §`enum FleetEvent` lines 20-106 — existing event variants; extend with `LaunchStatusChanged` per D-04 or convert in WS relay layer
- `crates/rc-common/src/fleet_event.rs` §`GameLaunchRetryResult` lines 73-80 — existing event already emitted by Phase 275, used as hint that `needs_manual_intervention` should fire when `success=false`
- `crates/racecontrol/src/ws/mod.rs` — WS broadcast module (where new kiosk event types register)
- `crates/rc-agent/src/ws_handler.rs` — rc-agent WS client (where relay events originate)

### Existing data model — reuse, do not duplicate
- `crates/racecontrol/src/db/mod.rs:401-414` — legacy `game_launch_events` table (keep writing, do NOT read in new UI)
- `crates/racecontrol/src/db/mod.rs:419-451` — `launch_events` table with error_taxonomy (read-only in Phase 368; plan-phase may expose for card detail expansion)
- `crates/racecontrol/src/db/mod.rs:650-675` — `launch_timeline_spans` with `billing_session_id` + `events_json` + `outcome` — **the underlying store for launch state history**
- `crates/racecontrol/src/game_launcher.rs:1575` — where `game_launch_events` rows are inserted (touchpoint for launch_id generation)
- `crates/racecontrol/src/game_launcher.rs:1685` — idempotent CREATE TABLE for legacy game_launch_events (reference for launch_notes migration pattern)

### Kiosk frontend
- `kiosk/src/hooks/useKioskSocket.ts` — WS subscription hook; register new event cases here (`launch_status_changed`, `launch_note_added`)
- `kiosk/src/hooks/useKioskSocket.ts:32-50` — existing DashboardEvent / AssistanceRequest / GameLaunchRequest interfaces (pattern for new LaunchStatusEvent interface)
- `kiosk/src/hooks/useKioskSocket.ts:210-228` — existing `game_state_changed` handler (do NOT modify — new event is additive)
- `kiosk/src/lib/types.ts:131-147` — `GameLaunchInfo` + `LaunchDiagnostics` interfaces; new `LaunchStatusCard` interface belongs here
- `kiosk/src/app/debug/page.tsx` — full 968-line debug page; "Live Activity" panel is the replacement target. Incidents sidebar + playbooks panel must stay untouched.
- `kiosk/src/app/debug/page.tsx:122` — `setInterval(loadData, 30000)` removal target (see D-14 conditional)
- `kiosk/src/lib/api.ts:16` — `sessionStorage.getItem("kiosk_staff_token")` — the auth pattern new POST endpoints will reuse

### Phase 362 — "playable" signal source
- `.planning/phases/362-post-launch-config-verification-layer-3/` (directory exists) — post-launch verification already emits a signal when game reaches playable state; `issue_fixed` state transition hooks into this signal
- `crates/racecontrol/src/game_launcher.rs` §Phase 362 instrumentation — plan-phase must locate the exact emission point

### Phase 318 — launch intelligence (read-only dep)
- `.planning/phases/318-launch-intelligence/318-CONTEXT.md` — aggregation patterns already established for launch events; read for consistency before adding new aggregation logic
- `crates/racecontrol/src/error_aggregator.rs` — existing per-sim_type rollups; Phase 368 does not modify but its events feed into the same aggregation bus

### Phase 311 — launch-billing coordination guard (read-only dep)
- Phase 311 handles the "billing not ready" rejection path on the server side. Phase 368's `"Launch blocked — billing not ready"` card text sources from its existing error enum — plan-phase identifies the enum variant to map.

### Standing rules (CLAUDE.md enforcement)
- `CLAUDE.md` §Standing Rules > Cross-Boundary Serialization — mandatory check: every kiosk/frontend field MUST have a matching Rust struct field. New `launch_status_changed` payload is cross-boundary and needs the Phase 62-style enum-value contract test.
- `CLAUDE.md` §Standing Rules > Never hold a lock across `.await` — WS broadcast fan-out in `ws/mod.rs` must clone/snapshot before iterating.
- `CLAUDE.md` §Subagent Gates — frontend phase requires `gsd-ui-researcher` (UI-SPEC.md) + `gsd-ui-auditor` (UI-REVIEW.md); business logic (state machine) requires `gsd-nyquist-auditor`; cross-system bridge requires MMA audit.
- `CLAUDE.md` §Deploy > Deploy Parity — venue .23 + cloud Bono VPS + Admin + Kiosk same commit.

### Project-level
- `.planning/PROJECT.md` — v47.0 constraints (parallel with v46.0, pre-opening deadline)
- `.planning/STATE.md` — Phase 368 roadmap-evolution entry (added 2026-04-11)
- `.planning/phases/368-live-launch-status-with-autonomous-debug/DESCRIPTION.md` — full scope spec as drafted in add-phase

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **useKioskSocket hook** (`kiosk/src/hooks/useKioskSocket.ts`) — already handles connect/reconnect/deserialize for the WS channel. New event cases slot into the existing `switch (msg.type)` block. Staying in this hook avoids a parallel WS connection and keeps the server from seeing extra socket churn.
- **KioskHeader + rp-card / rp-border Tailwind variables** (`kiosk/src/app/debug/page.tsx:293-358`) — existing layout shell, header, sidebar. LaunchCard plugs into the existing flex layout without touching the page frame.
- **Phase 275's autonomous retry machinery** — already runs on every pod. Phase 368 does NOT rebuild diagnosis/fix logic; it only emits new events at the existing function boundaries (game_doctor entry, tier_engine start/complete).
- **launch_timeline_spans.events_json** (existing JSON blob column) — ideal storage for durable state transition history without schema migration.
- **cloud_sync.rs dual-write pattern** (Phase 301) — template for replicating `launch_notes` to cloud.
- **Feature flag pattern** (`[kiosk]` section in racecontrol.toml + FF-01 infrastructure from Phase 177+) — standard toggle mechanism; launch_cards_enabled fits this pattern.

### Established Patterns
- **Cross-boundary enum values** — string-tagged enums between Rust and TypeScript, with Phase 62 contract tests as the pattern (Pod 8 pitlane incident 2026-04-08 is the failure mode being guarded against).
- **WS event fan-out** — clone snapshot before iterating (standing rule; v27.0 MMA finding).
- **Kiosk staff auth** — `sessionStorage.getItem("kiosk_staff_token")` → `Authorization: Bearer` header on every REST call. Cookie `kiosk_staff_jwt` for middleware gate. See also side-finding in the prior session: `tests/page-crawler/auth-setup.ts` writes to localStorage but should write to sessionStorage. NOT in scope for Phase 368 but worth noting so the new POST endpoints don't inherit the broken test helper.
- **Next.js basePath /kiosk** — all kiosk routes prefixed; middleware strips basePath for unauth'd requests on /debug route.

### Integration Points
- `/api/v1/games/launch` — existing endpoint; add `launch_id` generation + emit `launch_status_changed{state: "launch_started"}` event
- `/api/v1/debug/launches/{launch_id}/notes` — new POST endpoint for staff note submission
- `/api/v1/debug/launches/{launch_id}/approve-fix` — new POST endpoint for Tier 2+ staff approval
- `/api/v1/debug/launches/{launch_id}/dismiss` — new POST endpoint for manual dismiss of `needs_manual_intervention`
- `/api/v1/debug/launches/active` — new GET endpoint for initial load (list currently-active launch cards, sorted by newest) so a fresh page load doesn't wait for the next WS event
- rc-agent `game_doctor.rs` analysis start hook — add WS emit for `ai_analysis_requested`
- rc-agent `tier_engine.rs` fix start/complete/fail hooks — add WS emits for `issue_being_fixed` / `issue_fixed` / `needs_manual_intervention`

</code_context>

<specifics>
## Specific Ideas

- **"As if I were debugging it"** — the user's phrasing establishes the north star: the system should walk the same diagnostic sequence a human would (reproduce → hypothesize → test → fix → verify). Phase 275 already encodes this via tier_engine + game_doctor + knowledge_base. Phase 368's job is to SHOW that walk, not redo it. Each state transition is one step in the walk.
- **"We don't want to flood the chat"** — 4 transitions is the cap. Verbose log output goes into the card's collapsed "details" section (plan-phase decides the exact disclosure UI), not the main status line.
- **Realtime constraint** — user explicitly cited F1 25 / anti-cheat as the reason for WS-only. Even if the current 30s poll hits server (not pods), the principle stands and D-14 preserves a fallback for flag-off mode only.
- **"Coordinate with billing"** (invisible) — billing dependency exists, UI exposure does not. User's sequential clarification: "A game will not launch without some parts of billing" + "Not sure in depth to billing or do not show in that code". Interpretation: billing is an implicit gate on the launch pipeline, not a display concern.
- **User ran a Playwright probe on `/kiosk/debug` earlier this session** (prior turn) — observed real data rendering with live pod events. That rendered layout is the baseline for UI hardening — the new card system must not regress the existing incident triage workflow.

</specifics>

<deferred>
## Deferred Ideas

The following came up during discussion or scope exploration and are explicitly deferred to other phases:

- **Sim-type filter bar** — useful but not part of the condensed 4-state card model. Deferred to a future "Launch Debugger Pro" phase if staff ask for it after rollout.
- **Error taxonomy view** — `launch_events.error_taxonomy` column is populated but not exposed. Deferred.
- **Launch timeline drawer with stage breakdown** — `launch_timeline_spans.events_json` holds per-stage timing, worth surfacing for deep debug, but adds scope. Deferred.
- **Orphan/ghost billing detector** — billing-without-launch or launch-without-billing reconciliation. Belongs in a separate "Billing-Game Reconciliation" phase.
- **Comment threads on incidents and activity events** — original scope draft had generic `staff_comments` table. Narrowed to `launch_notes` only for this phase. Generic commenting deferred.
- **Gameplay diagnostics post-launch** — once a game is playable, Phase 364 (session quality monitor) + Phase 366 (fleet intelligence) own the space. Not Phase 368.
- **Filter-by-pod card view** — cards are venue-wide in D-06. Per-pod filtering is a low-cost UI addition but adds scope. Deferred.
- **Fix auto-apply override for staff** — staff clicking "apply fix anyway" on a card flagged `needs_manual_intervention`. Nice to have but requires more auth machinery. Deferred.

### Reviewed Todos (not folded)

None — `todo match-phase 368` returned empty.

</deferred>

---

*Phase: 368-live-launch-status-with-autonomous-debug*
*Context gathered: 2026-04-11 (auto-selected defaults)*
