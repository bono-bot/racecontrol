use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tracing::{info, warn};

/// Email alerter for watchdog notifications with per-pod and venue-wide rate limiting.
///
/// Rate limits:
/// - Per-pod: 1 email per 30 minutes (configurable)
/// - Venue-wide: 1 email per 5 minutes (configurable) to aggregate multi-pod failures
#[derive(Debug)]
pub struct EmailAlerter {
    /// Per-pod last-sent timestamps for cooldown enforcement
    last_sent_per_pod: HashMap<String, DateTime<Utc>>,
    /// Venue-wide last-sent timestamp
    last_venue_email: Option<DateTime<Utc>>,
    /// Email recipient address
    recipient: String,
    /// Whether email alerting is enabled
    enabled: bool,
    /// Path to the Node.js email script (send_email.js)
    script_path: String,
    /// Per-pod cooldown in seconds (default: 1800 = 30 min)
    pod_cooldown_secs: i64,
    /// Venue-wide cooldown in seconds (default: 300 = 5 min)
    venue_cooldown_secs: i64,
}

impl EmailAlerter {
    /// Create a new EmailAlerter.
    pub fn new(recipient: String, script_path: String, enabled: bool) -> Self {
        Self {
            last_sent_per_pod: HashMap::new(),
            last_venue_email: None,
            recipient,
            enabled,
            script_path,
            pod_cooldown_secs: 1800,
            venue_cooldown_secs: 300,
        }
    }

    /// Create a new EmailAlerter with custom cooldown durations.
    pub fn with_cooldowns(
        recipient: String,
        script_path: String,
        enabled: bool,
        pod_cooldown_secs: i64,
        venue_cooldown_secs: i64,
    ) -> Self {
        Self {
            last_sent_per_pod: HashMap::new(),
            last_venue_email: None,
            recipient,
            enabled,
            script_path,
            pod_cooldown_secs,
            venue_cooldown_secs,
        }
    }

    /// Check whether an alert email should be sent for the given pod at the given time.
    ///
    /// Both per-pod cooldown (30min) and venue-wide cooldown (5min) must have elapsed.
    /// Returns false if email alerting is disabled.
    pub fn should_send(&self, pod_id: &str, now: DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }

        // Check per-pod cooldown
        if let Some(last) = self.last_sent_per_pod.get(pod_id) {
            let elapsed = now.signed_duration_since(*last).num_seconds();
            if elapsed < self.pod_cooldown_secs {
                return false;
            }
        }

        // Check venue-wide cooldown
        if let Some(last_venue) = self.last_venue_email {
            let elapsed = now.signed_duration_since(last_venue).num_seconds();
            if elapsed < self.venue_cooldown_secs {
                return false;
            }
        }

        true
    }

    /// Record that an alert email was sent for the given pod at the given time.
    /// Updates both per-pod and venue-wide timestamps.
    pub fn record_sent(&mut self, pod_id: &str, now: DateTime<Utc>) {
        self.last_sent_per_pod.insert(pod_id.to_string(), now);
        self.last_venue_email = Some(now);
    }

    /// Send an alert email for the given pod, respecting rate limits.
    ///
    /// Shells out to `node {script_path} {recipient} {subject} {body}`.
    /// Uses a 15-second timeout to avoid blocking the watchdog loop.
    /// Logs warnings on failure -- never panics.
    pub async fn send_alert(&mut self, pod_id: &str, subject: &str, body: &str) {
        let now = Utc::now();
        if !self.should_send(pod_id, now) {
            info!(
                pod_id = pod_id,
                "Email alert rate-limited, skipping send"
            );
            return;
        }

        info!(
            pod_id = pod_id,
            recipient = %self.recipient,
            "Sending watchdog email alert"
        );

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            tokio::process::Command::new("node")
                .arg(&self.script_path)
                .arg(&self.recipient)
                .arg(subject)
                .arg(body)
                .kill_on_drop(true)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                if output.status.success() {
                    info!(pod_id = pod_id, "Watchdog email sent successfully");
                    self.record_sent(pod_id, now);
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    warn!(
                        pod_id = pod_id,
                        status = %output.status,
                        stderr = %stderr,
                        "Email send script failed"
                    );
                }
            }
            Ok(Err(e)) => {
                warn!(
                    pod_id = pod_id,
                    error = %e,
                    "Failed to spawn email send script"
                );
            }
            Err(_) => {
                warn!(
                    pod_id = pod_id,
                    "Email send timed out after 15 seconds"
                );
            }
        }
    }

    /// Format a readable alert email body with pod info and watchdog state.
    ///
    /// # Parameters
    /// - `pod_id` — e.g. "pod_3"
    /// - `reason` — human-readable description of what went wrong
    /// - `failure_type` — short failure category (e.g. "Max Escalation", "Pod Unreachable")
    /// - `attempt` — current restart attempt number
    /// - `cooldown_secs` — current escalation cooldown in seconds
    /// - `last_heartbeat` — timestamp of last successful heartbeat (None if never seen)
    /// - `next_action` — suggested next action for staff
    pub fn format_alert_body(
        pod_id: &str,
        reason: &str,
        failure_type: &str,
        attempt: u32,
        cooldown_secs: u64,
        last_heartbeat: Option<DateTime<Utc>>,
        next_action: &str,
    ) -> String {
        let now = Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
        let heartbeat_str = match last_heartbeat {
            Some(ts) => ts.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
            None => "Unknown".to_string(),
        };
        format!(
            "RaceControl Watchdog Alert\n\
             ==========================\n\
             \n\
             Pod: {pod_id}\n\
             Failure Type: {failure_type}\n\
             Reason: {reason}\n\
             Restart Attempt: #{attempt}\n\
             Current Cooldown: {cooldown_secs}s\n\
             Last Heartbeat: {heartbeat_str}\n\
             Next Action: {next_action}\n\
             Timestamp: {now}\n\
             \n\
             The watchdog has detected a failure and is attempting recovery.\n\
             If this pod continues to fail, escalation cooldowns will increase."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;

    fn test_alerter() -> EmailAlerter {
        EmailAlerter::new(
            "test@racingpoint.in".to_string(),
            "send_email.js".to_string(),
            true,
        )
    }

    #[test]
    fn new_creates_with_empty_maps_and_correct_config() {
        let a = test_alerter();
        assert!(a.last_sent_per_pod.is_empty());
        assert!(a.last_venue_email.is_none());
        assert_eq!(a.recipient, "test@racingpoint.in");
        assert!(a.enabled);
        assert_eq!(a.script_path, "send_email.js");
        assert_eq!(a.pod_cooldown_secs, 1800);
        assert_eq!(a.venue_cooldown_secs, 300);
    }

    #[test]
    fn should_send_returns_true_when_no_prior_email_for_pod() {
        let a = test_alerter();
        assert!(a.should_send("pod-1", Utc::now()));
    }

    #[test]
    fn should_send_returns_false_within_per_pod_cooldown() {
        let mut a = test_alerter();
        let now = Utc::now();
        a.record_sent("pod-1", now);

        // 10 minutes later (within 30-min cooldown)
        let check = now + TimeDelta::seconds(600);
        assert!(!a.should_send("pod-1", check));
    }

    #[test]
    fn should_send_returns_true_after_per_pod_cooldown_expires() {
        let mut a = test_alerter();
        let now = Utc::now();
        a.record_sent("pod-1", now);

        // 31 minutes later (past 30-min cooldown)
        let check = now + TimeDelta::seconds(1860);
        assert!(a.should_send("pod-1", check));
    }

    #[test]
    fn venue_wide_rate_limit_blocks_within_5_minutes() {
        let mut a = test_alerter();
        let now = Utc::now();
        a.record_sent("pod-1", now);

        // Different pod, but within venue-wide 5-min cooldown
        let check = now + TimeDelta::seconds(120);
        assert!(!a.should_send("pod-2", check));
    }

    #[test]
    fn venue_wide_rate_limit_allows_after_5_minutes() {
        let mut a = test_alerter();
        let now = Utc::now();
        a.record_sent("pod-1", now);

        // Different pod, past venue-wide 5-min cooldown
        let check = now + TimeDelta::seconds(301);
        assert!(a.should_send("pod-2", check));
    }

    #[test]
    fn record_sent_updates_both_timestamps() {
        let mut a = test_alerter();
        let now = Utc::now();
        a.record_sent("pod-3", now);

        assert_eq!(a.last_sent_per_pod.get("pod-3"), Some(&now));
        assert_eq!(a.last_venue_email, Some(now));
    }

    #[test]
    fn format_alert_body_produces_readable_output() {
        let body = EmailAlerter::format_alert_body(
            "pod-5",
            "heartbeat timeout",
            "Heartbeat Timeout",
            3,
            600,
            None,
            "Check pod connectivity",
        );
        assert!(body.contains("Pod: pod-5"));
        assert!(body.contains("Reason: heartbeat timeout"));
        assert!(body.contains("Restart Attempt: #3"));
        assert!(body.contains("Current Cooldown: 600s"));
        assert!(body.contains("RaceControl Watchdog Alert"));
    }

    #[test]
    fn format_alert_body_with_last_heartbeat_some_includes_timestamp() {
        let ts = Utc::now();
        let body = EmailAlerter::format_alert_body(
            "pod-3",
            "test reason",
            "Test Failure",
            1,
            30,
            Some(ts),
            "Retry in 30s",
        );
        assert!(body.contains("Last Heartbeat:"));
        assert!(!body.contains("Last Heartbeat: Unknown"));
    }

    #[test]
    fn format_alert_body_with_last_heartbeat_none_shows_unknown() {
        let body = EmailAlerter::format_alert_body(
            "pod-7",
            "never seen",
            "No Heartbeat",
            1,
            30,
            None,
            "Manual check required",
        );
        assert!(body.contains("Last Heartbeat: Unknown"));
    }

    #[test]
    fn format_alert_body_includes_failure_type_and_next_action() {
        let body = EmailAlerter::format_alert_body(
            "pod-2",
            "test",
            "Max Escalation",
            5,
            1800,
            None,
            "Manual intervention required",
        );
        assert!(body.contains("Max Escalation"));
        assert!(body.contains("Manual intervention required"));
    }

    #[test]
    fn disabled_alerter_never_sends() {
        let a = EmailAlerter::new(
            "test@racingpoint.in".to_string(),
            "send_email.js".to_string(),
            false,
        );
        assert!(!a.should_send("pod-1", Utc::now()));
    }

    #[test]
    fn custom_cooldowns_respected() {
        let mut a = EmailAlerter::with_cooldowns(
            "test@racingpoint.in".to_string(),
            "send_email.js".to_string(),
            true,
            60,  // 1 min pod cooldown
            10,  // 10s venue cooldown
        );
        let now = Utc::now();
        a.record_sent("pod-1", now);

        // Within pod cooldown (60s), should not send
        assert!(!a.should_send("pod-1", now + TimeDelta::seconds(30)));
        // Past pod cooldown
        assert!(a.should_send("pod-1", now + TimeDelta::seconds(61)));

        // Different pod, within venue cooldown (10s)
        assert!(!a.should_send("pod-2", now + TimeDelta::seconds(5)));
        // Past venue cooldown
        assert!(a.should_send("pod-2", now + TimeDelta::seconds(11)));
    }
}
