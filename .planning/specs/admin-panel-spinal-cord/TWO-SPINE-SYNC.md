# Two-spine sync — venue admin ↔ cloud admin

**Date:** 2026-04-23
**Status:** DESIGN (decision recorded; no code change)
**Companion:** `GATEWAY-CONTRACT.md`, `project_admin_panel_operator_model.md` doctrine §5

## Context

The body has TWO spines (venue admin `:3201` + cloud admin `admin.racingpoint.cloud`) and TWO brains (venue racecontrol Server `.23` + cloud racecontrol Bono VPS). Skeletons must stay in sync. This spec defines HOW.

The earlier "A3 collapsed — same repo serves both" framing was wrong-shape. They're separate organs that must IMPLEMENT THE SAME CONTRACT but stay independently deployed and tested.

## Decision

**Spines do not talk to each other directly.** Each spine talks only to its own brain. Sync between spines is inherited from existing brain-to-brain sync.

**Why option 2 over option 1 (direct spine-to-spine):**
- Reuses tested infrastructure: Phase 301 cloud_data_sync_v2 (30s pull/push, cloud-authoritative for drivers + pricing, local-authoritative for billing + laps + game state)
- No new authentication channel between spines
- No new failure mode (spine A unreachable doesn't take down spine B's view)
- Tradeoff: 30s lag means spines can briefly disagree about settings — acceptable for pricing / promo / config; not acceptable for live billing state, but live billing flows aren't spine-to-spine anyway

## How writes propagate

Operator changes pricing in venue admin:
1. Venue admin writes to its own brain (venue RC `pricing_rules` table)
2. Venue brain → cloud brain via Phase 301 sync (30s) — cloud is authoritative for pricing per existing rule
3. Cloud admin's next read fetches from cloud brain — sees the change
4. Cloud admin pushes settings to its connected surfaces (per doctrine §2 reflex)

Operator changes pricing in cloud admin:
1. Cloud admin writes to its own brain (cloud RC `pricing_rules`) — cloud is authoritative
2. Cloud brain → venue brain via Phase 301 (30s)
3. Venue admin's next read sees the change, pushes to venue surfaces

## How reads stay consistent

Each spine reads from its own brain. Brains are in sync (within 30s). Therefore spines are in sync (within 30s). Reads inherit consistency from brain sync — no spine-level cache that could drift further.

If a spine maintains a per-process cache (e.g. settings TTL'd in memory for 60s), invalidate on write OR keep the cache TTL ≤ brain-sync interval.

## Failure modes

| Mode | Effect | Mitigation |
|---|---|---|
| Cloud brain offline | Cloud admin sees stale data; venue admin unaffected | Cloud admin shows "stale read" warning when last brain success > 60s |
| Venue brain offline | Venue admin sees stale data; cloud admin unaffected | Same — staleness warning at 60s |
| Both brains online but Phase 301 stalled | Spines diverge: venue write not reflected in cloud spine, vice versa | Reconciliation timer (e.g. 5min) fires if Phase 301 lag > threshold; alert via WhatsApp |
| Spine writes to brain, surface push fails | Surface shows old value, brain has new value (split-state per doctrine §3) | Surface fetches from brain on connect / on user action; admin retries push N times then marks-divergent |

## What this is NOT solving

- Live billing session state across two venues (irrelevant — only one venue exists)
- Multi-tenancy where one cloud admin serves N venues (the doctrine assumes one body per cloud admin instance; tenant isolation is a separate spec)
- Partition-tolerance during full network outage (operator sees stale data; no automated resolution beyond Phase 301 retry)

## Implementation cost

Zero new code on the spine. Phase 301 already does the brain-to-brain sync. The only spine-side work is:
- Cache invalidation on write (if any spine adds caching beyond what exists)
- "Stale read" indicator on the admin UI when last brain success > 60s — UI work, not infra
- Reconciliation timer for split-state detection (lives in admin or as a separate cron)

The "two spines" model is a doctrinal clarification, not a new technical layer. Most of the work was already done.
