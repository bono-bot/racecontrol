//! V2 W1-S3+S4 integration tests — 3-band refund routing handler.
//!
//! Self-contained schema setup (avoids touching the giant integration.rs
//! schema replicator). Constructs minimal AppState + drivers + audit_log
//! (with PACT-091 `action_type` sibling column) + accounts/journal tables
//! for the post_refund fire-and-forget journal-entry path.
//!
//! Tests exercise the handler function directly (not via Router::oneshot).
//! Route-level tests are deferred to a fuller integration pass post-Wave-1
//! Session 7 PR-open.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::{Extension, Json};
use racecontrol_crate::api::refund_routing::{
    route_refund, RefundReason, RouteRefundRequest,
};
use racecontrol_crate::auth::middleware::StaffClaims;
use racecontrol_crate::state::AppState;
use serde_json::Value;
use sqlx::SqlitePool;

// ─── Test setup ──────────────────────────────────────────────────────────────

async fn create_test_db() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("create in-memory pool");

    // Drivers — referenced by post_refund's journal entry description.
    sqlx::query(
        "CREATE TABLE drivers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            phone TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    // audit_log — PACT-091 shape with action_type sibling column. The
    // `action` column has a CHECK constraint that's CRUD-only; the W1-S4
    // band classification rides on `action_type`.
    sqlx::query(
        "CREATE TABLE audit_log (
            id TEXT PRIMARY KEY,
            table_name TEXT NOT NULL,
            row_id TEXT NOT NULL,
            action TEXT NOT NULL CHECK(action IN ('create', 'update', 'delete')),
            old_values TEXT,
            new_values TEXT,
            staff_id TEXT,
            ip_address TEXT,
            action_type TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Journal-entry tables. post_refund is fire-and-forget and logs errors
    // on missing tables, but creating these makes the test path silent +
    // also exercises the journal-entry write end-to-end.
    sqlx::query(
        "CREATE TABLE accounts (
            id TEXT PRIMARY KEY,
            code INTEGER NOT NULL,
            name TEXT NOT NULL,
            account_type TEXT NOT NULL,
            description TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE journal_entries (
            id TEXT PRIMARY KEY,
            description TEXT NOT NULL,
            entry_type TEXT,
            reference_id TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE journal_lines (
            id TEXT PRIMARY KEY,
            entry_id TEXT NOT NULL,
            account_id TEXT NOT NULL,
            debit_paise INTEGER NOT NULL DEFAULT 0,
            credit_paise INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    // Seed the two accounts post_refund uses.
    sqlx::query("INSERT INTO accounts (id, code, name, account_type) VALUES ('acc_refunds', 3001, 'Refunds', 'expense'), ('acc_wallet', 2001, 'Customer Wallet', 'liability')")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

fn create_test_state(pool: SqlitePool) -> Arc<AppState> {
    let config = racecontrol_crate::config::Config::default_test();
    let field_cipher = racecontrol_crate::crypto::encryption::test_field_cipher();
    Arc::new(AppState::new_with_test_v2db(config, pool, field_cipher))
}

async fn seed_driver(pool: &SqlitePool, id: &str) {
    sqlx::query("INSERT INTO drivers (id, name, phone) VALUES (?, ?, ?)")
        .bind(id)
        .bind(format!("Test Driver {}", id))
        .bind("9999999999")
        .execute(pool)
        .await
        .unwrap();
}

fn cashier_claims() -> Option<Extension<StaffClaims>> {
    Some(Extension(StaffClaims {
        sub: "cashier_alice".to_string(),
        role: "cashier".to_string(),
        exp: 9_999_999_999,
        iat: 1,
    }))
}

fn manager_claims() -> Option<Extension<StaffClaims>> {
    Some(Extension(StaffClaims {
        sub: "manager_bob".to_string(),
        role: "manager".to_string(),
        exp: 9_999_999_999,
        iat: 1,
    }))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn band_a_refund_ok_writes_audit_row_with_action_type_a() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_a1").await;
    let state = create_test_state(pool.clone());

    let req = RouteRefundRequest {
        amount_paise: 99_900, // ₹999 → Band A
        driver_id: "drv_a1".to_string(),
        reason: None,
        reference_id: Some("ref_a1".to_string()),
    };

    let (status, Json(body)) = route_refund(
        axum::extract::State(state.clone()),
        cashier_claims(),
        Json(req),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "expected 200 OK; body={}", body);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["band"], "a");
    assert_eq!(body["amount_paise"], 99_900);
    assert!(body["refund_id"].is_string());

    // audit_log row exists with action_type = "refund_3band_a"
    // Tuple types match schema: action NOT NULL / action_type NULL / new_values NULL /
    // staff_id NULL / table_name NOT NULL.
    let (action, action_type, new_values, staff_id, table_name): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT action, action_type, new_values, staff_id, table_name FROM audit_log WHERE action_type = ?",
    )
    .bind("refund_3band_a")
    .fetch_one(&pool)
    .await
    .expect("audit_log row should exist");

    assert_eq!(action, "create"); // CHECK-constraint compliant
    assert_eq!(action_type.as_deref(), Some("refund_3band_a"));
    assert_eq!(staff_id.as_deref(), Some("cashier_alice"));
    assert_eq!(table_name, "admin_actions");

    // new_values is JSON; assert reason_code is null + driver_id matches.
    let parsed: Value = serde_json::from_str(&new_values.unwrap()).unwrap();
    assert!(parsed["reason_code"].is_null(), "Band A should have no reason_code");
    assert_eq!(parsed["driver_id"], "drv_a1");
    assert_eq!(parsed["amount_paise"], 99_900);
    assert_eq!(parsed["band"], "a");
}

#[tokio::test]
async fn band_b_refund_with_valid_reason_writes_audit_row_with_reason_code() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_b1").await;
    let state = create_test_state(pool.clone());

    let req = RouteRefundRequest {
        amount_paise: 150_000, // ₹1500 → Band B
        driver_id: "drv_b1".to_string(),
        reason: Some(RefundReason::SimPs5Crash),
        reference_id: None,
    };

    let (status, Json(body)) =
        route_refund(axum::extract::State(state), cashier_claims(), Json(req)).await;

    assert_eq!(status, StatusCode::OK, "expected 200 OK; body={}", body);
    assert_eq!(body["band"], "b");
    assert_eq!(body["reason_code"], "sim_ps5_crash");

    let new_values: String = sqlx::query_scalar(
        "SELECT new_values FROM audit_log WHERE action_type = 'refund_3band_b'",
    )
    .fetch_one(&pool)
    .await
    .expect("audit_log row should exist");

    let parsed: Value = serde_json::from_str(&new_values).unwrap();
    assert_eq!(parsed["reason_code"], "sim_ps5_crash");
    assert!(parsed["reason_other_text"].is_null());
}

#[tokio::test]
async fn band_b_refund_without_reason_returns_400_no_audit_written() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_b2").await;
    let state = create_test_state(pool.clone());

    let req = RouteRefundRequest {
        amount_paise: 200_000, // ₹2000 → Band B
        driver_id: "drv_b2".to_string(),
        reason: None,
        reference_id: None,
    };

    let (status, Json(body)) =
        route_refund(axum::extract::State(state), cashier_claims(), Json(req)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "body={}", body);
    assert_eq!(body["band"], "b");
    assert!(body["error"].as_str().unwrap().contains("requires a reason"));

    // No audit_log row should have been written
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn band_c_refund_without_manager_role_returns_403() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_c1").await;
    let state = create_test_state(pool.clone());

    let req = RouteRefundRequest {
        amount_paise: 350_000, // ₹3500 → Band C
        driver_id: "drv_c1".to_string(),
        reason: Some(RefundReason::ServiceDispute),
        reference_id: None,
    };

    let (status, Json(body)) =
        route_refund(axum::extract::State(state), cashier_claims(), Json(req)).await;

    assert_eq!(status, StatusCode::FORBIDDEN, "body={}", body);
    assert_eq!(body["band"], "c");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("manager-mode"));

    // No audit_log row written
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn band_c_refund_with_manager_role_succeeds() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_c2").await;
    let state = create_test_state(pool.clone());

    let req = RouteRefundRequest {
        amount_paise: 500_000, // ₹5000 → Band C
        driver_id: "drv_c2".to_string(),
        reason: Some(RefundReason::Other("post-incident comp adjustment".to_string())),
        reference_id: Some("incident_44".to_string()),
    };

    let (status, Json(body)) =
        route_refund(axum::extract::State(state), manager_claims(), Json(req)).await;

    assert_eq!(status, StatusCode::OK, "body={}", body);
    assert_eq!(body["band"], "c");
    assert_eq!(body["reason_code"], "other");

    let new_values: String = sqlx::query_scalar(
        "SELECT new_values FROM audit_log WHERE action_type = 'refund_3band_c'",
    )
    .fetch_one(&pool)
    .await
    .expect("audit_log row should exist");
    let parsed: Value = serde_json::from_str(&new_values).unwrap();
    assert_eq!(parsed["reason_code"], "other");
    assert_eq!(parsed["reason_other_text"], "post-incident comp adjustment");

    let staff_id: String =
        sqlx::query_scalar("SELECT staff_id FROM audit_log WHERE action_type = 'refund_3band_c'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(staff_id, "manager_bob");
}

#[tokio::test]
async fn band_a_with_explicit_reason_returns_400() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_a2").await;
    let state = create_test_state(pool);

    let req = RouteRefundRequest {
        amount_paise: 50_000, // ₹500 → Band A
        driver_id: "drv_a2".to_string(),
        reason: Some(RefundReason::BookingError), // not allowed for Band A
        reference_id: None,
    };

    let (status, Json(body)) =
        route_refund(axum::extract::State(state), cashier_claims(), Json(req)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "body={}", body);
    assert_eq!(body["band"], "a");
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("does not accept a reason"));
}

#[tokio::test]
async fn zero_or_negative_amount_returns_400() {
    let pool = create_test_db().await;
    let state = create_test_state(pool);

    for bad in [0i64, -1, -100_000] {
        let req = RouteRefundRequest {
            amount_paise: bad,
            driver_id: "drv_x".to_string(),
            reason: None,
            reference_id: None,
        };

        let (status, Json(body)) = route_refund(
            axum::extract::State(state.clone()),
            cashier_claims(),
            Json(req),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST, "amount={}", bad);
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("amount_paise must be positive"));
    }
}

#[tokio::test]
async fn band_b_with_other_reason_too_long_returns_400() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_b3").await;
    let state = create_test_state(pool);

    let req = RouteRefundRequest {
        amount_paise: 200_000,
        driver_id: "drv_b3".to_string(),
        reason: Some(RefundReason::Other("a".repeat(101))),
        reference_id: None,
    };

    let (status, Json(body)) =
        route_refund(axum::extract::State(state), cashier_claims(), Json(req)).await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "body={}", body);
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("invalid reason"));
}
