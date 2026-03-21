# Phase 91: Session Experience - Research

**Researched:** 2026-03-21
**Domain:** Rust (Axum) backend API enhancement, Next.js 16 PWA UI (confetti, toast, peak-end layout), real-time event delivery
**Confidence:** HIGH

## Summary

Phase 91 transforms the post-session experience into a psychologically optimized "last memory" for every customer visit. The work has four concrete deliverables: (1) confetti animation on PB detection, (2) peak-end reordered session reports, (3) percentile ranking in the main session detail view, and (4) real-time PB toast notifications during active sessions.

The critical insight from codebase analysis: most of the backend infrastructure already exists. The share report endpoint (`/customer/sessions/{id}/share`) already computes percentile ranking, PB detection, consistency ratings, and track records. The session detail endpoint (`/customer/sessions/{id}`) does NOT include these -- it returns raw session data (pod, pricing, laps, events). The primary backend work is (a) folding percentile + PB data from the share report logic into the session detail response, (b) adding a new DashboardEvent variant for PB events that can be broadcast from `persist_lap()`, and (c) creating a customer-facing polling or notification endpoint for real-time PB awareness. The frontend work is adding canvas-confetti for PB celebrations, sonner for toast notifications, restructuring the session detail page layout to show peak moments first, and wiring a polling mechanism for active-session PB detection.

A key architecture decision: the existing WebSocket (`/ws/dashboard`) broadcasts ALL events to ALL dashboard subscribers. There is no customer-scoped WebSocket. For SESS-04 (real-time PB toast), the simplest correct approach is a lightweight polling endpoint (`/customer/session-events?since=TIMESTAMP`) that the PWA calls every 5-10 seconds during active sessions only. This avoids building a new customer WebSocket infrastructure just for one notification type, and the polling load is negligible (< 8 concurrent active sessions at a venue with 8 pods).

**Primary recommendation:** Enhance the existing `customer_session_detail` response with percentile, PB, and peak-moment data (extracted from the share report logic). Add `canvas-confetti` (1.9.4) and `sonner` (2.0.7) as PWA dependencies. Restructure the session detail page to show peak moments first. Add a `PbAchieved` broadcast event in `lap_tracker.rs` and a simple polling endpoint for the PWA.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SESS-01 | PB achievement triggers confetti animation in PWA session view (canvas-confetti) | canvas-confetti 1.9.4 installed in PWA; triggered when session detail response includes `is_new_pb: true` or when polling endpoint returns a PB event for the active session |
| SESS-02 | Session-end report highlights best moment first (peak moment) before showing averages | Session detail page restructured: hero card shows best lap + PB status + percentile rank at top; stats grid (averages, total laps, consistency) moved below; billing/receipt pushed further down |
| SESS-03 | Session-end shows percentile ranking ("faster than 73% of drivers") | Percentile calculation logic already exists in the share report endpoint; extracted into a shared function and included in the `customer_session_detail` response |
| SESS-04 | Toast notification (sonner) for real-time PB during active session in PWA | sonner 2.0.7 installed in PWA; active session view polls `/customer/active-session/events` every 5s; when a PB event is returned, sonner toast fires + confetti |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8 | SQLite queries for percentile calculation | Already in workspace Cargo.toml |
| serde / serde_json | workspace | JSON serialization for enhanced API responses | Already used throughout |
| axum | 0.8 | HTTP handler for new/enhanced endpoints | Already used throughout |
| Next.js | 16.1.6 | PWA page framework | Already in pwa/package.json |
| React | 19.2.3 | UI components, hooks for polling | Already in pwa/package.json |
| Tailwind CSS | 4 | Styling for peak-end layout | Already in pwa/package.json |
| canvas-confetti | 1.9.4 | PB celebration confetti animation | **NEW** -- lightweight (6.3KB gzipped), zero dependencies, specified in SESS-01 requirements |
| sonner | 2.0.7 | Toast notification component | **NEW** -- React 19 compatible, specified in SESS-04 requirements |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| chrono | workspace | Timestamp handling for polling since-parameter | Already used throughout |
| uuid | workspace | Event ID generation | Already used throughout |
| tracing | workspace | Structured logging for PB events | Already used throughout |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| canvas-confetti | react-confetti | canvas-confetti is framework-agnostic, lighter, and explicitly required by SESS-01 |
| sonner | react-hot-toast | sonner is explicitly required by SESS-04; better React 19 support, cleaner API |
| Polling for active PB | Customer WebSocket | WebSocket requires new auth-scoped WS infrastructure; polling is simpler, load is trivial (max 8 concurrent polls at 5s interval = 1.6 req/s), and the feature only activates during active sessions |
| Server-Sent Events (SSE) | Polling | SSE would be cleaner but would require a new SSE endpoint with auth and connection management; for a single event type on max 8 concurrent customers, polling is correct |

**Installation:**
```bash
cd /root/racecontrol/pwa && npm install canvas-confetti@1.9.4 sonner@2.0.7
npm install --save-dev @types/canvas-confetti
```

**Version verification:**
- canvas-confetti: 1.9.4 (verified via `npm view canvas-confetti version` on 2026-03-21)
- sonner: 2.0.7 (verified via `npm view sonner version` on 2026-03-21)

## Architecture Patterns

### Recommended Project Structure
```
crates/racecontrol/src/
    lap_tracker.rs         # MODIFIED: broadcast PbAchieved event after PB detection
    api/routes.rs          # MODIFIED: enhance customer_session_detail with percentile/PB data;
                           #           add /customer/active-session/events polling endpoint
    psychology.rs          # READ-ONLY: existing notification infrastructure

crates/rc-common/src/
    protocol.rs            # MODIFIED: add PbAchieved variant to DashboardEvent enum

pwa/src/
    app/sessions/[id]/page.tsx  # MODIFIED: peak-end layout, confetti, percentile display
    app/book/active/page.tsx    # MODIFIED: add PB polling + toast during active session
    lib/api.ts                  # MODIFIED: add activeSessionEvents() method, update SessionDetailSession type
    components/Confetti.tsx     # NEW: canvas-confetti wrapper component
    components/Toaster.tsx      # NEW: sonner Toaster provider (mounted in layout.tsx)
    app/layout.tsx              # MODIFIED: add Toaster provider
    package.json                # MODIFIED: add canvas-confetti, sonner dependencies
```

### Pattern 1: Enhanced Session Detail Response (peak-end data)
**What:** Add percentile, PB status, and peak-moment data to the session detail API response
**When to use:** Every call to `/customer/sessions/{id}` for completed sessions
**Example:**
```rust
// In routes.rs customer_session_detail, after computing laps/stats:
// Reuse the same percentile logic from customer_session_share
let track = laps.first().map(|l| l.7.clone()).unwrap_or_default();
let car = laps.first().map(|l| l.8.clone()).unwrap_or_default();

let percentile = compute_percentile(&state, best_lap_ms, &track, &car).await;
let is_new_pb = check_is_session_pb(&state, &driver_id, best_lap_ms, &track, &car).await;
let personal_best_ms = get_personal_best(&state, &driver_id, &track, &car).await;

// Add to response JSON:
// "percentile_rank": percentile,
// "percentile_text": percentile.map(|p| format!("Faster than {}% of drivers", p)),
// "is_new_pb": is_new_pb,
// "personal_best_ms": personal_best_ms,
// "peak_moment": { "type": "pb" | "fastest_lap", "lap_number": N, "time_ms": T }
```

### Pattern 2: PB Event Broadcasting
**What:** When persist_lap detects is_pb=true, broadcast a PbAchieved event on dashboard_tx
**When to use:** Inside persist_lap() after the personal_bests UPSERT succeeds
**Example:**
```rust
// In lap_tracker.rs, after the is_pb block:
if is_pb {
    // ... existing UPSERT code ...

    // Broadcast PB event for real-time notification
    let _ = state.dashboard_tx.send(DashboardEvent::PbAchieved {
        driver_id: lap.driver_id.clone(),
        session_id: lap.session_id.clone(),
        track: lap.track.clone(),
        car: lap.car.clone(),
        lap_time_ms: lap.lap_time_ms as i64,
        lap_id: lap.id.clone(),
    });
}
```

### Pattern 3: Customer Active Session Events Polling
**What:** Lightweight polling endpoint that returns PB events since a timestamp
**When to use:** PWA calls this every 5s while on the active booking page
**Example:**
```rust
// GET /customer/active-session/events?since=2026-03-21T10:30:00Z
async fn customer_active_session_events(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<SinceParams>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Get active billing session for this driver
    let active_session = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM billing_sessions WHERE driver_id = ? AND status = 'active' LIMIT 1"
    ).bind(&driver_id).fetch_optional(&state.db).await.ok().flatten();

    let session_id = match active_session {
        Some((id,)) => id,
        None => return Json(json!({ "events": [] })),
    };

    // Query laps that are PBs since the given timestamp
    let since = params.since.unwrap_or_default();
    let pb_laps = sqlx::query_as::<_, (String, i64, String, String, String)>(
        "SELECT l.id, l.lap_time_ms, l.track, l.car, l.created_at
         FROM laps l
         JOIN personal_bests pb ON l.id = pb.lap_id
         WHERE l.session_id = ? AND l.driver_id = ? AND l.created_at > ?
         ORDER BY l.created_at ASC"
    )
    .bind(&session_id)
    .bind(&driver_id)
    .bind(&since)
    .fetch_all(&state.db).await.unwrap_or_default();

    Json(json!({
        "events": pb_laps.iter().map(|l| json!({
            "type": "pb",
            "lap_id": l.0,
            "lap_time_ms": l.1,
            "track": l.2,
            "car": l.3,
            "at": l.4,
        })).collect::<Vec<_>>()
    }))
}
```

### Pattern 4: Canvas Confetti Integration
**What:** Wrapper component that fires confetti on mount or on trigger
**When to use:** Session detail page when is_new_pb is true; active session when PB event arrives
**Example:**
```tsx
// components/Confetti.tsx
"use client";
import { useEffect, useCallback } from "react";
import confetti from "canvas-confetti";

export function fireConfetti() {
  // RacingPoint red + gold confetti
  confetti({
    particleCount: 100,
    spread: 70,
    origin: { y: 0.6 },
    colors: ["#E10600", "#FFD700", "#FFFFFF"],
  });
  // Second burst for emphasis
  setTimeout(() => {
    confetti({
      particleCount: 50,
      angle: 60,
      spread: 55,
      origin: { x: 0 },
      colors: ["#E10600", "#FFD700"],
    });
    confetti({
      particleCount: 50,
      angle: 120,
      spread: 55,
      origin: { x: 1 },
      colors: ["#E10600", "#FFD700"],
    });
  }, 250);
}

export function ConfettiOnMount({ enabled }: { enabled: boolean }) {
  useEffect(() => {
    if (enabled) {
      // Small delay so page renders first
      const timer = setTimeout(() => fireConfetti(), 300);
      return () => clearTimeout(timer);
    }
  }, [enabled]);
  return null;
}
```

### Pattern 5: Sonner Toast for Real-Time PB
**What:** Toast notification using sonner when polling detects a PB during active session
**When to use:** Active session page (book/active) when PB event arrives
**Example:**
```tsx
// In layout.tsx, add Toaster:
import { Toaster } from "sonner";
// Inside layout return: <Toaster theme="dark" position="top-center" richColors />

// In active session page:
import { toast } from "sonner";
import { fireConfetti } from "@/components/Confetti";

// When PB event received from polling:
toast.success("NEW PERSONAL BEST!", {
  description: `${formatLapTime(event.lap_time_ms)} on ${event.track}`,
  duration: 5000,
});
fireConfetti();
```

### Pattern 6: Peak-End Session Report Layout
**What:** Restructure session detail page to show best moments first
**When to use:** Session detail page for completed sessions
**Example:**
```
Current layout order:          Peak-end layout order:
1. Session Summary Header      1. Peak Moment Hero Card (PB / best lap + confetti)
2. Share Button                2. Percentile Ranking Banner
3. Share Card Modal            3. Session Summary Header (condensed)
4. Multiplayer Results         4. Session Stats Grid (best lap highlighted)
5. Receipt / Billing           5. Share Button
6. Session Stats Grid          6. Lap Times Chart
7. Session Timeline            7. Lap-by-Lap Table
8. Lap Times Chart             8. Multiplayer Results (if applicable)
9. Lap-by-Lap Table            9. Receipt / Billing
                               10. Session Timeline
```

### Anti-Patterns to Avoid
- **Building a customer WebSocket endpoint:** Overkill for a single notification type with max 8 concurrent customers. Use polling instead.
- **Computing percentile on every request without caching:** The percentile query does COUNT(DISTINCT driver_id) across the laps table. For < 1000 customers this is fast (< 10ms), but do NOT expand it to do expensive aggregation. Keep the existing query.
- **Duplicating share report logic:** The percentile, PB detection, and consistency calculations already exist in `customer_session_share`. Extract into shared helper functions, do not copy-paste.
- **Firing confetti on page re-visits:** Confetti should only fire ONCE when the session is first viewed after completion, or when a PB event arrives during an active session. Use a flag (sessionStorage or state) to prevent re-triggering on navigation.
- **Showing percentile when fewer than 5 drivers have driven the track+car:** A percentile of "faster than 100% of drivers" when only 2 people have driven that combo is misleading. Set a minimum threshold (e.g., 5 unique drivers) before showing percentile.
- **Blocking persist_lap on PB broadcast:** The DashboardEvent broadcast is non-blocking (returns immediately even if no subscribers). But the polling endpoint query should also be lightweight -- join on personal_bests by lap_id is O(1) with the index.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Confetti animation | CSS particles or custom canvas code | canvas-confetti 1.9.4 | Battle-tested, 6.3KB, handles cleanup, cross-browser, specified in requirements |
| Toast notifications | Custom toast component with timeouts | sonner 2.0.7 | Accessible, animation support, React 19 compatible, specified in requirements |
| Percentile calculation | New standalone function | Extract from existing customer_session_share endpoint | Logic already verified and working |
| PB detection | Custom comparison in frontend | Backend already detects PB in persist_lap() and stores in personal_bests | Single source of truth |
| Real-time polling | Custom setInterval with fetch | React useEffect + AbortController pattern from existing PWA pages | Handles cleanup, avoids memory leaks |

**Key insight:** The share report endpoint (customer_session_share) already computes everything needed for SESS-02 and SESS-03 (percentile, PB, consistency, track record). The work is making that data available in the standard session detail response and restructuring the PWA layout.

## Common Pitfalls

### Pitfall 1: Confetti Fires on Every Session Detail View
**What goes wrong:** Customer revisits an old session that had a PB and gets confetti every time.
**Why it happens:** Confetti trigger checks `is_new_pb` flag which is always true for that session.
**How to avoid:** Track whether confetti has already been shown for this session. Use `sessionStorage.setItem("confetti_shown_" + sessionId, "1")` and skip confetti if the key exists. For active-session PB toasts, track the last-seen PB lap_id to avoid duplicate toasts.
**Warning signs:** Users reporting confetti on every page load of old sessions.

### Pitfall 2: Polling Continues After Leaving Active Session Page
**What goes wrong:** PWA keeps polling `/customer/active-session/events` even after navigating away from the active session page, wasting bandwidth and battery.
**Why it happens:** useEffect cleanup not properly aborting the fetch or clearing the interval.
**How to avoid:** Use AbortController in the useEffect return cleanup. Also check billing session status -- if the session has ended (status != 'active'), stop polling entirely.
**Warning signs:** Network tab showing continued requests to the events endpoint after navigation.

### Pitfall 3: Percentile Shows "Faster than 100% of drivers" for Rare Track/Car Combos
**What goes wrong:** A customer drives a rare modded track with only 1-2 other drivers. They see "Faster than 100% of drivers" which feels meaningless.
**Why it happens:** The percentile calculation works mathematically but has no minimum sample size.
**How to avoid:** Only show percentile text when `total_count >= 5` (at least 5 unique drivers have set valid laps on this track+car). Below that threshold, omit the percentile or show "Not enough data for ranking" instead.
**Warning signs:** Percentile showing for very obscure tracks with tiny sample sizes.

### Pitfall 4: Share Report and Session Detail Return Different Percentiles
**What goes wrong:** The session detail page shows "Faster than 73% of drivers" but clicking Share shows "Top 28% of drivers" for the same session.
**Why it happens:** Duplicated percentile logic with slightly different SQL queries or rounding.
**How to avoid:** Extract the percentile calculation into a single shared function (`compute_percentile`) called by both `customer_session_detail` and `customer_session_share`. Same function, same result.
**Warning signs:** Users noticing different numbers on the same session.

### Pitfall 5: Active Session Polling Returns Stale Events
**What goes wrong:** When a customer opens the active session view, the first poll returns ALL PB events from the session start, triggering confetti and toast for events that happened minutes ago.
**Why it happens:** The `since` parameter defaults to empty/epoch, so the first query returns everything.
**How to avoid:** Initialize `since` to the current timestamp (ISO 8601) when the polling useEffect first mounts. Only events AFTER the customer opens the page should trigger toasts. Historical PBs are visible in the session detail but should NOT trigger animated celebrations.
**Warning signs:** Burst of toasts appearing immediately on page load for a session that has been running for a while.

### Pitfall 6: Sonner Toaster Not Themed to Match RacingPoint Dark UI
**What goes wrong:** Toast appears with default light-mode styling against the dark RacingPoint background.
**Why it happens:** Sonner defaults to system theme; RacingPoint PWA is always dark.
**How to avoid:** Configure Toaster with `theme="dark"` explicitly. Also set custom `toastOptions` to use RacingPoint colors (rp-red for success).
**Warning signs:** Bright white toast box popping up on dark background.

## Code Examples

Verified patterns from the existing codebase:

### Existing Percentile Calculation (from customer_session_share)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/api/routes.rs lines 8658-8696
// This EXACT logic needs to be extracted into a shared function
let percentile = if let Some(best) = best_lap_ms {
    if !track.is_empty() && !car.is_empty() {
        let total_count: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(DISTINCT driver_id) FROM laps WHERE track = ? AND car = ? AND valid = 1",
        )
        .bind(&track).bind(&car)
        .fetch_optional(&state.db).await.ok().flatten();

        let faster_count: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(DISTINCT driver_id) FROM (
                SELECT driver_id, MIN(lap_time_ms) as best
                FROM laps WHERE track = ? AND car = ? AND valid = 1
                GROUP BY driver_id
            ) WHERE best < ?",
        )
        .bind(&track).bind(&car).bind(best)
        .fetch_optional(&state.db).await.ok().flatten();

        match (total_count, faster_count) {
            (Some((total,)), Some((faster,))) if total >= 5 => {
                // Changed: minimum 5 drivers threshold
                Some(((total - faster) as f64 / total as f64 * 100.0).round() as u32)
            }
            _ => None,
        }
    } else { None }
} else { None };
```

### Existing PB Detection in persist_lap
```rust
// Source: /root/racecontrol/crates/racecontrol/src/lap_tracker.rs lines 121-158
// is_pb is already computed. We just need to broadcast the event.
let is_pb = match existing_pb {
    Some((current_best,)) => (lap.lap_time_ms as i64) < current_best,
    None => true, // First lap on this track+car
};

if is_pb {
    let _ = sqlx::query(
        "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, lap_id, achieved_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'))
         ON CONFLICT(driver_id, track, car) DO UPDATE SET ...
    // After the UPSERT, ADD: broadcast PbAchieved event
}
```

### Existing Session Detail Response (what we enhance)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/api/routes.rs lines 4565-4592
// Current response shape. We ADD: percentile_rank, percentile_text,
// is_new_pb, personal_best_ms, peak_moment
Json(json!({
    "session": {
        "id": session.0,
        "pod_id": session.1,
        // ... existing fields ...
        "total_laps": total_laps,
        "best_lap_ms": best_lap_ms,
        "average_lap_ms": avg_lap_ms,
        // NEW fields:
        "percentile_rank": percentile,
        "percentile_text": percentile.map(|p| format!("Faster than {}% of drivers", p)),
        "is_new_pb": is_new_pb,
        "personal_best_ms": personal_best_ms,
    },
    "laps": laps_json,
    "events": events_json,
}))
```

### PWA Polling Pattern (from existing telemetry page)
```tsx
// Source: /root/racecontrol/pwa/src/app/telemetry/page.tsx pattern
// Similar pattern for active session events polling
useEffect(() => {
  if (!isActive) return;

  let since = new Date().toISOString();
  const controller = new AbortController();

  const poll = async () => {
    try {
      const res = await api.activeSessionEvents(since);
      if (res.events && res.events.length > 0) {
        // Process new PB events
        for (const evt of res.events) {
          if (evt.type === "pb") {
            toast.success("NEW PERSONAL BEST!", {
              description: `${formatLapTime(evt.lap_time_ms)} on ${evt.track}`,
            });
            fireConfetti();
            since = evt.at; // Move cursor forward
          }
        }
      }
    } catch { /* abort or network error */ }
  };

  const interval = setInterval(poll, 5000);
  return () => {
    controller.abort();
    clearInterval(interval);
  };
}, [isActive]);
```

## Database Schema Design

### No New Tables Needed

This phase does not require new database tables. It uses:
- `personal_bests` (existing) -- for PB detection, already populated by `persist_lap`
- `laps` (existing) -- for percentile calculation, already has indexes on track+car
- `billing_sessions` (existing) -- for active session lookup

### New DashboardEvent Variant (in protocol.rs)
```rust
// Added to enum DashboardEvent in crates/rc-common/src/protocol.rs
/// Personal best achieved during a session (broadcast for real-time PWA notification)
PbAchieved {
    driver_id: String,
    session_id: String,
    track: String,
    car: String,
    lap_time_ms: i64,
    lap_id: String,
},
```

### Useful Existing Indexes
```sql
-- These already exist and support the percentile query:
-- idx_laps_track_car (track, car) on laps table
-- idx_personal_bests_driver (driver_id) on personal_bests table
-- personal_bests has UNIQUE(driver_id, track, car)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Session detail shows raw data (laps, billing, timeline) | Session detail enhanced with percentile, PB, peak moment at top | Phase 91 | Peak-end memory optimization |
| Percentile only in share report (separate click) | Percentile inline in session detail view | Phase 91 | Customers see ranking without extra action |
| No PB celebration | Confetti animation via canvas-confetti | Phase 91 | Emotional positive reinforcement on achievement |
| No real-time PB awareness | Polling-based PB toast via sonner during active sessions | Phase 91 | Immediate positive feedback loop |

**Existing assets to leverage:**
- `customer_session_share` endpoint: percentile logic, PB detection, consistency, track record comparison -- ALL reusable
- `persist_lap()` in lap_tracker.rs: already computes `is_pb` and updates `personal_bests` -- just needs broadcast
- `dashboard_tx` broadcast channel: already supports new event variants without subscriber changes
- Session detail page (`/sessions/[id]`): 867 lines, well-structured components (StatTile, ReceiptRow, BackButton) -- restructuring is straightforward

## Open Questions

1. **Confetti on first session ever**
   - What we know: `is_pb` is `true` when a driver has no existing personal_best for a track+car (first lap = auto-PB)
   - What's unclear: Should a customer's very first lap trigger confetti? It is technically a PB, but the customer may not understand the context.
   - Recommendation: Yes, show confetti for first-ever PB too. It creates a positive first impression. The toast can say "First Personal Best!" instead of "NEW Personal Best!" to distinguish.

2. **Percentile text format**
   - What we know: Share report uses "Top X% of drivers", session detail could use "Faster than Y% of drivers"
   - What's unclear: Which format is more motivating and consistent
   - Recommendation: Use "Faster than Y% of drivers" everywhere (both session detail and share report). This is positive-framed (focus on what you beat) rather than rank-framed (top X%). Update share report to match.

3. **Active session page location**
   - What we know: Active booking page is at `/book/active`. The session detail page is at `/sessions/[id]`.
   - What's unclear: Whether PB polling should be on `/book/active` (where customers watch their current session) or `/sessions/[id]` (which customers open post-session)
   - Recommendation: PB polling on `/book/active` only (this is the "live" view). The session detail page at `/sessions/[id]` shows the completed session report with confetti on first load if it had a PB. These are two distinct experiences.

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
| SESS-01 | Session detail response includes is_new_pb field | unit | `cargo test -p racecontrol --lib -- session_detail_pb --exact` | Wave 0 |
| SESS-01 | Confetti triggers on PB session load | manual-only | Visual verification in browser | N/A |
| SESS-02 | Session detail response includes peak_moment data | unit | `cargo test -p racecontrol --lib -- session_detail_peak --exact` | Wave 0 |
| SESS-02 | Session page shows peak moment first | manual-only | Visual verification in browser | N/A |
| SESS-03 | Percentile calculation returns correct value for 5+ drivers | unit | `cargo test -p racecontrol --lib -- compute_percentile --exact` | Wave 0 |
| SESS-03 | Percentile returns None for fewer than 5 drivers | unit | `cargo test -p racecontrol --lib -- percentile_minimum_threshold --exact` | Wave 0 |
| SESS-04 | Active session events endpoint returns PB events since timestamp | unit | `cargo test -p racecontrol --lib -- active_session_events --exact` | Wave 0 |
| SESS-04 | Toast appears on PB during active session | manual-only | Visual verification in browser | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol --lib`
- **Per wave merge:** `cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `compute_percentile` shared function unit test -- covers SESS-03 core logic
- [ ] `customer_active_session_events` endpoint unit test -- covers SESS-04 backend
- [ ] Session detail enhancement test -- covers SESS-01 + SESS-02 is_new_pb and peak_moment fields
- [ ] PbAchieved DashboardEvent variant -- verify it serializes correctly with serde

## Sources

### Primary (HIGH confidence)
- `/root/racecontrol/crates/racecontrol/src/api/routes.rs` lines 4440-4593 -- customer_session_detail endpoint (current response shape)
- `/root/racecontrol/crates/racecontrol/src/api/routes.rs` lines 8640-8760 -- customer_session_share endpoint (percentile, PB, consistency logic)
- `/root/racecontrol/crates/racecontrol/src/lap_tracker.rs` lines 120-158 -- PB detection in persist_lap
- `/root/racecontrol/crates/racecontrol/src/psychology.rs` -- full Phase 89 psychology module (notification dispatch, badge evaluation)
- `/root/racecontrol/crates/rc-common/src/protocol.rs` lines 439-560 -- DashboardEvent enum (all variants)
- `/root/racecontrol/crates/racecontrol/src/ws/mod.rs` lines 836-1062 -- dashboard WebSocket handler (broadcast pattern)
- `/root/racecontrol/crates/racecontrol/src/state.rs` lines 104, 181 -- dashboard_tx broadcast channel
- `/root/racecontrol/pwa/src/app/sessions/[id]/page.tsx` -- full session detail page (867 lines, all sub-components)
- `/root/racecontrol/pwa/src/lib/api.ts` -- fetchApi wrapper, all types including ShareReport, SessionDetailSession
- `/root/racecontrol/pwa/package.json` -- current dependencies (Next.js 16.1.6, React 19.2.3)
- `npm view canvas-confetti version` -> 1.9.4 (verified 2026-03-21)
- `npm view sonner version` -> 2.0.7 (verified 2026-03-21)

### Secondary (MEDIUM confidence)
- Phase 89 RESEARCH.md -- psychology module architecture, notification patterns
- Phase 90 RESEARCH.md -- passport/badge PWA patterns, customer API endpoint conventions

### Tertiary (LOW confidence)
- None -- all findings from direct codebase inspection and npm registry

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- canvas-confetti and sonner versions verified from npm registry; all other deps already in workspace
- Architecture: HIGH -- every pattern has working precedent (session detail endpoint, broadcast events, PWA polling)
- Pitfalls: HIGH -- identified from direct code inspection (duplicate logic, polling lifecycle, confetti re-triggering)
- API enhancement: HIGH -- percentile logic already exists in the share report endpoint, just needs extraction
- PWA changes: HIGH -- session detail page fully understood (867 lines), restructuring is mechanical

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable -- codebase patterns unlikely to change)
