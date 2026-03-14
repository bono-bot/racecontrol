# Stack Research

**Domain:** Sim racing competitive platform — leaderboards, telemetry visualization, championship management, driver skill rating
**Researched:** 2026-03-14
**Confidence:** HIGH (existing stack verified directly in codebase; new additions verified via npm registry, GitHub, official SQLite docs)

---

> **Note:** This file covers v3.0 Leaderboards, Telemetry & Competitive stack additions.
> The v2.0 Kiosk URL Reliability stack (NSSM, DHCP reservation, hosts file) remains unchanged.
> v3.0 adds zero new infrastructure — all new features extend the existing Rust/SQLite/Next.js stack.

---

## Context: What Already Exists (Do Not Re-add)

| Technology | Version | Role |
|------------|---------|------|
| Rust/Axum | 0.8 | rc-core HTTP server (port 8080) — all backend logic |
| sqlx + SQLite | 0.8 | All persistent storage — laps, telemetry, drivers, billing |
| Next.js App Router | 16.1.6 | PWA at app.racingpoint.cloud |
| React | 19.2.3 | PWA UI |
| Tailwind CSS | 4.x | Styling |
| recharts | 3.8.0 | Charts — speed, throttle, brake, steering, gear, RPM traces already built in TelemetryChart.tsx |
| tokio, serde, reqwest, chrono | workspace | Rust async, serialization, HTTP client, time |
| cloud_sync.rs | existing | Pushes laps, personal_bests, track_records, drivers to VPS every 30s |

**Key existing endpoints already built:**
- `GET /public/laps/{id}/telemetry` — returns speed, throttle, brake, steering, gear, rpm, pos_x/pos_y/pos_z samples
- `GET /public/leaderboard` — top laps per track
- `GET /public/leaderboard/{track}` — filtered by track
- `GET /events` — event list (schema exists, logic is stub)
- `GET /drivers/{id}/full-profile` — driver data

**PWA scaffold pages already exist (mostly empty):**
- `/leaderboard`, `/leaderboard/public` — leaderboard UI stubs
- `/telemetry` — live telemetry (fully built for real-time display)
- `/sessions/[id]` — session results
- `/tournaments` — championships stub

---

## New Stack Additions Required

### Frontend (PWA)

#### Charting: Stay on recharts 3.8.0 — no new library

recharts 3.8.0 is already installed, wired, and working. It supports all v3.0 chart needs:

- **Lap comparison:** Two datasets merged onto one `<ComposedChart>` using separate dataKeys (`lap_a_speed`, `lap_b_speed`) on a shared time axis (offset_ms). Multiple `<Line>` components per chart — supported natively.
- **Synchronized crosshairs:** `syncId` prop on each `<LineChart>` or `<AreaChart>` keeps speed, throttle, brake, and steering charts in sync when hovering. Already used in TelemetryChart.tsx.
- **Sector delta overlays:** Bar or reference line annotations on the existing chart types.

**Decision: no second charting library.** ECharts (600KB), Chart.js (230KB), and Victory (220KB) all add substantial bundle weight for zero capability gain over the already-working recharts integration.

#### Track Map: Canvas API via useRef — no library

The 2D track map renders pos_x/pos_z telemetry points as a polyline on an HTML `<canvas>`. This is 40-60 lines of vanilla canvas code in a `useRef` + `useEffect` hook. No external library is warranted.

**Why no D3:** D3's SVG approach adds 80KB for what is `ctx.lineTo(normalizedX, normalizedZ)` in a loop. Canvas renders crisply at mobile DPR with no DOM node overhead per point. A single lap at 60Hz for 90 seconds produces ~5,400 samples — SVG at that scale is visibly slow on phone CPUs.

**Why no react-konva or Pixi.js:** Scene graph frameworks (Konva, Pixi) are justified for interactive game-like rendering. A static track outline with a moving car dot does not need a retained scene graph or WebGL context.

**Implementation pattern:**

```typescript
// components/TrackMap.tsx
const canvasRef = useRef<HTMLCanvasElement>(null);

useEffect(() => {
  const canvas = canvasRef.current;
  const ctx = canvas?.getContext("2d");
  if (!ctx || !samples.length) return;

  // Normalize pos_x, pos_z into [0, canvas.width] x [0, canvas.height]
  const xs = samples.map(s => s.pos_x);
  const zs = samples.map(s => s.pos_z);
  const minX = Math.min(...xs), maxX = Math.max(...xs);
  const minZ = Math.min(...zs), maxZ = Math.max(...zs);
  const scale = Math.min(canvas.width / (maxX - minX), canvas.height / (maxZ - minZ));

  const toCanvas = (x: number, z: number) => ({
    cx: (x - minX) * scale + padding,
    cy: (z - minZ) * scale + padding,
  });

  // Draw reference lap outline in grey
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  ctx.strokeStyle = "#5A5A5A";
  ctx.lineWidth = 2;
  ctx.beginPath();
  samples.forEach((s, i) => {
    const { cx, cy } = toCanvas(s.pos_x, s.pos_z);
    i === 0 ? ctx.moveTo(cx, cy) : ctx.lineTo(cx, cy);
  });
  ctx.stroke();

  // Draw comparison lap in red (if present)
  if (compSamples?.length) {
    ctx.strokeStyle = "#E10600";
    // ... same pattern
  }

  // Draw car dot at hoverOffset position
  if (hoverOffset !== null) {
    const nearest = findNearestSample(samples, hoverOffset);
    const { cx, cy } = toCanvas(nearest.pos_x, nearest.pos_z);
    ctx.fillStyle = "#FFFFFF";
    ctx.beginPath();
    ctx.arc(cx, cy, 5, 0, Math.PI * 2);
    ctx.fill();
  }
}, [samples, compSamples, hoverOffset]);
```

#### Date Formatting: date-fns 4.1.0

Required for event timelines ("Event closes in 3 days"), championship standings ("Updated 2 hours ago"), and driver profile history ("Best lap set 5 days ago"). date-fns 4.1.0 is ESM-first and tree-shakes correctly in Next.js App Router server components.

```bash
# Install in pwa/
npm install date-fns
```

**Why not dayjs:** Both are functionally equivalent. date-fns chosen because it tree-shakes to individual function imports (no side effects) — more compatible with Next.js server component boundaries.

**Why not Luxon:** 25KB min+gzip vs date-fns ~13KB. No meaningful feature advantage for formatting and relative time calculations.

#### Driver Skill Rating: Custom percentile in Rust — no npm package

**Decision: custom percentile-based rating implemented in rc-core Rust, not a Glicko-2 npm package.**

Research findings:
- `glicko2` npm (v1.2.1) was last published 2 years ago. `glicko2.ts` (v1.3.2) is more recent but low-maintenance.
- Glicko-2 is designed for head-to-head match outcomes (win/loss). Hotlap leaderboards produce lap times, not match results. Adapting Glicko-2 requires treating each lap as a synthetic match against every other driver — a brittle workaround that produces inflated confidence intervals for inactive drivers.
- A percentile system is more explainable to venue customers: "You are in the top 15% at Spa" is more actionable than a Glicko RD number.
- iRating (iRacing's system) is Elo-based and similarly requires head-to-head result construction. Not suitable for time trials.

**Rating algorithm (pure Rust, zero new dependencies):**

```rust
// src/driver_rating.rs
// For each driver, compute percentile across all drivers on same track+car:
// percentile = (drivers_with_worse_time / total_drivers_with_times) * 100

// Class assignment:
// A: top 10%  (percentile >= 90)
// B: 10-25%   (percentile >= 75)
// C: 25-50%   (percentile >= 50)
// D: bottom 50% (percentile < 50)

// 107% rule: laps > (class_A_fastest * 1.07) excluded from class placement
// Recalculate nightly via tokio::time::interval(Duration::from_hours(24))
```

Store results in a new `driver_ratings` table. Recompute on demand when new track records are set, and nightly for full recalibration.

---

### Backend (rc-core Rust)

#### No new Rust crates required

All v3.0 backend features are achievable with existing workspace dependencies:

| Feature | Implementation | Existing dep used |
|---------|---------------|-------------------|
| Championship points accumulation | SQLite `SUM() OVER`, `ROW_NUMBER() OVER` via sqlx raw query strings | sqlx 0.8 |
| F1-style scoring (25/18/15/12/10/8/6/4/2/1) | Rust `const` array, applied when event closes | serde_json for response |
| 107% rule filtering | SQL `WHERE lap_time_ms <= ? * 1.07` in event entry query | sqlx 0.8 |
| Gold/silver/bronze badge assignment | Computed at event close: position 1=gold, 2=silver, 3=bronze | serde_json |
| Driver rating recalculation | `tokio::time::interval` scheduled task, pure arithmetic in Rust | tokio |
| Lap comparison API | New endpoint returning two laps' telemetry merged by lap_id pair | sqlx 0.8 |
| Public championship routes | Extend `api/routes.rs` with unauthenticated routes | axum 0.8 |

**SQLite is correct at venue scale.** 8 pods, ~50 active drivers, events weekly. The largest leaderboard query will never exceed a few thousand rows. SQLite WAL mode (already enabled: `PRAGMA journal_mode=WAL`) handles concurrent reads from cloud_sync and the API server without contention.

**SQLite window function support:** Available since SQLite 3.25.0 (September 2018). The version bundled with sqlx-sqlite feature is well above this threshold. `ROW_NUMBER() OVER` and `SUM() OVER` work directly via `sqlx::query_as` with raw SQL.

**sqlx window function quirk:** When using `OVER` clauses, sqlx infers nullable `Option<T>` for non-aggregate columns even when they cannot be NULL. Use the `!` column suffix in query! macros, or select into `Option<T>` and `.unwrap()` safely. See: https://github.com/launchbadge/sqlx/issues/2874

#### New schema tables (SQL migrations in db/mod.rs)

Add as additional `sqlx::query(...).execute(pool).await?` blocks following the existing pattern in `migrate()`:

```sql
-- Championships (multi-round competition)
CREATE TABLE IF NOT EXISTS championships (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT DEFAULT 'active',          -- active | completed | archived
    scoring_system TEXT DEFAULT 'f1_2025', -- f1_2025 | points_json
    created_at TEXT DEFAULT (datetime('now'))
);

-- Links events to championships (one event = one round)
CREATE TABLE IF NOT EXISTS championship_rounds (
    id TEXT PRIMARY KEY,
    championship_id TEXT NOT NULL REFERENCES championships(id),
    event_id TEXT NOT NULL REFERENCES events(id),
    round_number INTEGER NOT NULL,
    created_at TEXT DEFAULT (datetime('now'))
);

-- Cumulative standings (updated when each round closes)
CREATE TABLE IF NOT EXISTS championship_standings (
    championship_id TEXT NOT NULL REFERENCES championships(id),
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    points INTEGER DEFAULT 0,
    rounds_scored INTEGER DEFAULT 0,
    best_finish INTEGER,
    updated_at TEXT DEFAULT (datetime('now')),
    PRIMARY KEY (championship_id, driver_id)
);

-- Driver class rating (recomputed nightly)
CREATE TABLE IF NOT EXISTS driver_ratings (
    driver_id TEXT PRIMARY KEY REFERENCES drivers(id),
    class TEXT NOT NULL DEFAULT 'D',    -- A | B | C | D
    rating_score REAL DEFAULT 0.0,      -- 0.0-100.0 percentile
    sample_count INTEGER DEFAULT 0,     -- how many track+car combos rated on
    computed_at TEXT DEFAULT (datetime('now'))
);
```

Additionally, add columns to existing tables via `ALTER TABLE IF NOT EXISTS`:

```sql
-- events table: hotlap event config
ALTER TABLE events ADD COLUMN cutoff_107_percent INTEGER;  -- fastest_ms * 1.07 computed on close
ALTER TABLE events ADD COLUMN badge_gold_position INTEGER DEFAULT 1;
ALTER TABLE events ADD COLUMN badge_silver_position INTEGER DEFAULT 2;
ALTER TABLE events ADD COLUMN badge_bronze_position INTEGER DEFAULT 3;
ALTER TABLE events ADD COLUMN scoring_system TEXT DEFAULT 'f1_2025';

-- event_entries table: results
ALTER TABLE event_entries ADD COLUMN points_awarded INTEGER DEFAULT 0;
ALTER TABLE event_entries ADD COLUMN badge TEXT;            -- gold | silver | bronze | NULL
ALTER TABLE event_entries ADD COLUMN gap_to_first_ms INTEGER;
ALTER TABLE event_entries ADD COLUMN lap_id TEXT REFERENCES laps(id);
```

**Note:** SQLite `ALTER TABLE ADD COLUMN` ignores the statement if the column already exists when wrapped in a try-execute. Use `IF NOT EXISTS` (SQLite 3.37+) or catch the "duplicate column" error. Existing pattern in rc-core uses `CREATE TABLE IF NOT EXISTS`, so extend the pattern consistently.

#### New Rust modules required

| Module | Path | Responsibility |
|--------|------|---------------|
| `championship.rs` | `src/championship.rs` | F1-style scoring const array, compute_standings(), close_event_and_score() |
| `driver_rating.rs` | `src/driver_rating.rs` | compute_percentile_ratings(), schedule_nightly_recalculation() |

Both modules are pure domain logic with no external crate dependencies — just sqlx queries and arithmetic.

#### New public API routes

Extend `api/routes.rs` following the existing `/public/*` pattern (no auth required):

```rust
.route("/public/championships", get(public_championships))
.route("/public/championships/{id}", get(public_championship_detail))
.route("/public/championships/{id}/standings", get(public_championship_standings))
.route("/public/events", get(public_events))
.route("/public/events/{id}/results", get(public_event_results))
.route("/public/drivers/{id}/profile", get(public_driver_profile))
.route("/public/drivers/{id}/laps", get(public_driver_laps))
.route("/public/laps/compare", get(public_lap_compare))  // ?lap_a=UUID&lap_b=UUID
```

#### Cloud Sync Additions (cloud_sync.rs)

The existing `sync_push()` function sends laps, personal_bests, track_records, drivers to the VPS. For v3.0, extend with:

- Push `events` (hotlap event definitions, open/close status, scoring config)
- Push `event_entries` (results with points, badges, gaps)
- Push `championships` and `championship_standings`
- Push `driver_ratings` (class assignments)

No new Rust crates. Add additional `SELECT` queries and `reqwest::Client::post()` calls following the existing `sync_push()` pattern.

---

## Recommended Stack Summary (New Additions Only)

### Frontend (PWA — pwa/package.json)

| Library | Version | Purpose | Why |
|---------|---------|---------|-----|
| date-fns | 4.1.0 | Event/championship date formatting, relative time | Tree-shakeable ESM, server component safe, ~13KB |
| Canvas API | browser built-in | 2D track map (pos_x/pos_z polyline + car dot) | Zero bundle cost; 50 lines of ctx beats any library at this complexity |

**No new charting library.** recharts 3.8.0 is sufficient.

### Backend (rc-core — Cargo.toml)

| Addition | Type | Why |
|----------|------|-----|
| championship.rs | New Rust module | F1 points, standings accumulation, event close logic |
| driver_rating.rs | New Rust module | Percentile computation, nightly scheduled recalc |
| 4 new SQL tables (migrations) | Schema | Championships, rounds, standings, driver ratings |
| 4 ALTER TABLE statements (migrations) | Schema | Add scoring/badge columns to events, event_entries |
| 8 new public API routes | Extend routes.rs | Championships, events, driver profiles, lap comparison |
| cloud_sync.rs extensions | Extend existing module | Sync new tables to VPS |

**Zero new Rust crate dependencies.**

---

## Installation

```bash
# PWA — one new dependency
cd /c/Users/bono/racingpoint/racecontrol/pwa
npm install date-fns

# Backend — no new crates
# Cargo.toml unchanged
# New code goes in new .rs files + db/mod.rs migration additions
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| recharts (existing) for lap comparison | ECharts, Chart.js, Victory | Only if recharts hits a hard limitation with the two-dataset overlay (unlikely — ComposedChart + two Line sets is a documented pattern) |
| Canvas API for track map | D3.js SVG | If track map needs pan/zoom interactions, zoom-to-sector, or complex path operations. Not needed for v3.0's static outline + dot. |
| Canvas API for track map | react-konva | If track map becomes a full interactive widget with draggable elements. Not needed for v3.0. |
| Custom percentile rating | glicko2 npm (1.2.1) | If Racing Point adds real-time matchmaking between drivers in future. Glicko-2 would be appropriate for head-to-head races, not time trials. |
| Custom percentile rating | iRating-style Elo | Same objection — Elo requires win/loss outcomes. Inappropriate for time-trial data. |
| SQLite window functions | Application-level ranking | Never — pulling full tables into Rust to rank is both slower and more error-prone than a single SQL window function. |
| date-fns 4.1.0 | dayjs 1.11.x | dayjs is equally valid if it's already in the project. Neither is in the project currently, so date-fns wins on bundle size and tree-shaking behavior. |
| SQLite (existing) | PostgreSQL | Only if read concurrency > 100 concurrent requests or write throughput > 1,000 writes/second. Neither applies to an 8-pod venue. |
| SQLite (existing) | Redis for leaderboard caching | Only if leaderboard queries take > 100ms on cold reads. With proper indexes on (track, car, lap_time_ms), SQLite leaderboard queries complete in < 5ms. Redis adds a new service to manage on the constrained VPS. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| glicko2 / glicko2.ts npm packages | Designed for win/loss match outcomes; requires synthetic match construction for time-trial data; low maintenance activity (last published 2 years ago) | Custom percentile rating in Rust — simpler, more explainable, zero dependency |
| D3.js for track map | 80KB gzipped for feature equivalent to 50 lines of Canvas API code; SVG node-per-point approach is slow on mobile at 5,000+ sample counts | Canvas API via useRef in a plain React component |
| react-konva / Pixi.js | WebGL/canvas scene graph frameworks justified for interactive games, not for a static track map with one moving dot | Canvas API |
| Redis | New VPS service to manage; leaderboard at venue scale never needs caching layer; SQLite with WAL handles concurrent API + sync reads | SQLite + indexes |
| PostgreSQL | Full migration of existing SQLite schema; adds operational complexity; no benefit at 8-pod scale | SQLite — already in production |
| React Query / TanStack Query | Heavy addition (~30KB) for a codebase already using plain fetch() in useEffect; adds complexity without clear gain for v3.0's non-realtime public pages | Plain fetch + useEffect (existing pattern) |
| WebSockets for public leaderboard | Leaderboards update when laps complete — once per 5-20 minutes per driver, not per frame; poll-on-load is sufficient; WebSocket adds connection overhead on public unauthenticated pages | Page-load server fetch + optional manual refresh button |
| Separate microservice for competitive features | New deployment target on constrained VPS (2 vCPU, limited RAM); rc-core already handles all domain logic cleanly | Extend rc-core with championship.rs and driver_rating.rs modules |
| any TypeScript type in PWA | Project constraint; rc-core types flow via API response shapes; define proper interfaces for all new endpoints | Typed API response interfaces in lib/api.ts |

---

## Stack Patterns by Variant

**Lap comparison (two laps, synchronized charts):**
- Fetch both laps' telemetry via `GET /public/laps/compare?lap_a=X&lap_b=Y`
- Backend joins on offset_ms bins (round to nearest 100ms) to produce a merged array
- Each array entry: `{ offset_ms, a_speed, b_speed, a_throttle, b_throttle, delta_ms }`
- Frontend: two `<Line>` components per chart with distinct dataKeys; same `syncId` across all charts keeps hover crosshair synchronized
- Track map: draw lap_a outline in grey, lap_b in red (#E10600), two car dots

**Championship points calculation:**
```rust
// src/championship.rs
const F1_2025_POINTS: [i32; 10] = [25, 18, 15, 12, 10, 8, 6, 4, 2, 1];

fn assign_points(position: usize) -> i32 {
    F1_2025_POINTS.get(position.saturating_sub(1)).copied().unwrap_or(0)
}
```
- When a group event closes: rank `event_entries` by `result_time_ms ASC`, assign points, write to `championship_standings` via `INSERT OR REPLACE`
- Championship standings: `SUM(points) GROUP BY driver_id ORDER BY SUM(points) DESC` — no window function needed here

**Driver rating with percentile:**
```sql
-- In driver_rating.rs, run nightly
WITH driver_best AS (
    SELECT driver_id, track, car, MIN(lap_time_ms) as best_ms
    FROM laps WHERE valid = 1
    GROUP BY driver_id, track, car
),
ranked AS (
    SELECT
        driver_id,
        PERCENT_RANK() OVER (PARTITION BY track, car ORDER BY best_ms ASC) as pct_rank
    FROM driver_best
)
SELECT driver_id, AVG(pct_rank) * 100 as rating_score
FROM ranked
GROUP BY driver_id
```
Note: `PERCENT_RANK()` is a SQLite window function available from 3.25.0.

**Public pages with SSR (no login required):**
- Championship, event results, driver profile pages: use Next.js `async` server components that `fetch()` from rc-core `/public/*` endpoints at request time
- No `localStorage`, no JWT, no `"use client"` directive needed for the data fetch layer
- Chart components (recharts) remain `"use client"` as they use DOM/canvas APIs
- This gives good SEO (HTML content in initial response) and avoids hydration mismatches

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| recharts 3.8.0 | React 19.2.3, Next.js 16.1.6 | Already installed and working. React 19 peer dependency may show warning — use `--legacy-peer-deps` if reinstalling. |
| date-fns 4.1.0 | React 19, Next.js 16, TypeScript 5 | ESM-first, no peer dependency conflicts. |
| Canvas API | All mobile browsers in active use | Chrome 4+, Safari 9+, Firefox 2+. No polyfill needed. |
| SQLite window functions (PERCENT_RANK, ROW_NUMBER, SUM OVER) | sqlx 0.8 + bundled SQLite | Requires SQLite >= 3.25.0 (2018). sqlx-sqlite bundles a recent SQLite — this constraint is met. |
| sqlx 0.8 | Rust 1.93.1 (project version) | No conflict. Already in Cargo.lock. |

---

## Sources

- recharts 3.8.0 release history: https://github.com/recharts/recharts/releases (HIGH confidence — official GitHub)
- recharts React 19 support: https://github.com/recharts/recharts/issues/4558 (HIGH confidence — official issue tracker)
- glicko2 npm package: https://www.npmjs.com/package/glicko2 — v1.2.1, last published 2 years ago (HIGH confidence — npm registry)
- glicko2.ts TypeScript package: https://github.com/animafps/glicko2.ts — v1.3.2, low activity (MEDIUM confidence — GitHub)
- SQLite window functions official docs: https://www.sqlite.org/windowfunctions.html (HIGH confidence — official SQLite docs)
- sqlx window function nullable columns issue: https://github.com/launchbadge/sqlx/issues/2874 (HIGH confidence — official sqlx issue)
- date-fns 4.1.0: https://date-fns.org/ (HIGH confidence — official site)
- 107% rule definition: https://en.wikipedia.org/wiki/107%25_rule (MEDIUM confidence — Wikipedia)
- Existing codebase verified:
  - `pwa/package.json` — recharts 3.8.0, React 19.2.3, Next.js 16.1.6 confirmed
  - `pwa/src/components/TelemetryChart.tsx` — existing recharts AreaChart/LineChart/syncId pattern
  - `crates/rc-core/src/db/mod.rs` — schema: laps, telemetry_samples, events, event_entries, personal_bests, track_records
  - `crates/rc-core/src/api/routes.rs` — existing /public/* unauthenticated routes pattern
  - `crates/rc-core/Cargo.toml` — confirmed zero new crates needed (sqlx 0.8, tokio, serde_json, axum 0.8 all present)

---

*Stack research for: RaceControl v3.0 — Leaderboards, Telemetry & Competitive*
*Researched: 2026-03-14*
