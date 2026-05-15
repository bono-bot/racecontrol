use std::sync::Arc;

use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::AppState;

// ─── Role constants ───────────────────────────────────────────────────────────

/// Valid staff roles in ascending privilege order.
/// "staff" is a legacy alias accepted during token decode but mapped to "cashier".
pub const ROLE_CASHIER: &str = "cashier";
pub const ROLE_MANAGER: &str = "manager";
pub const ROLE_SUPERADMIN: &str = "superadmin";

/// All valid roles that require_staff_jwt will accept.
const VALID_ROLES: &[&str] = &["cashier", "manager", "superadmin", "staff"];

// ─── Staff JWT Claims ─────────────────────────────────────────────────────

/// JWT claims for staff/admin authentication.
/// Separate from customer `Claims` -- staff tokens carry a `role` field
/// that customer tokens do not have. `require_staff_jwt` rejects tokens
/// that cannot be deserialized into StaffClaims (missing role field).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaffClaims {
    /// Staff identifier ("admin", employee ID, etc.)
    pub sub: String,
    /// Role: "cashier", "manager", "superadmin" (or legacy "staff" -> treated as "cashier")
    pub role: String,
    /// Expiration (UNIX timestamp)
    pub exp: usize,
    /// Issued-at (UNIX timestamp)
    pub iat: usize,
}

impl StaffClaims {
    /// Normalized role — maps legacy "staff" to "cashier" for uniform comparison.
    pub fn normalized_role(&self) -> &str {
        if self.role == "staff" {
            "cashier"
        } else {
            &self.role
        }
    }

    /// Returns true if this claims' role is in the given allowed roles list.
    /// Applies legacy "staff" -> "cashier" mapping.
    pub fn has_role(&self, allowed: &[&str]) -> bool {
        allowed.contains(&self.normalized_role())
    }
}

// ─── Token extraction helper ──────────────────────────────────────────────

/// V2 row 7.6 cookie-auth substrate — session cookie name (Phase 1).
///
/// Captain D-7.6 RCA §4 (2026-05-13) names the V2 staff session cookie
/// `rp_staff_session`. Cookie attributes (HttpOnly + Secure + SameSite=Lax +
/// Domain=.racingpoint.cloud + Max-Age = JWT TTL) are emitted at
/// `api::auth_staff::staff_validate_pin`. This constant + the
/// `extract_session_cookie` helper below establish the bidirectional contract.
pub(crate) const STAFF_SESSION_COOKIE_NAME: &str = "rp_staff_session";

/// Extract the V2 staff session cookie value from request `Cookie` headers.
///
/// Returns `Some(token)` when exactly one `rp_staff_session=<value>` appears
/// across all `Cookie:` headers; returns `None` when:
///   - No `Cookie` header carries `rp_staff_session=`
///   - Multiple `rp_staff_session=` values are present (D-7.6-MMA Step 1
///     finding F-7.6-D-3 / R1 P0 0.95 / 5/5 consensus — duplicate cookies are
///     fail-closed to defeat session-fixation via attacker-planted cookie).
///   - Any `Cookie` header value is not valid UTF-8.
///
/// Per F-7.6-D-7 (R1) the helper iterates `headers.get_all("cookie")` rather
/// than `get("cookie")` first-only, so a `rp_staff_session=` value carried in
/// a second/third `Cookie:` header is still extracted (RFC 6265 §5.4 allows
/// multiple Cookie headers; axum HeaderMap does not auto-merge).
pub(crate) fn extract_session_cookie<B>(req: &Request<B>) -> Option<String> {
    let prefix = format!("{}=", STAFF_SESSION_COOKIE_NAME);
    let mut found: Option<String> = None;
    for hv in req.headers().get_all("cookie") {
        let Ok(s) = hv.to_str() else { return None; };
        for part in s.split(';') {
            let kv = part.trim();
            if let Some(val) = kv.strip_prefix(&prefix) {
                if found.is_some() {
                    // F-7.6-D-3 fail-closed on duplicates.
                    return None;
                }
                found = Some(val.to_string());
            }
        }
    }
    found
}

/// Extract and validate StaffClaims from the request.
///
/// Priority order (RCA §4 B + D-7.6-MMA Step 1 Q3 disposition 5/5 OR 4/5
/// majority — Bearer for service-key callers / rc-agent path, Cookie for
/// staff browser path):
///   1. `Authorization: Bearer <token>` header (service-key compatible)
///   2. `Cookie: rp_staff_session=<token>` header (V2 staff browser path)
///
/// Returns `Ok(StaffClaims)` on success, `Err(())` on any failure. The
/// downstream JWT decode + role check + idle-timeout check are identical
/// regardless of which transport delivered the token — the cookie transport
/// is parallel to Bearer, not a separate auth class.
fn extract_staff_claims<B>(state: &Arc<AppState>, req: &Request<B>) -> Result<StaffClaims, ()> {
    let bearer = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer ").map(str::to_string));

    let token = match bearer {
        Some(t) => t,
        None => extract_session_cookie(req).ok_or(())?,
    };
    let token = token.as_str();

    let secret = &state.config.auth.jwt_secret;

    // MMA-P3: Try current secret first, then previous secret for rotation grace period.
    // This prevents instant staff lockout when jwt_secret changes in config.
    let data = jsonwebtoken::decode::<StaffClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .or_else(|_| {
        // Fall back to previous secret if configured
        if let Some(ref prev_secret) = state.config.auth.jwt_secret_previous
            && !prev_secret.is_empty() {
                return jsonwebtoken::decode::<StaffClaims>(
                    token,
                    &DecodingKey::from_secret(prev_secret.as_bytes()),
                    &Validation::default(),
                );
            }
        Err(jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken))
    })
    .map_err(|_| ())?;

    // Accept "cashier", "manager", "superadmin" and legacy "staff"
    if !VALID_ROLES.contains(&data.claims.role.as_str()) {
        return Err(());
    }

    // PACT-20260506-001 §AMEND-1.F + Captain §S-82 Q3 idle-session-timeout.
    // V2.0 ships fixed-window-from-iat (token re-issued at `iat`, expires
    // `iat + idle_timeout_secs`). Captain §S-82 Q3 originally specified
    // sliding-window; gap was disclosed as K5 in PR #64 DEPLOY MANIFEST §3
    // and dispositioned 2026-05-08 ~13:50 IST as Path A: ship fixed-window
    // for V2.0; sliding-window refresh on activity (token re-issuance) is
    // scope-pinned to V2.1 — see memory file
    // project_v2_1_sliding_window_idle_timeout_pact_pin.md.
    let idle_timeout = state.config.auth.idle_timeout_secs;
    if is_idle_expired(&data.claims, idle_timeout) {
        return Err(());
    }

    Ok(data.claims)
}

/// Returns `true` when the token has aged beyond the configured idle window.
///
/// Reads the wall clock once via `chrono::Utc::now()`. Tokens issued in the
/// future (clock skew) treat as not-expired (saturating subtraction).
///
/// Per Captain §S-82 Q3 disposition (2026-05-07 ~05:30 IST): default 30 min
/// sliding window via `idle_timeout_secs = 1800` in `[auth]` config.
pub(crate) fn is_idle_expired(claims: &StaffClaims, idle_timeout_secs: u64) -> bool {
    if idle_timeout_secs == 0 {
        return false;
    }
    let now = chrono::Utc::now().timestamp();
    let iat = claims.iat as i64;
    let elapsed = now.saturating_sub(iat);
    elapsed > idle_timeout_secs as i64
}

// ─── Strict middleware (rejects unauthenticated requests) ─────────────────

/// Axum middleware that enforces staff JWT authentication.
///
/// - Extracts `Authorization: Bearer {token}` header
/// - Decodes with `jsonwebtoken::decode::<StaffClaims>` using `config.auth.jwt_secret`
/// - On success: inserts `StaffClaims` into request extensions, calls next
/// - On failure: returns 401 UNAUTHORIZED
pub async fn require_staff_jwt(
    State(state): State<Arc<AppState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let (mut parts, body) = req.into_parts();

    // Try to build a temporary request for extraction
    let temp_req = Request::from_parts(parts.clone(), axum::body::Body::empty());
    match extract_staff_claims(&state, &temp_req) {
        Ok(claims) => {
            parts.extensions.insert(claims);
            let req = Request::from_parts(parts, body);
            Ok(next.run(req).await)
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

// SEC-FP-4: Permissive JWT middleware DELETED (was dead code, regression risk).
// All routes now use strict require_staff_jwt only.

// ─── RBAC role-checking middleware ────────────────────────────────────────

/// Middleware that enforces RBAC role gating AFTER require_staff_jwt has
/// already inserted StaffClaims into request extensions.
///
/// `allowed_roles` is a `&'static [&'static str]` of permitted roles, e.g.
/// `&["manager", "superadmin"]`.
///
/// Returns 403 with `{"error": "Insufficient permissions"}` if the claims
/// role is not in the allowed list.
///
/// Must be used AFTER `require_staff_jwt` in the middleware chain -- it
/// expects StaffClaims to be present in extensions.
pub async fn require_role(
    allowed_roles: &'static [&'static str],
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    match req.extensions().get::<StaffClaims>() {
        Some(claims) if claims.has_role(allowed_roles) => Ok(next.run(req).await),
        Some(claims) => {
            tracing::warn!(
                role = %claims.role,
                allowed = ?allowed_roles,
                path = %req.uri().path(),
                "RBAC: role insufficient"
            );
            Err(
                (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": "Insufficient permissions" })),
                )
                    .into_response(),
            )
        }
        None => {
            // No claims in extensions — require_staff_jwt wasn't in the chain
            tracing::error!(
                path = %req.uri().path(),
                "require_role called without require_staff_jwt in middleware chain"
            );
            Err(
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "Not authenticated" })),
                )
                    .into_response(),
            )
        }
    }
}

/// Axum middleware layer that requires manager+ role.
/// Use: `.layer(axum::middleware::from_fn(require_role_manager))`
pub async fn require_role_manager(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    require_role(&["manager", "superadmin"], req, next).await
}

/// Axum middleware layer that requires superadmin role.
/// Use: `.layer(axum::middleware::from_fn(require_role_superadmin))`
pub async fn require_role_superadmin(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, Response> {
    require_role(&["superadmin"], req, next).await
}

// ─── Create staff JWT ─────────────────────────────────────────────────────

/// Create a staff JWT token with the given staff_id and duration.
///
/// Issues "cashier" role (the minimum privilege level) for backward compatibility.
/// Use `create_staff_jwt_with_role` when the caller knows the specific role.
///
/// Returns the encoded JWT string on success.
pub fn create_staff_jwt(secret: &str, staff_id: &str, duration_hours: u64) -> Result<String, String> {
    create_staff_jwt_with_role(secret, staff_id, "cashier", duration_hours)
}

/// Create a staff JWT token with an explicit role.
///
/// Role must be one of: "cashier", "manager", "superadmin" (or legacy "staff").
pub fn create_staff_jwt_with_role(
    secret: &str,
    staff_id: &str,
    role: &str,
    duration_hours: u64,
) -> Result<String, String> {
    let now = chrono::Utc::now();
    let exp = now + chrono::Duration::hours(duration_hours as i64);

    let claims = StaffClaims {
        sub: staff_id.to_string(),
        role: role.to_string(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}


// ─── Pod JWT Claims (Phase 306) ──────────────────────────────────────────────

/// JWT claims for pod/agent WebSocket authentication.
///
/// Issued by the server after PSK bootstrap authentication (WSAUTH-01/04).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PodClaims {
    /// Canonical pod ID, e.g. "pod_3"
    pub pod_id: String,
    /// Pod number (1-based), e.g. 3
    pub pod_number: u32,
    /// Expiration (UNIX timestamp)
    pub exp: usize,
    /// Issued-at (UNIX timestamp)
    pub iat: usize,
}

/// Create a JWT for pod WebSocket authentication (Phase 306).
pub fn create_pod_jwt(
    secret: &str,
    pod_id: &str,
    pod_number: u32,
    duration_hours: u64,
) -> Result<String, String> {
    let now = chrono::Utc::now();
    let exp = now + chrono::Duration::hours(duration_hours as i64);
    let claims = PodClaims {
        pod_id: pod_id.to_string(),
        pod_number,
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };
    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("Pod JWT encode error: {}", e))
}

/// Decode and validate a pod JWT (Phase 306).
/// Tries current secret first, then previous secret (rotation grace).
pub fn decode_pod_jwt(
    token: &str,
    secret: &str,
    prev_secret: Option<&str>,
) -> Result<PodClaims, String> {
    let result = jsonwebtoken::decode::<PodClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    );
    match result {
        Ok(data) => Ok(data.claims),
        Err(primary_err) => {
            if let Some(prev) = prev_secret
                && !prev.is_empty() {
                    return jsonwebtoken::decode::<PodClaims>(
                        token,
                        &DecodingKey::from_secret(prev.as_bytes()),
                        &Validation::default(),
                    )
                    .map(|d| d.claims)
                    .map_err(|_| format!("Pod JWT decode failed (both secrets): {}", primary_err));
                }
            Err(format!("Pod JWT decode failed: {}", primary_err))
        }
    }
}

#[cfg(test)]
#[path = "middleware_tests.rs"]
mod middleware_tests;
