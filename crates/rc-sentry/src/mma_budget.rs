//! MMA Budget Tracker for rc-sentry -- session-scoped $5 cap.
//!
//! "Session" = process lifetime (resets on rc-sentry restart).
//! Every spend event is appended to mma-spend.log for the /mma/status endpoint.
//!
//! Design: pure std, no tokio. Mutex<f64> for accumulation (AtomicU64 + f64::to_bits
//! would be incorrect -- IEEE 754 add != u64 add).

use serde::Serialize;
use std::sync::{Mutex, OnceLock};

const LOG_TARGET: &str = "mma-budget";
pub const SESSION_CAP_USD: f64 = 5.0;
pub const SPEND_LOG_PATH: &str = r"C:\RacingPoint\mma-spend.log";

static SESSION_SPENT: OnceLock<Mutex<f64>> = OnceLock::new();

fn session_spent_mutex() -> &'static Mutex<f64> {
    SESSION_SPENT.get_or_init(|| Mutex::new(0.0))
}

/// Current session spend in USD.
pub fn session_spent() -> f64 {
    session_spent_mutex()
        .lock()
        .map(|v| *v)
        .unwrap_or(0.0)
}

/// Check if `amount` can be spent within the session cap.
/// Returns false if adding amount would exceed SESSION_CAP_USD.
/// Returns false on Mutex poison (safe default -- blocks spend).
pub fn can_spend(amount: f64) -> bool {
    session_spent_mutex()
        .lock()
        .map(|v| *v + amount <= SESSION_CAP_USD)
        .unwrap_or(false)
}

/// Record a spend event.
/// Accumulates to session total and appends to spend log file.
pub fn record_spend(model: &str, amount: f64) {
    let cumulative = match session_spent_mutex().lock() {
        Ok(mut v) => {
            *v += amount;
            *v
        }
        Err(poisoned) => {
            let mut v = poisoned.into_inner();
            *v += amount;
            *v
        }
    };

    tracing::info!(
        target: LOG_TARGET,
        model = model,
        amount_usd = amount,
        session_total_usd = cumulative,
        "MMA spend recorded"
    );

    append_spend_log(model, amount, cumulative);
}

/// Reset session budget (called at MMA session start).
pub fn reset_session() {
    match session_spent_mutex().lock() {
        Ok(mut v) => { *v = 0.0; }
        Err(poisoned) => { *poisoned.into_inner() = 0.0; }
    }
    tracing::info!(target: LOG_TARGET, "MMA session budget reset");
}

/// Append a spend event line to the log file.
/// Format: `ISO-8601 | model | $cost | session=$cumulative`
/// No-op on file error (non-critical path).
fn append_spend_log(model: &str, amount: f64, cumulative: f64) {
    use std::io::Write;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let line = format!(
        "{} | {} | ${:.4} | session=${:.4}\n",
        timestamp, model, amount, cumulative
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(SPEND_LOG_PATH)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Status struct for /mma/status endpoint.
#[derive(Debug, Serialize)]
pub struct MmaBudgetStatus {
    pub session_spent_usd: f64,
    pub session_cap_usd: f64,
    pub remaining_usd: f64,
    pub cap_hit: bool,
    pub spend_log_path: &'static str,
}

pub fn get_budget_status() -> MmaBudgetStatus {
    let spent = session_spent();
    MmaBudgetStatus {
        session_spent_usd: spent,
        session_cap_usd: SESSION_CAP_USD,
        remaining_usd: (SESSION_CAP_USD - spent).max(0.0),
        cap_hit: spent >= SESSION_CAP_USD,
        spend_log_path: SPEND_LOG_PATH,
    }
}

// --- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;
    // Tests share a static OnceLock — serialize them to avoid races
    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    #[test]
    fn test_budget_cap_enforcement() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        reset_session();
        assert!(can_spend(4.99), "Should allow spend under cap");
        assert!(!can_spend(5.01), "Should deny spend over cap");
    }

    #[test]
    fn test_record_spend_accumulates() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        reset_session();
        record_spend("model-a", 1.00);
        record_spend("model-b", 2.00);
        let spent = session_spent();
        assert!((spent - 3.00).abs() < 0.001, "Should accumulate: got {}", spent);
    }

    #[test]
    fn test_reset_clears_session() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        record_spend("model-a", 2.00);
        reset_session();
        assert!((session_spent() - 0.0).abs() < 0.001, "Should be 0 after reset");
    }

    #[test]
    fn test_cap_hit_detection() {
        let _lock = TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        reset_session();
        record_spend("model", SESSION_CAP_USD + 0.01);
        let status = get_budget_status();
        assert!(status.cap_hit, "cap_hit should be true after exceeding cap");
    }
}
