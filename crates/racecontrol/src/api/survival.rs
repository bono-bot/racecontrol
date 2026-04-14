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
    routing::post,
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
    pub(crate) leases: Mutex<HashMap<String, HealLease>>,
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

// Tests moved to survival_tests.rs for <500 line compliance
