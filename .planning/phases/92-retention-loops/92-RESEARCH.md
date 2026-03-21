# Phase 92: Retention Loops - Research

**Researched:** 2026-03-21
**Domain:** Rust (Axum) backend psychology hooks, SQLite schema extension, PWA streak display, loss-framed copy, variable rewards
**Confidence:** HIGH

## Summary

Phase 92 builds six retention mechanics on top of the psychology.rs foundation (Phase 89) and the PbAchieved broadcast infrastructure (Phase 91). Every building block needed already exists in the codebase — no new libraries, no new DB tables except one optional `variable_reward_log` for RET-06 audit trail. The work is entirely new Rust functions in psychology.rs plus one new daily check in scheduler.rs or a second periodic task.

The six requirements divide into three groups: (1) streak display plumbing (RET-01 — streaks already tracked, just need grace-period-aware UI data), (2) PB-beaten and streak-at-risk WhatsApp notifications (RET-02, RET-05 — use the existing nudge_queue pipeline), and (3) variable reward credits and membership loss-framed warnings (RET-03, RET-04, RET-06 — use wallet::credit and the existing memberships table). The only technically subtle part is RET-02: detecting when Driver A's new PB beats Driver B's existing PB on the same track+car, then notifying Driver B without spamming active drivers.

**Primary recommendation:** Add four functions to psychology.rs (`maybe_grant_variable_reward`, `notify_pb_beaten_holders`, `check_streak_at_risk`, `check_membership_expiry_warnings`) and wire them from their trigger points (persist_lap for RET-02/RET-03, post_session_hooks for RET-03 milestone variant, scheduler.rs tick for RET-05/RET-04). The nudge_queue handles throttling automatically. No new dependencies are needed.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| RET-01 | System tracks weekly visit streaks with 1-week grace period before reset | streaks table already exists with current_streak, longest_streak, last_visit_date, grace_expires_date; update_streak() already called from post_session_hooks; passport API already exposes streak_weeks. Need to add grace_expires_date and days_until_grace_expires to customer-facing passport summary so PWA can show streak urgency |
| RET-02 | "Someone beat your PB" WhatsApp notification sent to affected customer (throttled, active customers only) | PbAchieved broadcast already fires in persist_lap() after PB upsert; need to add notify_pb_beaten_holders() called from persist_lap() — query personal_bests for other drivers with the same track+car who had a better time before this lap, filter to "active" (visited in last 30 days), queue WhatsApp via nudge_queue |
| RET-03 | Variable rewards (surprise bonus credits) triggered on PB achievement (15% probability) and milestones (10% probability) | rand 0.8 already in workspace Cargo.toml; wallet::credit() exists with full double-entry accounting; need maybe_grant_variable_reward() using rand::thread_rng().gen_bool(); RET-06 cap requires tracking monthly reward totals via variable_reward_log table (new) |
| RET-04 | Membership expiry warnings use loss-framed copy ("You'll lose your Pro Driver status") | memberships table exists with expires_at + tier_id; membership_tiers table has names (Rookie, Pro, Champion); scheduler.rs tick runs every 60s — add check for memberships expiring in 3 days; queue via nudge_queue with loss-framed template |
| RET-05 | Streak-at-risk WhatsApp nudge sent 2 days before grace period expires | streaks table has grace_expires_date; scheduler.rs tick runs every 60s — add daily query for streaks where grace_expires_date is 2 days from now and current_streak >= 2 (only meaningful streaks); queue via nudge_queue, deduplicate with sent check |
| RET-06 | Variable reward budget capped at 5% of customer spend with monthly reconciliation | wallets table has total_debited_paise (cumulative spend); need variable_reward_log table to track monthly reward totals; cap check = SELECT SUM(amount_paise) FROM variable_reward_log WHERE driver_id = ? AND month = current_month, compare vs 5% of total_debited_paise |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8 | All DB queries (streaks, memberships, personal_bests, wallet) | Already in workspace Cargo.toml |
| serde / serde_json | workspace | JSON payload for nudge_queue templates | Already used throughout |
| chrono | workspace | Date arithmetic for grace period, membership expiry, monthly cap window | Already used in psychology.rs (streak IST logic) |
| tokio | workspace | Async functions, spawn from scheduler | Already used throughout |
| rand | 0.8 | gen_bool() for 15%/10% probability gates | Already in workspace Cargo.toml (`rand = { workspace = true }`) |
| uuid | workspace | Primary key generation for variable_reward_log | Already used throughout |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing | workspace | Structured logging for retention events | All new functions |
| anyhow | workspace | Error handling in scheduler-called functions | scheduler.rs tick returns anyhow::Result |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| rand::thread_rng().gen_bool() | Seeded/deterministic random | gen_bool is correct — surprises must be non-deterministic |
| In-memory variable reward counter | variable_reward_log DB table | DB table survives restarts, queryable for admin reconciliation (RET-06 requirement) |
| Separate cron process for streak/membership checks | scheduler.rs tick | No new PM2 processes; scheduler already runs every 60s |

**Installation:**
No new dependencies needed. All libraries are already in the workspace Cargo.toml.

## Architecture Patterns

### Recommended Project Structure
```
crates/racecontrol/src/
    psychology.rs          # MODIFIED: 4 new functions for retention mechanics
    billing.rs             # MODIFIED: call maybe_grant_variable_reward from post_session_hooks (milestone variant)
    lap_tracker.rs         # MODIFIED: call notify_pb_beaten_holders + maybe_grant_variable_reward (PB variant)
    scheduler.rs           # MODIFIED: call check_streak_at_risk + check_membership_expiry_warnings in tick()
    db/mod.rs              # MODIFIED: add variable_reward_log table

pwa/src/
    app/passport/page.tsx  # MODIFIED: show streak with grace urgency indicator (days remaining)
    lib/api.ts             # MODIFIED: add grace_expires_date + days_until_grace to PassportSummary type
```

### Pattern 1: notify_pb_beaten_holders — RET-02
**What:** When Driver A's lap beats their own PB on track/car, find all OTHER drivers who previously held a PB on that same track/car and had a time that was faster than the NEW record (i.e., they are now displaced). Notify each via WhatsApp nudge.
**When to use:** Called from persist_lap() immediately after the PbAchieved broadcast, before returning.
**Key implementation detail:** The query must find drivers whose best_lap_ms is greater than the NEW lap time but only if they existed BEFORE this lap. This is a straightforward join — personal_bests holds one row per (driver_id, track, car) representing their current best. Any driver with best_lap_ms > new_lap_time_ms on the same track+car has just been beaten.
**Active customers filter:** Only notify drivers who have had a billing session in the last 30 days (not dormant drivers).
**Example:**
```rust
// In psychology.rs — new function
pub async fn notify_pb_beaten_holders(
    state: &Arc<AppState>,
    new_holder_driver_id: &str,
    track: &str,
    car: &str,
    new_lap_time_ms: i64,
) {
    // Find all OTHER drivers whose PB on this track+car is now slower
    // AND who have been active in the last 30 days
    let beaten_drivers: Vec<(String,)> = sqlx::query_as(
        "SELECT pb.driver_id
         FROM personal_bests pb
         JOIN billing_sessions bs ON bs.driver_id = pb.driver_id
         WHERE pb.track = ?
           AND pb.car = ?
           AND pb.driver_id != ?
           AND pb.best_lap_ms > ?
           AND bs.status IN ('completed', 'ended_early')
           AND datetime(bs.ended_at) > datetime('now', '-30 days')
         GROUP BY pb.driver_id"
    )
    .bind(track)
    .bind(car)
    .bind(new_holder_driver_id)
    .bind(new_lap_time_ms)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (driver_id,) in beaten_drivers {
        let payload = serde_json::json!({
            "track": track,
            "car": car,
            "new_time_ms": new_lap_time_ms,
        }).to_string();
        queue_notification(
            state,
            &driver_id,
            NotificationChannel::Whatsapp,
            3, // priority 3 — lower than streak-at-risk (priority 2)
            "pb_beaten",
            &payload,
        ).await;
    }
}
```

### Pattern 2: maybe_grant_variable_reward — RET-03/RET-06
**What:** On PB achievement (15% chance) or milestone badge award (10% chance), credit a surprise bonus amount. Check monthly cap (5% of total spend) before crediting.
**When to use:** Called from persist_lap() for PB trigger; called from evaluate_badges() when a new badge is awarded for milestone trigger.
**Reward amount:** The requirement does not specify an exact amount. Recommended: 50-200 credits (5000-20000 paise), varied by achievement type.
**Example:**
```rust
// In psychology.rs — new function
pub async fn maybe_grant_variable_reward(
    state: &Arc<AppState>,
    driver_id: &str,
    trigger: &str, // "pb" | "milestone"
) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // Probability gate
    let threshold = match trigger {
        "pb" => 0.15,
        "milestone" => 0.10,
        _ => return,
    };
    if !rng.gen_bool(threshold) {
        return;
    }

    // RET-06: check monthly reward cap (5% of total spend)
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let now_ist = chrono::Utc::now().with_timezone(&ist_offset);
    let month_str = now_ist.format("%Y-%m").to_string();

    let total_spend: i64 = sqlx::query_scalar(
        "SELECT COALESCE(total_debited_paise, 0) FROM wallets WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let cap_paise = total_spend / 20; // 5% of total spend

    let already_rewarded: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_paise), 0) FROM variable_reward_log
         WHERE driver_id = ? AND month = ?"
    )
    .bind(driver_id)
    .bind(&month_str)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if already_rewarded >= cap_paise {
        tracing::info!("[psychology] variable reward cap reached for driver {} this month", driver_id);
        return;
    }

    // Grant reward — amount 50-200 credits (5000-20000 paise), capped to remaining budget
    let base_amount: i64 = match trigger {
        "pb" => 5000,       // 50 credits
        "milestone" => 10000, // 100 credits
        _ => 5000,
    };
    let amount = base_amount.min(cap_paise - already_rewarded);
    if amount <= 0 {
        return;
    }

    let reward_id = uuid::Uuid::new_v4().to_string();
    let _ = crate::wallet::credit(
        state,
        driver_id,
        amount,
        "bonus",
        Some(&reward_id),
        Some(&format!("Surprise bonus — {}", trigger)),
        None,
    ).await;

    // Log for cap tracking (RET-06)
    let _ = sqlx::query(
        "INSERT INTO variable_reward_log (id, driver_id, amount_paise, trigger, month, created_at)
         VALUES (?, ?, ?, ?, ?, datetime('now'))"
    )
    .bind(&reward_id)
    .bind(driver_id)
    .bind(amount)
    .bind(trigger)
    .bind(&month_str)
    .execute(&state.db)
    .await;

    tracing::info!("[psychology] variable reward granted: driver={} trigger={} amount_paise={}", driver_id, trigger, amount);
}
```

### Pattern 3: check_streak_at_risk — RET-05
**What:** Daily sweep finding streaks where grace_expires_date is exactly 2 days from today (IST). Queue a WhatsApp nudge for each. Deduplicate by checking if a streak_at_risk nudge has already been sent this grace window.
**When to use:** Called from scheduler.rs tick(). The 60s interval is fine — add a guard to only run once per day (check if current IST hour is 10 AM, i.e., run at opening time).
**Example:**
```rust
// In psychology.rs — new function
pub async fn check_streak_at_risk(state: &Arc<AppState>) -> anyhow::Result<()> {
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
    let today_ist = chrono::Utc::now().with_timezone(&ist_offset).date_naive();
    let two_days_from_now = (today_ist + chrono::Duration::days(2))
        .format("%Y-%m-%d").to_string();

    // Find streaks expiring in 2 days with current_streak >= 2 (meaningful streaks only)
    let at_risk: Vec<(String,)> = sqlx::query_as(
        "SELECT s.driver_id FROM streaks s
         WHERE date(s.grace_expires_date) = ?
           AND s.current_streak >= 2
           AND NOT EXISTS (
               SELECT 1 FROM nudge_queue nq
               WHERE nq.driver_id = s.driver_id
                 AND nq.template = 'streak_at_risk'
                 AND nq.status IN ('pending', 'sent')
                 AND datetime(nq.created_at) > datetime('now', '-8 days')
           )"
    )
    .bind(&two_days_from_now)
    .fetch_all(&state.db)
    .await?;

    for (driver_id,) in at_risk {
        // Fetch streak data for the message
        let streak: Option<(i64, String)> = sqlx::query_as(
            "SELECT current_streak, grace_expires_date FROM streaks WHERE driver_id = ?"
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await?;

        if let Some((current, expires)) = streak {
            let payload = serde_json::json!({
                "streak": current,
                "expires": expires,
            }).to_string();
            queue_notification(
                state,
                &driver_id,
                NotificationChannel::Whatsapp,
                2, // priority 2 — high (streak risk is urgent)
                "streak_at_risk",
                &payload,
            ).await;
            tracing::info!("[psychology] streak_at_risk queued for driver {} (streak={})", driver_id, current);
        }
    }
    Ok(())
}
```

### Pattern 4: check_membership_expiry_warnings — RET-04
**What:** Find active memberships expiring within 3 days. Queue a WhatsApp nudge with loss-framed copy. Deduplicate using the same nudge_queue NOT EXISTS pattern.
**When to use:** Called from scheduler.rs tick(), same daily-guard pattern.
**Example:**
```rust
// In psychology.rs — new function
pub async fn check_membership_expiry_warnings(state: &Arc<AppState>) -> anyhow::Result<()> {
    // Find memberships expiring within 3 days that haven't been warned yet
    let expiring: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT m.driver_id, mt.name, m.expires_at
         FROM memberships m
         JOIN membership_tiers mt ON mt.id = m.tier_id
         WHERE m.status = 'active'
           AND datetime(m.expires_at) <= datetime('now', '+3 days')
           AND datetime(m.expires_at) > datetime('now')
           AND NOT EXISTS (
               SELECT 1 FROM nudge_queue nq
               WHERE nq.driver_id = m.driver_id
                 AND nq.template = 'membership_expiry'
                 AND nq.status IN ('pending', 'sent')
                 AND datetime(nq.created_at) > datetime('now', '-4 days')
           )"
    )
    .fetch_all(&state.db)
    .await?;

    for (driver_id, tier_name, expires_at) in expiring {
        let payload = serde_json::json!({
            "tier_name": tier_name,  // "Pro", "Champion", "Rookie"
            "expires_at": expires_at,
        }).to_string();
        queue_notification(
            state,
            &driver_id,
            NotificationChannel::Whatsapp,
            1, // priority 1 — highest (loss framing, high urgency)
            "membership_expiry",
            &payload,
        ).await;
        tracing::info!("[psychology] membership_expiry queued for driver {} tier={}", driver_id, tier_name);
    }
    Ok(())
}
```

### Pattern 5: Loss-Framed Template Strings (for resolve_template)
**What:** The existing `resolve_template()` function substitutes `{key}` placeholders. Templates are plain strings stored directly in the nudge_queue `template` column and resolved at dispatch time.
**When to use:** These are the actual message strings stored as the `template` parameter.
**Copy:**
```
// Template: "membership_expiry"
// Resolved message:
"You'll lose your {tier_name} Driver status in 3 days. Renew to keep your priority booking and league entry. Reply RENEW or visit racingpoint.in"

// Template: "streak_at_risk"
// Resolved message:
"Your {streak}-week visit streak expires in 2 days. Come race before {expires} to keep it alive!"

// Template: "pb_beaten"
// Resolved message:
"Someone just beat your personal best on {track}! Come back and reclaim it. Your record: we'll see you on track."

// Note: No new template engine needed. resolve_template() handles {key} substitution.
// Templates are stored verbatim in the nudge_queue.template column.
```

### Pattern 6: Streak Display Enhancement for RET-01 (PWA)
**What:** The passport API already returns `streak_weeks` (current_streak). For RET-01, it should also return `grace_expires_date` and `longest_streak` so the PWA can show the full streak context and urgency.
**When to use:** Modify the `/customer/passport` response's `summary` block.
**Example:**
```rust
// In api/routes.rs customer_passport, extend the summary object:
// Currently:  "streak_weeks": streak_weeks
// Add:
let streak_data: Option<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
    "SELECT current_streak, longest_streak, last_visit_date, grace_expires_date
     FROM streaks WHERE driver_id = ?"
)
.bind(&driver_id)
.fetch_optional(&state.db)
.await
.ok()
.flatten();

let (streak_weeks, longest_streak, last_visit, grace_expires) = streak_data
    .map(|(c, l, lv, ge)| (c, l, lv, ge))
    .unwrap_or((0, 0, None, None));

// In the response:
"summary": {
    "unique_tracks": ...,
    "unique_cars": ...,
    "total_laps": ...,
    "streak_weeks": streak_weeks,
    "longest_streak": longest_streak,
    "last_visit_date": last_visit,
    "grace_expires_date": grace_expires,
}
```

### Anti-Patterns to Avoid
- **Notifying inactive (dormant) drivers for RET-02:** If a customer hasn't visited in 6 months, a "your PB was beaten" message is spam. Always filter by last 30 days activity.
- **Running streak/membership checks every 60 seconds:** The scheduler tick runs every 60s. Add a daily-only guard using IST hour check (`now.hour() == 10`) so these heavy queries run once per day, not 1440 times.
- **Storing loss-framed copy in DB:** Templates are simple strings. The `template` column in nudge_queue stores the full message text (with `{placeholder}` slots), not a template identifier. `resolve_template()` handles substitution. No separate template table needed.
- **Calling maybe_grant_variable_reward synchronously in persist_lap:** Like evaluate_badges, wrap variable reward check in a spawned task inside persist_lap to avoid blocking the lap insert response.
- **Using total_credited_paise for cap calculation:** The RET-06 cap is 5% of SPEND (debit), not top-ups. Use `total_debited_paise` from the wallets table.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Notification throttling | Custom per-driver counter | nudge_queue + is_whatsapp_budget_exceeded() | Already enforced for all proactive messages |
| WhatsApp sending | New HTTP client | psychology.rs send_whatsapp() (internal) / queue_notification() | Already wired to Evolution API |
| Wallet credit | Direct DB UPDATE | wallet::credit() | Handles double-entry accounting, journal entries, atomicity |
| Random probability check | Custom entropy | rand::thread_rng().gen_bool(0.15) | rand 0.8 already in workspace |
| Date arithmetic (IST) | UTC math | chrono::FixedOffset::east_opt(5*3600+30*60) | Already used in update_streak(), same pattern |
| Membership expiry detection | New table/flag | Query memberships.expires_at directly | expires_at already exists, index exists |
| Deduplication of nudges | Custom flag columns | nudge_queue NOT EXISTS subquery | nudge_queue already tracks sent status + created_at |

**Key insight:** Phase 89 built the entire notification pipeline. Phase 92 only writes new trigger functions that call `queue_notification()`. The hard work (throttling, routing, delivery, cleanup) is already done.

## Common Pitfalls

### Pitfall 1: RET-02 Fan-Out Amplification
**What goes wrong:** A customer on a popular track (e.g., Spa-Francorchamps) beats their PB. There are 50 other drivers who also have PBs on that track. All 50 get WhatsApp messages — this blows through the daily budget for all of them.
**Why it happens:** The query returns ALL drivers with a slower PB, not just those for whom this is a meaningful beat (e.g., only notifying drivers whose PB was faster by < 5 seconds before).
**How to avoid:** Two mitigations: (1) the `billing_sessions` activity filter (last 30 days) already prunes dormant drivers; (2) add a "closeness" filter — only notify drivers whose old PB was within 5% of the new time. This keeps notifications meaningful. Example SQL addition: `AND pb.best_lap_ms <= ? * 1.05` (old time was at most 5% slower than new time). Alternatively, cap at 5 notifications per PB event.
**Warning signs:** nudge_queue filling up with pb_beaten entries after a particularly fast session.

### Pitfall 2: Scheduler check_streak_at_risk Running Every Tick
**What goes wrong:** scheduler.rs tick() runs every 60 seconds. If check_streak_at_risk() is called on every tick, the query runs 1440 times per day and may queue duplicate streak_at_risk nudges.
**Why it happens:** The NOT EXISTS deduplication in the query prevents double-sends, but the query itself is wasteful and adds DB load.
**How to avoid:** Add a time-of-day guard. Only run the streak and membership checks at a specific IST hour (e.g., between 10:00 and 10:01 AM — opening time is when staff arrive and can handle inquiries):
```rust
// In scheduler.rs tick():
let now_ist = chrono::Utc::now().with_timezone(&ist_offset);
if now_ist.hour() == 10 && now_ist.minute() == 0 {
    let _ = psychology::check_streak_at_risk(&state).await;
    let _ = psychology::check_membership_expiry_warnings(&state).await;
}
```
The 60s interval means this fires once at 10:00 IST and not again until the next day.
**Warning signs:** nudge_queue filling up with hundreds of streak_at_risk entries per day.

### Pitfall 3: variable_reward_log Month Boundary
**What goes wrong:** Monthly cap calculated using DB `strftime('%Y-%m', 'now')` but the log entries were created with UTC timestamps. A reward at 11:50 PM IST (6:20 PM UTC) is in month M, but the next reward at 12:10 AM IST (6:40 PM UTC) is in month M+1 IST but still month M UTC. The cap query uses the wrong month.
**Why it happens:** SQLite `datetime('now')` is UTC; IST business month should be used for customer-facing caps.
**How to avoid:** Store the `month` column explicitly as the IST month string (`format!("{}", now_ist.format("%Y-%m"))`) at insert time, not computed from UTC datetime. The maybe_grant_variable_reward function already does this correctly with the `month_str` variable in Pattern 2.
**Warning signs:** Customers receiving variable rewards in the first minutes of a new IST month that count against the previous month's cap.

### Pitfall 4: Loss-Framed Copy Too Aggressive for Rookie Tier
**What goes wrong:** "You'll lose your Rookie Driver status" sounds threatening but meaningless for a Rookie — Rookie is the entry-level tier, so losing it means returning to free-tier, which the customer may not care about.
**Why it happens:** Applying the same loss-framing template to all membership tiers.
**How to avoid:** Tier-specific copy. Rookie: "Your Rookie Member benefits (priority booking) expire in 3 days — renew to keep them." Pro/Champion: "You'll lose your {tier_name} Driver status — priority booking, league access, and coaching will end." Or simply use a single template that mentions the tier name naturally: "Your RacingPoint {tier_name} membership expires in 3 days. Don't lose your benefits."
**Warning signs:** Customer feedback about confusing or tone-deaf messages.

### Pitfall 5: gen_bool Seed Predictability
**What goes wrong:** If rand::thread_rng() is seeded the same way every process restart (e.g., in tests), the sequence becomes predictable. In production this is irrelevant, but tests will be flaky.
**Why it happens:** rand::thread_rng() is seeded from OS entropy, which is unpredictable in production. But unit tests calling gen_bool directly will get different results each run.
**How to avoid:** In tests, inject a fixed reward amount and bypass the probability check — test the cap logic and wallet credit separately from the random gate. Use `#[cfg(test)]` stubs or pass a `force_grant: bool` parameter to maybe_grant_variable_reward for test overrides.
**Warning signs:** Intermittent test failures in variable reward unit tests.

### Pitfall 6: PB-Beaten Notification for Driver's First-Ever PB
**What goes wrong:** Driver B has no prior PB on a track+car. Driver A sets a new time. Driver B's personal_bests row does not exist yet. No notification needed (nothing was beaten). But if the query uses `best_lap_ms > 0` without a NULL check, it may error or return unexpected results.
**Why it happens:** personal_bests.best_lap_ms is `INTEGER NOT NULL` per the schema — a row only exists if the driver has driven the track+car. So the `personal_bests JOIN` in notify_pb_beaten_holders naturally excludes drivers with no row. This is correct behavior, not a bug.
**Warning signs:** None (this is handled correctly by the schema).

## Code Examples

Verified patterns from the existing codebase:

### rand::gen_bool Usage Pattern
```rust
// rand 0.8 is in workspace Cargo.toml line 43:
// rand = { workspace = true }
// Cargo.toml workspace line 34: rand = "0.8"
// Usage:
use rand::Rng;
let mut rng = rand::thread_rng();
if rng.gen_bool(0.15) {
    // 15% probability branch
}
```

### wallet::credit Signature (from wallet.rs lines 61-68)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/wallet.rs lines 61-68
pub async fn credit(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,        // Use "bonus" for variable rewards (in wallet_transactions CHECK constraint)
    reference_id: Option<&str>,
    notes: Option<&str>,
    staff_id: Option<&str>,
) -> Result<i64, String>
// Returns Ok(new_balance_paise) on success
```

### wallet_transactions.txn_type Constraint (from db/mod.rs lines 766-771)
```sql
-- Valid txn_type values (from CREATE TABLE wallet_transactions):
'topup_cash','topup_card','topup_upi','topup_online',
'debit_session','debit_cafe','debit_merchandise','debit_penalty',
'refund_session','refund_manual',
'bonus','adjustment'
-- Use 'bonus' for variable rewards (already in the constraint)
```

### wallets Table — total_debited_paise (from db/mod.rs line 753)
```sql
-- wallets table schema:
total_credited_paise INTEGER NOT NULL DEFAULT 0,
total_debited_paise INTEGER NOT NULL DEFAULT 0,
-- Use total_debited_paise for 5% spend cap calculation
-- This is the cumulative sum of all debit transactions
```

### membership_tiers Seeded Names (from db/mod.rs lines 1283-1287)
```sql
-- IDs and names (for loss-framed copy):
('mem_rookie', 'Rookie', ...)
('mem_pro',    'Pro',    ...)
('mem_champion', 'Champion', ...)
-- tier_name in notifications: "Rookie", "Pro", "Champion"
-- Loss-framed template uses: "You'll lose your {tier_name} Driver status"
```

### Post-Session Hooks Call Site (from billing.rs lines 2408-2412)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/billing.rs lines 2408-2412
// This is where milestone variable reward trigger goes (step 7):
// 5. Evaluate badges for this driver
crate::psychology::evaluate_badges(state, driver_id).await;
// 6. Update visit streak
crate::psychology::update_streak(state, driver_id).await;
// ADD: 7. Maybe grant variable reward for milestone (10% probability)
// crate::psychology::maybe_grant_variable_reward(state, driver_id, "milestone").await;
```

### persist_lap Call Site for PB Trigger (from lap_tracker.rs lines 138-168)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/lap_tracker.rs lines 138-168
// After the PbAchieved broadcast at line 161, add:
// Notify drivers whose PB was beaten
let state_clone = state.clone();
let driver_id_clone = lap.driver_id.clone();
let track_clone = lap.track.clone();
let car_clone = lap.car.clone();
let lap_time_clone = lap.lap_time_ms as i64;
tokio::spawn(async move {
    crate::psychology::notify_pb_beaten_holders(
        &state_clone, &driver_id_clone, &track_clone, &car_clone, lap_time_clone
    ).await;
    crate::psychology::maybe_grant_variable_reward(
        &state_clone, &driver_id_clone, "pb"
    ).await;
});
```

### Scheduler Daily Guard Pattern (from scheduler.rs lines 10-21)
```rust
// Source: /root/racecontrol/crates/racecontrol/src/scheduler.rs (tick function)
// Add inside tick() after the existing WoL/wake checks:
let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
let now_ist = chrono::Utc::now().with_timezone(&ist_offset);
if now_ist.hour() == 10 && now_ist.minute() < 2 {
    // Run once per day at 10:00-10:01 AM IST (venue opening time)
    let _ = crate::psychology::check_streak_at_risk(&state).await;
    let _ = crate::psychology::check_membership_expiry_warnings(&state).await;
}
```

## Database Schema Design

### New Table: variable_reward_log
Required for RET-06 monthly reconciliation and cap enforcement.

```sql
-- In db/mod.rs migrate() function — add after existing psychology tables
CREATE TABLE IF NOT EXISTS variable_reward_log (
    id TEXT PRIMARY KEY,
    driver_id TEXT NOT NULL REFERENCES drivers(id),
    amount_paise INTEGER NOT NULL,
    trigger TEXT NOT NULL CHECK(trigger IN ('pb', 'milestone')),
    month TEXT NOT NULL,   -- IST month: 'YYYY-MM', e.g. '2026-03'
    created_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_variable_reward_driver_month
    ON variable_reward_log(driver_id, month);
```

### Existing Tables Used (no modifications needed)
```sql
-- personal_bests: used by notify_pb_beaten_holders
-- UNIQUE(driver_id, track, car) — one row per combination
-- Fields: driver_id, track, car, best_lap_ms, lap_id, achieved_at

-- streaks: used by check_streak_at_risk
-- Fields: driver_id, current_streak, longest_streak, last_visit_date,
--         grace_expires_date, streak_started_at, updated_at

-- memberships: used by check_membership_expiry_warnings
-- Fields: driver_id, tier_id, expires_at, status

-- membership_tiers: joined to get tier name
-- Fields: id, name, hours_included, price_paise, perks

-- wallets: used by RET-06 cap check
-- Fields: driver_id, balance_paise, total_credited_paise, total_debited_paise

-- nudge_queue: receives all queued notifications
-- Deduplication via NOT EXISTS subquery on template + status + created_at
```

## Streak Visibility in PWA (RET-01)

The streak is already displayed at `/passport` (Phase 90 PWA). The current display shows `streak_weeks` as a plain number. For RET-01 to be fully complete, the display should show:
- Current streak (already there)
- Grace expires date (to show urgency when streak is at risk)
- Longest streak (motivating context)

The backend passport endpoint already queries `current_streak` from the streaks table but does NOT return `grace_expires_date` or `longest_streak`. A two-line addition to the `customer_passport` handler resolves this.

**Current passport summary response:**
```json
"summary": { "unique_tracks": N, "unique_cars": N, "total_laps": N, "streak_weeks": N }
```

**Required passport summary response (RET-01):**
```json
"summary": {
  "unique_tracks": N, "unique_cars": N, "total_laps": N,
  "streak_weeks": N,
  "longest_streak": N,
  "last_visit_date": "2026-03-20",
  "grace_expires_date": "2026-03-27"
}
```

No new API endpoint is needed — it is a response extension to an existing endpoint.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No PB rivalry notification | notify_pb_beaten_holders queues WhatsApp nudge on new PB | Phase 92 | Creates urgency to return and reclaim |
| No surprise rewards | Variable reward via gen_bool + wallet::credit | Phase 92 | Operant conditioning — unpredictable rewards are more compelling |
| Generic membership expiry copy | Loss-framed: "You'll lose your Pro Driver status" | Phase 92 | Loss aversion > equivalent gain framing |
| Streak shown as plain number | Streak with grace-expires date and days remaining | Phase 92 | Makes the approaching deadline visible and actionable |
| No streak-at-risk notifications | WhatsApp nudge 2 days before grace expires | Phase 92 | Reduces streak breaks without requiring daily visits |

**Existing infrastructure that Phase 92 fully relies on:**
- psychology.rs `queue_notification()` — handles all throttling, delivery, retry
- psychology.rs `update_streak()` — already called from post_session_hooks
- psychology.rs `spawn_dispatcher()` — already running, will pick up new nudge types automatically
- wallet.rs `credit()` — full double-entry accounting, no new code needed
- scheduler.rs tick() — already runs every 60s, just needs new calls added

## Open Questions

1. **Closeness threshold for RET-02 fan-out**
   - What we know: Notifying ALL drivers slower than the new PB could result in many messages on popular track/car combos
   - What's unclear: Whether "beaten your PB" means strictly beaten (any margin) or meaningfully beaten (within racing relevance)
   - Recommendation: Use a 5% time threshold (only notify drivers whose old PB was within 5% of the new time) + cap at 5 notifications per PB event. This keeps messages relevant and prevents spam. Implement as `AND pb.best_lap_ms <= new_time * 1.05 LIMIT 5`.

2. **Variable reward amount**
   - What we know: RET-03 says "surprise bonus credits" but does not specify an amount
   - What's unclear: What amount feels surprising without being expensive or inflation-inducing
   - Recommendation: PB reward: 50 credits (₹50 = 5000 paise). Milestone reward: 100 credits (₹100 = 10000 paise). Both are below the 5% cap for most customers and feel meaningfully large vs a typical session cost of ₹700-900.

3. **Membership expiry warning timing**
   - What we know: Requirements say "use loss-framed copy" but do not specify how many days before expiry
   - What's unclear: Whether 3 days is the right window (may want to warn both 7 days and 3 days before)
   - Recommendation: Single warning at 3 days before expiry. Two warnings (7 days + 3 days) would be better UX but doubles WhatsApp cost and risks the daily budget for that customer. Start with one at 3 days.

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
| RET-01 | Passport summary includes grace_expires_date + longest_streak | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_streak_fields_in_passport -- --exact` | Wave 0 |
| RET-02 | notify_pb_beaten_holders queues nudges for slower drivers | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_notify_pb_beaten_holders -- --exact` | Wave 0 |
| RET-02 | Inactive drivers (>30 days) are not notified | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_pb_beaten_skips_inactive -- --exact` | Wave 0 |
| RET-03 | maybe_grant_variable_reward credits wallet at 15%/10% prob | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_variable_reward_credits_wallet -- --exact` | Wave 0 |
| RET-03 | Variable reward respects 5% monthly cap | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_variable_reward_cap -- --exact` | Wave 0 |
| RET-04 | check_membership_expiry_warnings queues loss-framed nudge | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_membership_expiry_warning -- --exact` | Wave 0 |
| RET-04 | Deduplication prevents double-sending membership warnings | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_membership_warning_dedup -- --exact` | Wave 0 |
| RET-05 | check_streak_at_risk queues nudge for grace-expiring streaks | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_streak_at_risk_queued -- --exact` | Wave 0 |
| RET-05 | Streak >= 2 threshold filters trivial streaks | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_streak_at_risk_min_streak -- --exact` | Wave 0 |
| RET-06 | variable_reward_log table created in migration | integration | `cargo test -p racecontrol-crate --test integration variable_reward_log_table -- --exact` | Wave 0 |
| RET-06 | Monthly cap uses IST month not UTC month | unit | `cargo test -p racecontrol-crate --lib psychology::tests::test_reward_cap_ist_month -- --exact` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate --lib`
- **Per wave merge:** `cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `psychology::tests` test block — 10 new unit tests listed above (add to existing `#[cfg(test)] mod tests` block in psychology.rs)
- [ ] Integration test addition in `crates/racecontrol/tests/integration.rs` — verify `variable_reward_log` table exists after migration
- [ ] Test helper: `create_test_db()` in integration.rs may need updating to include `variable_reward_log` table if it mirrors db/mod.rs manually

## Sources

### Primary (HIGH confidence)
- `/root/racecontrol/crates/racecontrol/src/psychology.rs` — full 1354-line implementation (Phase 89): update_streak, queue_notification, spawn_dispatcher, is_whatsapp_budget_exceeded, send_whatsapp, WHATSAPP_DAILY_BUDGET
- `/root/racecontrol/crates/racecontrol/src/lap_tracker.rs` lines 100-220 — PbAchieved broadcast location, personal_bests upsert pattern, track_record detection
- `/root/racecontrol/crates/racecontrol/src/billing.rs` lines 2317-2413 — post_session_hooks call chain showing hook insertion pattern
- `/root/racecontrol/crates/racecontrol/src/wallet.rs` — wallet::credit() full signature with txn_type constraint
- `/root/racecontrol/crates/racecontrol/src/db/mod.rs` lines 752-800 — wallet_transactions schema, txn_type CHECK constraint
- `/root/racecontrol/crates/racecontrol/src/db/mod.rs` lines 1269-1308 — memberships + membership_tiers schema and seeds
- `/root/racecontrol/crates/racecontrol/src/scheduler.rs` — full tick() pattern for daily guard
- `/root/racecontrol/crates/racecontrol/src/api/routes.rs` lines 15168-15208 — customer_passport response showing current streak_weeks field location
- `/root/racecontrol/pwa/src/app/passport/page.tsx` lines 185-195 — current streak display in PWA
- `/root/racecontrol/Cargo.toml` line 34 — `rand = "0.8"` workspace dependency
- `/root/racecontrol/crates/racecontrol/Cargo.toml` line 43 — `rand = { workspace = true }` crate dependency

### Secondary (MEDIUM confidence)
- Phase 89 RESEARCH.md — notification budget, nudge_queue architecture, send_whatsapp patterns
- Phase 91 RESEARCH.md — PbAchieved broadcast in lap_tracker.rs, active session polling patterns

### Tertiary (LOW confidence)
- None — all findings from direct codebase inspection

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies verified in Cargo.toml; no new dependencies
- Architecture: HIGH — every pattern has working precedent in psychology.rs, billing.rs, scheduler.rs
- Pitfalls: HIGH — identified from direct code inspection of notification throttling, DB UTC/IST patterns, and fan-out risks
- Database schema: HIGH — variable_reward_log follows exact pattern of other simple log tables in db/mod.rs
- PWA changes: HIGH — passport page fully understood, change is a 2-field response extension

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable — codebase patterns unlikely to change)
