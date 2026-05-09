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
use std::time::Instant;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Extension, Json, Router};
use racecontrol_crate::api::refund_routing::{
    route_refund, RefundReason, RouteRefundRequest,
};
use racecontrol_crate::auth::middleware::StaffClaims;
use racecontrol_crate::state::AppState;
use serde_json::Value;
use sqlx::SqlitePool;
use tokio::task::JoinSet;
use tower::ServiceExt;

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

// ─── Item 5 (NOT TESTED → tested): Router::oneshot HTTP-layer ───────────────
//
// Validates the actual axum routing layer + JSON body deserialization +
// Extension extraction end-to-end through the framework, not just direct
// handler invocation. JWT signature validation path (item 9) remains gated
// because that requires going through `require_staff_jwt` middleware on
// the FULL router with a signed bearer token — kaizen-min stays here.

fn build_minimal_router(state: Arc<AppState>, claims: Option<StaffClaims>) -> Router {
    let mut r = Router::new()
        .route("/api/v1/refund/3band", post(route_refund))
        .with_state(state);
    if let Some(c) = claims {
        r = r.layer(Extension(c));
    }
    r
}

#[tokio::test]
async fn router_oneshot_band_a_returns_200_via_real_http_layer() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_router_a").await;
    let state = create_test_state(pool.clone());

    let app = build_minimal_router(
        state,
        Some(StaffClaims {
            sub: "router_cashier".to_string(),
            role: "cashier".to_string(),
            exp: 9_999_999_999,
            iat: 1,
        }),
    );

    let payload = serde_json::json!({
        "amount_paise": 50_000,
        "driver_id": "drv_router_a",
        "reason": null,
        "reference_id": "router_ref_a"
    })
    .to_string();

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/refund/3band")
        .header("content-type", "application/json")
        .body(Body::from(payload))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");
    assert_eq!(resp.status(), StatusCode::OK);

    let body_bytes = to_bytes(resp.into_body(), 64 * 1024)
        .await
        .expect("read body");
    let body: Value = serde_json::from_slice(&body_bytes).expect("parse json");
    assert_eq!(body["status"], "ok");
    assert_eq!(body["band"], "a");

    // Through the router, the audit_log row should exist with staff_id from
    // the layered Extension (proves Extension extraction worked at the http layer).
    let staff_id: Option<String> = sqlx::query_scalar(
        "SELECT staff_id FROM audit_log WHERE action_type = 'refund_3band_a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(staff_id.as_deref(), Some("router_cashier"));
}

#[tokio::test]
async fn router_oneshot_rejects_unknown_field_via_deny_unknown_fields() {
    let pool = create_test_db().await;
    seed_driver(&pool, "drv_router_unknown").await;
    let state = create_test_state(pool.clone());

    let app = build_minimal_router(
        state,
        Some(StaffClaims {
            sub: "anyone".to_string(),
            role: "cashier".to_string(),
            exp: 9_999_999_999,
            iat: 1,
        }),
    );

    // Payload includes `extra_field` which the deny_unknown_fields serde
    // attribute should reject at deserialization time. axum returns 4xx
    // when the Json extractor fails to parse the body.
    let payload = serde_json::json!({
        "amount_paise": 50_000,
        "driver_id": "drv_router_unknown",
        "extra_field": "should_be_rejected"
    })
    .to_string();

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/refund/3band")
        .header("content-type", "application/json")
        .body(Body::from(payload))
        .expect("build request");

    let resp = app.oneshot(req).await.expect("oneshot");
    assert!(
        resp.status().is_client_error(),
        "expected 4xx for unknown field, got {}",
        resp.status()
    );

    // No audit row should be written when validation fails before handler runs.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ─── Item 10 (NOT TESTED → smoke-tested): Concurrent-writer ──────────────────
//
// SQLite SqlitePool with max_connections=1 serializes writes structurally,
// so this test isn't true concurrent contention — it's a smoke test that
// 20 parallel `route_refund` invocations don't panic / deadlock and produce
// exactly N audit rows. True multi-writer load testing under WAL+max_connections>1
// is deferred to Wave-1 Session-7 + post-Wave-1 (PACT-018 wallet HOLD-RELEASE
// state machine, Wave 3).

#[tokio::test]
async fn concurrent_band_a_route_refund_writes_distinct_audit_rows() {
    const N: usize = 20;

    let pool = create_test_db().await;
    // Seed N distinct drivers up-front (each task targets its own driver
    // to avoid FK contention surprising the smoke test scope).
    for i in 0..N {
        seed_driver(&pool, &format!("drv_conc_{i:02}")).await;
    }
    let state = create_test_state(pool.clone());

    let mut set = JoinSet::new();
    for i in 0..N {
        let st = state.clone();
        set.spawn(async move {
            let req = RouteRefundRequest {
                amount_paise: 50_000 + (i as i64), // distinct amounts for assert
                driver_id: format!("drv_conc_{i:02}"),
                reason: None,
                reference_id: Some(format!("conc_ref_{i:02}")),
            };
            route_refund(
                axum::extract::State(st),
                Some(Extension(StaffClaims {
                    sub: format!("conc_staff_{i:02}"),
                    role: "cashier".to_string(),
                    exp: 9_999_999_999,
                    iat: 1,
                })),
                Json(req),
            )
            .await
        });
    }

    let mut ok_count = 0;
    while let Some(joined) = set.join_next().await {
        let (status, Json(body)) = joined.expect("task panicked");
        assert_eq!(status, StatusCode::OK, "task body={}", body);
        ok_count += 1;
    }
    assert_eq!(ok_count, N);

    // All N audit rows present + each carries a distinct staff_id (proves no
    // overwrite/race-overwrite at the audit-log layer).
    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action_type = 'refund_3band_a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row_count, N as i64);

    let distinct_staff_ids: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT staff_id) FROM audit_log WHERE action_type = 'refund_3band_a'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(distinct_staff_ids, N as i64);
}

// ─── Item 11 (NOT TESTED → smoke-tested): audit_log INSERT perf smoke ────────
//
// Mirrors the cirs_lookup `perf_smoke_fetch_profile_preview_p95_under_500ms`
// pattern. N=100 sequential `route_refund` Band A calls; measures p50/p95/p99
// end-to-end latencies. Soft regression detector for INSERT-path performance
// degradation (e.g. lost index, lock contention, audit_log table bloat).
//
// Gate: p95 ≤ 100ms (debug build, in-memory SQLite, 5 indexes on audit_log).
// Production indexed audit_log on a real disk should observe p95 well under
// 5ms; 100ms is a generous regression gate, not a benchmark.

#[tokio::test]
async fn perf_smoke_route_refund_band_a_p95_under_100ms() {
    const ITERATIONS: usize = 100;

    let pool = create_test_db().await;
    seed_driver(&pool, "drv_perf").await;
    let state = create_test_state(pool.clone());

    // Warmup: one call to settle SQLite page cache + journal init.
    let _ = route_refund(
        axum::extract::State(state.clone()),
        cashier_claims(),
        Json(RouteRefundRequest {
            amount_paise: 50_000,
            driver_id: "drv_perf".to_string(),
            reason: None,
            reference_id: None,
        }),
    )
    .await;

    let mut latencies_us: Vec<u128> = Vec::with_capacity(ITERATIONS);
    for i in 0..ITERATIONS {
        let req = RouteRefundRequest {
            amount_paise: 50_000 + (i as i64),
            driver_id: "drv_perf".to_string(),
            reason: None,
            reference_id: Some(format!("perf_ref_{i:03}")),
        };
        let start = Instant::now();
        let (status, _) = route_refund(
            axum::extract::State(state.clone()),
            cashier_claims(),
            Json(req),
        )
        .await;
        let elapsed = start.elapsed().as_micros();
        assert_eq!(status, StatusCode::OK, "iter {}", i);
        latencies_us.push(elapsed);
    }

    latencies_us.sort_unstable();
    let p50 = latencies_us[ITERATIONS / 2];
    let p95 = latencies_us[(ITERATIONS as f64 * 0.95) as usize];
    let p99 = latencies_us[(ITERATIONS as f64 * 0.99) as usize];
    let max = *latencies_us.last().expect("non-empty");

    eprintln!(
        "[perf refund_3band] N={ITERATIONS}  p50={p50}us  p95={p95}us  p99={p99}us  max={max}us"
    );

    // Soft gate: p95 < 100ms (debug build + in-memory SQLite). Production
    // path against indexed disk audit_log should be ~10x faster.
    assert!(
        p95 < 100_000,
        "p95={p95}us exceeds 100_000us gate — perf regression on audit_log INSERT path"
    );
}
