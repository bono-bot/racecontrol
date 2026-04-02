//! Phase 283: Billing Security — Replay Protection
//!
//! Provides:
//! - Session nonce generation and rotation (UUID v4)
//! - HMAC-SHA256 validation for billing mutation endpoints
//! - Immutable billing_audit_log table (append-only)
//! - Nonce store with 5-minute TTL cleanup

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::sync::RwLock;

type HmacSha256 = Hmac<Sha256>;

/// Entry in the nonce store: nonce value + creation time for TTL.
#[derive(Debug, Clone)]
struct NonceEntry {
    nonce: String,
    created_at: Instant,
}

/// In-memory nonce store keyed by session_id.
/// Each billing session has exactly one valid nonce at a time.
/// Nonces expire after 5 minutes (TTL_SECS).
pub struct NonceStore {
    entries: RwLock<HashMap<String, NonceEntry>>,
}

const TTL_SECS: u64 = 300; // 5 minutes

impl NonceStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// Generate a new nonce for a session, replacing any existing one.
    pub async fn generate(&self, session_id: &str) -> String {
        let nonce = uuid::Uuid::new_v4().to_string();
        let entry = NonceEntry {
            nonce: nonce.clone(),
            created_at: Instant::now(),
        };
        self.entries
            .write()
            .await
            .insert(session_id.to_string(), entry);
        nonce
    }

    /// Validate and consume a nonce for a session.
    /// Returns Ok(()) if valid, Err(reason) if invalid/expired/missing.
    /// On success, rotates to a new nonce (returned via the second element).
    pub async fn validate_and_rotate(&self, session_id: &str, provided_nonce: &str) -> Result<String, String> {
        let mut entries = self.entries.write().await;

        let entry = entries.get(session_id).ok_or_else(|| {
            format!("No nonce found for session {session_id}")
        })?;

        // Check TTL
        if entry.created_at.elapsed() > Duration::from_secs(TTL_SECS) {
            entries.remove(session_id);
            return Err(format!("Nonce expired for session {session_id}"));
        }

        // Check value
        if entry.nonce != provided_nonce {
            return Err(format!("Invalid nonce for session {session_id}"));
        }

        // Rotate: generate new nonce
        let new_nonce = uuid::Uuid::new_v4().to_string();
        entries.insert(
            session_id.to_string(),
            NonceEntry {
                nonce: new_nonce.clone(),
                created_at: Instant::now(),
            },
        );

        Ok(new_nonce)
    }

    /// Remove nonce when session ends.
    pub async fn remove(&self, session_id: &str) {
        self.entries.write().await.remove(session_id);
    }

    /// Cleanup expired entries. Called periodically.
    pub async fn cleanup_expired(&self) {
        let mut entries = self.entries.write().await;
        let ttl = Duration::from_secs(TTL_SECS);
        entries.retain(|_, entry| entry.created_at.elapsed() < ttl);
    }
}

/// Compute HMAC-SHA256 of the request body using the given secret and nonce.
/// The signed message is: `{nonce}:{body}`
pub fn compute_hmac(secret: &[u8], nonce: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret)
        .expect("HMAC can take key of any size");
    mac.update(nonce.as_bytes());
    mac.update(b":");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

/// Verify an HMAC signature (constant-time comparison via hmac crate).
pub fn verify_hmac(secret: &[u8], nonce: &str, body: &[u8], provided_hmac: &str) -> bool {
    let mut mac = HmacSha256::new_from_slice(secret)
        .expect("HMAC can take key of any size");
    mac.update(nonce.as_bytes());
    mac.update(b":");
    mac.update(body);

    match hex::decode(provided_hmac) {
        Ok(provided_bytes) => mac.verify_slice(&provided_bytes).is_ok(),
        Err(_) => false,
    }
}

/// Insert an immutable audit log entry for a billing state change.
pub async fn insert_audit_log(
    db: &sqlx::SqlitePool,
    session_id: &str,
    pod_id: &str,
    event_type: &str,
    old_status: &str,
    new_status: &str,
    nonce_used: Option<&str>,
    actor: &str,
    venue_id: &str,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO billing_audit_log (id, session_id, pod_id, event_type, old_status, new_status, nonce_used, timestamp, actor, venue_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'), ?, ?)",
    )
    .bind(&id)
    .bind(session_id)
    .bind(pod_id)
    .bind(event_type)
    .bind(old_status)
    .bind(new_status)
    .bind(nonce_used)
    .bind(actor)
    .bind(venue_id)
    .execute(db)
    .await;

    if let Err(e) = result {
        tracing::warn!(
            session_id = %session_id,
            event_type = %event_type,
            "Failed to insert billing audit log: {}",
            e
        );
    }
}

/// Spawn a background task that cleans up expired nonces every 60 seconds.
pub fn spawn_nonce_cleanup(nonce_store: Arc<NonceStore>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            nonce_store.cleanup_expired().await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_compute_and_verify() {
        let secret = b"test-secret-key-for-billing";
        let nonce = "550e8400-e29b-41d4-a716-446655440000";
        let body = b"{\"session_id\":\"sess_123\"}";

        let hmac_hex = compute_hmac(secret, nonce, body);
        assert!(!hmac_hex.is_empty());
        assert!(verify_hmac(secret, nonce, body, &hmac_hex));

        // Wrong nonce should fail
        assert!(!verify_hmac(secret, "wrong-nonce", body, &hmac_hex));
        // Wrong body should fail
        assert!(!verify_hmac(secret, nonce, b"wrong body", &hmac_hex));
        // Wrong secret should fail
        assert!(!verify_hmac(b"wrong-secret", nonce, body, &hmac_hex));
        // Invalid hex should fail
        assert!(!verify_hmac(secret, nonce, body, "not-hex-zzzz"));
    }

    #[tokio::test]
    async fn test_nonce_generate_and_validate() {
        let store = NonceStore::new();
        let nonce = store.generate("sess_1").await;
        assert!(!nonce.is_empty());

        // Valid nonce returns new nonce
        let new_nonce = store.validate_and_rotate("sess_1", &nonce).await;
        assert!(new_nonce.is_ok());
        let new_nonce = new_nonce.expect("should succeed");
        assert_ne!(new_nonce, nonce); // rotated

        // Old nonce should now fail
        let result = store.validate_and_rotate("sess_1", &nonce).await;
        assert!(result.is_err());

        // New nonce should work
        let result = store.validate_and_rotate("sess_1", &new_nonce).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_nonce_missing_session() {
        let store = NonceStore::new();
        let result = store.validate_and_rotate("nonexistent", "some-nonce").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nonce_remove() {
        let store = NonceStore::new();
        let nonce = store.generate("sess_2").await;
        store.remove("sess_2").await;
        let result = store.validate_and_rotate("sess_2", &nonce).await;
        assert!(result.is_err());
    }

    // ── Phase 285: Security replay protection integration test ────────────

    #[tokio::test]
    async fn test_replay_protection_e2e() {
        let store = NonceStore::new();
        let secret = b"billing-session-secret-key";
        let session = "sess_replay_test";
        let body = b"{\"action\":\"end_early\",\"session_id\":\"sess_replay_test\"}";

        // Step 1: Generate nonce for session
        let nonce1 = store.generate(session).await;

        // Step 2: Compute valid HMAC and verify it passes
        let hmac1 = compute_hmac(secret, &nonce1, body);
        assert!(verify_hmac(secret, &nonce1, body, &hmac1), "Valid HMAC must pass");

        // Step 3: Use the nonce (validate_and_rotate) — should succeed and rotate
        let nonce2 = store.validate_and_rotate(session, &nonce1).await
            .expect("First use of nonce must succeed");
        assert_ne!(nonce1, nonce2, "Nonce must rotate after use");

        // Step 4: REPLAY ATTACK — reuse old nonce must fail
        let replay = store.validate_and_rotate(session, &nonce1).await;
        assert!(replay.is_err(), "Replayed nonce must be rejected");

        // Step 5: Old HMAC with old nonce must not verify against new nonce
        assert!(!verify_hmac(secret, &nonce2, body, &hmac1),
            "HMAC computed with old nonce must not verify with new nonce");

        // Step 6: Invalid HMAC must be rejected
        assert!(!verify_hmac(secret, &nonce2, body, "deadbeef00112233"),
            "Invalid HMAC must be rejected");
        assert!(!verify_hmac(secret, &nonce2, body, "not-even-hex"),
            "Non-hex HMAC must be rejected");

        // Step 7: New nonce works for next mutation
        let hmac2 = compute_hmac(secret, &nonce2, body);
        assert!(verify_hmac(secret, &nonce2, body, &hmac2), "New HMAC with rotated nonce must pass");
        let nonce3 = store.validate_and_rotate(session, &nonce2).await
            .expect("Rotated nonce must work");
        assert_ne!(nonce2, nonce3, "Nonce must rotate again after second use");
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let store = NonceStore::new();
        // Manually insert an expired entry
        {
            let mut entries = store.entries.write().await;
            entries.insert(
                "old_session".to_string(),
                NonceEntry {
                    nonce: "old-nonce".to_string(),
                    created_at: Instant::now() - Duration::from_secs(TTL_SECS + 10),
                },
            );
        }
        store.cleanup_expired().await;
        let entries = store.entries.read().await;
        assert!(!entries.contains_key("old_session"));
    }
}
