use std::sync::Arc;

use crate::state::AppState;
use rc_common::types::{FriendInfo, FriendRequestInfo};

/// Send a friend request by phone number or customer_id (e.g. "RP001").
pub async fn send_friend_request(
    state: &Arc<AppState>,
    sender_id: &str,
    identifier: &str,
) -> Result<String, String> {
    // Look up receiver by phone, customer_id, nickname, or exact name (registered only)
    let receiver: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE (phone = ? OR customer_id = ? OR nickname = ? OR LOWER(name) = LOWER(?)) AND registration_completed = 1 LIMIT 1",
    )
    .bind(identifier)
    .bind(identifier)
    .bind(identifier)
    .bind(identifier)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let receiver_id = receiver.ok_or("User not found")?.0;

    if sender_id == receiver_id {
        return Err("Cannot add yourself".to_string());
    }

    // Check existing friendship
    let (a, b) = canonical_pair(sender_id, &receiver_id);
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM friendships WHERE driver_a_id = ? AND driver_b_id = ?",
    )
    .bind(&a)
    .bind(&b)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if existing.is_some() {
        return Err("Already friends".to_string());
    }

    // Check duplicate pending request
    let pending: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM friend_requests
         WHERE sender_id = ? AND receiver_id = ? AND status = 'pending'",
    )
    .bind(sender_id)
    .bind(&receiver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if pending.is_some() {
        return Err("Request already sent".to_string());
    }

    // Check if the other person already sent us a request — auto-accept
    let reverse: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM friend_requests
         WHERE sender_id = ? AND receiver_id = ? AND status = 'pending'",
    )
    .bind(&receiver_id)
    .bind(sender_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if let Some((reverse_id,)) = reverse {
        // Auto-accept the reverse request
        accept_friend_request(state, &reverse_id, sender_id).await?;
        return Ok(reverse_id);
    }

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO friend_requests (id, sender_id, receiver_id, status) VALUES (?, ?, ?, 'pending')",
    )
    .bind(&id)
    .bind(sender_id)
    .bind(&receiver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    tracing::info!("Friend request {} → {}", sender_id, receiver_id);
    Ok(id)
}

/// Accept a friend request. Creates the bidirectional friendship.
pub async fn accept_friend_request(
    state: &Arc<AppState>,
    request_id: &str,
    receiver_id: &str,
) -> Result<(), String> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT id, sender_id, receiver_id FROM friend_requests WHERE id = ? AND status = 'pending'",
    )
    .bind(request_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let (_, sender_id, req_receiver_id) = row.ok_or("Request not found or not pending")?;

    if req_receiver_id != receiver_id {
        return Err("Not your request to accept".to_string());
    }

    // Update request
    sqlx::query("UPDATE friend_requests SET status = 'accepted', resolved_at = datetime('now') WHERE id = ?")
        .bind(request_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    // Create friendship (canonical order)
    let (a, b) = canonical_pair(&sender_id, receiver_id);
    let friendship_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT OR IGNORE INTO friendships (id, driver_a_id, driver_b_id, request_id) VALUES (?, ?, ?, ?)",
    )
    .bind(&friendship_id)
    .bind(&a)
    .bind(&b)
    .bind(request_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    tracing::info!("Friendship created: {} <-> {}", sender_id, receiver_id);
    Ok(())
}

/// Reject a friend request.
pub async fn reject_friend_request(
    state: &Arc<AppState>,
    request_id: &str,
    receiver_id: &str,
) -> Result<(), String> {
    let result = sqlx::query(
        "UPDATE friend_requests SET status = 'rejected', resolved_at = datetime('now')
         WHERE id = ? AND receiver_id = ? AND status = 'pending'",
    )
    .bind(request_id)
    .bind(receiver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if result.rows_affected() == 0 {
        return Err("Request not found or not pending".to_string());
    }
    Ok(())
}

/// Remove a friend.
pub async fn remove_friend(
    state: &Arc<AppState>,
    driver_id: &str,
    friend_driver_id: &str,
) -> Result<(), String> {
    let (a, b) = canonical_pair(driver_id, friend_driver_id);
    let result = sqlx::query("DELETE FROM friendships WHERE driver_a_id = ? AND driver_b_id = ?")
        .bind(&a)
        .bind(&b)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    if result.rows_affected() == 0 {
        return Err("Friendship not found".to_string());
    }
    tracing::info!("Friendship removed: {} <-> {}", driver_id, friend_driver_id);
    Ok(())
}

/// List all friends for a driver with online status and stats.
pub async fn list_friends(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<Vec<FriendInfo>, String> {
    let rows: Vec<(String, String, Option<String>, Option<String>, i64, i64, i64)> = sqlx::query_as(
        "SELECT d.id, d.name, d.customer_id, d.presence,
                COALESCE(d.total_laps, 0),
                COALESCE(d.total_time_ms, 0),
                (SELECT COUNT(*) FROM billing_sessions WHERE driver_id = d.id)
         FROM drivers d
         INNER JOIN friendships f ON (
             (f.driver_a_id = ? AND f.driver_b_id = d.id) OR
             (f.driver_b_id = ? AND f.driver_a_id = d.id)
         )",
    )
    .bind(driver_id)
    .bind(driver_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(rows
        .into_iter()
        .map(|(id, name, customer_id, presence, total_laps, total_time_ms, session_count)| FriendInfo {
            driver_id: id,
            name,
            customer_id,
            is_online: presence.as_deref() == Some("online"),
            total_laps,
            total_time_ms,
            session_count,
        })
        .collect())
}

/// List pending friend requests (incoming and outgoing).
pub async fn list_friend_requests(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<(Vec<FriendRequestInfo>, Vec<FriendRequestInfo>), String> {
    // Incoming
    let incoming: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT fr.id, d.id, d.name, d.customer_id, fr.created_at
         FROM friend_requests fr
         INNER JOIN drivers d ON d.id = fr.sender_id
         WHERE fr.receiver_id = ? AND fr.status = 'pending'
         ORDER BY fr.created_at DESC",
    )
    .bind(driver_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let incoming: Vec<FriendRequestInfo> = incoming
        .into_iter()
        .map(|(id, did, name, cid, created)| FriendRequestInfo {
            id,
            driver_id: did,
            driver_name: name,
            customer_id: cid,
            direction: "incoming".to_string(),
            created_at: created,
        })
        .collect();

    // Outgoing
    let outgoing: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT fr.id, d.id, d.name, d.customer_id, fr.created_at
         FROM friend_requests fr
         INNER JOIN drivers d ON d.id = fr.receiver_id
         WHERE fr.sender_id = ? AND fr.status = 'pending'
         ORDER BY fr.created_at DESC",
    )
    .bind(driver_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let outgoing: Vec<FriendRequestInfo> = outgoing
        .into_iter()
        .map(|(id, did, name, cid, created)| FriendRequestInfo {
            id,
            driver_id: did,
            driver_name: name,
            customer_id: cid,
            direction: "outgoing".to_string(),
            created_at: created,
        })
        .collect();

    Ok((incoming, outgoing))
}

/// Set driver presence (online/hidden).
pub async fn set_presence(
    state: &Arc<AppState>,
    driver_id: &str,
    presence: &str,
) -> Result<(), String> {
    if presence != "online" && presence != "hidden" {
        return Err("Invalid presence value".to_string());
    }

    sqlx::query("UPDATE drivers SET presence = ? WHERE id = ?")
        .bind(presence)
        .bind(driver_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    Ok(())
}

/// Canonical pair ordering for friendship uniqueness.
fn canonical_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b { (a, b) } else { (b, a) }
}
