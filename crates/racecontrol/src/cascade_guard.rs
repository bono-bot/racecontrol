//! Anti-cascade guard for recovery systems.
//!
//! Counts recovery actions in a 60-second sliding window.
//! If 3+ actions fire from DIFFERENT authorities in that window,
//! automated recovery pauses for PAUSE_DURATION and a WhatsApp alert fires.
//!
//! SERVER-DOWN EXEMPTION: if all recent actions share reason "server_startup_recovery"
//! or the action set contains only single-authority bursts (all 8 pods restarting via
//! PodHealer because server just came up), this is NOT treated as a cascade.
//!
//! STANDING RULE: No .unwrap() in production code.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use rc_common::recovery::{RecoveryAuthority, RecoveryDecision};

use crate::config::Config;

const CASCADE_WINDOW_SECS: u64 = 60;
const CASCADE_THRESHOLD: usize = 3;
const PAUSE_DURATION: Duration = Duration::from_secs(300); // 5 minutes

/// Reason substring that exempts an action from cascade counting.
/// Used for "all pods restart because server came up" scenarios.
const SERVER_STARTUP_EXEMPT_REASON: &str = "server_startup_recovery";

/// Extracted alert configuration — only the fields CascadeGuard needs.
/// Avoids requiring Config to implement Clone.
#[derive(Clone, Debug)]
pub struct CascadeAlertConfig {
    pub evolution_url: Option<String>,
    pub evolution_api_key: Option<String>,
    pub evolution_instance: Option<String>,
    pub uday_phone: Option<String>,
}

impl CascadeAlertConfig {
    pub fn from_config(config: &Config) -> Self {
        Self {
            evolution_url: config.auth.evolution_url.clone(),
            evolution_api_key: config.auth.evolution_api_key.clone(),
            evolution_instance: config.auth.evolution_instance.clone(),
            uday_phone: config.alerting.uday_phone.clone(),
        }
    }

    /// Returns an unconfigured instance (WA alerts disabled — used in tests).
    #[cfg(test)]
    fn empty() -> Self {
        Self {
            evolution_url: None,
            evolution_api_key: None,
            evolution_instance: None,
            uday_phone: None,
        }
    }
}

/// Lightweight record of one recovery action for cascade counting.
struct ActionRecord {
    recorded_at: Instant,
    authority: RecoveryAuthority,
    reason: String,
}

pub struct CascadeGuard {
    /// Sliding window of recent actions
    window: Vec<ActionRecord>,
    /// When automated recovery is paused until (None = not paused)
    pause_until: Option<Instant>,
    /// Alert configuration (Evolution API + Uday phone)
    alert_config: CascadeAlertConfig,
    /// HTTP client for WhatsApp alerts
    http_client: reqwest::Client,
}

impl CascadeGuard {
    pub fn new(alert_config: CascadeAlertConfig, http_client: reqwest::Client) -> Self {
        Self {
            window: Vec::new(),
            pause_until: None,
            alert_config,
            http_client,
        }
    }

    /// Record a recovery decision and check cascade threshold.
    /// Returns true if cascade was just triggered (callers may want to log this).
    /// Calls send_cascade_alert() internally (blocking-safe via tokio::spawn).
    pub fn record(&mut self, decision: &RecoveryDecision) -> bool {
        self.record_at(decision, Instant::now())
    }

    /// Internal record with injectable timestamp (enables time-travel in tests).
    fn record_at(&mut self, decision: &RecoveryDecision, now: Instant) -> bool {
        let cutoff = now.checked_sub(Duration::from_secs(CASCADE_WINDOW_SECS))
            .unwrap_or(now);

        // Prune old entries
        self.window.retain(|r| r.recorded_at > cutoff);

        // Add new entry
        self.window.push(ActionRecord {
            recorded_at: now,
            authority: decision.authority,
            reason: decision.reason.clone(),
        });

        // Already paused — don't re-evaluate
        if self.is_paused_at(now) {
            return false;
        }

        // Server-startup exemption: if ALL recent actions have the exempt reason, skip
        let all_exempt = self.window.iter().all(|r| r.reason.contains(SERVER_STARTUP_EXEMPT_REASON));
        if all_exempt {
            return false;
        }

        // Count distinct authorities in the non-exempt window
        let distinct_authorities: HashSet<_> = self.window
            .iter()
            .filter(|r| !r.reason.contains(SERVER_STARTUP_EXEMPT_REASON))
            .map(|r| r.authority)
            .collect();

        if distinct_authorities.len() >= CASCADE_THRESHOLD {
            self.pause_until = Some(now + PAUSE_DURATION);

            // Build cascade summary for alert
            let summary = format!(
                "CASCADE DETECTED: {} recovery actions in {}s from {} different systems. Recovery paused {}min. Recent: {}",
                self.window.len(),
                CASCADE_WINDOW_SECS,
                distinct_authorities.len(),
                PAUSE_DURATION.as_secs() / 60,
                self.window.iter()
                    .take(5)
                    .map(|r| format!("{:?}:{}", r.authority, r.reason))
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            // Fire WhatsApp alert in background — do not block the caller.
            // Only spawn if a Tokio runtime is active (tests may run without one).
            if tokio::runtime::Handle::try_current().is_ok() {
                let alert_config = self.alert_config.clone();
                let client = self.http_client.clone();
                let msg = summary.clone();
                tokio::spawn(async move {
                    send_cascade_alert(&alert_config, &client, &msg).await;
                });
            }

            tracing::error!(target: "cascade_guard", "{}", summary);
            return true;
        }

        false
    }

    /// Returns true if automated recovery is currently paused.
    pub fn is_paused(&self) -> bool {
        self.is_paused_at(Instant::now())
    }

    fn is_paused_at(&self, now: Instant) -> bool {
        match self.pause_until {
            Some(until) => now < until,
            None => false,
        }
    }

    /// Manually resume recovery (staff cleared the cascade situation).
    pub fn resume(&mut self) {
        self.pause_until = None;
        tracing::info!(target: "cascade_guard", "Recovery resumed manually");
    }

    /// Remaining pause duration (for display in fleet dashboard).
    pub fn pause_remaining(&self) -> Option<Duration> {
        let now = Instant::now();
        self.pause_until.map(|until| {
            if until > now { until - now } else { Duration::ZERO }
        })
    }

    /// Test-only: record decision with explicit timestamp for time-travel testing.
    #[cfg(test)]
    pub fn record_with_ts(&mut self, decision: &RecoveryDecision, at: Instant) -> bool {
        self.record_at(decision, at)
    }
}

/// Send WhatsApp alert for cascade detection. Best-effort — warns on failure, never panics.
async fn send_cascade_alert(config: &CascadeAlertConfig, client: &reqwest::Client, message: &str) {
    let (evo_url, evo_key, evo_instance, phone) = match (
        &config.evolution_url,
        &config.evolution_api_key,
        &config.evolution_instance,
        &config.uday_phone,
    ) {
        (Some(url), Some(key), Some(inst), Some(phone)) => (url, key, inst, phone),
        _ => {
            tracing::warn!(target: "cascade_guard", "Evolution API not configured — cascade WA alert skipped");
            return;
        }
    };

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({ "number": phone, "text": message });

    match client.post(&url).header("apikey", evo_key).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(target: "cascade_guard", "Cascade WA alert sent to {}", phone);
        }
        Ok(resp) => {
            tracing::warn!(target: "cascade_guard", "Evolution API returned {} for cascade alert", resp.status());
        }
        Err(e) => {
            tracing::warn!(target: "cascade_guard", "Cascade WA alert failed: {}", e);
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::recovery::{RecoveryAction, RecoveryAuthority, RecoveryDecision};

    fn make_client() -> reqwest::Client {
        reqwest::Client::new()
    }

    fn make_guard() -> CascadeGuard {
        // No Evolution API configured — WA alert is a no-op in tests
        CascadeGuard::new(CascadeAlertConfig::empty(), make_client())
    }

    fn decision(authority: RecoveryAuthority, reason: &str) -> RecoveryDecision {
        RecoveryDecision::new("server", "rc-agent.exe", authority, RecoveryAction::Restart, reason)
    }

    // ── Core threshold tests ──────────────────────────────────────────────────

    #[test]
    fn two_actions_within_window_does_not_pause() {
        let mut guard = make_guard();
        let base = Instant::now();

        guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), base);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(5));

        assert!(!guard.is_paused(), "2 actions from 2 authorities should NOT trigger pause");
    }

    #[test]
    fn three_actions_three_different_authorities_triggers_pause() {
        let mut guard = make_guard();
        let base = Instant::now();

        guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), base);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(5));
        guard.record_with_ts(&decision(RecoveryAuthority::JamesMonitor, "crash"), base + Duration::from_secs(10));

        assert!(guard.is_paused(), "3 actions from 3 different authorities MUST trigger pause");
    }

    #[test]
    fn three_actions_same_authority_does_not_pause() {
        let mut guard = make_guard();
        let base = Instant::now();

        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(5));
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(10));

        assert!(!guard.is_paused(), "3 actions from SAME authority should NOT trigger pause");
    }

    // ── Server-startup exemption ──────────────────────────────────────────────

    #[test]
    fn eight_actions_server_startup_exempt_reason_does_not_pause() {
        let mut guard = make_guard();
        let base = Instant::now();

        // Simulate 8 pods all restarting because server just came up
        let authorities = [
            RecoveryAuthority::PodHealer,
            RecoveryAuthority::PodHealer,
            RecoveryAuthority::PodHealer,
            RecoveryAuthority::PodHealer,
            RecoveryAuthority::RcSentry,
            RecoveryAuthority::RcSentry,
            RecoveryAuthority::RcSentry,
            RecoveryAuthority::JamesMonitor,
        ];
        for (i, auth) in authorities.iter().enumerate() {
            guard.record_with_ts(
                &decision(*auth, "server_startup_recovery"),
                base + Duration::from_secs(i as u64),
            );
        }

        assert!(
            !guard.is_paused(),
            "All actions with 'server_startup_recovery' reason should NOT trigger pause"
        );
    }

    // ── Window expiry ─────────────────────────────────────────────────────────

    #[test]
    fn actions_outside_window_do_not_count() {
        let mut guard = make_guard();
        let base = Instant::now();

        // Record 3 actions from 3 different authorities
        guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), base);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(5));
        guard.record_with_ts(&decision(RecoveryAuthority::JamesMonitor, "crash"), base + Duration::from_secs(10));

        // Guard is now paused — simulate it being reset for the next check
        guard.resume();
        assert!(!guard.is_paused(), "after resume, guard should not be paused");

        // Now record 2 more actions at t+90s (outside the 60s window from the first burst)
        let t90 = base + Duration::from_secs(90);
        guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), t90);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), t90 + Duration::from_secs(5));

        assert!(
            !guard.is_paused(),
            "only 2 authorities in new window — should not pause"
        );
    }

    // ── Resume ────────────────────────────────────────────────────────────────

    #[test]
    fn resume_clears_pause() {
        let mut guard = make_guard();
        let base = Instant::now();

        guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), base);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(1));
        guard.record_with_ts(&decision(RecoveryAuthority::JamesMonitor, "crash"), base + Duration::from_secs(2));

        assert!(guard.is_paused(), "should be paused after cascade");
        guard.resume();
        assert!(!guard.is_paused(), "resume() must clear pause");
    }

    // ── Pause duration ────────────────────────────────────────────────────────

    #[test]
    fn pause_until_set_to_five_minutes_when_cascade_detected() {
        let mut guard = make_guard();
        let base = Instant::now();

        guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), base);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(1));
        guard.record_with_ts(&decision(RecoveryAuthority::JamesMonitor, "crash"), base + Duration::from_secs(2));

        assert!(guard.is_paused());
        // Remaining should be close to 5 minutes (300s)
        let remaining = guard.pause_remaining().expect("pause_remaining should be Some when paused");
        let secs = remaining.as_secs();
        // Allow up to 302s: pause_until = record_at + 300s, and pause_remaining() uses
        // Instant::now() which may be slightly before or after the record_with_ts timestamp.
        assert!(
            secs >= 295 && secs <= 302,
            "pause should be ~5 minutes, got {}s",
            secs
        );
    }

    // ── record() return value ─────────────────────────────────────────────────

    #[test]
    fn record_returns_true_when_cascade_just_triggered() {
        let mut guard = make_guard();
        let base = Instant::now();

        guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), base);
        guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(1));
        let triggered = guard.record_with_ts(&decision(RecoveryAuthority::JamesMonitor, "crash"), base + Duration::from_secs(2));

        assert!(triggered, "record() should return true when cascade is just triggered");
    }

    #[test]
    fn record_returns_false_when_no_cascade() {
        let mut guard = make_guard();
        let base = Instant::now();

        let r1 = guard.record_with_ts(&decision(RecoveryAuthority::RcSentry, "crash"), base);
        let r2 = guard.record_with_ts(&decision(RecoveryAuthority::PodHealer, "crash"), base + Duration::from_secs(1));

        assert!(!r1, "first record should return false");
        assert!(!r2, "second record (only 2 authorities) should return false");
    }
}
