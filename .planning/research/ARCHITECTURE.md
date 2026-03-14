# Architecture Research

**Domain:** Sim racing competitive platform — leaderboards, championships, telemetry viz, driver rating
**Researched:** 2026-03-14
**Confidence:** HIGH — based on direct codebase inspection of all 3 crates, 1800-line db migration, routes.rs (276 endpoints), cloud_sync.rs, and all PWA pages

---

## Existing System Inventory

### What Already Exists (do not rebuild)

| Component | Location | What it Does |
|-----------|----------|--------------|
| `events` table | db/mod.rs line 131 | Basic event scaffold (id, name, type, status, sim_type, track, car_class, max_entries, config_json) |
| `event_entries` table | db/mod.rs line 148 | Driver-event join (event_id, driver_id, result_position, result_time_ms) |
| `tournaments` table | db/mod.rs line 1357 | Full tournament model (format: time_attack/bracket/round_robin, entry_fee, prize_pool, status lifecycle) |
| `tournament_registrations` | db/mod.rs line 1379 | Registration with seed, status, best_time_ms |
| `tournament_matches` | db/mod.rs line 1397 | Bracket match model with driver_a/b, times, winner |
| `time_trials` table | db/mod.rs line 1329 | Weekly time trial (track, car, week_start, week_end) |
| `group_sessions` table | db/mod.rs line 992 | Multiplayer session (host, shared_pin, status, track, car, ai_count) |
| `multiplayer_results` table | db/mod.rs line 1754 | Group session results (position, best_lap_ms, total_time_ms, laps_completed, dnf) |
| `/tournaments` admin routes | routes.rs line 218 | Full CRUD + bracket generation + match result recording |
| `/customer/tournaments` | routes.rs line 225 | List + register PWA endpoints |
| `/public/time-trial` | routes.rs line 250 | Public endpoint for current time trial |
| `/leaderboard/{track}` | routes.rs line 58 | Staff-facing per-track leaderboard |
| `/public/leaderboard` | routes.rs line 247 | Public leaderboard with track records, top drivers, time trial |
| `/public/leaderboard/{track}` | routes.rs line 248 | Public per-track leaderboard with stats |
| `/public/laps/{id}/telemetry` | routes.rs line 250 | Public telemetry samples for a lap |
| `TelemetryChart` component | pwa/src/components/TelemetryChart.tsx | Speed trace, throttle/brake, steering, gear/RPM using recharts with syncId |
| Leaderboard PWA page | pwa/src/app/leaderboard/page.tsx | Track selector + entries + inline telemetry expand |
| Public leaderboard page | pwa/src/app/leaderboard/public/page.tsx | Records, top drivers, tracks tabs — no login required |
| Tournaments PWA page | pwa/src/app/tournaments/page.tsx | List + register |
| `laps` table | db/mod.rs line 82 | Full lap record (lap_time_ms, sector1/2/3_ms, valid, sim_type, track, car, driver_id) |
| `telemetry_samples` table | db/mod.rs line 177 | Per-lap samples (offset_ms, speed, throttle, brake, steering, gear, rpm, pos_x/y/z) |
| `track_records` table | db/mod.rs line 117 | Best lap per (track, car) pair |
| `personal_bests` table | db/mod.rs line 103 | Best lap per (driver, track, car) |
| Cloud sync push | cloud_sync.rs | Pushes laps, track_records, personal_bests, billing_sessions, drivers, wallets, pods |
| recharts library | pwa/package.json | Already installed — LineChart, AreaChart, ResponsiveContainer, syncId |

### What is Missing (what v3.0 must build)

The `events` and `event_entries` tables exist as scaffolds but have no logic. Routes.rs has `list_events`/`create_event` stubs with no handler bodies. The competitive pipeline — hotlap event scoring, championship accumulation, driver rating, 2D track map, lap comparison — is entirely absent. Telemetry data is NOT currently synced to the cloud (only lap metadata is pushed), which blocks cloud-side telemetry visualization.

---

## System Overview

```
VENUE (192.168.31.x)
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│  Game (AC/F1)  UDP/SharedMem   rc-agent :18923   WebSocket   rc-core   │
│  ────────────────────────────> ────────────────────────────> :8080     │
│                                                                         │
│  Telemetry flow:                                                        │
│  TelemetryFrame -> WS msg -> lap_tracker.rs -> telemetry_samples        │
│  Lap complete -> laps table -> personal_bests -> track_records           │
│  [NEW] Lap complete -> check_hotlap_event_entry() -> hotlap_event_entries│
│                                                                         │
│  racecontrol.db (SQLite WAL, 40+ tables)                                │
└──────────────────────────┬──────────────────────────────────────────────┘
                           |
                           | cloud_sync.rs (2s relay / 30s HTTP)
                           | Pushes: laps, track_records, personal_bests,
                           |         billing_sessions, drivers, wallets
                           | [NEW] Also pushes: hotlap_events,
                           |   hotlap_event_entries, championships,
                           |   championship_standings, driver_ratings,
                           |   telemetry_samples (event laps only)
                           | Pulls: drivers, wallets, pricing, kiosk_settings
                           v
CLOUD (app.racingpoint.cloud :8080)
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│  rc-core (cloud instance — same binary, cloud.mode=true)                │
│  Public endpoints: /public/* (no auth required)                         │
│  Customer endpoints: /customer/* (JWT auth)                             │
│  [NEW] /public/events, /public/championships, /public/drivers/*         │
│  [NEW] /public/compare-laps, /public/laps/{id}/track-position           │
│                                                                         │
│  racecontrol.db (cloud copy — receives venue push, serves PWA)          │
└──────────────────────────┬──────────────────────────────────────────────┘
                           |
                           | HTTPS fetch (no auth for public endpoints)
                           v
Cloud PWA (app.racingpoint.cloud)
Next.js 14, port 3000

Existing pages: /leaderboard/public, /leaderboard, /telemetry, /tournaments
[NEW] pages: /events, /events/[id], /championships, /championships/[id]
             /drivers/[id], /telemetry/compare
[ENHANCED] /leaderboard/public: add Events + Championships tabs, rating badges
```

---

## New Database Tables Required

All tables added to `db/mod.rs` as `CREATE TABLE IF NOT EXISTS`. The same migration file runs on both venue and cloud DB at startup — identical schema on both sides is required for the sync to work.

### 1. hotlap_events

```sql
CREATE TABLE IF NOT EXISTS hotlap_events (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    track TEXT NOT NULL,
    car TEXT NOT NULL,
    car_class TEXT NOT NULL,              -- A/B/C/D
    sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
    status TEXT NOT NULL DEFAULT 'upcoming'
        CHECK(status IN ('upcoming', 'active', 'scoring', 'completed')),
    starts_at TEXT,
    ends_at TEXT,
    rule_107_percent INTEGER DEFAULT 1,   -- 1 = enforce 107% rule
    max_valid_laps INTEGER,               -- cap entries per driver
    championship_id TEXT REFERENCES championships(id),
    championship_round INTEGER,
    created_by TEXT,                      -- staff driver_id
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
)
```

### 2. hotlap_event_entries

```sql
CREATE TABLE IF NOT EXISTS hotlap_event_entries (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL REFERENCES hotlap_events(id),
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    lap_id TEXT REFERENCES laps(id),      -- best qualifying lap
    lap_time_ms INTEGER,
    sector1_ms INTEGER,
    sector2_ms INTEGER,
    sector3_ms INTEGER,
    position INTEGER,                     -- 1-based, null until scored
    points INTEGER DEFAULT 0,            -- F1 scoring after finalize
    badge TEXT,                          -- 'gold'/'silver'/'bronze'/null
    gap_to_leader_ms INTEGER,
    within_107_percent INTEGER DEFAULT 1,
    entered_at TEXT DEFAULT (datetime('now')),
    UNIQUE(event_id, driver_id)          -- one entry per driver per event
)
```

### 3. championships

```sql
CREATE TABLE IF NOT EXISTS championships (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    season TEXT,                          -- e.g. '2026-Q1'
    car_class TEXT NOT NULL,
    sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
    status TEXT NOT NULL DEFAULT 'upcoming'
        CHECK(status IN ('upcoming', 'active', 'completed')),
    scoring_system TEXT NOT NULL DEFAULT 'f1_2010',
    total_rounds INTEGER DEFAULT 0,
    completed_rounds INTEGER DEFAULT 0,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
)
```

### 4. championship_standings (materialized — recalculated on each round score)

```sql
CREATE TABLE IF NOT EXISTS championship_standings (
    championship_id TEXT NOT NULL REFERENCES championships(id),
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    position INTEGER,
    total_points INTEGER DEFAULT 0,
    rounds_entered INTEGER DEFAULT 0,
    best_result INTEGER,               -- best finish position across rounds
    wins INTEGER DEFAULT 0,
    podiums INTEGER DEFAULT 0,
    updated_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (championship_id, driver_id)
)
```

### 5. driver_ratings

```sql
CREATE TABLE IF NOT EXISTS driver_ratings (
    driver_id TEXT PRIMARY KEY REFERENCES drivers(id),
    rating_class TEXT NOT NULL DEFAULT 'Rookie',
    -- 'Rookie' / 'Bronze' / 'Silver' / 'Gold' / 'Platinum'
    class_points INTEGER NOT NULL DEFAULT 0,
    total_events INTEGER DEFAULT 0,
    total_podiums INTEGER DEFAULT 0,
    total_wins INTEGER DEFAULT 0,
    class_a_best_ms INTEGER,           -- best lap time in class A car
    class_b_best_ms INTEGER,
    class_c_best_ms INTEGER,
    class_d_best_ms INTEGER,
    updated_at TEXT DEFAULT (datetime('now'))
)
```

### 6. Column addition to laps table

```sql
-- Add car_class to laps (populated from billing_session experience at lap recording time)
ALTER TABLE laps ADD COLUMN car_class TEXT;
-- Indexes
CREATE INDEX IF NOT EXISTS idx_laps_car_class ON laps(track, car_class);
```

### 7. Indexes for new tables

```sql
CREATE INDEX IF NOT EXISTS idx_hotlap_events_updated ON hotlap_events(updated_at);
CREATE INDEX IF NOT EXISTS idx_hotlap_events_status ON hotlap_events(status, track);
CREATE INDEX IF NOT EXISTS idx_hotlap_entries_event ON hotlap_event_entries(event_id, position);
CREATE INDEX IF NOT EXISTS idx_hotlap_entries_driver ON hotlap_event_entries(driver_id);
CREATE INDEX IF NOT EXISTS idx_championships_updated ON championships(updated_at);
CREATE INDEX IF NOT EXISTS idx_champ_standings_champ ON championship_standings(championship_id, position);
CREATE INDEX IF NOT EXISTS idx_driver_ratings_class ON driver_ratings(rating_class, class_points);
```

---

## New API Endpoints Required

### Hotlap Events — Staff

```
POST   /events/hotlap                    Create hotlap event
PUT    /events/hotlap/{id}               Update (change status, dates)
POST   /events/hotlap/{id}/score         Trigger final scoring and badge assignment
DELETE /events/hotlap/{id}               Cancel event (sets status='cancelled' — add to CHECK)
```

### Hotlap Events — Public (no auth)

```
GET    /public/events                    List active + upcoming events
GET    /public/events/{id}               Event detail + live standings leaderboard
```

### Championships — Staff

```
POST   /championships                    Create championship
PUT    /championships/{id}               Update metadata
POST   /championships/{id}/recalculate   Force standing recalculation
```

### Championships — Public (no auth)

```
GET    /public/championships             List active championships
GET    /public/championships/{id}        Championship detail + full standings
GET    /public/championships/{id}/rounds Per-round event results for this championship
```

### Driver Profiles — Public (no auth)

```
GET    /public/drivers                   Paginated driver list with rating class
GET    /public/drivers/{id}              Full driver profile: stats, best times per class, rating, event history
GET    /public/drivers/{id}/laps         Lap history with track/car/time
GET    /public/drivers/{id}/events       Event participation history with positions
```

### Enhanced Leaderboard — extend existing public endpoints

```
GET    /public/leaderboard               Extend response: add events[], championships[] arrays
GET    /public/leaderboard/{track}       Extend: add ?class= query param for car class filter
GET    /public/circuit-records           Best lap per (track, car_class) combination
GET    /public/vehicle-records/{car}     Best times for one car across all tracks
```

### Telemetry — Public (no auth)

```
GET    /public/laps/{id}/telemetry       Existing — no change
GET    /public/compare-laps              ?lap_a=&lap_b= merged dual-trace array
GET    /public/laps/{id}/track-position  pos_x, pos_z, offset_ms only (2D map data)
```

---

## Cloud Sync Additions

### Extend `collect_push_payload()` in cloud_sync.rs

Add queries for 5 new tables using the same `json_object()` pattern as existing laps/track_records queries:

```rust
// hotlap_events: staff creates at venue, cloud displays them
// WHERE updated_at > last_push LIMIT 100

// hotlap_event_entries: auto-entered on lap completion, updated on scoring
// WHERE entered_at > last_push LIMIT 500

// championships: staff config at venue
// WHERE updated_at > last_push LIMIT 50

// championship_standings: recalculated at venue after scoring
// Push all standings for championships that changed (no delta — small table)

// driver_ratings: updated at venue after lap events
// WHERE updated_at > last_push LIMIT 500
```

### Targeted telemetry sync (critical — telemetry NOT currently synced)

The existing sync pushes `laps` metadata but NOT `telemetry_samples`. Syncing all telemetry is impractical (~2000 rows per lap * many laps). The solution: sync only telemetry for event-entered laps.

```rust
// In collect_push_payload(), after hotlap_event_entries push:
// Query telemetry_samples WHERE lap_id IN
//   (SELECT lap_id FROM hotlap_event_entries WHERE entered_at > last_push)
// This ensures only event-relevant laps get their telemetry synced
// Cap: LIMIT 10000 rows total (5 laps * 2000 samples)
```

### Cloud receives via existing /sync/push handler

The cloud's `/sync/push` handler already iterates all JSON keys and upserts rows into matching tables. No new handler code required — as long as the tables exist in the cloud DB (same migration runs on both instances at startup). Simply adding the new table names to the push payload is sufficient.

---

## PWA Page Architecture

### New pages

```
pwa/src/app/
├── events/
│   ├── page.tsx                Event list (public, no login required)
│   └── [id]/
│       └── page.tsx            Event detail: live standings + inline telemetry
├── championships/
│   ├── page.tsx                Championship list (public)
│   └── [id]/
│       └── page.tsx            Standings table + round-by-round breakdown
├── drivers/
│   └── [id]/
│       └── page.tsx            Driver public profile
└── telemetry/
    └── compare/
        └── page.tsx            Dual-trace lap comparison viewer
```

### Enhanced existing pages

```
pwa/src/app/leaderboard/public/page.tsx
  Add tab: "Events" — list active events with countdown, link to /events/[id]
  Add tab: "Championships" — current standings summary
  Existing "Records": add car_class filter dropdown (A/B/C/D)
  Existing "Drivers": add DriverRatingBadge alongside driver name

pwa/src/app/leaderboard/page.tsx (authenticated, venue)
  Add car class filter pill row below track selector
  Add "Enter Event" CTA card when active event matches current track/car

pwa/src/app/profile/page.tsx (authenticated)
  Add driver rating card: class badge + class_points progress bar toward next class
  Add "My Events" section: recent event participations with position + points
```

### New reusable components

```
pwa/src/components/
├── EventCard.tsx            Status badge, countdown timer, car class, track name
├── ChampionshipStandings.tsx  Sortable table — points + per-round result columns
├── DriverRatingBadge.tsx    Colored pill: Rookie(grey)/Bronze/Silver/Gold/Platinum
├── LapComparisonChart.tsx   Dual-trace recharts (extends TelemetryChart syncId pattern)
├── TrackMapOverlay.tsx      SVG 2D map from pos_x/pos_z telemetry coordinate data
└── SectorBadge.tsx          F1-style delta chip: purple (personal best) / green (improvement) / yellow (slower)
```

---

## Component Boundaries

| Component | Owns | Communicates With |
|-----------|------|-------------------|
| `lap_tracker.rs` (venue rc-core) | Lap validation, personal_bests update, track_records update | Calls `check_hotlap_event_entry()` — NEW |
| `hotlap_events.rs` (new module) | Event CRUD, auto-entry logic, 107% check, F1 scoring | lap_tracker.rs, cloud_sync.rs |
| `championships.rs` (new module) | Championship CRUD, standing recalculation | hotlap_events.rs (called after scoring) |
| `driver_rating.rs` (new module) | Rating class updates (lap count + event results) | lap_tracker.rs, hotlap_events.rs |
| `cloud_sync.rs` | Push new tables venue to cloud | All new modules add their tables to push payload |
| Cloud `/public/events/*` handlers | Read from cloud DB, serve PWA | Read-only — no writes to venue |
| PWA `/events/[id]` page | Fetch event standings, show inline telemetry | TelemetryChart (reused as-is) |
| PWA `/telemetry/compare` | Fetch merged dual-trace data | LapComparisonChart (new component) |
| PWA `/drivers/[id]` | Fetch driver profile, best times, event history | DriverRatingBadge, EventCard |

---

## Data Flow for Key Operations

### 1. Lap Auto-Entry into Hotlap Event

```
Driver completes lap in AC
  -> UDP packet -> rc-agent -> TelemetryFrame (WS) -> rc-core
  -> lap_tracker.rs: insert into laps table
  -> Update personal_bests (if PB)
  -> Update track_records (if track record)
  -> [NEW] hotlap_events.check_hotlap_event_entry(pool, &lap, driver_id)
       -> SELECT id FROM hotlap_events
          WHERE track=? AND car_class=? AND status='active'
          AND starts_at <= now() AND ends_at >= now()
       -> If event found AND lap.valid = true:
           -> SELECT lap_time_ms FROM hotlap_event_entries WHERE event_id=? AND driver_id=?
           -> If no existing entry OR new time is faster:
               -> Apply 107% rule: if leader_time exists AND new_time > leader_time * 1.07 -> reject
               -> UPSERT hotlap_event_entries (event_id, driver_id, lap_id, time, sectors)
  -> cloud_sync.rs: next 2s relay cycle picks up new hotlap_event_entries row
  -> Cloud DB receives entry via /sync/push
  -> PWA: polls /public/events/{id} -> sees updated standings in real time
```

### 2. Event Scoring and Championship Points

```
Staff: POST /events/hotlap/{id}/score

hotlap_events.rs: finalize_event_scoring(pool, event_id)
  -> BEGIN TRANSACTION
  -> SELECT * FROM hotlap_event_entries WHERE event_id=? AND within_107_percent=1
     ORDER BY lap_time_ms ASC
  -> Assign position (1-based rank by time)
  -> Assign F1 points: [25, 18, 15, 12, 10, 8, 6, 4, 2, 1, 0, ...]
  -> Assign badges: pos=1->gold, pos=2->silver, pos=3->bronze
  -> Calculate gap_to_leader_ms for each entry
  -> UPDATE hotlap_event_entries SET position, points, badge, gap_to_leader_ms
  -> UPDATE hotlap_events SET status='completed'
  -> COMMIT

If event.championship_id is set:
  championships.rs: recalculate_standings(pool, championship_id)
  -> SELECT driver_id, SUM(points) as total, COUNT(*) as rounds,
            MIN(position) as best_result, SUM(position=1) as wins
     FROM hotlap_event_entries
     JOIN hotlap_events ON event_id = hotlap_events.id
     WHERE championship_id=? AND hotlap_events.status='completed'
     GROUP BY driver_id
     ORDER BY total DESC
  -> Upsert championship_standings with new position rankings

driver_rating.rs: update_ratings_after_event(pool, event_id)
  -> For each entry: award class_points based on position
  -> Check class upgrade thresholds (configurable in settings)
  -> Upsert driver_ratings

cloud_sync.rs: all changed tables sync on next cycle
```

### 3. Telemetry Lap Comparison

```
User: PWA /telemetry/compare?lap_a={id}&lap_b={id}

GET /public/compare-laps?lap_a=&lap_b=
  rc-core (cloud):
  -> Fetch telemetry_samples for lap_a: SELECT offset_ms, speed, throttle, brake, steering FROM ...
  -> Fetch telemetry_samples for lap_b: same query
  -> Time-normalize both to common offset_ms domain
     (both laps start at 0ms, interpolate where sample rates differ)
  -> Merge: [{offset_ms, speed_a, speed_b, throttle_a, throttle_b, brake_a, brake_b, ...}]
  -> Return: { merged_samples, lap_a_meta: {time, car, track, driver}, lap_b_meta: {...} }

LapComparisonChart component:
  -> recharts with syncId="compare" (keeps all sub-charts aligned on hover)
  -> Speed chart: two lines (speed_a in blue, speed_b in orange)
  -> Delta line: speed_a - speed_b (green=A faster, red=B faster)
  -> Throttle/Brake: two traces each
  -> Vertical reference lines at sector boundaries
```

### 4. 2D Track Map Rendering

```
GET /public/laps/{id}/track-position
  rc-core: SELECT pos_x, pos_z, offset_ms, speed
           FROM telemetry_samples WHERE lap_id=? ORDER BY offset_ms ASC

TrackMapOverlay component:
  -> Normalize pos_x / pos_z to SVG viewport (find min/max, scale to 300x200 px)
  -> Note: AC uses right-hand Y-up coordinate: pos_x = longitudinal, pos_z = lateral
  -> Draw polyline of (pos_x, pos_z) points scaled to SVG — track outline appears
  -> Color each segment by speed: blue (slow) -> green -> yellow -> red (fast)
  -> Optional: animate a dot along the path if showing playback
  -> Works automatically for any track where telemetry was recorded — no asset files needed
```

---

## Suggested Build Order

Dependencies must be respected — schema before logic, logic before API, API before frontend.

### Step 1: Database Schema (venue + cloud, prerequisite for everything)

1. Add `hotlap_events` table to db/mod.rs
2. Add `hotlap_event_entries` table to db/mod.rs
3. Add `championships` table to db/mod.rs
4. Add `championship_standings` table to db/mod.rs
5. Add `driver_ratings` table to db/mod.rs
6. Add `car_class` column to `laps` table via ALTER TABLE
7. Add all required indexes

Test: `cargo test -p rc-core` — migration runs clean on fresh DB and on upgrade from existing racecontrol.db.

### Step 2: Core Business Logic Modules (Rust)

8. Create `crates/rc-core/src/hotlap_events.rs`
   - Event CRUD functions
   - `check_hotlap_event_entry()` — auto-entry on lap completion
   - `finalize_event_scoring()` — F1 scoring, badges, gap calculation
9. Create `crates/rc-core/src/championships.rs`
   - Championship CRUD
   - `recalculate_standings()` — aggregate points across events
10. Create `crates/rc-core/src/driver_rating.rs`
    - `update_rating_after_lap()` — called from lap_tracker.rs
    - `update_rating_after_event()` — called from hotlap_events.rs after scoring
11. Modify `crates/rc-core/src/lap_tracker.rs`
    - After valid lap insertion: call `hotlap_events::check_hotlap_event_entry()`
    - Populate `car_class` column from active billing_session's experience

Test: Unit tests for 107% rule math, F1 scoring table, rating threshold logic.

### Step 3: API Endpoints (Rust)

12. Register all new route handlers in `api/routes.rs`
13. Implement staff hotlap event handlers (POST/PUT/POST score)
14. Implement public event handlers (GET /public/events, GET /public/events/{id})
15. Implement championship handlers (staff + public)
16. Implement public driver profile handlers
17. Implement GET /public/compare-laps (merge two telemetry arrays server-side)
18. Implement GET /public/laps/{id}/track-position (pos_x, pos_z only)
19. Extend GET /public/leaderboard response with events[] + championships[] arrays

Test: cargo test + curl against local rc-core with seeded test data.

### Step 4: Cloud Sync (Rust)

20. Extend `collect_push_payload()` with queries for 5 new tables
21. Add targeted telemetry sync for event-entered laps (event lap_ids only)

Test: Run venue rc-core, verify cloud DB receives new table data on next sync cycle.

### Step 5: PWA New Pages (Next.js)

22. Add typed API functions for all new endpoints to `pwa/src/lib/api.ts`
23. `/events` page — event list with EventCard components
24. `/events/[id]` page — live standings leaderboard with inline TelemetryChart
25. `/championships` page — championship list
26. `/championships/[id]` page — full standings with round breakdown
27. `/drivers/[id]` page — driver profile: stats, best times per class, event history
28. `/telemetry/compare` page — dual-trace comparison

### Step 6: PWA Enhancements + New Components (Next.js)

29. `LapComparisonChart` component — dual recharts traces with syncId
30. `TrackMapOverlay` component — SVG from pos_x/pos_z data
31. `EventCard`, `ChampionshipStandings`, `DriverRatingBadge`, `SectorBadge` components
32. Enhance `/leaderboard/public` — add Events + Championships tabs
33. Enhance `/leaderboard/public` — car class filter on Records tab, rating badge on Drivers tab
34. Enhance `/profile` — driver rating card, My Events section

---

## Integration Points with Existing Code

### Existing files to MODIFY

| File | Required Change |
|------|-----------------|
| `crates/rc-core/src/db/mod.rs` | Add 5 new tables + `car_class` ALTER + indexes at end of `migrate()` |
| `crates/rc-core/src/lap_tracker.rs` | After valid lap insert: call `check_hotlap_event_entry()`, populate `car_class` |
| `crates/rc-core/src/cloud_sync.rs` | Extend `collect_push_payload()` with 5 new table queries + event telemetry |
| `crates/rc-core/src/api/routes.rs` | Register all new route handlers, add them to `api_routes()` |
| `crates/rc-core/src/lib.rs` | Add `pub mod hotlap_events; pub mod championships; pub mod driver_rating;` |
| `pwa/src/app/leaderboard/public/page.tsx` | Add Events/Championships tabs, car class filter, rating badges |
| `pwa/src/app/profile/page.tsx` | Add rating card and My Events section |
| `pwa/src/lib/api.ts` | Add typed fetch functions for all new public endpoints |

### Existing code to REUSE unchanged

| Component | How reused |
|-----------|-----------|
| `TelemetryChart.tsx` | Used as-is in `/events/[id]` for inline lap telemetry expand |
| `recharts` library | Already installed — extend for LapComparisonChart |
| `publicApi.lapTelemetry()` | Reused by comparison page (call twice, backend merges) |
| JWT Bearer auth pattern | All authenticated pages follow same pattern — no new auth infra |
| `BottomNav` component | Reused on all new PWA pages |
| WAL-mode SQLite pool | No change — existing 5-connection pool handles new queries |
| `/sync/push` endpoint handler | No change — generic upsert handles any new table in payload |

---

## Architectural Patterns

### Pattern 1: Idempotent Migration Additions

**What:** New tables use `CREATE TABLE IF NOT EXISTS`. New columns use `let _ = ALTER TABLE ... ADD COLUMN`. The migration runs at every rc-core startup on both venue and cloud.

**When to use:** All schema additions in v3.0. The codebase has no migration versioning system — this is the established pattern for all 40+ existing tables.

**Do not use:** For columns that need non-null constraints on existing rows. In that case, use the backfill pattern (see customer_id backfill in db/mod.rs lines 893-919).

### Pattern 2: Venue-Authoritative One-Way Push

**What:** All event scoring, championship standings, and driver rating updates happen exclusively at venue rc-core. Cloud is a read-only replica for competitive data. Cloud never writes back to venue on these tables.

**Why this matters:** The SYNC_TABLES constant in cloud_sync.rs handles bidirectional sync for config tables (pricing, experiences, settings). Competitive data must join the `collect_push_payload()` path only — venue to cloud, not bidirectional. Mixing the two would allow stale cloud data to overwrite venue scores.

### Pattern 3: Server-Side Telemetry Merge for Comparison

**What:** GET /public/compare-laps merges two lap telemetry arrays on the server into one response. Client receives a single merged array `{offset_ms, speed_a, speed_b, throttle_a, throttle_b, ...}`.

**Why:** recharts dual-trace requires all data in a single array with two `dataKey` values. Server merge is O(n) at the DB layer. Client-side merge would require two HTTP round-trips and non-trivial interpolation JavaScript. Server merge produces the correct shape directly.

**Trade-off:** Server does slightly more CPU per request. Acceptable at venue scale (<50 concurrent).

### Pattern 4: Materialized Championship Standings

**What:** Championship standings are written to the `championship_standings` table after each round is scored, not calculated live on every GET request.

**Why:** Live calculation requires aggregating across N events * M drivers per read request. With no auth on public endpoints, this could be called frequently. Materializing into a table makes reads O(1) per driver. Recalculation only happens on explicit staff trigger (POST /championships/{id}/recalculate) or automatic trigger after event scoring.

### Pattern 5: SVG Track Map from Telemetry Data

**What:** The 2D track map is rendered client-side in SVG by normalizing `pos_x`/`pos_z` telemetry coordinates to an SVG viewport. No pre-generated track asset files are needed.

**Why:** `telemetry_samples` already contains `pos_x`, `pos_y`, `pos_z` for every recorded lap. Any valid lap's position data yields the track outline automatically. This works for any track that has been driven, without needing to register tracks manually or maintain a library of track SVG assets.

**AC coordinate system note:** Assetto Corsa uses right-hand Y-up coordinates. `pos_x` is the East-West axis (longitudinal) and `pos_z` is the North-South axis (lateral). When rendering: SVG `x` = normalized `pos_x`, SVG `y` = normalized `pos_z` (inverted, as SVG Y increases downward).

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Adding Competitive Tables to SYNC_TABLES

**What people do:** Add `hotlap_events`, `championships` to the `SYNC_TABLES` constant string to make them sync bidirectionally.

**Why it's wrong:** SYNC_TABLES is for cloud-authoritative config that the venue pulls (pricing, experiences, settings). Making competitive tables bidirectional would allow the cloud to overwrite venue scores with stale copies — corrupting event results.

**Do this instead:** Add queries to `collect_push_payload()` only. Competitive data is venue-authoritative and one-directional.

### Anti-Pattern 2: Scoring as a Background Task

**What people do:** Run event scoring on a tokio background task triggered by time (e.g., when `ends_at` passes).

**Why it's wrong:** Scoring is a staff-triggered action, not a timer event. Auto-triggering prevents staff from extending events, adding grace periods, or reviewing times before publication. SQLite transaction completes in milliseconds — no background task is needed.

**Do this instead:** Explicit staff action POST /events/hotlap/{id}/score. Simple, predictable, zero state coordination.

### Anti-Pattern 3: Syncing All Telemetry to Cloud

**What people do:** Add `telemetry_samples` to `collect_push_payload()` with no filter.

**Why it's wrong:** A 60-minute session at 60fps on 8 pods = ~1.7 million rows. Syncing all telemetry would saturate the connection and write hundreds of MB to the cloud DB. The 5-connection SQLite pool cannot handle this at sync cadence.

**Do this instead:** Only sync telemetry for laps referenced in `hotlap_event_entries`. This bounds the sync to a reasonable set (typically <20 laps per event).

### Anti-Pattern 4: Client-Side Lap Time Sorting for Rankings

**What people do:** Return all event entry times to the client and sort/rank in JavaScript.

**Why it's wrong:** Ranking requires consistent tiebreaking rules, 107% filtering, and potentially complex adjustments (class handicaps in future). These rules should be authoritative at the server. Client-side ranking can drift from the official server ranking.

**Do this instead:** Always return pre-ranked entries from the server (ORDER BY position ASC after scoring, or ORDER BY lap_time_ms ASC before scoring for live standings).

---

## Open Questions and Gaps

These require decisions before implementation begins — noted here so the roadmap can flag them as requiring pre-phase research.

**1. Car class assignment for existing laps**

The `laps` table currently has no `car_class` column. Hotlap event auto-entry needs to know the class. Two options: (a) look up from `kiosk_experiences` by car name at query time, or (b) add `car_class` to laps and populate from billing_session experience at lap recording time. Option (b) is correct — add `ALTER TABLE laps ADD COLUMN car_class TEXT` and populate in `lap_tracker.rs`. Historical laps will have NULL car_class and will not auto-qualify for events (acceptable).

**2. 107% rule baseline**

The 107% rule requires a reference time (the current leader's time). If an event has zero entries, there is no baseline. Decision: skip 107% check until at least one valid lap exists in the event. The first driver always qualifies. Document this edge case in the scoring logic.

**3. Driver rating formula**

PROJECT.md says "driver skill rating system alongside vehicle-based classes" but gives no formula. Suggested approach: `class_points` accumulates at fixed rates (1 point per valid lap, 10 per event entry, 25 per podium, 50 per win). Class thresholds: Rookie 0-99, Bronze 100-299, Silver 300-599, Gold 600-999, Platinum 1000+. This is a product decision that needs Uday's sign-off before implementation.

**4. Telemetry availability on cloud**

Current cloud sync does NOT push `telemetry_samples`. Track map and lap comparison features require telemetry on cloud. The targeted sync (event laps only) resolves this but requires knowing which laps are event-relevant before they are entered. Race condition: lap completes -> event entry created (same transaction) -> telemetry sync includes that lap_id. The sync must query telemetry by lap_ids found in hotlap_event_entries, not by created_at alone. This requires a JOIN in the sync query rather than a simple timestamp filter — slightly more complex but still one SQL query.

**5. Public leaderboard caching**

The public leaderboard endpoint is called with no auth and no rate limit. At launch scale (Racing Point venue, <100 visitors/day) this is fine. If the endpoint gets shared virally (e.g., WhatsApp share of a championship result), it could spike. A 30-second in-memory cache on the public leaderboard response (same pattern as existing pod status cache) would protect against this. Flag this for the phase that implements public driver profiles.

---

## Sources

- Direct codebase inspection: `crates/rc-core/src/db/mod.rs` (1791 lines, 40+ tables confirmed)
- Direct codebase inspection: `crates/rc-core/src/api/routes.rs` (276 endpoints mapped)
- Direct codebase inspection: `crates/rc-core/src/cloud_sync.rs` (full push/pull logic, SYNC_TABLES constant)
- Direct codebase inspection: `pwa/src/app/` (all existing PWA pages and scaffolds)
- Direct codebase inspection: `pwa/src/components/TelemetryChart.tsx` (recharts implementation with syncId pattern)
- Direct codebase inspection: `.planning/PROJECT.md` (v3.0 requirements)
- Inspiration reference: rps.racecentres.com (Track of the Month, Group Events, Circuit Records, Driver Data patterns)
- Confidence: HIGH — all architectural claims based on code that exists in the repository

---

*Architecture research for: RaceControl v3.0 — Leaderboards, Telemetry & Competitive*
*Researched: 2026-03-14*
