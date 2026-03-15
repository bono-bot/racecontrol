# Phase 14: Events and Championships - Research

**Researched:** 2026-03-15
**Domain:** Hotlap events, group session scoring, multi-round championships, and competitive cloud sync — all built on top of the Phase 12 schema foundation and Phase 13 public PWA architecture
**Confidence:** HIGH — all findings grounded in direct codebase inspection of db/mod.rs, lap_tracker.rs, routes.rs, cloud_sync.rs, integration tests, and all Phase 12/13 summaries

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| EVT-01 | Staff can create a hotlap event with track, car class, start/end date, description, reference time | `hotlap_events` table exists with all columns; staff endpoint pattern uses `check_terminal_auth()` |
| EVT-02 | Laps automatically enter the matching hotlap event when track, car class, and date range match | `persist_lap()` has the insertion point; `car_class` and `track` already on every lap; auto-entry is a new step after lap insert |
| EVT-03 | User can view public event leaderboard showing position, driver, time, date, vehicle, venue | New public endpoint `/public/events/{id}/leaderboard`; follows existing `public_track_leaderboard` pattern |
| EVT-04 | Event leaderboard displays car class tabs — one ranking per class within the event | `hotlap_event_entries.driver_id` + join on laps gives class; query groups by class |
| EVT-05 | 107% rule: laps slower than 107% of class leader are flagged | Pure computation at event entry insert time or at read time in the leaderboard query |
| EVT-06 | Gold/Silver/Bronze badges auto-calculated from staff-set reference_time_ms (within 2%/5%/8%) | `hotlap_events.reference_time_ms` already exists; badge logic is pure arithmetic |
| EVT-07 | User can browse all active and past hotlap events from an events listing page | New public endpoint `/public/events` + PWA page `/events` |
| GRP-01 | When a multiplayer group session completes, race results auto-scored using F1 points (25/18/15...) | `group_sessions`, `group_session_members`, `multiplayer_results` tables exist; need scoring on completion |
| GRP-02 | User can view group event summary showing position, driver, qual points, race points, best laps, wins, total | `hotlap_event_entries` has points, position, best laps; group event links via `ac_session_id` |
| GRP-03 | User can view per-session breakdowns within a group event (qualification, race) | Sessions table has `type` (qual/race); `laps` table has `session_id`; need to surface these |
| GRP-04 | Group event results include gap-to-leader timing | `hotlap_event_entries.gap_to_leader_ms` column already exists |
| CHP-01 | Staff can create a championship, assign group event rounds to it | `championships` and `championship_rounds` tables exist; need staff create/assign endpoints |
| CHP-02 | Championship standings auto-calculated by summing F1 points across rounds | `championship_standings` table exists; need a recalculate function |
| CHP-03 | User can view championship standings page with overall table and per-round breakdown | New public endpoint `/public/championships/{id}/standings` + PWA page |
| CHP-04 | Championship tiebreaker follows F1 rules: most wins, then most P2s, then most P3s, then earliest | `championship_standings.wins` exists; need P2/P3 counts — may need ALTER TABLE or compute at read time |
| CHP-05 | Event entries have result_status (finished/DNS/DNF/pending) for correct scoring | `hotlap_event_entries.result_status` column with CHECK constraint already exists |
| SYNC-01 | Cloud sync extends to push hotlap_events, event_entries, championships, standings, driver_ratings | `collect_push_payload()` in cloud_sync.rs; new table sections follow existing pattern |
| SYNC-02 | Telemetry sync is targeted — only event-entered lap telemetry synced | Need event_lap_id set tracking + conditional telemetry push in collect_push_payload() |
| SYNC-03 | Competitive data sync is venue-authoritative one-way push — cloud never writes back | Existing architecture is already venue-push-only for laps/records; competitive tables follow same pattern |
</phase_requirements>

---

## Summary

Phase 14 builds on a complete schema foundation. Every table this phase needs — `hotlap_events`, `hotlap_event_entries`, `championships`, `championship_rounds`, `championship_standings`, `driver_ratings` — was created and indexed in Phase 12 (`db/mod.rs` lines 1816-1957). The `laps` table already has `car_class`, `suspect`, `valid`, `track`, and `sim_type`. The `hotlap_event_entries` table already has `position`, `points`, `badge`, `gap_to_leader_ms`, `within_107_percent`, and `result_status`. Zero schema changes are needed to start implementation.

The gap is entirely in three layers: (1) the staff-facing API endpoints to create events and championships, (2) the automatic lap-to-event matching logic inside `persist_lap()`, (3) the public read endpoints and PWA pages, and (4) cloud_sync.rs extension to push competitive tables. All four layers have clear implementation patterns established by Phases 12 and 13. The group session scoring (GRP-01 through GRP-04) is the most novel logic — it requires connecting the existing `group_sessions`/`group_session_members`/`multiplayer_results` data to the `hotlap_events` competitive framework, and applying F1 points arithmetic. The championship tiebreaker (CHP-04) requires tracking per-position finish counts that are not currently in `championship_standings`; this is the only schema gap — two ALTER TABLE statements to add `p2_count` and `p3_count` columns.

**Primary recommendation:** Build in five layers — (1) staff event/championship CRUD endpoints, (2) auto-entry hook in `persist_lap()`, (3) group scoring logic on session completion, (4) public read endpoints + PWA pages, (5) cloud_sync extension. Each layer is independently testable with the existing `cargo test -p rc-core` infrastructure.

---

## Standard Stack

### Core (all already in place from Phases 12/13)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| Rust/Axum | 0.8 | HTTP API endpoints | Zero new crates needed |
| sqlx | 0.8 | Async SQLite queries | WAL-tuned pool from Phase 12 |
| serde_json | in Cargo.toml | JSON response shaping | `json!()` macro throughout |
| SQLite 3.25+ | bundled | Storage (window functions available) | All 6 competitive tables indexed |
| Next.js App Router | in pwa/package.json | PWA pages | "use client" + publicApi pattern from Phase 13 |
| React | in pwa/package.json | UI components | Hooks pattern from Phase 13 |
| uuid | in Cargo.toml | Generate event/championship IDs | Already imported |
| chrono | in Cargo.toml | Date range comparison for event matching | Already in scope |

### No New Dependencies Required

Phase 14 adds zero new Rust crates and zero new npm packages. F1 points computation, 107% rule, badge arithmetic — all pure Rust/SQL with existing tools.

### F1 2010 Points System (confirmed from championships.scoring_system column)

```
P1:25, P2:18, P3:15, P4:12, P5:10, P6:8, P7:6, P8:4, P9:2, P10:1
DNS/DNF: 0 points
```

This is what the schema encodes via `scoring_system = 'f1_2010'`.

---

## Architecture Patterns

### Recommended Project Structure (Phase 14 additions)

```
crates/rc-core/src/
├── db/mod.rs              MODIFY: 2 ALTER TABLE for championship tiebreaker counts
├── lap_tracker.rs         MODIFY: add auto-event-entry after lap insert
├── api/routes.rs          MODIFY: staff endpoints + public endpoints + cloud sync push
├── cloud_sync.rs          MODIFY: add competitive tables to collect_push_payload()

pwa/src/
├── app/
│   ├── events/
│   │   ├── page.tsx              NEW: events listing page
│   │   └── [id]/page.tsx         NEW: event leaderboard page
│   └── championships/
│       ├── page.tsx              NEW: championships listing page
│       └── [id]/page.tsx         NEW: championship standings page
└── lib/api.ts             MODIFY: add publicApi.events, .eventLeaderboard,
                                          .championships, .championshipStandings
```

### Pattern 1: Staff Event/Championship Endpoints (using existing `check_terminal_auth`)

Staff endpoints use the existing `check_terminal_auth()` function. This is the pattern for all staff-only write operations:

```rust
// Source: crates/rc-core/src/api/routes.rs line 5968, 6911
async fn create_hotlap_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO hotlap_events
            (id, name, description, track, car, car_class, sim_type, starts_at, ends_at,
             reference_time_ms, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'upcoming', datetime('now'), datetime('now'))"
    )
    // ... bind all fields from body ...
    .execute(&state.db).await;
    Json(json!({ "id": id, "status": "created" }))
}
```

Route registration follows the existing pattern at lines 247-255 of routes.rs, under the same router builder:
```rust
// In the route builder, alongside existing /public/* routes:
.route("/staff/events", post(create_hotlap_event).get(list_hotlap_events))
.route("/staff/events/{id}", get(get_hotlap_event).put(update_hotlap_event))
.route("/staff/championships", post(create_championship).get(list_championships))
.route("/staff/championships/{id}/rounds", post(add_championship_round))
```

### Pattern 2: Auto Event Entry in `persist_lap()`

This is the most critical new behavior. After the lap is inserted (step 1 in `persist_lap()`), before returning, check if the lap matches any active hotlap event:

```rust
// In persist_lap() — after lap INSERT succeeds, after personal_best and track_record logic
// Step NEW: Auto-enter into matching active hotlap events

// Only attempt auto-entry if lap is valid and not suspect
if lap.valid && suspect_flag == 0 {
    if let Some(ref class) = car_class {
        let sim_str = format!("{:?}", lap.sim_type).to_lowercase();
        // Find all active events matching track + car_class + sim_type + date range
        let matching_events = sqlx::query_as::<_, (String, Option<i64>)>(
            "SELECT id, reference_time_ms
             FROM hotlap_events
             WHERE track = ?
               AND car_class = ?
               AND sim_type = ?
               AND status IN ('active', 'upcoming')
               AND (starts_at IS NULL OR starts_at <= datetime('now'))
               AND (ends_at IS NULL OR ends_at >= datetime('now'))",
        )
        .bind(&lap.track)
        .bind(class)
        .bind(&sim_str)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        for (event_id, reference_time_ms) in matching_events {
            // Check if driver already has a better entry
            let existing = sqlx::query_as::<_, (i64,)>(
                "SELECT lap_time_ms FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?"
            )
            .bind(&event_id).bind(&lap.driver_id)
            .fetch_optional(&state.db).await.ok().flatten();

            let is_faster = match existing {
                Some((current_ms,)) => (lap.lap_time_ms as i64) < current_ms,
                None => true,
            };

            if !is_faster { continue; }

            // Compute badge
            let badge = reference_time_ms.map(|ref_ms| {
                let ratio = lap.lap_time_ms as f64 / ref_ms as f64;
                if ratio <= 1.02 { "gold" }
                else if ratio <= 1.05 { "silver" }
                else if ratio <= 1.08 { "bronze" }
                else { "none" }
            });

            let entry_id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO hotlap_event_entries
                    (id, event_id, driver_id, lap_id, lap_time_ms,
                     sector1_ms, sector2_ms, sector3_ms,
                     badge, result_status, entered_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'finished', datetime('now'))
                 ON CONFLICT(event_id, driver_id) DO UPDATE SET
                    lap_id = excluded.lap_id,
                    lap_time_ms = excluded.lap_time_ms,
                    sector1_ms = excluded.sector1_ms,
                    sector2_ms = excluded.sector2_ms,
                    sector3_ms = excluded.sector3_ms,
                    badge = excluded.badge,
                    entered_at = excluded.entered_at"
            )
            .bind(&entry_id).bind(&event_id).bind(&lap.driver_id)
            .bind(&lap.id).bind(lap.lap_time_ms as i64)
            .bind(lap.sector1_ms.map(|v| v as i64))
            .bind(lap.sector2_ms.map(|v| v as i64))
            .bind(lap.sector3_ms.map(|v| v as i64))
            .bind(badge.unwrap_or("none"))
            .execute(&state.db).await;

            tracing::info!(
                "Auto-entered driver {} into event {} with time {}ms",
                lap.driver_id, event_id, lap.lap_time_ms
            );
        }
    }
}
```

**CRITICAL DECISION:** The `ON CONFLICT(event_id, driver_id) DO UPDATE` replaces the existing entry only when the new lap is faster. This means the `is_faster` check gates the upsert — don't upsert if not faster.

### Pattern 3: Position and Gap-to-Leader Recalculation

After any new entry is inserted, the leaderboard positions and gap-to-leader must be updated:

```rust
// After all auto-entry upserts for a given event, recalculate positions
async fn recalculate_event_positions(pool: &SqlitePool, event_id: &str) {
    // Leader is the minimum lap_time_ms among 'finished' entries
    // Update position + gap_to_leader + within_107_percent in one pass
    let _ = sqlx::query(
        "WITH ranked AS (
            SELECT id,
                   ROW_NUMBER() OVER (ORDER BY lap_time_ms ASC) as pos,
                   lap_time_ms,
                   MIN(lap_time_ms) OVER () as leader_ms
            FROM hotlap_event_entries
            WHERE event_id = ? AND result_status = 'finished'
        )
        UPDATE hotlap_event_entries SET
            position = (SELECT pos FROM ranked WHERE ranked.id = hotlap_event_entries.id),
            gap_to_leader_ms = lap_time_ms - (SELECT MIN(lap_time_ms) FROM hotlap_event_entries WHERE event_id = ? AND result_status = 'finished'),
            within_107_percent = CASE WHEN lap_time_ms <= (SELECT MIN(lap_time_ms) FROM hotlap_event_entries WHERE event_id = ? AND result_status = 'finished') * 1.07 THEN 1 ELSE 0 END
        WHERE event_id = ? AND result_status = 'finished'"
    )
    .bind(event_id).bind(event_id).bind(event_id).bind(event_id)
    .execute(pool).await;
}
```

**NOTE:** SQLite 3.25+ supports window functions. The `ROW_NUMBER() OVER (ORDER BY ...)` pattern is valid. The Phase 12 research confirmed SQLite version is 3.25+.

### Pattern 4: F1 Points Array (Const, Not DB Lookup)

The F1 2010 points are a static array — no need to store per-position in DB:

```rust
// In a scoring module or inline in route handlers
const F1_2010_POINTS: [i64; 10] = [25, 18, 15, 12, 10, 8, 6, 4, 2, 1];

fn f1_points_for_position(position: i64) -> i64 {
    if position < 1 || position > 10 { return 0; }
    F1_2010_POINTS[(position - 1) as usize]
}
```

### Pattern 5: Public Event Endpoints (follow Phase 13 public/* pattern exactly)

```rust
// Source: routes.rs lines 247-255 — follow this exact pattern
.route("/public/events", get(public_events_list))
.route("/public/events/{id}", get(public_event_leaderboard))
.route("/public/championships", get(public_championships_list))
.route("/public/championships/{id}", get(public_championship_standings))
```

No auth middleware — follow the same no-auth setup as Phase 13's `/public/drivers`.

### Pattern 6: Cloud Sync Extension (follow existing collect_push_payload pattern exactly)

```rust
// Source: crates/rc-core/src/cloud_sync.rs lines 246-461 — add these sections
// Add after billing_events section, before Ok((payload, has_data))

// Competitive tables: always push all (small tables, venue-authoritative)
let events = sqlx::query_as::<_, (String,)>(
    "SELECT json_object('id', id, 'name', name, 'track', track, 'car', car,
        'car_class', car_class, 'sim_type', sim_type, 'status', status,
        'starts_at', starts_at, 'ends_at', ends_at,
        'reference_time_ms', reference_time_ms, 'updated_at', updated_at)
     FROM hotlap_events WHERE updated_at > ?",
)
.bind(&last_push)
.fetch_all(&state.db).await?;
// ... same pattern as existing track_records/personal_bests push ...

// Targeted telemetry: only for event-entered laps
let event_telemetry = sqlx::query_as::<_, (String,)>(
    "SELECT json_object('lap_id', ts.lap_id, 'offset_ms', ts.offset_ms,
        'speed', ts.speed, 'throttle', ts.throttle, 'brake', ts.brake,
        'steering', ts.steering, 'gear', ts.gear, 'rpm', ts.rpm,
        'pos_x', ts.pos_x, 'pos_y', ts.pos_y, 'pos_z', ts.pos_z)
     FROM telemetry_samples ts
     INNER JOIN hotlap_event_entries hee ON hee.lap_id = ts.lap_id
     WHERE ts.lap_id IN (SELECT lap_id FROM hotlap_event_entries WHERE lap_id IS NOT NULL)
       AND ts.lap_id > ?  -- or use a separate push cursor
     LIMIT 10000",
)
// Note: needs a separate push cursor for telemetry to avoid re-sending
```

**SYNC-02 key decision:** Use a separate `competitive_telemetry_push` entry in `sync_state` table to track what telemetry has been pushed. The existing `sync_state` table already handles per-table push timestamps. Only event-entered lap IDs are candidates for telemetry sync.

### Pattern 7: Championship Tiebreaker — Schema Gap Resolution

`championship_standings` currently has `wins` but not `p2_count` or `p3_count`. F1 tiebreaker requires these. The fix is two idempotent ALTER TABLE statements:

```rust
// In db/mod.rs migrate() — new additions for Phase 14
let _ = sqlx::query("ALTER TABLE championship_standings ADD COLUMN p2_count INTEGER DEFAULT 0")
    .execute(pool).await;
let _ = sqlx::query("ALTER TABLE championship_standings ADD COLUMN p3_count INTEGER DEFAULT 0")
    .execute(pool).await;
// Also add earliest_podium_at for the "earliest occurrence" tiebreaker
let _ = sqlx::query("ALTER TABLE championship_standings ADD COLUMN earliest_win_at TEXT")
    .execute(pool).await;
```

Alternatively, p2/p3 counts can be computed at read time from `hotlap_event_entries` — joining on championship_rounds to know which events belong to the championship. This avoids the schema change but makes the standings query more complex. **Recommendation: compute at read time for correctness simplicity** — no denormalization, no risk of stale counts.

### Pattern 8: Group Session → Event Scoring (GRP-01 to GRP-04)

The existing `multiplayer_results` table (referenced in routes.rs line 10910) has `position`, `best_lap_ms`, `total_time_ms`, `laps_completed`, `dnf`. When a group session completes and a `hotlap_event` is linked to it (via `hotlap_events.championship_id` OR a new `group_session_id` column on `hotlap_events`), the scoring trigger is:

**CRITICAL GAP:** There is currently no FK link between `group_sessions` and `hotlap_events`. The schema design in Phase 12 linked them via `hotlap_events.championship_id`, but a group session needs to produce results for a specific hotlap event round. Need to evaluate how to connect them.

**Options:**
1. Add `hotlap_event_id TEXT` column to `group_sessions` — set when staff creates the session for an event round
2. Staff manually links a completed group session to a hotlap event after the fact via a staff endpoint

**Recommendation:** Option 1 (schema extension via ALTER TABLE) is cleaner. Staff creates a `hotlap_event` for each group race, then creates a `group_session` with `hotlap_event_id` pointing to it. On `group_session` completion, the scoring function reads `multiplayer_results` and upserts into `hotlap_event_entries`.

### Anti-Patterns to Avoid

- **Don't compute 107% in the leaderboard query with a subquery per row.** Pre-compute `within_107_percent` at entry insert time (as shown in Pattern 2) and index it. Subquery-per-row on large events is O(n²).
- **Don't lock the position column.** Position is a derived/cached field — always recalculate from the sorted entry list. Never trust stored position without recalculating on inserts.
- **Don't sync all telemetry.** SYNC-02 explicitly limits to event-entered laps only. Syncing all telemetry_samples would be unbounded in size.
- **Don't expose driver PII in public event endpoints.** Follow Phase 13's public_driver_profile pattern: `CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| F1 points table | Dynamic DB-driven points table | `const F1_2010_POINTS: [i64; 10]` array | Fixed for this scoring system; DB-driven adds zero value at venue scale |
| 107% computation | Per-query subselect | Pre-computed `within_107_percent` column on insert, indexed | O(n) insert vs O(n²) per read |
| Position ranking | Application-level sort + loop | SQLite `ROW_NUMBER() OVER (ORDER BY lap_time_ms)` window function (3.25+) | Correct, atomic, DB-handles ties |
| Badge computation | Complex rule engine | Simple ratio check: `lap_ms / ref_ms <= threshold` | Three thresholds, no complexity |
| Championship recalculation | Event-triggered incremental updates | Full recalculate on each standings fetch (small table) or on demand via staff endpoint | At 8 pods, standings have <100 rows; full recalc is sub-millisecond |
| UUID generation | Custom ID scheme | `uuid::Uuid::new_v4().to_string()` — already in Cargo.toml | Consistent with all other tables |
| Telemetry sync all laps | Unbounded telemetry push | Join telemetry_samples on hotlap_event_entries.lap_id | SYNC-02 requirement; bounded, controlled volume |

---

## Common Pitfalls

### Pitfall 1: Auto-Entry Matches on Wrong sim_type

**What goes wrong:** `hotlap_events.sim_type` is stored as e.g. `"assetto_corsa"`. The `lap.sim_type` in `LapData` is an enum that gets serialized via `format!("{:?}", lap.sim_type).to_lowercase()`. If the serialization format changes or diverges from the stored string, no events will match.

**Why it happens:** `lap_tracker.rs` line 71 uses `format!("{:?}", lap.sim_type).to_lowercase()` for the laps table. The hotlap_events table stores `sim_type TEXT NOT NULL DEFAULT 'assetto_corsa'`. These must match exactly.

**How to avoid:** In the auto-entry query, use the same `format!("{:?}", lap.sim_type).to_lowercase()` expression to build the WHERE clause filter. Verify in tests that the enum-to-string produces `"assetto_corsa"` not `"AssettoCorsa"`.

**Warning signs:** Integration test shows 0 matching events when sim_type matches by human inspection.

### Pitfall 2: Position Recalculation Race Condition

**What goes wrong:** Two laps arrive for the same event within the same sync cycle. Both trigger auto-entry. Both call `recalculate_event_positions()`. The second recalculation correctly overwrites the first — but if they run concurrently on different tokio tasks, positions may be computed against partially-updated state.

**Why it happens:** `persist_lap()` runs concurrently for multiple pods (each pod sends laps independently).

**How to avoid:** `recalculate_event_positions()` is a pure UPDATE on the DB — SQLite's serialized writer ensures correctness. The second call simply produces the final correct state. No mutex needed — SQLite WAL serializes writes. Just ensure the recalculation runs within the same DB connection (not spawned into a separate task).

**Warning signs:** Positions are out of order or two drivers share the same position number.

### Pitfall 3: Group Session to Event Link Missing

**What goes wrong:** Staff creates a group session and runs a race, but there's no FK from `group_sessions` to `hotlap_events`. GRP-01 scoring has nowhere to write results.

**Why it happens:** Phase 12 schema didn't include this FK (it linked events to championships, not group sessions to events).

**How to avoid:** Add `hotlap_event_id TEXT` to `group_sessions` via `ALTER TABLE` in db/mod.rs. Add a staff endpoint to set this before the session starts. Scoring on completion reads `hotlap_event_id` to know where to write.

**Warning signs:** The scoring function for GRP-01 has no way to know which event to write results into.

### Pitfall 4: Championship Standings Stale After New Round

**What goes wrong:** Staff adds a new round to an existing championship. The standings page shows old data because standings were computed before the round was added.

**Why it happens:** `championship_standings` is a materialized table (pre-computed totals), not a view. Adding a round doesn't automatically trigger recalculation.

**How to avoid:** Two options:
1. Recalculate standings on every `/public/championships/{id}/standings` read (correct but adds latency)
2. Expose a staff endpoint `POST /staff/championships/{id}/recalculate` and call it after adding rounds

**Recommendation:** Option 1 for correctness at venue scale. The standings query joins at most ~8 drivers × ~10 rounds = 80 rows. Sub-millisecond.

**Warning signs:** Adding a round produces stale standings until manual refresh.

### Pitfall 5: SYNC-02 Telemetry Unbounded Growth

**What goes wrong:** `collect_push_payload()` is extended to sync all telemetry_samples since last push. A busy venue with 8 pods running simultaneously can produce 10,000+ samples per lap × 8 pods × many laps = millions of rows per hour.

**Why it happens:** The existing laps sync in cloud_sync.rs limits to 500 rows with `LIMIT 500`. Telemetry has no natural limit because it's not structured around "events".

**How to avoid:** SYNC-02 says "only event-entered lap telemetry." Use:
```sql
WHERE ts.lap_id IN (SELECT lap_id FROM hotlap_event_entries WHERE lap_id IS NOT NULL)
```
This bounds telemetry sync to only laps that appeared in a competitive event. Typical event has ≤8 laps. Manageable volume.

**Warning signs:** Sync payload exceeds 1MB; cloud DB grows linearly with total lap count, not event count.

### Pitfall 6: 107% Computed With Integer Division

**What goes wrong:** `lap_time_ms * 107 / 100` in integer arithmetic truncates. A leader time of `85001ms × 107 / 100 = 90951ms` should be `90951.07ms` threshold. The integer truncation might admit one extra lap that's technically outside 107%.

**Why it happens:** Rust integer arithmetic truncates, not rounds.

**How to avoid:** Use float comparison: `(lap.lap_time_ms as f64) <= (leader_ms as f64 * 1.07)`. Or multiply both sides: `lap.lap_time_ms * 100 <= leader_ms * 107` (avoids floats, correct for integers). The second form is exact.

**Warning signs:** Edge cases where a lap is 107.00% of leader time is included/excluded inconsistently.

### Pitfall 7: Badge Reference Time Is NULL for Events Without Reference

**What goes wrong:** `hotlap_events.reference_time_ms` is nullable (created as `INTEGER` with no NOT NULL). If staff creates an event without setting a reference time, badge computation panics or produces unexpected results.

**Why it happens:** Reference time is optional by design (EVT-06 says "optional reference time").

**How to avoid:** Badge logic must be guarded: `if let Some(ref_ms) = reference_time_ms { ... } else { badge = "none" }`. Return `null` (not `"none"`) in the JSON when no reference time is set so the PWA can show "—" instead of "none".

---

## Code Examples

### Auto-Entry Date Range Query

```rust
// Source: db/mod.rs lines 1816-1840 for table structure
// In persist_lap() after lap INSERT:
let matching_events = sqlx::query_as::<_, (String, Option<i64>)>(
    "SELECT id, reference_time_ms
     FROM hotlap_events
     WHERE track = ?
       AND car_class = ?
       AND sim_type = ?
       AND status = 'active'
       AND (starts_at IS NULL OR datetime(starts_at) <= datetime('now'))
       AND (ends_at IS NULL OR datetime(ends_at) >= datetime('now'))",
)
.bind(&lap.track)
.bind(class)   // from car_class lookup
.bind(&sim_str) // format!("{:?}", lap.sim_type).to_lowercase()
.fetch_all(&state.db)
.await
.unwrap_or_default();
```

### F1 Points Scoring on Group Session Complete

```rust
// Source: group_sessions and multiplayer_results tables confirmed in routes.rs line 10910
const F1_2010_POINTS: [i64; 10] = [25, 18, 15, 12, 10, 8, 6, 4, 2, 1];

async fn score_group_event(pool: &SqlitePool, group_session_id: &str, hotlap_event_id: &str) {
    let results = sqlx::query_as::<_, (String, i64, Option<i64>, i64)>(
        "SELECT driver_id, position, best_lap_ms, dnf
         FROM multiplayer_results
         WHERE group_session_id = ?
         ORDER BY position ASC",
    )
    .bind(group_session_id)
    .fetch_all(pool).await.unwrap_or_default();

    let leader_ms = results.iter()
        .filter_map(|(_, _, best, _)| *best)
        .min()
        .unwrap_or(0);

    for (driver_id, position, best_lap_ms, dnf) in &results {
        let pts = if *dnf == 1 { 0 } else {
            F1_2010_POINTS.get((position - 1) as usize).copied().unwrap_or(0)
        };
        let result_status = if *dnf == 1 { "dnf" } else { "finished" };
        let gap_ms = best_lap_ms.map(|ms| ms - leader_ms as i64);
        let within_107 = best_lap_ms.map(|ms| ms * 100 <= leader_ms * 107).unwrap_or(false);

        let entry_id = uuid::Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "INSERT INTO hotlap_event_entries
                (id, event_id, driver_id, lap_time_ms, points, gap_to_leader_ms,
                 within_107_percent, result_status, entered_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))
             ON CONFLICT(event_id, driver_id) DO UPDATE SET
                lap_time_ms = excluded.lap_time_ms,
                points = excluded.points,
                gap_to_leader_ms = excluded.gap_to_leader_ms,
                within_107_percent = excluded.within_107_percent,
                result_status = excluded.result_status"
        )
        .bind(&entry_id).bind(hotlap_event_id).bind(driver_id)
        .bind(*best_lap_ms).bind(pts).bind(gap_ms).bind(within_107 as i32)
        .bind(result_status)
        .execute(pool).await;
    }
    // Recalculate positions after all entries are inserted
    // recalculate_event_positions(pool, hotlap_event_id).await;
}
```

### Championship Standings Recalculation (at read time)

```rust
// For GET /public/championships/{id}/standings
// Compute standings live from hotlap_event_entries + championship_rounds
async fn compute_championship_standings(pool: &SqlitePool, championship_id: &str) -> Vec<Value> {
    // Get all rounds for this championship
    let rounds = sqlx::query_as::<_, (String, i64)>(
        "SELECT event_id, round_number FROM championship_rounds
         WHERE championship_id = ? ORDER BY round_number ASC",
    )
    .bind(championship_id)
    .fetch_all(pool).await.unwrap_or_default();

    if rounds.is_empty() { return vec![]; }

    // For each round, get all entries with points + result
    // Aggregate: SUM(points) per driver, COUNT wins/p2/p3
    let standings = sqlx::query_as::<_, (String, i64, i64, i64, i64)>(
        "SELECT hee.driver_id,
                SUM(hee.points) as total_points,
                SUM(CASE WHEN hee.position = 1 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as wins,
                SUM(CASE WHEN hee.position = 2 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p2,
                SUM(CASE WHEN hee.position = 3 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p3
         FROM hotlap_event_entries hee
         INNER JOIN championship_rounds cr ON cr.event_id = hee.event_id
         WHERE cr.championship_id = ?
           AND hee.result_status IN ('finished', 'dnf', 'dns')
         GROUP BY hee.driver_id
         ORDER BY total_points DESC, wins DESC, p2 DESC, p3 DESC",
    )
    .bind(championship_id)
    .fetch_all(pool).await.unwrap_or_default();

    // Join with drivers for name, add position number
    // ... shape and return
    vec![]  // placeholder
}
```

### Cloud Sync Extension — Competitive Tables

```rust
// Source: cloud_sync.rs lines 246-461 — add these blocks in collect_push_payload()
// After billing_events block, before Ok((payload, has_data)):

// SYNC-01: Push competitive tables (venue-authoritative, one-way)
let hotlap_events = sqlx::query_as::<_, (String,)>(
    "SELECT json_object('id', id, 'name', name, 'description', description,
        'track', track, 'car', car, 'car_class', car_class, 'sim_type', sim_type,
        'status', status, 'starts_at', starts_at, 'ends_at', ends_at,
        'reference_time_ms', reference_time_ms, 'updated_at', updated_at)
     FROM hotlap_events WHERE updated_at > ?",
)
.bind(&last_push)
.fetch_all(&state.db).await?;

// SYNC-01: Event entries (all entries, small table)
let event_entries = sqlx::query_as::<_, (String,)>(
    "SELECT json_object('id', id, 'event_id', event_id, 'driver_id', driver_id,
        'lap_id', lap_id, 'lap_time_ms', lap_time_ms, 'position', position,
        'points', points, 'badge', badge, 'gap_to_leader_ms', gap_to_leader_ms,
        'within_107_percent', within_107_percent, 'result_status', result_status,
        'entered_at', entered_at)
     FROM hotlap_event_entries WHERE entered_at > ?",
)
.bind(&last_push)
.fetch_all(&state.db).await?;

// SYNC-01: Championships (all, small table)
// SYNC-01: Championship rounds + standings
// SYNC-02: Targeted telemetry for event laps only (separate push cursor)
```

**SYNC-03 assurance:** The `/sync/push` handler in routes.rs must NOT accept writes to competitive tables from cloud. The existing handler only processes: `laps`, `drivers`, `billing_sessions`, `billing_events`, `wallet_transactions`, `pricing_tiers`, `kiosk_experiences`, `kiosk_settings`. Competitive tables are absent — maintain this absence.

### PWA Event Leaderboard Page Pattern

```tsx
// pwa/src/app/events/[id]/page.tsx — follows Phase 13 "use client" pattern exactly
"use client";
import { useEffect, useState } from "react";
import { publicApi } from "@/lib/api";

export default function EventLeaderboardPage({ params }: { params: { id: string } }) {
  const [event, setEvent] = useState<any>(null);
  const [selectedClass, setSelectedClass] = useState<string | null>(null);

  useEffect(() => {
    publicApi.eventLeaderboard(params.id).then(data => {
      setEvent(data);
      if (data?.car_classes?.length > 0) setSelectedClass(data.car_classes[0]);
    });
  }, [params.id]);

  // Render per-class tabs, position table, gold/silver/bronze badges
  // 107% flagged entries shown with strikethrough/grey styling
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| No event framework | Phase 12 created all 6 competitive tables | Zero schema work needed in Phase 14 |
| No car_class on laps | car_class populated in persist_lap() since Phase 12 | Auto-entry matching is ready |
| No lap validity hardening | suspect column + sector sum check since Phase 13 | Auto-entry only enters valid, non-suspect laps |
| Cloud sync covers operations tables only | Phase 14 extends sync to competitive tables | SYNC-01/02/03 requirements met by extending existing pattern |
| No group session→event link | Phase 14 adds hotlap_event_id to group_sessions | GRP-01 scoring has a target to write to |
| championship_standings lacks p2/p3 | Phase 14 computes at read time OR adds columns | CHP-04 tiebreaker correct |

---

## Open Questions

1. **How is group session completion triggered?**
   - What we know: `group_sessions.status` transitions through `'forming' → 'active' → 'completed'`. The `multiplayer_results` table is populated from AC race results somehow.
   - What's unclear: What triggers the status change to 'completed'? Is it staff-initiated via a terminal endpoint? Is there a webhook from the AC server?
   - Recommendation: Add a staff endpoint `POST /staff/group-sessions/{id}/complete` that: (a) sets status='completed', (b) scores F1 points into hotlap_event_entries, (c) triggers championship standings recalculation. This is the safest explicit trigger.

2. **Does `multiplayer_results` table exist in the production schema?**
   - What we know: routes.rs line 10910 queries `FROM multiplayer_results mr` and it's referenced in `customer_multiplayer_results`. The table must exist somewhere.
   - What's unclear: It was not found in `db/mod.rs` migrations. It may be created elsewhere or may be missing from integration tests.
   - Recommendation: Search for `multiplayer_results` in the full codebase. If absent from migrations, add it in the Phase 14 Wave 0 schema step.

3. **Should events listing support filtering by date / status?**
   - What we know: EVT-07 says "browse all active and past hotlap events." The events table has `status` and `starts_at` / `ends_at`.
   - What's unclear: Should the PWA listing show events in reverse chronological order? Should it separate active vs. completed? Should it filter by sim_type?
   - Recommendation: Return all events in reverse chronological order; let the PWA show status badges (Active/Completed/Upcoming) as visual differentiation. Add sim_type filter as optional query param.

4. **Is there an existing `multiplayer_results` table?**
   - Direct inspection of `db/mod.rs` shows no `CREATE TABLE IF NOT EXISTS multiplayer_results`. The route at line 10910 queries it, which means either (a) it was added in a migration not visible in mod.rs, (b) it's in a different file, or (c) the route currently fails silently.
   - Recommendation: Before implementing GRP-01, run `SELECT name FROM sqlite_master WHERE type='table'` on a running venue DB to confirm. If absent, create it in Wave 0.

---

## Validation Architecture

nyquist_validation is enabled (config.json).

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[tokio::test]` (in-memory SQLite) — existing `crates/rc-core/tests/integration.rs` |
| Config file | Cargo.toml (auto-discovered) |
| Quick run command | `cargo test -p rc-core test_event` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EVT-01 | Staff create event inserts row with all columns | unit | `cargo test -p rc-core test_create_hotlap_event` | Wave 0 |
| EVT-02 | Lap matching track+car_class+sim_type+date enters event automatically | unit | `cargo test -p rc-core test_auto_event_entry` | Wave 0 |
| EVT-02 | Lap NOT matching (wrong car_class) does NOT enter event | unit | `cargo test -p rc-core test_auto_entry_no_match` | Wave 0 |
| EVT-02 | Lap outside date range does NOT enter event | unit | `cargo test -p rc-core test_auto_entry_date_range` | Wave 0 |
| EVT-02 | Second faster lap for same driver replaces existing entry | unit | `cargo test -p rc-core test_auto_entry_faster_lap` | Wave 0 |
| EVT-02 | Slower second lap does NOT replace existing entry | unit | `cargo test -p rc-core test_auto_entry_no_replace_slower` | Wave 0 |
| EVT-03 | Public event leaderboard returns position/driver/time/gap | unit | `cargo test -p rc-core test_public_event_leaderboard` | Wave 0 |
| EVT-04 | Event leaderboard groups correctly by car class | unit | `cargo test -p rc-core test_event_per_class_tabs` | Wave 0 |
| EVT-05 | 107% rule: entry at 108% of leader gets within_107_percent=0 | unit | `cargo test -p rc-core test_107_percent_rule` | Wave 0 |
| EVT-05 | Entry at exactly 107% gets within_107_percent=1 (boundary) | unit | `cargo test -p rc-core test_107_boundary` | Wave 0 |
| EVT-06 | Gold badge when lap_ms / ref_ms <= 1.02 | unit | `cargo test -p rc-core test_badge_gold` | Wave 0 |
| EVT-06 | Silver badge when 1.02 < ratio <= 1.05 | unit | `cargo test -p rc-core test_badge_silver` | Wave 0 |
| EVT-06 | Bronze badge when 1.05 < ratio <= 1.08 | unit | `cargo test -p rc-core test_badge_bronze` | Wave 0 |
| EVT-06 | No badge when reference_time_ms IS NULL | unit | `cargo test -p rc-core test_badge_no_reference` | Wave 0 |
| EVT-07 | Public events list returns active + completed events | unit | `cargo test -p rc-core test_public_events_list` | Wave 0 |
| GRP-01 | F1 points: P1=25, P2=18, P3=15, DNF=0 | unit | `cargo test -p rc-core test_f1_points_scoring` | Wave 0 |
| GRP-01 | DNS/DNF entries get 0 points regardless of position | unit | `cargo test -p rc-core test_dns_dnf_zero_points` | Wave 0 |
| GRP-04 | gap_to_leader_ms = entry_ms - min(entry_ms) for all entries | unit | `cargo test -p rc-core test_gap_to_leader` | Wave 0 |
| CHP-02 | Championship standings = SUM of F1 points across rounds | unit | `cargo test -p rc-core test_championship_standings_sum` | Wave 0 |
| CHP-04 | Tiebreaker: driver with more wins ranks higher at equal points | unit | `cargo test -p rc-core test_championship_tiebreaker_wins` | Wave 0 |
| CHP-04 | Tiebreaker: equal wins → more P2s ranks higher | unit | `cargo test -p rc-core test_championship_tiebreaker_p2` | Wave 0 |
| CHP-05 | result_status CHECK: 'dns' inserts without error | unit | `cargo test -p rc-core test_result_status_dns` | ✅ (existing test_competitive_tables covers insert) |
| SYNC-01 | collect_push_payload includes hotlap_events when updated_at > last_push | unit | `cargo test -p rc-core test_sync_competitive_tables` | Wave 0 |
| SYNC-02 | Telemetry sync only includes laps referenced in hotlap_event_entries | unit | `cargo test -p rc-core test_sync_targeted_telemetry` | Wave 0 |
| SYNC-03 | /sync/push handler does NOT write to hotlap_events (venue only) | integration | manual curl test + code review | N/A |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-core 2>&1 | tail -5`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_event_entry` (EVT-02 happy path)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_no_match` (wrong car_class)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_date_range` (expired event)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_faster_lap` (replace on faster)
- [ ] `crates/rc-core/tests/integration.rs` — add `test_auto_entry_no_replace_slower`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_107_percent_rule` + `test_107_boundary`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_badge_gold/silver/bronze/no_reference`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_f1_points_scoring` + `test_dns_dnf_zero_points`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_championship_standings_sum`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_championship_tiebreaker_wins/p2`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_sync_competitive_tables`
- [ ] `crates/rc-core/tests/integration.rs` — add `test_sync_targeted_telemetry`
- [ ] `crates/rc-core/tests/integration.rs` — verify `run_test_migrations()` includes Phase 14 `ALTER TABLE` additions for `group_sessions.hotlap_event_id` and `championship_standings.p2_count / p3_count`
- [ ] Confirm whether `multiplayer_results` table exists in production schema — check with `SELECT name FROM sqlite_master WHERE type='table'` on venue DB; add migration if absent

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)

- `crates/rc-core/src/db/mod.rs` lines 1816-1957 — confirmed all 6 competitive tables with exact column names, types, CHECK constraints, indexes, and FKs
- `crates/rc-core/src/db/mod.rs` lines 1959-1968 — confirmed `car_class` on laps + `suspect` column both present
- `crates/rc-core/src/lap_tracker.rs` lines 1-160 — full `persist_lap()` flow confirmed; car_class lookup, suspect flag, personal best, track record — insertion point for auto-entry is clear
- `crates/rc-core/src/api/routes.rs` lines 247-255 — confirmed public route registration pattern for Phase 14 additions
- `crates/rc-core/src/api/routes.rs` lines 6911-6937 — `check_terminal_auth()` confirmed for staff endpoints
- `crates/rc-core/src/api/routes.rs` lines 10899-10942 — `customer_multiplayer_results` confirmed; reads from `multiplayer_results` table
- `crates/rc-core/src/cloud_sync.rs` lines 246-461 — `collect_push_payload()` full structure confirmed; extension points identified for SYNC-01/02
- `crates/rc-core/src/cloud_sync.rs` line 18 — `SYNC_TABLES` constant confirmed as not including competitive tables
- `pwa/src/lib/api.ts` lines 926-969 — `publicApi` object confirmed; 5 new methods needed for Phase 14 events/championships
- `crates/rc-core/tests/integration.rs` lines 1280-1350 — competitive table insert tests confirmed; `run_test_migrations()` already includes all 6 tables
- `.planning/STATE.md` — confirmed pre-phase decision: "Championship scoring edge cases (DNS/DNF, tiebreaker, round cancellation) need characterization tests before implementing scoring logic"
- `.planning/phases/12-data-foundation/12-01-SUMMARY.md` — confirmed tables + indexes in production schema
- `.planning/phases/13-leaderboard-core/13-05-SUMMARY.md` — confirmed PWA page pattern, publicApi pattern, formatting conventions for Phase 14 to follow

### Secondary (MEDIUM confidence — verified against codebase)

- `.planning/phases/12-data-foundation/12-RESEARCH.md` — idempotent ALTER TABLE pattern, migrate() extension approach
- `.planning/phases/13-leaderboard-core/13-RESEARCH.md` — public endpoint pattern, sim_type filter, Privacy rules, check_terminal_auth usage

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies; all patterns directly confirmed in source code
- Architecture: HIGH — direct reading of all files being modified; lap_tracker.rs insertion point identified precisely
- Competitive tables: HIGH — schema confirmed column-by-column in db/mod.rs lines 1816-1957
- Pitfalls: HIGH — most pitfalls derived from direct schema inspection (nullability, missing FKs, integer arithmetic)
- Cloud sync: HIGH — collect_push_payload() structure confirmed line by line; extension pattern is identical to existing sections
- Group session scoring: MEDIUM — multiplayer_results table existence uncertain (referenced in routes.rs but not found in db/mod.rs migrations); needs verification before GRP-01 implementation

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (stable stack; schema confirmed; no external dependencies)

**Key finding not in prior research:** `championship_standings` lacks `p2_count` and `p3_count` columns for F1 tiebreaker — but these can be computed at read time by joining `hotlap_event_entries` on `championship_rounds`, avoiding a schema change. The `group_sessions` table has no FK to `hotlap_events` — this is a concrete schema gap requiring either `ALTER TABLE group_sessions ADD COLUMN hotlap_event_id TEXT` or a staff-managed linking table. The `multiplayer_results` table is referenced in routes.rs but absent from visible db/mod.rs migrations — must verify existence before implementing GRP-01.
