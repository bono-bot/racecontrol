use super::*;
use std::sync::Arc;

// ─── Plan 01 tests (pure logic, no DB) ───────────────────────────────────

#[test]
fn test_parse_criteria_json_total_laps() {
    let json = r#"{"type":"total_laps","operator":">=","value":100}"#;
    let criteria = parse_criteria_json(json).expect("should parse");
    assert_eq!(criteria.metric_type, MetricType::TotalLaps);
    assert_eq!(criteria.value, 100);
}

#[test]
fn test_parse_criteria_json_unique_tracks() {
    let json = r#"{"type":"unique_tracks","operator":">=","value":10}"#;
    let criteria = parse_criteria_json(json).expect("should parse");
    assert_eq!(criteria.metric_type, MetricType::UniqueTracks);
}

#[test]
fn test_parse_criteria_json_first_lap() {
    let json = r#"{"type":"first_lap","operator":">=","value":1}"#;
    let criteria = parse_criteria_json(json).expect("should parse");
    assert_eq!(criteria.metric_type, MetricType::FirstLap);
}

#[test]
fn test_parse_criteria_json_invalid_returns_none() {
    assert!(parse_criteria_json("not json").is_none());
    assert!(parse_criteria_json(r#"{"type":"unknown","operator":">=","value":1}"#).is_none());
    assert!(parse_criteria_json(r#"{"type":"total_laps"}"#).is_none()); // missing fields
}

#[test]
fn test_evaluate_criteria_gte() {
    let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Gte, value: 100 };
    assert!(evaluate_criteria(&c, 100));
    assert!(evaluate_criteria(&c, 150));
    assert!(!evaluate_criteria(&c, 99));
}

#[test]
fn test_evaluate_criteria_gt() {
    let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Gt, value: 100 };
    assert!(!evaluate_criteria(&c, 100));
    assert!(evaluate_criteria(&c, 101));
}

#[test]
fn test_evaluate_criteria_eq() {
    let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Eq, value: 50 };
    assert!(evaluate_criteria(&c, 50));
    assert!(!evaluate_criteria(&c, 51));
}

#[test]
fn test_evaluate_criteria_lte() {
    let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Lte, value: 10 };
    assert!(evaluate_criteria(&c, 10));
    assert!(evaluate_criteria(&c, 5));
    assert!(!evaluate_criteria(&c, 11));
}

#[test]
fn test_evaluate_criteria_lt() {
    let c = BadgeCriteria { metric_type: MetricType::TotalLaps, operator: Operator::Lt, value: 10 };
    assert!(evaluate_criteria(&c, 9));
    assert!(!evaluate_criteria(&c, 10));
}

#[test]
fn test_notification_channel_as_str() {
    assert_eq!(NotificationChannel::Whatsapp.as_str(), "whatsapp");
    assert_eq!(NotificationChannel::Discord.as_str(), "discord");
    assert_eq!(NotificationChannel::Pwa.as_str(), "pwa");
}

#[test]
fn test_notification_channel_from_str() {
    assert_eq!(NotificationChannel::from_str("whatsapp"), Some(NotificationChannel::Whatsapp));
    assert_eq!(NotificationChannel::from_str("discord"), Some(NotificationChannel::Discord));
    assert_eq!(NotificationChannel::from_str("pwa"), Some(NotificationChannel::Pwa));
    assert_eq!(NotificationChannel::from_str("email"), None);
}

#[test]
fn test_nudge_status_as_str() {
    assert_eq!(NudgeStatus::Pending.as_str(), "pending");
    assert_eq!(NudgeStatus::Sent.as_str(), "sent");
    assert_eq!(NudgeStatus::Failed.as_str(), "failed");
    assert_eq!(NudgeStatus::Expired.as_str(), "expired");
    assert_eq!(NudgeStatus::Throttled.as_str(), "throttled");
}

#[test]
fn test_whatsapp_daily_budget_is_2() {
    assert_eq!(WHATSAPP_DAILY_BUDGET, 2);
}

// ─── Plan 02 tests (DB-backed) ────────────────────────────────────────────

/// Build an in-memory SQLite DB with psychology tables for testing.
/// Foreign key checks are disabled so tests can insert without creating drivers first.
async fn make_test_db() -> sqlx::SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("in-memory sqlite");

    // Disable foreign keys so tests can insert without parent rows
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&pool)
        .await
        .unwrap();

    // drivers table (minimal — for total_laps and phone lookups)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS drivers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            phone TEXT,
            total_laps INTEGER DEFAULT 0
        )"
    ).execute(&pool).await.unwrap();

    // achievements
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS achievements (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            criteria_json TEXT NOT NULL,
            is_active INTEGER DEFAULT 1
        )"
    ).execute(&pool).await.unwrap();

    // driver_achievements
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS driver_achievements (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            achievement_id TEXT NOT NULL,
            earned_at TEXT DEFAULT (datetime('now')),
            UNIQUE(driver_id, achievement_id)
        )"
    ).execute(&pool).await.unwrap();

    // streaks
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS streaks (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL UNIQUE,
            current_streak INTEGER NOT NULL DEFAULT 0,
            longest_streak INTEGER NOT NULL DEFAULT 0,
            last_visit_date TEXT,
            grace_expires_date TEXT,
            streak_started_at TEXT,
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(&pool).await.unwrap();

    // driving_passport (for UniqueTracks/UniqueCars metric)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS driving_passport (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            UNIQUE(driver_id, track, car)
        )"
    ).execute(&pool).await.unwrap();

    // billing_sessions (for SessionCount metric)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_sessions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            status TEXT NOT NULL
        )"
    ).execute(&pool).await.unwrap();

    // personal_bests (for PbCount metric)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS personal_bests (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL
        )"
    ).execute(&pool).await.unwrap();

    // nudge_queue
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS nudge_queue (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            channel TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 5,
            template TEXT NOT NULL,
            payload_json TEXT DEFAULT '{}',
            status TEXT NOT NULL DEFAULT 'pending',
            scheduled_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT,
            sent_at TEXT,
            error_text TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(&pool).await.unwrap();

    pool
}

/// Build a minimal AppState using the provided pool.
async fn make_state_with_db(db: sqlx::SqlitePool) -> Arc<AppState> {
    let config = crate::config::Config::default_test();
    let field_cipher = crate::crypto::encryption::test_field_cipher();
    Arc::new(AppState::new(config, db, field_cipher))
}

// ─── Badge evaluation tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_evaluate_badges_awards_badge_for_100_laps() {
    let db = make_test_db().await;
    let driver_id = "driver-badge-1";

    // Insert driver with 100 total_laps
    sqlx::query("INSERT INTO drivers (id, name, total_laps) VALUES (?, 'Test Driver', 100)")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

    // Insert achievement: total_laps >= 100
    sqlx::query("INSERT INTO achievements (id, name, criteria_json, is_active) VALUES (?, 'Century', ?, 1)")
        .bind("ach-century")
        .bind(r#"{"type":"total_laps","operator":">=","value":100}"#)
        .execute(&db)
        .await
        .unwrap();

    let state = make_state_with_db(db).await;
    evaluate_badges(&state, driver_id).await;

    // Badge should be awarded
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM driver_achievements WHERE driver_id = ? AND achievement_id = 'ach-century'"
    )
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap();

    assert_eq!(count, 1, "Badge should be awarded for 100 laps");
}

#[tokio::test]
async fn test_evaluate_badges_skips_already_earned() {
    let db = make_test_db().await;
    let driver_id = "driver-badge-2";

    sqlx::query("INSERT INTO drivers (id, name, total_laps) VALUES (?, 'Test Driver', 200)")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

    sqlx::query("INSERT INTO achievements (id, name, criteria_json, is_active) VALUES (?, 'Century', ?, 1)")
        .bind("ach-century-2")
        .bind(r#"{"type":"total_laps","operator":">=","value":100}"#)
        .execute(&db)
        .await
        .unwrap();

    // Pre-insert the earned badge
    sqlx::query("INSERT INTO driver_achievements (id, driver_id, achievement_id) VALUES (?, ?, ?)")
        .bind("da-existing")
        .bind(driver_id)
        .bind("ach-century-2")
        .execute(&db)
        .await
        .unwrap();

    let state = make_state_with_db(db).await;
    evaluate_badges(&state, driver_id).await;

    // Should still be exactly 1 row — no duplicate
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM driver_achievements WHERE driver_id = ? AND achievement_id = 'ach-century-2'"
    )
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap();

    assert_eq!(count, 1, "Badge should not be duplicated");
}

#[tokio::test]
async fn test_evaluate_badges_does_not_award_below_threshold() {
    let db = make_test_db().await;
    let driver_id = "driver-badge-3";

    // Driver has only 50 laps
    sqlx::query("INSERT INTO drivers (id, name, total_laps) VALUES (?, 'Test Driver', 50)")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

    sqlx::query("INSERT INTO achievements (id, name, criteria_json, is_active) VALUES (?, 'Century', ?, 1)")
        .bind("ach-century-3")
        .bind(r#"{"type":"total_laps","operator":">=","value":100}"#)
        .execute(&db)
        .await
        .unwrap();

    let state = make_state_with_db(db).await;
    evaluate_badges(&state, driver_id).await;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM driver_achievements WHERE driver_id = ? AND achievement_id = 'ach-century-3'"
    )
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap();

    assert_eq!(count, 0, "Badge should NOT be awarded for 50 laps (need 100)");
}

// ─── Streak tracking tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_update_streak_creates_new_row() {
    let db = make_test_db().await;
    let driver_id = "driver-streak-1";

    // Insert driver (FK off so not strictly needed, but good practice)
    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

    let state = make_state_with_db(db).await;
    update_streak(&state, driver_id).await;

    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT current_streak, longest_streak FROM streaks WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (current, longest) = row.expect("streak row should exist");
    assert_eq!(current, 1, "New streak should start at 1");
    assert_eq!(longest, 1, "New longest should start at 1");
}

#[tokio::test]
async fn test_update_streak_same_date_does_not_change() {
    let db = make_test_db().await;
    let driver_id = "driver-streak-2";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

    let state = make_state_with_db(db).await;

    // Call once to create streak
    update_streak(&state, driver_id).await;

    // Call again — should be idempotent (same IST day)
    update_streak(&state, driver_id).await;

    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT current_streak FROM streaks WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (current,) = row.expect("streak row should exist");
    assert_eq!(current, 1, "Streak should not increment when visiting same day");
}

#[tokio::test]
async fn test_update_streak_within_grace_increments() {
    let db = make_test_db().await;
    let driver_id = "driver-streak-3";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

    // Insert an existing streak with last_visit 7 days ago (within 14-day grace)
    let past_date = (chrono::Utc::now() - chrono::Duration::days(7))
        .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let future_grace = (chrono::Utc::now() + chrono::Duration::days(7))
        .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    sqlx::query(
        "INSERT INTO streaks (id, driver_id, current_streak, longest_streak, last_visit_date, grace_expires_date, streak_started_at) VALUES (?, ?, 3, 3, ?, ?, ?)"
    )
    .bind("streak-id-3")
    .bind(driver_id)
    .bind(&past_date)
    .bind(&future_grace)
    .bind(&past_date)
    .execute(&db)
    .await
    .unwrap();

    let state = make_state_with_db(db).await;
    update_streak(&state, driver_id).await;

    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT current_streak, longest_streak FROM streaks WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (current, longest) = row.expect("streak should exist");
    assert_eq!(current, 4, "Streak should increment from 3 to 4 within grace period");
    assert_eq!(longest, 4, "Longest should update when current exceeds it");
}

#[tokio::test]
async fn test_update_streak_after_grace_resets() {
    let db = make_test_db().await;
    let driver_id = "driver-streak-4";

    sqlx::query("INSERT INTO drivers (id, name) VALUES (?, 'Streaker')")
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();

    // Insert streak with grace that expired 1 day ago
    let past_date = (chrono::Utc::now() - chrono::Duration::days(30))
        .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let expired_grace = (chrono::Utc::now() - chrono::Duration::days(1))
        .with_timezone(&chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap())
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    sqlx::query(
        "INSERT INTO streaks (id, driver_id, current_streak, longest_streak, last_visit_date, grace_expires_date, streak_started_at) VALUES (?, ?, 5, 5, ?, ?, ?)"
    )
    .bind("streak-id-4")
    .bind(driver_id)
    .bind(&past_date)
    .bind(&expired_grace)
    .bind(&past_date)
    .execute(&db)
    .await
    .unwrap();

    let state = make_state_with_db(db).await;
    update_streak(&state, driver_id).await;

    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT current_streak, longest_streak FROM streaks WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (current, longest) = row.expect("streak should exist");
    assert_eq!(current, 1, "Streak should reset to 1 after grace expires");
    assert_eq!(longest, 5, "Longest should be preserved at previous high");
}

// ─── WhatsApp budget tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_budget_not_exceeded_with_zero_sent() {
    let db = make_test_db().await;
    let driver_id = "driver-budget-1";
    let state = make_state_with_db(db).await;

    let exceeded = is_whatsapp_budget_exceeded(&state, driver_id).await;
    assert!(!exceeded, "Budget should not be exceeded with 0 sent messages");
}

#[tokio::test]
async fn test_budget_not_exceeded_with_one_sent() {
    let db = make_test_db().await;
    let driver_id = "driver-budget-2";

    sqlx::query(
        "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, status, sent_at) VALUES (?, ?, 'whatsapp', 5, 'test', 'sent', datetime('now'))"
    )
    .bind("nq-1")
    .bind(driver_id)
    .execute(&db)
    .await
    .unwrap();

    let state = make_state_with_db(db).await;
    let exceeded = is_whatsapp_budget_exceeded(&state, driver_id).await;
    assert!(!exceeded, "Budget should not be exceeded with 1 sent message");
}

#[tokio::test]
async fn test_budget_exceeded_with_two_sent() {
    let db = make_test_db().await;
    let driver_id = "driver-budget-3";

    for i in 0..2 {
        sqlx::query(
            "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, status, sent_at) VALUES (?, ?, 'whatsapp', 5, 'test', 'sent', datetime('now'))"
        )
        .bind(format!("nq-budget-{}", i))
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();
    }

    let state = make_state_with_db(db).await;
    let exceeded = is_whatsapp_budget_exceeded(&state, driver_id).await;
    assert!(exceeded, "Budget should be exceeded with 2 sent messages");
}

// ─── queue_notification test ──────────────────────────────────────────────

#[tokio::test]
async fn test_queue_notification_inserts_pending_row() {
    let db = make_test_db().await;
    let driver_id = "driver-queue-1";
    let state = make_state_with_db(db).await;

    queue_notification(
        &state,
        driver_id,
        NotificationChannel::Pwa,
        3,
        "You have a new badge!",
        "{}",
    ).await;

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM nudge_queue WHERE driver_id = ?"
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (status,) = row.expect("nudge_queue row should exist");
    assert_eq!(status, "pending", "Queued notification should have status='pending'");
}

// ─── drain_notification_queue: throttle test ──────────────────────────────

#[tokio::test]
async fn test_drain_throttles_whatsapp_when_budget_exceeded() {
    let db = make_test_db().await;
    let driver_id = "driver-throttle-1";

    // Insert 2 already-sent WhatsApp messages today (budget used up)
    for i in 0..2 {
        sqlx::query(
            "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, status, sent_at) VALUES (?, ?, 'whatsapp', 5, 'prev', 'sent', datetime('now'))"
        )
        .bind(format!("nq-prev-{}", i))
        .bind(driver_id)
        .execute(&db)
        .await
        .unwrap();
    }

    // Insert a new pending WhatsApp message
    sqlx::query(
        "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, payload_json, status) VALUES (?, ?, 'whatsapp', 5, 'Hello!', '{}', 'pending')"
    )
    .bind("nq-new")
    .bind(driver_id)
    .execute(&db)
    .await
    .unwrap();

    let state = make_state_with_db(db).await;
    drain_notification_queue(&state).await.unwrap();

    // The pending message should be throttled
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM nudge_queue WHERE id = 'nq-new'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (status,) = row.expect("nudge row should exist");
    assert_eq!(status, "throttled", "WhatsApp message should be throttled when budget exceeded");
}

// ─── drain_notification_queue: expired entries ────────────────────────────

#[tokio::test]
async fn test_drain_marks_expired_entries() {
    let db = make_test_db().await;
    let driver_id = "driver-expire-1";

    // Insert a pending entry that already expired (1 hour ago)
    sqlx::query(
        "INSERT INTO nudge_queue (id, driver_id, channel, priority, template, payload_json, status, expires_at) VALUES (?, ?, 'pwa', 5, 'old', '{}', 'pending', datetime('now', '-1 hour'))"
    )
    .bind("nq-expired")
    .bind(driver_id)
    .execute(&db)
    .await
    .unwrap();

    let state = make_state_with_db(db).await;
    drain_notification_queue(&state).await.unwrap();

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT status FROM nudge_queue WHERE id = 'nq-expired'"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap();

    let (status,) = row.expect("nudge row should exist");
    assert_eq!(status, "expired", "Past-deadline entries should be marked expired");
}

// ─── resolve_template test ────────────────────────────────────────────────

#[test]
fn test_resolve_template_substitutes_placeholders() {
    let result = resolve_template("Hello {name}, you earned {badge}!", r#"{"name":"Uday","badge":"Century"}"#);
    assert_eq!(result, "Hello Uday, you earned Century!");
}

#[test]
fn test_resolve_template_plain_string_passthrough() {
    let result = resolve_template("No placeholders here.", "{}");
    assert_eq!(result, "No placeholders here.");
}
