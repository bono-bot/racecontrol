#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use axum::{
    Json,
    extract::State,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::billing;
use crate::catalog;
use crate::pod_reservation;
use crate::wallet;
use crate::state::AppState;
use rc_common::types::*;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Continue Session (Multi-Sub-Session) ───────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ContinueSessionRequest {
    experience_id: String,
    pricing_tier_id: String,
}

pub(crate) async fn customer_continue_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ContinueSessionRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Must have an active reservation
    let reservation = match pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        Some(r) => r,
        None => return Json(json!({ "error": "No active reservation. Book a new session instead." })),
    };

    // Must not have active billing on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(&reservation.pod_id) {
            return Json(json!({ "error": "A session is still active on this pod" }));
        }
    }

    // Get pricing tier
    let tier = match sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "error": "Invalid pricing tier" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let price_paise = tier.3;

    // Debit wallet
    if price_paise > 0 {
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": e })),
        };

        if balance < price_paise {
            return Json(json!({
                "error": "Insufficient wallet balance",
                "balance_paise": balance,
                "required_paise": price_paise,
            }));
        }

        match wallet::debit(
            &state,
            &driver_id,
            price_paise,
            "debit_session",
            None,
            Some(&format!("Continue: {}", tier.1)),
        )
        .await
        {
            Ok(_) => {}
            Err(e) => return Json(json!({ "error": e })),
        }
    }

    // Touch reservation
    pod_reservation::touch_reservation(&state, &reservation.id).await;

    // Start billing session directly (skip auth token — customer is already at pod)
    let billing_session_id = match billing::start_billing_session(
        &state,
        reservation.pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        None,
        None,
        None, // customer-initiated continue
        None, // split_count
        None, // split_duration_minutes
    )
    .await
    {
        Ok(id) => id,
        Err(reason) => {
            // Refund on failure
            if price_paise > 0 {
                let _ = wallet::refund(&state, &driver_id, price_paise, None, Some("Continue failed — auto-refund")).await;
            }
            return Json(json!({ "error": reason }));
        }
    };

    // Link billing session to reservation and record wallet debit
    let _ = sqlx::query(
        "UPDATE billing_sessions SET reservation_id = ?, wallet_debit_paise = ? WHERE id = ?",
    )
    .bind(&reservation.id)
    .bind(price_paise)
    .bind(&billing_session_id)
    .execute(&state.db)
    .await;

    // Auto-launch game
    let exp = sqlx::query_as::<_, (String, String, String)>(
        "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&req.experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((game, track, car)) = exp {
        let sim_type = match game.as_str() {
            "assetto_corsa" | "ac" => SimType::AssettoCorsa,
            "iracing" => SimType::IRacing,
            "f1_25" | "f1" => SimType::F125,
            "le_mans_ultimate" | "lmu" => SimType::LeMansUltimate,
            "forza" => SimType::Forza,
            _ => SimType::AssettoCorsa,
        };

        // Check if this game supports auto-spawn
        let needs_assistance = matches!(sim_type, SimType::F125);

        // Clone the sender once; use it in whichever branch needs to send.
        let sender = {
            let agent_senders = state.agent_senders.read().await;
            agent_senders.get(&reservation.pod_id).cloned()
        };
        if needs_assistance {
            // Send assistance screen instead of launching
            if let Some(ref s) = sender {
                let _ = s.send(CoreMessage::wrap(rc_common::protocol::CoreToAgentMessage::ShowAssistanceScreen {
                    driver_name: driver_id.clone(),
                    message: "A team member is on the way to help launch your game.".to_string(),
                })).await;
            }
            let _ = state.dashboard_tx.send(DashboardEvent::AssistanceNeeded {
                pod_id: reservation.pod_id.clone(),
                driver_name: driver_id.clone(),
                game: game.clone(),
                reason: "Game requires manual launch".to_string(),
            });
        } else {
            // Validate car/track combo against pod's content manifest
            let manifest = state.pod_manifests.read().await.get(&reservation.pod_id).cloned();
            if let Err(reason) = catalog::validate_launch_combo(manifest.as_ref(), &car, &track, "") {
                tracing::warn!("customer_book_session: launch rejected for pod {}: {}", reservation.pod_id, reason);
                crate::activity_log::log_pod_activity(&state, &reservation.pod_id, "content", "Launch Rejected", &reason, "core", None);
            } else {
                let launch_args = serde_json::json!({
                    "car": car, "track": track, "driver": "Driver",
                    "transmission": "auto",
                    "aids": { "abs": 1, "tc": 1, "stability": 1, "autoclutch": 1, "ideal_line": 1 },
                    "conditions": { "damage": 0 }
                }).to_string();
                if let Some(ref s) = sender {
                    let _ = s.send(CoreMessage::wrap(rc_common::protocol::CoreToAgentMessage::LaunchGame {
                        sim_type,
                        launch_args: Some(launch_args),
                        force_clean: false,
                        duration_minutes: None,
                        launch_id: None,
                    })).await;
                }
            }
        }

        // Update billing session with experience info
        let _ = sqlx::query(
            "UPDATE billing_sessions SET experience_id = ?, car = ?, track = ?, sim_type = ? WHERE id = ?",
        )
        .bind(&req.experience_id)
        .bind(&car)
        .bind(&track)
        .bind(&game)
        .bind(&billing_session_id)
        .execute(&state.db)
        .await;
    }

    Json(json!({
        "status": "ok",
        "billing_session_id": billing_session_id,
        "reservation_id": reservation.id,
        "pod_id": reservation.pod_id,
    }))
}
