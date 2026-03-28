use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ---- Admin login lockout ----------------------------------------------------

/// Tracks failed admin login attempts. After MAX_FAILED_ATTEMPTS failures within
/// the LOCKOUT_WINDOW, the source is locked out for LOCKOUT_DURATION.
const MAX_FAILED_ATTEMPTS: usize = 5;
const LOCKOUT_WINDOW_SECS: u64 = 300;  // 5 minutes
const LOCKOUT_DURATION_SECS: u64 = 900; // 15 minutes

struct LockoutState {
    attempts: Vec<std::time::Instant>,
    locked_until: Option<std::time::Instant>,
}

static ADMIN_LOCKOUT: std::sync::LazyLock<std::sync::Mutex<LockoutState>> =
    std::sync::LazyLock::new(|| {
        std::sync::Mutex::new(LockoutState {
            attempts: Vec::new(),
            locked_until: None,
        })
    });

fn check_lockout() -> Result<(), std::time::Duration> {
    let mut state = ADMIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();

    // Check if currently locked out
    if let Some(until) = state.locked_until {
        if now < until {
            return Err(until - now);
        }
        // Lockout expired — clear
        state.locked_until = None;
        state.attempts.clear();
    }
    Ok(())
}

fn record_failed_attempt() {
    let mut state = ADMIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
    let now = std::time::Instant::now();
    let window = std::time::Duration::from_secs(LOCKOUT_WINDOW_SECS);

    // Prune old attempts outside the window
    state.attempts.retain(|t| now.duration_since(*t) < window);
    state.attempts.push(now);

    if state.attempts.len() >= MAX_FAILED_ATTEMPTS {
        state.locked_until = Some(now + std::time::Duration::from_secs(LOCKOUT_DURATION_SECS));
        state.attempts.clear();
        tracing::warn!(
            "admin_login: LOCKOUT activated — {} failed attempts in {}s, locked for {}s",
            MAX_FAILED_ATTEMPTS, LOCKOUT_WINDOW_SECS, LOCKOUT_DURATION_SECS
        );
    }
}

fn clear_lockout_on_success() {
    if let Ok(mut state) = ADMIN_LOCKOUT.lock() {
        state.attempts.clear();
        state.locked_until = None;
    }
}

// ---- Request / Response types ------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdminLoginRequest {
    pub pin: String,
}

#[derive(Debug, Serialize)]
pub struct AdminLoginResponse {
    pub token: String,
    pub expires_in: u64,
}

// ---- Argon2 helpers ----------------------------------------------------------

/// Hash an admin PIN using Argon2id with a random salt.
/// Returns the PHC-format hash string (starts with "$argon2id$").
pub fn hash_admin_pin(pin: &str) -> Result<String, String> {
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        Argon2, PasswordHasher,
    };
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(pin.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Argon2 hash error: {}", e))
}

/// Verify a PIN against an argon2 hash string.
/// Returns `true` if the PIN matches the hash, `false` otherwise (including on parse errors).
pub fn verify_admin_pin(pin: &str, hash_str: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let Ok(parsed) = PasswordHash::new(hash_str) else {
        return false;
    };
    Argon2::default()
        .verify_password(pin.as_bytes(), &parsed)
        .is_ok()
}

// ---- Admin login handler -----------------------------------------------------

/// POST /api/v1/auth/admin-login
///
/// Validates the admin PIN against the stored argon2id hash.
/// On success, returns a staff JWT with 12-hour expiry.
///
/// - 503 if no admin_pin_hash is configured
/// - 401 if PIN is wrong
/// - 200 + { token, expires_in } on success
pub async fn admin_login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AdminLoginRequest>,
) -> impl IntoResponse {
    // Check lockout before processing
    if let Err(remaining) = check_lockout() {
        tracing::warn!("admin_login: locked out for {}s more", remaining.as_secs());
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Check if admin_pin_hash is configured
    let hash = match &state.config.auth.admin_pin_hash {
        Some(h) => h.clone(),
        None => {
            tracing::warn!("admin_login called but no admin_pin_hash configured");
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };

    let pin = body.pin.clone();

    // Run argon2 verification on spawn_blocking (CPU-heavy, must not block tokio)
    let valid = tokio::task::spawn_blocking(move || verify_admin_pin(&pin, &hash))
        .await
        .unwrap_or(false);

    if !valid {
        record_failed_attempt();
        tracing::warn!("admin_login: invalid PIN attempt");
        return Err(StatusCode::UNAUTHORIZED);
    }
    clear_lockout_on_success();

    // Generate 12-hour staff JWT with superadmin role (admin PIN = full access)
    let secret = &state.config.auth.jwt_secret;
    match super::middleware::create_staff_jwt_with_role(secret, "admin", "superadmin", 12) {
        Ok(token) => {
            // Audit trail + WhatsApp alert for admin login
            crate::accounting::log_admin_action(
                &state, "admin_login", "Admin login successful", None, None,
            ).await;
            crate::whatsapp_alerter::send_admin_alert(
                &state.config, "Admin Login", "Successful admin login",
            ).await;

            Ok(Json(AdminLoginResponse {
                token,
                expires_in: 43200, // 12 hours in seconds
            }))
        }
        Err(e) => {
            tracing::error!("admin_login: JWT creation failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use tower::ServiceExt;

    const TEST_SECRET: &str = "test-secret-key-for-admin-tests";

    /// Build a minimal AppState with a known admin_pin_hash for testing.
    async fn test_state_with_hash(hash: Option<String>) -> Arc<AppState> {
        let mut config = crate::config::Config::default_test();
        config.auth.jwt_secret = TEST_SECRET.to_string();
        config.auth.admin_pin_hash = hash;

        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite pool");

        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, pool, field_cipher))
    }

    fn test_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/admin-login", post(admin_login))
            .with_state(state)
    }

    fn login_request(pin: &str) -> Request<Body> {
        Request::builder()
            .uri("/admin-login")
            .method("POST")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"pin":"{}"}}"#, pin)))
            .unwrap()
    }

    // ---- hash / verify unit tests ----

    #[test]
    fn hash_admin_pin_produces_argon2id_hash() {
        let hash = hash_admin_pin("1234").unwrap();
        assert!(hash.starts_with("$argon2id$"), "Expected argon2id hash, got: {}", hash);
    }

    #[test]
    fn verify_admin_pin_correct_pin_returns_true() {
        let hash = hash_admin_pin("1234").unwrap();
        assert!(verify_admin_pin("1234", &hash));
    }

    #[test]
    fn verify_admin_pin_wrong_pin_returns_false() {
        let hash = hash_admin_pin("1234").unwrap();
        assert!(!verify_admin_pin("9999", &hash));
    }

    #[test]
    fn verify_admin_pin_invalid_hash_returns_false() {
        assert!(!verify_admin_pin("1234", "invalid-hash"));
    }

    #[test]
    fn verify_261121_against_stored_hash() {
        let hash = "$argon2id$v=19$m=19456,t=2,p=1$7HM4TtJrDU5lnhCTMIiusQ$3xT9d98mexHIx+4yWJwzDEfdls72XY+wEJVmR9aHFkU";
        let result = verify_admin_pin("261121", hash);
        println!("verify_admin_pin(261121) = {}", result);
        assert!(result, "PIN 261121 should match the stored hash");
    }

    #[test]
    fn hash_admin_pin_produces_different_hashes_random_salt() {
        let h1 = hash_admin_pin("1234").unwrap();
        let h2 = hash_admin_pin("1234").unwrap();
        assert_ne!(h1, h2, "Two hashes of same PIN should differ (random salt)");
    }

    // ---- handler tests ----

    #[tokio::test]
    async fn admin_login_wrong_pin_returns_401() {
        let hash = hash_admin_pin("1234").unwrap();
        let state = test_state_with_hash(Some(hash)).await;
        let app = test_router(state);
        let resp = app.oneshot(login_request("9999")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_login_correct_pin_returns_200_with_token() {
        let hash = hash_admin_pin("1234").unwrap();
        let state = test_state_with_hash(Some(hash)).await;
        let app = test_router(state);
        let resp = app.oneshot(login_request("1234")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1024 * 10).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("token").is_some(), "Response must have 'token' field");
        assert_eq!(json["expires_in"], 43200, "expires_in must be 43200 (12h)");

        // Verify the token is a valid staff JWT
        let token = json["token"].as_str().unwrap();
        let data = jsonwebtoken::decode::<super::super::middleware::StaffClaims>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(TEST_SECRET.as_bytes()),
            &jsonwebtoken::Validation::default(),
        )
        .expect("Token should be valid staff JWT");
        assert_eq!(data.claims.sub, "admin");
        assert_eq!(data.claims.role, "superadmin");
    }

    #[tokio::test]
    async fn admin_login_no_hash_configured_returns_503() {
        let state = test_state_with_hash(None).await;
        let app = test_router(state);
        let resp = app.oneshot(login_request("1234")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
