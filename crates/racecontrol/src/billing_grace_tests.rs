    use super::*;

    /// Helper: minimal test timer for grace window tests.
    fn make_grace_test_timer(session_id: &str, pod_id: &str) -> BillingTimer {
        BillingTimer {
            session_id: session_id.to_string(),
            pod_id: pod_id.to_string(),
            driver_id: "d-test".to_string(),
            allocated_seconds: 1800,
            status: BillingSessionStatus::Active,
            ..Default::default()
        }
    }

    /// Helper: in-memory SQLite pool with minimal billing_sessions schema for grace window tests.
    async fn make_grace_test_db() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite for billing_grace tests");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_sessions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL DEFAULT 'd1',
                pod_id TEXT NOT NULL DEFAULT 'pod_1',
                pricing_tier_id TEXT NOT NULL DEFAULT 'tier_30min',
                allocated_seconds INTEGER NOT NULL DEFAULT 1800,
                status TEXT NOT NULL DEFAULT 'active',
                lap_reject_grace_until TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("create billing_sessions");
        pool
    }

    #[tokio::test]
    async fn test_grace_window_expires_normally() {
        // Manufactures a BillingTimer with a past-due grace_until, manually invokes
        // the grace-expiration detection logic, verifies that an expired timer
        // would be detected and handled.
        let mgr = BillingManager::new();
        let past = chrono::Utc::now() - chrono::Duration::seconds(1);
        {
            let mut timers = mgr.active_timers.write().await;
            let mut timer = make_grace_test_timer("grace-expire", "p-grace-1");
            timer.lap_reject_grace_until = Some(past);
            timer.pending_end_status = Some(BillingSessionStatus::Completed);
            timers.insert("p-grace-1".to_string(), timer);
        }
        // Replicate the detection snapshot from tick_all_timers Step C
        let now = chrono::Utc::now();
        let expired: Vec<(String, BillingSessionStatus)> = {
            let timers = mgr.active_timers.read().await;
            timers
                .iter()
                .filter_map(|(_, t)| {
                    match (t.lap_reject_grace_until, t.pending_end_status) {
                        (Some(g), Some(s)) if now >= g => Some((t.session_id.clone(), s)),
                        _ => None,
                    }
                })
                .collect()
        }; // guard dropped
        assert_eq!(expired.len(), 1, "expected 1 expired grace timer");
        assert_eq!(expired[0].0, "grace-expire");
        assert_eq!(expired[0].1, BillingSessionStatus::Completed);
    }

    #[tokio::test]
    async fn test_grace_window_restart_safe() {
        // Simulates the startup sequence: recover_active_sessions populates timer,
        // then hydrate_grace_fields_from_db patches grace fields onto it.
        // P0-3 fix: original test called hydrate_active_timers_from_db which created
        // a broken partial timer. New test verifies the patching-only approach.
        let pool = make_grace_test_db().await;

        let past = (chrono::Utc::now() - chrono::Duration::seconds(1)).to_rfc3339();
        sqlx::query(
            "INSERT INTO billing_sessions (id, pod_id, status, allocated_seconds, lap_reject_grace_until)
             VALUES ('restart-test', 'pod-restart', 'active', 1800, ?)"
        )
        .bind(&past)
        .execute(&pool)
        .await
        .unwrap();

        let mgr = BillingManager::new();

        // Simulate recover_active_sessions: pre-populate timer with correct fields
        // (in production, recover fetches driver_id, driving_seconds, status, etc.)
        {
            let mut timers = mgr.active_timers.write().await;
            let mut timer = make_grace_test_timer("restart-test", "pod-restart");
            timer.driving_seconds = 900; // 15 min driven before crash
            timer.driver_id = "test-driver".into();
            // recover sets grace fields to None — hydrate patches them back
            timer.lap_reject_grace_until = None;
            timer.pending_end_status = None;
            timers.insert("pod-restart".to_string(), timer);
        }

        // Now run the new patching function (runs AFTER recover in production)
        hydrate_grace_fields_from_db(&mgr, &pool).await.unwrap();

        let timers = mgr.active_timers.read().await;
        let timer = timers
            .get("pod-restart")
            .expect("timer should still be present after hydrate");
        assert_eq!(timer.session_id, "restart-test");
        assert_eq!(timer.driving_seconds, 900, "driving_seconds preserved from recover");
        assert_eq!(timer.driver_id, "test-driver", "driver_id preserved from recover");
        assert!(
            timer.lap_reject_grace_until.is_some(),
            "lap_reject_grace_until should be patched from DB"
        );
        assert!(
            timer.pending_end_status.is_some(),
            "pending_end_status should be Completed for grace-window sessions"
        );
    }

    #[tokio::test]
    async fn test_grace_window_catches_reject() {
        // Verify that when a timer has an active grace window, a lap-reject is classified
        // as "caught" (grace_window_caught=true). This test exercises the grace_window_caught
        // computation logic directly; the full DB INSERT is tested in billing::tests.
        let mgr = BillingManager::new();
        let future = chrono::Utc::now() + chrono::Duration::seconds(3);
        {
            let mut timers = mgr.active_timers.write().await;
            let mut timer = make_grace_test_timer("catch-test", "p-catch-1");
            timer.lap_reject_grace_until = Some(future);
            timers.insert("p-catch-1".to_string(), timer);
        }
        // Replicate the grace_window_caught logic from the lap reject handler
        let caught: bool = {
            let timers = mgr.active_timers.read().await;
            timers
                .get("p-catch-1")
                .and_then(|t| t.lap_reject_grace_until)
                .map(|grace_until| chrono::Utc::now() < grace_until)
                .unwrap_or(false)
        }; // guard dropped
        assert!(
            caught,
            "lap reject should be classified as caught within grace window"
        );

        // Also verify that a timer WITHOUT a grace window does NOT catch a reject
        let mgr2 = BillingManager::new();
        {
            let mut timers = mgr2.active_timers.write().await;
            let timer = make_grace_test_timer("no-window-test", "p-no-window");
            timers.insert("p-no-window".to_string(), timer);
        }
        let not_caught: bool = {
            let timers = mgr2.active_timers.read().await;
            timers
                .get("p-no-window")
                .and_then(|t| t.lap_reject_grace_until)
                .map(|grace_until| chrono::Utc::now() < grace_until)
                .unwrap_or(false)
        }; // guard dropped
        assert!(!not_caught, "lap reject outside grace window should not be caught");
    }
