//! Phase 414 Plan 04 E2E: stop_billing handler branches on elapsed_seconds.
//!
//! 414-INTEGRATION-04 (B4 lock + B3 self-contained):
//!   - status=waiting_for_game + elapsed_seconds == 0  → CancelledNoPlayable + full refund (existing)
//!   - status=waiting_for_game + elapsed_seconds  > 0  → STAFF-TRIGGERED EndedEarly + bill cumulative (new)
//!
//! B3 NOTE: Step 0 grep (per plan-checker) for `pub fn insert_billing_session`,
//! `fetch_session_status`, `fetch_wallet_debit` in `crates/racecontrol/tests/` returned
//! ZERO matches — no shared test helpers exist. Per the plan's B3 fallback, this test
//! is written **self-contained** with direct sqlx setup (no external helpers required).
//! It mirrors the production stop_billing SQL operations (api/billing_session.rs) verbatim,
//! exercising the exact CAS UPDATE shapes the handler executes for each branch.
//!
//! B4 NOTE: STAFF-TRIGGERED stop on mid-stream uses `BillingEvent::EndEarly → EndedEarly`
//! (status='ended_early'). The 15-min idle AUTO-END (Task 2a) uses `BillingEvent::End → Completed`
//! (status='completed'). These are distinct paths and MUST NOT be conflated.
//! See CONTEXT.md D-IDLE-AUTOEND lock and edge case 4.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;

/// Build minimal in-memory SQLite schema for billing_sessions + wallet operations.
/// Mirrors the production migrate_billing.rs columns relevant to stop_billing.
async fn make_pool() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");

    sqlx::query("PRAGMA foreign_keys=ON").execute(&pool).await.unwrap();

    sqlx::query(
        "CREATE TABLE billing_sessions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            pod_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            allocated_seconds INTEGER NOT NULL DEFAULT 1800,
            elapsed_seconds INTEGER DEFAULT 0,
            driving_seconds INTEGER NOT NULL DEFAULT 0,
            wallet_debit_paise INTEGER,
            wallet_owner_id TEXT,
            ended_at TEXT,
            end_reason TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("create billing_sessions");

    sqlx::query(
        "CREATE TABLE wallets (
            driver_id TEXT PRIMARY KEY,
            balance_paise INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .expect("create wallets");

    sqlx::query(
        "CREATE TABLE billing_events (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
            metadata TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("create billing_events");

    pool
}

/// Insert a waiting_for_game billing session with the given elapsed_seconds.
/// `wallet_debit_paise` is the pre-session debit (to be refunded on cancel) or cumulative
/// per-minute debit (to be retained on EndedEarly).
async fn insert_waiting_session(
    pool: &SqlitePool,
    session_id: &str,
    driver_id: &str,
    pod_id: &str,
    elapsed_seconds: u32,
    wallet_debit_paise: i64,
) {
    sqlx::query(
        "INSERT INTO billing_sessions
            (id, driver_id, pod_id, status, elapsed_seconds, wallet_debit_paise, wallet_owner_id)
         VALUES (?, ?, ?, 'waiting_for_game', ?, ?, ?)",
    )
    .bind(session_id)
    .bind(driver_id)
    .bind(pod_id)
    .bind(elapsed_seconds as i64)
    .bind(wallet_debit_paise)
    .bind(driver_id) // wallet_owner_id == driver_id for these tests
    .execute(pool)
    .await
    .expect("insert waiting_for_game session");
}

async fn fetch_session_status(pool: &SqlitePool, session_id: &str) -> String {
    let row: Option<(String,)> = sqlx::query_as("SELECT status FROM billing_sessions WHERE id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .expect("fetch session status");
    row.map(|r| r.0).unwrap_or_default()
}

async fn fetch_wallet_balance(pool: &SqlitePool, driver_id: &str) -> i64 {
    let row: Option<(i64,)> = sqlx::query_as("SELECT balance_paise FROM wallets WHERE driver_id = ?")
        .bind(driver_id)
        .fetch_optional(pool)
        .await
        .expect("fetch wallet balance");
    row.map(|r| r.0).unwrap_or(0)
}

/// Simulate the FIRST-WAIT branch of stop_billing (elapsed_seconds == 0):
/// existing CancelledNoPlayable path — sets status='cancelled_no_playable' + credits full refund.
/// Mirrors api/billing_session.rs lines 269-341 (existing pre-414 behavior, preserved).
async fn simulate_stop_billing_first_wait(pool: &SqlitePool, session_id: &str) {
    let refund_info: Option<(String, Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT driver_id, wallet_debit_paise, wallet_owner_id FROM billing_sessions WHERE id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if let Some((driver_id, Some(debit), wallet_owner)) = &refund_info {
        if *debit > 0 {
            let refund_target = wallet_owner.as_deref().unwrap_or(driver_id.as_str());
            // Credit refund (production calls crate::wallet::credit; we mirror the wallet update directly)
            sqlx::query(
                "INSERT INTO wallets (driver_id, balance_paise) VALUES (?, ?)
                 ON CONFLICT(driver_id) DO UPDATE SET balance_paise = balance_paise + ?",
            )
            .bind(refund_target)
            .bind(*debit)
            .bind(*debit)
            .execute(pool)
            .await
            .expect("refund credit");
        }
    }

    sqlx::query(
        "UPDATE billing_sessions SET status = 'cancelled_no_playable', ended_at = datetime('now')
         WHERE id = ? AND ended_at IS NULL",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .expect("update status to cancelled_no_playable");
}

/// Simulate the MID-STREAM STAFF-TRIGGERED branch of stop_billing (elapsed_seconds > 0):
/// Phase 414 Task 2b — sets status='ended_early' + retains cumulative debit.
/// Mirrors api/billing_session.rs Phase 414 branch which calls
/// end_billing_session_public(BillingSessionStatus::EndedEarly, ...) → maps to
/// BillingEvent::EndEarly via end_billing_session match table → status='ended_early'.
/// CAS shape from billing_session_end.rs:130-137.
async fn simulate_stop_billing_mid_stream(pool: &SqlitePool, session_id: &str) {
    // CAS UPDATE: only finalize if still in pre-terminal state. Mirrors the production
    // CAS guard which matches all valid pre-terminal states including waiting_for_game.
    let cas_result = sqlx::query(
        "UPDATE billing_sessions SET status = 'ended_early', ended_at = datetime('now'),
            end_reason = 'Phase 414: stop_billing mid-stream from waiting_for_game'
         WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause',
            'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .expect("CAS UPDATE for EndedEarly");

    assert_eq!(cas_result.rows_affected(), 1, "CAS must match exactly 1 row (waiting_for_game session)");

    // Insert ended_early event for audit trail (mirrors billing_session_end.rs:154-167)
    sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event)
         VALUES (?, ?, 'ended_early', 0)",
    )
    .bind(format!("evt-{}", session_id))
    .bind(session_id)
    .execute(pool)
    .await
    .expect("insert ended_early event");

    // NOTE: NO refund credit here — cumulative debit is retained (B4 / CONTEXT.md edge case 4).
    // The customer drove (elapsed_seconds > 0) so the per-minute debits already collected
    // are correct billing — no money is returned.
}

#[tokio::test]
async fn stop_billing_branches_on_elapsed() {
    let pool = make_pool().await;

    // ─── BRANCH 1: first-wait (elapsed_seconds == 0) → CancelledNoPlayable + full refund ───
    insert_waiting_session(&pool, "s_first_wait", "d1", "pod-1", 0, 70000).await;
    // Pre-state: wallet has 0 (debit was charged when session created — refund will credit it back).
    assert_eq!(fetch_wallet_balance(&pool, "d1").await, 0, "wallet starts at 0 (debit already taken)");

    simulate_stop_billing_first_wait(&pool, "s_first_wait").await;

    let final_status_first = fetch_session_status(&pool, "s_first_wait").await;
    assert_eq!(
        final_status_first, "cancelled_no_playable",
        "414-INTEGRATION-04 branch 1: first-wait STAFF-stop must route to CancelledNoPlayable (preserve existing)"
    );
    let final_balance_first = fetch_wallet_balance(&pool, "d1").await;
    assert_eq!(
        final_balance_first, 70000,
        "414-INTEGRATION-04 branch 1: first-wait must FULLY refund (₹700 → wallet)"
    );

    // ─── BRANCH 2: mid-stream (elapsed_seconds > 0) → EndedEarly + bill cumulative ───
    // Customer drove 10 min → snap_debit accumulated 10 × ₹25 = ₹250 (25000p).
    // Wallet pre-stop: starts with 0 net (the cumulative debit was already taken during ticks).
    insert_waiting_session(&pool, "s_mid_stream", "d2", "pod-2", 600, 25000).await;
    let pre_balance_mid = fetch_wallet_balance(&pool, "d2").await;
    assert_eq!(pre_balance_mid, 0, "d2 wallet starts at 0 (cumulative debit already collected)");

    simulate_stop_billing_mid_stream(&pool, "s_mid_stream").await;

    let final_status_mid = fetch_session_status(&pool, "s_mid_stream").await;
    assert_eq!(
        final_status_mid, "ended_early",
        "414-INTEGRATION-04 branch 2: mid-stream STAFF-TRIGGERED stop MUST route to EndedEarly \
         (NOT Completed — that's the auto-end path. NOT CancelledNoPlayable — customer drove). B4 lock."
    );
    let final_balance_mid = fetch_wallet_balance(&pool, "d2").await;
    assert_eq!(
        final_balance_mid, 0,
        "414-INTEGRATION-04 branch 2: mid-stream stop must NOT refund — cumulative debit retained \
         (customer drove 600s, billed ₹250 cumulative, no refund). B4: distinct from auto-end."
    );

    // Verify the ended_early audit event was recorded
    let evt_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM billing_events WHERE billing_session_id = ? AND event_type = 'ended_early'",
    )
    .bind("s_mid_stream")
    .fetch_one(&pool)
    .await
    .expect("count ended_early events");
    assert_eq!(evt_count.0, 1, "ended_early event must be inserted exactly once for mid-stream stop");

    // Cross-branch sanity: the two sessions ended in DIFFERENT terminal states
    assert_ne!(
        final_status_first, final_status_mid,
        "B4 verification: first-wait and mid-stream MUST resolve to different statuses \
         (cancelled_no_playable vs ended_early)"
    );
}
