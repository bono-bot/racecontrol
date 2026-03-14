# Phase 13: Leaderboard Core - Research

**Researched:** 2026-03-15
**Domain:** Public leaderboard UI, driver profiles, lap validity hardening, track-record email notifications — all served from cloud PWA over existing Rust/Axum + SQLite + Next.js stack
**Confidence:** HIGH — all findings grounded in direct codebase inspection of routes.rs, lap_tracker.rs, email_alerts.rs, public/page.tsx, and the completed Phase 12 summaries

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LB-01 | User can view public leaderboard for any track, filtered by car and sim_type, sorted by fastest valid lap time | Existing `/public/leaderboard/{track}` endpoint is a start; needs sim_type param + car filter + valid-only toggle |
| LB-02 | User can view circuit records page showing the all-time fastest lap per vehicle per circuit | `/public/leaderboard` already returns track_records; dedicated `/public/circuit-records` endpoint needed for full page |
| LB-03 | User can view vehicle records page showing the fastest lap per circuit for a given vehicle | New `/public/vehicle-records/{car}` endpoint; aggregate query over laps table |
| LB-04 | Leaderboard endpoints require sim_type filter — AC and F1 25 laps never mixed | Confirmed absence in current `/public/leaderboard/{track}` — must add mandatory sim_type filter |
| LB-05 | Lap validity hardened with sanity range check and sector-sum consistency | lap_tracker.rs currently trusts game valid flag only; needs `suspect` column + validation logic |
| LB-06 | Only valid laps appear on leaderboards by default; user can toggle to show invalid laps | Current queries use `WHERE valid = 1` — need showInvalid toggle in PWA |
| DRV-01 | User can search for any driver by name and view their public profile page (no login required) | No `/public/drivers` endpoint exists; need to add |
| DRV-02 | Driver profile shows stats cards: total laps, total time, personal bests per track/car, class badge | Existing `get_driver_full_profile` at `/drivers/{id}/full-profile` is auth-only staff route — need public equivalent |
| DRV-03 | Driver profile shows full lap history with circuit, vehicle, date, time, S1/S2/S3 sector times | Need paginated lap history query in public driver handler |
| DRV-04 | Driver profile accessible via shareable URL (e.g. /drivers/{id} or /drivers?name=X) | Need `/public/drivers` and `/public/drivers/{id}` routes + `/drivers/[id]` PWA page |
| NTF-01 | When a track record is beaten, previous record holder receives automated email via send_email.js | lap_tracker.rs detects new records but fires no notification; email_alerts.rs pattern exists (node send_email.js) |
| NTF-02 | Notification email includes track, car, old time, new time, new record holder name + leaderboard link | Data available at record-detection point in persist_lap(); need tokio::spawn + node invocation |
| PUB-01 | All leaderboard, records, and driver profile pages accessible without login | All `/public/*` routes are already unauthenticated in routes.rs; new routes must follow same pattern |
| PUB-02 | PWA pages are mobile-first with responsive tables/cards (minimum 14px for times, 16px for positions) | Existing public leaderboard page uses similar patterns; new pages must follow same mobile-first conventions |
</phase_requirements>

---

## Summary

Phase 13 builds the public customer-facing competitive surface on top of Phase 12's schema foundation. The database is ready: `idx_laps_leaderboard` (track, car, valid, lap_time_ms), `idx_laps_car_class`, `track_records`, `personal_bests`, all exist and are indexed. The gap is entirely in the API layer and PWA layer — plus two critical hardening items (lap validity and email notification) that have no implementation today.

The existing codebase has a strong head start. `/public/leaderboard` and `/public/leaderboard/{track}` exist and return correct data, but both are missing `sim_type` filtering (Pitfall 5 from research — cross-game contamination). The existing `email_alerts.rs` module already uses `tokio::process::Command::new("node").arg(&self.script_path)` to fire `send_email.js` — the "beaten" notification hooks directly into this pattern with no new infrastructure. Driver profiles exist as a staff-only route (`/drivers/{id}/full-profile`) and need a public, privacy-safe equivalent.

The phase scope is deliberately self-contained: all work is on the cloud read path (no event logic, no championships, no telemetry sync). Every new feature draws from existing `laps`, `personal_bests`, `track_records`, and `drivers` tables that are already populated and indexed.

**Primary recommendation:** Build in three layers — (1) Rust API hardening + new endpoints, (2) email notification in lap_tracker.rs, (3) PWA pages. Each layer is independently testable. Do not ship the public PWA pages before the `sim_type` filter is enforced — that is the highest-severity correctness issue in the current code.

---

## Standard Stack

### Core (all already in place from Phase 12)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| Rust/Axum | 0.8 | HTTP API endpoints | Zero new crates needed |
| sqlx | 0.8 | Async SQLite queries | WAL-tuned pool from Phase 12 |
| serde_json | (in Cargo.toml) | JSON response shaping | Existing `json!()` macro pattern |
| SQLite | 3.25+ | Storage (with window functions) | idx_laps_leaderboard already in place |
| Next.js App Router | (in pwa/package.json) | PWA pages | Existing `"use client"` pattern |
| React 19 | (in pwa/package.json) | UI components | Existing hooks pattern |
| tokio::process::Command | tokio 1.x | Fire send_email.js for notifications | Already used in email_alerts.rs |

### No New Dependencies Required

Phase 13 adds zero new Rust crates and zero new npm packages. All functionality is achievable with existing infrastructure. The email notification reuses `tokio::process::Command::new("node")` exactly as `email_alerts.rs` does today.

---

## Architecture Patterns

### Pattern 1: Public Route Registration (Existing — Follow Exactly)

All public endpoints live in routes.rs under the `/public/` prefix with no auth middleware. The current set is lines 247-251:

```rust
// Source: crates/rc-core/src/api/routes.rs lines 247-251
.route("/public/leaderboard", get(public_leaderboard))
.route("/public/leaderboard/{track}", get(public_track_leaderboard))
.route("/public/time-trial", get(public_time_trial))
.route("/public/laps/{lap_id}/telemetry", get(public_lap_telemetry))
.route("/public/sessions/{id}", get(public_session_summary))
```

New Phase 13 routes follow this exact pattern:

```rust
// Phase 13 additions — same section in routes.rs
.route("/public/leaderboard/{track}", get(public_track_leaderboard))  // MODIFY: add sim_type param
.route("/public/circuit-records", get(public_circuit_records))         // NEW
.route("/public/vehicle-records/{car}", get(public_vehicle_records))   // NEW
.route("/public/drivers", get(public_drivers_search))                  // NEW
.route("/public/drivers/{id}", get(public_driver_profile))             // NEW
```

### Pattern 2: sim_type as Required Query Param

The existing `/public/leaderboard/{track}` has no `sim_type` filter. This must be added as a mandatory query parameter using Axum's `Query` extractor. Requests without it should default to `assetto_corsa` (the primary sim at the venue) rather than returning a 400 — this keeps backward compatibility for the existing PWA page.

```rust
// Source: Axum 0.8 Query extraction pattern — existing in routes.rs throughout
#[derive(serde::Deserialize)]
struct LeaderboardQuery {
    sim_type: Option<String>,  // defaults to "assetto_corsa" if absent
    car: Option<String>,
    show_invalid: Option<bool>,
}

async fn public_track_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(track): Path<String>,
    Query(params): Query<LeaderboardQuery>,
) -> Json<Value> {
    let sim = params.sim_type.as_deref().unwrap_or("assetto_corsa");
    // ...WHERE l.track = ? AND l.sim_type = ? AND l.valid = ...
}
```

### Pattern 3: Lap Validity Hardening — suspect Column

`lap_tracker.rs` currently trusts the game's `valid` flag directly. Two hardening checks must be added before the lap is persisted:

1. **Sector sum consistency:** If all three sector times are present and non-zero, verify `sector1_ms + sector2_ms + sector3_ms` is within ±500ms of `lap_time_ms`. If not, set `suspect = true`.
2. **Sanity range:** Reject laps with `lap_time_ms < 30_000` (30 seconds) as physically impossible for any track+car at this venue. This is a configurable floor, not a per-track minimum.

The `laps` table needs a `suspect` boolean column (added via `ALTER TABLE laps ADD COLUMN suspect INTEGER NOT NULL DEFAULT 0` in db/mod.rs). All existing laps default to suspect=0 (clean). New laps get the check applied at persist time.

```rust
// In persist_lap() — after validity check, before INSERT
let sector_sum_ok = match (lap.sector1_ms, lap.sector2_ms, lap.sector3_ms) {
    (Some(s1), Some(s2), Some(s3)) if s1 > 0 && s2 > 0 && s3 > 0 => {
        let sum = (s1 + s2 + s3) as i64;
        let diff = (sum - lap.lap_time_ms as i64).abs();
        diff <= 500
    }
    _ => true, // sectors unavailable — do not flag
};
let sanity_ok = lap.lap_time_ms >= 30_000;
let suspect = !sector_sum_ok || !sanity_ok;
```

Leaderboard queries then use `WHERE valid = 1 AND suspect = 0` by default, with the `show_invalid` toggle exposing `valid = 0` (but never `suspect = 1` without explicit staff override).

### Pattern 4: Track Record Email Notification

`lap_tracker.rs` `persist_lap()` already returns `is_record: bool` and logs `"NEW TRACK RECORD"`. The notification hook goes here — after the `track_records` UPSERT, if `is_record` was just set:

```rust
// In persist_lap() — after track_records UPSERT, within `if is_record` block
// Fetch previous record holder's email to notify them
// (previous holder's driver_id was captured before the UPSERT)

// Fire-and-forget: tokio::spawn + node send_email.js
// Mirrors email_alerts.rs: tokio::process::Command::new("node")
//   .arg(&state.config.email_script_path)  // "send_email.js"
//   .arg(&prev_holder_email)
//   .arg("Your track record has been broken!")
//   .arg(&email_body)
```

Critical detail: the previous record holder's email must be fetched BEFORE the UPSERT overwrites `driver_id`. Restructure the track record section to:
1. Fetch current record row (driver_id, best_lap_ms, driver email) in one query
2. Execute UPSERT
3. If new record: spawn notification task using fetched previous holder data

This avoids any race condition between reading and writing.

### Pattern 5: Public Driver Profile — Privacy Rules

The existing `get_driver_full_profile` returns full PII (email, phone, wallet balance). The public equivalent must return only display-safe fields:

- Include: driver name (respecting `show_nickname_on_leaderboard`), total_laps, total_time_ms, personal bests (track, car, best_lap_ms, achieved_at), lap history (track, car, lap_time_ms, sector times, created_at)
- Exclude: email, phone, wallet balance, billing history, waiver data, internal IDs beyond what's needed for URL construction

Driver search (`/public/drivers?name=X`) uses a `LIKE '%name%'` query on `drivers.name` with a LIMIT 20 cap. Driver profile by ID (`/public/drivers/{id}`) fetches the full public profile in one round trip.

### Pattern 6: PWA "use client" Page Pattern (Existing — Follow Exactly)

All new PWA pages follow the existing `public/page.tsx` pattern:

```tsx
"use client";
import { useEffect, useState } from "react";
import { publicApi } from "@/lib/api";

export default function DriversPage() {
  const [data, setData] = useState(null);
  useEffect(() => { publicApi.someCall().then(setData); }, []);
  // render
}
```

No SSR, no ISR, no revalidateTag needed for Phase 13 — the cloud PWA is a client-rendered SPA polling the cloud API. The "last updated" timestamp pattern is sufficient for freshness signaling.

### Recommended Project Structure (Phase 13 additions)

```
crates/rc-core/src/
├── lap_tracker.rs          MODIFY: add suspect column logic + track record notification
├── db/mod.rs               MODIFY: ADD COLUMN suspect INTEGER to laps
├── api/routes.rs           MODIFY: add 4 new public routes + extend existing 2

pwa/src/
├── app/
│   ├── leaderboard/public/page.tsx   MODIFY: add sim_type filter UI + showInvalid toggle
│   ├── drivers/
│   │   ├── page.tsx                  NEW: driver search page
│   │   └── [id]/page.tsx            NEW: driver public profile page
│   └── records/
│       ├── circuit/page.tsx          NEW: circuit records page
│       └── vehicle/[car]/page.tsx    NEW: vehicle records page (optional — can be tab on circuit records)
├── lib/api.ts              MODIFY: add publicApi.circuitRecords, .vehicleRecords, .drivers, .driverProfile
└── components/
    └── LapTimeDisplay.tsx  NEW (optional): shared formatter to avoid duplication
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Email sending | Custom SMTP client, custom queue | `tokio::process::Command::new("node").arg("send_email.js")` as in email_alerts.rs | Already authenticated, rate-limited, and battle-tested at venue |
| Lap time formatting | Custom formatter per page | Existing `format!("{}:{:02}.{:03}", ms/60000, (ms%60000)/1000, ms%1000)` pattern | Identical in routes.rs and public/page.tsx — extract to shared util if needed |
| Public auth bypass | Custom middleware or per-route auth skip | Follow existing `/public/*` route pattern — no auth middleware on those routes | Routes.rs already handles this correctly |
| Full-text driver search | PostgreSQL FTS, Elasticsearch | `WHERE name LIKE ? LIMIT 20` with `%name%` | At venue scale (<5,000 drivers), SQLite LIKE is sub-millisecond |
| Async email queue | Redis, RabbitMQ, Tokio channel | `tokio::spawn` fire-and-forget | Notification is best-effort; if it fails, the record still stands |

---

## Common Pitfalls

### Pitfall 1: sim_type Not Filtered (CRITICAL — Already Present in Codebase)

**What goes wrong:** The existing `/public/leaderboard/{track}` (line 8213-8260 of routes.rs) has NO `sim_type` filter. If AC and F1 25 both have laps for `silverstone`/`ks_silverstone`, they are mixed. The query returns `GROUP BY l.driver_id, l.car ORDER BY MIN(l.lap_time_ms) ASC` — AC GT3 times and F1 25 times will appear on the same board.

**Prevention:** Add `sim_type` to ALL leaderboard queries before shipping. This is the highest-priority fix in Phase 13 because it affects existing functionality, not just new routes.

**Warning signs:** `EXPLAIN QUERY PLAN` shows query without `sim_type` predicate. A driver who only drove F1 25 appears on the Assetto Corsa leaderboard.

### Pitfall 2: Previous Record Holder Email Fetched After UPSERT

**What goes wrong:** `persist_lap()` does `INSERT INTO track_records ... ON CONFLICT DO UPDATE` which overwrites `driver_id` with the new record holder immediately. If you query the previous holder after the UPSERT, you get the new holder's data (yourself), and the "beaten" email is sent to the new record holder — congratulating themselves.

**Prevention:** Fetch the existing record row (including `drivers.email`) in a `SELECT` BEFORE the `track_records` UPSERT. Store in a local variable. The UPSERT runs. If `is_record`, use the pre-fetched email to notify.

### Pitfall 3: suspect Column Missing from Leaderboard Queries on Existing Endpoints

**What goes wrong:** The `suspect` column is added to `laps` via `ALTER TABLE`. New laps get the check. But the existing `/public/leaderboard/{track}` query uses `WHERE l.valid = 1` with no `suspect` filter. Suspect laps (sector-sum mismatch) are valid per the game but should be hidden from the public board.

**Prevention:** All leaderboard queries must be updated to `WHERE valid = 1 AND (suspect IS NULL OR suspect = 0)`. The `IS NULL OR suspect = 0` handles pre-migration rows where suspect is NULL (treated as clean). All new laps have explicit 0 or 1.

### Pitfall 4: Driver Profile PII Leak

**What goes wrong:** Copy-pasting `get_driver_full_profile` for the public endpoint and forgetting to strip email, phone, and wallet fields. The public endpoint would expose PII to unauthenticated callers.

**Prevention:** Write a separate `public_driver_profile` handler that explicitly selects only safe fields. Do not call the staff handler and filter the result — select only what is needed.

### Pitfall 5: Mobile Table Overflow

**What goes wrong:** Leaderboard tables with columns for position, driver, car, time, sectors, date are 7+ columns. On a 375px-wide phone screen, unformatted tables overflow horizontally or squish text below 12px.

**Prevention:** Use card layout on mobile (`@media (max-width: 640px)` — or Tailwind `sm:` breakpoints). Each leaderboard entry is a card showing position + driver on one row, car + time on next row. Table layout only on desktop. Font size minimum: 14px for times, 16px for positions (per PUB-02).

### Pitfall 6: sector Time "0" vs NULL Display

**What goes wrong:** AC laps frequently have `sector1_ms = 0` in the DB (not NULL) when sectors were not transmitted. Displaying `0:00.000` for a sector time is confusing. Customers assume the car stopped.

**Prevention:** In both the API response and the PWA: treat `sector_ms <= 0` as absent. Return `null` from the API (not `0`). Display `"–"` in the PWA for null sector times. This is a display rule, not a data rule — the DB stores whatever the game sent.

---

## Code Examples

### Leaderboard Query with sim_type and suspect Filter

```rust
// Source: extend existing public_track_leaderboard in routes.rs
let show_invalid = params.show_invalid.unwrap_or(false);
let valid_clause = if show_invalid { "1=1" } else { "l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)" };

let query = format!(
    "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
            THEN d.nickname ELSE d.name END,
            l.car, l.driver_id, MIN(l.lap_time_ms), MAX(l.created_at),
            (SELECT l2.id FROM laps l2
             WHERE l2.driver_id = l.driver_id AND l2.car = l.car AND l2.track = l.track
               AND l2.sim_type = ? AND l2.valid = 1
             ORDER BY l2.lap_time_ms ASC LIMIT 1)
     FROM laps l
     JOIN drivers d ON l.driver_id = d.id
     WHERE l.track = ? AND l.sim_type = ? AND {}
     GROUP BY l.driver_id, l.car
     ORDER BY MIN(l.lap_time_ms) ASC
     LIMIT 50",
    valid_clause
);
```

### Circuit Records Query (New Endpoint)

```rust
// /public/circuit-records — all-time fastest per (track, car, sim_type)
// Source: aggregate query pattern from existing track_records JOIN
sqlx::query_as::<_, (String, String, String, i64, String, String)>(
    "SELECT tr.track, tr.car, tr.sim_type,
            tr.best_lap_ms,
            CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                 THEN d.nickname ELSE d.name END,
            tr.achieved_at
     FROM track_records tr
     JOIN drivers d ON tr.driver_id = d.id
     ORDER BY tr.track ASC, tr.best_lap_ms ASC"
)
// Note: track_records table has no sim_type column yet — need ALTER TABLE
// OR query from laps instead: SELECT track, car, sim_type, MIN(lap_time_ms)...
```

**Important:** The `track_records` table (created in Phase 12) does not have a `sim_type` column. The circuit records query for Phase 13 should use the `laps` table with `GROUP BY track, car, sim_type` and the covering index `idx_laps_leaderboard`, or add `sim_type` to `track_records` via `ALTER TABLE`.

The simplest correct approach: query from `laps` directly for circuit records (bypass `track_records` which lacks sim_type):

```sql
SELECT track, car, sim_type,
       MIN(lap_time_ms) as best_lap_ms,
       driver_id
FROM laps
WHERE valid = 1 AND (suspect IS NULL OR suspect = 0)
GROUP BY track, car, sim_type
ORDER BY track, car, sim_type
```

Join this with `drivers` for driver name. The `idx_laps_leaderboard` covering index on `(track, car, valid, lap_time_ms)` makes this fast.

### Track Record Notification in lap_tracker.rs

```rust
// In persist_lap() — restructure the track record section:

// STEP 1: Fetch current record holder BEFORE upsert
let prev_record: Option<(i64, String, Option<String>)> = sqlx::query_as(
    "SELECT tr.best_lap_ms, d.name, d.email
     FROM track_records tr
     JOIN drivers d ON tr.driver_id = d.id
     WHERE tr.track = ? AND tr.car = ?"
)
.bind(&lap.track)
.bind(&lap.car)
.fetch_optional(&state.db)
.await
.ok()
.flatten();

let is_record = match &prev_record {
    Some((current_ms, _, _)) => (lap.lap_time_ms as i64) < *current_ms,
    None => true,
};

if is_record {
    // STEP 2: Upsert the new record
    let _ = sqlx::query(
        "INSERT INTO track_records (track, car, driver_id, best_lap_ms, lap_id, achieved_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(track, car) DO UPDATE SET
            driver_id = excluded.driver_id,
            best_lap_ms = excluded.best_lap_ms,
            lap_id = excluded.lap_id,
            achieved_at = excluded.achieved_at"
    )
    .bind(&lap.track).bind(&lap.car)
    .bind(&lap.driver_id).bind(lap.lap_time_ms as i64)
    .bind(&lap.id)
    .execute(&state.db).await;

    // STEP 3: Notify previous holder (fire-and-forget)
    if let Some((prev_ms, prev_name, Some(prev_email))) = prev_record {
        let new_time_display = format_lap_ms(lap.lap_time_ms as i64);
        let old_time_display = format_lap_ms(prev_ms);
        let track = lap.track.clone();
        let car = lap.car.clone();
        let script = state.config.email_script_path.clone();
        tokio::spawn(async move {
            let subject = format!("Your {} record at {} has been beaten!", car, track);
            let body = format!(
                "Hi {},\n\nYour track record at {} in the {} has been broken.\n\nOld time: {}\nNew time: {}\n\nCome back and reclaim it!\n\nhttps://app.racingpoint.cloud/leaderboard/public",
                prev_name, track, car, old_time_display, new_time_display
            );
            let _ = tokio::process::Command::new("node")
                .arg(&script)
                .arg(&prev_email)
                .arg(&subject)
                .arg(&body)
                .kill_on_drop(true)
                .output()
                .await;
        });
    }
}
```

### Public Driver Profile Query

```rust
// /public/drivers/{id} — safe public fields only
async fn public_driver_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Basic stats
    let driver = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL
                THEN nickname ELSE name END,
                id, total_laps, total_time_ms
         FROM drivers WHERE id = ?"
    ).bind(&id).fetch_optional(&state.db).await;

    // Personal bests
    let pbs = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT track, car, best_lap_ms, achieved_at
         FROM personal_bests WHERE driver_id = ?
         ORDER BY achieved_at DESC"
    ).bind(&id).fetch_all(&state.db).await;

    // Recent lap history (last 100)
    let laps = sqlx::query_as::<_, (String, String, i64, Option<i64>, Option<i64>, Option<i64>, i32, String)>(
        "SELECT track, car, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at
         FROM laps
         WHERE driver_id = ? AND (suspect IS NULL OR suspect = 0)
         ORDER BY created_at DESC LIMIT 100"
    ).bind(&id).fetch_all(&state.db).await;
    // ... shape and return
}
```

### PWA Driver Search Page Structure

```tsx
// pwa/src/app/drivers/page.tsx
"use client";
import { useState } from "react";
import { publicApi } from "@/lib/api";

export default function DriversPage() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState([]);

  const search = async () => {
    const data = await publicApi.searchDrivers(query);
    setResults(data.drivers);
  };

  return (
    <main>
      <input value={query} onChange={e => setQuery(e.target.value)}
             placeholder="Search by driver name..." />
      <button onClick={search}>Search</button>
      {results.map(d => (
        <a key={d.id} href={`/drivers/${d.id}`}>{d.name}</a>
      ))}
    </main>
  );
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Trust game valid flag only | trust flag + suspect column (sector sum + sanity range) | Phase 13 | Prevents leaderboard gaming without full manual review |
| All laps mixed regardless of sim | sim_type filter mandatory on all leaderboard endpoints | Phase 13 | Prevents AC/F1 time mixing on same board |
| No public driver profile | `/public/drivers/{id}` with safe PII fields only | Phase 13 | Enables shareable URLs and social loop |
| Track record set silently | Record set + previous holder notified by email | Phase 13 | Core repeat-visit engagement mechanic |
| Records served from track_records table | Records served from laps GROUP BY (to include sim_type) | Phase 13 | track_records table has no sim_type; laps query is correct |

---

## Open Questions

1. **track_records table lacks sim_type column**
   - What we know: `track_records` was created in Phase 12 without sim_type. It stores one record per `(track, car)` — ambiguous when both AC and F1 25 have laps on the same track name.
   - What's unclear: Should we ALTER TABLE track_records ADD COLUMN sim_type and backfill, or bypass track_records for circuit-records queries and use laps directly?
   - Recommendation: Use laps directly for all Phase 13 leaderboard queries (bypass track_records). The covering index makes it fast. Defer track_records refactor to Phase 14 if needed. The track_records table continues to be used for personal_bests notification trigger only.

2. **Driver email availability for notifications**
   - What we know: `drivers.email` column exists. Email is captured during registration at the kiosk (when a driver creates their profile). Not all drivers will have emails (walk-in cash customers may skip email).
   - What's unclear: What fraction of existing drivers have emails populated?
   - Recommendation: Notification is best-effort. If `prev_holder.email IS NULL`, skip the notification silently. Log at DEBUG level. Do not fail the lap persistence because a notification could not be sent.

3. **Lap time sanity floor**
   - What we know: The 30-second floor is proposed. Some very short tracks (e.g., karting-style) might have genuine laps under 30 seconds.
   - What's unclear: Does Racing Point's track catalog include any sub-30-second tracks?
   - Recommendation: Use 20,000ms (20 seconds) as the floor — physically impossible for any car on any circuit in the AC/F1 catalog. Flag at warn! level for anything under 60,000ms for staff awareness.

4. **Vehicle records page scope**
   - What we know: LB-03 says "fastest lap per circuit for a given vehicle." The `/public/vehicle-records/{car}` endpoint needs a car identifier in the URL.
   - What's unclear: Car names in the DB are AC internal strings (e.g., `ks_ferrari_sf70h`) not display names. Does the PWA need a car selector or should vehicle records be a tab on the circuit records page with a filter dropdown?
   - Recommendation: Implement as a tab on the circuit records page with a car selector dropdown (populated from `SELECT DISTINCT car FROM laps WHERE valid=1`). Simpler URL structure, same data.

---

## Validation Architecture

nyquist_validation is enabled (config.json).

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[tokio::test]` (in-memory SQLite) — in `crates/rc-core/tests/integration.rs` |
| Config file | Cargo.toml (test in `[[test]]` — auto-discovered) |
| Quick run command | `cargo test -p rc-core test_leaderboard` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LB-01 | Leaderboard query returns fastest valid lap per driver/car for a track, filtered by sim_type | unit | `cargo test -p rc-core test_leaderboard_sim_type_filter` | Wave 0 |
| LB-02 | Circuit records query returns all-time best per (track, car, sim_type) | unit | `cargo test -p rc-core test_circuit_records` | Wave 0 |
| LB-03 | Vehicle records query returns best per track for a given car | unit | `cargo test -p rc-core test_vehicle_records` | Wave 0 |
| LB-04 | Leaderboard with two sims on same track returns only the requested sim_type | unit | `cargo test -p rc-core test_leaderboard_no_cross_sim` | Wave 0 |
| LB-05 | Lap with sector sum mismatch >500ms gets suspect=1; lap <20s gets suspect=1 | unit | `cargo test -p rc-core test_lap_suspect_sector_sum` `cargo test -p rc-core test_lap_suspect_sanity` | Wave 0 |
| LB-06 | Suspect laps excluded from default leaderboard; show_invalid=true includes valid=0 (not suspect) | unit | `cargo test -p rc-core test_leaderboard_invalid_toggle` | Wave 0 |
| DRV-01 | Driver search by name returns matching drivers (case-insensitive LIKE) | unit | `cargo test -p rc-core test_driver_search` | Wave 0 |
| DRV-02 | Public driver profile returns stats without PII (no email, no phone, no wallet) | unit | `cargo test -p rc-core test_public_driver_no_pii` | Wave 0 |
| DRV-03 | Driver lap history returns sector times as null (not 0) when sectors=0 in DB | unit | `cargo test -p rc-core test_driver_lap_history_null_sectors` | Wave 0 |
| DRV-04 | Public driver endpoint accessible without auth token | integration (HTTP) | Manual smoke test — curl with no Authorization header | N/A |
| NTF-01 | persist_lap returns is_record=true when new time beats existing track record | unit | `cargo test -p rc-core test_track_record_detected` | ✅ (existing test covers partially) |
| NTF-02 | Previous record holder data fetched before UPSERT (new holder data not returned) | unit | `cargo test -p rc-core test_notification_data_before_upsert` | Wave 0 |
| PUB-01 | All /public/* routes return 200 without Authorization header | integration (HTTP) | Manual smoke test | N/A |
| PUB-02 | Mobile layout — tested visually | manual | Browser DevTools mobile emulation at 375px | N/A |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-core 2>&1 | tail -5`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/rc-core/tests/integration.rs` — add `test_leaderboard_sim_type_filter` covering LB-01, LB-04
- [ ] `crates/rc-core/tests/integration.rs` — add `test_lap_suspect_sector_sum` and `test_lap_suspect_sanity` covering LB-05
- [ ] `crates/rc-core/tests/integration.rs` — add `test_leaderboard_invalid_toggle` covering LB-06
- [ ] `crates/rc-core/tests/integration.rs` — add `test_circuit_records` covering LB-02
- [ ] `crates/rc-core/tests/integration.rs` — add `test_vehicle_records` covering LB-03
- [ ] `crates/rc-core/tests/integration.rs` — add `test_driver_search` covering DRV-01
- [ ] `crates/rc-core/tests/integration.rs` — add `test_public_driver_no_pii` covering DRV-02
- [ ] `crates/rc-core/tests/integration.rs` — add `test_notification_data_before_upsert` covering NTF-02
- [ ] `crates/rc-core/db/mod.rs` — `suspect` column must be in `run_test_migrations()` (mirroring production)

Existing `test_leaderboard_ordering` (line 1108) partially covers LB-01 but does not test sim_type or suspect. It will be updated rather than replaced.

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)

- `crates/rc-core/src/api/routes.rs` lines 247-251, 8137-8320 — existing public endpoint implementations confirmed; sim_type absence confirmed
- `crates/rc-core/src/lap_tracker.rs` lines 1-166 — full persist_lap() flow read; track_records UPSERT pattern confirmed; no notification hook confirmed
- `crates/rc-core/src/email_alerts.rs` lines 1-158 — `tokio::process::Command::new("node")` pattern confirmed; send_email.js integration confirmed
- `pwa/src/app/leaderboard/public/page.tsx` lines 1-60+ — existing public PWA page structure confirmed; "use client" pattern confirmed
- `pwa/src/lib/api.ts` lines 926-941 — existing `publicApi` functions confirmed; gaps identified
- `pwa/src/components/` — only 3 components exist (BottomNav, SessionCard, TelemetryChart); no shared lap time formatter
- `.planning/phases/12-data-foundation/12-01-SUMMARY.md` — confirmed: idx_laps_leaderboard (track, car, valid, lap_time_ms) exists; track_records has no sim_type column
- `.planning/phases/12-data-foundation/12-02-SUMMARY.md` — confirmed: laps.car_class exists; kiosk_experiences in test migrations; NULL sentinel for pre-v3.0 laps
- `crates/rc-core/tests/integration.rs` — 20 integration tests, test infrastructure confirmed; run_test_migrations() must mirror production schema

### Secondary (MEDIUM confidence — domain research from prior research phase)

- `.planning/research/PITFALLS.md` — Pitfall 4 (valid flag trust), Pitfall 5 (cross-game), Pitfall 11 (cache staleness) directly relevant to Phase 13
- `.planning/research/ARCHITECTURE.md` — public driver profile pattern, email notification flow, leaderboard anti-patterns documented
- `.planning/research/FEATURES.md` — lap validity display (LB-06), mobile-first requirements (PUB-02), no-login requirement (PUB-01) confirmed by competitor analysis

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new dependencies; all patterns confirmed in existing codebase
- Architecture: HIGH — direct code reading of all files being modified; no guessing
- Pitfalls: HIGH — most critical pitfalls (sim_type gap, notification ordering) confirmed by direct code inspection of routes.rs and lap_tracker.rs
- Notification email: HIGH — email_alerts.rs pattern confirmed; applies directly

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (stable stack; no fast-moving dependencies)

**Key finding not in prior research:** The `track_records` table has no `sim_type` column — circuit records queries must use the `laps` table with `GROUP BY track, car, sim_type` instead. The existing covering index supports this query efficiently. This is not a blocker but is a deviation from the architecture doc which assumed `track_records` was the source for circuit records.
