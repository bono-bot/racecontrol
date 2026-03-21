# Phase 88: Leaderboard Integration - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Multi-game lap data stored in existing laps table with sim_type. Track name normalization mapping. Leaderboard endpoints serve multi-game data with sim_type filtering.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion (all areas)

**Track Name Normalization:**
- Create a `track_name_map` table or in-memory mapping: per-game track IDs → canonical Racing Point track names
- Normalization happens at lap persistence time (lap_tracker.rs) — not query time
- Start with known tracks from AC catalog (36 tracks) and extend as other games produce data
- Unknown tracks pass through unchanged — don't block lap storage on missing mappings

**Leaderboard Filtering:**
- Add optional `?sim_type=` query parameter to all leaderboard endpoints
- Without filter: show all games (current behavior, backward compatible)
- With filter: show only laps from that sim type
- Public leaderboard and track leaderboard both get the filter
- `track_records` and `personal_bests` should be scoped per-game — a track record in F1 25 is separate from AC

**Lap Storage:**
- `lap_tracker.rs` already persists `sim_type` — no changes needed for storage
- Normalize track name before persistence using the mapping table
- Cloud sync: laps already in SYNC_TABLES — multi-game data syncs automatically

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Lap Persistence
- `crates/racecontrol/src/lap_tracker.rs` — persist_lap(), personal_bests update, track_records update, event scoring
- `crates/racecontrol/src/db/mod.rs` — laps table schema (has sim_type TEXT), personal_bests, track_records schemas

### Leaderboard Endpoints
- `crates/racecontrol/src/api/routes.rs` — public_leaderboard (line 9206), public_track_leaderboard (line 9289), track_leaderboard (line 1783)

### Cloud Sync
- `crates/racecontrol/src/cloud_sync.rs` — SYNC_TABLES for lap/record replication

</canonical_refs>

<code_context>
## Existing Code Insights

### Already Working
- `laps` table has `sim_type` column — all adapters set it correctly
- `lap_tracker::persist_lap()` inserts sim_type into DB
- `personal_bests` and `track_records` queries use track+car but NOT sim_type currently
- Cloud sync replicates laps automatically

### Changes Needed
- Leaderboard SQL queries need optional `WHERE sim_type = ?` clause
- `personal_bests` and `track_records` should include sim_type in uniqueness (track+car+sim_type)
- Track name normalization mapping (new table or in-code HashMap)
- Query parameter parsing for sim_type filter on leaderboard routes

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

- Cross-game unified leaderboard (v2 — XGAME-01) — showing best times regardless of game on shared tracks

</deferred>

---

*Phase: 88-leaderboard-integration*
*Context gathered: 2026-03-21*
