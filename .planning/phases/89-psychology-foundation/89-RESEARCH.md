# Phase 89: Psychology Foundation - Research

**Researched:** 2026-03-21
**Domain:** Rust (Axum) backend module design, SQLite schema extension, notification routing architecture
**Confidence:** HIGH

## Summary

Phase 1 creates the centralized psychology engine inside the existing RaceControl Rust/Axum server at `/root/racecontrol`. The work is entirely backend: a new `psychology.rs` module, 6 new SQLite tables, a notification priority queue with channel routing, and a global notification budget enforcer. No frontend changes are needed in this phase.

The existing codebase already has strong patterns to follow: `billing.rs` demonstrates how modules interact with `AppState`, `db/mod.rs` shows the idempotent migration pattern (CREATE TABLE IF NOT EXISTS + ALTER TABLE with error suppression), `post_session_hooks()` shows the trigger-on-event pattern, and `send_whatsapp_receipt()` shows the Evolution API integration for WhatsApp messaging. The notification dispatch must route through WhatsApp (Evolution API already configured in `racecontrol.toml`), Discord (webhook URL in config), and PWA (existing WebSocket broadcast channel via `dashboard_tx`).

The critical constraint is the 2-message-per-day throttle for proactive WhatsApp messages. This must be enforced at the system level in RaceControl, not in individual bots. The existing `rateLimiter.js` in the WhatsApp bot handles per-minute inbound spam protection -- it is NOT the right place for outbound proactive message throttling.

**Primary recommendation:** Build `psychology.rs` as a single new module in `/root/racecontrol/crates/racecontrol/src/` following the exact patterns of `billing.rs` and `bot_coordinator.rs`. All 6 tables go into the existing `db/mod.rs` migration function. Notification dispatch uses the existing Evolution API config for WhatsApp, Discord webhook for Discord, and `dashboard_tx` broadcast for PWA real-time.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FOUND-01 | System enforces a global notification budget (max 2 proactive WhatsApp messages per customer per day) | nudge_queue table with daily_count tracking per driver + channel; WhatsApp sending goes through queue gatekeeper that checks count before dispatch |
| FOUND-02 | Psychology engine module (psychology.rs) centralizes badge evaluation, streak tracking, XP, and notification dispatch | New module following billing.rs/bot_coordinator.rs patterns; registered in lib.rs; accesses AppState.db for all operations |
| FOUND-03 | Badge criteria stored as JSON in database for no-code extensibility | achievements table with criteria_json TEXT column containing evaluation rules as JSON; psychology.rs evaluates at runtime via serde_json::Value |
| FOUND-04 | Notification priority queue with channel routing (WhatsApp, Discord, PWA push) | nudge_queue table with priority, channel, status columns; background tokio task drains queue respecting throttles; routes to Evolution API / Discord webhook / dashboard_tx |
| FOUND-05 | DB schema for psychology tables (achievements, streaks, driving_passport, nudge_queue, staff_badges, staff_challenges) | 6 CREATE TABLE statements added to db/mod.rs migrate() function following existing idempotent pattern |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8 | SQLite database (already in Cargo.toml) | Already used throughout RaceControl |
| serde / serde_json | workspace | JSON serialization for badge criteria | Already used throughout RaceControl |
| chrono | workspace | Date/time for streaks, throttle windows | Already used throughout RaceControl |
| tokio | workspace | Async runtime, background task spawning | Already used throughout RaceControl |
| uuid | workspace | Primary key generation | Already used throughout RaceControl |
| reqwest | 0.12 | HTTP client for Evolution API + Discord webhook | Already used for WhatsApp receipts and cloud sync |
| axum | 0.8 | Web framework for API endpoints | Already used throughout RaceControl |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing | workspace | Structured logging | All psychology module operations |
| rand | workspace | Random number generation | Variable reward probability checks (Phase 4, but foundation should include the trait) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| In-process notification queue | Redis/RabbitMQ | Overkill for single-server SQLite setup; adds operational dependency |
| Separate notification microservice | Embedded module | Constraint says no new PM2 processes; embedded is correct |
| JSON badge criteria | Lua/WASM scripting | Massively over-engineered for < 1000 customers |

**Installation:**
No new dependencies needed. All libraries are already in the workspace Cargo.toml. Psychology module uses existing dependencies only.

## Architecture Patterns

### Recommended Project Structure
```
crates/racecontrol/src/
    psychology.rs          # NEW: centralized psychology engine
    db/mod.rs              # MODIFIED: add 6 new tables to migrate()
    lib.rs                 # MODIFIED: add `pub mod psychology;`
    api/routes.rs          # MODIFIED: add psychology API endpoints
    billing.rs             # MODIFIED: call psychology hooks from post_session_hooks()
    lap_tracker.rs         # MODIFIED: call psychology hooks from persist_lap() on PB
    state.rs               # MODIFIED: add PsychologyManager to AppState (optional)
```

### Pattern 1: Module Registration (following existing patterns)
**What:** New module registered in lib.rs, accessing shared AppState
**When to use:** Always for new feature modules in RaceControl
**Example:**
```rust
// lib.rs — add this line alongside existing modules
pub mod psychology;

// psychology.rs — module structure following billing.rs pattern
use std::sync::Arc;
use crate::state::AppState;

/// Evaluate all badge criteria for a driver after a lap/session event.
pub async fn evaluate_badges(state: &Arc<AppState>, driver_id: &str) {
    // Load badge definitions from DB (JSON criteria)
    // Check each against driver's stats
    // Award new badges, skip already-earned ones
}

/// Check and update streak for a driver after a session.
pub async fn update_streak(state: &Arc<AppState>, driver_id: &str) {
    // Load current streak from streaks table
    // Compare last_visit_date with today
    // Increment or reset based on grace period logic
}

/// Queue a notification through the priority system.
pub async fn queue_notification(
    state: &Arc<AppState>,
    driver_id: &str,
    channel: NotificationChannel,
    priority: i32,
    template: &str,
    payload_json: &str,
) {
    // Insert into nudge_queue with status='pending'
    // Background dispatcher will pick it up
}
```

### Pattern 2: Idempotent DB Migration (existing pattern in db/mod.rs)
**What:** New tables use CREATE TABLE IF NOT EXISTS; new columns use ALTER TABLE with error suppression
**When to use:** All schema changes in RaceControl
**Example:**
```rust
// In db/mod.rs migrate() function — add after existing tables
sqlx::query(
    "CREATE TABLE IF NOT EXISTS achievements (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        description TEXT,
        category TEXT NOT NULL DEFAULT 'general',
        criteria_json TEXT NOT NULL,
        badge_icon TEXT,
        sort_order INTEGER DEFAULT 0,
        is_active INTEGER DEFAULT 1,
        created_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;
```

### Pattern 3: Background Task Spawning (following scheduler.rs)
**What:** Tokio spawn for periodic notification queue drain
**When to use:** The notification dispatcher that processes the nudge_queue
**Example:**
```rust
// psychology.rs — spawn notification dispatcher
pub fn spawn_dispatcher(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Err(e) = drain_notification_queue(&state).await {
                tracing::error!("[psychology] dispatcher error: {}", e);
            }
        }
    });
}
```

### Pattern 4: WhatsApp Sending via Evolution API (following billing.rs)
**What:** HTTP POST to Evolution API for outbound WhatsApp messages
**When to use:** Notification dispatch for WhatsApp channel
**Example:**
```rust
// Reuse existing pattern from billing.rs send_whatsapp_receipt
async fn send_whatsapp(state: &Arc<AppState>, phone: &str, message: &str) -> bool {
    if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
        &state.config.auth.evolution_url,
        &state.config.auth.evolution_api_key,
        &state.config.auth.evolution_instance,
    ) {
        let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
        let body = serde_json::json!({ "number": phone, "text": message });
        // 5-second timeout, best effort
        match state.http_client.post(&url)
            .header("apikey", evo_key)
            .json(&body)
            .send().await
        {
            Ok(resp) if resp.status().is_success() => true,
            _ => false,
        }
    } else {
        false
    }
}
```

### Pattern 5: Discord Webhook Sending
**What:** HTTP POST to Discord webhook URL
**When to use:** Notification dispatch for Discord channel
**Example:**
```rust
async fn send_discord(state: &Arc<AppState>, content: &str) -> bool {
    if let Some(webhook_url) = &state.config.integrations.discord.webhook_url {
        let body = serde_json::json!({ "content": content });
        match state.http_client.post(webhook_url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => true,
            _ => false,
        }
    } else {
        false
    }
}
```

### Anti-Patterns to Avoid
- **Putting notification throttle in the WhatsApp bot:** The bot handles inbound messages; outbound proactive notifications must be throttled at the source (RaceControl), not the delivery layer.
- **Hardcoding badge criteria in Rust:** FOUND-03 requires JSON-in-DB for no-code extensibility. Badge thresholds, conditions, and rewards MUST be data, not code.
- **Separate notification service:** Constraint says no new PM2 processes. Notification dispatch lives inside RaceControl as a background tokio task.
- **Evaluating badges synchronously in request handlers:** Badge evaluation may involve multiple DB queries. Run it in a spawned task after session/lap events, not blocking the HTTP response.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Notification throttling | Custom in-memory counter per endpoint | nudge_queue DB table with daily_count check per (driver, channel, date) | Must survive server restarts; must be queryable for admin debugging |
| WhatsApp message sending | New HTTP client wrapper | Existing Evolution API pattern from billing.rs | Already works, already handles timeouts and error logging |
| Discord message sending | Bot SDK integration | Discord webhook URL (simple HTTP POST) | Webhook is stateless, no bot token management needed for outbound-only |
| UUID generation | Custom ID scheme | uuid::Uuid::new_v4() | Already used everywhere in RaceControl |
| JSON parsing for badge criteria | Custom DSL or expression evaluator | serde_json::Value with match/comparison logic | Simple enough for operator comparisons (>, <, ==, >=); avoid script engines |
| Background task scheduling | External cron / PM2 process | tokio::spawn + interval (same as scheduler.rs) | In-process, no external dependency, follows existing pattern |

**Key insight:** RaceControl already has every building block needed. The psychology module is a new consumer of existing infrastructure (SQLite, Evolution API, Discord webhook, tokio tasks, broadcast channels), not a new stack.

## Common Pitfalls

### Pitfall 1: Throttle Bypass via Multiple Entry Points
**What goes wrong:** WhatsApp messages sent from billing.rs (receipts), from psychology.rs (nudges), and from the WhatsApp bot (campaigns) all bypass the 2-per-day limit because each has its own send path.
**Why it happens:** The receipt in billing.rs calls Evolution API directly; the bot has its own Evolution API client.
**How to avoid:** All proactive outbound messages MUST flow through the nudge_queue. Reactive messages (OTP, receipts in response to customer action) are exempt from the daily budget but still logged. Define clearly: proactive = system-initiated (PB beaten, streak risk, review nudge). Reactive = customer-triggered (OTP, receipt after session).
**Warning signs:** `send_whatsapp_receipt()` in billing.rs should remain as-is (it is reactive, triggered by customer ending a session). New proactive messages MUST go through the queue.

### Pitfall 2: Badge Criteria JSON Too Complex
**What goes wrong:** Designing a JSON schema that tries to support arbitrary boolean logic, nested conditions, and computed fields — turning it into a mini programming language.
**Why it happens:** Trying to make the system handle every future badge type without code changes.
**How to avoid:** Keep criteria simple: `{"type": "total_laps", "operator": ">=", "value": 100}` or `{"type": "unique_tracks", "operator": ">=", "value": 10}`. Support a fixed set of metric types (total_laps, unique_tracks, unique_cars, session_count, pb_count, streak_weeks). Adding a new metric type requires a code change — that is acceptable and better than a complex DSL.
**Warning signs:** If the JSON schema has "if/then/else" or "and/or" combinators, it is too complex.

### Pitfall 3: Notification Queue Grows Unbounded
**What goes wrong:** Queue fills up if dispatcher is slow or WhatsApp API is down, consuming disk space and making queries slow.
**Why it happens:** No TTL or cleanup on old queue entries.
**How to avoid:** Nudge queue entries should have an `expires_at` column. Dispatcher skips expired entries. Cleanup job (in the same background task) deletes entries older than 7 days. Status transitions: pending -> sent | expired | failed.
**Warning signs:** nudge_queue row count growing beyond a few hundred.

### Pitfall 4: Database Lock Contention with Background Tasks
**What goes wrong:** The notification dispatcher running SELECT + UPDATE on nudge_queue causes lock contention with the main billing/lap tracking writes.
**Why it happens:** SQLite has a single-writer model; WAL mode helps but heavy writes can still contend.
**How to avoid:** Keep dispatcher transactions short — SELECT one batch, process, UPDATE status. Use `LIMIT 10` per drain cycle. The 30-second interval is more than sufficient for < 1000 customers.
**Warning signs:** `SQLITE_BUSY` errors in tracing logs from the dispatcher.

### Pitfall 5: Streak Grace Period Off-by-One
**What goes wrong:** Streaks break a day early or a day late due to timezone confusion between UTC (stored in DB) and IST (customer experience).
**Why it happens:** Dates compared in UTC when the customer thinks in IST (UTC+5:30). A visit at 11 PM IST on Saturday is Sunday UTC.
**How to avoid:** Always convert to IST (Asia/Kolkata) before comparing dates for streak purposes. The config already has `timezone = "Asia/Kolkata"`. Use chrono's FixedOffset or Local::now() with the timezone.
**Warning signs:** Customers reporting broken streaks after late-night visits.

### Pitfall 6: Forgetting Cloud-Venue Sync for New Tables
**What goes wrong:** New psychology tables exist on the cloud VPS but not at the venue (or vice versa), because cloud_sync.rs does not know about them.
**Why it happens:** The sync system has an explicit list of tables it pushes/pulls.
**How to avoid:** For Phase 1, the new tables only need to exist on the cloud VPS (where Bono's RaceControl runs). Badges and streaks are computed from lap/session data that syncs from venue. The nudge_queue processes on the cloud side (where Evolution API and Discord webhook are configured). Do NOT add psychology tables to cloud sync in Phase 1 — it would add complexity for no benefit. Revisit in Phase 2 if venue-side display is needed.
**Warning signs:** James reporting missing tables after rebuild.

## Code Examples

Verified patterns from the existing codebase:

### DB Table Creation (from db/mod.rs)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/db/mod.rs line 1146
sqlx::query(
    "CREATE TABLE IF NOT EXISTS coupons (
        id TEXT PRIMARY KEY,
        code TEXT NOT NULL UNIQUE,
        coupon_type TEXT NOT NULL DEFAULT 'flat' CHECK(coupon_type IN ('percent', 'flat', 'free_minutes')),
        value INTEGER NOT NULL,
        max_uses INTEGER,
        used_count INTEGER DEFAULT 0,
        valid_from TEXT,
        valid_until TEXT,
        min_spend_paise INTEGER DEFAULT 0,
        first_session_only INTEGER DEFAULT 0,
        is_active INTEGER DEFAULT 1,
        created_at TEXT DEFAULT (datetime('now'))
    )",
)
.execute(pool)
.await?;
```

### Post-Session Hook Trigger (from billing.rs)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/billing.rs line 2047
// Inside end_billing_session(), after session ends successfully:
tokio::spawn(async move {
    post_session_hooks(&state_clone, &session_id_clone, &driver_id_clone).await;
});
```

### Evolution API WhatsApp Send (from billing.rs)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/billing.rs line 2236-2271
if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
    &state.config.auth.evolution_url,
    &state.config.auth.evolution_api_key,
    &state.config.auth.evolution_instance,
) {
    let wa_phone = format_wa_phone(&phone);
    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({
        "number": wa_phone,
        "text": message
    });
    // 5-second timeout
    match client.post(&url).header("apikey", evo_key).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => { /* logged */ }
        Ok(resp) => { /* warn logged */ }
        Err(e) => { /* warn logged */ }
    }
}
```

### Background Task Spawn (from scheduler.rs)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/scheduler.rs line 11-21
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = tick(&state).await {
                tracing::error!("[scheduler] tick error: {}", e);
            }
        }
    });
}
```

### AppState Shared Access (from billing.rs)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/billing.rs line 24
// All DB access goes through state.db (SqlitePool)
let rules = sqlx::query_as::<_, (String, f64, i64)>(
    "SELECT rule_type, multiplier, flat_adjustment_paise
     FROM pricing_rules WHERE is_active = 1 ..."
)
.fetch_optional(&state.db)
.await
.ok()
.flatten();
```

## Database Schema Design

### New Tables (6 total for FOUND-05)

```sql
-- 1. achievements: badge/achievement definitions with JSON criteria
CREATE TABLE IF NOT EXISTS achievements (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL DEFAULT 'general'
        CHECK(category IN ('milestone', 'skill', 'dedication', 'social', 'special')),
    criteria_json TEXT NOT NULL,   -- e.g. {"type":"total_laps","operator":">=","value":100}
    badge_icon TEXT,               -- icon identifier for PWA display
    reward_credits_paise INTEGER DEFAULT 0,  -- optional credit reward
    sort_order INTEGER DEFAULT 0,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now'))
);

-- 2. driver_achievements: which drivers earned which badges
-- (This is the join table; not listed in requirement but implied)
CREATE TABLE IF NOT EXISTS driver_achievements (
    id TEXT PRIMARY KEY,
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    achievement_id TEXT NOT NULL REFERENCES achievements(id),
    earned_at TEXT DEFAULT (datetime('now')),
    notified INTEGER DEFAULT 0,
    UNIQUE(driver_id, achievement_id)
);

-- 3. streaks: weekly visit streak tracking per driver
CREATE TABLE IF NOT EXISTS streaks (
    id TEXT PRIMARY KEY,
    driver_id TEXT NOT NULL UNIQUE REFERENCES drivers(id),
    current_streak INTEGER NOT NULL DEFAULT 0,
    longest_streak INTEGER NOT NULL DEFAULT 0,
    last_visit_date TEXT,          -- ISO date in IST (YYYY-MM-DD)
    grace_expires_date TEXT,       -- date when grace period ends
    streak_started_at TEXT,
    updated_at TEXT DEFAULT (datetime('now'))
);

-- 4. driving_passport: track/car completion progress per driver
CREATE TABLE IF NOT EXISTS driving_passport (
    id TEXT PRIMARY KEY,
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    track TEXT NOT NULL,
    car TEXT NOT NULL,
    first_driven_at TEXT DEFAULT (datetime('now')),
    best_lap_ms INTEGER,
    lap_count INTEGER DEFAULT 1,
    UNIQUE(driver_id, track, car)
);

-- 5. nudge_queue: notification priority queue with channel routing
CREATE TABLE IF NOT EXISTS nudge_queue (
    id TEXT PRIMARY KEY,
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    channel TEXT NOT NULL CHECK(channel IN ('whatsapp', 'discord', 'pwa')),
    priority INTEGER NOT NULL DEFAULT 5,  -- 1=highest, 10=lowest
    template TEXT NOT NULL,               -- template identifier
    payload_json TEXT DEFAULT '{}',       -- template variables
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'sent', 'failed', 'expired', 'throttled')),
    scheduled_at TEXT DEFAULT (datetime('now')),
    expires_at TEXT,
    sent_at TEXT,
    error_text TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);

-- 6. staff_badges + staff_challenges (two tables counted as one requirement item)

CREATE TABLE IF NOT EXISTS staff_badges (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    criteria_json TEXT NOT NULL,
    badge_icon TEXT,
    is_active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS staff_challenges (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    goal_type TEXT NOT NULL,       -- e.g. 'sessions_handled', 'customer_ratings'
    goal_target INTEGER NOT NULL,
    reward_description TEXT,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    current_progress INTEGER DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active'
        CHECK(status IN ('active', 'completed', 'expired')),
    created_at TEXT DEFAULT (datetime('now'))
);
```

### Notification Budget Tracking
```sql
-- Daily budget check query (enforces FOUND-01)
-- Count proactive messages sent to this driver today via WhatsApp
SELECT COUNT(*) FROM nudge_queue
WHERE driver_id = ?
  AND channel = 'whatsapp'
  AND status = 'sent'
  AND date(sent_at) = date('now');
-- If count >= 2, do not send; mark as 'throttled'
```

### Badge Criteria JSON Schema
```json
// Simple single-condition badge
{"type": "total_laps", "operator": ">=", "value": 100}

// Supported metric types (evaluated in psychology.rs):
// - total_laps: from drivers.total_laps
// - unique_tracks: COUNT(DISTINCT track) FROM driving_passport WHERE driver_id = ?
// - unique_cars: COUNT(DISTINCT car) FROM driving_passport WHERE driver_id = ?
// - session_count: COUNT(*) FROM billing_sessions WHERE driver_id = ? AND status IN ('completed')
// - pb_count: COUNT(*) FROM personal_bests WHERE driver_id = ?
// - streak_weeks: from streaks.current_streak
// - first_lap: special (auto-awarded on first completed lap)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| WhatsApp messages sent directly from billing.rs | All proactive messages queue through nudge_queue | Phase 1 | Unified throttling, audit trail |
| No notification throttling | 2/day WhatsApp budget enforced in DB | Phase 1 | Prevents spam, builds trust |
| Badge logic hardcoded per feature | JSON criteria in achievements table | Phase 1 | Admin can add badges without deploys |
| No streak tracking | streaks table with IST-aware date comparison | Phase 1 | Foundation for Phase 4 retention loops |

**Existing patterns to preserve:**
- `send_whatsapp_receipt()` in billing.rs is REACTIVE (customer triggers session end). It should NOT go through the nudge_queue. Keep it as-is.
- OTP sending in auth/mod.rs is REACTIVE. Keep it as-is.
- Only PROACTIVE messages (system-initiated) go through the nudge_queue.

## Open Questions

1. **Discord webhook URL configuration**
   - What we know: `config.integrations.discord.webhook_url` exists in the config struct but is `None` in the current `racecontrol.toml`
   - What's unclear: Whether a Discord webhook has been created for the RacingPoint server
   - Recommendation: Add webhook URL to racecontrol.toml config during implementation. Discord webhook creation is a one-time manual step (Server Settings > Integrations > Webhooks).

2. **PWA push notification mechanism**
   - What we know: `dashboard_tx` broadcast channel exists for real-time venue dashboard events. The PWA has WebSocket support.
   - What's unclear: Whether PWA customers have a persistent WebSocket connection for receiving push notifications
   - Recommendation: For Phase 1, PWA "push" means writing to a `pwa_notifications` table that the PWA polls on next page load, or including in the existing customer API responses. True WebSocket push to PWA customers can be deferred to Phase 3 (real-time PB toast).

3. **Seed badge data**
   - What we know: Phase 2 will define specific badges (first lap, 10 tracks, 100 laps, etc.)
   - What's unclear: Whether Phase 1 should seed initial badge definitions or leave the table empty
   - Recommendation: Seed 3-5 basic badges in Phase 1 to verify the JSON criteria evaluation works end-to-end. This validates the schema design before Phase 2 builds on it.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (cargo test) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p racecontrol-crate --lib` |
| Full suite command | `cargo test -p racecontrol-crate` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FOUND-01 | WhatsApp budget enforced (max 2/day) | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_whatsapp_budget_enforced -- --exact` | Wave 0 |
| FOUND-01 | Reactive messages bypass budget | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_reactive_bypasses_budget -- --exact` | Wave 0 |
| FOUND-02 | Badge evaluation returns correct results | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_badge_evaluation -- --exact` | Wave 0 |
| FOUND-02 | Streak update increments correctly | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_streak_update -- --exact` | Wave 0 |
| FOUND-03 | JSON criteria parsed and evaluated | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_json_criteria_parsing -- --exact` | Wave 0 |
| FOUND-04 | Queue dispatches to correct channel | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_channel_routing -- --exact` | Wave 0 |
| FOUND-04 | Priority ordering respected | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_priority_ordering -- --exact` | Wave 0 |
| FOUND-05 | All 6+ tables created successfully | integration | `cargo test -p racecontrol-crate --test integration psychology_tables -- --exact` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate --lib`
- **Per wave merge:** `cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `psychology::tests` module — unit tests for badge evaluation, streak logic, budget enforcement, channel routing
- [ ] Integration test additions in `crates/racecontrol/tests/integration.rs` — verify all new tables exist and seed data works
- [ ] Test helper: `create_test_db()` in integration.rs needs updating to include new psychology tables (currently mirrors db/mod.rs manually)

## Sources

### Primary (HIGH confidence)
- `/root/racecontrol/crates/racecontrol/src/db/mod.rs` — full DB schema, migration patterns, all existing tables (1700+ lines)
- `/root/racecontrol/crates/racecontrol/src/billing.rs` — post_session_hooks, WhatsApp receipt sending, Evolution API integration
- `/root/racecontrol/crates/racecontrol/src/lib.rs` — module registration pattern
- `/root/racecontrol/crates/racecontrol/src/state.rs` — AppState structure, shared state pattern
- `/root/racecontrol/crates/racecontrol/src/scheduler.rs` — background task spawn pattern
- `/root/racecontrol/crates/racecontrol/src/bot_coordinator.rs` — event routing pattern
- `/root/racecontrol/crates/racecontrol/src/config.rs` — config structure, Evolution API config, Discord webhook config
- `/root/racecontrol/racecontrol.toml` — live config with Evolution API credentials
- `/root/racecontrol/crates/racecontrol/src/api/routes.rs` — API route registration, 270+ routes
- `/root/racecontrol/crates/racecontrol/Cargo.toml` — all dependencies already present

### Secondary (MEDIUM confidence)
- `/root/racingpoint-whatsapp-bot/src/services/evolutionService.js` — Evolution API message format verification
- `/root/racingpoint-whatsapp-bot/src/services/rateLimiter.js` — existing rate limiter (NOT for outbound throttling; inbound only)
- `/root/racingpoint-discord-bot/src/config.js` — Discord bot configuration pattern

### Tertiary (LOW confidence)
- Discord webhook API format — assumed standard `{"content": "..."}` POST; verify with Discord docs during implementation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all dependencies already in workspace, no new libraries needed
- Architecture: HIGH - every pattern has a working precedent in the existing codebase
- Pitfalls: HIGH - identified from direct code inspection of existing notification paths and SQLite usage patterns
- Database schema: HIGH - follows exact patterns from 40+ existing tables in db/mod.rs
- Notification routing: MEDIUM - Discord webhook URL not yet configured; PWA push mechanism needs decision

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable — existing codebase patterns unlikely to change)
