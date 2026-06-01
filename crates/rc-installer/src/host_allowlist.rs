//! Pure host-policy decision for the redeemed `server_address` / `bono_address` (INC-2H, MMA G5).
//!
//! Defends against a tampered/poisoned redeem response that points the pod at a hostile
//! host. The decision is pure and fully unit-tested; INC-5/6 wires a 2-line call in front
//! of any connection to `server_address` (no premature coupling here).
//!
//! Scope: these policies target `server_address` (the pod -> venue-heart host on the LAN).
//! `bono_address` (the cloud control-plane, e.g. a `*.hstgr.cloud` VPS) is fail-closed
//! under both policies today; its allowlist is an INC-5 wire-time decision (add a dedicated
//! policy/suffix then — see tests/host_allowlist.rs).

use crate::error::{InstallError, Result};
use std::net::Ipv4Addr;
use std::str::FromStr;

/// Allowlist policy for a server host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPolicy {
    /// Pilot default: a private-LAN (RFC1918/loopback) literal OR a racingpoint/racecontrol suffix.
    LanOrRacingpoint,
    /// Strict (VLAN-isolated pilot): only RFC1918/loopback literals, no DNS names.
    LanOnly,
}

/// Allowed DNS suffixes (apex or `.suffix`) under `LanOrRacingpoint`.
const ALLOWED_SUFFIXES: [&str; 3] = ["racingpoint.in", "racingpoint.cloud", "racecontrol.in"];

/// Extract the bare host from `host`, `host:port`, or a full URL. Lowercased, ASCII-only.
pub fn extract_host(addr: &str) -> Result<String> {
    let s = addr.trim();
    if s.is_empty() {
        return Err(InstallError::HostNotAllowed("<empty>".into()));
    }
    // strip scheme
    let after_scheme = match s.find("://") {
        Some(i) => &s[i + 3..],
        None => s,
    };
    // strip path / query / fragment
    let host_port = after_scheme.split(['/', '?', '#']).next().unwrap_or("");
    // strip :port (only when the tail is all digits — leaves bare IPv4/host intact)
    let host = match host_port.rsplit_once(':') {
        Some((h, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => h,
        _ => host_port,
    };
    if host.is_empty() {
        return Err(InstallError::HostNotAllowed(addr.to_string()));
    }
    if !host.is_ascii() {
        return Err(InstallError::HostNotAllowed(format!("non-ascii host: {addr}")));
    }
    // A clean hostname or bare IPv4 has no ':' left after port-stripping. A residual ':'
    // means a malformed authority (e.g. a non-numeric pseudo-port `evil.com:.racingpoint.in`
    // that the digit-only port guard refused to strip) OR a bracket-less IPv6 — reject both,
    // so the suffix check can never run against a string carrying a smuggled colon segment.
    if host.contains(':') {
        return Err(InstallError::HostNotAllowed(format!(
            "host has an unexpected ':' (malformed authority): {addr}"
        )));
    }
    Ok(host.to_ascii_lowercase())
}

fn is_rfc1918_or_loopback(host: &str) -> bool {
    match Ipv4Addr::from_str(host) {
        Ok(ip) => ip.is_private() || ip.is_loopback(),
        Err(_) => false,
    }
}

fn suffix_allowed(host: &str) -> bool {
    ALLOWED_SUFFIXES
        .iter()
        .any(|suf| host == *suf || host.ends_with(&format!(".{suf}")))
}

/// Validate a redeemed server/bono address against `policy`. Ok => safe to connect.
pub fn validate_server_address(addr: &str, policy: HostPolicy) -> Result<()> {
    let host = extract_host(addr)?;
    let ok = match policy {
        HostPolicy::LanOnly => is_rfc1918_or_loopback(&host),
        HostPolicy::LanOrRacingpoint => is_rfc1918_or_loopback(&host) || suffix_allowed(&host),
    };
    if ok {
        Ok(())
    } else {
        Err(InstallError::HostNotAllowed(host))
    }
}
