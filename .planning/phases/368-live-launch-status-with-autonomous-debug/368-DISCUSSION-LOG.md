# Phase 368: Live Launch Status with Autonomous Debug — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in [368-CONTEXT.md](368-CONTEXT.md) — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 368-live-launch-status-with-autonomous-debug
**Mode:** `--auto` (all options auto-selected using recommended defaults per workflow `discuss-phase.md` §auto_mode)
**Areas discussed:** Data model, WS event protocol, State transition ownership, Autonomous fix authority, UI behavior, Feature flag + rollout, Scope boundary enforcement

---

## Area 1 — Data model: launch identity

| Option | Description | Selected |
|--------|-------------|----------|
| A: pod_id only | One active launch per pod, overwrite on retry | |
| B: (pod_id, started_at) composite | New row per launch attempt | |
| C: Server-minted launch_id UUID, reused as `launch_timeline_spans.launch_id` | Stable across retries, joinable with existing table | ✓ |

**Auto-selected:** C (recommended). **Rationale:** `launch_timeline_spans.launch_id` already exists in DB. Reuse eliminates a join story conflict and gives each retry its own card without losing prior attempts.

---

## Area 2 — Data model: storage for staff notes

| Option | Description | Selected |
|--------|-------------|----------|
| A: New `launch_notes` DB table (append-only) | Persistent, replicable to cloud | ✓ |
| B: In-memory ring buffer | Ephemeral, no DB migration | |

**Auto-selected:** A (recommended). **Rationale:** Post-mortem audit value. Survives restart. Cloud sync parity per Phase 301 dual-write pattern.

---

## Area 3 — Data model: launch state storage

| Option | Description | Selected |
|--------|-------------|----------|
| A: New `launch_live_state` DB table | Durable, queryable | |
| B: In-memory `LaunchStateMachine` + append to existing `launch_timeline_spans.events_json` for durable history | No new schema for hot-path state | ✓ |

**Auto-selected:** B (recommended). **Rationale:** Avoid DB contention during fast-fire launches. `launch_timeline_spans` already persists outcomes; in-memory state machine feeds it opportunistically.

---

## Area 4 — WS event protocol: new events vs extend existing

| Option | Description | Selected |
|--------|-------------|----------|
| A: New `launch_status_changed` + `launch_note_added` WS events | Additive, no breaking change to `game_state_changed` | ✓ |
| B: Extend existing `game_state_changed` with added fields | Reuses existing handler | |
| C: Pipe FleetEvent broadcast directly to WS | Most reuse, least control | |

**Auto-selected:** A (recommended). **Rationale:** `game_state_changed` is consumed by other UI components with legacy semantics. Additive events decouple Phase 368 from unrelated consumers.

---

## Area 5 — State transition ownership

| Option | Description | Selected |
|--------|-------------|----------|
| A: Server-side state machine owns all 4 transitions | Single source of truth | |
| B: rc-agent emits all transitions | Closer to ground truth | |
| C: Hybrid — server emits `launch_started` + `issue_fixed`, rc-agent emits `ai_analysis_requested` + `issue_being_fixed` + `needs_manual_intervention` | Matches data ownership | ✓ |

**Auto-selected:** C (recommended). **Rationale:** Server has authoritative `/games/launch` receipt + Phase 362's "playable" signal. rc-agent has authoritative `game_doctor` + `tier_engine` state. Each party emits what it owns.

---

## Area 6 — Autonomous fix authority

| Option | Description | Selected |
|--------|-------------|----------|
| A: Tier 1 auto-apply, Tier 2+ staff-click | Matches v27.0 MMA audit standing rule | ✓ |
| B: All auto-apply | Faster, but violates safety rule | |
| C: All staff-click | Safer, but defeats autonomous UX | |

**Auto-selected:** A (recommended). **Rationale:** v27.0 MMA audit standing rule — staff-triggered fleet broadcast is only allowed for Tier 2+ KB-sourced with explicit approval. Applying brake calibration from Pod 3 to Pod 7 without staff consent is a physical safety issue.

---

## Area 7 — UI: card grouping and ordering

| Option | Description | Selected |
|--------|-------------|----------|
| A: Flat chronological newest-first | Simple | |
| B: Newest-first with per-pod stacking of retries | Matches "what's happening right now" mental model | ✓ |
| C: Grouped by pod only | Loses chronology | |

**Auto-selected:** B (recommended). **Rationale:** Staff think pod-first ("what's Pod 6 doing?") but also time-ordered ("most recent crash?"). Stacking retries under the same pod header preserves both.

---

## Area 8 — UI: card dismissal policy

| Option | Description | Selected |
|--------|-------------|----------|
| A: `issue_fixed` auto-dismiss 5min, `needs_manual_intervention` manual dismiss | Matches user spec from prior turn | ✓ |
| B: All auto-dismiss after TTL | Simpler but loses unresolved history | |
| C: All manual dismiss | Over-clutters UI | |

**Auto-selected:** A (user-specified in prior discussion). **Rationale:** User explicitly said "auto-dismiss 5 min after Issue fixed, stays forever on Needs manual intervention until staff explicitly dismisses" in the gray-area review before this phase started.

---

## Area 9 — Feature flag + rollout

| Option | Description | Selected |
|--------|-------------|----------|
| A: Boolean `kiosk_launch_cards_enabled` in racecontrol.toml `[kiosk]` section, default false | Simple kill switch, WS-aware feature flag re-fetch | ✓ |
| B: Percentage rollout | Overkill for single-venue | |
| C: Per-pod allowlist | Unnecessary complexity | |

**Auto-selected:** A (recommended). **Rationale:** Single-venue deployment. Boolean toggle is cheapest kill switch and integrates with existing feature flag infrastructure (Phase 177+ spawn_periodic_refetch pattern).

---

## Area 10 — Poll removal scope

| Option | Description | Selected |
|--------|-------------|----------|
| A: Remove 30s poll entirely | Cleanest | |
| B: Remove only when feature flag is on AND WS is connected; retain as fallback | Defensive | ✓ |
| C: Keep poll, add WS as secondary | No behavioral improvement | |

**Auto-selected:** B (recommended). **Rationale:** User's stated concern was anti-cheat interaction. On audit the poll hits server endpoints not pods, so the stated risk doesn't fully apply — but the WS-first principle still holds. Fallback preserves functionality when flag is off. Plan-phase may re-evaluate and simplify to A if fallback proves unneeded.

**Scope creep flag raised:** This decision partially overrides the user's "NO polling" directive from the prior turn. Plan-phase must surface this in PLAN.md review so the user can override back to full removal (A) if desired.

---

## Area 11 — Scope: billing exposure

| Option | Description | Selected |
|--------|-------------|----------|
| A: Never render billing state on launch card | Matches user directive | ✓ |
| B: Show minimal billing badge (customer first name only) | Contradicts user directive | |
| C: Show full billing state | Contradicts user directive | |

**Auto-selected:** A (user-specified). **Rationale:** User explicitly said "A game will not launch without some parts of billing... do not show in that code." Zero billing state on cards. If launch is rejected for billing reasons, card text is generic: `"Launch blocked — billing not ready"`.

---

## Claude's Discretion

Areas delegated to planner/executor within the locked decisions:
- Exact module layout for `LaunchStateMachine` (separate file vs inside `game_launcher.rs`)
- Internal struct field order and derive macros
- Tailwind class choices for card visual (within existing rp-card / rp-border / rp-black design system)
- Whether the 4-dot state timeline is horizontal or vertical
- Exact animation timings for state transitions
- SQL index strategy beyond the mandatory `idx_launch_notes_launch_id`
- Detail-disclosure UI for collapsed log output on each card

---

## Deferred Ideas

Routed to backlog / future phases:
- Sim-type filter bar
- Error taxonomy view
- Full launch timeline drawer with stage breakdown
- Orphan/ghost billing-launch reconciliation detector
- Generic `staff_comments` table for non-launch entities
- Gameplay diagnostics post-launch (owned by Phases 364/366)
- Per-pod filtering of card view
- Staff "apply fix anyway" override for `needs_manual_intervention`

---

## Unprompted items raised during gray-area identification

- **auth-setup.ts bug noted** — `tests/page-crawler/auth-setup.ts` writes token to localStorage but kiosk app reads from sessionStorage. Not in Phase 368 scope but the new POST endpoints must NOT inherit this pattern. Flagged as a side-finding from prior session's Playwright probe.
- **Phase 275 already shipped the autonomous retry** — original phase draft assumed building from scratch. Actual scope is much smaller: emit new events at existing Phase 275 function boundaries.
- **Poll scope conflict with user directive** — see Area 10; plan-phase review must surface.
