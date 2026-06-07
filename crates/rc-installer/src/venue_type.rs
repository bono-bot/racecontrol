//! Venue ownership axis (Captain 2026-06-07 direction; RCA + MMA-DIAGNOSE
//! `.planning/specs/v2/rc-installer/`).
//!
//! Orthogonal to [`crate::profile::Profile`] (Server/Pod): `VenueType` selects
//! the pod **remote-access trust model**, not the install role.
//!
//! - `Own`  — Racing Point eSports venue (has a Tailscale tailnet). The installer
//!   PROVISIONS key-only SSH over Tailscale (control-node key), as a complement to
//!   the audited rc-sentry / heart exec channels.
//! - `Sold` — third-party venue (no Tailscale). The installer REMOVES OpenSSH
//!   (today's hardening); rc-sentry exec only. This is the **default** — opening
//!   SSH must be an explicit opt-in (MMA-DIAGNOSE M3 / RC#4: a mis-set flag must
//!   never plant an HQ key on a third party's machine).
//!
//! The binary stays identical across venues (`config.rs` invariant): venue type
//! is resolved at install time from a CLI flag + an on-disk **immutability marker**,
//! never a per-venue compile-time constant.

/// Pod remote-access trust model. Default = [`VenueType::Sold`] (fail-safe).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VenueType {
    /// Racing Point eSports (Tailscale) — provision key-only SSH.
    Own,
    /// Third-party — remove OpenSSH (default).
    #[default]
    Sold,
}

impl VenueType {
    /// Stable lowercase label (also the marker-file content).
    pub fn as_str(self) -> &'static str {
        match self {
            VenueType::Own => "own",
            VenueType::Sold => "sold",
        }
    }

    /// Parse a CLI / marker value. Unknown / empty → `None` (caller decides the
    /// default; we never silently coerce an unknown string to `Own`).
    pub fn parse(s: &str) -> Option<VenueType> {
        match s.trim().to_ascii_lowercase().as_str() {
            "own" => Some(VenueType::Own),
            "sold" => Some(VenueType::Sold),
            _ => None,
        }
    }
}

/// Outcome of reconciling a requested venue type against an existing immutability
/// marker (MMA-DIAGNOSE M3 — kills RC#4 backdoor-on-sold AND old-installer
/// downgrade-strip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VenueDecision {
    /// The venue type the installer MUST act on.
    pub effective: VenueType,
    /// `true` when an existing marker pinned the type (the request could not change it).
    pub locked: bool,
    /// `true` when a marker existed AND the request disagreed with it — the caller
    /// should refuse silently-destructive behavior (e.g. an old installer trying to
    /// strip SSH off an `own` pod) and surface a loud warning.
    pub requested_mismatch: bool,
    /// `true` when no marker existed and the caller should WRITE one for `effective`.
    pub write_marker: bool,
}

/// Resolve the venue type. **Marker wins** — once a pod is provisioned `own` (or
/// `sold`), a later install cannot flip it without an explicit out-of-band
/// override (not modeled here; deliberately requires human action).
///
/// - `marker`: current marker-file content if present (`None` = first install).
/// - `requested`: the CLI-requested type, or `None` if the flag was omitted
///   (omitted → [`VenueType::Sold`], the fail-safe default).
pub fn resolve(marker: Option<VenueType>, requested: Option<VenueType>) -> VenueDecision {
    match marker {
        Some(pinned) => VenueDecision {
            effective: pinned,
            locked: true,
            requested_mismatch: requested.is_some_and(|r| r != pinned),
            write_marker: false,
        },
        None => {
            let effective = requested.unwrap_or_default(); // Sold
            VenueDecision {
                effective,
                locked: false,
                requested_mismatch: false,
                write_marker: true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_sold_failsafe() {
        assert_eq!(VenueType::default(), VenueType::Sold);
        assert_eq!(VenueType::Sold.as_str(), "sold");
        assert_eq!(VenueType::Own.as_str(), "own");
    }

    #[test]
    fn parse_is_strict_never_coerces_unknown_to_own() {
        assert_eq!(VenueType::parse("own"), Some(VenueType::Own));
        assert_eq!(VenueType::parse(" SOLD "), Some(VenueType::Sold));
        // Anything unrecognized must NOT become Own (RC#4 fail-safe).
        for bad in ["", "yes", "1", "tailscale", "true", "Own-venue"] {
            assert_eq!(VenueType::parse(bad), None, "'{bad}' must not parse");
        }
    }

    #[test]
    fn first_install_no_marker_uses_request_and_writes_marker() {
        let d = resolve(None, Some(VenueType::Own));
        assert_eq!(d.effective, VenueType::Own);
        assert!(!d.locked && !d.requested_mismatch && d.write_marker);
    }

    #[test]
    fn first_install_omitted_flag_defaults_sold() {
        let d = resolve(None, None);
        assert_eq!(d.effective, VenueType::Sold);
        assert!(d.write_marker);
    }

    #[test]
    fn marker_wins_old_installer_cannot_strip_ssh_off_own_pod() {
        // Old/unaware installer re-run (requests Sold or omits) on an `own` pod.
        for req in [None, Some(VenueType::Sold)] {
            let d = resolve(Some(VenueType::Own), req);
            assert_eq!(d.effective, VenueType::Own, "marker must pin own");
            assert!(d.locked && !d.write_marker);
            assert_eq!(d.requested_mismatch, req == Some(VenueType::Sold));
        }
    }

    #[test]
    fn marker_wins_sold_cannot_be_silently_upgraded_to_own() {
        // A mis-set `--venue-type own` on a SOLD pod must NOT plant a key.
        let d = resolve(Some(VenueType::Sold), Some(VenueType::Own));
        assert_eq!(d.effective, VenueType::Sold);
        assert!(d.locked && d.requested_mismatch && !d.write_marker);
    }
}
