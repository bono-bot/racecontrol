// survival.rs — Phase 267-02: Heal Lease Protocol
//
// Server-arbitrated heal lease: any healing layer must request exclusive access
// to a pod before acting. Prevents the 5-healer fight (Pitfall 1).
//
// Endpoints:
//   POST   /api/v1/pods/:pod_id/heal-lease        -> request lease
//   DELETE /api/v1/pods/:pod_id/heal-lease        -> release lease
//   POST   /api/v1/pods/:pod_id/heal-lease/renew  -> renew lease

use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, post},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::state::AppState;
use rc_common::survival_types::{ActionId, HealLease, HealLeaseRequest, HealLeaseResponse, SurvivalLayer};

// ─── LeaseManager ─────────────────────────────────────────────────────────────

/// In-memory, mutex-protected heal lease store.
/// Maps pod_id -> active HealLease. One lease per pod at a time.
///
/// NEVER hold this lock across .await — clone/snapshot before async work.
pub struct LeaseManager {
    leases: Mutex<HashMap<String, HealLease>>,
}

impl LeaseManager {
    pub fn new() -> Self {
        Self {
            leases: Mutex::new(HashMap::new()),
        }
    }

    /// Request a heal lease for the given pod.
    ///
    /// - Grants if no active (non-expired) lease exists.
    /// - Auto-frees expired leases and grants to the new requester.
    /// - Denies if a valid lease is held by another layer.
    pub fn request_lease(&self, req: &HealLeaseRequest) -> HealLeaseResponse {
        let mut leases = self.leases.lock().unwrap_or_else(|e| e.into_inner());

        // Check for existing lease
        if let Some(existing) = leases.get(&req.pod_id) {
            let is_expired = chrono::DateTime::parse_from_rfc3339(&existing.expires_at)
                .map(|dt| dt < Utc::now())
                .unwrap_or(true); // treat parse errors as expired

            if !is_expired {
                return HealLeaseResponse {
                    granted: false,
                    lease: None,
                    reason: Some(format!(
                        "pod {} already held by {:?} (action_id: {}, expires: {})",
                        req.pod_id, existing.granted_to, existing.action_id, existing.expires_at
                    )),
                };
            }
            // Expired lease — remove and grant below
            tracing::info!(
                action_id = %req.action_id,
                pod_id = %req.pod_id,
                "heal lease: expired lease auto-freed, granting to {:?}",
                req.layer
            );
        }

        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(req.ttl_secs as i64);
        let lease = HealLease {
            pod_id: req.pod_id.clone(),
            granted_to: req.layer,
            action_id: req.action_id.clone(),
            ttl_secs: req.ttl_secs,
            granted_at: now.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
        };

        tracing::info!(
            action_id = %req.action_id,
            pod_id = %req.pod_id,
            layer = ?req.layer,
            ttl_secs = req.ttl_secs,
            reason = %req.reason,
            "heal lease GRANTED"
        );

        leases.insert(req.pod_id.clone(), lease.clone());
        HealLeaseResponse {
            granted: true,
            lease: Some(lease),
            reason: None,
        }
    }

    /// Renew an active lease. Only the current holder can renew.
    /// Extends expires_at by ttl_secs from now.
    pub fn renew_lease(
        &self,
        pod_id: &str,
        layer: SurvivalLayer,
        action_id: &ActionId,
        ttl_secs: u64,
    ) -> Result<HealLease, String> {
        let mut leases = self.leases.lock().unwrap_or_else(|e| e.into_inner());

        let existing = leases
            .get_mut(pod_id)
            .ok_or_else(|| format!("no active lease for pod {}", pod_id))?;

        // Verify the renewer is the current holder
        if existing.granted_to != layer {
            return Err(format!(
                "renew rejected: lease held by {:?}, not {:?}",
                existing.granted_to, layer
            ));
        }

        // Check if expired (can't renew an expired lease)
        let is_expired = chrono::DateTime::parse_from_rfc3339(&existing.expires_at)
            .map(|dt| dt < Utc::now())
            .unwrap_or(true);
        if is_expired {
            return Err(format!(
                "renew rejected: lease for pod {} has already expired",
                pod_id
            ));
        }

        let now = Utc::now();
        let new_expires = now + chrono::Duration::seconds(ttl_secs as i64);
        existing.expires_at = new_expires.to_rfc3339();
        existing.ttl_secs = ttl_secs;
        // Preserve action_id from the original grant for traceability
        let _ = action_id; // action_id is logged but original is kept in the lease

        tracing::info!(
            action_id = %existing.action_id,
            pod_id = pod_id,
            layer = ?layer,
            new_expires = %existing.expires_at,
            "heal lease RENEWED"
        );

        Ok(existing.clone())
    }

    /// Release a lease for the given pod. Idempotent — releasing a non-existent lease is Ok.
    pub fn release_lease(&self, pod_id: &str) {
        let mut leases = self.leases.lock().unwrap_or_else(|e| e.into_inner());
        leases.remove(pod_id);
        tracing::info!(pod_id = pod_id, "heal lease RELEASED");
    }

    /// Returns the current lease for a pod if it exists and is not expired.
    pub fn get_lease(&self, pod_id: &str) -> Option<HealLease> {
        let leases = self.leases.lock().unwrap_or_else(|e| e.into_inner());
        let lease = leases.get(pod_id)?;
        let is_expired = chrono::DateTime::parse_from_rfc3339(&lease.expires_at)
            .map(|dt| dt < Utc::now())
            .unwrap_or(true);
        if is_expired {
            None
        } else {
            Some(lease.clone())
        }
    }
}

impl Default for LeaseManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Endpoint request/response types ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RenewLeaseBody {
    pub layer: SurvivalLayer,
    pub action_id: ActionId,
    pub ttl_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct RenewLeaseResponse {
    pub ok: bool,
    pub lease: Option<HealLease>,
    pub reason: Option<String>,
}

// ─── Auth helper ──────────────────────────────────────────────────────────────

fn check_service_key(state: &Arc<AppState>, headers: &HeaderMap) -> bool {
    match &state.config.pods.sentry_service_key {
        Some(expected) if !expected.is_empty() => {
            let provided = headers
                .get("x-service-key")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            provided == expected.as_str()
        }
        // No key configured — allow (permissive mode during initial setup)
        _ => true,
    }
}

// ─── Axum endpoint handlers ───────────────────────────────────────────────────

/// POST /api/v1/pods/:pod_id/heal-lease
/// Request exclusive heal lease for a pod.
/// Auth: X-Service-Key header (same pattern as fleet_alert, sentry_crash).
async fn request_heal_lease(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(pod_id): Path<String>,
    Json(mut req): Json<HealLeaseRequest>,
) -> (StatusCode, Json<HealLeaseResponse>) {
    if !check_service_key(&state, &headers) {
        tracing::warn!(pod_id = %pod_id, "heal-lease: rejected — invalid X-Service-Key");
        return (
            StatusCode::UNAUTHORIZED,
            Json(HealLeaseResponse {
                granted: false,
                lease: None,
                reason: Some("invalid or missing X-Service-Key".to_string()),
            }),
        );
    }

    // Normalize pod_id: use the path parameter as authoritative
    req.pod_id = pod_id.clone();

    let response = state.lease_manager.request_lease(&req);
    let status = if response.granted {
        StatusCode::OK
    } else {
        StatusCode::CONFLICT
    };

    (status, Json(response))
}

/// DELETE /api/v1/pods/:pod_id/heal-lease
/// Release the heal lease for a pod. Idempotent.
/// Auth: X-Service-Key header.
async fn release_heal_lease(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(pod_id): Path<String>,
) -> StatusCode {
    if !check_service_key(&state, &headers) {
        tracing::warn!(pod_id = %pod_id, "heal-lease release: rejected — invalid X-Service-Key");
        return StatusCode::UNAUTHORIZED;
    }

    state.lease_manager.release_lease(&pod_id);
    StatusCode::OK
}

/// POST /api/v1/pods/:pod_id/heal-lease/renew
/// Extend an active lease TTL. Only the current holder can renew.
/// Auth: X-Service-Key header.
async fn renew_heal_lease(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(pod_id): Path<String>,
    Json(body): Json<RenewLeaseBody>,
) -> (StatusCode, Json<RenewLeaseResponse>) {
    if !check_service_key(&state, &headers) {
        tracing::warn!(pod_id = %pod_id, "heal-lease renew: rejected — invalid X-Service-Key");
        return (
            StatusCode::UNAUTHORIZED,
            Json(RenewLeaseResponse {
                ok: false,
                lease: None,
                reason: Some("invalid or missing X-Service-Key".to_string()),
            }),
        );
    }

    match state
        .lease_manager
        .renew_lease(&pod_id, body.layer, &body.action_id, body.ttl_secs)
    {
        Ok(lease) => (
            StatusCode::OK,
            Json(RenewLeaseResponse {
                ok: true,
                lease: Some(lease),
                reason: None,
            }),
        ),
        Err(reason) => {
            tracing::warn!(
                pod_id = %pod_id,
                action_id = %body.action_id,
                reason = %reason,
                "heal-lease renew DENIED"
            );
            (
                StatusCode::CONFLICT,
                Json(RenewLeaseResponse {
                    ok: false,
                    lease: None,
                    reason: Some(reason),
                }),
            )
        }
    }
}

// ─── Router ───────────────────────────────────────────────────────────────────

/// Returns the heal-lease sub-router. Call from api_routes() to register.
pub fn survival_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/pods/{pod_id}/heal-lease",
            post(request_heal_lease).delete(release_heal_lease),
        )
        .route("/pods/{pod_id}/heal-lease/renew", post(renew_heal_lease))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
