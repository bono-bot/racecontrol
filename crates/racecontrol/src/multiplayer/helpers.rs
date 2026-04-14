use std::sync::Arc;

use crate::state::AppState;
use rc_common::types::{GroupMemberInfo, GroupSessionInfo};

/// Check if all invitees have responded (accepted or declined). If so, set status to 'ready'.
pub(crate) async fn check_all_responded(state: &Arc<AppState>, group_session_id: &str) {
    let pending: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM group_session_members
         WHERE group_session_id = ? AND status = 'pending'",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if pending.map(|r| r.0).unwrap_or(1) == 0 {
        let _ = sqlx::query(
            "UPDATE group_sessions SET status = 'ready' WHERE id = ? AND status = 'forming'",
        )
        .bind(group_session_id)
        .execute(&state.db)
        .await;
    }
}

/// Build full GroupSessionInfo from DB.
pub(crate) async fn build_group_session_info(
    state: &Arc<AppState>,
    group_session_id: &str,
) -> Result<GroupSessionInfo, String> {
    let session = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT gs.id, gs.host_driver_id, gs.experience_id, gs.shared_pin, gs.status, gs.created_at
         FROM group_sessions gs WHERE gs.id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Group session not found")?;

    let (id, host_driver_id, experience_id, shared_pin, status, created_at) = session;

    let host_name = get_driver_name(state, &host_driver_id).await;

    let experience_name: String = sqlx::query_scalar(
        "SELECT name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    // Get members
    let member_rows: Vec<(String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT gsm.driver_id, gsm.role, gsm.pod_id, gsm.status, d.name
         FROM group_session_members gsm
         INNER JOIN drivers d ON d.id = gsm.driver_id
         WHERE gsm.group_session_id = ?
         ORDER BY gsm.role DESC, gsm.invited_at",
    )
    .bind(group_session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let mut members = Vec::new();
    for (driver_id, role, pod_id, member_status, name) in member_rows {
        let customer_id = get_customer_id(state, &driver_id).await;
        let pod_number = if let Some(ref pid) = pod_id {
            get_pod_number(state, pid).await
        } else {
            None
        };

        members.push(GroupMemberInfo {
            driver_id,
            driver_name: name.unwrap_or_else(|| "Unknown".to_string()),
            customer_id,
            role,
            status: member_status,
            pod_id,
            pod_number,
        });
    }

    let pricing_tier_name: String = sqlx::query_scalar(
        "SELECT pt.name FROM pricing_tiers pt
         INNER JOIN group_sessions gs ON gs.pricing_tier_id = pt.id
         WHERE gs.id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    // Query enrichment fields from group_sessions (track, car, ai_count added in Phase 9)
    let enrichment: Option<(Option<String>, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT track, car, ai_count FROM group_sessions WHERE id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (track, car, ai_count) = enrichment.unwrap_or((None, None, None));

    // Query difficulty_tier from experience (if available)
    let difficulty_tier: Option<String> = sqlx::query_scalar(
        "SELECT difficulty_tier FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Ok(GroupSessionInfo {
        id,
        host_driver_id,
        host_name,
        experience_name,
        pricing_tier_name,
        shared_pin,
        status,
        members,
        created_at,
        track,
        car,
        ai_count: ai_count.map(|c| c as u32),
        difficulty_tier,
    })
}

pub(crate) async fn get_driver_name(state: &Arc<AppState>, driver_id: &str) -> String {
    sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Unknown".to_string())
}

pub(crate) async fn get_customer_id(state: &Arc<AppState>, driver_id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT customer_id FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
}

pub(crate) async fn get_pod_number(state: &Arc<AppState>, pod_id: &str) -> Option<u32> {
    let pods = state.pods.read().await;
    pods.get(pod_id).map(|p| p.number)
}

pub(crate) fn canonical_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b { (a, b) } else { (b, a) }
}

/// Get driver name and steam_guid for AC entry list population.
pub(crate) async fn get_driver_entry_info(state: &Arc<AppState>, driver_id: &str) -> (String, String) {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT name, steam_guid FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some((name, guid)) => (
            name.unwrap_or_else(|| "Driver".to_string()),
            guid.unwrap_or_default(),
        ),
        None => ("Driver".to_string(), String::new()),
    }
}
