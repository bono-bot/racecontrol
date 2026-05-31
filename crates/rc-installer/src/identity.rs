//! Venue-identity redemption contract (INC-1).
//!
//! The trust core ([`crate::manifest`]) answers WHAT software to install; this
//! module answers WHO/WHERE — the per-venue identity the installer learns by
//! redeeming a one-time install token against the control plane.
//!
//! Canonical source of truth for the wire shape:
//! `rp-v2-apps/apps/racecontrol-console/app/install/redeem/route.ts`
//! (`POST /install/redeem`, token in the BODY). This module mirrors that
//! response **field-for-field**; it is pure (no I/O) so it unit-tests on Linux
//! and cross-compiles to `x86_64-pc-windows-gnu`. The HTTP transport that calls
//! it is a separate, feature-gated increment ([`crate::redeem_client`], INC-2).
//!
//! N-altitude guardrails (handoff R3): `venue_id` is `Option` and a missing
//! value is a HARD ERROR ([`RedeemedIdentity::require_venue_id`]) — NEVER
//! defaulted to a baked venue. `tenant_id`/`venue_id` are opaque strings: never
//! parsed, ranged, or assumed to be `[1-8]`.

use serde::Deserialize;
use thiserror::Error;

use crate::config::INSTALLER_TIER_1_BASE;
use crate::profile::Profile;

/// The venue identity returned by a successful `/install/redeem`.
///
/// Mirrors the route's 200 body field-for-field. `venue_id`/`server_address`/
/// `bono_address` are `Option` because the route returns them as `null` when the
/// tenant row has no mapping yet; `tenant_id`, `profile`, and `racecontrol_toml`
/// are always present on a 200.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedeemedIdentity {
    pub tenant_id: String,
    pub venue_id: Option<String>,
    pub profile: Profile,
    pub server_address: Option<String>,
    pub bono_address: Option<String>,
    pub racecontrol_toml: String,
}

impl RedeemedIdentity {
    /// `venue_id` is required at provisioning time; a missing/empty value is a
    /// hard error, NEVER defaulted to a baked venue (handoff R3, INC-5 guardrail).
    pub fn require_venue_id(&self) -> Result<&str, RedeemError> {
        self.venue_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or(RedeemError::VenueIdMissing)
    }
}

/// A redemption failure — the route's status codes plus response-shape errors.
///
/// `PartialEq`/`Debug` let the integration tests `assert_eq!` exact variants,
/// matching [`crate::error::VerifyError`]'s convention.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RedeemError {
    /// 404 — the token is not a known install token.
    #[error("install token is invalid (404)")]
    InvalidToken,

    /// 410 — the token expired or was already consumed (single-use).
    #[error("install token is expired or already consumed (410)")]
    ExpiredOrConsumed,

    /// 429 — too many redeem attempts; retry after the route's backoff.
    #[error("redeem is rate-limited (429); retry later")]
    RateLimited,

    /// 400 — the request or response body was malformed; carries a short detail.
    #[error("redeem request/response malformed (400): {0}")]
    Malformed(String),

    /// Any other HTTP status the route is not contracted to return.
    #[error("redeem failed with unexpected status {0}")]
    UnexpectedStatus(u16),

    /// A 200 body parsed as JSON but a required field was absent/empty.
    #[error("redeem response is missing required field '{0}'")]
    MissingField(&'static str),

    /// A 200 body carried a `profile` outside the `server`/`pod` contract.
    #[error("redeem response carried an unknown profile '{0}' (expected 'server' or 'pod')")]
    UnknownProfile(String),

    /// `venue_id` was required by the caller but the response did not include one.
    #[error("venue_id is required but the redeem response did not include one")]
    VenueIdMissing,

    /// The request never produced an HTTP status (DNS / connect / TLS / timeout).
    #[error("redeem transport error: {0}")]
    Transport(String),
}

impl RedeemError {
    /// Map a non-2xx HTTP status from the redeem route to a typed error.
    /// `body` supplies the detail for the 400 (`Malformed`) case only.
    pub fn from_status(status: u16, body: &str) -> RedeemError {
        match status {
            404 => RedeemError::InvalidToken,
            410 => RedeemError::ExpiredOrConsumed,
            429 => RedeemError::RateLimited,
            400 => RedeemError::Malformed(snippet(body)),
            other => RedeemError::UnexpectedStatus(other),
        }
    }
}

/// Intermediate deserialization target. Every field is `Option` so we can emit a
/// precise [`RedeemError::MissingField`] rather than a generic serde message, and
/// unknown fields are tolerated (forward-compat: the route may add fields).
#[derive(Deserialize)]
struct RawRedeem {
    tenant_id: Option<String>,
    venue_id: Option<String>,
    profile: Option<String>,
    server_address: Option<String>,
    bono_address: Option<String>,
    racecontrol_toml: Option<String>,
}

/// Parse a 200 `/install/redeem` body into a [`RedeemedIdentity`].
///
/// `null`/absent `venue_id`/`server_address`/`bono_address` become `None`
/// (the type keeps the Option; the hard-error on a required venue_id is deferred
/// to [`RedeemedIdentity::require_venue_id`]). A JSON syntax error maps to
/// [`RedeemError::Malformed`]; an absent required field to
/// [`RedeemError::MissingField`]; an off-contract profile to
/// [`RedeemError::UnknownProfile`].
pub fn parse_redeem_response(body: &str) -> Result<RedeemedIdentity, RedeemError> {
    let raw: RawRedeem =
        serde_json::from_str(body).map_err(|e| RedeemError::Malformed(e.to_string()))?;

    let tenant_id = non_empty(raw.tenant_id).ok_or(RedeemError::MissingField("tenant_id"))?;
    let profile_label = non_empty(raw.profile).ok_or(RedeemError::MissingField("profile"))?;
    let profile = parse_profile(&profile_label)?;
    // racecontrol_toml may legitimately be empty in some shapes, but the route
    // always emits a non-empty fragment; treat absent (not empty) as missing.
    let racecontrol_toml = raw
        .racecontrol_toml
        .ok_or(RedeemError::MissingField("racecontrol_toml"))?;

    Ok(RedeemedIdentity {
        tenant_id,
        venue_id: non_empty(raw.venue_id),
        profile,
        server_address: non_empty(raw.server_address),
        bono_address: non_empty(raw.bono_address),
        racecontrol_toml,
    })
}

/// Interpret a redeem HTTP result: a 2xx body → [`parse_redeem_response`]; any
/// other status → [`RedeemError::from_status`]. Pure (no I/O), so the
/// status→error and body→identity mapping is unit-tested without a live server;
/// the actual HTTP round-trip lives in [`crate::redeem_client`] (feature
/// `redeem_client`) and delegates here.
pub fn interpret_response(status: u16, body: &str) -> Result<RedeemedIdentity, RedeemError> {
    if (200..300).contains(&status) {
        parse_redeem_response(body)
    } else {
        Err(RedeemError::from_status(status, body))
    }
}

/// The redeem endpoint URL under a given control-plane base. Always a child of
/// `{base}/install/` so it can never leak onto a third-party origin.
pub fn redeem_url(base: &str) -> String {
    format!("{}/install/redeem", base.trim_end_matches('/'))
}

/// [`redeem_url`] bound to the single baked tier-1 base ([`INSTALLER_TIER_1_BASE`]).
pub fn default_redeem_url() -> String {
    redeem_url(INSTALLER_TIER_1_BASE)
}

/// Map the route's lowercase `profile` label to [`Profile`]. Mirrors
/// [`Profile::as_str`] exactly (server/pod only).
fn parse_profile(label: &str) -> Result<Profile, RedeemError> {
    match label.trim() {
        "server" => Ok(Profile::Server),
        "pod" => Ok(Profile::Pod),
        other => Err(RedeemError::UnknownProfile(other.to_string())),
    }
}

/// `Some(trimmed)` if present and non-blank, else `None`.
fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// First 200 chars of an error body, for a bounded `Malformed` detail.
fn snippet(body: &str) -> String {
    body.chars().take(200).collect()
}
