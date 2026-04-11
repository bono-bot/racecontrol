# Phase 368 — Live Launch Status with Autonomous Debug

**Depends on:** Phase 275 (`autonomous game launch fix` — rc-agent retry + KB + gossip, shipped 2026-04-01). Phase 275 already built the autonomous diagnose-and-fix machinery on the pod side. This phase surfaces it via WS events + a new kiosk UI.

## Goal

Kiosk `/debug` page shows **real-time per-launch status cards** with a condensed 4-state model, replacing the current flat activity feed. Staff sees what matters — not log floods.

## The 4 states per launch card

1. **Launch started** — `/games/launch` received, pod + sim_type recorded
2. **AI analysis requested** — launch stalled or failed, diagnosis underway
3. **Issue being fixed** — fix action running (Tier 1 auto-apply OR Tier 2+ staff-approved)
4. **Issue fixed** (playable) — or **Needs manual intervention**

## Scope (HARD BOUNDED)

- **Launch phase only.** From `/games/launch` command until the game is playable (window focused + telemetry flowing). After playable, this feature is done with that launch. Gameplay, mid-session issues, race telemetry: **OUT OF SCOPE**.
- **All launch origins:** kiosk customer flow, staff terminal manual launch, auto-launch after auth token consume, retry-after-crash.
- **All sim types** that `sim_type` writers populate today — enumerate from code, do not hardcode. Expected: AC / ACE / ACR / LMU / FH5 / F1 25 / iRacing.
- **Billing is INTERNAL and INVISIBLE.** The launch pipeline already coordinates with billing (auth_token → billing_session → launch). Do **NOT** surface billing state, customer names, wallet balances, or pricing tiers in the debug UI. If a launch is blocked because billing is not ready, the card says "Launch blocked — billing not ready" without exposing internals.

## Autonomous diagnose-and-fix loop

- On launch failure or stall, system auto-invokes AI analysis (existing playbook + pod diagnostic event pipeline, already built in Phase 275).
- **Tier 1 deterministic fixes** (from `fleet_kb` / known playbooks) apply automatically without staff click. Safe because pod-local, reversible.
- **Tier 2+ fixes** (KB-sourced, confidence < 0.8, or affecting multiple pods) require staff click on the card to approve. Rule from v27.0 MMA audit standing rule on staff-triggered broadcast.
- Every state transition emits a WS event; card updates live.

## Realtime only — NO polling

- Add `LaunchStateChanged` WS event on the existing `useKioskSocket` channel.
- **REMOVE** the existing `setInterval(loadData, 30000)` in `kiosk/src/app/debug/page.tsx` (30s poll).
- **REASON:** repeated HTTP polls during gameplay risk anti-cheat false positives on F1 25 (EAC), iRacing, BattlEye-protected titles. WS push from rc-agent → server → kiosk is already how game state flows; extend it.

## Staff notes (inline on card)

- **NEW table `launch_notes`** (scope-bounded naming — NOT `staff_comments`): `id, launch_id, pod_id, staff_id, staff_name, body, created_at`
- Append-only, no edit, no delete
- Rendered inline in the card timeline between state transitions
- Pushed via WS (`LaunchNoteAdded` event) so two staff terminals see each other live
- `POST /api/v1/debug/launches/{launch_id}/notes`, staff JWT required

## Card lifetime

- Auto-dismiss 5 min after "Issue fixed"
- Stays forever on "Needs manual intervention" until staff explicitly dismisses

## Data model — REUSE, DO NOT DUPLICATE

- Existing `launch_timeline_spans` ([db/mod.rs:650](../../../crates/racecontrol/src/db/mod.rs#L650)) has `billing_session_id` link + `events_json` + `outcome`. Use as underlying store.
- Existing `launch_events` ([db/mod.rs:419](../../../crates/racecontrol/src/db/mod.rs#L419)) has richer taxonomy. Read-only in this feature; do not write new rows.
- Existing `game_launch_events` ([db/mod.rs:401](../../../crates/racecontrol/src/db/mod.rs#L401)) is legacy stream. Continue writing but do not read from UI.
- **NEW:** `launch_notes` table only.
- **NEW:** Launch state machine emits `LaunchStateChanged` on transitions. In-memory vs DB-backed decided during plan-phase.

## Backend work

- **Launch state machine:** extend `game_launcher.rs` to emit 4 state transitions via WS: `LaunchStateChanged { launch_id, pod_id, sim_type, state, detail, timestamp }`.
- Hook the AI analysis + playbook fix path (from Phase 275) so it auto-triggers on failure and emits state events as it progresses.
- `launch_notes` table + `POST`/`GET` endpoints + WS broadcast on insert.
- **Feature flag** `kiosk_launch_cards_enabled` (default `false` initially) so it can be toggled off if it misbehaves live.

## Frontend work

- Replace "Live Activity" panel in `kiosk/src/app/debug/page.tsx` with `LaunchCard[]` component, grouped by `launch_id`.
- `LaunchCard` renders: pod number, sim name, state timeline (4 dots), staff notes inline, "Add note" composer, dismiss button (when resolved).
- Subscribe to `LaunchStateChanged` + `LaunchNoteAdded` via `useKioskSocket` hook.
- **Remove 30s `setInterval` poll.**
- Keep incidents sidebar + playbooks exactly as-is (out of scope).

## Deploy

- Venue (`.23` kiosk `:3300` + racecontrol `:8080`) + cloud (Bono VPS racecontrol + kiosk) **same commit**. Deploy parity rule.

## Gates required (per CLAUDE.md subagent gates)

- **gsd-ui-researcher** → `UI-SPEC.md` (new frontend component)
- **gsd-ui-auditor** → `UI-REVIEW.md` (before ship)
- **gsd-nyquist-auditor** → test coverage (launch state machine = business logic)
- **MMA audit** → mandatory for new cross-system bridge (rc-agent → server → kiosk via WS)
- **Both reasoning modes** (non-thinking + thinking model variants) per v27.0 standing rule

## Out of scope (explicit)

- Filter-by-sim-type bar
- Error taxonomy view
- Full launch-timeline drawer with stage breakdown
- Orphan/ghost billing detector
- Comment thread on incidents/activity
- Any UI surfacing of billing internals
- Gameplay diagnostics (launch phase only)

## Related prior work

- **Phase 275** (autonomous game launch fix) — rc-agent already has `game_launch_retry.rs`, hint-based retry classification, KB recording, mesh gossip of fixes. This phase surfaces those events; it does **not** rebuild them.
- **Phase 318** (launch intelligence) — server-side aggregation; read-only dependency.
- **Phase 362** (post-launch config verification Layer 3) — already ships; use its completion signal as "playable" marker.
- **Phase 311** (launch-billing coordination guard) — already handles the billing-not-ready case; this phase just renders its existing error state as card text.
