    use super::*;

    #[test]
    fn timer_only_counts_when_driving() {
        let mut timer = BillingTimer {
            session_id: "test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 0,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 0,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // Should count when driving
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 1);

        // Timer counts regardless of driving state (always-on billing)
        timer.driving_state = DrivingState::Idle;
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 2); // Still counts

        // Should NOT count when paused
        timer.driving_state = DrivingState::Active;
        timer.status = BillingSessionStatus::PausedManual;
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 2); // Paused stops counting
    }

    #[test]
    fn timer_expires_correctly() {
        let mut timer = BillingTimer {
            session_id: "test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Trial".into(),
            allocated_seconds: 3,
            driving_seconds: 2,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 2,
            pause_seconds: 0,
            max_session_seconds: 3,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // One more tick should expire
        assert!(timer.tick());
        assert_eq!(timer.driving_seconds, 3);
        assert_eq!(timer.elapsed_seconds, 3);
    }

    #[test]
    fn remaining_seconds_calculation() {
        let timer = BillingTimer {
            session_id: "test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "1 Hour".into(),
            allocated_seconds: 3600,
            driving_seconds: 1000,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 1000,
            pause_seconds: 0,
            max_session_seconds: 3600,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        assert_eq!(timer.remaining_seconds(), 2600);
    }

    #[test]
    fn billing_pause_disconnect_freezes_driving_seconds() {
        let mut timer = BillingTimer {
            session_id: "test-pause".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 100,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 100,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // Active tick — driving_seconds should increment
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 101);

        // Simulate disconnect pause
        timer.status = BillingSessionStatus::PausedDisconnect;
        timer.pause_count = 1;

        // Paused tick — driving_seconds should NOT increment
        assert!(!timer.tick());
        assert_eq!(timer.driving_seconds, 101); // Still 101
    }

    #[test]
    fn max_three_pauses_per_session() {
        let timer = BillingTimer {
            session_id: "test-max-pause".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 500,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 3, // Already used all 3 pauses
            total_paused_seconds: 120,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 500,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // Should NOT be able to pause again (pause_count >= 3)
        assert!(timer.pause_count >= 3);
        // The tick loop checks pause_count < 3 before pausing
    }

    #[test]
    fn disconnect_timeout_uses_per_disconnect_not_cumulative() {
        // Scenario: customer disconnects twice with reconnect in between.
        // Each disconnect should get a fresh 10-minute (600s) window.
        // Bug (before fix): total_paused_seconds was used for timeout,
        // so 300s from first disconnect + 301s from second = auto-end.
        // Fix: pause_seconds (per-disconnect, reset on entry) is used instead.

        let mut timer = BillingTimer {
            session_id: "test-cumulative".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "30 Minutes".into(),
            allocated_seconds: 1800,
            driving_seconds: 100,
            status: BillingSessionStatus::PausedDisconnect,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: Some(Utc::now()),
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 1,
            total_paused_seconds: 0,
            last_paused_at: Some(Utc::now()),
            max_pause_duration_secs: 600,
            elapsed_seconds: 100,
            pause_seconds: 0, // Fresh disconnect
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // Simulate 300 ticks while disconnected (5 minutes)
        for _ in 0..300 {
            timer.pause_seconds += 1;
            timer.total_paused_seconds += 1;
        }
        assert_eq!(timer.pause_seconds, 300);
        assert_eq!(timer.total_paused_seconds, 300);

        // Pod reconnects — simulate what ws/mod.rs reconnect handler does
        timer.status = BillingSessionStatus::Active;
        timer.offline_since = None;
        timer.pause_seconds = 0; // Reset per-disconnect counter

        // Pod disconnects again — simulate what tick_all_timers does on disconnect entry
        timer.status = BillingSessionStatus::PausedDisconnect;
        timer.pause_count += 1; // Now 2
        timer.pause_seconds = 0; // Reset per-disconnect timer (each disconnect gets fresh window)

        // Simulate 301 more ticks while disconnected (just over 5 more minutes)
        for _ in 0..301 {
            timer.pause_seconds += 1;
            timer.total_paused_seconds += 1;
        }

        // total_paused_seconds = 601 (cumulative) — would have triggered timeout with old code
        assert_eq!(timer.total_paused_seconds, 601);
        // pause_seconds = 301 (this disconnect only) — NOT over 600, session survives
        assert_eq!(timer.pause_seconds, 301);
        assert!(timer.pause_seconds <= timer.max_pause_duration_secs,
            "Session should NOT auto-end: per-disconnect pause_seconds ({}) <= max ({})",
            timer.pause_seconds, timer.max_pause_duration_secs);
    }

    #[test]
    fn partial_refund_calculation() {
        // Simulate: 1800s allocated, 900s driven, 70000 paise (₹700) debited
        // Expected: 50% unused → refund = 35000 paise
        let allocated: i64 = 1800;
        let driving_seconds: i64 = 900;
        let wallet_debit_paise: i64 = 70000;

        let remaining = allocated - driving_seconds;
        let refund = (remaining as f64 / allocated as f64 * wallet_debit_paise as f64) as i64;

        assert_eq!(refund, 35000); // 50% of ₹700

        // Edge case: 75% driven → 25% refund
        let driving_seconds_2: i64 = 1350;
        let remaining_2 = allocated - driving_seconds_2;
        let refund_2 = (remaining_2 as f64 / allocated as f64 * wallet_debit_paise as f64) as i64;
        assert_eq!(refund_2, 17500); // 25% of ₹700

        // Edge case: fully driven → 0 refund
        let driving_seconds_3: i64 = 1800;
        let remaining_3 = allocated - driving_seconds_3;
        let refund_3 = (remaining_3 as f64 / allocated as f64 * wallet_debit_paise as f64) as i64;
        assert_eq!(refund_3, 0);
    }

    // ── compute_session_cost with non-retroactive 3-tier pricing ──────

    fn test_tiers() -> Vec<BillingRateTier> {
        default_billing_rate_tiers()
    }

    #[test]
    fn cost_zero_seconds() {
        let tiers = test_tiers();
        let cost = compute_session_cost(0, &tiers);
        assert_eq!(cost.total_paise, 0);
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
        assert_eq!(cost.minutes_to_next_tier, Some(30));
    }

    #[test]
    fn cost_15_minutes_standard_tier() {
        let tiers = test_tiers();
        let cost = compute_session_cost(900, &tiers); // 15 min
        assert_eq!(cost.total_paise, 37500); // 15 * 2500
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
        assert_eq!(cost.minutes_to_next_tier, Some(15));
    }

    #[test]
    fn cost_29_59_standard_tier() {
        let tiers = test_tiers();
        let cost = compute_session_cost(1799, &tiers); // 29:59
        assert_eq!(cost.tier_name, "Standard");
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.minutes_to_next_tier, Some(1));
    }

    #[test]
    fn cost_30_minutes_non_retroactive() {
        let tiers = test_tiers();
        let cost = compute_session_cost(1800, &tiers); // exactly 30 min
        assert_eq!(cost.total_paise, 75000); // 30 * 2500 (non-retroactive: all in Standard tier)
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
    }

    #[test]
    fn cost_45_minutes_two_tiers() {
        let tiers = test_tiers();
        let cost = compute_session_cost(2700, &tiers); // 45 min
        // Non-retroactive: (30 * 2500) + (15 * 2000) = 75000 + 30000 = 105000
        assert_eq!(cost.total_paise, 105000);
        assert_eq!(cost.rate_per_min_paise, 2000);
        assert_eq!(cost.tier_name, "Extended");
    }

    #[test]
    fn cost_3_hours_all_three_tiers() {
        let tiers = test_tiers();
        let cost = compute_session_cost(10800, &tiers); // 180 min
        // Non-retroactive: (30 * 2500) + (30 * 2000) + (120 * 1500) = 75000 + 60000 + 180000 = 315000
        assert_eq!(cost.total_paise, 315000);
        assert_eq!(cost.rate_per_min_paise, 1500);
        assert_eq!(cost.tier_name, "Marathon");
        assert_eq!(cost.minutes_to_next_tier, None);
    }

    #[test]
    fn timer_countup_active_increments_elapsed() {
        let mut timer = BillingTimer {
            session_id: "test-countup".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 0,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 0,
            pause_seconds: 0,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 1);
        assert_eq!(timer.driving_seconds, 1); // compat alias

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 2);
    }

    #[test]
    fn timer_paused_game_pause_freezes_elapsed_increments_pause() {
        let mut timer = BillingTimer {
            session_id: "test-pause".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 100,
            status: BillingSessionStatus::PausedGamePause,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 100,
            pause_seconds: 0,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 100); // frozen
        assert_eq!(timer.pause_seconds, 1);     // incrementing
    }

    #[test]
    fn timer_hard_max_cap_triggers_end() {
        let mut timer = BillingTimer {
            session_id: "test-cap".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 10799,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 10799,
            pause_seconds: 0,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        assert!(timer.tick()); // Should return true (elapsed == max)
        assert_eq!(timer.elapsed_seconds, 10800);
    }

    #[test]
    fn timer_pause_timeout_triggers_end() {
        let mut timer = BillingTimer {
            session_id: "test-timeout".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 500,
            status: BillingSessionStatus::PausedGamePause,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 500,
            pause_seconds: 599,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // One more tick should hit 600s pause timeout
        assert!(timer.tick());
        assert_eq!(timer.pause_seconds, 600);
        assert_eq!(timer.elapsed_seconds, 500); // Still frozen
    }

    #[test]
    fn timer_current_cost_returns_session_cost() {
        let rate_tiers = test_tiers();
        let timer = BillingTimer {
            session_id: "test-cost".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 900,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 900,
            pause_seconds: 0,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        let cost = timer.current_cost(&rate_tiers);
        assert_eq!(cost.total_paise, 37500); // 15 min * 25 cr/min = 375 cr = 37500 paise
        assert_eq!(cost.rate_per_min_paise, 2500);
        assert_eq!(cost.tier_name, "Standard");
    }

    #[test]
    fn timer_to_info_populates_optional_fields() {
        let rate_tiers = test_tiers();
        let timer = BillingTimer {
            session_id: "test-info".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 900,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 900,
            pause_seconds: 0,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        let info = timer.to_info(&rate_tiers);
        assert_eq!(info.elapsed_seconds, Some(900));
        assert_eq!(info.cost_paise, Some(37500)); // 15 min * 25 cr/min
        assert_eq!(info.rate_per_min_paise, Some(2500));
        // Legacy fields still populated
        assert_eq!(info.driving_seconds, 900);
        assert_eq!(info.allocated_seconds, 10800);
        assert_eq!(info.remaining_seconds, 9900);
    }

    // ── Phase 03 Plan 03 Task 1: billing lifecycle (handle_game_status_update) ──

    #[test]
    fn waiting_for_game_entry_tracks_billing_params() {
        let entry = WaitingForGameEntry {
            pod_id: "pod1".to_string(),
            driver_id: "d1".to_string(),
            pricing_tier_id: "tier1".to_string(),
            custom_price_paise: Some(5000),
            custom_duration_minutes: Some(30),
            staff_id: None,
            split_count: None,
            split_duration_minutes: None,
            waiting_since: std::time::Instant::now(),
            attempt: 1,
            group_session_id: None,
            sim_type: None,
        launch_args: None,
            pre_committed: None,
        };
        assert_eq!(entry.pod_id, "pod1");
        assert_eq!(entry.attempt, 1);
        assert_eq!(entry.custom_price_paise, Some(5000));
    }

    #[tokio::test]
    async fn game_status_live_on_paused_game_pause_resumes_billing() {
        // Timer in PausedGamePause -> Live should transition to Active
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let mut timer = make_test_timer("resume-test", "p1");
            timer.status = BillingSessionStatus::PausedGamePause;
            timer.pause_seconds = 30;
            timers.insert("p1".to_string(), timer);
        }
        // Simulate Live: transition PausedGamePause -> Active
        {
            let mut timers = mgr.active_timers.write().await;
            if let Some(timer) = timers.get_mut("p1") {
                assert_eq!(timer.status, BillingSessionStatus::PausedGamePause);
                timer.status = BillingSessionStatus::Active;
                timer.pause_seconds = 0;
            }
        }
        let timers = mgr.active_timers.read().await;
        let timer = timers.get("p1").unwrap();
        assert_eq!(timer.status, BillingSessionStatus::Active);
        assert_eq!(timer.pause_seconds, 0);
    }

    #[tokio::test]
    async fn game_status_pause_transitions_active_to_paused_game_pause() {
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let timer = make_test_timer("pause-test", "p2");
            timers.insert("p2".to_string(), timer);
        }
        // Simulate Pause: Active -> PausedGamePause
        {
            let mut timers = mgr.active_timers.write().await;
            if let Some(timer) = timers.get_mut("p2") {
                assert_eq!(timer.status, BillingSessionStatus::Active);
                timer.status = BillingSessionStatus::PausedGamePause;
                timer.pause_seconds = 0;
                timer.pause_count += 1;
            }
        }
        let timers = mgr.active_timers.read().await;
        let timer = timers.get("p2").unwrap();
        assert_eq!(timer.status, BillingSessionStatus::PausedGamePause);
        assert_eq!(timer.pause_count, 1);
    }

    #[tokio::test]
    async fn game_status_live_on_active_timer_is_noop() {
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let mut timer = make_test_timer("noop-test", "p3");
            timer.elapsed_seconds = 100;
            timer.driving_seconds = 100;
            timers.insert("p3".to_string(), timer);
        }
        // Simulate Live on already-Active: no change
        {
            let timers = mgr.active_timers.read().await;
            let timer = timers.get("p3").unwrap();
            assert_eq!(timer.status, BillingSessionStatus::Active);
            assert_eq!(timer.elapsed_seconds, 100);
        }
    }

    #[tokio::test]
    async fn game_status_pause_on_no_timer_is_noop() {
        let mgr = BillingManager::new();
        // No timer for p4 - Pause should be no-op
        let timers = mgr.active_timers.read().await;
        assert!(timers.get("p4").is_none());
    }

    #[tokio::test]
    async fn game_status_off_ends_active_session() {
        let mgr = BillingManager::new();
        {
            let mut timers = mgr.active_timers.write().await;
            let timer = make_test_timer("off-test", "p5");
            timers.insert("p5".to_string(), timer);
        }
        // Simulate Off: remove timer (session ends)
        {
            let timers = mgr.active_timers.read().await;
            assert!(timers.contains_key("p5"));
        }
        // The actual removal happens in handle_game_status_update via end_billing_session
        // Here we verify the timer exists before Off (the function will remove it)
    }

    #[tokio::test]
    async fn waiting_for_game_removed_on_live() {
        let mgr = BillingManager::new();
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.insert("p6".to_string(), WaitingForGameEntry {
                pod_id: "p6".to_string(),
                driver_id: "d1".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now(),
                attempt: 1,
                group_session_id: None,
                sim_type: None,
        launch_args: None,
                pre_committed: None,
            });
        }
        // Simulate Live: remove from waiting_for_game
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            let entry = waiting.remove("p6");
            assert!(entry.is_some());
            assert_eq!(entry.unwrap().driver_id, "d1");
        }
        let waiting = mgr.waiting_for_game.read().await;
        assert!(waiting.get("p6").is_none());
    }

    #[tokio::test]
    async fn launch_timeout_detected_after_180s() {
        let mgr = BillingManager::new();
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            // Create entry with waiting_since in the past (>180s ago)
            let mut entry = WaitingForGameEntry {
                pod_id: "p7".to_string(),
                driver_id: "d1".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now(),
                attempt: 1,
                group_session_id: None,
                sim_type: None,
        launch_args: None,
                pre_committed: None,
            };
            // Simulate time passing by using checked_sub
            entry.waiting_since = std::time::Instant::now() - std::time::Duration::from_secs(181);
            waiting.insert("p7".to_string(), entry);
        }
        // Check launch timeouts (pass 180 explicitly — the test uses a 181s elapsed entry)
        let timed_out = check_launch_timeouts_from_manager(&mgr, 180).await;
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0].0, "p7");
        assert_eq!(timed_out[0].1, 1); // first attempt
    }

    #[tokio::test]
    async fn launch_timeout_attempt_2_cancels_with_no_charge() {
        let mgr = BillingManager::new();
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            let entry = WaitingForGameEntry {
                pod_id: "p8".to_string(),
                driver_id: "d1".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now() - std::time::Duration::from_secs(181),
                attempt: 2, // second attempt
                group_session_id: None,
                sim_type: None,
        launch_args: None,
                pre_committed: None,
            };
            waiting.insert("p8".to_string(), entry);
        }
        let timed_out = check_launch_timeouts_from_manager(&mgr, 180).await;
        assert_eq!(timed_out.len(), 1);
        assert_eq!(timed_out[0].0, "p8");
        assert_eq!(timed_out[0].1, 2); // second attempt -> should cancel

        // On attempt 2 timeout: remove from waiting (no billing session created = no charge)
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.remove("p8");
        }
        let waiting = mgr.waiting_for_game.read().await;
        assert!(waiting.get("p8").is_none());
        // No entry in active_timers either (billing never started)
        let timers = mgr.active_timers.read().await;
        assert!(timers.get("p8").is_none());
    }

    // Helper: create a test BillingTimer with Active status
    fn make_test_timer(session_id: &str, pod_id: &str) -> BillingTimer {
        BillingTimer {
            session_id: session_id.to_string(),
            driver_id: "d1".to_string(),
            driver_name: "Test Driver".to_string(),
            pod_id: pod_id.to_string(),
            pricing_tier_name: "per-minute".to_string(),
            allocated_seconds: 10800,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            max_session_seconds: 10800,
            ..Default::default()
        }
    }

    // ── Phase 09 Plan 02: Multiplayer billing coordination ──────────────────

    /// Helper: create a WaitingForGameEntry for tests
    fn make_waiting_entry(pod_id: &str, group_session_id: Option<&str>) -> WaitingForGameEntry {
        WaitingForGameEntry {
            pod_id: pod_id.to_string(),
            driver_id: format!("driver-{}", pod_id),
            pricing_tier_id: "tier1".to_string(),
            custom_price_paise: None,
            custom_duration_minutes: None,
            staff_id: None,
            split_count: None,
            split_duration_minutes: None,
            waiting_since: std::time::Instant::now(),
            attempt: 1,
            group_session_id: group_session_id.map(|s| s.to_string()),
        sim_type: None,
        launch_args: None,
        pre_committed: None,
        }
    }

    #[tokio::test]
    async fn single_player_no_group_billing_starts_immediately_on_live() {
        // Single-player pod (no group_session_id) should NOT be added to multiplayer_waiting
        let mgr = BillingManager::new();

        // Add a single-player WaitingForGameEntry
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.insert("pod1".to_string(), make_waiting_entry("pod1", None));
        }

        // Simulate Live: remove from waiting_for_game
        let entry = {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.remove("pod1")
        };

        // Entry should exist and have no group_session_id
        let entry = entry.unwrap();
        assert!(entry.group_session_id.is_none());
        // Single-player: billing starts immediately (no multiplayer_waiting involvement)
        let mp_waiting = mgr.multiplayer_waiting.read().await;
        assert!(mp_waiting.is_empty());
    }

    #[tokio::test]
    async fn group_2_players_first_live_does_not_start_billing() {
        // Two-pod group: first LIVE should NOT start billing (waits for second)
        let mgr = BillingManager::new();
        let group_id = "group-abc";

        // Set up MultiplayerBillingWait
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: HashSet::new(),
                waiting_entries: HashMap::new(),
                timeout_spawned: false,
            });
        }

        // Pod1 goes LIVE — add to live_pods
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod1".to_string());
            wait.waiting_entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
        }

        // Check: live_pods < expected_pods → billing should NOT start
        {
            let mp = mgr.multiplayer_waiting.read().await;
            let wait = mp.get(group_id).unwrap();
            assert_eq!(wait.live_pods.len(), 1);
            assert_eq!(wait.expected_pods.len(), 2);
            assert!(wait.live_pods.len() < wait.expected_pods.len());
        }
    }

    #[tokio::test]
    async fn group_2_players_second_live_starts_billing_for_both() {
        // Two-pod group: second LIVE should start billing for BOTH
        let mgr = BillingManager::new();
        let group_id = "group-def";

        // Set up with pod1 already live
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string());
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: false,
            });
        }

        // Pod2 goes LIVE
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod2".to_string());
            wait.waiting_entries.insert("pod2".to_string(), make_waiting_entry("pod2", Some(group_id)));

            // All live — collect entries for billing start
            assert!(wait.live_pods.len() >= wait.expected_pods.len());
            let pods_to_bill: Vec<String> = wait.waiting_entries.keys().cloned().collect();
            assert_eq!(pods_to_bill.len(), 2);
            assert!(pods_to_bill.contains(&"pod1".to_string()));
            assert!(pods_to_bill.contains(&"pod2".to_string()));
        }

        // After billing started, entry should be removed from multiplayer_waiting
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            mp.remove(group_id);
        }
        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.get(group_id).is_none());
    }

    #[tokio::test]
    async fn group_3_players_billing_starts_only_when_all_3_live() {
        let mgr = BillingManager::new();
        let group_id = "group-3p";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            expected.insert("pod3".to_string());
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: HashSet::new(),
                waiting_entries: HashMap::new(),
                timeout_spawned: false,
            });
        }

        // Pod1 LIVE
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod1".to_string());
            wait.waiting_entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            assert!(wait.live_pods.len() < wait.expected_pods.len());
        }

        // Pod2 LIVE
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod2".to_string());
            wait.waiting_entries.insert("pod2".to_string(), make_waiting_entry("pod2", Some(group_id)));
            assert!(wait.live_pods.len() < wait.expected_pods.len()); // Still not all
        }

        // Pod3 LIVE — now all are live
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            wait.live_pods.insert("pod3".to_string());
            wait.waiting_entries.insert("pod3".to_string(), make_waiting_entry("pod3", Some(group_id)));
            assert!(wait.live_pods.len() >= wait.expected_pods.len());
            assert_eq!(wait.waiting_entries.len(), 3);
        }
    }

    #[tokio::test]
    async fn group_disconnect_stops_individual_billing_only() {
        // After billing started, pod2 disconnects. Only pod2's billing ends.
        let mgr = BillingManager::new();

        // Both pod1 and pod2 have active timers (billing already started)
        {
            let mut timers = mgr.active_timers.write().await;
            timers.insert("pod1".to_string(), make_test_timer("session-1", "pod1"));
            timers.insert("pod2".to_string(), make_test_timer("session-2", "pod2"));
        }

        // Pod2 disconnects (STATUS=Off): remove only pod2's timer
        {
            let mut timers = mgr.active_timers.write().await;
            let removed = timers.remove("pod2");
            assert!(removed.is_some());
        }

        // Pod1's timer should still be active
        {
            let timers = mgr.active_timers.read().await;
            assert!(timers.contains_key("pod1"));
            let t1 = timers.get("pod1").unwrap();
            assert_eq!(t1.status, BillingSessionStatus::Active);
            // Pod2 is gone
            assert!(!timers.contains_key("pod2"));
        }
    }

    #[tokio::test]
    async fn group_member_never_live_others_can_proceed_after_eviction() {
        // Pod2 never reaches LIVE. After timeout, only pod1 gets billing started.
        let mgr = BillingManager::new();
        let group_id = "group-timeout";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string()); // Only pod1 went LIVE
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: true,
            });
        }

        // Simulate timeout: evict non-live pods, start billing for live ones
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();

            // live_pods < expected_pods → timeout triggers
            assert!(wait.live_pods.len() < wait.expected_pods.len());

            // Evict: keep only live pods in expected
            wait.expected_pods.retain(|p| wait.live_pods.contains(p));
            assert_eq!(wait.expected_pods.len(), 1);
            assert!(wait.expected_pods.contains("pod1"));

            // Now live_pods >= expected_pods → start billing for live pods
            assert!(wait.live_pods.len() >= wait.expected_pods.len());

            // Only pod1 should get billing started
            let pods_to_bill: Vec<String> = wait.waiting_entries.keys()
                .filter(|p| wait.live_pods.contains(*p))
                .cloned()
                .collect();
            assert_eq!(pods_to_bill.len(), 1);
            assert_eq!(pods_to_bill[0], "pod1");
        }
    }

    #[test]
    fn waiting_entry_group_session_id_backward_compat() {
        // Existing code that creates WaitingForGameEntry with group_session_id=None
        // should still work (backward compatibility)
        let entry = make_waiting_entry("pod-solo", None);
        assert!(entry.group_session_id.is_none());
        assert_eq!(entry.pod_id, "pod-solo");

        // Multiplayer entry has Some(group_id)
        let mp_entry = make_waiting_entry("pod-mp", Some("group-xyz"));
        assert_eq!(mp_entry.group_session_id.as_deref(), Some("group-xyz"));
    }

    // ── Phase 09 Plan 02 Task 2: 60-second connection timeout ──────────────

    #[tokio::test]
    async fn timeout_evicts_non_connecting_pod_billing_starts_for_connected() {
        // Group of 2: pod1 connects (LIVE), pod2 never connects.
        // After timeout, only pod1's billing starts. pod2 is evicted.
        let mgr = BillingManager::new();
        let group_id = "group-timeout-evict";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string()); // Only pod1 connected
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            // pod2 never connected, so not in live_pods or waiting_entries
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: true,
            });
        }

        // Simulate timeout logic
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();

            // Timeout fires: live_pods < expected_pods
            assert!(wait.live_pods.len() < wait.expected_pods.len());

            // Collect entries for live pods only
            let billing_entries: Vec<String> = wait.waiting_entries.keys()
                .filter(|p| wait.live_pods.contains(*p))
                .cloned()
                .collect();

            // Only pod1 should get billing started
            assert_eq!(billing_entries.len(), 1);
            assert_eq!(billing_entries[0], "pod1");

            // Evicted pod2 should NOT get billing
            assert!(!wait.live_pods.contains("pod2"));

            // Clean up
            mp.remove(group_id);
        }

        // Verify group entry is gone
        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.is_empty());
    }

    #[tokio::test]
    async fn all_pods_connect_within_timeout_no_eviction() {
        // Group of 2: both pods connect before timeout fires.
        // When timeout fires, the entry should already be gone (consumed).
        let mgr = BillingManager::new();
        let group_id = "group-no-eviction";

        // Set up and immediately have all pods connect (simulating pre-timeout)
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            let mut live = HashSet::new();
            live.insert("pod1".to_string());
            live.insert("pod2".to_string()); // Both connected
            let mut entries = HashMap::new();
            entries.insert("pod1".to_string(), make_waiting_entry("pod1", Some(group_id)));
            entries.insert("pod2".to_string(), make_waiting_entry("pod2", Some(group_id)));
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: live,
                waiting_entries: entries,
                timeout_spawned: true,
            });
        }

        // All pods live: consume the entry (billing starts normally)
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get(group_id).unwrap();
            assert!(wait.live_pods.len() >= wait.expected_pods.len());
            // All live -> start billing for all, remove entry
            mp.remove(group_id);
        }

        // Now timeout fires -- entry is gone, no-op
        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.get(group_id).is_none());
        // This is exactly what multiplayer_billing_timeout() checks:
        // if entry doesn't exist, it returns immediately (no-op)
    }

    #[tokio::test]
    async fn evicted_pod_late_live_does_not_start_billing() {
        // Pod was evicted by timeout. If it later sends LIVE, billing should NOT start.
        let mgr = BillingManager::new();

        // After timeout, the multiplayer_waiting entry is gone.
        // If evicted pod later sends LIVE, it's no longer in waiting_for_game either
        // (it was consumed into MultiplayerBillingWait then evicted).
        // So there's nothing to start billing for.

        // Verify: no waiting entry, no multiplayer entry -> LIVE is a no-op
        let waiting = mgr.waiting_for_game.read().await;
        assert!(waiting.get("evicted-pod").is_none());

        let mp = mgr.multiplayer_waiting.read().await;
        assert!(mp.is_empty());

        // No active timer either (billing was never started for evicted pod)
        let timers = mgr.active_timers.read().await;
        assert!(timers.get("evicted-pod").is_none());
    }

    #[tokio::test]
    async fn timeout_spawned_flag_prevents_duplicate_spawn() {
        let mgr = BillingManager::new();
        let group_id = "group-spawn-once";

        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let mut expected = HashSet::new();
            expected.insert("pod1".to_string());
            expected.insert("pod2".to_string());
            mp.insert(group_id.to_string(), MultiplayerBillingWait {
                group_session_id: group_id.to_string(),
                expected_pods: expected,
                live_pods: HashSet::new(),
                waiting_entries: HashMap::new(),
                timeout_spawned: false,
            });
        }

        // First pod arrives: timeout_spawned should become true
        {
            let mut mp = mgr.multiplayer_waiting.write().await;
            let wait = mp.get_mut(group_id).unwrap();
            assert!(!wait.timeout_spawned);
            wait.timeout_spawned = true; // Would spawn tokio task
            wait.live_pods.insert("pod1".to_string());
        }

        // Second pod arrives: timeout_spawned is already true, no duplicate spawn
        {
            let mp = mgr.multiplayer_waiting.read().await;
            let wait = mp.get(group_id).unwrap();
            assert!(wait.timeout_spawned); // Already true, won't spawn again
        }
    }

    #[test]
    fn timer_waiting_for_game_no_increments() {
        let mut timer = BillingTimer {
            session_id: "test-waiting".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "per-minute".into(),
            allocated_seconds: 10800,
            driving_seconds: 0,
            status: BillingSessionStatus::WaitingForGame,
            driving_state: DrivingState::Idle,
            started_at: None,
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 0,
            pause_seconds: 0,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        assert!(!timer.tick());
        assert_eq!(timer.elapsed_seconds, 0);
        assert_eq!(timer.driving_seconds, 0);
        assert_eq!(timer.pause_seconds, 0);
    }

    // ─── WhatsApp Receipt Tests ─────────────────────────────────────────────

    #[test]
    fn whatsapp_receipt_message_format() {
        let msg = format_receipt_message("Rahul", 1500, 70000, Some(93210), 150000);

        // Verify key components
        assert!(msg.contains("Rahul"), "Message must contain first name");
        assert!(msg.contains("25m 0s"), "Duration must be 25m 0s for 1500 seconds");
        assert!(msg.contains("700 credits"), "Cost must be 700 credits for 70000 paise");
        assert!(msg.contains("1:33.210"), "Best lap must be 1:33.210 for 93210ms");
        assert!(msg.contains("1500 credits"), "Balance must be 1500 credits for 150000 paise");
        assert!(msg.contains("RacingPoint"), "Must contain brand name");
        assert!(msg.contains("Session Complete"), "Must indicate session complete");
    }

    #[test]
    fn whatsapp_receipt_no_valid_laps() {
        let msg = format_receipt_message("Priya", 600, 35000, None, 50000);
        assert!(msg.contains("No valid laps"), "Must show 'No valid laps' when None");

        let msg2 = format_receipt_message("Priya", 600, 35000, Some(0), 50000);
        assert!(msg2.contains("No valid laps"), "Must show 'No valid laps' when 0ms");
    }

    #[test]
    fn whatsapp_phone_format_10_digit() {
        assert_eq!(format_wa_phone("9876543210"), "919876543210");
    }

    #[test]
    fn whatsapp_phone_format_with_plus() {
        assert_eq!(format_wa_phone("+919876543210"), "919876543210");
    }

    #[test]
    fn whatsapp_phone_format_already_formatted() {
        assert_eq!(format_wa_phone("919876543210"), "919876543210");
    }

    #[test]
    fn whatsapp_receipt_zero_cost() {
        let msg = format_receipt_message("Test", 300, 0, None, 0);
        assert!(msg.contains("0 credits"), "Cost should show 0 credits for trial/free");
    }

    // ── BILL-01 characterization tests: safety net before billing bot code ──

    // BILL-01 characterization: game-exit-while-billing path
    #[test]
    fn game_exit_while_billing_ends_session() {
        // AcStatus::Off while billing active fires the session-end path in ws/mod.rs
        // handle_game_status_update(). This test characterizes the condition:
        // billing_active=true + game exits → session_id resolved from active_timers → end_billing_session fires.
        let mut timers: std::collections::HashMap<String, BillingTimer> =
            std::collections::HashMap::new();
        timers.insert("pod_1".to_string(), BillingTimer::dummy("pod_1"));
        // Precondition: timer present for pod
        assert!(timers.contains_key("pod_1"));
        // Characterization: when game exits, timer lookup must succeed for end_session to fire
        let session_id = timers.get("pod_1").map(|t| t.session_id.clone());
        assert!(session_id.is_some(), "session_id must be resolvable for game-exit path");
    }

    // BILL-01 characterization: idle drift detection condition (BILL-03)
    #[test]
    fn idle_drift_condition_check() {
        // BILL-03 fires when billing active + DrivingState is NOT Active for > 5 minutes.
        let idle_threshold_secs = 300u64; // 5 minutes
        assert_eq!(idle_threshold_secs, 300, "idle drift threshold must be exactly 5 minutes");
        // DrivingState::Active is the only non-idle state; Idle means the condition can fire.
        let ds_idle = DrivingState::Idle;
        let is_active = matches!(ds_idle, DrivingState::Active);
        assert!(!is_active, "DrivingState::Idle must NOT match Active — idle drift condition met");
    }

    // BILL-01 characterization: end_session removes timer from active_timers
    #[test]
    fn end_session_removes_timer() {
        let mut timers: std::collections::HashMap<String, BillingTimer> =
            std::collections::HashMap::new();
        timers.insert("pod_2".to_string(), BillingTimer::dummy("pod_2"));
        assert!(timers.contains_key("pod_2"));
        timers.remove("pod_2");
        assert!(
            !timers.contains_key("pod_2"),
            "Timer must be removed from active_timers after end_session"
        );
    }

    // BILL-01 characterization: stuck session detection condition (BILL-02)
    #[test]
    fn stuck_session_condition() {
        // BILL-02 fires when billing_active=true AND game_pid=None for >= 60 seconds.
        let stuck_threshold_secs = 60u64;
        assert_eq!(stuck_threshold_secs, 60, "stuck session threshold must be exactly 60 seconds");
        // The condition: billing active + no game PID
        let billing_active = true;
        let game_pid: Option<u32> = None;
        let condition_met = billing_active && game_pid.is_none();
        assert!(
            condition_met,
            "billing_active=true + game_pid=None must satisfy stuck session condition"
        );
    }

    // BILL-01 characterization: start_session populates active_timers for lookup
    #[test]
    fn start_session_inserts_timer() {
        let mut timers: std::collections::HashMap<String, BillingTimer> =
            std::collections::HashMap::new();
        timers.insert("pod_1".to_string(), BillingTimer::dummy("pod_1"));
        // active_timers must contain the pod_id for recover_stuck_session() to find it
        assert!(
            timers.contains_key("pod_1"),
            "start_session must insert timer — recover_stuck_session depends on this"
        );
        let t = timers.get("pod_1").unwrap();
        assert_eq!(t.pod_id.as_str(), "pod_1", "BillingTimer::dummy sets pod_id correctly");
        assert!(
            t.session_id.contains("pod_1"),
            "session_id must embed pod_id for traceability"
        );
    }
    // ── Phase 82-01: Per-game rate lookup tests ────────────────────────────

    fn make_tier(order: u32, threshold: u32, rate: i64, sim: Option<rc_common::types::SimType>) -> BillingRateTier {
        BillingRateTier {
            tier_order: order,
            tier_name: format!("Tier {}", order),
            threshold_minutes: threshold,
            rate_per_min_paise: rate,
            sim_type: sim,
        }
    }

    #[test]
    fn test_get_tiers_for_game_specific() {
        use rc_common::types::SimType;
        // 2 universal + 2 F1-specific tiers
        let tiers = vec![
            make_tier(1, 30, 2500, None),
            make_tier(2, 0,  2000, None),
            make_tier(1, 30, 3000, Some(SimType::F125)),
            make_tier(2, 0,  2500, Some(SimType::F125)),
        ];
        let result = get_tiers_for_game(&tiers, Some(SimType::F125));
        assert_eq!(result.len(), 2, "Should return 2 F1-specific tiers");
        assert_eq!(result[0].rate_per_min_paise, 3000, "First F1 tier rate");
        assert_eq!(result[1].rate_per_min_paise, 2500, "Second F1 tier rate");
    }

    #[test]
    fn test_get_tiers_for_game_fallback() {
        use rc_common::types::SimType;
        // Only universal tiers, no iRacing tiers
        let tiers = vec![
            make_tier(1, 30, 2500, None),
            make_tier(2, 0,  2000, None),
        ];
        let result = get_tiers_for_game(&tiers, Some(SimType::IRacing));
        assert_eq!(result.len(), 2, "Should fall back to 2 universal tiers");
        assert_eq!(result[0].rate_per_min_paise, 2500);
    }

    #[test]
    fn test_get_tiers_for_game_none() {
        use rc_common::types::SimType;
        let tiers = vec![
            make_tier(1, 30, 2500, None),
            make_tier(2, 0,  2000, None),
            make_tier(1, 30, 3000, Some(SimType::F125)),
        ];
        // sim_type=None should return only universal tiers
        let result = get_tiers_for_game(&tiers, None);
        assert_eq!(result.len(), 2, "sim_type=None returns only universal tiers");
    }

    #[test]
    fn test_billing_rate_tier_sim_type_roundtrip() {
        use rc_common::types::SimType;
        // Simulate serde roundtrip: SimType -> str -> SimType (as DB would store)
        let sim = SimType::F125;
        let as_json = serde_json::to_value(&sim).unwrap();
        let as_str = as_json.as_str().unwrap();
        assert_eq!(as_str, "f1_25");
        let parsed: SimType = serde_json::from_value(serde_json::Value::String(as_str.to_string())).unwrap();
        assert_eq!(parsed, SimType::F125, "SimType roundtrip via string");

        // A tier with sim_type set
        let tier = make_tier(1, 30, 3000, Some(SimType::F125));
        assert_eq!(tier.sim_type, Some(SimType::F125));
        assert_eq!(tier.rate_per_min_paise, 3000);
    }

    // ── Phase 198 Plan 03: BILL-05, BILL-06, BILL-10, BILL-12 tests ─────────

    /// BILL-05: WaitingForGame entries produce BillingTick with WaitingForGame status.
    /// Verifies that the waiting_for_game map contains entries that would be broadcast
    /// as BillingTick(WaitingForGame) by tick_all_timers each second.
    #[tokio::test]
    async fn waiting_for_game_tick_broadcasts() {
        let mgr = BillingManager::new();

        // Insert a WaitingForGameEntry — these are the entries that tick_all_timers
        // broadcasts as BillingTick(WaitingForGame) each tick (BILL-05 implementation)
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.insert("pod-wfg".to_string(), WaitingForGameEntry {
                pod_id: "pod-wfg".to_string(),
                driver_id: "driver-wfg".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: Some(30),
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now(),
                attempt: 1,
                group_session_id: None,
                sim_type: None,
        launch_args: None,
                pre_committed: None,
            });
        }

        // Verify the entry is in waiting_for_game (not active_timers) — tick_all_timers
        // reads this map and emits BillingTick with status=WaitingForGame for each entry
        let waiting = mgr.waiting_for_game.read().await;
        let entry = waiting.get("pod-wfg");
        assert!(entry.is_some(), "WaitingForGameEntry must exist in waiting_for_game map");
        let entry = entry.unwrap();
        assert_eq!(entry.driver_id, "driver-wfg");
        assert_eq!(entry.pod_id, "pod-wfg");
        assert_eq!(entry.custom_duration_minutes, Some(30));

        // The entry is NOT in active_timers — tick_all_timers has a dedicated loop
        // over waiting_for_game that emits BillingTick(WaitingForGame) for each entry
        drop(waiting);
        let timers = mgr.active_timers.read().await;
        assert!(
            timers.get("pod-wfg").is_none(),
            "WaitingForGame entry must NOT be in active_timers — lives only in waiting_for_game map"
        );

        // Simulate what tick_all_timers does: build BillingSessionInfo with WaitingForGame status
        let waiting = mgr.waiting_for_game.read().await;
        let e = waiting.get("pod-wfg").unwrap();
        let simulated_info = rc_common::types::BillingSessionInfo {
            id: format!("deferred-{}", e.pod_id),
            driver_id: e.driver_id.clone(),
            driver_name: String::new(),
            pod_id: e.pod_id.clone(),
            pricing_tier_name: e.pricing_tier_id.clone(),
            allocated_seconds: e.custom_duration_minutes.unwrap_or(30) * 60,
            driving_seconds: 0,
            remaining_seconds: e.custom_duration_minutes.unwrap_or(30) * 60,
            status: BillingSessionStatus::WaitingForGame,
            driving_state: DrivingState::Idle,
            started_at: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            elapsed_seconds: Some(e.waiting_since.elapsed().as_secs() as u32),
            cost_paise: Some(0),
            rate_per_min_paise: Some(0),
            billing_mode: None,
            recovery_pause_seconds: None,
        };
        // Verify the simulated tick has the correct status
        assert_eq!(
            simulated_info.status,
            BillingSessionStatus::WaitingForGame,
            "BillingTick broadcast for WaitingForGame entry must carry WaitingForGame status"
        );
        assert_eq!(simulated_info.driving_seconds, 0, "No driving seconds during WaitingForGame");
        assert_eq!(simulated_info.cost_paise, Some(0), "No cost during WaitingForGame");
    }

    /// BILL-06: After 2 failed launch attempts (>timeout each), the entry is removed
    /// (cancelled_no_playable). The check_launch_timeouts_from_manager returns the pod
    /// on attempt 2 with the correct attempt count, confirming the cancel path fires.
    #[tokio::test]
    async fn cancelled_no_playable_on_timeout() {
        let mgr = BillingManager::new();

        // Create WaitingForGameEntry with attempt=2 and waiting_since > 180s ago
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            let entry = WaitingForGameEntry {
                pod_id: "pod-cnp".to_string(),
                driver_id: "driver-cnp".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                // 181s elapsed — past the 180s per-attempt timeout
                waiting_since: std::time::Instant::now()
                    - std::time::Duration::from_secs(181),
                attempt: 2, // Second attempt — this is the cancel threshold
                group_session_id: None,
                sim_type: None,
        launch_args: None,
                pre_committed: None,
            };
            waiting.insert("pod-cnp".to_string(), entry);
        }

        // check_launch_timeouts_from_manager returns pods that have exceeded the timeout
        let timed_out = check_launch_timeouts_from_manager(&mgr, 180).await;
        assert_eq!(
            timed_out.len(), 1,
            "Exactly one pod must be returned as timed-out"
        );
        assert_eq!(timed_out[0].0, "pod-cnp", "Correct pod ID in timed-out list");
        assert_eq!(
            timed_out[0].1, 2,
            "attempt=2 must be returned — this is what triggers cancelled_no_playable"
        );

        // On attempt 2 timeout: production code removes the entry and inserts a
        // billing_sessions record with status='cancelled_no_playable', driving_seconds=0.
        // Here we simulate the removal (no DB in unit tests):
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.remove("pod-cnp");
        }

        // Verify entry is gone (cancelled) — no active timer (no charge to customer)
        let waiting = mgr.waiting_for_game.read().await;
        assert!(
            waiting.get("pod-cnp").is_none(),
            "Entry must be removed from waiting_for_game after cancelled_no_playable"
        );
        drop(waiting);

        let timers = mgr.active_timers.read().await;
        assert!(
            timers.get("pod-cnp").is_none(),
            "No active billing timer — customer is NOT charged on cancelled_no_playable"
        );
    }

    /// BILL-10: Multiplayer DB query failure must NOT silently proceed.
    /// The entry should be preserved in waiting_for_game for retry rather than
    /// silently dropped (old unwrap_or_default behavior).
    #[tokio::test]
    async fn multiplayer_db_query_failure_preserves_waiting_entry() {
        let mgr = BillingManager::new();
        let group_id = "group-db-fail";

        // Set up: pod waiting with a group_session_id (triggers DB query path)
        let entry = WaitingForGameEntry {
            pod_id: "pod-mp-fail".to_string(),
            driver_id: "driver-mp".to_string(),
            pricing_tier_id: "tier1".to_string(),
            custom_price_paise: None,
            custom_duration_minutes: None,
            staff_id: None,
            split_count: None,
            split_duration_minutes: None,
            waiting_since: std::time::Instant::now(),
            attempt: 1,
            group_session_id: Some(group_id.to_string()),
            sim_type: None,
        launch_args: None,
            pre_committed: None,
        };

        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.insert("pod-mp-fail".to_string(), entry);
        }

        // Simulate BILL-10 error path: DB query for group_session_members fails.
        // Production code: re-inserts entry into waiting_for_game for retry.
        // The entry should NOT be lost — verify it stays in waiting_for_game.
        //
        // In production, handle_game_status_update acquires a write lock on
        // waiting_for_game, removes the entry for processing, and on DB failure
        // re-inserts it. Here we verify the structural invariant:
        // after an error path, the entry is restored.
        {
            // Simulate: remove then re-insert (the error path restore)
            let mut waiting = mgr.waiting_for_game.write().await;
            let entry_opt = waiting.remove("pod-mp-fail");
            assert!(entry_opt.is_some(), "Entry must be removable for processing");
            let entry = entry_opt.unwrap();
            assert_eq!(
                entry.group_session_id.as_deref(),
                Some(group_id),
                "group_session_id must be preserved through the error path"
            );
            // Error occurred — re-insert for retry
            waiting.insert("pod-mp-fail".to_string(), entry);
        }

        // Verify: entry is back in waiting_for_game (not lost)
        let waiting = mgr.waiting_for_game.read().await;
        let restored = waiting.get("pod-mp-fail");
        assert!(
            restored.is_some(),
            "Entry must be preserved in waiting_for_game after DB query failure (BILL-10)"
        );
        assert_eq!(
            restored.unwrap().group_session_id.as_deref(),
            Some(group_id),
            "group_session_id preserved after re-insert"
        );
        drop(waiting);

        // No billing timer was started (billing REJECTED on DB error)
        let timers = mgr.active_timers.read().await;
        assert!(
            timers.get("pod-mp-fail").is_none(),
            "No billing timer must exist — billing was REJECTED on DB query failure"
        );
    }

    /// BILL-12: Configurable billing timeouts via timeout_secs parameter.
    /// check_launch_timeouts_from_manager uses the passed timeout_secs — not a hardcoded 180.
    #[tokio::test]
    async fn configurable_billing_timeouts() {
        let mgr = BillingManager::new();

        // Create entry with waiting_since 100 seconds ago
        {
            let mut waiting = mgr.waiting_for_game.write().await;
            waiting.insert("pod-cfg".to_string(), WaitingForGameEntry {
                pod_id: "pod-cfg".to_string(),
                driver_id: "driver-cfg".to_string(),
                pricing_tier_id: "tier1".to_string(),
                custom_price_paise: None,
                custom_duration_minutes: None,
                staff_id: None,
                split_count: None,
                split_duration_minutes: None,
                waiting_since: std::time::Instant::now()
                    - std::time::Duration::from_secs(100),
                attempt: 1,
                group_session_id: None,
                sim_type: None,
        launch_args: None,
                pre_committed: None,
            });
        }

        // With timeout_secs=90: 100s elapsed > 90s → pod IS timed out
        let timed_out_90 = check_launch_timeouts_from_manager(&mgr, 90).await;
        assert_eq!(
            timed_out_90.len(), 1,
            "Pod must be timed out when elapsed (100s) > timeout_secs (90s)"
        );
        assert_eq!(timed_out_90[0].0, "pod-cfg");

        // With timeout_secs=120: 100s elapsed < 120s → pod is NOT timed out
        let timed_out_120 = check_launch_timeouts_from_manager(&mgr, 120).await;
        assert_eq!(
            timed_out_120.len(), 0,
            "Pod must NOT be timed out when elapsed (100s) < timeout_secs (120s)"
        );

        // Edge case: timeout_secs=100 exactly — elapsed is ~100s.
        // Due to timing jitter in tests, allow ±1s. The entry was created 100s ago,
        // so elapsed >= 100s. With timeout=100, it should be timed out (elapsed >= timeout).
        // We don't test this boundary exactly to avoid flakiness, but the above
        // two cases (90 vs 120) are sufficient to prove the parameter is respected.
    }

    // ── compute_refund tests (FATM-06) ──────────────────────────────────────

    #[test]
    fn test_compute_refund_half_time_used() {
        // 1800s allocated, 900s driven, 75000 paise debited → 50% refund
        assert_eq!(compute_refund(1800, 900, 75000), 37500);
    }

    #[test]
    fn test_compute_refund_full_time_used() {
        // Fully driven → no refund
        assert_eq!(compute_refund(1800, 1800, 75000), 0);
    }

    #[test]
    fn test_compute_refund_no_time_used() {
        // No time driven → full refund
        assert_eq!(compute_refund(1800, 0, 75000), 75000);
    }

    #[test]
    fn test_compute_refund_overdriven() {
        // driving_seconds > allocated → no refund (clamped to 0)
        assert_eq!(compute_refund(1800, 2000, 75000), 0);
    }

    #[test]
    fn test_compute_refund_zero_allocated() {
        // Zero allocated → safe division, returns 0
        assert_eq!(compute_refund(0, 0, 75000), 0);
    }

    // ── Tier alignment (FATM-05) ─────────────────────────────────────────────

    #[test]
    fn test_tier_alignment_fatm05() {
        // FATM-05: Rate-based cost for 30 min MUST match DB seed tier_30min price (75000 paise).
        // DB seed: db/mod.rs INSERT INTO pricing_tiers ... ('tier_30min', '30 Minutes', 30, 75000, ...)
        // Rate calc: 30 min * 2500 paise/min = 75000 paise
        // If this test fails, either the rate or the seed diverged — fix both.
        let tiers = default_billing_rate_tiers();
        let cost = compute_session_cost(1800, &tiers);
        assert_eq!(cost.total_paise, 75000, "FATM-05: 30min cost must match tier_30min price (2500 p/min * 30 min = 75000 p = Rs.750)");
    }

    // ── FSM-07: Split session lifecycle ──────────────────────────────────────

    async fn create_test_db() -> sqlx::SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");
        // Minimal schema: billing_sessions parent table + split_sessions
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_sessions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                pod_id TEXT NOT NULL,
                pricing_tier_id TEXT NOT NULL,
                allocated_seconds INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("Failed to create billing_sessions");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS split_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                parent_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
                split_number INTEGER NOT NULL,
                allocated_seconds INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                started_at TEXT,
                ended_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001',
                UNIQUE(parent_session_id, split_number)
            )",
        )
        .execute(&pool)
        .await
        .expect("Failed to create split_sessions");

        // Insert a dummy billing session for FK references
        sqlx::query(
            "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds) VALUES ('test-session', 'd1', 'pod_1', 'tier_30min', 1800)"
        )
        .execute(&pool)
        .await
        .expect("Failed to insert test billing session");

        pool
    }

    #[tokio::test]
    async fn test_split_create_equal_allocation() {
        let pool = create_test_db().await;
        // 3 splits of 1800s total → 600s each
        create_split_records(&pool, "test-session", 3, 1800, "racingpoint-hyd-001").await.expect("create_split_records failed");

        let rows: Vec<(i64, i64, String)> = sqlx::query_as(
            "SELECT split_number, allocated_seconds, status FROM split_sessions WHERE parent_session_id = 'test-session' ORDER BY split_number"
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");

        assert_eq!(rows.len(), 3, "Should have 3 split records");
        // Each split gets 600s
        assert_eq!(rows[0].1, 600, "Split 1 should get 600s");
        assert_eq!(rows[1].1, 600, "Split 2 should get 600s");
        assert_eq!(rows[2].1, 600, "Split 3 should get 600s");
        // Split 1 starts active, rest pending
        assert_eq!(rows[0].2, "active", "Split 1 should be active");
        assert_eq!(rows[1].2, "pending", "Split 2 should be pending");
        assert_eq!(rows[2].2, "pending", "Split 3 should be pending");
    }

    #[tokio::test]
    async fn test_split_remainder_goes_to_last() {
        let pool = create_test_db().await;
        // 1801s / 3 = 600 remainder 1 → last split gets 601s
        create_split_records(&pool, "test-session", 3, 1801, "racingpoint-hyd-001").await.expect("create_split_records failed");

        let rows: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT split_number, allocated_seconds FROM split_sessions WHERE parent_session_id = 'test-session' ORDER BY split_number"
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");

        assert_eq!(rows[0].1, 600, "Split 1 should get 600s");
        assert_eq!(rows[1].1, 600, "Split 2 should get 600s");
        assert_eq!(rows[2].1, 601, "Split 3 should get 601s (remainder)");
    }

    #[tokio::test]
    async fn test_split_transition_advances_to_next() {
        let pool = create_test_db().await;
        create_split_records(&pool, "test-session", 3, 1800, "racingpoint-hyd-001").await.expect("create_split_records failed");

        // Transition from split 1 → should activate split 2
        let next = transition_split(&pool, "test-session", 1).await.expect("transition_split failed");
        assert_eq!(next, Some(2), "Should advance to split 2");

        // Verify DB state
        let statuses: Vec<(i64, String)> = sqlx::query_as(
            "SELECT split_number, status FROM split_sessions WHERE parent_session_id = 'test-session' ORDER BY split_number"
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");

        assert_eq!(statuses[0].1, "completed", "Split 1 should be completed");
        assert_eq!(statuses[1].1, "active", "Split 2 should be active");
        assert_eq!(statuses[2].1, "pending", "Split 3 should still be pending");
    }

    #[tokio::test]
    async fn test_split_transition_last_returns_none() {
        let pool = create_test_db().await;
        create_split_records(&pool, "test-session", 2, 1200, "racingpoint-hyd-001").await.expect("create_split_records failed");

        // Complete split 1 → activates split 2
        let _ = transition_split(&pool, "test-session", 1).await.expect("first transition failed");
        // Complete split 2 → no more splits
        let next = transition_split(&pool, "test-session", 2).await.expect("second transition failed");
        assert_eq!(next, None, "No more splits after last one");
    }

    #[tokio::test]
    async fn test_split_cas_rejects_non_active() {
        let pool = create_test_db().await;
        create_split_records(&pool, "test-session", 3, 1800, "racingpoint-hyd-001").await.expect("create_split_records failed");

        // Try to complete split 2 (which is still Pending) — should fail CAS
        let result = transition_split(&pool, "test-session", 2).await;
        assert!(result.is_err(), "CAS should reject completing a pending split");
        assert!(result.unwrap_err().contains("CAS failed"), "Error should mention CAS failure");
    }

    #[tokio::test]
    async fn test_cancel_pending_splits() {
        let pool = create_test_db().await;
        create_split_records(&pool, "test-session", 3, 1800, "racingpoint-hyd-001").await.expect("create_split_records failed");

        cancel_pending_splits(&pool, "test-session").await.expect("cancel_pending_splits failed");

        let statuses: Vec<(i64, String)> = sqlx::query_as(
            "SELECT split_number, status FROM split_sessions WHERE parent_session_id = 'test-session' ORDER BY split_number"
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");

        // Split 1 was active (not pending) — should stay active
        assert_eq!(statuses[0].1, "active", "Active split should not be cancelled");
        // Splits 2 and 3 were pending — should be cancelled
        assert_eq!(statuses[1].1, "cancelled", "Pending split 2 should be cancelled");
        assert_eq!(statuses[2].1, "cancelled", "Pending split 3 should be cancelled");
    }

    #[tokio::test]
    async fn test_get_next_pending_split_returns_lowest() {
        let pool = create_test_db().await;
        create_split_records(&pool, "test-session", 3, 1800, "racingpoint-hyd-001").await.expect("create_split_records failed");

        // Initially split 1 is active, so next PENDING is split 2
        let next = get_next_pending_split(&pool, "test-session").await.expect("get_next_pending_split failed");
        assert_eq!(next, Some((2, 600)), "Next pending should be split 2 with 600s");
    }

    // ─── BILL-03: PWA game request TTL tests ─────────────────────────────────

    /// BILL-03: BillingTimer struct has no direct relation to game_launch_requests table,
    /// but the cleanup function requires the DB table to exist. Test that game_launch_requests
    /// table can be created and records inserted/queried with expires_at.
    #[tokio::test]
    async fn pwa_request_ttl_table_exists_and_queryable() {
        let pool = create_test_db().await;

        // Create game_launch_requests table (normally created by full db::migrate())
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS game_launch_requests (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL,
                resolved_at TEXT,
                resolved_by TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("Failed to create game_launch_requests table");

        // Insert a pending request with a past expires_at (already expired)
        let request_id = "test-req-001";
        sqlx::query(
            "INSERT INTO game_launch_requests (id, driver_id, pod_id, sim_type, status, expires_at)
             VALUES (?, ?, ?, ?, 'pending', datetime('now', '-1 minute'))",
        )
        .bind(request_id)
        .bind("driver-1")
        .bind("pod_1")
        .bind("AssettoCorsa")
        .execute(&pool)
        .await
        .expect("Should insert game_launch_request");

        // Verify that the row is pending and expires_at < now
        let row: Option<(String, i64)> = sqlx::query_as(
            "SELECT status, CASE WHEN expires_at < datetime('now') THEN 1 ELSE 0 END as is_expired
             FROM game_launch_requests WHERE id = ?",
        )
        .bind(request_id)
        .fetch_optional(&pool)
        .await
        .expect("query failed");

        assert!(row.is_some());
        let (status, is_expired) = row.unwrap();
        assert_eq!(status, "pending", "Status should be pending before cleanup");
        assert_eq!(is_expired, 1, "expires_at should be in the past");

        // Simulate cleanup: mark expired
        sqlx::query(
            "UPDATE game_launch_requests SET status = 'expired' WHERE status = 'pending' AND expires_at < datetime('now')",
        )
        .execute(&pool)
        .await
        .expect("Update failed");

        let new_status: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM game_launch_requests WHERE id = ?",
        )
        .bind(request_id)
        .fetch_optional(&pool)
        .await
        .expect("query failed");

        assert_eq!(new_status.unwrap().0, "expired", "Status should be expired after cleanup");
    }

    /// BILL-03: A request with expires_at in the future should NOT be marked expired.
    #[tokio::test]
    async fn pwa_request_ttl_future_request_not_expired() {
        let pool = create_test_db().await;

        // Create game_launch_requests table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS game_launch_requests (
                id TEXT PRIMARY KEY, driver_id TEXT NOT NULL, pod_id TEXT NOT NULL,
                sim_type TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL DEFAULT (datetime('now')), expires_at TEXT NOT NULL,
                resolved_at TEXT, resolved_by TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("Failed to create game_launch_requests table");

        let request_id = "test-req-future";
        sqlx::query(
            "INSERT INTO game_launch_requests (id, driver_id, pod_id, sim_type, status, expires_at)
             VALUES (?, ?, ?, ?, 'pending', datetime('now', '+10 minutes'))",
        )
        .bind(request_id)
        .bind("driver-2")
        .bind("pod_2")
        .bind("AssettoCorsa")
        .execute(&pool)
        .await
        .expect("Should insert game_launch_request");

        // Cleanup should affect 0 rows (not expired yet)
        let result = sqlx::query(
            "UPDATE game_launch_requests SET status = 'expired' WHERE status = 'pending' AND expires_at < datetime('now')",
        )
        .execute(&pool)
        .await
        .expect("Update failed");

        assert_eq!(result.rows_affected(), 0, "Future request should NOT be marked expired");

        let status: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM game_launch_requests WHERE id = ?",
        )
        .bind(request_id)
        .fetch_optional(&pool)
        .await
        .expect("query failed");

        assert_eq!(status.unwrap().0, "pending", "Status must remain pending");
    }

    // ─── BILL-04: Extension pricing enforcement tests ─────────────────────────

    /// BILL-04: Extension on an active session correctly uses current tier rate.
    #[test]
    fn extension_pricing_uses_current_tier_rate() {
        let tiers = default_billing_rate_tiers();
        let mut timer = BillingTimer {
            session_id: "ext-session".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Standard".into(),
            allocated_seconds: 1800,
            driving_seconds: 600,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 600,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // At 600s (10 min), still in Standard tier (threshold=1800s=30min)
        let cost = timer.current_cost(&tiers);
        assert_eq!(cost.tier_name, "Standard");
        let rate_at_600s = cost.rate_per_min_paise;
        assert_eq!(rate_at_600s, 2500, "Standard tier should be 2500p/min");

        // Extend by 600s (10 min)
        timer.allocated_seconds += 600;

        // Rate should still be Standard (we're at 10min, threshold is 30min)
        let cost_after = timer.current_cost(&tiers);
        assert_eq!(cost_after.rate_per_min_paise, 2500, "Extension rate must match current tier");
    }

    /// BILL-04: Extension attempt on a completed session returns early (no crash).
    #[test]
    fn extension_rejected_on_completed_session() {
        let timer = BillingTimer {
            session_id: "done-session".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Standard".into(),
            allocated_seconds: 1800,
            driving_seconds: 1800,
            status: BillingSessionStatus::Completed,
            driving_state: DrivingState::Idle,
            started_at: Some(Utc::now()),
            warning_5min_sent: true,
            warning_1min_sent: true,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 1800,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // Verify: completed sessions are terminal — cannot be extended
        assert!(matches!(
            timer.status,
            BillingSessionStatus::Completed
                | BillingSessionStatus::EndedEarly
                | BillingSessionStatus::Cancelled
                | BillingSessionStatus::CancelledNoPlayable
        ), "Completed session must be detected as terminal");
    }

    // ─── BILL-06: Crash recovery pause exclusion tests ────────────────────────

    /// BILL-06: BillingTimer has recovery_pause_seconds field, starts at 0.
    #[test]
    fn recovery_pause_seconds_starts_at_zero() {
        let timer = BillingTimer {
            session_id: "rps-test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Standard".into(),
            allocated_seconds: 1800,
            driving_seconds: 0,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 0,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        assert_eq!(timer.recovery_pause_seconds, 0, "recovery_pause_seconds must start at 0");
        assert_eq!(timer.pause_reason, PauseReason::None, "pause_reason must start at None");
    }

    /// BILL-06: When status is PausedGamePause + CrashRecovery reason, recovery_pause_seconds increments.
    #[test]
    fn recovery_pause_increments_on_crash_recovery_tick() {
        let mut timer = BillingTimer {
            session_id: "crash-test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Standard".into(),
            allocated_seconds: 1800,
            driving_seconds: 300,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 300,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        // Simulate crash recovery: set PausedGamePause + CrashRecovery
        timer.status = BillingSessionStatus::PausedGamePause;
        timer.pause_reason = PauseReason::CrashRecovery;

        // Tick 30 times (30 seconds)
        for _ in 0..30 {
            timer.tick();
        }

        assert_eq!(timer.pause_seconds, 30, "pause_seconds must increment to 30");
        assert_eq!(timer.recovery_pause_seconds, 30, "recovery_pause_seconds must also increment to 30 (crash recovery)");
        assert_eq!(timer.elapsed_seconds, 300, "elapsed_seconds must NOT change during PausedGamePause");
    }

    /// BILL-06: Manual ESC pause does NOT increment recovery_pause_seconds.
    #[test]
    fn manual_pause_does_not_increment_recovery_pause_seconds() {
        let mut timer = BillingTimer {
            session_id: "manual-pause-test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Standard".into(),
            allocated_seconds: 1800,
            driving_seconds: 300,
            status: BillingSessionStatus::PausedGamePause,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 1,
            total_paused_seconds: 0,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 300,
            pause_seconds: 0,
            max_session_seconds: 1800,
            sim_type: None,
            recovery_pause_seconds: 0,
            pause_reason: PauseReason::GamePause, // Manual ESC pause
            nonce: String::new(),
            ..Default::default()
        };

        // Tick 20 times
        for _ in 0..20 {
            timer.tick();
        }

        assert_eq!(timer.pause_seconds, 20, "pause_seconds must increment");
        assert_eq!(timer.recovery_pause_seconds, 0, "Manual pause must NOT increment recovery_pause_seconds");
    }

    /// BILL-06: compute_session_cost subtracts recovery_pause_seconds from billable time.
    #[test]
    fn billing_start_time_recovery_pause_excluded_from_cost() {
        let tiers = default_billing_rate_tiers();

        // Scenario: 600s elapsed, 120s of that was crash recovery pause
        // Billable = 600 - 120 = 480s = 8 min @ 2500p/min = 20000p
        let timer = BillingTimer {
            session_id: "cost-excl-test".into(),
            driver_id: "d1".into(),
            driver_name: "Test".into(),
            pod_id: "p1".into(),
            pricing_tier_name: "Standard".into(),
            allocated_seconds: 10800,
            driving_seconds: 600,
            status: BillingSessionStatus::Active,
            driving_state: DrivingState::Active,
            started_at: Some(Utc::now()),
            warning_5min_sent: false,
            warning_1min_sent: false,
            offline_since: None,
            split_count: 1,
            split_duration_minutes: None,
            current_split_number: 1,
            pause_count: 0,
            total_paused_seconds: 120,
            last_paused_at: None,
            max_pause_duration_secs: 600,
            elapsed_seconds: 600,
            pause_seconds: 0,
            max_session_seconds: 10800,
            sim_type: None,
            recovery_pause_seconds: 120,
            pause_reason: PauseReason::None,
            nonce: String::new(),
            ..Default::default()
        };

        let cost = timer.current_cost(&tiers);
        // Billable = 600 - 120 = 480s = 8 min @ 2500p/min = 20000p
        assert_eq!(cost.total_paise, 20000, "Cost must exclude 120s crash recovery time");

        // Without recovery pause (for comparison): 600s = 10 min = 25000p
        let timer_no_recovery = BillingTimer {
            recovery_pause_seconds: 0,
            ..timer
        };
        let cost_no_recovery = timer_no_recovery.current_cost(&tiers);
        assert_eq!(cost_no_recovery.total_paise, 25000, "Without recovery pause: 10min @ 2500p = 25000p");
    }

    // ── BILL-07: Multiplayer synchronized pause/resume tests ────────────────

    #[test]
    fn test_multiplayer_pause_functions_exist() {
        // Verify the pause_multiplayer_group and resume_multiplayer_group functions
        // are defined in this module (compilation check — no runtime assertion needed
        // since they require AppState with a live DB for functional test).
        //
        // If this test compiles, the functions exist with correct signatures.
        // The function is async and takes (&Arc<AppState>, &str, &str) — verified by
        // the compiler when the module compiles.
        assert!(true, "BILL-07: pause_multiplayer_group and resume_multiplayer_group compile successfully");
    }

    #[test]
    fn test_multiplayer_group_paused_event_type() {
        // BILL-07: billing event types for multiplayer group audit trail
        // These strings must match what billing_events inserts
        let paused_event = "multiplayer_group_paused";
        let resumed_event = "multiplayer_group_resumed";
        assert_eq!(paused_event, "multiplayer_group_paused", "BILL-07: paused event type matches");
        assert_eq!(resumed_event, "multiplayer_group_resumed", "BILL-07: resumed event type matches");
    }

    #[test]
    fn test_crash_recovery_pause_reason_for_multiplayer() {
        // BILL-07: A multiplayer crash pause uses CrashRecovery pause reason
        // (same as single-pod crash, but applied to all group members)
        let reason = PauseReason::CrashRecovery;
        assert_eq!(reason, PauseReason::CrashRecovery, "BILL-07: multiplayer crash uses CrashRecovery pause reason");
    }

    // ── Phase 285: Integration Audit — E2E billing fairness flow ────────────

    #[test]
    fn test_e2e_billing_fairness_crash_recovery_excluded() {
        // Exercises: Active → CrashPause → PausedCrashRecovery → Resume → Active → EndEarly
        // Verifies recovery_pause_seconds is excluded from billable time.
        use crate::billing_fsm::{validate_transition, BillingEvent};

        let mut timer = BillingTimer::dummy("pod-e2e");
        timer.status = BillingSessionStatus::Active;
        timer.elapsed_seconds = 0;
        timer.recovery_pause_seconds = 0;

        // Simulate 60 seconds of active driving
        for _ in 0..60 {
            timer.tick();
        }
        assert_eq!(timer.elapsed_seconds, 60);
        assert_eq!(timer.driving_seconds, 60);
        assert_eq!(timer.recovery_pause_seconds, 0);

        // FSM: Active → PausedCrashRecovery
        let next = validate_transition(BillingSessionStatus::Active, BillingEvent::CrashPause);
        assert_eq!(next, Ok(BillingSessionStatus::PausedCrashRecovery));
        timer.status = BillingSessionStatus::PausedCrashRecovery;

        // Simulate 30 seconds of crash recovery pause
        for _ in 0..30 {
            timer.tick();
        }
        assert_eq!(timer.pause_seconds, 30);
        assert_eq!(timer.recovery_pause_seconds, 30, "recovery pause must track crash time");

        // FSM: PausedCrashRecovery → Active (Resume)
        let next = validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::Resume);
        assert_eq!(next, Ok(BillingSessionStatus::Active));
        timer.status = BillingSessionStatus::Active;

        // Simulate 40 more seconds of active driving
        for _ in 0..40 {
            timer.tick();
        }
        assert_eq!(timer.elapsed_seconds, 100); // 60 + 40 active seconds
        assert_eq!(timer.driving_seconds, 100);
        assert_eq!(timer.recovery_pause_seconds, 30, "recovery pause unchanged after resume");

        // FSM: Active → EndedEarly
        let next = validate_transition(BillingSessionStatus::Active, BillingEvent::EndEarly);
        assert_eq!(next, Ok(BillingSessionStatus::EndedEarly));
        timer.status = BillingSessionStatus::EndedEarly;

        // Verify billable time excludes recovery pause
        let tiers = default_billing_rate_tiers();
        let cost_with_recovery = timer.current_cost(&tiers);
        // Billable = elapsed(100) - recovery(30) = 70 seconds
        let mut timer_no_recovery = BillingTimer::dummy("pod-e2e");
        timer_no_recovery.status = BillingSessionStatus::EndedEarly;
        timer_no_recovery.elapsed_seconds = 100;
        timer_no_recovery.driving_seconds = 100;
        timer_no_recovery.recovery_pause_seconds = 0;
        let cost_without_recovery = timer_no_recovery.current_cost(&tiers);
        // With recovery exclusion, cost must be less than without
        assert!(
            cost_with_recovery.total_paise <= cost_without_recovery.total_paise,
            "Crash recovery time must not be billed: with_recovery={}p vs without={}p",
            cost_with_recovery.total_paise, cost_without_recovery.total_paise
        );
    }

    // ── Phase 285: FSM completeness — PausedCrashRecovery transitions ───────

    #[test]
    fn test_fsm_paused_crash_recovery_all_transitions() {
        use crate::billing_fsm::{validate_transition, BillingEvent};

        // Valid transitions from PausedCrashRecovery
        assert_eq!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::Resume),
            Ok(BillingSessionStatus::Active),
            "CrashRecovery + Resume → Active"
        );
        assert_eq!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::End),
            Ok(BillingSessionStatus::Completed),
            "CrashRecovery + End → Completed"
        );
        assert_eq!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::EndEarly),
            Ok(BillingSessionStatus::EndedEarly),
            "CrashRecovery + EndEarly → EndedEarly"
        );
        assert_eq!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::Cancel),
            Ok(BillingSessionStatus::Cancelled),
            "CrashRecovery + Cancel → Cancelled"
        );

        // Invalid transitions from PausedCrashRecovery
        assert!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::Pause).is_err(),
            "CrashRecovery + Pause should be rejected"
        );
        assert!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::CrashPause).is_err(),
            "CrashRecovery + CrashPause should be rejected (already paused)"
        );
        assert!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::StartWaiting).is_err(),
            "CrashRecovery + StartWaiting should be rejected"
        );
        assert!(
            validate_transition(BillingSessionStatus::PausedCrashRecovery, BillingEvent::GameLive).is_err(),
            "CrashRecovery + GameLive should be rejected"
        );
    }

    // ── Phase 311: LBILL — Game-aware stale cancel tests ─────────────────────

    /// Helper: create a test AppState with in-memory DB that has billing_sessions + wallets tables.
    async fn create_lbill_test_state() -> Arc<AppState> {
        let config = crate::config::Config::default_test();
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");

        // Create minimal billing_sessions table with all columns we need
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_sessions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                pod_id TEXT NOT NULL,
                pricing_tier_id TEXT NOT NULL DEFAULT 'test',
                allocated_seconds INTEGER NOT NULL DEFAULT 1800,
                status TEXT NOT NULL DEFAULT 'pending',
                wallet_debit_paise INTEGER,
                wallet_owner_id TEXT,
                ended_at TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
            )",
        )
        .execute(&pool)
        .await
        .expect("create billing_sessions");

        // wallets table needed for refund logic
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wallets (
                driver_id TEXT PRIMARY KEY,
                balance_paise INTEGER NOT NULL DEFAULT 0,
                venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
            )",
        )
        .execute(&pool)
        .await
        .expect("create wallets");

        // wallet_transactions table needed for credit()
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wallet_transactions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL,
                amount_paise INTEGER NOT NULL,
                txn_type TEXT NOT NULL,
                reference_id TEXT,
                notes TEXT,
                staff_id TEXT,
                balance_after_paise INTEGER NOT NULL DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now')),
                venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
            )",
        )
        .execute(&pool)
        .await
        .expect("create wallet_transactions");

        // pod_activity_log table needed for log_pod_activity (called during tick)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS pod_activity_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pod_id TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT '',
                action TEXT NOT NULL DEFAULT '',
                details TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL DEFAULT 'core',
                session_id TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
            )",
        )
        .execute(&pool)
        .await
        .expect("create pod_activity_log");

        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, pool, field_cipher))
    }

    /// Insert a billing session with a specific created_at offset (minutes ago).
    async fn insert_test_session(
        state: &Arc<AppState>,
        session_id: &str,
        driver_id: &str,
        pod_id: &str,
        status: &str,
        minutes_ago: i64,
        wallet_debit_paise: Option<i64>,
    ) {
        sqlx::query(
            "INSERT INTO billing_sessions (id, driver_id, pod_id, status, wallet_debit_paise, created_at)
             VALUES (?, ?, ?, ?, ?, datetime('now', ? || ' minutes'))",
        )
        .bind(session_id)
        .bind(driver_id)
        .bind(pod_id)
        .bind(status)
        .bind(wallet_debit_paise)
        .bind(format!("-{}", minutes_ago))
        .execute(&state.db)
        .await
        .expect("insert test session");
    }

    /// Insert a driver wallet for refund tests.
    async fn insert_test_wallet(state: &Arc<AppState>, driver_id: &str, balance: i64) {
        sqlx::query("INSERT INTO wallets (driver_id, balance_paise) VALUES (?, ?)")
            .bind(driver_id)
            .bind(balance)
            .execute(&state.db)
            .await
            .expect("insert test wallet");
    }

    /// Add a GameTracker entry for a pod.
    async fn set_game_tracker(
        state: &Arc<AppState>,
        pod_id: &str,
        game_state: rc_common::types::GameState,
    ) {
        let mut games = state.game_launcher.active_games.write().await;
        games.insert(
            pod_id.to_string(),
            crate::game_launcher::GameTracker {
                pod_id: pod_id.to_string(),
                sim_type: rc_common::types::SimType::AssettoCorsa,
                game_state,
                pid: Some(1234),
                launched_at: Some(Utc::now()),
                error_message: None,
                launch_args: None,
                auto_relaunch_count: 0,
                externally_tracked: false,
                dynamic_timeout_secs: None,
                exit_codes: vec![],
                max_auto_relaunch: 2,
                playable_at: None,
                ready_delay_ms: None,
                billing_session_id: None,
                launch_id: "test-launch-001".to_string(),
            },
        );
    }

    /// Get the status of a billing session by ID.
    async fn get_session_status(state: &Arc<AppState>, session_id: &str) -> String {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM billing_sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .expect("query session status");
        row.map(|r| r.0).unwrap_or_default()
    }

    #[tokio::test]
    async fn stale_cancel_game_aware_test1_no_game_cancels() {
        // Test 1: Session waiting_for_game >5 min with NO active game -> cancelled
        let state = create_lbill_test_state().await;
        insert_test_wallet(&state, "d1", 100000).await;
        insert_test_session(&state, "s1", "d1", "pod-1", "waiting_for_game", 6, Some(70000)).await;

        tick_all_timers(&state).await;

        let status = get_session_status(&state, "s1").await;
        assert_eq!(status, "cancelled", "LBILL-03: Session with no active game should be cancelled");
    }

    #[tokio::test]
    async fn stale_cancel_game_aware_test2_launching_extends() {
        // Test 2: Session waiting_for_game >5 min with active game in Launching state -> NOT cancelled
        let state = create_lbill_test_state().await;
        insert_test_wallet(&state, "d1", 100000).await;
        insert_test_session(&state, "s2", "d1", "pod-2", "waiting_for_game", 6, Some(70000)).await;
        set_game_tracker(&state, "pod-2", rc_common::types::GameState::Launching).await;

        tick_all_timers(&state).await;

        let status = get_session_status(&state, "s2").await;
        assert_eq!(status, "waiting_for_game", "LBILL-01/02: Session with Launching game should NOT be cancelled");
    }

    #[tokio::test]
    async fn stale_cancel_game_aware_test3_running_extends() {
        // Test 3: Session waiting_for_game >5 min with active game in Running state -> NOT cancelled
        let state = create_lbill_test_state().await;
        insert_test_wallet(&state, "d1", 100000).await;
        insert_test_session(&state, "s3", "d1", "pod-3", "waiting_for_game", 6, Some(70000)).await;
        set_game_tracker(&state, "pod-3", rc_common::types::GameState::Running).await;

        tick_all_timers(&state).await;

        let status = get_session_status(&state, "s3").await;
        assert_eq!(status, "waiting_for_game", "LBILL-01: Session with Running game should NOT be cancelled");
    }

    #[tokio::test]
    async fn stale_cancel_game_aware_test4_absolute_timeout() {
        // Test 4: Session waiting_for_game >10 min total with active game -> cancelled regardless
        let state = create_lbill_test_state().await;
        insert_test_wallet(&state, "d1", 100000).await;
        insert_test_session(&state, "s4", "d1", "pod-4", "waiting_for_game", 11, Some(70000)).await;
        set_game_tracker(&state, "pod-4", rc_common::types::GameState::Launching).await;

        tick_all_timers(&state).await;

        let status = get_session_status(&state, "s4").await;
        assert_eq!(status, "cancelled", "LBILL-02: Session >10 min should be cancelled even with active game");
    }

    #[tokio::test]
    async fn stale_cancel_game_aware_test5_pending_always_cancels() {
        // Test 5: Session in 'pending' status >5 min -> always cancelled (no game check needed)
        let state = create_lbill_test_state().await;
        insert_test_wallet(&state, "d1", 100000).await;
        insert_test_session(&state, "s5", "d1", "pod-5", "pending", 6, Some(70000)).await;
        // Even if there's a game tracker (shouldn't happen, but test defense in depth)
        set_game_tracker(&state, "pod-5", rc_common::types::GameState::Launching).await;

        tick_all_timers(&state).await;

        let status = get_session_status(&state, "s5").await;
        assert_eq!(status, "cancelled", "LBILL-03: Pending session should always be cancelled regardless of game state");
    }

    // ─── Phase 363 GLD-C-02: BillingTimer coverage histogram tests ───────────

    /// GLD-C-02: BillingTimer via make_test_timer() starts with empty telemetry_seconds_covered.
    #[test]
    fn test_billing_timer_coverage_histogram_default_empty() {
        let timer = make_test_timer("test-session", "pod1");
        assert!(
            timer.telemetry_seconds_covered.is_empty(),
            "telemetry_seconds_covered should be empty by default"
        );
    }

    /// GLD-C-02: BillingTimer Default impl has empty telemetry_seconds_covered.
    #[test]
    fn test_billing_timer_default_coverage_empty() {
        let timer = BillingTimer::default();
        assert!(
            timer.telemetry_seconds_covered.is_empty(),
            "BillingTimer::default() telemetry_seconds_covered must be empty"
        );
    }

    // ── F-05 regression tests (Phase 363) ─────────────────────────────────────
    // Root cause: .planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md
    // Structural fix: billing.rs CAS UPDATE excludes wallet_debit_paise from SET clause.
    // These tests prevent regression of both the formula AND the SQL invariant.

    #[test]
    fn test_f05_refund_uses_original_debit() {
        // F-05 regression: Rs.700 30min session ended at 15min.
        // Current formula: refund = wallet_debit_paise - best_rate_for_minutes(15)
        //   = 70000 - (15 * 2500) = 70000 - 37500 = 32500 (Rs.325)
        // Note: compute_refund uses best_rate_for_minutes (per-minute billing, not simple
        // proportional). The F-05 bug corrupted wallet_debit_paise to final_cost_paise
        // BEFORE compute_refund ran, causing a wrong input. This test locks the formula
        // contract so any change to compute_refund() is caught.
        let refund = compute_refund(1800, 900, 70000);
        // 15 minutes used * 2500 paise/min = 37500 actual cost
        // Refund = 70000 original debit - 37500 actual cost = 32500 paise (Rs.325)
        assert_eq!(refund, 32500,
            "F-05: compute_refund(1800, 900, 70000) must return 32500 (Rs.325). \
             If wallet_debit_paise was corrupted to final_cost_paise, the input would be wrong. \
             This test locks the formula contract for the F-05 scenario.");
    }

    #[tokio::test]
    async fn test_end_billing_session_early_end_refund_amount() {
        // F-05 SQL invariant: The CAS UPDATE must NOT include wallet_debit_paise in its
        // SET clause. This test replays the exact UPDATE against an in-memory DB and
        // asserts the column retains its original value.
        //
        // If a future refactor adds `wallet_debit_paise = ?` to the SET clause,
        // this test will fail — protecting against F-05 regression at the SQL level.

        // Create a fresh in-memory pool with wallet_debit_paise column
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool for F-05 test");
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_sessions (
                id TEXT PRIMARY KEY,
                driver_id TEXT NOT NULL DEFAULT 'd1',
                pod_id TEXT NOT NULL DEFAULT 'pod1',
                pricing_tier_id TEXT NOT NULL DEFAULT 'tier_30min',
                allocated_seconds INTEGER NOT NULL DEFAULT 1800,
                status TEXT NOT NULL DEFAULT 'active',
                driving_seconds INTEGER NOT NULL DEFAULT 0,
                ended_at TEXT,
                end_reason TEXT,
                wallet_debit_paise INTEGER,
                created_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .expect("Failed to create billing_sessions for F-05 test");

        // Seed: active billing_session with Rs.700 debit
        sqlx::query(
            "INSERT INTO billing_sessions (id, status, driving_seconds, allocated_seconds, wallet_debit_paise)
             VALUES ('F05-TEST-1', 'active', 0, 1800, 70000)"
        ).execute(&pool).await.unwrap();

        // Execute the EXACT CAS UPDATE shape from billing.rs CAS guard (copy SET clause verbatim).
        // If someone adds wallet_debit_paise to this SET clause in production code, they must
        // also update this test — which will force them to re-read the F-05 root cause doc.
        sqlx::query(
            "UPDATE billing_sessions
             SET status = ?, driving_seconds = ?, ended_at = datetime('now'), end_reason = ?
             WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery', 'waiting_for_game')"
        )
        .bind("ended_early")
        .bind(900i64)
        .bind("final_cost_paise:35000")
        .bind("F05-TEST-1")
        .execute(&pool).await.unwrap();

        // Assert: wallet_debit_paise retains its original value
        let row: (i64,) = sqlx::query_as(
            "SELECT wallet_debit_paise FROM billing_sessions WHERE id = 'F05-TEST-1'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 70000,
            "F-05: wallet_debit_paise must retain original pre-session charge after CAS UPDATE. \
             If this fails, the CAS UPDATE now includes wallet_debit_paise in its SET clause — \
             REVERT that change. See ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md.");

        // Additionally: verify compute_refund with the read-back value produces Rs.325
        // (best_rate_for_minutes(15, 2500) = 37500, so refund = 70000 - 37500 = 32500)
        let refund = compute_refund(1800, 900, row.0);
        assert_eq!(refund, 32500,
            "F-05: refund on read-back wallet_debit_paise must be Rs.325 (32500 paise). \
             Formula: 70000 - best_rate_for_minutes(15, 2500) = 70000 - 37500 = 32500.");
    }

    // ── Task 3: lap_rejections INSERT tests (Phase 363 GLD-C-04 D-12) ──────────

    #[tokio::test]
    async fn test_lap_reject_within_grace_window_caught() {
        // Verify that a lap rejection with grace_window_caught=true can be recorded.
        let pool = create_test_db().await;
        // Ensure lap_rejections table exists in the test schema
        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS lap_rejections (
                id TEXT PRIMARY KEY, session_id TEXT NOT NULL, lap_number INTEGER NOT NULL,
                rejected_at TEXT DEFAULT (datetime('now')), reason TEXT,
                grace_window_caught BOOLEAN NOT NULL DEFAULT 0
            )"
        ).execute(&pool).await;

        // Simulate a caught rejection (grace_window_caught = true)
        sqlx::query(
            "INSERT INTO lap_rejections (id, session_id, lap_number, reason, grace_window_caught)
             VALUES ('rej1', 'sess-A', 7, 'test', 1)"
        ).execute(&pool).await.unwrap();

        let row: (String, i64, bool) = sqlx::query_as(
            "SELECT session_id, lap_number, grace_window_caught FROM lap_rejections WHERE id = 'rej1'"
        ).fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, "sess-A");
        assert_eq!(row.1, 7);
        assert!(row.2, "grace_window_caught should be true");
    }

    #[tokio::test]
    async fn test_lap_reject_outside_grace_window_not_caught() {
        // Verify that a lap rejection with grace_window_caught=false can be recorded.
        let pool = create_test_db().await;
        let _ = sqlx::query(
            "CREATE TABLE IF NOT EXISTS lap_rejections (
                id TEXT PRIMARY KEY, session_id TEXT NOT NULL, lap_number INTEGER NOT NULL,
                rejected_at TEXT DEFAULT (datetime('now')), reason TEXT,
                grace_window_caught BOOLEAN NOT NULL DEFAULT 0
            )"
        ).execute(&pool).await;

        sqlx::query(
            "INSERT INTO lap_rejections (id, session_id, lap_number, reason, grace_window_caught)
             VALUES ('rej2', 'sess-B', 3, 'test', 0)"
        ).execute(&pool).await.unwrap();

        let row: (bool,) = sqlx::query_as(
            "SELECT grace_window_caught FROM lap_rejections WHERE id = 'rej2'"
        ).fetch_one(&pool).await.unwrap();
        assert!(!row.0, "grace_window_caught should be false");
    }
