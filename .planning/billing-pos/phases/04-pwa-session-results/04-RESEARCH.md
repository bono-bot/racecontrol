# Phase 4: PWA Session Results & Receipt - Research

**Researched:** 2026-03-14
**Domain:** Customer-facing session detail (Rust/Axum API, Next.js PWA, WhatsApp via comms-link)
**Confidence:** HIGH

## Summary

Phase 4 builds on a strong existing foundation. The PWA already has a `/sessions/[id]` page (`pwa/src/app/sessions/[id]/page.tsx`, 800 lines) that displays: a receipt card with original price, discount, charged amount, refund, and net cost; session stats (total laps, best lap, average lap); a lap-by-lap telemetry chart; and a share button. The backend `customer_session_detail` handler (routes.rs lines 3459-3588) already returns all billing fields needed for PWA-01 (cost breakdown) and most of PWA-02 (performance). The `billing_session_events` handler (routes.rs lines 2182-2210) already returns all lifecycle events for PWA-03 (timeline).

The main work needed is: (1) adding the events timeline to the session detail page (currently missing from PWA), (2) adding a public shareable route `/sessions/[id]/public` for PWA-05, (3) wiring a WhatsApp receipt via Bono's comms-link for PWA-04, and (4) adding top speed to the session detail response (currently only tracked in rc-agent in-memory, not persisted). The billing_events table already has all the event data needed; the PWA just doesn't fetch or display it.

**Primary recommendation:** Extend the existing `customer_session_detail` handler to include events timeline in the same response (avoid a second API call). Add a new public endpoint `GET /public/sessions/{id}` (no auth). Wire `post_session_hooks` to send a WhatsApp receipt via comms-link to Bono. Mark top speed as "N/A" in the PWA since it is not persisted in the database (rc-agent tracks it in memory only).

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PWA-01 | Customer views session cost breakdown: base price, discount, final charged, refund | **95% DONE.** `customer_session_detail` already returns `price_paise`, `discount_paise`, `original_price_paise`, `discount_reason`, `wallet_debit_paise`, `refund_paise`. PWA `[id]/page.tsx` already renders Receipt card with all these fields (lines 454-498). Only minor labeling changes needed. |
| PWA-02 | Customer views session performance: total laps, best lap time, top speed | **Partially done.** `customer_session_detail` returns `total_laps`, `best_lap_ms`, `average_lap_ms`. PWA renders these in StatTile grid (lines 502-524). **Gap:** Top speed is NOT in the database. rc-agent tracks `session_max_speed_kmh` in memory during gameplay but never persists it. Options: (A) add it to laps table, (B) show N/A. Requirement says "where telemetry available -- N/A otherwise" which covers option B. |
| PWA-03 | Customer views session timeline: start, pauses, warnings, end | **Backend done, frontend missing.** `billing_session_events` handler (GET `/billing/sessions/{id}/events`) returns all events. Event types in billing.rs include: `created`, `started`, `time_expired`, `ended_early`, `cancelled`, `ended`, `paused_disconnect`, `pause_timeout_ended`, `resumed_disconnect`, `extended`, `paused_manual`, `resumed`. PWA page does NOT fetch or display events currently. |
| PWA-04 | WhatsApp receipt within 60s of session end | **Not implemented.** `post_session_hooks` (billing.rs line 1997) runs after session end but only does: referral rewards, review nudge, membership hours. Must add a 4th hook to send WhatsApp via Bono's comms-link. Bono's VPS has Evolution API configured. Comms-link uses WebSocket with `send-message.js` one-shot sender. Alternative: HTTP POST to Bono (simpler than WebSocket from Rust). |
| PWA-05 | Shareable public link `/sessions/{id}/public` with name, duration, best lap | **Not implemented.** No public session endpoint exists. Existing public endpoints (`/public/leaderboard`, `/public/time-trial`) show the pattern: no auth header required, uses `publicApi` in api.ts. Need: backend `GET /public/sessions/{id}` + PWA route `/sessions/[id]/public/page.tsx`. |
</phase_requirements>

## Standard Stack

### Core (already in use -- do NOT add new dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.7.x | HTTP server + routes | Already powers rc-core API |
| sqlx | 0.7.x | SQLite async queries | Already used for all DB ops |
| reqwest | 0.12.x | HTTP client (for webhook to Bono) | Already used in cloud_sync.rs and auth OTP |
| serde_json | 1.x | JSON payload construction | Already used throughout |
| next | 16.1.6 | PWA framework | Already in pwa/package.json |
| react | 19.2.3 | UI rendering | Already in pwa/package.json |
| tailwindcss | 4.x | Styling | Already in pwa/package.json |

### No new dependencies needed

Both backend (Rust) and frontend (Next.js) have everything required. The WhatsApp receipt goes through Bono via HTTP webhook -- reqwest already handles HTTP calls. No new npm packages or Rust crates needed.

## Architecture Patterns

### Existing Session Detail Architecture
```
Customer PWA                    rc-core (venue, port 8080)
    |                                    |
    |-- GET /customer/sessions/{id} ---> customer_session_detail()
    |   (auth: Bearer JWT)               |-- billing_sessions JOIN pricing_tiers
    |                                    |-- discount_paise, refund_paise
    |                                    |-- laps JOIN drivers
    |                                    |-- Returns: session + laps JSON
    |
    |-- GET /billing/sessions/{id}/events (admin route, not used by PWA)
    |                                    |-- billing_events WHERE billing_session_id
    |                                    |-- Returns: events[] JSON
```

### Recommended Architecture for Phase 4
```
Customer PWA                    rc-core (venue, port 8080)
    |                                    |
    |-- GET /customer/sessions/{id} ---> customer_session_detail()
    |   (auth: Bearer JWT)               |-- EXISTING session + laps
    |                                    |-- NEW: billing_events query added
    |                                    |-- Returns: { session, laps, events }
    |
    |-- GET /public/sessions/{id} -----> public_session_summary()
    |   (no auth)                        |-- billing_sessions + laps (limited fields)
    |                                    |-- Returns: { name, duration, best_lap }
    |
end_billing_session()                  post_session_hooks()
    |                                    |-- NEW: send_whatsapp_receipt()
    |                                    |    |-- HTTP POST to Bono's VPS
    |                                    |    |-- Bono formats + sends via Evolution API
```

### Pattern: Adding Events to customer_session_detail
Instead of making the PWA do a second API call for events, add the events query directly to `customer_session_detail()`. This follows the existing pattern where session + laps are fetched in one handler. The events query is small (10-20 rows max per session).

```rust
// In customer_session_detail() after laps query:
let events = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
    "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
     FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
)
.bind(&id)
.fetch_all(&state.db)
.await
.unwrap_or_default();

// Add to response JSON:
"events": events_json,
```

### Pattern: Public Endpoint (No Auth)
Follow the existing public endpoint pattern at routes.rs lines 244-247:

```rust
// In api_routes():
.route("/public/sessions/{id}", get(public_session_summary))

// Handler -- no extract_driver_id, no auth:
async fn public_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Query billing_sessions + drivers (name only) + laps (best lap only)
    // Return minimal fields: driver first name, duration, best lap, track, car
}
```

### Pattern: WhatsApp Receipt via Bono Webhook
The venue rc-core sends an HTTP POST to Bono's VPS, which then formats and sends via Evolution API. This avoids rc-core needing Evolution API credentials directly (Bono already has them) and follows the separation of concerns (James = venue ops, Bono = cloud/messaging).

**Webhook endpoint on Bono's side:** `POST https://app.racingpoint.cloud:8080/webhook/session-receipt` or similar. Bono would need to implement this receiver.

**Alternative (simpler):** Use comms-link's `task_request` message type. James sends a task via WebSocket with receipt data, Bono processes it. But this requires Node.js on the venue side (comms-link) -- rc-core is Rust.

**Recommended approach:** HTTP POST from rc-core to Bono's VPS. rc-core already uses reqwest for cloud sync (cloud_sync.rs), so adding one more HTTP call is trivial. Configure the webhook URL in `racecontrol.toml` under `[integrations.whatsapp]`.

```rust
// In post_session_hooks(), after membership hours update:
// 4. Send WhatsApp receipt via Bono
if state.config.integrations.whatsapp.enabled {
    if let Some(webhook_url) = &state.config.integrations.whatsapp.webhook_url {
        let phone = get_driver_phone(state, driver_id).await;
        if let Some(phone) = phone {
            let receipt_data = gather_receipt_data(state, session_id, driver_id).await;
            let _ = reqwest::Client::new()
                .post(format!("{}/webhook/session-receipt", webhook_url))
                .json(&receipt_data)
                .send()
                .await;
        }
    }
}
```

### Anti-Patterns to Avoid
- **Do NOT add Evolution API credentials to rc-core config.** Bono's VPS already has them. Duplicating credentials creates a security and maintenance burden. Use a webhook to Bono instead.
- **Do NOT use comms-link WebSocket from Rust.** The comms-link is Node.js with a specific protocol. Adding a WebSocket client to rc-core for one message type is over-engineered. Use HTTP POST instead.
- **Do NOT make a separate API call for events in the PWA.** Add events to the existing `customer_session_detail` response to avoid a waterfall of API calls.
- **Do NOT try to persist top speed retroactively.** The laps table has no `top_speed_kmh` column, and rc-agent's in-memory tracking is ephemeral. For now, show "N/A" as the requirement permits. Add persistence in a future phase if needed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WhatsApp sending | Direct Evolution API call from rc-core | HTTP webhook to Bono's VPS | Bono already has Evolution API credentials; avoids credential duplication |
| Session timeline | Custom timeline component from scratch | Simple ordered list with event type icons | Billing_events already has all data; just render it chronologically |
| Public session page auth bypass | Custom middleware for public routes | Follow existing `/public/*` route pattern (no auth) | Pattern already proven with leaderboard, time-trial |
| Receipt formatting | Rust HTML template | Let Bono format the WhatsApp message | Bono knows WhatsApp formatting rules (Evolution API quirks) |

**Key insight:** Almost all the data layer work is done. This phase is 80% frontend and 20% wiring.

## Common Pitfalls

### Pitfall 1: CORS for Public Endpoint
**What goes wrong:** The public session page calls an API endpoint from a different origin (customer's phone browser), and CORS blocks the request.
**Why it happens:** rc-core's CORS config may only allow the PWA's origin, not arbitrary origins needed for shared links.
**How to avoid:** Ensure the `/public/*` routes have permissive CORS (Access-Control-Allow-Origin: *). Check existing public endpoint CORS handling.
**Warning signs:** "CORS error" in browser console when opening a shared link.

### Pitfall 2: WhatsApp Webhook Timeout in post_session_hooks
**What goes wrong:** `post_session_hooks` is fire-and-forget (`tokio::spawn`), but the reqwest call to Bono's VPS could hang if the VPS is down, blocking the spawned task.
**Why it happens:** No timeout on the HTTP client call.
**How to avoid:** Use `reqwest::Client::builder().timeout(Duration::from_secs(5)).build()` or `.timeout(Duration::from_secs(5))` on the request. The receipt is best-effort -- never block session end.
**Warning signs:** Accumulating spawned tasks that never complete.

### Pitfall 3: Missing Driver Phone Number
**What goes wrong:** WhatsApp receipt tries to send but the driver has no phone number on file.
**Why it happens:** Some drivers may have been created via PIN auth without phone registration.
**How to avoid:** Check phone exists before attempting to send. Log a warning if no phone. The requirement says "customer receives a WhatsApp message" -- this only works if they have a phone on file.
**Warning signs:** Silent receipt failures for phone-less drivers.

### Pitfall 4: Next.js Hydration with Public vs Auth Pages
**What goes wrong:** The public page `/sessions/[id]/public` shouldn't redirect to login, but the existing session detail page (`/sessions/[id]`) does. If routing is wrong, public users get redirected.
**Why it happens:** `isLoggedIn()` check in `useEffect` redirects unauthenticated users.
**How to avoid:** The public page is a separate route (`/sessions/[id]/public/page.tsx`) that does NOT call `isLoggedIn()` and uses `publicApi` (no auth headers).
**Warning signs:** Shared links redirect to login page.

### Pitfall 5: Event Type Display Names
**What goes wrong:** Raw event types like `paused_disconnect` or `pause_timeout_ended` are shown to customers.
**Why it happens:** Forgetting to map internal event types to customer-friendly labels.
**How to avoid:** Create a mapping in the PWA:
```typescript
const eventLabels: Record<string, string> = {
  created: "Session Created",
  started: "Session Started",
  paused_manual: "Paused",
  paused_disconnect: "Paused (Disconnected)",
  resumed: "Resumed",
  resumed_disconnect: "Reconnected",
  ended: "Session Completed",
  ended_early: "Session Ended Early",
  cancelled: "Session Cancelled",
  time_expired: "Time Expired",
  extended: "Time Extended",
  warning_5min: "5 Minute Warning",
  warning_1min: "1 Minute Warning",
};
```
**Warning signs:** Technical jargon visible to customers.

### Pitfall 6: Bono Webhook Endpoint Does Not Exist Yet
**What goes wrong:** rc-core sends HTTP POST to Bono's VPS but there is no endpoint to receive it.
**Why it happens:** Bono (partner AI on VPS) needs to implement the `/webhook/session-receipt` handler. This is a cross-system dependency.
**How to avoid:** Plan 04-03 must coordinate with Bono. Either: (A) define the webhook spec and email Bono to implement it, or (B) use Evolution API directly from rc-core (rc-core already does this for OTP -- see auth/mod.rs lines 988-1022).
**Warning signs:** HTTP 404 responses from Bono's VPS.

**IMPORTANT DECISION POINT:** The requirement says "via Bono (Evolution API)". Two viable approaches:
1. **Webhook to Bono:** James sends receipt data, Bono sends WhatsApp. Requires Bono to implement an endpoint. Pro: separation of concerns. Con: dependency on Bono.
2. **Direct Evolution API from rc-core:** rc-core already sends WhatsApp OTPs via Evolution API (auth/mod.rs). The same pattern works for receipts. Pro: no external dependency, already proven. Con: duplicates Evolution API config (but it's already there for OTP).

**Recommendation:** Use direct Evolution API from rc-core (option 2). The pattern is already implemented for OTP sending. The config fields (`evolution_url`, `evolution_api_key`, `evolution_instance`) are already in AuthConfig. Move them to a shared config or reference them from IntegrationsConfig. This eliminates the Bono dependency entirely.

## Code Examples

### Existing Receipt Card (PWA -- already working)
```tsx
// Source: pwa/src/app/sessions/[id]/page.tsx lines 454-498
// Already renders: Plan name, Original price, Discount, Charged, Refund, Net Cost
// This satisfies PWA-01 with only minor label tweaks
<div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
  <h2 className="text-sm font-medium text-rp-grey mb-3">Receipt</h2>
  <ReceiptRow label="Plan" value={session.pricing_tier_name} />
  {session.discount_paise > 0 && (
    <>
      <ReceiptRow label="Original" value={formatCredits(session.original_price_paise)} />
      <ReceiptRow label="Discount" value={`-${formatCredits(session.discount_paise)}`} accent="green" />
    </>
  )}
  <ReceiptRow label="Charged" value={formatCredits(session.wallet_debit_paise)} />
  {session.refund_paise > 0 && (
    <ReceiptRow label="Refund" value={`+${formatCredits(session.refund_paise)}`} accent="green" />
  )}
  <ReceiptRow label="Net Cost" value={formatCredits(netCharged)} bold />
</div>
```

### Existing Events Endpoint (Backend -- already working)
```rust
// Source: crates/rc-core/src/api/routes.rs lines 2182-2210
// GET /billing/sessions/{id}/events -- admin route, returns all events
// Same query can be added to customer_session_detail
async fn billing_session_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
        "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
         FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;
    // ...
}
```

### Existing WhatsApp OTP Sending Pattern (rc-core)
```rust
// Source: crates/rc-core/src/auth/mod.rs lines 988-1022
// This pattern can be reused for WhatsApp receipts
if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
    &state.config.auth.evolution_url,
    &state.config.auth.evolution_api_key,
    &state.config.auth.evolution_instance,
) {
    let wa_phone = format_phone_for_whatsapp(phone);
    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({
        "number": wa_phone,
        "text": receipt_message
    });
    let client = reqwest::Client::new();
    let _ = client.post(&url).header("apikey", evo_key).json(&body).send().await;
}
```

### Existing Public API Pattern (PWA)
```typescript
// Source: pwa/src/lib/api.ts lines 900-917
// publicApi uses raw fetch without auth headers
export const publicApi = {
  leaderboard: () =>
    fetch(`${API_BASE_URL}/public/leaderboard`).then(r => r.json()),
  // Same pattern for public session summary
};
```

### Existing Public Route Pattern (Backend)
```rust
// Source: crates/rc-core/src/api/routes.rs lines 244-247
// Public routes under /public/* -- no auth middleware
.route("/public/leaderboard", get(public_leaderboard))
.route("/public/leaderboard/{track}", get(public_track_leaderboard))
// Add: .route("/public/sessions/{id}", get(public_session_summary))
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No session receipt in PWA | Receipt card exists with full cost breakdown | Already implemented | PWA-01 is ~95% done |
| No events timeline | billing_events table + admin endpoint exist | Already implemented | Backend ready, frontend needed |
| No WhatsApp receipt | OTP via Evolution API exists | Already implemented | Same pattern for receipts |
| No public sharing | Share button with native share API exists | Already implemented | Share report exists, public page needed |

**Already done (do NOT rebuild):**
- Receipt card in `[id]/page.tsx` (lines 454-498) -- renders cost breakdown
- Session stats tiles (lines 502-524) -- renders total laps, best lap, avg lap
- Lap chart and lap table (lines 527-666) -- CSS bar chart + detailed table
- Share button and share modal (lines 300-377) -- native share + report card
- `customer_session_detail` handler -- returns session + laps with all billing fields
- `billing_session_events` handler -- returns all lifecycle events
- `post_session_hooks` -- fire-and-forget hook system after session end
- Evolution API integration for WhatsApp OTP -- same pattern for receipts

## Open Questions

1. **WhatsApp receipt: via Bono webhook or direct Evolution API?**
   - What we know: The requirement says "via Bono (Evolution API)". rc-core ALREADY sends WhatsApp messages via Evolution API for OTP (auth/mod.rs). The config fields exist. Bono does NOT have a webhook endpoint to receive receipt data.
   - What's unclear: Does Uday specifically want Bono involved, or is "via Evolution API" the key requirement?
   - Recommendation: Use direct Evolution API from rc-core (already proven pattern). If Bono involvement is required, define webhook spec and email Bono to implement. The planner should choose option 2 (direct) unless Uday specifies otherwise -- it has zero external dependencies and the pattern is already coded.

2. **Top speed: persist or show N/A?**
   - What we know: rc-agent tracks `session_max_speed_kmh` in memory during gameplay. It is included in the `SessionEnded` message payload. But it is never written to the database. The laps table has no `top_speed_kmh` column.
   - What's unclear: How important is top speed to the customer experience? The requirement says "where telemetry available -- N/A otherwise."
   - Recommendation: Show "N/A" for now with a note "Available during live sessions only." To persist top speed, a future change would: (a) add `top_speed_kmh REAL` column to billing_sessions, (b) write it in end_billing_session, (c) read from billing_sessions in customer_session_detail. This is out of scope for Phase 4 unless explicitly requested.

3. **Public page privacy: first name only or full name?**
   - What we know: The requirement says "name, duration, best lap only." Sharing full name publicly raises privacy concerns.
   - Recommendation: Show first name only (or nickname if set). The existing share report uses `driver_name` (full name) but that's opt-in via a share button. A public URL should be more conservative.

4. **WhatsApp config: reuse auth config or add new config?**
   - What we know: Evolution API config is currently inside `AuthConfig` (auth.evolution_url, etc.) because it was originally added for OTP only. IntegrationsConfig.WhatsAppConfig exists but only has `enabled` and `contact` fields.
   - Recommendation: Reference the existing auth config fields for now. Do not duplicate config. A future refactor could move Evolution API config to a shared location, but that is unnecessary for this phase.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `#[test]` + cargo test (backend), no frontend tests |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p rc-core -- billing` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PWA-01 | customer_session_detail returns cost breakdown fields | unit | `cargo test -p rc-core -- customer_session_detail` | Implicit (handler already tested via existing tests) |
| PWA-02 | customer_session_detail returns performance fields | unit | `cargo test -p rc-core -- customer_session_detail` | Implicit |
| PWA-03 | customer_session_detail returns events timeline | unit | `cargo test -p rc-core -- customer_session_detail_events` | No -- Wave 0 |
| PWA-04 | post_session_hooks sends WhatsApp receipt | unit | `cargo test -p rc-core -- whatsapp_receipt` | No -- Wave 0 |
| PWA-05 | public_session_summary returns limited fields without auth | unit | `cargo test -p rc-core -- public_session_summary` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-core -- billing`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Test for events inclusion in `customer_session_detail` response
- [ ] Test for `public_session_summary` returning only public-safe fields (no auth required, limited data)
- [ ] Test for WhatsApp receipt formatting (message text, phone number formatting)
- [ ] Note: Frontend (PWA) has no test framework -- verification is manual (visual inspection in browser)

## Sources

### Primary (HIGH confidence)
- `pwa/src/app/sessions/[id]/page.tsx` -- read in full (800 lines). Existing session detail page with receipt, stats, laps, share.
- `pwa/src/lib/api.ts` -- read in full (931 lines). All API client types and calls, including SessionDetailSession interface.
- `crates/rc-core/src/api/routes.rs` -- read relevant sections: customer_session_detail (3459-3588), billing_session_events (2182-2210), customer_session_share (7252-7467), public routes (244-247), api_routes (27-260).
- `crates/rc-core/src/billing.rs` -- read end_billing_session (1748-1994), post_session_hooks (1997-2084).
- `crates/rc-core/src/auth/mod.rs` -- read OTP WhatsApp sending (988-1022).
- `crates/rc-core/src/config.rs` -- read IntegrationsConfig, WhatsAppConfig, AuthConfig (evolution fields).
- `crates/rc-core/src/db/mod.rs` -- read laps schema (82-97), billing_events schema (252-261).
- `pwa/src/app/globals.css` -- read Tailwind theme config (rp-red, rp-card, etc.).

### Secondary (MEDIUM confidence)
- `comms-link/shared/protocol.js` -- message types and createMessage function.
- `comms-link/send-message.js` -- one-shot WebSocket message sender.
- Memory files: `comms-link-launch.md` -- comms-link architecture and connection details.

### Tertiary (LOW confidence)
- Top speed persistence: based on grep results showing rc-agent tracks `session_max_speed_kmh` in main.rs but never writes to DB. Could not verify all code paths.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies
- Architecture: HIGH -- existing patterns (public endpoints, customer session detail, Evolution API) are directly reusable
- Pitfalls: HIGH -- identified from direct code reading, all verified against actual implementation
- PWA-01 (cost breakdown): HIGH -- already implemented, verified by reading page.tsx
- PWA-02 (performance): HIGH -- partially done, top speed gap well-understood
- PWA-03 (timeline): HIGH -- backend complete, frontend gap clearly scoped
- PWA-04 (WhatsApp): MEDIUM -- Evolution API pattern exists but webhook-vs-direct decision pending
- PWA-05 (public page): HIGH -- pattern established by existing public endpoints

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable codebase, no expected upstream changes)
