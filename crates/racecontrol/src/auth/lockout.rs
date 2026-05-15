// PR-A W1-S6 PIN-LOCKOUT auto-rotate — publisher contract scaffold (Phase 1)
// Captain Option C hybrid (2026-05-09 ~20:28 IST) → cascade #8 PR-A authoring
// Authored per W1-S6-PLAN.md §4.1 + Wave A.2.1 §10 Q-DECISION compliance map
//
// PHASE 1 SCOPE: types + public API surface only. Implementation in Phase 2.
// PHASE 2: dispatch.rs (retry queue) + audit.rs (V2 Audit-Log Doctrine writes)
// PHASE 3: middleware.rs wire-in + integration tests
//
// Q-W1-CROSS-1 (security-class explicit): refresh path MUST consult
// is_locked_out() predicate; bypass = gate violation.
// Q-W1-CROSS-2 (default-a sequencing): this is PR-A FIRST; PR-C (W1-S5
// refresh consumer) merges AFTER PR-A.
// Q-W1-S6-NEW-2 (retry queue): see dispatch.rs (Phase 2).
// NEW-Q-2 (V2 Audit-Log Doctrine): see audit.rs (Phase 2).

use std::sync::Arc;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::auth::dispatch::{DispatchError, DispatchManager, EmailPayload};

/// Predicate returned by `LockoutManager::is_locked_out`.
///
/// Q-W1-CROSS-1: SECURITY-CLASS EXPLICIT — W1-S5 refresh path MUST
/// consult this predicate via the publisher; bypass is gate violation
/// per Captain G33 v5 #2 ratification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockoutPredicate {
    /// Staff is currently locked out. Refresh path returns 401 with `until`.
    Active {
        /// Lockout expiry (ISO-8601 UTC). NULL on V1 means "indefinite".
        until: DateTime<Utc>,
        /// Free-form reason code: "threshold_breach" | "admin_manual" | etc.
        reason: String,
    },
    /// Staff is not locked out — refresh path may proceed.
    Inactive,
}

/// Errors returned by `LockoutManager`.
#[derive(Debug, Error)]
pub enum LockoutError {
    #[error("staff_id not found: {0}")]
    StaffNotFound(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid lockout_until value in DB: {0}")]
    InvalidLockoutUntil(String),
}

/// Publisher-side contract for PIN-LOCKOUT state.
///
/// Owned by racecontrol-crate startup; injected as `Arc<LockoutManager>`
/// into the W1-S5 refresh path (PR-C consumer) per Q-W1-CROSS-1
/// security-class-explicit ratification (Captain G33 v5 #2).
///
/// Phase 1: skeleton with method signatures only. Phase 2 implements
/// `is_locked_out` against the v2-db `staff.lockout_until` column,
/// `record_failed_attempt` against the threshold logic, and
/// `subscribe` for cross-coupling to W1-S5 refresh path.
pub struct LockoutManager {
    // Phase 1 scaffold: pool injected; logic added in Phase 2.
    #[allow(dead_code)]
    pool: sqlx::SqlitePool,
    // Phase 2 (PR #87 amendment / DISPATCH-8 Finding I-3): real `DispatchManager`
    // handle replacing the Phase 1 `DispatchTaskPlaceholder` zero-size marker.
    // Optional so existing call sites that construct `LockoutManager::new(pool)`
    // (without an injected dispatcher) keep compiling; threshold-breach paths
    // route through `enqueue_alert_email` which short-circuits when None.
    dispatch_manager: Option<Arc<DispatchManager>>,
}

impl LockoutManager {
    /// Construct a new `LockoutManager` without an alert dispatcher.
    ///
    /// Convenience shim for tests / call sites that don't need lockout-alert
    /// email dispatch. Production wires the dispatcher via
    /// [`Self::new_with_dispatch`] from startup.
    pub fn new(pool: sqlx::SqlitePool) -> Arc<Self> {
        Self::new_with_dispatch(pool, None)
    }

    /// Construct a new `LockoutManager` with an injected `DispatchManager`.
    ///
    /// Wires the Q-W1-S6-NEW-2 retry-queue path: threshold-breach alerts
    /// enqueue an `EmailPayload` via the dispatcher's `enqueue_email`. Pass
    /// `None` for `dispatch_manager` to skip alert dispatch (testing /
    /// degraded-mode startup).
    pub fn new_with_dispatch(
        pool: sqlx::SqlitePool,
        dispatch_manager: Option<Arc<DispatchManager>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            dispatch_manager,
        })
    }

    /// Internal helper: enqueue a lockout-alert email through the injected
    /// `DispatchManager`, if one is configured. Returns `Ok(())` (no-op) when
    /// no dispatcher is wired so the caller path does not fail closed in
    /// degraded-mode startup.
    ///
    /// Threshold-breach logic (Phase 2.2 sub-PR) calls this after writing the
    /// `staff.lockout_until` row + audit-log entry.
    #[allow(dead_code)]
    pub(crate) fn enqueue_alert_email(
        &self,
        payload: EmailPayload,
    ) -> Result<(), DispatchError> {
        match self.dispatch_manager.as_ref() {
            Some(mgr) => mgr.enqueue_email(payload),
            None => Ok(()),
        }
    }

    /// Q-W1-CROSS-1 publisher contract.
    ///
    /// Returns `LockoutPredicate::Active { until, reason }` if `staff_id`
    /// has `lockout_until > now()`; `LockoutPredicate::Inactive` otherwise.
    ///
    /// Phase 2 implementation reads `staff.lockout_until` from v2-db pool.
    pub async fn is_locked_out(&self, _staff_id: &str) -> Result<LockoutPredicate, LockoutError> {
        // Phase 2: SELECT lockout_until FROM staff WHERE id = ?
        // Phase 1 stub: returns Inactive so refresh-path consumer (PR-C)
        // can compile + test against the API surface ahead of merge.
        Ok(LockoutPredicate::Inactive)
    }

    /// Record a failed login attempt. If threshold breached within window,
    /// transitions staff to lockout state, rotates PIN, dispatches alert.
    ///
    /// Phase 2 implementation: threshold logic + PIN rotation + dispatch
    /// enqueue + audit write per NEW-Q-2 V2 Audit-Log Doctrine.
    pub async fn record_failed_attempt(&self, _staff_id: &str) -> Result<(), LockoutError> {
        // Phase 2 implementation pending.
        Ok(())
    }

    /// Reset lockout state for `staff_id` (admin-triggered).
    ///
    /// Phase 2 implementation: zeros lockout_count + clears lockout_until
    /// + audit-writes 'admin_manual_unlock' event.
    pub async fn admin_unlock(&self, _staff_id: &str) -> Result<(), LockoutError> {
        // Phase 2 implementation pending.
        Ok(())
    }

    /// Subscribe to lockout-state events (forward-compat for W1-S5 PR-C).
    ///
    /// Q-W1-CROSS-2: PR-A FIRST means PR-C (W1-S5 refresh consumer) wires
    /// itself via this API in a subsequent merge. Phase 1 placeholder; PR-C
    /// integration test will mock this.
    pub fn subscribe<F>(&self, _consumer: F)
    where
        F: Fn(&str, &LockoutPredicate) + Send + Sync + 'static,
    {
        // Phase 2 implementation pending — pub-sub channel wiring.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockout_predicate_active_constructs() {
        let p = LockoutPredicate::Active {
            until: Utc::now() + chrono::Duration::seconds(300),
            reason: "threshold_breach".to_string(),
        };
        match p {
            LockoutPredicate::Active { reason, .. } => assert_eq!(reason, "threshold_breach"),
            LockoutPredicate::Inactive => panic!("expected Active"),
        }
    }

    #[test]
    fn lockout_predicate_inactive_compares_equal() {
        assert_eq!(LockoutPredicate::Inactive, LockoutPredicate::Inactive);
    }
}
