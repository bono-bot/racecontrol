# Pitfalls Research

**Domain:** Sim racing competitive platform — leaderboards, telemetry visualization, championship scoring, driver rating added to existing venue management system (Rust/Axum + SQLite + Next.js)
**Researched:** 2026-03-14
**Confidence:** HIGH — all critical pitfalls verified against actual codebase (db/mod.rs, lap_tracker.rs, cloud_sync.rs) and cross-referenced with game UDP specs, SQLite official docs, and sim racing platform post-mortems.

---

## Critical Pitfalls

### Pitfall 1: telemetry_samples Has No Index — Full Table Scan on Every Lap Load

**What goes wrong:**
The `telemetry_samples` table in `db/mod.rs` is created without any index:
```sql
CREATE TABLE IF NOT EXISTS telemetry_samples (
    lap_id TEXT REFERENCES laps(id),
    offset_ms INTEGER NOT NULL,
    speed REAL, throttle REAL, brake REAL, ...
)
```
At AC's 33Hz telemetry rate, a 90-second lap generates ~3,000 rows per lap. After 1,000 customer laps (roughly one month of moderate use), the table has 3 million rows. Querying `WHERE lap_id = ?` without an index is a full table scan — 3 million comparisons. At 2,000 laps, it's 6 million. The telemetry visualization page will become unusable within weeks of launch.

**Why it happens:**
Telemetry was built for data capture, not for retrieval. The insert path works fine without indexes. The problem is invisible during development (small tables) and only emerges in production at scale.

**How to avoid:**
Add a composite covering index immediately when the table is created:
```sql
CREATE INDEX IF NOT EXISTS idx_telemetry_lap_offset
ON telemetry_samples(lap_id, offset_ms);
```
This makes `SELECT * FROM telemetry_samples WHERE lap_id = ? ORDER BY offset_ms` a pure index scan — sub-millisecond at any realistic scale.

**Warning signs:**
Telemetry chart API endpoint response time is under 5ms in development but grows to 2+ seconds after a few weeks in production. EXPLAIN QUERY PLAN shows `SCAN TABLE telemetry_samples` instead of `SEARCH TABLE telemetry_samples USING INDEX`.

**Phase to address:** Phase 1 (data foundation) — add index in the migration before any telemetry data accumulates. Cannot be deferred.

---

### Pitfall 2: telemetry_samples Volume Will Exhaust SQLite on Cloud

**What goes wrong:**
Telemetry is listed in SYNC_TABLES indirectly — even if not currently synced, there is pressure to sync it to cloud for PWA telemetry charts. At 33Hz for AC and ~20Hz for F1 25, a single 2-minute hotlap generates 3,960–6,600 rows. Eight pods running simultaneously generate ~50,000 rows per minute of active use. At 6 hours of daily venue use, that is 18 million rows per day. The cloud PostgreSQL instance (Bono's VPS) and the venue SQLite both face a table that grows faster than any other by 3 orders of magnitude.

**Why it happens:**
Game telemetry protocols push every physics frame. The system captures all of it because it is available, not because all of it is needed for visualization. Per-frame granularity is not required for a speed trace chart — 10Hz is sufficient for human-readable visualization.

**How to avoid:**
Two strategies, apply both:
1. **Downsample on capture:** In rc-agent, only write a telemetry sample every N frames (e.g., every 3rd frame at 33Hz = 11Hz effective). Store the raw frame counter in `offset_ms` so chart rendering remains accurate. This reduces volume by 3x at capture time.
2. **Downsample on sync:** Never sync raw `telemetry_samples` to the cloud. Instead, sync a pre-aggregated `telemetry_chart_data` table containing one row per lap with the data serialized as a compact JSON blob or msgpack binary. The cloud PWA reads the blob; the venue SQLite keeps the raw samples for AI coaching tools if needed.

**Warning signs:**
`telemetry_samples` table size exceeds 1GB in SQLite. Cloud VPS disk usage climbs continuously. Sync cycle duration exceeds 30 seconds (the current sync interval) because the payload is too large.

**Phase to address:** Phase 1 (data foundation) — downsampling strategy must be decided before telemetry capture goes to production. Retrofitting after 50M rows exist is painful.

---

### Pitfall 3: Leaderboard Queries on laps Table Have No Indexes

**What goes wrong:**
The `laps` table also has no indexes in the current migration (only `ac_sessions` has one). A leaderboard query like:
```sql
SELECT driver_id, MIN(lap_time_ms) FROM laps
WHERE track = ? AND car = ? AND valid = 1
GROUP BY driver_id ORDER BY MIN(lap_time_ms) LIMIT 20
```
scans every row in the laps table. At 200 laps/day with 8 pods, after 6 months you have 36,000 laps. Without an index this query may still be fast, but add filters for events, date ranges, or car classes, and the planner will scan everything. Leaderboard queries must be sub-100ms at all times because the PWA renders them on page load.

**Why it happens:**
The existing `personal_bests` and `track_records` tables provide fast single-row lookups for record queries, but the `laps` table is the source of truth for filtered leaderboards (by event, date range, car class, driver group). These require composite indexes that match the query patterns.

**How to avoid:**
Add these indexes in the migration:
```sql
-- Leaderboard: fastest laps per track+car (valid only)
CREATE INDEX IF NOT EXISTS idx_laps_track_car_valid_time
ON laps(track, car, valid, lap_time_ms);

-- Driver profile: all laps for a driver
CREATE INDEX IF NOT EXISTS idx_laps_driver_created
ON laps(driver_id, created_at);

-- Event leaderboard: laps within a session
CREATE INDEX IF NOT EXISTS idx_laps_session_time
ON laps(session_id, lap_time_ms);
```
The track+car+valid+time index is a covering index for the most common leaderboard query pattern.

**Warning signs:**
EXPLAIN QUERY PLAN shows `SCAN TABLE laps` on a leaderboard endpoint. Response times grow week-over-week as lap count increases.

**Phase to address:** Phase 1 (data foundation) — add all indexes before any leaderboard feature is built. Must be part of the migration, not an afterthought.

---

### Pitfall 4: The valid Flag Trusts the Game Completely

**What goes wrong:**
`lap_tracker.rs` skips laps where `lap.valid == false`. But the `valid` flag originates from the UDP game packet and is only as trustworthy as the game's own track limit detection. AC's UDP protocol does not expose fine-grained cut detection — it uses the server's track boundary cuts configuration. If the AC server preset `RP_OPTIMAL` does not have cut detection configured, every lap is valid regardless of how many corners the driver cut. F1 25 does provide `lapValidityBitFlags` in its UDP spec (0x01 = lap valid, 0x02/0x04/0x08 = sector valid), but these are only populated if the game's own penalty system is active.

At a venue leaderboard level, a driver can legitimately cut every chicane and post a physically impossible lap time that tops the board permanently.

**Why it happens:**
The trust assumption is reasonable for in-session use (billing, driver display), but leaderboard use requires higher integrity. Games are not designed with external leaderboard integrity in mind — their validity flags catch internal game rule violations, not deliberate gaming for external leaderboards.

**How to avoid:**
Two layers:
1. **Sanity range filter:** Reject laps below a minimum plausible time for each track. Store `track_min_plausible_ms` in the `track_records` or a `tracks` table (seeded by staff). Any lap time below this floor is flagged as suspect, not simply invalid.
2. **Sector sum consistency:** For AC, if sector times are available (sector1_ms + sector2_ms + sector3_ms), verify they sum within ±500ms of `lap_time_ms`. A large discrepancy indicates a corrupted or spoofed lap.
3. **Staff review flag:** Add a `suspect` boolean to the laps table. Leaderboards show laps where `valid = 1 AND suspect = 0`. Staff can review flagged laps from the admin dashboard.

**Warning signs:**
A lap time appears on a leaderboard that is significantly faster than the previous record (more than 5% below the field). Sector times do not sum to lap time. The same driver repeatedly posts times that are physically impossible for the car on that track.

**Phase to address:** Phase 2 (leaderboard feature) — validation layer must be in place before the leaderboard goes public. Any public leaderboard without lap validity hardening will be gamed immediately.

---

### Pitfall 5: Cross-Game Comparability Is Impossible — Do Not Try

**What goes wrong:**
The venue runs AC and F1 25. Both games are in `laps.sim_type`. If leaderboards do not filter by `sim_type`, AC laps will appear alongside F1 25 laps for the same circuit (e.g., Silverstone exists in both). The physics, tire models, aerodynamics, and lap time targets are fundamentally different. An AC GT3 car at Silverstone might post 1:42 where an F1 25 car posts 1:28. Mixed leaderboards are misleading and will confuse customers.

**Why it happens:**
The `track` string is game-specific and not standardized. AC uses `ks_silverstone` while F1 25 uses `silverstone` or a variant. However, if track name normalization is added for display purposes, leaderboard joins across sims may accidentally combine them.

**How to avoid:**
- All leaderboard queries MUST include `sim_type` as a filter at the query level, not just at the display level.
- The leaderboard page URL should encode the sim: `/leaderboard/ac/silverstone` not `/leaderboard/silverstone`.
- Never create a "combined" leaderboard. The car class system (A/B/C/D) must be within a single sim, not across sims.
- Driver ratings and skill scores must be tracked per-sim, not globally. A driver's AC rating means nothing for F1 25 performance.

**Warning signs:**
A leaderboard query joins laps without a `sim_type` filter. The track name normalization map accidentally collapses `ks_silverstone` and `silverstone` into the same string.

**Phase to address:** Phase 2 (leaderboard feature) — sim_type must be a required parameter in every leaderboard API endpoint. Reject requests without it.

---

### Pitfall 6: SQLite WAL Checkpoint Starvation Under Concurrent Reads

**What goes wrong:**
`db/mod.rs` enables WAL mode (`PRAGMA journal_mode=WAL`) but does not configure `PRAGMA wal_autocheckpoint`. The default autocheckpoint triggers every 1,000 pages. Under a competitive event with 8 pods simultaneously writing laps AND the cloud sync loop AND the PWA leaderboard polling, the WAL file can grow unbounded if there is always at least one active reader when the checkpoint tries to run. SQLite's WAL documentation explicitly states: "If a database has many concurrent overlapping readers...no checkpoints will be able to complete and the WAL file will grow without bound."

A growing WAL file means every read must traverse more of the WAL to reconstruct the current state, so read performance degrades proportionally to WAL file size.

**Why it happens:**
WAL mode is enabled as a best practice but the connection pool (`max_connections(5)`) means there are always connections held open by sqlx. The sqlx pool keeps connections alive, which means readers are never fully absent, which means checkpoints may be perpetually blocked.

**How to avoid:**
```sql
PRAGMA wal_autocheckpoint=400;   -- checkpoint every 400 pages (~1.6MB) not 1000
PRAGMA busy_timeout=5000;        -- wait up to 5s instead of failing immediately
```
Also configure sqlx pool with a maximum connection lifetime so connections are periodically recycled, giving the checkpoint a window to complete:
```rust
SqlitePoolOptions::new()
    .max_connections(5)
    .max_lifetime(Duration::from_secs(300))  // recycle connections every 5 minutes
    .connect(&url)
```

**Warning signs:**
The WAL file at `racecontrol.db-wal` grows continuously and never shrinks. Read latency increases over uptime (hours). `PRAGMA wal_checkpoint(PASSIVE)` run manually returns a non-zero `busy` count.

**Phase to address:** Phase 1 (data foundation) — add these pragmas to `db/mod.rs` alongside the existing WAL mode setup.

---

### Pitfall 7: Cloud Sync ID Mismatch Corrupts Competitive Data

**What goes wrong:**
MEMORY.md documents the known ID mismatch: "Local/cloud have different UUIDs. sync_push resolves by phone/email." The cloud sync in `cloud_sync.rs` currently covers `drivers,wallets,pricing_tiers,pricing_rules,billing_rates,kiosk_experiences,kiosk_settings` — the competitive tables (`laps`, `personal_bests`, `track_records`, `events`, `event_entries`) are NOT in SYNC_TABLES.

When competitive tables ARE added to sync (which v3.0 requires), the ID mismatch problem becomes acute: a driver's laps reference `driver_id` from the local UUID, but the cloud record for that driver may have a different UUID. Foreign key integrity breaks silently. The leaderboard on the cloud PWA shows laps with no associated driver name.

**Why it happens:**
The sync was designed for configuration data (pricing, settings) where ID consistency is enforced by the cloud being authoritative. For locally-generated data (laps, sessions), the venue is authoritative and IDs are generated locally. When the same driver registers on both systems through different flows, two UUIDs exist for one person.

**How to avoid:**
- **Resolve IDs before syncing laps.** The sync must verify that for every `driver_id` referenced in a lap being pushed, the cloud has a matching driver record with the same UUID. If not, resolve via phone/email match first, then push the lap.
- **Use the cloud-assigned UUID as canonical.** Once a driver is registered on cloud, push the cloud UUID back to the venue. All future laps are written with the cloud UUID from the start.
- **Add a `cloud_driver_id` column** to the venue's `drivers` table for the mapping. Lap sync uses `cloud_driver_id` not `id`.
- **Never sync laps for drivers whose IDs are unresolved.** The sync should skip and log rather than push orphaned lap records.

**Warning signs:**
Cloud leaderboard shows laps with `driver_id` that does not match any driver in the cloud driver table. PWA shows "Unknown Driver" on leaderboards. After sync, `COUNT(DISTINCT driver_id) FROM laps` exceeds `COUNT(*) FROM drivers` on the cloud.

**Phase to address:** Phase 1 (data foundation) — the ID resolution strategy must be finalized before any lap sync is implemented. Build the `cloud_driver_id` mapping column into the migration.

---

### Pitfall 8: Driver Rating With Hotlap-Only Data Is Fundamentally Broken

**What goes wrong:**
A skill rating system (Elo or otherwise) requires head-to-head comparisons to be meaningful. In a venue hotlap context, drivers never race each other simultaneously — they each set times on different days, in different conditions, possibly in different cars. Applying standard Elo to hotlap times produces ratings that measure car choice and track familiarity, not driver skill. A driver who exclusively uses the fastest car in class A will rate higher than an equally skilled driver who varies their car choices.

Research into generalizing Elo for racing (Powell, Journal of Quantitative Analysis in Sports, 2024) confirms that standard Elo is not appropriate for time-only competitions — it requires positional outcome comparison.

**Why it happens:**
Elo is the go-to for competitive rating because it is simple and well-understood. Developers apply it without considering that hotlap times require normalization across car+track combinations before comparison is valid.

**How to avoid:**
For a venue leaderboard context, use a **percentile-based class rating** instead of Elo:
1. For each driver+track+car combination, compute where their personal best falls in the distribution of all times on that track+car.
2. A driver's class rating is their median percentile across all track+car combinations they have competed on, weighted by sample count.
3. Classes A/B/C/D map to percentile bands (A = top 10%, B = 11-30%, C = 31-60%, D = 61-100%).

This is fair, transparent to customers, and does not require head-to-head data. Reserve Elo-style ratings for actual group events where finishing order is known.

**Warning signs:**
The top-rated driver exclusively uses the single fastest car in the catalog. A new driver with 1 fast lap in the fastest car outranks a veteran with 200 laps in varied cars. Rating changes wildly after a single session.

**Phase to address:** Phase 3 (driver profiles and rating) — algorithm design must be settled before any rating is displayed publicly. Published ratings are difficult to retract without damaging trust.

---

### Pitfall 9: Championship Scoring Has Silent Edge Cases That Break Standings

**What goes wrong:**
F1-style scoring (25/18/15/12/10/8/6/4/2/1) applied naively produces wrong standings when:
- A driver DNFs or does not start a round — do they score 0 or are they excluded?
- Two drivers tie on points for the championship — tiebreaker is not defined (most wins? most podiums? alphabetical?)
- A driver registers for a championship round but never sets a lap — do they score last-place points or null?
- A championship has 5 drivers and only 5 scoring positions — what about 6th driver added mid-championship?
- A round is cancelled after some laps are set — do those laps count toward the round result?

Each of these edge cases, unhandled, produces incorrect standings that are visible to all customers on the public PWA.

**Why it happens:**
Scoring rules are defined for ideal conditions. Edge cases only appear in production. Since the scoring table (`event_entries.result_position`) does not distinguish between "DNS" (did not start), "DNF" (did not finish), and "last place finisher," the system cannot apply the correct rule.

**How to avoid:**
- Add a `result_status` column to `event_entries`: `'DNS' | 'DNF' | 'finished' | 'pending'`.
- Define tiebreaker rules in the championship configuration (stored in `config_json`) and implement them explicitly in the scoring query.
- Write characterization tests for all edge cases BEFORE implementing scoring. Tests must cover: tie on points, DNS, DNF, late registration, round cancellation.
- Points are only awarded to drivers with `result_status = 'finished'`. DNS/DNF score zero.

**Warning signs:**
Two drivers show the same championship points total and the leaderboard ordering is non-deterministic (sorted by insertion order). A driver who did not participate in a round appears with a result position. Championship standings change unexpectedly after a late lap is recorded.

**Phase to address:** Phase 2 (events and championship) — scoring rules must be fully specified and tested before the first event is created.

---

### Pitfall 10: Track Map Generation from Position Data Requires Normalization

**What goes wrong:**
AC's UDP telemetry provides `pos_x`, `pos_y`, `pos_z` in world space coordinates. F1 25 provides `worldPositionX/Y/Z`. These are absolute 3D coordinates in the game world's coordinate system — they are not latitude/longitude and they vary by track, car, and game. A 2D track map overlay requires:
1. Projecting 3D positions to 2D (typically XZ plane, dropping Y/altitude)
2. Normalizing to canvas coordinates (scale + translate to fit viewport)
3. Aligning multiple laps from different drivers (same coordinate system, but floating point drift can cause misalignment)
4. Handling the gap between the end of one lap and the start of the next (the car teleports to the start line)

If coordinates from AC and F1 25 are mixed on the same "track" (same track name, different sim), the scale and coordinate system are completely different and will produce visual garbage.

**Why it happens:**
Position data is straightforward to capture, but coordinate system normalization is non-trivial. Game developers choose coordinate systems for physics fidelity, not for external visualization.

**How to avoid:**
- Generate track map templates offline: run one lap per track per sim, capture the coordinate bounding box, store as a `track_map_bounds` configuration (minX, maxX, minZ, maxZ per track+sim).
- The rendering layer uses these bounds to normalize any lap's XZ coordinates to [0,1] space, then scales to the canvas. This means any lap on that track auto-fits correctly.
- Store position data as `pos_x_norm` and `pos_z_norm` (normalized) in the chart data blob, not raw game coordinates. Eliminates the normalization work from the browser.
- Never mix AC and F1 25 position data on the same map. The track_map_bounds are keyed by `(track, sim_type)`.

**Warning signs:**
Track map shows a straight line or a single dot (coordinates not normalized). Two laps of the same track do not overlap (coordinate drift). The map looks correct for one driver but rotated/mirrored for another (different car starting positions).

**Phase to address:** Phase 3 (telemetry visualization) — a pre-computation step to capture bounds per track+sim must be done as venue setup before the feature is enabled for customers.

---

### Pitfall 11: Public Leaderboard Caching Strategy Mismatches Data Freshness Expectations

**What goes wrong:**
The Next.js PWA at app.racingpoint.cloud serves leaderboards publicly. Without caching, every page load hits the cloud API (Axum on Bono's VPS), which queries the cloud database. With 8 pods potentially setting records in real time, a customer checking the leaderboard 30 seconds after a record expects to see the new record. If the page is statically generated (SSG with ISR), the revalidation period determines staleness. ISR with `revalidate: 60` means a record is invisible for up to 60 seconds — acceptable. ISR with `revalidate: 3600` means a record set at 2pm is invisible until 3pm — unacceptable for a competitive venue.

Conversely, with no caching (SSR on every request), the Bono VPS handles every page load directly. During a busy group event, 20 customers hitting the leaderboard simultaneously = 20 database queries per second, which is fine for SQLite/Postgres at this scale but wastes resources.

**Why it happens:**
Caching decisions are made at framework setup and are not revisited. The wrong ISR revalidation period is set once and forgotten. Freshness expectations in a competitive racing context are much higher than for typical web content.

**How to avoid:**
- Use ISR with `revalidate: 30` for leaderboard pages — matches the 30s cloud sync interval from the venue.
- Use `on-demand revalidation` for record-setting events: when the cloud sync receives a new track record, call `revalidateTag('leaderboard')` to purge the ISR cache immediately.
- The `/api/leaderboard/[track]` endpoint should set `Cache-Control: s-maxage=30, stale-while-revalidate=60` for CDN edge caching.
- Add a "last updated" timestamp to every leaderboard page so customers know the data age.

**Warning signs:**
A customer refreshes the leaderboard and does not see their own record they just set. The ISR revalidation period in `page.tsx` is hardcoded to a large value like 3600. The API route has no Cache-Control header.

**Phase to address:** Phase 2 (leaderboard public surface) — caching strategy must be designed alongside the API, not added after.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| No index on telemetry_samples | Works fine at dev scale | Full table scan after 1 month — unusable visualization | Never — add index in migration day one |
| Sync raw telemetry_samples to cloud | Simple sync implementation | VPS disk exhausted in weeks, sync cycle exceeds interval | Never — sync pre-aggregated blobs only |
| Trust game valid flag only | Zero extra code | Easily gamed leaderboard, customer complaints | Never for public leaderboard; acceptable for billing display |
| No result_status for event entries | Simpler schema | Cannot distinguish DNS/DNF/finished, broken standings | Never — add result_status before first event |
| Global driver rating across sims | One score per driver | Meaningless rating for multi-game venue | Never — must be per sim_type |
| ISR revalidate > 60s for leaderboards | Fewer cloud requests | Records invisible for minutes — defeats competitive purpose | Only for historical records older than 7 days |
| Raw world coordinates stored in telemetry | Exactly what game sends | Browser must normalize; coordinate system coupling to specific game version | Acceptable at MVP if normalization happens at render time; fix in Phase 3 |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| AC UDP sectors | Sector times not always populated — AC sends 0 for sectors on incomplete laps | Check for `sector_ms > 0 AND sector_ms < lap_time_ms` before storing sectors; store NULL not 0 |
| F1 25 UDP lap validity | Reading `currentLapInvalid` flag only | Also check `lapValidityBitFlags` in `LapHistoryData` (0x01 = lap valid); the per-session flag resets, history flags are permanent |
| F1 25 UDP packet loss | Running at 60Hz for fine telemetry | Run at 20Hz max; higher rates cause packet loss on LAN UDP; use `offset_ms` to detect gaps in received data |
| Cloud sync timestamp comparison | Mixing ISO-T and space-separator formats | Already hit in v1.0 — `normalize_timestamp()` in cloud_sync.rs handles this. Any new competitive tables synced must pass through same normalization |
| SQLite concurrent writes during group event | 8 pods writing laps simultaneously | sqlx pool serializes writes naturally. Do NOT increase `max_connections` above 5 for the write path — more connections increase WAL contention |
| SQLite on cloud VPS | Assuming SQLite on Bono's VPS is fine for public reads | Cloud rc-core uses SQLite too. Under concurrent PWA traffic, add `PRAGMA busy_timeout=5000` so reads queue instead of returning SQLITE_BUSY |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| No telemetry_samples index | Telemetry load time grows week-over-week | `CREATE INDEX idx_telemetry_lap_offset ON telemetry_samples(lap_id, offset_ms)` at migration | After ~500 laps (~1.5M rows) |
| Unbounded telemetry capture | SQLite DB grows without limit | Downsample to 10Hz at capture; or cap rows per lap (`LIMIT 600` = 60s at 10Hz) | After ~2 weeks of 8-pod operation |
| Leaderboard GROUP BY without covering index | GROUP BY+ORDER BY forces temp table sort | Covering index `(track, car, valid, lap_time_ms)` eliminates sort | After ~5,000 laps |
| WAL file growing without checkpoint | Read latency grows proportional to WAL size | `PRAGMA wal_autocheckpoint=400` + connection max_lifetime | After ~6 hours of continuous operation |
| Per-request leaderboard query on cloud | VPS handles O(N) queries during busy events | ISR cache + on-demand revalidation | At ~10 concurrent PWA users |
| Track map normalization in browser | CPU spike on telemetry page open | Pre-compute normalized coordinates before storing | On mobile devices immediately |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Public leaderboard accepts driver_id filter without validation | Driver can enumerate other drivers' lap details | All public endpoints must only expose aggregated data (best lap, rank). Raw lap details only visible with driver authentication or for the driver themselves |
| Championship scoring re-runs on every request | A race condition during concurrent scoring updates produces inconsistent results | Scoring calculation should be idempotent and cached — run once at event close, store result in `event_entries.result_position`, serve from stored value |
| telemetry endpoint has no size limit | A malformed lap_id could trigger a query returning millions of rows | Always add `LIMIT` to telemetry queries (e.g., `LIMIT 2000`) as a hard safety cap |
| Track record update is not atomic | Concurrent lap completions on two pods could both see the same "current record" and both update, leaving a stale record if the second update has a higher time | Wrap `personal_bests` and `track_records` updates in a single transaction with `BEGIN IMMEDIATE` to serialize |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Leaderboard shows "Unknown Driver" | Breaks social sharing; driver can't prove it's their time | Never sync a lap without a resolved driver_id; show placeholder "Venue Driver" only if ID truly unresolvable |
| Rating drops after more laps | Driver is discouraged from driving more | Use a percentile with minimum sample floor (e.g., at least 5 laps before rating shown); never penalize volume |
| Track map renders as straight line on first visit | Customer thinks visualization is broken | Show loading state and only render map when >50 points are available; validate coordinate variance before rendering |
| Championship standings change retroactively | Customers see their historical position change | Lock standings at round close; mark rounds as "final" — subsequent lap data does not affect closed rounds |
| Sector times shown as "0:00.000" when unavailable | Confusing for customers | Show "–" not zero for NULL sector times; add tooltip explaining sector data availability per sim |

---

## "Looks Done But Isn't" Checklist

- [ ] **Telemetry index:** `EXPLAIN QUERY PLAN SELECT * FROM telemetry_samples WHERE lap_id = 'test'` shows `USING INDEX`, not `SCAN TABLE`. Verify after migration runs on production DB.
- [ ] **Lap validity:** Submit a test lap with sector1+2+3 that do not sum to lap_time — verify it is flagged as suspect, not silently accepted.
- [ ] **Sim type filter:** Call `/api/leaderboard/{track}` without `sim_type` parameter — should return 400 or default to a single sim, never mix AC and F1 laps.
- [ ] **Championship tiebreaker:** Create two drivers with identical points — verify standings order is deterministic and documented (not random).
- [ ] **Track map bounds:** Open telemetry chart for a lap and verify the track outline fills the canvas (not a dot in one corner, not a line).
- [ ] **Cloud sync driver ID:** Push a lap whose driver_id is not in the cloud drivers table — verify it is not synced (not silently orphaned).
- [ ] **WAL checkpoint:** After 1 hour of load test, check `ls -lh racecontrol.db-wal` — WAL file should be under 2MB, not growing.
- [ ] **ISR freshness:** Set a new track record in venue, wait 35 seconds, reload public leaderboard — new record must appear (ISR revalidate ≤ 30s).
- [ ] **result_status for events:** Record a lap for a driver who was marked DNS — verify they score 0 points, not last-place points.
- [ ] **F1 25 at 60Hz:** Verify packet loss is not occurring — check that `offset_ms` values in stored telemetry have consistent ~50ms gaps (20Hz) not irregular gaps indicating drops.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| No telemetry index discovered after data accumulates | LOW | `CREATE INDEX idx_telemetry_lap_offset ON telemetry_samples(lap_id, offset_ms)` — runs online in SQLite without locking reads; may take 30-60s on large table |
| Telemetry volume exhausts disk | MEDIUM | Add `WHERE lap_id IN (SELECT id FROM laps ORDER BY created_at DESC LIMIT 1000)` safety scope; run `DELETE FROM telemetry_samples WHERE lap_id NOT IN (SELECT id FROM laps ORDER BY created_at DESC LIMIT 2000)` to prune oldest; add downsampling to rc-agent |
| Leaderboard gamed by invalid laps | LOW | Add `suspect` column via ALTER TABLE; mark suspicious laps manually via admin; re-run leaderboard query with `AND suspect = 0` filter |
| Cloud sync ID mismatch corrupts lap data | HIGH | Export mismatched lap records; run manual phone/email reconciliation to map local UUIDs to cloud UUIDs; UPDATE laps SET driver_id = ? WHERE driver_id = ?; re-sync |
| Championship scoring edge case produces wrong standings | MEDIUM | Identify the affected round; recalculate manually; UPDATE event_entries SET result_position = ?, points = ? WHERE event_id = ? AND driver_id = ?; publish correction notice |
| WAL file grown to hundreds of MB | LOW | `PRAGMA wal_checkpoint(TRUNCATE)` during a low-traffic window; verify it returns (0,0,0); adjust wal_autocheckpoint for future |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| telemetry_samples no index | Phase 1: Data Foundation | EXPLAIN QUERY PLAN shows index use |
| telemetry volume / sync explosion | Phase 1: Data Foundation | 30-day volume projection stays under 500MB |
| laps table no leaderboard indexes | Phase 1: Data Foundation | Leaderboard API p99 < 50ms at 10k laps |
| valid flag only trusts game | Phase 2: Leaderboards | Suspect lap filter active on public endpoints |
| Cross-game comparability | Phase 2: Leaderboards | sim_type required in all leaderboard API routes |
| WAL checkpoint starvation | Phase 1: Data Foundation | WAL file stays < 2MB after 6h uptime |
| Cloud sync ID mismatch | Phase 1: Data Foundation | cloud_driver_id column present; unresolved IDs block sync |
| Driver rating algorithm | Phase 3: Driver Profiles | Rating ignores car choice; percentile method verified against test data |
| Championship scoring edge cases | Phase 2: Events | All edge case tests pass before first event created |
| Track map coordinate normalization | Phase 3: Telemetry Visualization | Track outline fills canvas on all 8 test tracks |
| PWA leaderboard cache staleness | Phase 2: Leaderboards | Record appears within 35s of being set at venue |
| telemetry endpoint no size limit | Phase 3: Telemetry Visualization | API rejects or caps at 2000 rows per lap |

---

## Sources

- **db/mod.rs (codebase — HIGH):** Confirmed telemetry_samples has no index; confirmed laps has no composite index; confirmed WAL mode enabled without checkpoint tuning; confirmed SYNC_TABLES excludes lap tables
- **lap_tracker.rs (codebase — HIGH):** Confirmed valid flag is trusted directly from game UDP packet; confirmed track_records update is not wrapped in a transaction
- **cloud_sync.rs (codebase — HIGH):** Confirmed ID mismatch problem is documented and partially handled for drivers; lap sync not yet implemented
- **[SQLite WAL documentation — sqlite.org (HIGH)](https://sqlite.org/wal.html):** "If a database has many concurrent overlapping readers...no checkpoints will be able to complete and the WAL file will grow without bound" — direct quote confirming starvation risk
- **[SQLite Query Optimizer — sqlite.org (HIGH)](https://sqlite.org/optoverview.html):** Covering index behavior for GROUP BY + ORDER BY leaderboard patterns confirmed
- **[F1 25 UDP Specification — EA Forums (HIGH)](https://forums.ea.com/discussions/f1-25-general-discussion-en/f1-2025-udp-specification/12082129):** LapHistoryData bit flags (0x01 lap valid, 0x02/0x04/0x08 sector valid); 20Hz recommended; 60Hz causes packet loss
- **[AC UDP telemetry — rickwest/ac-remote-telemetry-client (MEDIUM)](https://github.com/rickwest/ac-remote-telemetry-client):** Confirmed sector times not always populated; UDP "does not provide enough details for automatic track detection"
- **[Generalizing Elo for racing — de Gruyter (MEDIUM)](https://www.degruyterbrill.com/document/doi/10.1515/jqas-2023-0004/html):** Standard Elo inappropriate for time-only competitions; speed-Elo vs endure-Elo distinction; hotlap-only venue requires different approach
- **[SQLite performance tuning — phiresky (HIGH)](https://phiresky.github.io/blog/2020/sqlite-performance-tuning/):** WAL mode + PRAGMA busy_timeout + connection pool configuration confirmed effective
- **[High-performance time series SQLite — DEV Community (MEDIUM)](https://dev.to/zanzythebar/building-high-performance-time-series-on-sqlite-with-go-uuidv7-sqlc-and-libsql-3ejb):** Monthly table partitioning for time series; downsampling recommendation for high-frequency capture
- **[Next.js ISR caching — nextjs.org (HIGH)](https://nextjs.org/docs/app/guides/caching):** On-demand revalidation with revalidateTag; s-maxage Cache-Control for leaderboard freshness
- **[Sim Racing Alliance rules — simracingalliance.com (MEDIUM)](https://www.simracingalliance.com/about/rules):** Championship scoring edge case handling (DNS, DNF, point tiebreakers) documented in competitive organizations
- **[iRacing cheating prevention — bsimracing.com (MEDIUM)](https://www.bsimracing.com/iracing-new-cheat-prevention-detection-system-coming/):** Detection via telemetry review; automated watches on suspicious results — informs suspect flag approach
- **MEMORY.md (HIGH):** ID mismatch already known and documented; timestamp normalization bug already fixed; existing sync tables confirmed

---
*Pitfalls research for: RaceControl v3.0 — Leaderboards, Telemetry and Competitive Features*
*Researched: 2026-03-14*
