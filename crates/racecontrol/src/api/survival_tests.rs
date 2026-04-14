//! Tests for survival.rs (Heal Lease Protocol) — extracted for <500 line compliance.

#[cfg(test)]
mod tests {
    use super::super::survival::*;
    use chrono::Utc;
    use rc_common::survival_types::{ActionId, HealLeaseRequest, HealLease, SurvivalLayer};

    fn make_req(pod_id: &str, layer: SurvivalLayer, reason: &str, ttl_secs: u64) -> HealLeaseRequest {
        HealLeaseRequest {
            pod_id: pod_id.to_string(),
            layer,
            action_id: ActionId::new(),
            ttl_secs,
            reason: reason.to_string(),
        }
    }

    // ─── grant tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_request_lease_grants_when_no_active_lease() {
        let mgr = LeaseManager::new();
        let req = make_req("pod-1", SurvivalLayer::Layer1Watchdog, "watchdog restart", 120);
        let resp = mgr.request_lease(&req);
        assert!(resp.granted, "must grant when no lease exists");
        assert!(resp.lease.is_some(), "must return lease on grant");
        assert!(resp.reason.is_none(), "must not have denial reason on grant");
        let lease = resp.lease.unwrap();
        assert_eq!(lease.pod_id, "pod-1");
        assert_eq!(lease.ttl_secs, 120);
    }

    #[test]
    fn test_request_lease_denies_when_another_layer_holds_non_expired_lease() {
        let mgr = LeaseManager::new();
        // Layer1 gets the lease first
        let req1 = make_req("pod-2", SurvivalLayer::Layer1Watchdog, "first heal", 300);
        let resp1 = mgr.request_lease(&req1);
        assert!(resp1.granted, "first request must be granted");

        // Layer2 tries to take the lease while Layer1 holds it
        let req2 = make_req("pod-2", SurvivalLayer::Layer2FleetHealer, "second heal", 300);
        let resp2 = mgr.request_lease(&req2);
        assert!(!resp2.granted, "must deny when another layer holds active lease");
        assert!(resp2.lease.is_none());
        assert!(resp2.reason.is_some(), "denial must include a reason");
    }

    #[test]
    fn test_request_lease_grants_when_existing_lease_is_expired() {
        let mgr = LeaseManager::new();
        // Manually insert an expired lease
        {
            let past = Utc::now() - chrono::Duration::hours(1);
            let expires_at = (Utc::now() - chrono::Duration::minutes(30)).to_rfc3339();
            let lease = HealLease {
                pod_id: "pod-3".to_string(),
                granted_to: SurvivalLayer::Layer1Watchdog,
                action_id: ActionId::new(),
                ttl_secs: 60,
                granted_at: past.to_rfc3339(),
                expires_at,
            };
            let mut leases = mgr.leases.lock().unwrap();
            leases.insert("pod-3".to_string(), lease);
        }

        // New request should succeed (auto-frees expired lease)
        let req = make_req("pod-3", SurvivalLayer::Layer2FleetHealer, "takeover expired", 120);
        let resp = mgr.request_lease(&req);
        assert!(resp.granted, "must grant when existing lease is expired");
        assert_eq!(
            resp.lease.as_ref().unwrap().granted_to,
            SurvivalLayer::Layer2FleetHealer,
            "new lease must belong to Layer2"
        );
    }

    // ─── renew tests ──────────────────────────────────────────────────────────

    #[test]
    fn test_renew_lease_extends_ttl_for_lease_holder() {
        let mgr = LeaseManager::new();
        let req = make_req("pod-4", SurvivalLayer::Layer2FleetHealer, "fleet heal", 120);
        let resp = mgr.request_lease(&req);
        assert!(resp.granted);

        let new_action_id = ActionId::new();
        let renewed = mgr.renew_lease("pod-4", SurvivalLayer::Layer2FleetHealer, &new_action_id, 300);
        assert!(renewed.is_ok(), "renewal must succeed for the holder");
        let lease = renewed.unwrap();
        assert_eq!(lease.ttl_secs, 300, "ttl must be updated");
        // expires_at must be in the future (at least 290s from now to allow for test latency)
        let expires = chrono::DateTime::parse_from_rfc3339(&lease.expires_at).unwrap();
        let remaining = expires.signed_duration_since(Utc::now()).num_seconds();
        assert!(remaining >= 290, "renewed lease must expire ~300s from now, got {}s", remaining);
    }

    #[test]
    fn test_renew_lease_rejects_different_layer_than_holder() {
        let mgr = LeaseManager::new();
        let req = make_req("pod-5", SurvivalLayer::Layer1Watchdog, "watchdog heal", 120);
        mgr.request_lease(&req);

        // Layer2 tries to renew Layer1's lease
        let action_id = ActionId::new();
        let result = mgr.renew_lease("pod-5", SurvivalLayer::Layer2FleetHealer, &action_id, 300);
        assert!(result.is_err(), "non-holder must not be able to renew");
        let err = result.unwrap_err();
        assert!(
            err.contains("Layer1Watchdog") || err.contains("Layer2FleetHealer"),
            "error must mention layers: {}",
            err
        );
    }

    #[test]
    fn test_renew_lease_rejects_when_no_lease_exists() {
        let mgr = LeaseManager::new();
        let action_id = ActionId::new();
        let result = mgr.renew_lease("pod-99", SurvivalLayer::Layer1Watchdog, &action_id, 60);
        assert!(result.is_err(), "renew must fail when no lease exists");
    }

    // ─── release tests ────────────────────────────────────────────────────────

    #[test]
    fn test_release_lease_removes_the_lease() {
        let mgr = LeaseManager::new();
        let req = make_req("pod-6", SurvivalLayer::Layer3Guardian, "guardian heal", 120);
        mgr.request_lease(&req);

        // Verify it exists
        assert!(mgr.get_lease("pod-6").is_some(), "lease must exist before release");

        // Release
        mgr.release_lease("pod-6");

        // Verify it's gone
        let leases = mgr.leases.lock().unwrap();
        assert!(
            !leases.contains_key("pod-6"),
            "lease must be removed after release"
        );
    }

    #[test]
    fn test_release_lease_is_idempotent() {
        let mgr = LeaseManager::new();
        // Release a non-existent lease — must not panic
        mgr.release_lease("pod-does-not-exist");
        mgr.release_lease("pod-does-not-exist"); // second call also must not panic
    }

    // ─── action_id preservation ───────────────────────────────────────────────

    #[test]
    fn test_action_id_preserved_through_request_grant_cycle() {
        let mgr = LeaseManager::new();
        let action_id = ActionId("test-action-id-123".to_string());
        let req = HealLeaseRequest {
            pod_id: "pod-7".to_string(),
            layer: SurvivalLayer::Layer1Watchdog,
            action_id: action_id.clone(),
            ttl_secs: 60,
            reason: "tracing test".to_string(),
        };
        let resp = mgr.request_lease(&req);
        assert!(resp.granted);
        let lease = resp.lease.unwrap();
        assert_eq!(
            lease.action_id, action_id,
            "action_id must be preserved in the granted lease"
        );
    }

    #[test]
    fn test_action_id_preserved_after_renew() {
        let mgr = LeaseManager::new();
        let original_action_id = ActionId("original-action-456".to_string());
        let req = HealLeaseRequest {
            pod_id: "pod-8".to_string(),
            layer: SurvivalLayer::Layer2FleetHealer,
            action_id: original_action_id.clone(),
            ttl_secs: 120,
            reason: "action_id preservation test".to_string(),
        };
        mgr.request_lease(&req);

        // Renew with a different action_id (the original should be preserved in the lease)
        let renew_action_id = ActionId("renew-action-789".to_string());
        let renewed = mgr.renew_lease(
            "pod-8",
            SurvivalLayer::Layer2FleetHealer,
            &renew_action_id,
            200,
        );
        assert!(renewed.is_ok());
        let lease = renewed.unwrap();
        // The lease keeps the original action_id (the renew action_id is for audit log only)
        assert_eq!(
            lease.action_id, original_action_id,
            "original action_id must be preserved through renew"
        );
    }

    #[test]
    fn test_after_release_new_grant_is_possible() {
        let mgr = LeaseManager::new();
        // Layer1 gets lease
        let req1 = make_req("pod-9", SurvivalLayer::Layer1Watchdog, "first", 120);
        assert!(mgr.request_lease(&req1).granted);

        // Release
        mgr.release_lease("pod-9");

        // Layer2 can now get the lease
        let req2 = make_req("pod-9", SurvivalLayer::Layer2FleetHealer, "second", 120);
        let resp2 = mgr.request_lease(&req2);
        assert!(resp2.granted, "must grant after release");
        assert_eq!(resp2.lease.unwrap().granted_to, SurvivalLayer::Layer2FleetHealer);
    }
}
