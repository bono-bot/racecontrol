---
created: 2026-04-27T15:48:00.000Z
title: Add /api/v1/debug/sync_push_recent endpoint for cross-hemisphere diagnostic
area: api
files:
  - crates/racecontrol/src/api/sync_cloud_push.rs
  - crates/racecontrol/src/api/routes.rs
---

## Problem

When V.11 forensic was running, james tried to grep the venue's
racecontrol stderr to confirm the post-fix dynamic baseline (whether
`Rejecting sync_push from same origin: local` lines stopped appearing).
He couldn't — the venue's `racecontrol/logs/racecontrol-*.jsonl` path
has been dead since 2026-04-08. Live racecontrol stderr from .23 is
redirected elsewhere by the schtask wrapper, and james's .27 sandbox
has no hop to it.

This made the "did the V.11 fix actually stop venue-to-cloud rejects
from venue's perspective?" question unanswerable from the AMPLIFIER
hemisphere. The cloud-side observation (rejects stopped, upserts
flowing) is sufficient for confidence but breaks falsifiability:
without venue-side logs, we cannot independently confirm.

This pattern will recur for any future sync-related fix where
verification needs both hemispheres' log views.

## Solution

Add a small read-only HTTP endpoint on each racecontrol instance
exposing the last N sync_push events as structured JSON:

```
GET /api/v1/debug/sync_push_recent?limit=20
->
{
  "events": [
    {"ts": "...", "direction": "incoming|outgoing",
     "origin": "local|cloud", "outcome": "accepted|rejected",
     "reason": "same_origin|hmac_failed|...|null",
     "records": 508, "table_breakdown": {...}}
  ]
}
```

Implementation outline:

1. Add a small in-memory ring buffer (~128 entries) to `AppState` —
   `sync_push_log: Arc<Mutex<VecDeque<SyncPushEvent>>>`.
2. Append to the ring buffer at each of these sites in
   `crates/racecontrol/src/api/sync_cloud_push.rs`:
   - `:67-73` reject path (same_origin)
   - `:120` upsert success path (per-table)
   - `:34-57` HMAC verification path (failure mode)
3. Also append at outbound-push sites in `cloud_sync_push.rs::do_push` so
   both directions are visible.
4. Add new route in `api/routes.rs` under `staff_routes` (requires staff
   JWT) — diagnostic endpoint, never public.
5. Limit query param: default 20, max 128 (prevent memory pressure).
6. Add an admin-page tile on the cloud admin dashboard that reads from
   it and renders a 2-column ledger (cloud-side events / venue-side
   events) — but that's optional polish, the API alone is the
   load-bearing piece.

Test plan:

- Unit test: ring buffer eviction at boundary (insert 130, read 128).
- Integration test: induce a same_origin reject, see it in the response.
- Cross-hemisphere: after deploy on cloud + venue, james can query
  venue's endpoint via Tailscale; bono can query cloud's; both sides
  produce reciprocal evidence.

Reciprocal property: every fix to cloud-only OR venue-only sync code
must add the corresponding event-emission line in this ring buffer. New
event types (auth_failed, schema_mismatch, etc.) can be added without
breaking consumers (events are tag-discriminated).

## Refs

- james's PACT-V11 baseline reply 2026-04-27 21:02 IST (commit `85eac75f`,
  comms-link/INBOX.md): "Add a /api endpoint: small
  /api/v1/debug/sync_push_recent that returns last N events. Useful as
  standing diagnostic regardless of V11."
- Memory: `project_pact_v11_false_positive_origin_collision.md`
- Related: SEC-GATE-02 hook diff-aware todo (`2026-04-27-make-sec-gate-02-hook-diff-aware-not-filename-blocking.md`)
