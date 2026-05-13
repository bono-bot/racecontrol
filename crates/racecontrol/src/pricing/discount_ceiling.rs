//! MAX_DISCOUNT_PCT ceiling primitive — V2 row 7.3 Phase 1 substrate.
//!
//! Captain Q-2-1 ratify (verbatim): `"max_discount_pct = 0.50"` — landed at
//! comms-link `c203135d` (§S-252). Source-of-truth RCA:
//! `racecontrol/.planning/audits/RCA-2026-05-13-row-7.3-max-discount-pct-ceiling.md`.
//!
//! Composes with `billing::DISCOUNT_FLOOR_PAISE` (FATM-10 floor) and
//! `billing::DISCOUNT_APPROVAL_THRESHOLD_PAISE` (STAFF-01 approval gate). The
//! ceiling clamps BEFORE the floor at call-sites — ceiling+floor are independent
//! pricing invariants.
//!
//! Future atom (NOT in Phase 1): a `discount_ceiling_pct` override field on
//! `AppState.config` so venues can lower the cap below 0.50 without a binary
//! redeploy. `max_discount_pct(&state)` is the seam for that override; today it
//! returns the const directly.

use crate::state::AppState;

/// Default MAX_DISCOUNT_PCT cap.
///
/// Captain Q-2-1 ratify verbatim 2026-05-13 IST: `"max_discount_pct = 0.50"` —
/// 50% maximum effective discount on any single billing session. Anchor:
/// comms-link §S-252 commit `c203135d`. RCA: `racecontrol/.planning/audits/
/// RCA-2026-05-13-row-7.3-max-discount-pct-ceiling.md` §4-§5.
pub const MAX_DISCOUNT_PCT_DEFAULT: f64 = 0.50;

/// Effective MAX_DISCOUNT_PCT cap for the current `AppState`.
///
/// Today returns `MAX_DISCOUNT_PCT_DEFAULT` directly — no override hook wired
/// yet. The signature accepts `&AppState` so a future atom can add a
/// `discount_ceiling_pct` config field without changing call-sites. Matches the
/// pattern of `billing::DISCOUNT_APPROVAL_THRESHOLD_PAISE` /
/// `billing::DISCOUNT_FLOOR_PAISE` which are also const today but routed through
/// `billing::` for the same forward-compat reason.
pub fn max_discount_pct(_state: &AppState) -> f64 {
    // FUTURE ATOM: when `AppState.config.discount_ceiling_pct: Option<f64>` lands,
    // read it here and fall back to MAX_DISCOUNT_PCT_DEFAULT when absent.
    MAX_DISCOUNT_PCT_DEFAULT
}

/// Result of a clamp operation against the MAX_DISCOUNT_PCT cap.
///
/// `Allowed` = requested discount is within cap, no clamping performed.
/// `Clamped` = discount exceeds cap (or the call was defensive-rejected, e.g.
/// `original_price_paise <= 0`); call-sites should log the original/clamped
/// pair and use the clamped value going forward.
#[derive(Debug, Clone, PartialEq)]
pub enum ClampResult {
    Allowed {
        pct: f64,
        paise: Option<i64>,
    },
    Clamped {
        original_pct: f64,
        clamped_pct: f64,
        original_paise: Option<i64>,
        clamped_paise: Option<i64>,
        cap_source: &'static str,
    },
}

/// Pure-function clamp in pct domain.
///
/// Returns `Allowed` when `requested_pct <= cap`; returns `Clamped` otherwise
/// with `cap_source = "MAX_DISCOUNT_PCT"`. No state dependency.
pub fn clamp_discount_pct(requested_pct: f64, cap: f64) -> ClampResult {
    if requested_pct <= cap {
        ClampResult::Allowed {
            pct: requested_pct,
            paise: None,
        }
    } else {
        ClampResult::Clamped {
            original_pct: requested_pct,
            clamped_pct: cap,
            original_paise: None,
            clamped_paise: None,
            cap_source: "MAX_DISCOUNT_PCT",
        }
    }
}

/// Paise-domain convenience clamp for billing call-sites.
///
/// Computes effective pct as `requested / original`; if `> cap`, clamps to
/// `(original_price_paise as f64 * cap).round() as i64`. Defensive edge:
/// `original_price_paise <= 0` returns `Clamped` with
/// `cap_source = "ZERO_ORIGINAL_PRICE"`.
pub fn clamp_discount_paise(
    requested_discount_paise: i64,
    original_price_paise: i64,
    cap: f64,
) -> ClampResult {
    if original_price_paise <= 0 {
        // Defensive: dividing by zero or negative original price is nonsensical;
        // refuse to compute a pct, return a Clamped with explicit cap_source so
        // the call-site logs and surfaces the case.
        return ClampResult::Clamped {
            original_pct: 0.0,
            clamped_pct: 0.0,
            original_paise: Some(requested_discount_paise),
            clamped_paise: Some(0),
            cap_source: "ZERO_ORIGINAL_PRICE",
        };
    }

    let effective_pct = requested_discount_paise as f64 / original_price_paise as f64;
    if effective_pct <= cap {
        ClampResult::Allowed {
            pct: effective_pct,
            paise: Some(requested_discount_paise),
        }
    } else {
        let clamped_paise = (original_price_paise as f64 * cap).round() as i64;
        ClampResult::Clamped {
            original_pct: effective_pct,
            clamped_pct: cap,
            original_paise: Some(requested_discount_paise),
            clamped_paise: Some(clamped_paise),
            cap_source: "MAX_DISCOUNT_PCT",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_under_cap_allowed() {
        let r = clamp_discount_pct(0.30, 0.50);
        assert_eq!(
            r,
            ClampResult::Allowed {
                pct: 0.30,
                paise: None
            }
        );
    }

    #[test]
    fn clamp_at_cap_allowed() {
        let r = clamp_discount_pct(0.50, 0.50);
        assert_eq!(
            r,
            ClampResult::Allowed {
                pct: 0.50,
                paise: None
            }
        );
    }

    #[test]
    fn clamp_over_cap_clamped() {
        let r = clamp_discount_pct(0.80, 0.50);
        assert_eq!(
            r,
            ClampResult::Clamped {
                original_pct: 0.80,
                clamped_pct: 0.50,
                original_paise: None,
                clamped_paise: None,
                cap_source: "MAX_DISCOUNT_PCT",
            }
        );
    }

    #[test]
    fn clamp_paise_over_cap() {
        // 80% discount (800 paise on 1000 paise) with cap 0.50 → clamped to 500.
        let r = clamp_discount_paise(800, 1000, 0.50);
        match r {
            ClampResult::Clamped {
                original_pct,
                clamped_pct,
                original_paise,
                clamped_paise,
                cap_source,
            } => {
                assert!((original_pct - 0.80).abs() < 1e-9);
                assert!((clamped_pct - 0.50).abs() < 1e-9);
                assert_eq!(original_paise, Some(800));
                assert_eq!(clamped_paise, Some(500));
                assert_eq!(cap_source, "MAX_DISCOUNT_PCT");
            }
            other => panic!("expected Clamped, got {:?}", other),
        }
    }

    #[test]
    fn clamp_paise_zero_original() {
        let r = clamp_discount_paise(100, 0, 0.50);
        match r {
            ClampResult::Clamped {
                cap_source,
                clamped_paise,
                ..
            } => {
                assert_eq!(cap_source, "ZERO_ORIGINAL_PRICE");
                assert_eq!(clamped_paise, Some(0));
            }
            other => panic!("expected Clamped with ZERO_ORIGINAL_PRICE, got {:?}", other),
        }
    }
}
