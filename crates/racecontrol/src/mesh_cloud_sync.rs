//! Multi-Venue Cloud KB Sync — solutions sync to Bono VPS for cross-venue sharing.
//!
//! Phase 239 — CLOUD-01, CLOUD-02, CLOUD-03.
//!
//! Venue server pushes fleet-verified + hardened solutions to cloud every 30 min.
//! New venue can pull entire KB on day 1 (zero cold-start).

use serde::{Deserialize, Serialize};

const LOG_TARGET: &str = "mesh-cloud-sync";

/// Payload for cloud sync push (venue → cloud)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncPayload {
    pub venue_id: String,
    pub solutions: Vec<CloudSolution>,
    pub synced_at: String,
}

/// Solution in cloud-portable format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSolution {
    pub problem_hash: String,
    pub problem_key: String,
    pub root_cause: String,
    pub fix_action: String,
    pub fix_type: String,
    pub confidence: f64,
    pub source_venue: String,
    pub promotion_status: String,
    pub success_count: i64,
}

/// CLOUD-03: Import cloud solutions — cross-venue confidence is capped.
pub fn import_cloud_solutions(payload: &CloudSyncPayload) -> Vec<CloudSolution> {
    tracing::info!(
        target: LOG_TARGET,
        venue = %payload.venue_id,
        solutions = payload.solutions.len(),
        "Importing cloud solutions into local fleet KB"
    );
    payload
        .solutions
        .iter()
        .map(|s| {
            let mut imported = s.clone();
            imported.confidence = (s.confidence * 0.8).min(0.79);
            imported.source_venue = format!("cloud:{}", payload.venue_id);
            imported
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_caps_confidence() {
        let payload = CloudSyncPayload {
            venue_id: "venue_1".to_string(),
            solutions: vec![CloudSolution {
                problem_hash: "h1".to_string(),
                problem_key: "ws_disconnect".to_string(),
                root_cause: "sentinel".to_string(),
                fix_action: "clear".to_string(),
                fix_type: "deterministic".to_string(),
                confidence: 0.95,
                source_venue: "venue_1".to_string(),
                promotion_status: "fleet_verified".to_string(),
                success_count: 5,
            }],
            synced_at: "2026-03-27".to_string(),
        };
        let imported = import_cloud_solutions(&payload);
        assert_eq!(imported.len(), 1);
        assert!(imported[0].confidence <= 0.79);
        assert!(imported[0].source_venue.starts_with("cloud:"));
    }
}
