# Phase 194: Pod ID Normalization - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Introduce a single `normalize_pod_id()` function that canonicalizes all pod ID formats (pod-1, pod_1, POD_1, Pod-1) to one canonical form. Replace all 5+ inconsistent billing_alt_id workarounds in game_launcher.rs, billing.rs, and agent_senders lookups. Every map lookup in the system uses the canonical form after this phase.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key decisions:
- Canonical format choice (pod_1 vs pod-1 — pick one, use everywhere)
- Where to place the normalize function (rc-common for shared use)
- How to handle invalid pod IDs (return Result<String, Error>)
- Whether to normalize at API entry point or at each lookup (prefer entry point)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-common/` — shared types library, ideal location for normalize_pod_id()
- `crates/racecontrol/src/game_launcher.rs` — 5+ billing_alt_id workarounds to replace
- `crates/racecontrol/src/billing.rs` — billing timer lookups use inconsistent formats
- `crates/racecontrol/src/api/routes.rs` — API entry points where pod_id first arrives

### Established Patterns
- Pod IDs currently stored as String in HashMaps (active_timers, agent_senders, active_games)
- Two formats in use: "pod-N" (API/kiosk) and "pod_N" (agent registration)
- Workaround pattern: compute alt_id, check both keys with .or_else()

### Integration Points
- API route handlers (POST /games/launch, POST /billing/start, etc.)
- WebSocket agent registration (agent_senders map)
- Billing maps (active_timers, waiting_for_game)
- Game launcher maps (active_games)
- Dashboard broadcast events (pod_id in payloads)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Normalize once at entry, use canonical everywhere.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
