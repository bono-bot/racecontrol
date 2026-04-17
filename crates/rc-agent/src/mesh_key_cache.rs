//! Mesh service-key cache for rc-agent.
//!
//! Phase 413 — Option Z. rc-agent fetches the mesh service key from the server
//! at boot + every 5 minutes (via rc_common::boot_resilience::spawn_periodic_refetch)
//! instead of reading it from a machine-local env var. Key lives only in the
//! server's racecontrol.toml; no per-pod HKLM provisioning.
//!
//! # Cache semantics
//!
//! - `Arc<RwLock<Option<String>>>` — shared, single-writer-many-readers.
//! - Server returns 200 + non-empty key → cache stores `Some(key)`.
//! - Server returns 200 + empty key → cache stores `None` (server has unconfigured key).
//! - Server returns 4xx/5xx or network error → cache UNCHANGED (last-known-good preserved).
//! - First-boot + server unreachable → cache stays `None`; consumers fall back to env or bail.
//!
//! The empty-response-overwrites-existing behavior is deliberate: an explicit
//! response from the server is authoritative. A network failure is not.
//!
//! # Observability (W5)
//!
//! A 403/FORBIDDEN from `/pods/mesh-service-key` is logged at `warn!` level so
//! that "silent 403 = pod IP removed from allowlist" surfaces in rc-agent.log.
//! Other non-2xx responses are logged at `debug!` level (transient, will retry).
//! Cache preservation behavior is identical across all 4xx/5xx — last-known-good
//! is kept via `error_for_status()?` propagating Err to `spawn_periodic_refetch`.
//!
//! # Consumers (rewired in Plan 04)
//!
//! - `ai_debugger::check_audit_known_issues` (Tier 0 mesh oracle)
//! - `remote_ops::require_service_key` (middleware for pod `/exec` endpoint)
//! - `ws_handler` csv_lap_fallback push

use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared, thread-safe cache holding the current mesh service key.
///
/// `None` = no key available (consumers bail to env fallback or no-key behavior).
/// `Some(key)` = current key, fetched from server within the last periodic interval.
pub type MeshKeyCache = Arc<RwLock<Option<String>>>;

/// Construct an empty cache. Caller typically stores this in `Arc::clone` form
/// and passes clones to consumers + the periodic_refetch spawn.
pub fn new_cache() -> MeshKeyCache {
    Arc::new(RwLock::new(None))
}

/// Fetch the mesh service key from the server and update the cache.
///
/// # Arguments
/// - `client`: A reqwest::Client (use a client with a reasonable timeout, e.g. 10s)
/// - `http_base`: Server HTTP base URL INCLUDING `/api/v1` (e.g. `http://192.168.31.23:8080/api/v1`)
/// - `cache`: The shared MeshKeyCache to update
///
/// # Return
/// - `Ok(())` on any 2xx response (including "empty key" response — cache is set to None)
/// - `Err(reqwest::Error)` on network failure or non-2xx HTTP status
///
/// # Cache-update rules
/// - 200 + non-empty key → cache = Some(key)
/// - 200 + empty key → cache = None (server unconfigured)
/// - Non-2xx or network error → cache UNCHANGED (last-known-good preserved)
pub async fn fetch_from_server(
    client: &reqwest::Client,
    http_base: &str,
    cache: &MeshKeyCache,
) -> Result<(), reqwest::Error> {
    let url = format!("{}/pods/mesh-service-key", http_base.trim_end_matches('/'));
    let resp = client.get(&url).send().await?;

    // W5 fix — log 403 at warn level to make "silent 403" observable.
    // A 403 from /pods/mesh-service-key means this pod's source IP is no longer
    // on the Pod allowlist (e.g., IP change, reclassification). Cache behavior
    // is unchanged (last-known-good preserved via error_for_status below), but
    // the distinct warn! line makes stale-key-after-rotation surfaceable in logs
    // instead of silently failing next time consumers try to authenticate.
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        // W5 emit: tracing::warn! on 403 / FORBIDDEN status — distinct from transient non-2xx.
        tracing::warn!(
            target: "mesh_key_cache",
            status = 403,
            url = %url,
            "Mesh key fetch rejected by server (403 FORBIDDEN) — pod IP may no longer be on the Pod allowlist. Last-known-good cache value preserved. Verify network_source.rs classification + pod IP. See Phase 413 CONTEXT.md."
        );
    } else if !resp.status().is_success() {
        // Other non-2xx — transient server errors, different log keyword so
        // observability can distinguish 403 (auth/allowlist) from 500/503 (transient).
        tracing::debug!(
            target: "mesh_key_cache",
            status = resp.status().as_u16(),
            url = %url,
            "Mesh key fetch returned non-2xx (not 403) — transient, will retry"
        );
    }

    // error_for_status() converts 4xx/5xx into Err. We MUST call this so the
    // caller's periodic_refetch logs "failed" and preserves last-known-good.
    let resp = resp.error_for_status()?;

    let body: serde_json::Value = resp.json().await?;
    let key_opt: Option<String> = body
        .get("mesh_service_key")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let new_value: Option<String> = match key_opt {
        Some(k) if !k.is_empty() => Some(k),
        _ => None, // empty string or missing field → None
    };

    let mut guard = cache.write().await;
    *guard = new_value;
    drop(guard);

    tracing::debug!(
        target: "mesh_key_cache",
        "fetch_from_server success (cache updated)"
    );
    Ok(())
}

/// Read the cache, or fall back to the legacy `RCAGENT_SERVICE_KEY` env var.
///
/// Returns `None` when BOTH cache and env are empty/unset.
/// Returns `Some(key)` when either source has a non-empty value (cache wins).
///
/// Production: cache is the source of truth (populated by periodic_refetch).
/// Tests: tests set `RCAGENT_SERVICE_KEY` env; they do not populate the cache.
/// The env fallback exists ONLY for test compatibility — production env is unset.
pub async fn get_key_or_env(cache: &MeshKeyCache) -> Option<String> {
    let cached = cache.read().await.clone();
    if let Some(k) = cached {
        if !k.is_empty() {
            return Some(k);
        }
    }
    match std::env::var("RCAGENT_SERVICE_KEY") {
        Ok(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .expect("reqwest client build failed — static config")
    }

    #[tokio::test]
    async fn fetch_populates_cache() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/pods/mesh-service-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mesh_service_key": "abc123"
            })))
            .mount(&server)
            .await;

        let cache = new_cache();
        let http_base = format!("{}/api/v1", server.uri());
        let res = fetch_from_server(&test_client(), &http_base, &cache).await;
        assert!(res.is_ok(), "fetch should succeed on 200");
        assert_eq!(*cache.read().await, Some("abc123".to_string()));
    }

    #[tokio::test]
    async fn fetch_preserves_last_known_good_on_500() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/pods/mesh-service-key"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let cache = new_cache();
        *cache.write().await = Some("old-key".to_string());
        let http_base = format!("{}/api/v1", server.uri());
        let res = fetch_from_server(&test_client(), &http_base, &cache).await;
        assert!(res.is_err(), "fetch should return Err on 500");
        assert_eq!(
            *cache.read().await,
            Some("old-key".to_string()),
            "cache must preserve last-known-good on 500"
        );
    }

    #[tokio::test]
    async fn fetch_403_logs_warn_and_preserves_cache() {
        // W5 — 403 from the server means the pod IP is no longer on the Pod allowlist.
        // Cache behavior is identical to any other 4xx (last-known-good preserved via
        // error_for_status), but the tracing::warn! line distinguishes this path for
        // observability. This test asserts the cache preservation; log-line assertion
        // requires a tracing-test subscriber and is covered in the integration test
        // in Plan 10 (grep for the warn line in rc-agent.log after a deliberate 403).
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/pods/mesh-service-key"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let cache = new_cache();
        *cache.write().await = Some("old-key".to_string());
        let http_base = format!("{}/api/v1", server.uri());
        let res = fetch_from_server(&test_client(), &http_base, &cache).await;
        assert!(res.is_err(), "fetch should return Err on 403");
        assert_eq!(
            *cache.read().await,
            Some("old-key".to_string()),
            "cache must preserve last-known-good on 403 (same as any 4xx)"
        );
    }

    #[tokio::test]
    async fn fetch_preserves_last_known_good_on_network_failure() {
        // Unreachable port — no mock server
        let cache = new_cache();
        *cache.write().await = Some("old-key".to_string());
        let http_base = "http://127.0.0.1:1/api/v1"; // port 1 is nearly always closed
        let res = fetch_from_server(&test_client(), http_base, &cache).await;
        assert!(res.is_err(), "fetch should return Err on connection refused");
        assert_eq!(
            *cache.read().await,
            Some("old-key".to_string()),
            "cache must preserve last-known-good on network error"
        );
    }

    #[tokio::test]
    async fn fetch_empty_response_sets_cache_to_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/pods/mesh-service-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mesh_service_key": ""
            })))
            .mount(&server)
            .await;

        let cache = new_cache();
        let http_base = format!("{}/api/v1", server.uri());
        let _ = fetch_from_server(&test_client(), &http_base, &cache).await;
        assert_eq!(*cache.read().await, None, "empty key → None");
    }

    #[tokio::test]
    async fn fetch_empty_response_overwrites_existing_key() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/pods/mesh-service-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "mesh_service_key": ""
            })))
            .mount(&server)
            .await;

        let cache = new_cache();
        *cache.write().await = Some("rotated-out".to_string());
        let http_base = format!("{}/api/v1", server.uri());
        let _ = fetch_from_server(&test_client(), &http_base, &cache).await;
        assert_eq!(
            *cache.read().await,
            None,
            "explicit empty response from server must overwrite stale key"
        );
    }

    // get_key_or_env MUST run with #[serial] to avoid env-var race across parallel tests.

    #[tokio::test]
    #[serial_test::serial]
    async fn get_key_or_env_prefers_cache() {
        unsafe {
            std::env::set_var("RCAGENT_SERVICE_KEY", "from-env");
        }
        let cache = new_cache();
        *cache.write().await = Some("from-cache".to_string());
        let got = get_key_or_env(&cache).await;
        unsafe {
            std::env::remove_var("RCAGENT_SERVICE_KEY");
        }
        assert_eq!(got, Some("from-cache".to_string()));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn get_key_or_env_falls_back_to_env_when_cache_none() {
        unsafe {
            std::env::set_var("RCAGENT_SERVICE_KEY", "from-env");
        }
        let cache = new_cache();
        let got = get_key_or_env(&cache).await;
        unsafe {
            std::env::remove_var("RCAGENT_SERVICE_KEY");
        }
        assert_eq!(got, Some("from-env".to_string()));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn get_key_or_env_returns_none_when_both_empty() {
        unsafe {
            std::env::remove_var("RCAGENT_SERVICE_KEY");
        }
        let cache = new_cache();
        let got = get_key_or_env(&cache).await;
        assert_eq!(got, None);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn get_key_or_env_returns_none_when_cache_empty_string() {
        unsafe {
            std::env::remove_var("RCAGENT_SERVICE_KEY");
        }
        let cache = new_cache();
        *cache.write().await = Some("".to_string());
        let got = get_key_or_env(&cache).await;
        assert_eq!(got, None);
    }
}
