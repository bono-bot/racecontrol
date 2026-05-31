//! INC-2 — the HTTP transport for venue-identity redemption.
//!
//! Feature-gated behind `redeem_client` so the default `--lib` build stays pure
//! (no network/TLS deps). Blocking `ureq` v3 (rustls) — no async runtime — which
//! cross-compiles cleanly to `x86_64-pc-windows-gnu`. ALL response
//! interpretation lives in [`crate::identity::interpret_response`]; this module
//! only performs the round-trip, so the status/parse logic is unit-tested
//! without a live server (`tests/identity_contract.rs`) and the live path is a
//! single `#[ignore]` test exercised by hand at INC-9.

use std::time::Duration;

use crate::identity::{interpret_response, redeem_url, RedeemError, RedeemedIdentity};

/// Overall timeout for the single redeem POST.
const REDEEM_TIMEOUT: Duration = Duration::from_secs(15);

/// POST a one-time install token to `{base}/install/redeem` and return the
/// venue identity.
///
/// The token travels in the JSON **body** (never the query string / URL),
/// matching the route, so it can't leak via referrer/proxy/CDN logs;
/// `Cache-Control: no-store` is set for the same reason. We disable ureq's
/// status-as-error behaviour so EVERY response (2xx and 4xx alike) returns
/// `Ok`, then route status + body uniformly through [`interpret_response`] —
/// non-2xx → typed [`RedeemError`], malformed 2xx body → `Malformed`. A failure
/// with no HTTP status (DNS/connect/TLS/timeout) becomes [`RedeemError::Transport`].
pub fn redeem(base: &str, token: &str) -> Result<RedeemedIdentity, RedeemError> {
    let url = redeem_url(base);
    let body = serde_json::json!({ "token": token }).to_string();

    let mut response = ureq::post(&url)
        .config()
        .http_status_as_error(false)
        .timeout_global(Some(REDEEM_TIMEOUT))
        .build()
        .header("content-type", "application/json")
        .header("cache-control", "no-store")
        .send(body.as_str())
        .map_err(|e| RedeemError::Transport(e.to_string()))?;

    let status = response.status().as_u16();
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|e| RedeemError::Transport(e.to_string()))?;

    interpret_response(status, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live smoke against a real one-time token. Hand-run at INC-9:
    /// `RC_REDEEM_TOKEN=… cargo test -p rc-installer --features redeem_client -- --ignored`
    #[test]
    #[ignore = "requires a live control plane + a real one-time token (INC-9)"]
    fn live_redeem() {
        let base = std::env::var("RC_REDEEM_BASE")
            .unwrap_or_else(|_| crate::config::INSTALLER_TIER_1_BASE.to_string());
        let token = std::env::var("RC_REDEEM_TOKEN").expect("set RC_REDEEM_TOKEN");
        let id = redeem(&base, &token).expect("redeem succeeds");
        assert!(!id.tenant_id.is_empty());
    }
}
