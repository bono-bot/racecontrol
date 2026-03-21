# Phase 90: Customer Progression - Research

**Researched:** 2026-03-21
**Domain:** Rust (Axum) backend API endpoints, SQLite backfill logic, Next.js 16 PWA pages (customer-facing)
**Confidence:** HIGH

## Summary

Phase 90 builds on the psychology foundation (Phase 89) to deliver the customer-facing driving passport and badge showcase. The `driving_passport` table already exists (7 columns, UNIQUE constraint on driver_id+track+car) but is currently empty -- nothing inserts into it. The `achievements` table has 5 seed badges, and `evaluate_badges()` + `update_streak()` already fire on every session end via `post_session_hooks()`. The phase has three distinct work streams: (1) backend -- wire `driving_passport` upserts into `persist_lap()`, create a backfill function for existing laps, and add customer-facing API endpoints; (2) frontend -- build a `/passport` PWA page with tiered collection display and a badge section on the `/profile` page; (3) data -- ensure backfill runs on first load (lazy) or server start.

The customer-facing PWA lives at `/root/racecontrol/pwa/` (Next.js 16, port 3500 in production). It uses `fetchApi()` against the RaceControl API at `/api/v1/customer/*` with JWT auth. The existing `BottomNav` has 7 tabs (Home, Live, Sessions, Race, Friends, Stats, Profile). The passport page will be a new route accessible from the dashboard or profile. The AC catalog has 36 featured tracks (50 total) and 41 featured cars (325 total) -- the tiered collection system uses THESE featured counts to define Starter (6 items), Explorer (15 items), and Legend (36+ items) tiers.

**Primary recommendation:** Add a `update_driving_passport()` function in `psychology.rs` called from `persist_lap()` in `lap_tracker.rs`. Add a one-time `backfill_driving_passport()` function called from server startup. Create 3 new customer API endpoints (`/customer/passport`, `/customer/badges`, `/customer/passport/backfill`) in `routes.rs`. Build 1 new PWA page (`/passport`) and extend the existing `/profile` page with a badge section.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PROG-01 | Customer can view their driving passport showing track/car completion progress in PWA | New `/customer/passport` API endpoint returns track/car collections with completion counts; new `/passport` PWA page renders it |
| PROG-02 | Driving passport uses tiered collections (Starter 6 items, Explorer 15 items, Legend 36+ items) to prevent Zeigarnik backfire | API response groups tracks/cars into tiers based on featured catalog; PWA renders tiers with progressive disclosure |
| PROG-03 | System awards badges for milestones (first lap, 10 tracks, 100 laps, PB streak, etc.) | Already implemented in Phase 89: `evaluate_badges()` fires on every session end, 5 seed badges exist. Phase 90 adds customer-facing display |
| PROG-04 | Existing lap data is backfilled into passport on first load | `backfill_driving_passport()` function aggregates laps table grouped by driver_id+track+car and upserts into driving_passport; triggered lazily on first `/customer/passport` API call |
| PROG-05 | Customer can see badge showcase on their profile page | New `/customer/badges` API endpoint (customer-scoped, not staff-scoped); badge grid section added to existing `/profile` PWA page |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8 | SQLite queries for passport/badge APIs and backfill | Already in workspace Cargo.toml |
| serde / serde_json | workspace | JSON serialization for API responses | Already used throughout |
| axum | 0.8 | HTTP handler registration | Already used throughout |
| chrono | workspace | Timestamps for first_driven_at formatting | Already used throughout |
| Next.js | 16.1.6 | PWA page framework | Already in pwa/package.json |
| React | 19.2.3 | UI components | Already in pwa/package.json |
| Tailwind CSS | 4 | Styling | Already in pwa/package.json |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | workspace | ID generation for backfill inserts | Already used for all INSERT operations |
| tracing | workspace | Structured logging for backfill progress | All backend operations |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Lazy backfill on first API call | Eager backfill on server startup | Lazy avoids startup delay and handles new drivers seamlessly; eager would block boot for potentially thousands of laps. Lazy is correct. |
| Separate /passport page | Embed in existing /stats page | Passport is distinct from stats -- own page gives proper room for tiered grid display and future expansion |
| Customer-scoped /customer/badges endpoint | Reuse staff /psychology/badges/{driver_id} | Customer endpoint uses JWT to extract driver_id, does not expose other drivers' data. Staff endpoint stays separate. |

**Installation:**
No new dependencies needed. All libraries already in workspace.

## Architecture Patterns

### Recommended Project Structure
```
crates/racecontrol/src/
    psychology.rs          # MODIFIED: add update_driving_passport(), backfill_driving_passport()
    lap_tracker.rs         # MODIFIED: call update_driving_passport() after lap INSERT
    api/routes.rs          # MODIFIED: add 3 customer endpoints
    catalog.rs             # READ-ONLY: reference FEATURED_TRACKS/FEATURED_CARS for tier definitions

pwa/src/
    app/passport/page.tsx  # NEW: driving passport page with tiered collections
    app/profile/page.tsx   # MODIFIED: add badge showcase section
    lib/api.ts             # MODIFIED: add passport() and badges() API methods
    components/BottomNav.tsx # OPTIONALLY modified: add Passport link (or link from dashboard/profile)
```

### Pattern 1: Driving Passport Upsert (in persist_lap)
**What:** After every valid lap INSERT, upsert into driving_passport with best_lap_ms and lap_count
**When to use:** Every call to persist_lap() that succeeds
**Example:**
```rust
// In psychology.rs — called from persist_lap after lap INSERT
pub async fn update_driving_passport(
    state: &Arc<AppState>,
    driver_id: &str,
    track: &str,
    car: &str,
    lap_time_ms: i64,
) {
    let id = uuid::Uuid::new_v4().to_string();
    if let Err(e) = sqlx::query(
        "INSERT INTO driving_passport (id, driver_id, track, car, best_lap_ms, lap_count)
         VALUES (?, ?, ?, ?, ?, 1)
         ON CONFLICT(driver_id, track, car) DO UPDATE SET
           lap_count = driving_passport.lap_count + 1,
           best_lap_ms = CASE WHEN excluded.best_lap_ms < driving_passport.best_lap_ms
                         THEN excluded.best_lap_ms ELSE driving_passport.best_lap_ms END"
    )
    .bind(&id)
    .bind(driver_id)
    .bind(track)
    .bind(car)
    .bind(lap_time_ms)
    .execute(&state.db)
    .await {
        tracing::error!("[psychology] driving_passport upsert failed: {}", e);
    }
}
```

### Pattern 2: Lazy Backfill on First API Call
**What:** When a customer's driving_passport is empty but they have laps, backfill from laps table
**When to use:** Inside the /customer/passport endpoint handler
**Example:**
```rust
// Check if driver has passport entries
let passport_count: i64 = sqlx::query_scalar(
    "SELECT COUNT(*) FROM driving_passport WHERE driver_id = ?"
).bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);

// If empty but driver has laps, backfill
if passport_count == 0 {
    backfill_driving_passport(&state, &driver_id).await;
}

// Then return passport data normally
```

### Pattern 3: Customer API Endpoint (JWT extraction in-handler)
**What:** Customer-scoped endpoints that extract driver_id from JWT, not from URL path
**When to use:** All /customer/* endpoints -- prevents customers from viewing other drivers' data
**Example:**
```rust
// Following existing pattern from customer_profile, customer_stats, etc.
async fn customer_passport(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    // ... query driving_passport and return response
}
```

### Pattern 4: Tiered Collection Grouping
**What:** Group passport entries into Starter/Explorer/Legend tiers using featured catalog data
**When to use:** In the /customer/passport API response
**Example:**
```rust
// Tier definitions based on featured catalog counts
// Tracks: 36 featured total -> Starter: 6, Explorer: 15, Legend: 36
// Cars: 41 featured total -> Starter: 6, Explorer: 15, Legend: 41
// Tier assignment happens server-side, returned in API response
// Featured tracks/cars have human-readable names from catalog.rs
```

### Pattern 5: PWA Page with BottomNav
**What:** Standard PWA page layout with auth check, loading state, and bottom navigation
**When to use:** All new customer-facing pages
**Example:**
```tsx
// Following existing pattern from stats/page.tsx, sessions/page.tsx
"use client";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

export default function PassportPage() {
  const router = useRouter();
  const [data, setData] = useState(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isLoggedIn()) { router.replace("/login"); return; }
    api.passport().then(res => { setData(res); setLoading(false); })
      .catch(() => setLoading(false));
  }, [router]);

  // ... render with BottomNav
  return (
    <div className="min-h-screen pb-20">
      {/* content */}
      <BottomNav />
    </div>
  );
}
```

### Anti-Patterns to Avoid
- **Showing all 50 tracks and 325 cars in one flat grid:** This causes Zeigarnik backfire -- the overwhelming list demotivates. Use tiered collections with progressive disclosure.
- **Client-side tier calculation:** The server knows the catalog and the driver's passport. Calculate tiers server-side and return them in the API response, not in the PWA.
- **Backfilling on every API call:** Use a guard -- if driving_passport is already populated, skip backfill. The upsert in persist_lap handles ongoing updates.
- **Blocking server startup with backfill:** Do NOT backfill all drivers on boot. Lazy per-driver backfill on first API call is correct -- avoids multi-second startup delay.
- **Adding badge display to staff /psychology/ routes:** Create NEW /customer/badges endpoint scoped by JWT. Staff endpoint stays separate for admin workflows.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Track/car display names | Hardcoded strings in PWA | catalog.rs FEATURED_TRACKS/FEATURED_CARS via API response | Already has human-readable names, categories, countries |
| Badge criteria evaluation | Custom evaluation in PWA | Existing psychology.rs evaluate_badges() | Already works, fires on session end |
| Driving passport updates | Manual SQL from multiple places | Single update_driving_passport() in psychology.rs called from persist_lap | Single source of truth, consistent upsert |
| Collection tier breakpoints | Hardcoded in PWA | Server-side tier definitions returned in API | Tier boundaries may change, keep server-authoritative |
| Auth/JWT handling | Custom token parsing in new endpoints | Existing extract_driver_id() | Already handles all JWT edge cases |

**Key insight:** The heavy lifting (badge evaluation, streak tracking, notification dispatch) was done in Phase 89. This phase is primarily about data population (passport upserts + backfill) and presentation (API + PWA).

## Common Pitfalls

### Pitfall 1: Backfill Creates Duplicate Passport Entries
**What goes wrong:** Backfill runs twice (race condition between page load and another request) and tries to INSERT duplicates.
**Why it happens:** Two concurrent requests both see passport_count == 0 and both run backfill.
**How to avoid:** Use INSERT OR IGNORE (the UNIQUE constraint on driver_id+track+car already exists). Backfill should use INSERT OR IGNORE, and ongoing upserts already use ON CONFLICT DO UPDATE. The UNIQUE constraint is the safety net.
**Warning signs:** sqlx errors about UNIQUE constraint violations in logs.

### Pitfall 2: Passport Shows Tracks/Cars Not in Featured Catalog
**What goes wrong:** Customer drove on a custom/modded track not in FEATURED_TRACKS. Passport shows raw folder IDs like "ks_monza66" with no display name.
**Why it happens:** The laps table stores raw track IDs from the sim telemetry. Not all track IDs map to featured catalog entries.
**How to avoid:** API response should include a "non-featured" section for track/car IDs that don't match the catalog. Display them with humanized names (replace underscores, capitalize). The tiered collections only use featured items; non-featured items appear in a separate "Other" section.
**Warning signs:** Passport grid showing cryptic technical IDs.

### Pitfall 3: Tier Boundaries Cause Empty Tiers
**What goes wrong:** A customer with 3 unique tracks sees only the Starter tier, but it shows "3/6 tracks" -- and the Explorer and Legend tiers are empty, which feels discouraging.
**Why it happens:** Strict tier gating where you only show the current tier.
**How to avoid:** Show ALL tiers but with different visual treatment: completed items are full color, locked items are grayed/dimmed. The customer can see the full journey but their current progress tier is highlighted. This is the correct Zeigarnik approach -- visibility of the whole with emphasis on achievable near-term goals.
**Warning signs:** Customers only seeing a tiny grid with no sense of what is ahead.

### Pitfall 4: Backfill Queries Are Slow on Large laps Table
**What goes wrong:** A driver with thousands of laps triggers a backfill that takes seconds, blocking the HTTP response.
**Why it happens:** GROUP BY driver_id, track, car on the entire laps table for one driver, with MIN(created_at), MIN(lap_time_ms), COUNT(*).
**How to avoid:** Filter the backfill query by `WHERE driver_id = ?` (not a full table scan). With proper indexing (idx_laps_driver_id already exists), this is a partition scan of one driver's laps only. For a venue with < 1000 customers and typical lap counts < 500 per driver, this will complete in < 100ms.
**Warning signs:** Slow first load of passport page.

### Pitfall 5: Badge Display Shows No Badges for New Customers
**What goes wrong:** A new customer opens their profile and sees "No badges yet" -- feels empty and unwelcoming.
**Why it happens:** Badge evaluation only runs on session end. A customer who just registered has no session yet.
**How to avoid:** This is correct behavior -- do not award fake badges. Instead, show the AVAILABLE badges with a "locked" state and what is needed to earn them. "Complete your first lap to earn the First Lap badge" is motivating. An empty "No badges" message is not.
**Warning signs:** Profile page with a blank badges section and no indication of what is achievable.

### Pitfall 6: hydration mismatch on Passport page
**What goes wrong:** Next.js hydration error because sessionStorage is read during server render.
**Why it happens:** `isLoggedIn()` reads localStorage which doesn't exist on server.
**How to avoid:** Follow the existing pattern in all PWA pages: check `isLoggedIn()` inside useEffect, not at render time. All existing pages (dashboard, profile, stats) do this correctly -- follow the same pattern.
**Warning signs:** React hydration mismatch error in browser console.

## Code Examples

Verified patterns from the existing codebase:

### Backfill Query (aggregate laps into passport)
```sql
-- Backfill driving_passport for a single driver from existing laps
-- This groups by driver_id+track+car and computes aggregates
INSERT OR IGNORE INTO driving_passport (id, driver_id, track, car, first_driven_at, best_lap_ms, lap_count)
SELECT
    lower(hex(randomblob(16))),  -- UUID
    driver_id,
    track,
    car,
    MIN(created_at) as first_driven_at,
    MIN(lap_time_ms) as best_lap_ms,
    COUNT(*) as lap_count
FROM laps
WHERE driver_id = ? AND valid = 1 AND lap_time_ms > 0
GROUP BY driver_id, track, car;
```

### Passport API Response Shape
```json
{
  "passport": {
    "tracks": {
      "total_driven": 12,
      "total_available": 36,
      "tiers": {
        "starter": {
          "name": "Starter Circuits",
          "target": 6,
          "items": [
            { "id": "monza", "name": "Monza", "category": "F1 Circuits", "country": "Italy",
              "driven": true, "lap_count": 47, "best_lap_ms": 95432, "first_driven_at": "2026-02-15" },
            { "id": "spa", "name": "Spa-Francorchamps", "category": "F1 Circuits", "country": "Belgium",
              "driven": false, "lap_count": 0, "best_lap_ms": null, "first_driven_at": null }
          ]
        },
        "explorer": { "name": "Explorer Circuits", "target": 15, "items": [...] },
        "legend": { "name": "Legend Circuits", "target": 36, "items": [...] }
      },
      "other": [
        { "id": "ks_monza66", "name": "Monza 1966", "driven": true, "lap_count": 3, "best_lap_ms": 104200 }
      ]
    },
    "cars": {
      "total_driven": 8,
      "total_available": 41,
      "tiers": { ... }
    },
    "summary": {
      "unique_tracks": 12,
      "unique_cars": 8,
      "total_laps": 234,
      "streak_weeks": 3
    }
  }
}
```

### Badge API Response Shape
```json
{
  "badges": {
    "earned": [
      { "id": "badge_first_lap", "name": "First Lap", "description": "Completed your very first lap at RacingPoint",
        "category": "milestone", "icon": "flag", "earned_at": "2026-02-15T10:30:00Z" }
    ],
    "available": [
      { "id": "badge_10_tracks", "name": "Explorer", "description": "Driven on 10 different tracks",
        "category": "milestone", "icon": "map", "progress": 7, "target": 10, "earned": false }
    ],
    "total_earned": 2,
    "total_available": 5
  }
}
```

### Customer Profile Extension for Badges
```tsx
// Existing profile page gets a new badge section
{/* Badges section -- add after wallet section */}
<div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
  <div className="flex items-center justify-between mb-3">
    <h3 className="text-sm font-semibold text-white">Badges</h3>
    <span className="text-xs text-rp-grey">{earned}/{total}</span>
  </div>
  <div className="grid grid-cols-4 gap-3">
    {badges.map(badge => (
      <div key={badge.id} className={`flex flex-col items-center ${badge.earned ? '' : 'opacity-30'}`}>
        <div className="w-12 h-12 rounded-full bg-rp-red/20 flex items-center justify-center mb-1">
          {/* icon based on badge.icon */}
        </div>
        <span className="text-[10px] text-center text-rp-grey">{badge.name}</span>
      </div>
    ))}
  </div>
</div>
```

## Tier Definition Logic

The tiered collection system maps to the existing AC catalog featured items:

### Track Tiers
| Tier | Count | Criteria | Psychology |
|------|-------|----------|------------|
| Starter | 6 | First 6 featured tracks (most popular F1 circuits: Spa, Monza, Silverstone, Red Bull Ring, Barcelona, Monaco) | Near-term achievable goal -- a newcomer can drive 6 tracks in 2-3 visits |
| Explorer | 15 | Next 9 featured tracks added (total 15, mix of F1 + Real Circuits) | Medium-term goal -- regular customers hit this in 1-2 months |
| Legend | 36 | All 36 featured tracks | Long-term goal -- completionists who have tried everything |

### Car Tiers
| Tier | Count | Criteria | Psychology |
|------|-------|----------|------------|
| Starter | 6 | First 6 featured cars (mix of F1 + GT3 + Supercar) | Quick wins -- customers try different car types early |
| Explorer | 15 | Next 9 featured cars added (total 15) | Category exploration -- JDM, Porsche, Classics |
| Legend | 41 | All 41 featured cars | Full catalog mastery |

The server defines which featured items belong to which tier using sort_order from `catalog.rs` FEATURED_TRACKS/FEATURED_CARS arrays. The first N items in the array map to Starter, next to Explorer, remainder to Legend.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| driving_passport table exists but is empty | Passport upserted on every valid lap + backfilled from history | Phase 90 | Passport data is always current |
| Badge display only in staff /psychology/ endpoints | Customer-facing /customer/badges endpoint + PWA profile section | Phase 90 | Customers can see their own badges |
| No driving journey visualization | Tiered passport page in PWA | Phase 90 | Customers see progress and goals |

**Dependencies from Phase 89 (all verified complete):**
- psychology.rs module with evaluate_badges(), update_streak(), queue_notification()
- 7 DB tables including driving_passport (UNIQUE driver_id+track+car) and achievements (5 seed badges)
- post_session_hooks calls evaluate_badges + update_streak
- 5 staff API endpoints under /psychology/

## Open Questions

1. **Passport navigation placement**
   - What we know: BottomNav has 7 tabs (Home, Live, Sessions, Race, Friends, Stats, Profile). Adding an 8th would be crowded.
   - What's unclear: Where to link the passport page
   - Recommendation: Add a "Passport" card on the /dashboard (Home) page and a link from /profile. Do NOT add to BottomNav -- use in-page navigation. The passport is a destination reached from the dashboard hero area or profile.

2. **Badge icon rendering**
   - What we know: 5 seed badges have icon fields: 'flag', 'map', 'trophy', 'car', 'fire'
   - What's unclear: Whether to use emoji, SVG icons, or an icon library
   - Recommendation: Use inline SVG icons matching the existing PWA pattern (BottomNav already uses inline SVGs). Define a small badge icon map in the passport/profile component. No external icon library needed for 5-10 icons.

3. **Non-AC game tracks in passport**
   - What we know: Laps come from AC (track IDs), F1 25 (generic track names), Forza, iRacing, LMU
   - What's unclear: Whether non-AC tracks have human-readable names in the catalog
   - Recommendation: The catalog.rs only covers AC. For non-AC games, display the raw track/car names from telemetry (they are usually human-readable already for F1 25, e.g., "Bahrain International Circuit"). Group non-AC entries separately under their sim type.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (cargo test) + manual PWA verification |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p racecontrol --lib` |
| Full suite command | `cargo test -p racecontrol` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PROG-01 | /customer/passport returns track/car passport data | unit | `cargo test -p racecontrol --lib -- passport --exact` | Wave 0 |
| PROG-02 | Passport response has tiered collections (starter/explorer/legend) | unit | `cargo test -p racecontrol --lib -- passport_tiers --exact` | Wave 0 |
| PROG-03 | Badges awarded and retrievable via /customer/badges | unit | `cargo test -p racecontrol --lib -- customer_badges --exact` | Wave 0 |
| PROG-04 | Backfill populates passport from laps for driver with no passport entries | unit | `cargo test -p racecontrol --lib -- passport_backfill --exact` | Wave 0 |
| PROG-05 | /customer/badges returns earned + available badges | unit | `cargo test -p racecontrol --lib -- badges_earned_available --exact` | Wave 0 |
| PROG-01 | PWA /passport page renders passport grid | manual-only | Visual verification in browser | N/A |
| PROG-05 | PWA /profile page shows badge showcase | manual-only | Visual verification in browser | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol --lib`
- **Per wave merge:** `cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `psychology::tests::test_update_driving_passport` -- covers PROG-01 upsert logic
- [ ] `psychology::tests::test_backfill_driving_passport` -- covers PROG-04 backfill from laps
- [ ] Unit tests for tier grouping logic (featured catalog -> starter/explorer/legend split)
- [ ] Unit test for /customer/badges response shape (earned vs available with progress)

## Sources

### Primary (HIGH confidence)
- `/root/racecontrol/crates/racecontrol/src/psychology.rs` -- Full Phase 89 implementation (evaluate_badges, update_streak, queue_notification, spawn_dispatcher, 5 metric types)
- `/root/racecontrol/crates/racecontrol/src/db/mod.rs` -- driving_passport table schema (lines 2104-2116), UNIQUE(driver_id, track, car), 5 seed badges (lines 2063-2072)
- `/root/racecontrol/crates/racecontrol/src/lap_tracker.rs` -- persist_lap() flow (lap INSERT -> PB check -> track record check), NO driving_passport upsert currently
- `/root/racecontrol/crates/racecontrol/src/catalog.rs` -- 36 featured tracks, 41 featured cars, ALL_TRACK_IDS (50), ALL_CAR_IDS (325)
- `/root/racecontrol/crates/racecontrol/src/api/routes.rs` -- customer_routes() pattern, extract_driver_id(), 5 existing /psychology/ staff endpoints
- `/root/racecontrol/pwa/src/lib/api.ts` -- fetchApi() wrapper, DriverProfile type, existing api methods
- `/root/racecontrol/pwa/src/app/profile/page.tsx` -- existing profile page structure (nickname, wallet, stats)
- `/root/racecontrol/pwa/src/components/BottomNav.tsx` -- 7 tabs (Home, Live, Sessions, Race, Friends, Stats, Profile)
- `/root/racecontrol/pwa/src/app/globals.css` -- RacingPoint theme colors (rp-red, rp-card, rp-border, rp-grey)

### Secondary (MEDIUM confidence)
- Phase 89 VERIFICATION.md -- confirms all foundation artifacts are complete and working
- Phase 89 Plan summaries -- confirm post_session_hooks wiring, seed badges, dispatcher startup

### Tertiary (LOW confidence)
- None -- all findings from direct codebase inspection

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all libraries already in workspace
- Architecture: HIGH -- every pattern has working precedent in existing codebase (persist_lap, customer_profile, PWA pages)
- Pitfalls: HIGH -- identified from direct code inspection (empty driving_passport table, catalog mapping gaps, tier UX)
- Database schema: HIGH -- driving_passport table already exists with correct schema from Phase 89
- PWA patterns: HIGH -- existing pages (profile, stats, dashboard) provide exact template to follow

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable -- codebase patterns unlikely to change)
