# Phase 94: Pricing & Conversion - Research

**Researched:** 2026-03-24
**Domain:** Psychology-driven pricing UI, real-time availability, commitment tracking, social proof
**Confidence:** HIGH (full codebase access, all integration points verified)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Pricing Display & Anchoring
- 3-tier pricing display lives in both kiosk booking wizard AND web PWA /book page for consistent customer touchpoints
- Middle "value" tier visually emphasized with "Most Popular" badge, Racing Red (#E10600) border, and slightly larger card — classic decoy anchoring
- Prices update dynamically using existing `pricing_rules` table (peak/off-peak multipliers already implemented in billing.rs)
- Anchor display uses strikethrough original price + bold current price for anchoring effect

#### Pod Scarcity & Social Proof
- Real-time pod availability shown as "X of 8 pods available now" with color gradient (green→yellow→red) using live fleet health data
- Social proof displays "Y drivers raced this week" + "Z sessions today" from real billing_sessions data — actual counts, never fabricated
- Social proof placed below pricing tiers on booking page — visible during decision moment
- Zero availability shows "All pods in use — next slot likely in ~Xmin" with waitlist CTA — loss-framed scarcity
- No Fake Data rule applies: social proof uses actual counts, never fabricated

#### Commitment Ladder & Nudges
- Ladder steps: Trial → Single Session → Package (5-pack) → Membership — matches existing pricing_tiers
- Next-step nudges delivered via post-session WhatsApp through psychology engine nudge_queue — e.g., "You've done 3 sessions! Save 20% with a 5-pack"
- Ladder position tracked via new `commitment_ladder` column on drivers table (enum: trial/single/package/member)
- Nudge triggers: after 2nd single session (→ package nudge) or after 3rd package use (→ membership nudge) — natural escalation points

### Claude's Discretion
- API endpoint naming and response structure
- Component file organization within kiosk/web apps
- Exact color gradient thresholds for pod availability indicator

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PRICE-01 | Pricing page displays 3-tier structure with middle tier visually emphasized (decoy/anchoring) | Existing `pricing_tiers` table + `list_pricing_tiers` endpoint at `GET /api/v1/pricing` ready; kiosk `select_plan` step + web `/bookings` need new `PricingDisplay` component with anchoring UI |
| PRICE-02 | Booking wizard shows real-time pod availability from live RaceControl data | `GET /api/v1/fleet/health` exists and returns `ws_connected` + `http_reachable` per pod; kiosk already has `api.fleetHealth()` client method; need new availability summary endpoint + ScarcityBanner component |
| PRICE-03 | System tracks each customer's commitment ladder position and surfaces next-step nudges | `post_session_hooks()` in billing.rs is the right injection point; `queue_notification()` in psychology.rs is the dispatch mechanism; need DB migration + ladder evaluation logic |
| PRICE-04 | Booking page displays real social proof using actual data | `billing_sessions` table has `started_at` timestamps; need new `GET /api/v1/pricing/social-proof` endpoint querying real counts; web `/bookings` page needs SocialProofBar component |
</phase_requirements>

## Summary

Phase 94 adds psychology-driven conversion mechanics to the pricing and booking experience across kiosk and web PWA. The implementation is almost entirely additive — it builds on existing infrastructure without replacing it.

The backend has three key existing assets: (1) `pricing_tiers` + `pricing_rules` tables with `compute_dynamic_price()` already in billing.rs, (2) `GET /api/v1/fleet/health` returning live pod status, (3) `psychology::queue_notification()` + `post_session_hooks()` providing the nudge pipeline. The only new backend work is two new endpoints (pricing display + social proof), one DB migration (commitment_ladder column on drivers), and commitment ladder evaluation logic hooked into `post_session_hooks()`.

On the frontend, the kiosk `select_plan` step in `SetupWizard.tsx` currently renders a flat list of tiers with no visual hierarchy. The web `/bookings` page currently shows only booking history, not a conversion-oriented pricing page. Both need new components: `PricingDisplay` (3-tier anchoring card layout), `ScarcityBanner` (live pod count with color gradient), and `SocialProofBar` (real weekly/daily counts). The web app currently has NO fleet health API calls — that client method needs to be added.

**Primary recommendation:** 3 plans — Plan 01: backend (2 new endpoints + DB migration + ladder logic), Plan 02: kiosk UI (enhance SetupWizard select_plan step), Plan 03: web UI (add pricing page to /book with all three components).

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust/Axum | (project standard) | New backend endpoints | Already the racecontrol server framework |
| SQLite/sqlx | (project standard) | DB migration + queries | Already the project DB layer |
| Next.js App Router | (project standard) | Frontend pages | Already used in kiosk and web |
| SWR | (project standard) | Data fetching with revalidation | Already used for live data in both apps |
| Tailwind CSS | (project standard) | Styling | Already used in both apps |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Sonner | (project standard) | Toast notifications | Already configured in both apps — for waitlist CTA feedback |
| Racing Red #E10600 | — | Brand color | "Most Popular" badge border + active tier highlight |

### No New Dependencies Required
All required libraries are already in the project. This phase adds zero new npm packages or Rust crates.

## Architecture Patterns

### Recommended Project Structure

New files to create:
```
kiosk/src/components/
├── PricingDisplay.tsx        # 3-tier anchoring card layout
└── ScarcityBanner.tsx        # Real-time pod availability

web/src/app/book/
└── page.tsx                  # New customer-facing pricing + booking page
web/src/components/           # (if it exists, otherwise inline)
├── PricingDisplay.tsx        # Same logic, shared or duplicated
├── ScarcityBanner.tsx        # Same availability display
└── SocialProofBar.tsx        # Weekly drivers + today sessions

crates/racecontrol/src/
└── pricing_display.rs        # New module: social proof + pricing display endpoints
```

### Pattern 1: Anchoring Card Layout (3-tier)

**What:** Middle card ("Most Popular") gets visual prominence — larger size, Racing Red border, badge label.
**When to use:** `select_plan` step in kiosk wizard + new `/book` page in web.
**Pattern (verified from existing SetupWizard.tsx and brand guidelines):**

```typescript
// Source: existing SetupWizard.tsx select_plan step + CLAUDE.md brand identity
// tiers sorted by sort_order ASC — middle tier (index 1 of 3) is "Most Popular"
const tiers = activeTiers.filter(t => !t.is_trial || !driver?.has_used_trial);
const mostPopularIndex = Math.floor(tiers.length / 2); // middle tier

tiers.map((tier, idx) => {
  const isPopular = idx === mostPopularIndex && tiers.length === 3;
  return (
    <div className={`relative rounded-xl border-2 p-4 transition-all ${
      isPopular
        ? "border-[#E10600] scale-105 bg-[#E10600]/5"
        : "border-rp-border bg-rp-surface"
    }`}>
      {isPopular && (
        <span className="absolute -top-3 left-1/2 -translate-x-1/2 bg-[#E10600] text-white text-xs font-bold px-3 py-1 rounded-full">
          Most Popular
        </span>
      )}
      {/* strikethrough original + bold current for anchoring */}
      {tier.price_paise !== dynamicPrice && (
        <span className="line-through text-rp-grey text-sm">
          ₹{(tier.price_paise / 100).toFixed(0)}
        </span>
      )}
      <span className="text-xl font-bold text-white">
        {tier.is_trial ? "Free" : `${(dynamicPrice / 100).toFixed(0)} credits`}
      </span>
    </div>
  );
});
```

### Pattern 2: Scarcity Banner from Fleet Health

**What:** Read `ws_connected` + `http_reachable` from `/api/v1/fleet/health` to compute available pods.
**When to use:** Both kiosk plan selection step and web /book page.

```typescript
// Source: packages/shared-types/src/fleet.ts + kiosk/src/lib/api.ts
// kiosk already has: api.fleetHealth() -> FleetHealthResponse
// web does NOT have this yet — add to web/src/lib/api.ts

const available = fleetHealth.filter(p => p.ws_connected && p.http_reachable).length;
const total = fleetHealth.length; // 8 pods

// Color gradient thresholds (Claude's discretion — recommend):
// 5-8 available → green (text-green-400)
// 2-4 available → yellow (text-yellow-400)
// 0-1 available → red (text-[#E10600]) + loss-framed message
```

**Zero-pod state:**
```typescript
// "All pods in use — next slot likely in ~Xmin" with waitlist CTA
// Estimate: average session duration is 30-60min, so show "~30min" when 0 available
// Waitlist CTA: toast notification signup (existing Sonner pattern)
```

### Pattern 3: Social Proof Endpoint

**What:** New `GET /api/v1/pricing/social-proof` returning real counts from billing_sessions.
**When to use:** Called by web /book page and kiosk select_plan step (SWR fetch with 5-min refresh).

```rust
// Source: verified from billing.rs query patterns and routes.rs structure
// Note: racecontrol logs are UTC — use UTC queries, display in IST on frontend
async fn pricing_social_proof(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Drivers who raced in the last 7 days (UTC)
    let drivers_this_week: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT driver_id) FROM billing_sessions
         WHERE status IN ('completed', 'ended_early')
         AND started_at >= datetime('now', '-7 days')"
    ).fetch_one(&state.db).await.unwrap_or(0);

    // Sessions completed today (UTC — frontend converts to IST for display)
    let sessions_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions
         WHERE status IN ('completed', 'ended_early')
         AND date(started_at) = date('now')"
    ).fetch_one(&state.db).await.unwrap_or(0);

    Json(json!({ "drivers_this_week": drivers_this_week, "sessions_today": sessions_today }))
}
```

**IMPORTANT:** This endpoint must be in `public_routes` (no auth required) — customer-facing booking page cannot require a JWT. Verify placement follows the `kiosk_routes` pattern or add to unrestricted routes.

### Pattern 4: Commitment Ladder DB Migration

**What:** Add `commitment_ladder` TEXT column to drivers table with CHECK constraint.
**When to use:** DB migration in `db/mod.rs` (existing ALTER TABLE pattern).

```rust
// Source: verified pattern from db/mod.rs lines 352-395
// Must use ALTER TABLE (not CREATE TABLE) — existing databases don't have the column
let _ = sqlx::query(
    "ALTER TABLE drivers ADD COLUMN commitment_ladder TEXT DEFAULT 'trial'
     CHECK(commitment_ladder IN ('trial', 'single', 'package', 'member'))"
).execute(pool).await;
// Silent failure (let _ =) is correct — column already exists in fresh DBs
```

### Pattern 5: Ladder Evaluation in post_session_hooks

**What:** After each completed session, re-evaluate ladder position and queue nudge if escalation triggered.
**When to use:** Add as step 8 in `post_session_hooks()` in billing.rs.

```rust
// Source: verified from billing.rs post_session_hooks() pattern (lines 2403-2502)
// Ladder evaluation logic:
// 1. Fetch completed session count for this driver
// 2. Determine ladder position from count
// 3. Queue WhatsApp nudge at escalation thresholds

async fn evaluate_commitment_ladder(state: &Arc<AppState>, driver_id: &str) {
    let session_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions
         WHERE driver_id = ? AND status IN ('completed', 'ended_early')
         AND is_trial = 0" // exclude trial from ladder progression
    ).bind(driver_id).fetch_one(&state.db).await.unwrap_or(0);

    let (new_position, should_nudge, nudge_template) = match session_count {
        0     => ("trial",   false, ""),
        1     => ("single",  false, ""),    // first real session — no nudge yet
        2     => ("single",  true,  "package_nudge"),  // 2 sessions → push to package
        3..=4 => ("package", false, ""),   // in package range
        5     => ("package", true,  "membership_nudge"), // 5 sessions → push to membership
        _     => ("member",  false, ""),
    };

    // Update ladder position
    let _ = sqlx::query(
        "UPDATE drivers SET commitment_ladder = ? WHERE id = ?"
    ).bind(new_position).bind(driver_id).execute(&state.db).await;

    // Queue nudge if at escalation point (deduplication: check nudge_queue for recent same template)
    if should_nudge {
        let already_sent: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM nudge_queue
             WHERE driver_id = ? AND template = ?
             AND created_at >= datetime('now', '-7 days')"
        ).bind(driver_id).bind(nudge_template)
         .fetch_one(&state.db).await.unwrap_or(false);

        if !already_sent {
            crate::psychology::queue_notification(
                state, driver_id,
                crate::psychology::NotificationChannel::Whatsapp,
                3, // priority 3 (lower than PB notifications)
                nudge_template,
                "{}"
            ).await;
        }
    }
}
```

### Pattern 6: WhatsApp Nudge Templates

The `template` field in `nudge_queue` maps to message text. Verified from psychology.rs dispatch logic: when `channel = 'whatsapp'`, the template string IS the message body sent to WhatsApp. No template registry needed — it's a direct string.

```rust
// nudge_template for package nudge:
"You've done 2 sessions at RacingPoint! Save 20% with a 5-pack — ask at the counter."

// nudge_template for membership nudge:
"5 sessions in! Become a RacingPoint member for unlimited sessions and priority booking."
```

### Anti-Patterns to Avoid

- **Do not fabricate social proof numbers.** The "No Fake Data" standing rule is absolute — `sessions_today` and `drivers_this_week` must come from real `billing_sessions` queries. If counts are zero (new venue day), display "Be the first today!" not a fake number.
- **Do not add fleet health to web api.ts as an admin route** — the web app's existing `fetchApi` adds an Authorization header. The fleet health endpoint is a public endpoint (no auth needed). Use a raw `fetch` call or add a separate `fetchPublic` helper for the /book page.
- **Do not store ladder position as an integer.** The CHECK constraint enum (trial/single/package/member) prevents invalid states and makes query intent clear.
- **Do not run pricing display on the kiosk pod timer.** SWR `refreshInterval: 30000` (30s) is appropriate for social proof and availability. Real-time for availability can be `refreshInterval: 10000` on the scarcity banner.
- **Do not mark the social-proof endpoint as requiring staff JWT.** It serves the customer-facing /book page.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Real-time availability polling | Custom WebSocket or polling loop | SWR `refreshInterval` | Already used in kiosk live panels; handles revalidation, dedup, error states |
| Nudge deduplication | Custom in-memory set | nudge_queue check with 7-day window | nudge_queue already has created_at; consistent with existing streak_at_risk dedup pattern (psychology.rs:865) |
| WhatsApp message dispatch | Direct HTTP call | `psychology::queue_notification()` | Handles throttling, budget enforcement (daily cap), retry, audit trail |
| Dynamic price fetch | Separate endpoint | Inline in pricing display endpoint | `compute_dynamic_price()` is already an async fn that takes AppState; call it per-tier in the display endpoint |

**Key insight:** The psychology engine's `queue_notification()` function already handles WhatsApp throttling (daily budget), deduplication via nudge_queue, and multi-channel routing. Never bypass it with direct HTTP calls to the WhatsApp bot.

## Common Pitfalls

### Pitfall 1: Social Proof Endpoint Auth
**What goes wrong:** New endpoint added to admin-protected routes returns 401 to unauthenticated /book page visitors.
**Why it happens:** All routes in the main `protected_routes()` block require staff JWT. The /book page has no JWT.
**How to avoid:** Add `GET /pricing/social-proof` and `GET /pricing/display` to `public_routes()` in routes.rs (same pattern as `/kiosk/experiences` GET which is in `kiosk_routes` without admin auth).
**Warning signs:** Frontend console shows 401 on the social proof fetch; booking page shows empty/zero counts.

### Pitfall 2: UTC vs IST in Social Proof Queries
**What goes wrong:** `sessions_today` shows wrong count because `date(started_at) = date('now')` uses UTC while customers are in IST (UTC+5:30).
**Why it happens:** racecontrol stores all timestamps in UTC (per CLAUDE.md warning). A session started at 00:01 IST is stored as 18:31 UTC previous day.
**How to avoid:** Use `datetime('now', '+5:30:00')` for IST "today" boundary, or simply accept slight UTC offset for social proof (it's non-critical display data, not billing).
**Warning signs:** At midnight IST, `sessions_today` resets 5.5 hours before customers notice.

### Pitfall 3: Kiosk vs Web API Auth Difference
**What goes wrong:** Web /book page fails fleet health fetch because web's `fetchApi()` always adds `Authorization: Bearer <token>` header, but fleet health requires staff JWT.
**Why it happens:** Web's `api.ts:fetchApi()` always reads the JWT from `getToken()` and adds the Authorization header (lines 1-28 of web/src/lib/api.ts). The /book page is customer-facing with no JWT.
**How to avoid:** For the /book page's fleet health and social proof fetches, use raw `fetch()` directly (not `fetchApi()`) since these are public endpoints. Add to web api.ts as separate functions that don't use the auth wrapper.
**Warning signs:** Browser console shows 401 on fleet health fetch from /book page.

### Pitfall 4: commitment_ladder Column Not in Cloud DB
**What goes wrong:** Cloud racecontrol on Bono VPS doesn't get the new column; cloud sync queries fail with "no such column: commitment_ladder".
**Why it happens:** DB migrations run at server startup (`db/mod.rs`). Cloud binary must be rebuilt + redeployed to run the migration. The `ALTER TABLE ... ADD COLUMN` pattern is idempotent (let _ = ignores error if column exists).
**How to avoid:** Follow Cross-Process Updates standing rule — after deploying updated binary to server .23, also rebuild + deploy to Bono VPS. The `ALTER TABLE` migration uses `let _ =` so running on a DB that already has the column is safe.
**Warning signs:** Cloud API returns error on any query that includes `commitment_ladder` column.

### Pitfall 5: Flat List vs. Anchoring Layout in Kiosk
**What goes wrong:** The existing `select_plan` step renders tiers as a flat list (current code: lines 318-336 of SetupWizard.tsx). Just styling the middle item differently may not create enough visual hierarchy on a pod's vertical display.
**Why it happens:** The kiosk uses a compact vertical layout designed for the SetupWizard drawer — cards are designed narrow. The "slightly larger card" anchoring effect needs careful sizing to avoid overflow.
**How to avoid:** Keep the 3-tier layout within the existing wizard container width. Use `scale-105` + prominent border instead of actually increasing padding. Test on kiosk pod display (1920px width, wizard drawer width).
**Warning signs:** Middle card overflows wizard container or pushes other cards off-screen.

### Pitfall 6: is_trial Tier Included in 3-Tier Anchoring
**What goes wrong:** If `pricing_tiers` has 4 rows (trial + 3 paid tiers), the "middle tier" calculation picks the wrong tier, and the trial card appears in the anchoring display.
**Why it happens:** The current SetupWizard filters `is_trial` only if `has_used_trial` is true. For the anchoring layout, trials should always be excluded from the 3-tier count (trial is a separate CTA, not a paid tier in the anchor).
**How to avoid:** Filter out `is_trial = true` tiers before building the anchoring layout. Show trial as a separate "Try for Free" button below the 3-tier display (only if driver hasn't used it).
**Warning signs:** Anchoring shows "Free / 30 Minutes / 1 Hour" instead of "30 Minutes / 60 Minutes / Package".

## Code Examples

Verified patterns from codebase investigation:

### Existing: compute_dynamic_price in billing.rs
```rust
// Source: crates/racecontrol/src/billing.rs:16-53
// Returns adjusted price in paise given base price — uses pricing_rules table
let dynamic_paise = compute_dynamic_price(&state, tier.price_paise).await;
```

### Existing: queue_notification signature
```rust
// Source: crates/racecontrol/src/psychology.rs:354-378
pub async fn queue_notification(
    state: &Arc<AppState>,
    driver_id: &str,
    channel: NotificationChannel, // Whatsapp, Discord, Pwa
    priority: i32,                // 1=highest, 5=lowest
    template: &str,               // The message body string
    payload_json: &str,           // Extra JSON context "{}"
)
```

### Existing: post_session_hooks hook pattern
```rust
// Source: crates/racecontrol/src/billing.rs:2403-2502
// This is WHERE to add ladder evaluation (after step 7, as step 8)
async fn post_session_hooks(state: &Arc<AppState>, session_id: &str, driver_id: &str) {
    // Steps 1-7 already exist...
    // Step 8: Evaluate commitment ladder and queue escalation nudge
    evaluate_commitment_ladder(state, driver_id).await;
}
```

### Existing: kiosk api.fleetHealth()
```typescript
// Source: kiosk/src/lib/api.ts:49
fleetHealth: () => fetchApi<FleetHealthResponse>("/fleet/health"),
// FleetHealthResponse = { pods: PodFleetStatus[] }
// PodFleetStatus fields: pod_number, ws_connected, http_reachable, version, build_id, uptime_secs, last_seen
```

### Existing: SWR pattern for live data (kiosk)
```typescript
// Source: kiosk convention — infer from SetupWizard.tsx useEffect pattern
// For availability banner, use SWR:
import useSWR from 'swr';
const { data: fleet } = useSWR('/fleet/health', fetcher, { refreshInterval: 10000 });
const available = fleet?.pods?.filter(p => p.ws_connected && p.http_reachable).length ?? 0;
```

### New: DB migration pattern for commitment_ladder
```rust
// Source: db/mod.rs pattern (lines 352-395) — idempotent ALTER TABLE
let _ = sqlx::query(
    "ALTER TABLE drivers ADD COLUMN commitment_ladder TEXT DEFAULT 'trial' \
     CHECK(commitment_ladder IN ('trial', 'single', 'package', 'member'))"
).execute(pool).await;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat tier list in SetupWizard | 3-tier anchoring with "Most Popular" | Phase 94 | Higher conversion via decoy effect |
| No availability display | Real-time "X of 8 pods" with scarcity | Phase 94 | Loss aversion when pods are limited |
| No post-session upsell | Commitment ladder WhatsApp nudges | Phase 94 | Natural progression: trial→single→package→member |
| No social proof | Real counts from billing_sessions | Phase 94 | Social validation at decision moment |

**No deprecated patterns:** All existing APIs and components being extended remain valid.

## Open Questions

1. **Where does the web /book page live?**
   - What we know: There is currently no `web/src/app/book/` directory. The `/bookings` page is admin-facing session history. The context says "web PWA /book page" but this directory does not exist yet.
   - What's unclear: Does /book need to be a customer-facing unauthenticated page, or is it behind the admin login?
   - Recommendation: Create `web/src/app/book/page.tsx` as a new public (no-auth) customer-facing pricing page. The existing `/bookings` route stays as-is for admin history.

2. **Is the web PWA the admin dashboard or a customer-facing PWA?**
   - What we know: `web/src/app/billing/page.tsx` uses `DashboardLayout` and requires admin JWT. The "web PWA" mentioned in CONTEXT.md may refer to the customer-facing web app on a different port/app.
   - What's unclear: The CONTEXT.md says "kiosk booking wizard AND web PWA /book page" but the web app at :3200 is the admin dashboard.
   - Recommendation: Treat `web/src/app/book/` as a new page added to the admin dashboard (same app at :3200) but make it publicly accessible without JWT. This is consistent with the existing `kiosk/` subdirectory pattern in web/src/app/.

3. **Ladder trigger thresholds — are "5-pack packages" tracked?**
   - What we know: The `commitment_ladder` tracks session counts. The "Package (5-pack)" tier may be a `pricing_tiers` entry, not a separate table.
   - What's unclear: How to distinguish "driver bought a 5-pack" vs "driver paid for 5 separate sessions." The session count approach counts sessions, not purchase type.
   - Recommendation: Use session count as the ladder proxy (2 completed sessions = package nudge; 5 completed = membership nudge). This is what the CONTEXT.md specifies and is implementable with existing data.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` + Vitest (kiosk) |
| Config file | kiosk/package.json (`"test": "vitest run"`) |
| Quick run command | `cargo test -p racecontrol -- psychology billing 2>&1 \| tail -20` |
| Full suite command | `cargo test -p racecontrol && cd kiosk && npm test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PRICE-01 | PricingDisplay renders 3 tiers with middle emphasized | unit (Vitest) | `cd kiosk && npm test -- PricingDisplay` | ❌ Wave 0 |
| PRICE-02 | ScarcityBanner shows correct available count from fleet data | unit (Vitest) | `cd kiosk && npm test -- ScarcityBanner` | ❌ Wave 0 |
| PRICE-03 | evaluate_commitment_ladder queues nudge at session_count=2 | unit (cargo test) | `cargo test -p racecontrol -- test_commitment_ladder` | ❌ Wave 0 |
| PRICE-04 | /pricing/social-proof returns real counts, not fabricated | integration (cargo test) | `cargo test -p racecontrol -- test_social_proof_endpoint` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- psychology billing 2>&1 | tail -20`
- **Per wave merge:** `cargo test -p racecontrol && cd kiosk && npm test`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `kiosk/src/components/__tests__/PricingDisplay.test.tsx` — covers PRICE-01 (3-tier rendering, middle emphasis)
- [ ] `kiosk/src/components/__tests__/ScarcityBanner.test.tsx` — covers PRICE-02 (available count, color thresholds)
- [ ] Test in `crates/racecontrol/src/psychology.rs` (existing tests mod) — covers PRICE-03 (ladder evaluation at 2 and 5 sessions)
- [ ] Integration test in `crates/racecontrol/src/api/routes.rs` (existing tests mod) — covers PRICE-04 (social proof endpoint returns non-negative counts)

## Sources

### Primary (HIGH confidence)
- Direct codebase inspection — `crates/racecontrol/src/billing.rs` (compute_dynamic_price, post_session_hooks)
- Direct codebase inspection — `crates/racecontrol/src/psychology.rs` (queue_notification, nudge_queue, dispatcher)
- Direct codebase inspection — `crates/racecontrol/src/api/routes.rs` (existing pricing endpoints, fleet health route)
- Direct codebase inspection — `crates/racecontrol/src/db/mod.rs` (ALTER TABLE migration pattern)
- Direct codebase inspection — `kiosk/src/components/SetupWizard.tsx` (existing select_plan step)
- Direct codebase inspection — `packages/shared-types/src/billing.ts` and `driver.ts` (PricingTier, Driver types)
- Direct codebase inspection — `kiosk/src/lib/api.ts` (fleetHealth client method)
- CLAUDE.md — brand colors (#E10600), No Fake Data rule, UTC/IST warning, DB migration rules

### Secondary (MEDIUM confidence)
- 94-CONTEXT.md (user decisions) — locked implementation choices
- ROADMAP.md Phase 94 success criteria — requirement definitions

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in project, versions verified
- Architecture: HIGH — all integration points verified against actual source code
- Pitfalls: HIGH — sourced from standing rules in CLAUDE.md and actual code patterns
- Ladder thresholds: MEDIUM — session count thresholds (2 and 5) derived from CONTEXT.md spec, not empirical data

**Research date:** 2026-03-24 IST
**Valid until:** 2026-04-24 (stable codebase, no fast-moving dependencies)
