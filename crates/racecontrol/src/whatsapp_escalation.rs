//! WhatsApp Tier 5 escalation handler — receives EscalationRequest from pods,
//! deduplicates by incident_id (30-min suppression), sends WhatsApp via Bono
//! relay (localhost:8766), falls back to comms-link INBOX.md on failure.
//!
//! WhatsApp routing: Pod -> WS -> Server -> Bono relay (localhost:8766) -> VPS -> Evolution API
//! NEVER direct from server to Evolution API (standing rule).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use rc_common::protocol::EscalationPayload;

const LOG_TARGET: &str = "whatsapp_escalation";

/// 30-minute dedup suppression window for same incident_id
const DEDUP_TTL_SECS: u64 = 1800;

/// Stale entry cleanup threshold
const DEDUP_CLEANUP_THRESHOLD: usize = 50;

/// WhatsApp Tier 5 escalation handler.
///
/// Shared across all WS handler tasks via `Arc<WhatsAppEscalation>`.
/// Dedup map uses std::sync::Mutex (not tokio) because escalations are rare
/// and the critical section is sub-microsecond (no async work under lock).
pub struct WhatsAppEscalation {
    /// incident_id -> last_sent timestamp for 30-min suppression
    dedup: Mutex<HashMap<String, Instant>>,
    /// HTTP client for Bono relay calls
    client: reqwest::Client,
    /// Uday's WhatsApp number (international, no +)
    uday_number: String,
    /// Bono relay URL
    relay_url: String,
    /// Comms-link repo path for INBOX.md fallback
    comms_link_path: PathBuf,
}

impl WhatsAppEscalation {
    /// Create a new escalation handler using the shared HTTP client.
    ///
    /// - `uday_number`: from env `ESCALATION_WHATSAPP_NUMBER`, default `"919059833001"`
    /// - `relay_url`: Bono relay at `http://localhost:8766/relay/exec/run`
    /// - `comms_link_path`: fallback git repo for INBOX.md
    pub fn new(client: reqwest::Client) -> Self {
        let uday_number = std::env::var("ESCALATION_WHATSAPP_NUMBER")
            .unwrap_or_else(|_| "919059833001".to_string());

        let relay_url = "http://localhost:8766/relay/exec/run".to_string();

        let comms_link_path = PathBuf::from("C:/Users/bono/racingpoint/comms-link");

        Self {
            dedup: Mutex::new(HashMap::new()),
            client,
            uday_number,
            relay_url,
            comms_link_path,
        }
    }

    /// Remove stale entries older than 30 min from the dedup map.
    /// Called inline since escalations are rare events.
    fn cleanup_stale(&self) {
        let mut map = self.dedup.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() > DEDUP_CLEANUP_THRESHOLD {
            map.retain(|_, v| v.elapsed() < Duration::from_secs(DEDUP_TTL_SECS));
        }
    }

    /// Check if this incident_id is a duplicate (within 30-min window).
    /// Returns `true` if this is a duplicate that should be suppressed.
    /// Inserts/updates the timestamp if not a duplicate.
    fn is_duplicate(&self, incident_id: &str) -> bool {
        let mut map = self.dedup.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(last_sent) = map.get(incident_id) {
            if last_sent.elapsed() < Duration::from_secs(DEDUP_TTL_SECS) {
                return true; // suppress
            }
        }
        map.insert(incident_id.to_string(), Instant::now());
        false
    }

    /// Main entry point: handle an incoming EscalationRequest from a pod.
    ///
    /// 1. Dedup check (30-min suppression per incident_id)
    /// 2. Format WhatsApp message
    /// 3. Send via Bono relay
    /// 4. Fallback to INBOX.md if relay fails
    pub async fn handle_escalation(&self, payload: EscalationPayload) {
        // Periodic cleanup
        self.cleanup_stale();

        // Step A: Dedup check (ESC-03)
        if self.is_duplicate(&payload.incident_id) {
            tracing::info!(
                target: LOG_TARGET,
                incident_id = %payload.incident_id,
                pod_id = %payload.pod_id,
                "Suppressing duplicate escalation for incident (30-min window)"
            );
            return;
        }

        // Step B: Format WhatsApp message (ESC-02)
        let message = format!(
            "*Tier 5 Escalation*\n\
             Severity: {}\n\
             Pod: {}\n\
             Issue: {}\n\
             AI tried: {}\n\
             Impact: {}\n\
             Dashboard: {}",
            payload.severity,
            payload.pod_id,
            payload.summary,
            payload.actions_tried.join(", "),
            payload.impact,
            payload.dashboard_url,
        );

        // Step C: Send via Bono relay (ESC-01, ESC-04)
        let relay_result = self.send_via_relay(&message, &payload.incident_id).await;

        // Step D: Fallback to INBOX.md if relay failed (ESC-04)
        if let Err(relay_err) = relay_result {
            tracing::error!(
                target: LOG_TARGET,
                incident_id = %payload.incident_id,
                error = %relay_err,
                "Bono relay failed — falling back to INBOX.md"
            );
            self.fallback_inbox(&payload).await;
        }
    }

    /// Send WhatsApp message via Bono relay POST.
    async fn send_via_relay(&self, message: &str, incident_id: &str) -> Result<(), String> {
        let body = serde_json::json!({
            "command": "whatsapp_send",
            "reason": "Tier 5 escalation",
            "params": {
                "number": self.uday_number,
                "message": message,
            }
        });

        let resp = self
            .client
            .post(&self.relay_url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("relay HTTP error: {}", e))?;

        let status = resp.status();
        if status.is_success() {
            tracing::warn!(
                target: LOG_TARGET,
                incident_id = %incident_id,
                number = %self.uday_number,
                "WhatsApp escalation sent via Bono relay"
            );
            Ok(())
        } else {
            let body_text = resp.text().await.unwrap_or_default();
            Err(format!("relay returned {} — {}", status, body_text))
        }
    }

    /// Fallback: append escalation to INBOX.md and git push.
    async fn fallback_inbox(&self, payload: &EscalationPayload) {
        let ist_now = chrono::Utc::now()
            .with_timezone(&chrono_tz::Asia::Kolkata)
            .format("%Y-%m-%d %H:%M IST")
            .to_string();

        let entry = format!(
            "\n## {} -- from james (auto-escalation)\n\n\
             **Tier 5 Escalation** -- incident {}\n\
             - Severity: {}\n\
             - Pod: {}\n\
             - Issue: {}\n\
             - AI tried: {}\n\
             - Impact: {}\n\
             - Dashboard: {}\n\n\
             WhatsApp delivery FAILED -- this is the fallback notification.\n",
            ist_now,
            payload.incident_id,
            payload.severity,
            payload.pod_id,
            payload.summary,
            payload.actions_tried.join(", "),
            payload.impact,
            payload.dashboard_url,
        );

        let inbox_path = self.comms_link_path.join("INBOX.md");

        // Append to INBOX.md
        match tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&inbox_path)
            .await
        {
            Ok(mut file) => {
                use tokio::io::AsyncWriteExt;
                if let Err(e) = file.write_all(entry.as_bytes()).await {
                    tracing::error!(
                        target: LOG_TARGET,
                        incident_id = %payload.incident_id,
                        error = %e,
                        "Failed to write INBOX.md fallback"
                    );
                    return;
                }
            }
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    incident_id = %payload.incident_id,
                    error = %e,
                    "Failed to open INBOX.md for fallback"
                );
                return;
            }
        }

        // Git add + commit + push
        let git_dir = self.comms_link_path.to_string_lossy().to_string();
        let commit_msg = format!("auto-escalation: {}", payload.incident_id);

        let git_result = tokio::process::Command::new("git")
            .args(["-C", &git_dir, "add", "INBOX.md"])
            .output()
            .await;

        if let Err(e) = git_result {
            tracing::error!(target: LOG_TARGET, error = %e, "git add INBOX.md failed");
            return;
        }

        let commit_result = tokio::process::Command::new("git")
            .args(["-C", &git_dir, "commit", "-m", &commit_msg])
            .output()
            .await;

        if let Err(e) = commit_result {
            tracing::error!(target: LOG_TARGET, error = %e, "git commit failed");
            return;
        }

        let push_result = tokio::process::Command::new("git")
            .args(["-C", &git_dir, "push"])
            .output()
            .await;

        match push_result {
            Ok(output) if output.status.success() => {
                tracing::warn!(
                    target: LOG_TARGET,
                    incident_id = %payload.incident_id,
                    "INBOX.md fallback: written + committed + pushed"
                );
            }
            Ok(output) => {
                tracing::error!(
                    target: LOG_TARGET,
                    incident_id = %payload.incident_id,
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    "CRITICAL: git push failed for INBOX.md fallback"
                );
            }
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    incident_id = %payload.incident_id,
                    error = %e,
                    "CRITICAL: Both WhatsApp and INBOX.md fallback failed"
                );
            }
        }
    }
}
