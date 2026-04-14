/// Reconnect replay: resend unacked config pushes when a pod reconnects.

use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage};
use rc_common::types::ConfigPushPayload;

use super::config_push_types::ConfigQueueEntry;

/// Replay all unacked config pushes for a pod that just reconnected.
///
/// IMPORTANT: Uses `status != 'acked'` as the filter — NOT a sequence number comparison.
/// FlagCacheSync.cached_version is a FLAG version counter, not a config push sequence number.
/// Using it as last_seq would silently skip valid config pushes.
pub async fn replay_pending_config_pushes(
    state: &AppState,
    pod_id: &str,
    cmd_tx: &mpsc::Sender<CoreMessage>,
) {
    let rows = sqlx::query_as::<_, ConfigQueueEntry>(
        "SELECT id, pod_id, payload, seq_num, status, created_at, acked_at \
         FROM config_push_queue \
         WHERE pod_id = ? AND status != 'acked' \
         ORDER BY seq_num ASC",
    )
    .bind(pod_id)
    .fetch_all(&state.db)
    .await;

    let rows = match rows {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to query pending config pushes for pod {}: {}", pod_id, e);
            return;
        }
    };

    for entry in rows {
        let fields: HashMap<String, serde_json::Value> = match serde_json::from_str(&entry.payload) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(
                    "Skipping malformed config push payload for pod {} seq={}: {}",
                    pod_id, entry.seq_num, e
                );
                continue;
            }
        };

        let push_payload = ConfigPushPayload {
            fields,
            schema_version: 1,
            sequence: entry.seq_num as u64,
        };

        match cmd_tx.send(CoreMessage::wrap(CoreToAgentMessage::ConfigPush(push_payload))).await {
            Ok(_) => {
                tracing::info!(
                    "Replayed config push seq={} to reconnected pod {}",
                    entry.seq_num, pod_id
                );
                let _ = sqlx::query(
                    "UPDATE config_push_queue SET status = 'delivered' WHERE id = ?",
                )
                .bind(entry.id)
                .execute(&state.db)
                .await;
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to replay config push seq={} to pod {}: {}",
                    entry.seq_num, pod_id, e
                );
            }
        }
    }
}
