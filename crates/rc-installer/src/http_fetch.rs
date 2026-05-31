//! INC-3 transport — the concrete [`Fetcher`] backed by ureq v3.
//!
//! Feature-gated behind `redeem_client` (the same opt-in that pulls ureq): the
//! default `--lib` build stays pure, and all fetch/verify/stage LOGIC is tested
//! against a mock [`Fetcher`] without this. This module is just the thin glue
//! that turns a real HTTP GET into `(status, bytes)`; the trust gates live in
//! [`crate::manifest_fetcher`] / [`crate::artifact_stager`].

use std::time::Duration;

use crate::manifest_fetcher::Fetcher;

/// Per-GET timeout for manifest + artifact downloads.
const FETCH_TIMEOUT: Duration = Duration::from_secs(120);

/// A blocking ureq-backed [`Fetcher`] (rustls; clean gnu cross-compile).
pub struct UreqFetcher;

impl Fetcher for UreqFetcher {
    fn get(&self, url: &str) -> Result<(u16, Vec<u8>), String> {
        let mut resp = ureq::get(url)
            .config()
            .http_status_as_error(false)
            .timeout_global(Some(FETCH_TIMEOUT))
            .build()
            .call()
            .map_err(|e| e.to_string())?;
        let status = resp.status().as_u16();
        let bytes = resp
            .body_mut()
            .read_to_vec()
            .map_err(|e| e.to_string())?;
        Ok((status, bytes))
    }
}
