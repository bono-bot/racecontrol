//! Inactivity monitor for agent-side idle detection (BILL-01).
//!
//! Tracks the last input activity during an active game session.
//! After `threshold_secs` of no steering/pedal/button input, fires a one-shot
//! InactivityAlert which the event loop sends to the server as
//! AgentMessage::InactivityAlert.
//!
//! - Alert is one-shot per idle period: only one alert until input resets state.
//! - `record_input()` resets the timer AND clears alert_sent.
//! - `reset()` is called on session start/end to clear all state.

use std::time::Instant;

const LOG_TARGET: &str = "inactivity-monitor";

/// Monitor for customer input inactivity during an active game session.
pub struct InactivityMonitor {
    /// How many seconds of no input before firing an alert.
    threshold_secs: u64,
    /// Last time any input was recorded.
    last_input_at: Instant,
    /// Whether the alert has already been sent for this idle period.
    alert_sent: bool,
}

impl InactivityMonitor {
    /// Create a new monitor with the given threshold in seconds.
    /// Default for production use is 600 (10 minutes).
    pub fn new(threshold_secs: u64) -> Self {
        Self {
            threshold_secs,
            last_input_at: Instant::now(),
            alert_sent: false,
        }
    }

    /// Record that input was received — resets idle timer and clears alert state.
    /// Call this whenever steering angle, pedal position, or button input is observed.
    pub fn record_input(&mut self) {
        self.last_input_at = Instant::now();
        self.alert_sent = false;
    }

    /// Poll the monitor. Must be called approximately every second.
    ///
    /// Returns `Some(idle_seconds)` (the elapsed idle time) when the threshold
    /// is exceeded AND the alert hasn't been sent yet. Returns `None` otherwise.
    ///
    /// Alert is one-shot: subsequent ticks return None until `record_input()` is called.
    pub fn tick(&mut self) -> Option<u64> {
        let idle_secs = self.last_input_at.elapsed().as_secs();

        if idle_secs >= self.threshold_secs && !self.alert_sent {
            self.alert_sent = true;
            tracing::warn!(
                target: LOG_TARGET,
                idle_secs,
                threshold_secs = self.threshold_secs,
                "BILL-01: Inactivity threshold exceeded — alerting staff",
            );
            return Some(idle_secs);
        }

        None
    }

    /// Reset all state — call on session start and session end.
    pub fn reset(&mut self) {
        self.last_input_at = Instant::now();
        self.alert_sent = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_new_creates_monitor_with_threshold() {
        let monitor = InactivityMonitor::new(600);
        assert_eq!(monitor.threshold_secs, 600);
        assert!(!monitor.alert_sent);
    }

    #[test]
    fn test_record_input_resets_timer_and_clears_alert() {
        let mut monitor = InactivityMonitor::new(1);
        // Force alert_sent to true
        monitor.alert_sent = true;
        monitor.record_input();
        assert!(!monitor.alert_sent);
    }

    #[test]
    fn test_tick_returns_none_while_below_threshold() {
        let mut monitor = InactivityMonitor::new(600);
        // Fresh monitor — way under threshold
        assert_eq!(monitor.tick(), None);
    }

    #[test]
    fn test_tick_returns_some_after_threshold_exceeded() {
        let mut monitor = InactivityMonitor::new(0);
        // Threshold is 0 seconds — any tick should exceed it
        // Slight delay to ensure elapsed > 0
        std::thread::sleep(Duration::from_millis(10));
        let result = monitor.tick();
        assert!(result.is_some(), "Expected Some(idle_secs) after threshold");
        assert!(result.unwrap() >= 0);
    }

    #[test]
    fn test_alert_is_one_shot_second_tick_returns_none() {
        let mut monitor = InactivityMonitor::new(0);
        std::thread::sleep(Duration::from_millis(10));
        let first = monitor.tick();
        assert!(first.is_some());
        // Second tick without input reset — should be None
        let second = monitor.tick();
        assert_eq!(second, None, "Alert must be one-shot until input resets it");
    }

    #[test]
    fn test_alert_resets_after_record_input() {
        let mut monitor = InactivityMonitor::new(0);
        std::thread::sleep(Duration::from_millis(10));
        let first = monitor.tick();
        assert!(first.is_some());
        // Input received — clears alert
        monitor.record_input();
        // threshold is 0, so immediately exceeds again — should fire another alert
        std::thread::sleep(Duration::from_millis(10));
        let second = monitor.tick();
        assert!(second.is_some(), "Expected alert to fire again after input reset");
    }

    #[test]
    fn test_reset_clears_alert_state_for_new_session() {
        let mut monitor = InactivityMonitor::new(0);
        std::thread::sleep(Duration::from_millis(10));
        monitor.tick(); // fire alert
        monitor.reset();
        assert!(!monitor.alert_sent);
        // After reset, a fresh tick against threshold=0 fires again
        std::thread::sleep(Duration::from_millis(10));
        let result = monitor.tick();
        assert!(result.is_some());
    }
}
