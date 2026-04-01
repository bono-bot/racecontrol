# Phase 301: Cloud Data Sync v2 - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Key intelligence tables are synced to Bono VPS and the system is ready for cross-venue data flows. Extend cloud_sync.rs to sync fleet_solutions, model_evaluations, and metrics_rollups tables. Server-authoritative for venue data flowing to cloud, cloud-authoritative for cross-venue data flowing back. Last-write-wins conflict resolution with venue_id tiebreaker. Admin dashboard sync status panel.

Requirements: SYNC-01 through SYNC-06

Success Criteria:
1. fleet_solutions, model_evaluations, and metrics_rollups rows written at the venue appear in the Bono VPS database within the next sync cycle (server-authoritative direction)
2. A row written with a future venue_id on Bono VPS flows back to the venue database on the next sync (cloud-authoritative direction established)
3. When two writes target the same row, the row with the later updated_at timestamp wins; if timestamps are equal, the row with the lexicographically smaller venue_id wins
4. Admin dashboard sync panel shows last sync timestamp, number of tables synced, and running conflict count

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

Key constraints:
- Extend existing cloud_sync.rs (additive, not rewrite)
- Cloud sync uses existing HTTP-based sync mechanism to Bono VPS racecontrol at :8080
- Server-authoritative for: fleet_solutions, model_evaluations, metrics_rollups
- Cloud-authoritative for: cross-venue solutions (future venue_id rows)
- Conflict resolution: last-write-wins by updated_at, venue_id tiebreaker on equal timestamps
- Admin panel in racingpoint-admin Next.js app

</decisions>

<code_context>
## Existing Code Insights

Codebase context will be gathered during plan-phase research.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — discuss phase skipped.

</deferred>
