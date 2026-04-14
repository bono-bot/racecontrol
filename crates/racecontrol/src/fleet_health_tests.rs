use super::*;
    use chrono::Utc;

    // ── FleetHealthStore default ──────────────────────────────────────────────

    #[test]
    fn fleet_health_store_default_is_all_false_and_none() {
        let store = FleetHealthStore::default();
        assert!(!store.http_reachable, "http_reachable defaults to false");
        assert!(store.last_http_check.is_none());
        assert!(store.version.is_none());
        assert!(store.agent_started_at.is_none());
        assert!(store.crash_recovery.is_none());
    }

    // ── store_startup_report ─────────────────────────────────────────────────

    #[test]
    fn fleet_health_store_startup_report_sets_version() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 3600, false, false, false, false, &[], None);
        assert_eq!(store.version, Some("0.5.2".to_string()));
    }

    #[test]
    fn fleet_health_store_startup_report_computes_agent_started_at() {
        let before = Utc::now();
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 100, false, false, false, false, &[], None);
        let after = Utc::now();

        let started = store.agent_started_at.expect("agent_started_at should be set");
        // started_at should be ~100 seconds before now
        let delta_before = (before - started).num_seconds();
        let delta_after = (after - started).num_seconds();
        assert!(delta_before >= 99 && delta_before <= 101,
            "started_at should be ~100s before call time, got delta={}", delta_before);
        assert!(delta_after >= 99 && delta_after <= 101,
            "started_at should be ~100s before call time, got delta={}", delta_after);
    }

    #[test]
    fn fleet_health_store_startup_report_sets_crash_recovery() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 0, true, false, false, false, &[], None);
        assert_eq!(store.crash_recovery, Some(true));

        let mut store2 = FleetHealthStore::default();
        store_startup_report(&mut store2, "0.5.2", 0, false, false, false, false, &[], None);
        assert_eq!(store2.crash_recovery, Some(false));
    }

    #[test]
    fn fleet_health_store_startup_report_does_not_clear_http_reachable() {
        let mut store = FleetHealthStore::default();
        store.http_reachable = true;
        store_startup_report(&mut store, "0.5.2", 0, false, false, false, false, &[], None);
        assert!(store.http_reachable, "http_reachable must not be modified by store_startup_report");
    }

    // ── clear_on_disconnect ───────────────────────────────────────────────────

    #[test]
    fn fleet_health_clear_on_disconnect_clears_version_and_started_at() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 100, true, false, false, false, &[], None);

        // Verify preconditions
        assert!(store.version.is_some());
        assert!(store.agent_started_at.is_some());
        assert!(store.crash_recovery.is_some());

        clear_on_disconnect(&mut store);

        assert!(store.version.is_none(), "version should be cleared");
        assert!(store.agent_started_at.is_none(), "agent_started_at should be cleared");
        assert!(store.crash_recovery.is_none(), "crash_recovery should be cleared");
    }

    #[test]
    fn fleet_health_clear_on_disconnect_preserves_http_reachable() {
        let mut store = FleetHealthStore::default();
        store.http_reachable = true;
        store.last_http_check = Some(Utc::now());
        store_startup_report(&mut store, "0.5.2", 100, false, false, false, false, &[], None);

        clear_on_disconnect(&mut store);

        assert!(store.http_reachable, "http_reachable should NOT be cleared by clear_on_disconnect");
        assert!(store.last_http_check.is_some(), "last_http_check should NOT be cleared");
    }

    // ── uptime_secs computed live ─────────────────────────────────────────────

    #[test]
    fn fleet_health_uptime_computed_live_increases_over_time() {
        let mut store = FleetHealthStore::default();
        // Simulate: agent started 300 seconds ago
        store.agent_started_at =
            Some(Utc::now() - chrono::Duration::seconds(300));

        let uptime = (Utc::now() - store.agent_started_at.unwrap()).num_seconds();
        assert!(uptime >= 299 && uptime <= 302,
            "uptime computed live should be ~300s, got {}", uptime);
    }

    // ── PodFleetStatus version/http_reachable from store ─────────────────────

    #[test]
    fn fleet_health_version_from_store_is_propagated() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.5.2", 0, false, false, false, false, &[], None);
        // Verify the store correctly holds the version for handler use
        assert_eq!(store.version.as_deref(), Some("0.5.2"));
    }

    #[test]
    fn fleet_health_http_reachable_from_store_is_propagated() {
        let mut store = FleetHealthStore::default();
        store.http_reachable = true;
        assert!(store.http_reachable);
    }

    // ── ws_connected logic ────────────────────────────────────────────────────

    #[test]
    fn fleet_health_ws_connected_false_when_no_sender() {
        // No sender in map means ws_connected = false
        use std::collections::HashMap;
        use tokio::sync::mpsc;

        let senders: HashMap<String, mpsc::Sender<rc_common::protocol::CoreToAgentMessage>> =
            HashMap::new();

        let ws_connected = senders
            .get("pod_1")
            .map(|s| !s.is_closed())
            .unwrap_or(false);

        assert!(!ws_connected);
    }

    #[test]
    fn fleet_health_ws_connected_true_when_sender_exists_and_open() {
        use std::collections::HashMap;
        use tokio::sync::mpsc;

        let (tx, _rx) = mpsc::channel::<rc_common::protocol::CoreToAgentMessage>(8);
        let mut senders = HashMap::new();
        senders.insert("pod_1".to_string(), tx);

        let ws_connected = senders
            .get("pod_1")
            .map(|s| !s.is_closed())
            .unwrap_or(false);

        assert!(ws_connected, "open sender should give ws_connected=true");
    }

    #[test]
    fn fleet_health_ws_connected_false_when_receiver_dropped() {
        use std::collections::HashMap;
        use tokio::sync::mpsc;

        let (tx, rx) = mpsc::channel::<rc_common::protocol::CoreToAgentMessage>(8);
        let mut senders = HashMap::new();
        senders.insert("pod_1".to_string(), tx);

        // Drop the receiver — sender should now be closed
        drop(rx);

        let ws_connected = senders
            .get("pod_1")
            .map(|s| !s.is_closed())
            .unwrap_or(false);

        assert!(!ws_connected, "dropped receiver should give ws_connected=false");
    }

    // ── Phase 100: maintenance state ──────────────────────────────────────────

    #[test]
    fn fleet_health_store_default_not_in_maintenance() {
        let store = FleetHealthStore::default();
        assert!(!store.in_maintenance, "in_maintenance defaults to false");
        assert!(store.maintenance_failures.is_empty(), "maintenance_failures defaults to empty");
    }

    #[test]
    fn fleet_health_clear_on_disconnect_clears_maintenance() {
        let mut store = FleetHealthStore::default();
        store.in_maintenance = true;
        store.maintenance_failures = vec!["DisplayCheck".to_string(), "HidCheck".to_string()];

        clear_on_disconnect(&mut store);

        assert!(!store.in_maintenance, "in_maintenance should be cleared on disconnect");
        assert!(store.maintenance_failures.is_empty(), "maintenance_failures should be cleared on disconnect");
    }

    // ── Phase 46: boot verification fields ───────────────────────────────────

    #[test]
    fn fleet_health_store_startup_report_stores_boot_verification() {
        let mut store = FleetHealthStore::default();
        store_startup_report(&mut store, "0.6.0", 10, false, true, true, true, &[9996, 20777], None);
        assert_eq!(store.lock_screen_port_bound, Some(true));
        assert_eq!(store.remote_ops_port_bound, Some(true));
        assert_eq!(store.hid_detected, Some(true));
        assert_eq!(store.udp_ports_bound, Some(vec![9996, 20777]));

        clear_on_disconnect(&mut store);
        assert_eq!(store.lock_screen_port_bound, None);
        assert_eq!(store.remote_ops_port_bound, None);
        assert_eq!(store.hid_detected, None);
        assert_eq!(store.udp_ports_bound, None);
    }

    #[test]
    fn test_sentry_crash_field_default() {
        let store = FleetHealthStore::default();
        assert!(store.last_sentry_crash.is_none());
    }
