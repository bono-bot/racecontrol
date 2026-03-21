use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ---- Request / Response types ------------------------------------------------

#[derive(Debug, Deserialize)]
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
        tracing::warn!("admin_login: invalid PIN attempt");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Generate 12-hour staff JWT
    let secret = &state.config.auth.jwt_secret;
    match super::middleware::create_staff_jwt(secret, "admin", 12) {
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
        assert_eq!(data.claims.role, "staff");
    }

    #[tokio::test]
    async fn admin_login_no_hash_configured_returns_503() {
        let state = test_state_with_hash(None).await;
        let app = test_router(state);
        let resp = app.oneshot(login_request("1234")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
