use chrono::{DateTime, Utc};
use std::time::Duration;

/// Escalating backoff state machine for watchdog restart cooldowns.
///
/// Produces increasing cooldown durations between restart attempts:
/// 30s -> 2m -> 10m -> 30m (capped).
/// Resets to the first step on successful recovery.
#[derive(Debug, Clone)]
pub struct EscalatingBackoff {
    attempt: u32,
    last_attempt_at: Option<DateTime<Utc>>,
    steps: Vec<Duration>,
}

impl EscalatingBackoff {
    /// Default escalation steps: 30s, 2m, 10m, 30m.
    const DEFAULT_STEPS: &[u64] = &[30, 120, 600, 1800];

    /// Create a new backoff with default escalation steps [30s, 120s, 600s, 1800s].
    pub fn new() -> Self {
        Self {
            attempt: 0,
            last_attempt_at: None,
            steps: Self::DEFAULT_STEPS
                .iter()
                .map(|&s| Duration::from_secs(s))
                .collect(),
        }
    }

    /// Create a new backoff with custom step durations.
    pub fn with_steps(steps: Vec<Duration>) -> Self {
        Self {
            attempt: 0,
            last_attempt_at: None,
            steps,
        }
    }

    /// Current cooldown duration based on attempt count.
    /// Clamped to the last step if attempt exceeds the number of steps.
    pub fn current_cooldown(&self) -> Duration {
        let idx = (self.attempt as usize).min(self.steps.len().saturating_sub(1));
        self.steps.get(idx).copied().unwrap_or(Duration::from_secs(30))
    }

    /// Whether enough time has elapsed since the last attempt to allow another.
    /// Returns true if no prior attempt exists or if elapsed time >= current cooldown.
    pub fn ready(&self, now: DateTime<Utc>) -> bool {
        match self.last_attempt_at {
            None => true,
            Some(last) => {
                let elapsed = now.signed_duration_since(last);
                let cooldown_secs = self.current_cooldown().as_secs() as i64;
                elapsed.num_seconds() >= cooldown_secs
            }
        }
    }

    /// Record that a restart attempt was made at the given time.
    pub fn record_attempt(&mut self, now: DateTime<Utc>) {
        self.last_attempt_at = Some(now);
        self.attempt = self.attempt.saturating_add(1);
    }

    /// Reset backoff state after successful recovery.
    pub fn reset(&mut self) {
        self.attempt = 0;
        self.last_attempt_at = None;
    }

    /// Whether all escalation steps have been exhausted.
    pub fn exhausted(&self) -> bool {
        self.attempt as usize >= self.steps.len()
    }

    /// Current attempt count.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    #[test]
    fn new_starts_at_attempt_zero_with_no_last_attempt() {
        let b = EscalatingBackoff::new();
        assert_eq!(b.attempt, 0);
        assert!(b.last_attempt_at.is_none());
    }

    #[test]
    fn current_cooldown_returns_30s_at_attempt_0() {
        let b = EscalatingBackoff::new();
        assert_eq!(b.current_cooldown(), Duration::from_secs(30));
    }

    #[test]
    fn current_cooldown_returns_120s_at_attempt_1() {
        let mut b = EscalatingBackoff::new();
        b.attempt = 1;
        assert_eq!(b.current_cooldown(), Duration::from_secs(120));
    }

    #[test]
    fn current_cooldown_returns_600s_at_attempt_2() {
        let mut b = EscalatingBackoff::new();
        b.attempt = 2;
        assert_eq!(b.current_cooldown(), Duration::from_secs(600));
    }

    #[test]
    fn current_cooldown_returns_1800s_at_attempt_3() {
        let mut b = EscalatingBackoff::new();
        b.attempt = 3;
        assert_eq!(b.current_cooldown(), Duration::from_secs(1800));
    }

    #[test]
    fn current_cooldown_caps_at_1800s_for_attempt_4_plus() {
        let mut b = EscalatingBackoff::new();
        b.attempt = 4;
        assert_eq!(b.current_cooldown(), Duration::from_secs(1800));
        b.attempt = 10;
        assert_eq!(b.current_cooldown(), Duration::from_secs(1800));
        b.attempt = 100;
        assert_eq!(b.current_cooldown(), Duration::from_secs(1800));
    }

    #[test]
    fn ready_returns_true_when_no_prior_attempt() {
        let b = EscalatingBackoff::new();
        assert!(b.ready(Utc::now()));
    }

    #[test]
    fn ready_returns_false_when_elapsed_less_than_cooldown() {
        let mut b = EscalatingBackoff::new();
        let now = Utc::now();
        b.last_attempt_at = Some(now);
        // Still within 30s cooldown
        let check_time = now + TimeDelta::seconds(10);
        assert!(!b.ready(check_time));
    }

    #[test]
    fn ready_returns_true_when_elapsed_exceeds_cooldown() {
        let mut b = EscalatingBackoff::new();
        let now = Utc::now();
        b.last_attempt_at = Some(now);
        // Past 30s cooldown
        let check_time = now + TimeDelta::seconds(31);
        assert!(b.ready(check_time));
    }

    #[test]
    fn record_attempt_increments_and_sets_timestamp() {
        let mut b = EscalatingBackoff::new();
        let now = Utc::now();
        b.record_attempt(now);
        assert_eq!(b.attempt, 1);
        assert_eq!(b.last_attempt_at, Some(now));

        let later = now + TimeDelta::seconds(60);
        b.record_attempt(later);
        assert_eq!(b.attempt, 2);
        assert_eq!(b.last_attempt_at, Some(later));
    }

    #[test]
    fn reset_clears_state() {
        let mut b = EscalatingBackoff::new();
        b.record_attempt(Utc::now());
        b.record_attempt(Utc::now());
        assert_eq!(b.attempt, 2);
        assert!(b.last_attempt_at.is_some());

        b.reset();
        assert_eq!(b.attempt, 0);
        assert!(b.last_attempt_at.is_none());
    }

    #[test]
    fn exhausted_returns_true_when_attempt_gte_steps_len() {
        let mut b = EscalatingBackoff::new();
        b.attempt = 4; // steps.len() == 4
        assert!(b.exhausted());
        b.attempt = 5;
        assert!(b.exhausted());
    }

    #[test]
    fn exhausted_returns_false_when_attempt_lt_steps_len() {
        let mut b = EscalatingBackoff::new();
        assert!(!b.exhausted()); // attempt=0
        b.attempt = 3;
        assert!(!b.exhausted()); // attempt=3, steps.len()=4
    }

    #[test]
    fn with_steps_accepts_custom_durations() {
        let custom = vec![
            Duration::from_secs(5),
            Duration::from_secs(10),
        ];
        let mut b = EscalatingBackoff::with_steps(custom);
        assert_eq!(b.current_cooldown(), Duration::from_secs(5));
        b.attempt = 1;
        assert_eq!(b.current_cooldown(), Duration::from_secs(10));
        // Cap at last step
        b.attempt = 2;
        assert_eq!(b.current_cooldown(), Duration::from_secs(10));
        // Exhausted at attempt >= 2
        assert!(b.exhausted());
    }
}
