//! Budget Tracker — per-node daily cost tracking for OpenRouter model calls.
//!
//! Hard ceiling: $10/day per pod, $20/day for server.
//! When ceiling is reached, Tiers 3+4 are blocked → falls back to Tier 1+2 (free).
//! Budget resets at midnight IST.
//!
//! Phase 232 — Meshed Intelligence BUDGET-01 to BUDGET-06.

use chrono::{FixedOffset, NaiveDate, Utc};
use serde::Serialize;

const LOG_TARGET: &str = "budget-tracker";

/// Default daily budget for pods ($10/day)
pub const DEFAULT_POD_DAILY_LIMIT: f64 = 10.0;

/// Default daily budget for server ($20/day)
pub const DEFAULT_SERVER_DAILY_LIMIT: f64 = 20.0;

/// Minimum reserve — if remaining < this, block model calls
pub const MIN_RESERVE: f64 = 2.0;

/// Monthly soft alert threshold
pub const MONTHLY_SOFT_ALERT: f64 = 50.0;

/// IST offset (UTC+5:30) for midnight reset
fn ist_today() -> NaiveDate {
    let ist = FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .unwrap_or_else(|| FixedOffset::east_opt(0).expect("UTC offset 0 is always valid"));
    Utc::now().with_timezone(&ist).date_naive()
}

/// Budget status exposed via health endpoint (BUDGET-06)
#[derive(Debug, Clone, Serialize)]
pub struct BudgetStatus {
    pub daily_limit: f64,
    pub spent_today: f64,
    pub remaining_today: f64,
    pub monthly_spent: f64,
    pub budget_date: String,
    pub ceiling_hit: bool,
    pub model_calls_today: u32,
}

/// Per-node budget tracker. Thread-safe via Arc<RwLock<BudgetTracker>> in AppState.
#[derive(Debug)]
pub struct BudgetTracker {
    daily_limit: f64,
    spent_today: f64,
    monthly_spent: f64,
    model_calls_today: u32,
    budget_date: NaiveDate,
}

impl BudgetTracker {
    /// Create a new budget tracker with the given daily limit.
    pub fn new(daily_limit: f64) -> Self {
        let today = ist_today();
        tracing::info!(
            target: LOG_TARGET,
            daily_limit = daily_limit,
            date = %today,
            "Budget tracker initialized"
        );
        Self {
            daily_limit,
            spent_today: 0.0,
            monthly_spent: 0.0,
            model_calls_today: 0,
            budget_date: today,
        }
    }

    /// Create with default pod budget ($10/day)
    pub fn new_pod() -> Self {
        Self::new(DEFAULT_POD_DAILY_LIMIT)
    }

    /// Check if we should reset for a new day (midnight IST crossing)
    fn maybe_reset(&mut self) {
        let today = ist_today();
        if today != self.budget_date {
            tracing::info!(
                target: LOG_TARGET,
                old_date = %self.budget_date,
                new_date = %today,
                spent_yesterday = self.spent_today,
                "Budget reset — new day (midnight IST)"
            );
            self.monthly_spent += self.spent_today;
            self.spent_today = 0.0;
            self.model_calls_today = 0;
            self.budget_date = today;
        }
    }

    /// Check if a model call with estimated cost can proceed.
    /// Returns true if within budget, false if ceiling would be hit.
    /// BUDGET-03: Hard ceiling enforcement.
    pub fn can_spend(&mut self, estimated_cost: f64) -> bool {
        self.maybe_reset();
        let remaining = self.daily_limit - self.spent_today;

        // Block if remaining would drop below minimum reserve (BUDGET-04)
        if remaining - estimated_cost < MIN_RESERVE && self.spent_today > 0.0 {
            tracing::warn!(
                target: LOG_TARGET,
                remaining = remaining,
                estimated_cost = estimated_cost,
                min_reserve = MIN_RESERVE,
                "Budget check: would breach minimum reserve — blocking model call"
            );
            return false;
        }

        // Hard ceiling check
        if self.spent_today + estimated_cost > self.daily_limit {
            tracing::warn!(
                target: LOG_TARGET,
                spent = self.spent_today,
                estimated_cost = estimated_cost,
                daily_limit = self.daily_limit,
                "Budget check: daily ceiling reached — blocking model call"
            );
            return false;
        }

        true
    }

    /// Record actual spend after a model call completes.
    /// BUDGET-02: Per-incident cost tracking.
    pub fn record_spend(&mut self, actual_cost: f64) {
        self.maybe_reset();
        self.spent_today += actual_cost;
        self.model_calls_today += 1;

        tracing::info!(
            target: LOG_TARGET,
            cost = actual_cost,
            spent_today = self.spent_today,
            remaining = self.daily_limit - self.spent_today,
            calls_today = self.model_calls_today,
            "Budget: cost recorded"
        );

        // Monthly soft alert (BUDGET-05)
        let total_month = self.monthly_spent + self.spent_today;
        if total_month >= MONTHLY_SOFT_ALERT && total_month - actual_cost < MONTHLY_SOFT_ALERT {
            tracing::warn!(
                target: LOG_TARGET,
                monthly_total = total_month,
                threshold = MONTHLY_SOFT_ALERT,
                "Budget WARNING: monthly soft alert threshold crossed"
            );
        }
    }

    /// Get current budget status for health endpoint (BUDGET-06).
    pub fn status(&mut self) -> BudgetStatus {
        self.maybe_reset();
        let remaining = (self.daily_limit - self.spent_today).max(0.0);
        BudgetStatus {
            daily_limit: self.daily_limit,
            spent_today: self.spent_today,
            remaining_today: remaining,
            monthly_spent: self.monthly_spent + self.spent_today,
            budget_date: self.budget_date.to_string(),
            ceiling_hit: remaining < MIN_RESERVE,
            model_calls_today: self.model_calls_today,
        }
    }

    /// Get remaining budget for today
    pub fn remaining(&mut self) -> f64 {
        self.maybe_reset();
        (self.daily_limit - self.spent_today).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker() {
        let t = BudgetTracker::new(10.0);
        assert!((t.daily_limit - 10.0).abs() < f64::EPSILON);
        assert!((t.spent_today - 0.0).abs() < f64::EPSILON);
        assert_eq!(t.model_calls_today, 0);
    }

    #[test]
    fn test_can_spend_within_budget() {
        let mut t = BudgetTracker::new(10.0);
        assert!(t.can_spend(3.0), "Should allow $3 spend on fresh $10 budget");
    }

    #[test]
    fn test_can_spend_ceiling_hit() {
        let mut t = BudgetTracker::new(10.0);
        t.record_spend(9.0); // Spent $9 of $10
        assert!(!t.can_spend(3.0), "Should block $3 when only $1 left");
    }

    #[test]
    fn test_can_spend_reserve_protection() {
        let mut t = BudgetTracker::new(10.0);
        t.record_spend(7.5); // $2.50 remaining, reserve is $2.00
        // $0.60 would leave $1.90 (below $2 reserve)
        assert!(!t.can_spend(0.60), "Should block when result would breach $2 reserve");
    }

    #[test]
    fn test_can_spend_first_call_ignores_reserve() {
        let mut t = BudgetTracker::new(10.0);
        // First call ($0 spent) should work even if it would leave < $2 reserve
        assert!(t.can_spend(9.0), "First call should be allowed even if large");
    }

    #[test]
    fn test_record_spend_tracks_cost() {
        let mut t = BudgetTracker::new(10.0);
        t.record_spend(0.05);
        t.record_spend(3.01);
        assert!((t.spent_today - 3.06).abs() < 0.001);
        assert_eq!(t.model_calls_today, 2);
    }

    #[test]
    fn test_status_format() {
        let mut t = BudgetTracker::new(10.0);
        t.record_spend(4.50);
        let s = t.status();
        assert!((s.daily_limit - 10.0).abs() < f64::EPSILON);
        assert!((s.spent_today - 4.50).abs() < 0.001);
        assert!((s.remaining_today - 5.50).abs() < 0.001);
        assert!(!s.ceiling_hit);
        assert_eq!(s.model_calls_today, 1);
    }

    #[test]
    fn test_status_ceiling_hit_flag() {
        let mut t = BudgetTracker::new(10.0);
        t.record_spend(9.50); // Only $0.50 left, below $2 reserve
        let s = t.status();
        assert!(s.ceiling_hit, "ceiling_hit should be true when remaining < reserve");
    }

    #[test]
    fn test_remaining() {
        let mut t = BudgetTracker::new(10.0);
        assert!((t.remaining() - 10.0).abs() < f64::EPSILON);
        t.record_spend(3.0);
        assert!((t.remaining() - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_new_pod_default() {
        let t = BudgetTracker::new_pod();
        assert!((t.daily_limit - 10.0).abs() < f64::EPSILON);
    }
}
