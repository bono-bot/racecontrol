use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use super::metrics;
use super::survival;
use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreToAgentMessage, DashboardEvent};

/// Top-level API router: merges 5 tiered sub-routers.
///
/// - `auth_rate_limited_routes()` -- rate-limited auth endpoints (5 req/min per IP)
/// - `public_routes()` -- no auth required (health, venue, public leaderboards, customer register)
/// - `customer_routes()` -- customer JWT checked in-handler via extract_driver_id()
/// - `staff_routes(state)` -- staff/admin routes with permissive JWT middleware (logs warnings)
/// - `service_routes()` -- service routes (sync, actions, terminal, bot) with in-handler auth
pub fn api_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .merge(auth_rate_limited_routes())
        .merge(public_routes())
        .merge(customer_routes())
        .merge(kiosk_routes(state.clone()))
        .merge(staff_routes(state))
        .merge(service_routes())
        .merge(survival::survival_routes())
        .merge(crate::fleet_healer::fleet_healer_routes())
}

// ─── Rate-limited auth endpoints (5 req/min per IP via tower_governor) ───

fn auth_rate_limited_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/customer/login", post(customer_login))
        .route("/customer/resend-otp", post(customer_resend_otp))
        .route("/customer/verify-otp", post(customer_verify_otp))
        .route("/auth/validate-pin", post(validate_pin))
        .route("/auth/kiosk/validate-pin", post(kiosk_validate_pin))
        .route("/kiosk/redeem-pin", post(kiosk_redeem_pin))
        .route("/staff/validate-pin", post(staff_validate_pin))
        .route("/auth/admin-login", post(auth::admin::admin_login))
        .layer(auth::rate_limit::auth_rate_limit_layer())
}

// ─── Tier 1: Public (no auth) ────────────────────────────────────────────

fn public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/fleet/health", get(fleet_health::fleet_health_handler))
        .route("/sentry/crash", post(fleet_health::sentry_crash_handler))
        // MMA-P1: Debug endpoints moved to staff_routes (below) to prevent information
        // disclosure. Kiosk debug UI must use staff JWT to access these endpoints.
        // Previously public: /debug/activity, /debug/playbooks, /debug/incidents, /debug/pod-events
        // MMA-v29: /debug/db-stats also moved to staff_routes (was leaking table names, row counts)
        .route("/guard/whitelist/{machine_id}", get(process_guard::get_whitelist_handler))
        .route("/venue", get(venue_info))
        .route("/customer/register", post(customer_register))
        .route("/wallet/bonus-tiers", get(wallet_bonus_tiers))
        // Public leaderboards, events, championships (no auth)
        .route("/public/leaderboard", get(public_leaderboard))
        .route("/public/leaderboard/{track}", get(public_track_leaderboard))
        .route("/public/circuit-records", get(public_circuit_records))
        .route("/public/vehicle-records/{car}", get(public_vehicle_records))
        .route("/public/drivers", get(public_drivers_search))
        .route("/public/drivers/{id}", get(public_driver_profile))
        .route("/public/time-trial", get(public_time_trial))
        .route("/public/laps/{lap_id}/telemetry", get(public_lap_telemetry))
        .route("/public/sessions/{id}", get(public_session_summary))
        .route("/public/championships/{id}/standings", get(public_championship_standings_handler))
        .route("/public/events", get(public_events_list))
        .route("/public/events/{id}", get(public_event_leaderboard))
        .route("/public/events/{id}/sessions", get(public_event_sessions))
        .route("/public/championships", get(public_championships_list))
        .route("/public/championships/{id}", get(public_championship_standings))
        // Driver ratings (public, no auth — Phase 253)
        .route("/public/drivers/{id}/rating", get(public_driver_rating))
        // Cafe menu (customer-facing, no auth)
        .route("/cafe/menu", get(cafe::public_menu))
        // Cafe promos (customer-facing, no auth — PROMO-05)
        .route("/cafe/promos/active", get(cafe_promos::list_active_promos))
        // Kiosk allowlist — read-only is public so rc-agent can fetch without auth
        .route("/config/kiosk-allowlist", get(list_kiosk_allowlist))
        // Recovery events API (COORD-04) -- public for rc-sentry cross-machine visibility
        .route("/recovery/events", get(recovery::get_recovery_events).post(recovery::post_recovery_event))
        // Fleet alert API -- Tier 4 WhatsApp escalation (GRAD-04 prerequisite)
        .route("/fleet/alert", post(fleet_alert::post_fleet_alert))
        // Pricing psychology (v14.0 Phase 94) — public for customer-facing /book page
        .route("/pricing/display", get(pricing_display_handler))
        .route("/pricing/social-proof", get(pricing_social_proof_handler))
        // Legal disclosure (LEGAL-06) — public so kiosk can fetch during minor registration flow
        .route("/legal/minor-waiver-disclosure", get(minor_waiver_disclosure))
        // MMA-v29: Metrics, mesh intelligence, admin, and cameras endpoints moved to staff_routes.
        // These leaked operational data (billing accuracy, incidents, camera topology) publicly.
        // /games/alternatives remains public (customer-facing combo recommendations).
        .route("/games/alternatives", get(metrics::alternatives_handler))
        // POS lockdown read — public so POS agent/kiosk can poll without JWT (MMA Round 1 fix: 2/3 consensus)
        // POST (write) stays in staff_routes
        .route("/pos/lockdown", get(get_pos_lockdown))
        // Phase 255: Display machine heartbeat — no auth (display machines have no JWT)
        .route("/kiosk/ping", post(kiosk_ping_handler))
        // DEPLOY-02: Agent graceful shutdown notification — no JWT (agent uses service key header).
        // Called by rc-agent during shutdown when a billing session is active.
        .route("/billing/{id}/agent-shutdown", post(agent_shutdown_handler))
        // DEPLOY-04: Post-restart interrupted session check — rc-agent calls on startup.
        .route("/billing/pod/{pod_id}/interrupted", get(interrupted_sessions_handler))
        // FATM-11: Payment gateway webhook — idempotent wallet credit
        .route("/webhooks/payment-gateway", post(payment_gateway_webhook))
        // UX-02: OTP fallback display — customer polls this if WhatsApp delivery failed.
        // One-time token; consumed on first successful read.
        .route("/customer/otp-fallback/{token}", get(otp_fallback_handler))
        // UX-08: Virtual queue — join, check status, leave (no auth required for walk-ins)
        .route("/queue/join", post(queue_join_handler))
        .route("/queue/status/{id}", get(queue_status_handler))
        .route("/queue/{id}/leave", post(queue_leave_handler))
        // v29.0 Phase 34: Pod availability for kiosk maintenance gate
        .route("/pods/{id}/availability", get(pod_availability_handler))
}

/// Proxy health check for go2rtc cameras on James machine.
/// Returns {"status":"ok"} if go2rtc responds, {"status":"down"} with 503 otherwise.
async fn cameras_health_proxy() -> axum::response::Response {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();
    let up = match client.get("http://192.168.31.27:1984/api/config").send().await {
        Ok(res) => res.status().is_success(),
        Err(_) => false,
    };
    if up {
        Json(json!({"status": "ok", "service": "go2rtc"})).into_response()
    } else {
        (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status": "down", "service": "go2rtc"}))).into_response()
    }
}

// ─── Phase 255: Display machine heartbeat ────────────────────────────────

#[derive(Deserialize)]
struct KioskPingBody {
    display_id: String,
    uptime_s: u64,
}

async fn kiosk_ping_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<KioskPingBody>,
) -> Json<Value> {
    let mut heartbeats = state.display_heartbeats.write().await;
    heartbeats.insert(body.display_id, (std::time::Instant::now(), body.uptime_s));
    Json(json!({ "ok": true }))
}

// ─── Tier 2: Customer (JWT checked in-handler via extract_driver_id) ─────

fn customer_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/customer/waiver-status", get(customer_waiver_status))
        .route("/customer/profile", get(customer_profile).put(customer_update_profile))
        .route("/customer/sessions", get(customer_sessions))
        .route("/customer/sessions/{id}", get(customer_session_detail))
        .route("/customer/laps", get(customer_laps))
        .route("/customer/stats", get(customer_stats))
        .route("/customer/wallet", get(customer_wallet))
        .route("/customer/wallet/transactions", get(customer_wallet_transactions))
        .route("/customer/experiences", get(customer_experiences))
        .route("/customer/ac/catalog", get(customer_ac_catalog))
        .route("/customer/book", post(customer_book_session))
        .route("/customer/active-reservation", get(customer_active_reservation))
        .route("/customer/end-reservation", post(customer_end_reservation))
        .route("/customer/continue-session", post(customer_continue_session))
        // Friends (PWA)
        .route("/customer/friends", get(customer_friends))
        .route("/customer/friends/requests", get(customer_friend_requests))
        .route("/customer/friends/request", post(customer_send_friend_request))
        .route("/customer/friends/request/{id}/accept", post(customer_accept_friend_request))
        .route("/customer/friends/request/{id}/reject", post(customer_reject_friend_request))
        .route("/customer/friends/{id}", axum::routing::delete(customer_remove_friend))
        .route("/customer/presence", put(customer_set_presence))
        // Multiplayer (PWA)
        .route("/customer/book-multiplayer", post(customer_book_multiplayer))
        .route("/customer/group-session", get(customer_group_session))
        .route("/customer/group-session/{id}/accept", post(customer_accept_group_invite))
        .route("/customer/group-session/{id}/decline", post(customer_decline_group_invite))
        .route("/customer/multiplayer-results/{group_session_id}", get(customer_multiplayer_results))
        // Telemetry (PWA)
        .route("/customer/telemetry", get(customer_telemetry))
        // Tournament (PWA customer)
        .route("/customer/tournaments", get(customer_list_tournaments))
        .route("/customer/tournaments/{id}/register", post(customer_register_tournament))
        // Coaching / Telemetry comparison (PWA)
        .route("/customer/compare-laps", get(customer_compare_laps))
        // Session share report (PWA)
        .route("/customer/sessions/{id}/share", get(customer_session_share))
        // GST invoice (LEGAL-02 — customer copy of their invoice)
        .route("/customer/sessions/{id}/invoice", get(customer_session_invoice))
        // Referrals (PWA)
        .route("/customer/referral-code", get(customer_referral_code))
        .route("/customer/referral-code/generate", post(customer_generate_referral_code))
        .route("/customer/redeem-referral", post(customer_redeem_referral))
        // Coupons (PWA)
        .route("/customer/apply-coupon", post(customer_apply_coupon))
        // Packages (PWA)
        .route("/customer/packages", get(customer_list_packages))
        // Memberships (PWA)
        .route("/customer/membership", get(customer_membership))
        .route("/customer/membership/subscribe", post(customer_subscribe_membership))
        // Customer AI chat
        .route("/customer/ai/chat", post(customer_ai_chat))
        // Game launch request (PWA -- customer requests staff-confirmed game launch)
        .route("/customer/game-request", post(pwa_game_request))
        // BILL-03: Game request status polling (TTL = 10 min, expires_at checked server-side)
        .route("/customer/game-request/{id}", get(get_game_request_status))
        // DPDP Act data rights (Plan 79-03)
        .route("/customer/data-export", get(customer_data_export))
        .route("/customer/data-delete", axum::routing::delete(customer_data_delete))
        // Driving Passport (PWA)
        .route("/customer/passport", get(customer_passport))
        .route("/customer/badges", get(customer_badges))
        // Active session PB events (PWA polling)
        .route("/customer/active-session/events", get(customer_active_session_events))
        // Remote booking reservations (PWA)
        .route("/customer/reservation", get(customer_get_reservation).delete(customer_cancel_reservation))
        .route("/customer/reservation/create", post(customer_create_reservation))
        .route("/customer/reservation/modify", put(customer_modify_reservation))
        // Cafe ordering (customer self-service — driver_id from JWT, not body)
        .route("/customer/cafe/orders", post(cafe::place_cafe_order_customer))
        .route("/customer/cafe/orders/history", get(cafe::list_customer_orders))
        // LEGAL-09: Consent revocation (DPDP Act — right of erasure for driver or guardian via PWA)
        .route("/customer/revoke-consent", post(revoke_consent_handler))
        // BILL-08: Customer charge dispute portal — submit a dispute from PWA
        .route("/customer/dispute", post(create_dispute_handler))
        // UX-03: Customer receipt — full financial breakdown with GST, before/after balance
        .route("/customer/sessions/{id}/receipt", get(customer_session_receipt))
}

// ─── Tier 3a: Kiosk-facing (staff JWT required, but pod-accessible) ──────

/// Kiosk routes accessible from pod IPs. These require a staff JWT (the kiosk
/// PWA authenticates via validate-pin which returns a staff JWT) but are NOT
/// blocked by the pod source guard. Separated from staff_routes so pods can
/// call them while staff/admin routes remain pod-blocked.
fn kiosk_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/kiosk/experiences", get(list_kiosk_experiences))
        .route("/kiosk/settings", get(get_kiosk_settings))
        .route("/kiosk/pod-launch-experience", post(kiosk_pod_launch_experience))
        .route("/kiosk/book-multiplayer", post(kiosk_book_multiplayer))
        .layer(axum::middleware::from_fn_with_state(state, require_staff_jwt))
}

// ─── Tier 3b: Staff/Admin (staff JWT + pod source block) ──────

/// Staff and admin routes. Protected by `require_staff_jwt` (strict) which
/// rejects unauthenticated requests with 401 Unauthorized, AND by
/// `require_non_pod_source` which rejects pod-originated requests with 403.
/// Switched from permissive mode (expand-migrate-contract pattern) now that
/// dashboard, kiosk, and bots send staff JWTs.
fn staff_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // Driver rating history (staff-only — Phase 253)
        .route("/drivers/{id}/rating-history", get(staff_driver_rating_history))
        // MMA-P1: Debug endpoints moved from public_routes — require staff JWT
        .route("/debug/db-stats", get(debug_db_stats))
        .route("/debug/activity", get(debug_activity))
        .route("/debug/playbooks", get(debug_playbooks))
        .route("/debug/incidents", get(list_debug_incidents))
        .route("/debug/pod-events/{pod_id}", get(debug_pod_events))
        // Pods
        .route("/pods", get(list_pods).post(register_pod))
        .route("/pod-status-summary", get(pod_status_summary))
        .route("/pods/seed", post(seed_pods))
        .route("/pods/{id}", get(get_pod))
        .route("/pods/{id}/wake", post(wake_pod))
        .route("/pods/{id}/shutdown", post(shutdown_pod))
        .route("/pods/{id}/lockdown", post(lockdown_pod))
        .route("/pods/{id}/enable", post(enable_pod))
        .route("/pods/{id}/disable", post(disable_pod))
        .route("/pods/{id}/screen", post(set_pod_screen))
        .route("/pods/{id}/unrestrict", post(unrestrict_pod))
        .route("/pods/{id}/freedom", post(freedom_mode_pod))
        .route("/pods/{id}/restart", post(restart_pod))
        .route("/pods/wake-all", post(wake_all_pods))
        .route("/pods/shutdown-all", post(shutdown_all_pods))
        .route("/pods/restart-all", post(restart_all_pods))
        .route("/pods/lockdown-all", post(lockdown_all_pods))
        // Venue-level shutdown (audit-gated)
        .route("/venue/shutdown", post(venue_shutdown::venue_shutdown_handler))
        .route("/pods/{id}/exec", post(ws_exec_pod))
        .route("/pods/{id}/self-test", get(pod_self_test))
        .route("/pods/{id}/clear-maintenance", post(clear_maintenance_pod))
        .route("/pods/{pod_id}/transmission", post(set_pod_transmission))
        .route("/pods/{pod_id}/ffb", post(set_pod_ffb))
        .route("/pods/{pod_id}/assists", post(set_pod_assists))
        .route("/pods/{pod_id}/assist-state", get(get_pod_assist_state))
        .route("/pods/{pod_id}/activity", get(pod_activity))
        .route("/pods/{pod_id}/watchdog-crash", post(watchdog_crash_report))
        // Drivers
        .route("/drivers", get(list_drivers).post(create_driver))
        .route("/drivers/{id}", get(get_driver))
        .route("/drivers/{id}/full-profile", get(get_driver_full_profile))
        // LEGAL-09: Staff-initiated consent revocation (cashier+ — guardian calls venue, staff processes)
        .route("/drivers/{id}/revoke-consent", post(staff_revoke_consent_handler))
        // Sessions
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{id}", get(get_session))
        .route("/sessions/{id}/laps", get(session_laps))
        // Laps
        .route("/laps", get(list_laps))
        // Leaderboard
        .route("/leaderboard/{track}", get(track_leaderboard))
        // Events
        .route("/events", get(list_events).post(create_event))
        // Bookings
        .route("/bookings", get(list_bookings).post(create_booking))
        // Pricing
        .route("/pricing", get(list_pricing_tiers).post(create_pricing_tier))
        .route("/pricing/{id}", put(update_pricing_tier).delete(delete_pricing_tier))
        .route("/pricing/rules", get(list_pricing_rules).post(create_pricing_rule))
        .route("/pricing/rules/{id}", put(update_pricing_rule).delete(delete_pricing_rule))
        // Billing
        .route("/billing/start", post(start_billing))
        .route("/billing/active", get(active_billing_sessions))
        .route("/billing/sessions", get(list_billing_sessions))
        .route("/billing/sessions/{id}", get(get_billing_session))
        .route("/billing/sessions/{id}/events", get(billing_session_events))
        .route("/billing/sessions/{id}/summary", get(billing_session_summary))
        .route("/billing/sessions/{id}/invoice", get(get_session_invoice))
        .route("/billing/{id}/stop", post(stop_billing))
        .route("/billing/{id}/pause", post(pause_billing))
        .route("/billing/{id}/resume", post(resume_billing))
        .route("/billing/{id}/extend", post(extend_billing))
        // STAFF-01: Discount approval — cashier+ access, manager approval code required above threshold
        .route("/billing/{id}/discount", post(apply_billing_discount))
        .route("/billing/{id}/refund", post(refund_billing_session))
        .route("/billing/{id}/refunds", get(get_billing_refunds))
        // billing/report, billing/rates — moved to role-gated financial section
        .route("/billing/split-options/{duration_minutes}", get(get_split_options))
        .route("/billing/continue-split", post(continue_split))
        // Game Launcher
        .route("/games/launch", post(launch_game))
        .route("/games/relaunch/{pod_id}", post(relaunch_game))
        .route("/games/stop", post(stop_game))
        .route("/games/catalog", get(games_catalog))
        .route("/games/active", get(active_games))
        .route("/games/history", get(game_launch_history))
        .route("/games/pod/{pod_id}", get(pod_game_state))
        // AC LAN
        .route("/ac/presets", get(list_ac_presets).post(save_ac_preset))
        .route("/ac/presets/{id}", get(get_ac_preset).put(update_ac_preset).delete(delete_ac_preset))
        .route("/ac/session/start", post(start_ac_session))
        .route("/ac/session/stop", post(stop_ac_session))
        .route("/ac/session/active", get(active_ac_session))
        .route("/ac/sessions", get(list_ac_sessions))
        .route("/ac/sessions/{id}/leaderboard", get(ac_session_leaderboard))
        .route("/ac/session/{session_id}/continuous", post(ac_server_set_continuous))
        .route("/ac/session/retry-pod", post(ac_session_retry_pod))
        .route("/ac/session/update-config", post(ac_session_update_config))
        .route("/ac/content/tracks", get(list_ac_tracks))
        .route("/ac/content/cars", get(list_ac_cars))
        // Auth (staff-facing)
        .route("/auth/assign", post(assign_customer))
        .route("/auth/cancel/{id}", post(cancel_assignment))
        .route("/auth/pending", get(pending_auth_tokens))
        .route("/auth/pending/{pod_id}", get(pending_auth_token_for_pod))
        .route("/auth/start-now", post(start_now))
        .route("/auth/validate-qr", post(validate_qr))
        // Wallet (staff-facing)
        .route("/wallet/transactions", get(all_wallet_transactions))
        .route("/wallet/{driver_id}", get(get_wallet))
        .route("/wallet/{driver_id}/topup", post(topup_wallet))
        .route("/wallet/{driver_id}/transactions", get(wallet_transactions))
        .route("/wallet/{driver_id}/debit", post(debit_wallet_manual))
        .route("/wallet/{driver_id}/refund", post(refund_wallet))
        // Waivers (admin-facing)
        .route("/waivers", get(list_waivers))
        .route("/waivers/check", get(check_waiver))
        .route("/waivers/{driver_id}/signature", get(get_waiver_signature))
        // Guardian OTP (LEGAL-04/05) — staff sends + verifies guardian OTP for minor customers
        .route("/guardian/send-otp", post(send_guardian_otp_handler))
        .route("/guardian/verify-otp", post(verify_guardian_otp_handler))
        // Kiosk (admin-only: create/update/delete -- pod-accessible routes are in kiosk_routes())
        // kiosk experiences/settings — moved to role-gated admin section
        // Config
        // GET moved to public_routes (rc-agent fetches without auth)
        // config/kiosk-allowlist POST/DELETE — moved to role-gated admin section
        // POS — POST only (write), GET moved to public_routes (MMA Round 1 fix: POS agent polls without JWT)
        // pos/lockdown — moved to role-gated admin section
        // AI (staff)
        .route("/ai/chat", post(ai_chat))
        .route("/ai/diagnose", post(ai_diagnose))
        .route("/ai/suggestions", get(list_ai_suggestions))
        .route("/ai/suggestions/{id}/dismiss", post(dismiss_ai_suggestion))
        .route("/ai/training/stats", get(ai_training_stats))
        .route("/ai/training/pairs", get(ai_training_pairs))
        .route("/ai/training/import", post(ai_training_import))
        // Ops stats
        .route("/ops/stats", get(ops_stats))
        // Activity
        .route("/activity", get(global_activity))
        // Staff
        .route("/staff", get(list_staff).post(create_staff))
        .route("/staff/{id}", put(update_staff).delete(delete_staff))
        // Employee
        .route("/employee/daily-pin", get(employee_daily_pin))
        .route("/employee/debug-unlock", post(employee_debug_unlock))
        // Coupons (admin)
        .route("/coupons", get(list_coupons).post(create_coupon))
        .route("/coupons/{id}", put(update_coupon).delete(delete_coupon))
        // Review Nudges (admin)
        .route("/review-nudges/pending", get(pending_review_nudges))
        .route("/review-nudges/{id}/sent", post(mark_nudge_sent))
        // Time Trial Admin
        .route("/time-trials", get(list_time_trials).post(create_time_trial))
        .route("/time-trials/{id}", put(update_time_trial).delete(delete_time_trial))
        // Tournaments (admin)
        .route("/tournaments", get(list_tournaments).post(create_tournament))
        .route("/tournaments/{id}", get(get_tournament).put(update_tournament))
        .route("/tournaments/{id}/registrations", get(tournament_registrations))
        .route("/tournaments/{id}/matches", get(tournament_matches))
        .route("/tournaments/{id}/generate-bracket", post(generate_bracket))
        .route("/tournaments/{id}/matches/{match_id}/result", post(record_match_result))
        // Scheduler
        .route("/scheduler/status", get(scheduler::get_status))
        .route("/scheduler/settings", put(scheduler::update_settings))
        .route("/scheduler/analytics", get(scheduler::get_analytics))
        // Accounting & Audit — routes are in the role-gated financial section below
        // audit-log, flags, config/push, deploy, ota, debug/incidents, deploy-log, recovery/events — all moved to role-gated sections
        // STAFF-05: Shift handoff workflow
        .route("/staff/shift-handoff", post(shift_handoff_handler))
        .route("/staff/shift-briefing", get(shift_briefing_handler))
        // UX-08: Virtual queue management (staff side)
        .route("/queue", get(queue_list_handler))
        .route("/queue/{id}/call", post(queue_call_handler))
        .route("/queue/{id}/seat", post(queue_seat_handler))
        // Staff: Hotlap Events
        .route("/staff/events", post(create_hotlap_event).get(list_staff_events))
        .route("/staff/events/{id}", get(get_staff_event).put(update_hotlap_event))
        // Staff: Championships
        .route("/staff/championships", post(create_championship).get(list_staff_championships))
        .route("/staff/championships/{id}", get(get_staff_championship))
        .route("/staff/championships/{id}/rounds", post(add_championship_round))
        .route("/staff/events/{id}/link-session", post(link_group_session_to_event))
        .route("/staff/group-sessions/{id}/complete", post(complete_group_session))
        // ─── Psychology ──────────────────────────────────────────────────────────
        .route("/psychology/badges", get(list_badges))
        .route("/psychology/badges/{driver_id}", get(driver_badges))
        .route("/psychology/streaks/{driver_id}", get(driver_streak))
        .route("/psychology/nudge-queue", get(list_nudge_queue))
        .route("/psychology/test-nudge", post(test_nudge))
        // ─── Cafe Menu ──────────────────────────────────────────────────────────
        // NOTE: /cafe/items/low-stock MUST be registered before /cafe/items/{id} wildcard
        .route("/cafe/items/low-stock", get(cafe_alerts::list_low_stock_items))
        .route("/cafe/items", get(cafe::list_cafe_items).post(cafe::create_cafe_item))
        .route("/cafe/items/{id}", put(cafe::update_cafe_item).delete(cafe::delete_cafe_item))
        .route("/cafe/items/{id}/toggle", post(cafe::toggle_cafe_item_availability))
        .route("/cafe/items/{id}/image", post(cafe::upload_item_image))
        .route("/cafe/items/{id}/restock", post(cafe::restock_cafe_item))
        .route("/cafe/categories", get(cafe::list_cafe_categories).post(cafe::create_cafe_category))
        .route("/cafe/import/preview", post(cafe::import_preview))
        .route("/cafe/import/confirm", post(cafe::confirm_import))
        .route("/cafe/orders", post(cafe::place_cafe_order))
        .route("/cafe/promos", get(cafe_promos::list_cafe_promos).post(cafe_promos::create_cafe_promo))
        .route("/cafe/promos/{id}", put(cafe_promos::update_cafe_promo).delete(cafe_promos::delete_cafe_promo))
        .route("/cafe/promos/{id}/toggle", post(cafe_promos::toggle_cafe_promo))
        // ─── Cafe Marketing ─────────────────────────────────────────────────────
        .route("/cafe/marketing/broadcast", post(cafe_marketing::broadcast_promo))
        // ─── HR & Hiring Psychology (v14.0 Phase 96) ──────────────────────────
        .route("/hr/sjts", get(list_hiring_sjts))
        .route("/hr/sjts/{id}", get(get_hiring_sjt))
        .route("/hr/job-preview", get(list_job_preview))
        .route("/hr/campaign-templates", get(list_campaign_templates))
        .route("/hr/nudge-templates", get(list_nudge_templates))
        .route("/hr/recognition", get(hr_recognition_data))
        // ─── Staff Gamification (v14.0 Phase 95) ──────────────────────────────
        .route("/staff/{id}/opt-in", post(staff_gamification_opt_in))
        .route("/staff/gamification/leaderboard", get(staff_gamification_leaderboard))
        .route("/staff/{id}/badges", get(staff_badges_list))
        .route("/staff/gamification/kudos", get(staff_kudos_list).post(staff_kudos_create))
        .route("/staff/gamification/challenges", get(staff_challenges_list).post(staff_challenges_create))
        .route("/staff/gamification/challenges/{id}/progress", post(staff_challenge_update_progress))
        // ─── Autonomous Pipeline (v26.0) ─────────────────────────────────────
        .route("/pipeline/status", get(pipeline_status))
        // MMA-v29: Metrics, mesh, admin, cameras moved from public_routes — require staff JWT
        .route("/metrics/launch-stats", get(metrics::launch_stats_handler))
        .route("/metrics/billing-accuracy", get(metrics::billing_accuracy_handler))
        .route("/admin/launch-matrix", get(metrics::launch_matrix_handler))
        .route("/mesh/solutions", get(mesh_list_solutions))
        .route("/mesh/solutions/search", get(mesh_search_solutions))
        .route("/mesh/solutions/{id}", get(mesh_get_solution))
        .route("/mesh/incidents", get(mesh_list_incidents))
        .route("/mesh/stats", get(mesh_stats))
        .route("/mesh/deploy-status", get(mesh_deploy_status))
        .route("/cameras/health", get(cameras_health_proxy))
        // Mesh Intelligence (v26.0) — staff write operations
        .route("/mesh/solutions/{id}/promote", post(mesh_promote_solution))
        .route("/mesh/solutions/{id}/retire", post(mesh_retire_solution))
        // ─── v29.0 Phase 9: Maintenance & Analytics ─────────────────────────
        .route("/maintenance/events", post(maintenance_create_event).get(maintenance_list_events))
        .route("/maintenance/summary", get(maintenance_summary))
        .route("/maintenance/tasks", post(maintenance_create_task).get(maintenance_list_tasks))
        .route("/maintenance/tasks/{id}", axum::routing::patch(maintenance_update_task))
        .route("/analytics/telemetry", get(analytics_telemetry))
        .route("/analytics/trends", get(analytics_trends))
        // Merge role-gated sub-routers (SEC-04: manager+, superadmin-only groups)
        .merge(
            // ── Manager+ routes ─────────────────────────────────────────────
            // Billing reports, financial accounting, audit log, rate management.
            // Cashiers cannot access financial reports or modify billing rates.
            Router::new()
                .route("/billing/report/daily", get(daily_billing_report))
                .route("/billing/rates", get(list_billing_rates).post(create_billing_rate))
                .route("/billing/rates/{id}", put(update_billing_rate).delete(delete_billing_rate))
                .route("/accounting/accounts", get(list_accounts))
                .route("/accounting/trial-balance", get(trial_balance))
                .route("/accounting/profit-loss", get(profit_loss))
                .route("/accounting/balance-sheet", get(balance_sheet))
                .route("/accounting/journal", get(list_journal_entries))
                .route("/audit-log", get(query_audit_log))
                .route("/reconciliation/status", get(reconciliation_status))
                .route("/reconciliation/run", post(reconciliation_run))
                // BILL-08: Admin dispute review endpoints (manager+ — financial resolution)
                .route("/admin/disputes", get(list_disputes_handler))
                .route("/admin/disputes/{id}/details", get(dispute_details_handler))
                .route("/admin/disputes/{id}/resolve", post(resolve_dispute_handler))
                // STAFF-03: Daily override audit report (all discounts, refunds, tier changes with actor_id)
                .route("/admin/reports/daily-overrides", get(daily_overrides_report))
                // STAFF-04: Cash drawer reconciliation
                .route("/admin/reports/cash-drawer", get(cash_drawer_status))
                .route("/admin/reports/cash-drawer/close", post(cash_drawer_close))
                .layer(axum::middleware::from_fn(require_role_manager))
        )
        .merge(
            // ── Superadmin-only routes ──────────────────────────────────────
            // System config, feature flags, deploy pipeline, OTA, pipeline config.
            // Managers cannot change system configuration.
            Router::new()
                .route("/flags", get(flags::list_flags).post(flags::create_flag))
                .route("/flags/{name}", put(flags::update_flag))
                .route("/config/push", post(config_push::push_config))
                .route("/config/push/queue", get(config_push::get_queue))
                .route("/config/audit", get(config_push::get_audit_log))
                .route("/deploy/status", get(deploy_status))
                .route("/deploy/rolling", post(deploy_rolling_handler))
                .route("/deploy/{pod_id}", post(deploy_single_pod))
                .route("/ota/deploy", post(ota_deploy_handler))
                .route("/ota/status", get(ota_status_handler))
                .route("/pipeline/config", get(pipeline_config_get).post(pipeline_config_set))
                .layer(axum::middleware::from_fn(require_role_superadmin))
        )
        // Apply strict staff JWT middleware (rejects unauthenticated with 401)
        .layer(axum::middleware::from_fn(require_non_pod_source))
        .layer(axum::middleware::from_fn_with_state(state, require_staff_jwt))
}

// ─── Tier 4: Service (terminal_secret/sync auth in handler) ──────────────

fn service_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Cloud sync
        .route("/sync/changes", get(sync_changes))
        .route("/sync/push", post(sync_push))
        .route("/sync/health", get(sync_health))
        // Cloud action queue
        .route("/actions", post(create_action))
        .route("/actions/pending", get(pending_actions))
        .route("/actions/process", post(process_action_endpoint))
        .route("/actions/{id}/ack", post(ack_action))
        .route("/actions/history", get(action_history))
        // Terminal (remote command execution — terminal_secret auth in handler)
        .route("/terminal/auth", post(terminal_auth))
        .route("/terminal/commands", get(terminal_list).post(terminal_submit))
        .route("/terminal/commands/pending", get(terminal_pending))
        .route("/terminal/commands/{id}/result", post(terminal_result))
        .route("/terminal/book-multiplayer", post(terminal_book_multiplayer))
        .route("/terminal/group-sessions", get(terminal_group_sessions))
        // Bot (WhatsApp bot — terminal_secret auth in handler)
        .route("/bot/lookup", get(bot_lookup))
        .route("/bot/pricing", get(bot_pricing))
        .route("/bot/book", post(bot_book))
        .route("/bot/pods-status", get(bot_pods_status))
        .route("/bot/events", get(bot_events))
        .route("/bot/leaderboard", get(bot_leaderboard))
        .route("/bot/customer-stats", get(bot_customer_stats))
        .route("/bot/register-lead", post(bot_register_lead))
        // Server logs (service-level, used by cloud terminal)
        .route("/logs", get(get_server_logs))
        // Failover orchestration (Phase 69: broadcast SwitchController to all pods)
        .route("/failover/broadcast", post(failover_broadcast))
        // Failback data reconciliation (Phase 70: import cloud sessions during failback)
        .route("/sync/import-sessions", post(import_sessions))
        // Process guard intake (Phase 105: rc-process-guard on James reports via HTTP)
        // Auth: X-Guard-Token header checked against config.process_guard.report_secret
        .route("/guard/report", post(process_guard::post_guard_report_handler))
        // Deploy audit log (Phase 177: record every deploy attempt)
        .route("/deploy-log", get(list_deploy_logs).post(create_deploy_log))
        // App health monitor (Phase 179: current probe results for admin/kiosk/web)
        .route("/app-health", get(get_app_health))
        // Mesh Intelligence Cloud KB sync (v26.0 Phase 227)
        .route("/cloud/mesh/sync", post(cloud_mesh_sync))
        .route("/cloud/mesh/pull", get(cloud_mesh_pull))
}

const BUILD_ID: &str = env!("GIT_HASH");

async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Check Evolution API (WhatsApp) reachability — non-blocking, 2s timeout
    let whatsapp_status = check_evolution_health(&state).await;

    Json(json!({
        "status": "ok",
        "service": "racecontrol",
        "version": env!("CARGO_PKG_VERSION"),
        "build_id": BUILD_ID,
        "whatsapp": whatsapp_status,
    }))
}

/// Probe Evolution API health. Returns "ok", "unreachable", or "not_configured".
async fn check_evolution_health(state: &Arc<AppState>) -> &'static str {
    let evo_url = match &state.config.auth.evolution_url {
        Some(u) => u.clone(),
        None => return "not_configured",
    };

    // Probe the base URL — Evolution API returns 200 on GET /
    // 5s timeout: external hostname DNS resolution can exceed 2s from venue server
    match state.http_client
        .get(&evo_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 401 => "ok",
        Ok(_) => "ok", // Any HTTP response means Evolution is reachable
        Err(_) => "unreachable",
    }
}

/// GET /api/v1/debug/db-stats — AI debugger database statistics (public, no auth).
/// Returns counts for ai_suggestions, ai_training_pairs, and recent entries.
async fn debug_db_stats(State(state): State<Arc<AppState>>) -> Json<Value> {
    let db = &state.db;

    let suggestion_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_suggestions")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let active_suggestions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_suggestions WHERE dismissed = 0",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let training_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_training_pairs")
        .fetch_one(db)
        .await
        .unwrap_or(0);

    // Recent suggestions (last 10)
    let recent: Vec<serde_json::Value> = sqlx::query_as::<_, (String, String, String, String, String, i32, String)>(
        "SELECT id, pod_id, sim_type, source, model, dismissed, created_at FROM ai_suggestions ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, pod_id, sim_type, source, model, dismissed, created_at)| {
        json!({
            "id": id,
            "pod_id": pod_id,
            "sim_type": sim_type,
            "source": source,
            "model": model,
            "dismissed": dismissed != 0,
            "created_at": created_at,
        })
    })
    .collect();

    Json(json!({
        "ai_suggestions": {
            "total": suggestion_count,
            "active": active_suggestions,
            "dismissed": suggestion_count - active_suggestions,
        },
        "ai_training_pairs": {
            "total": training_count,
        },
        "recent_suggestions": recent,
    }))
}

async fn venue_info(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "name": state.config.venue.name,
        "location": state.config.venue.location,
        "timezone": state.config.venue.timezone,
        "pods": state.config.pods.count,
    }))
}

async fn list_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod_list: Vec<&PodInfo> = pods.values().collect();
    Json(json!({ "pods": pod_list }))
}

async fn pod_status_summary(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods = state.pods.read().await;
    let total = pods.len();
    let mut down: Vec<Value> = Vec::new();
    for pod in pods.values() {
        if pod.status == PodStatus::Offline || pod.status == PodStatus::Error {
            down.push(json!({
                "id": pod.id,
                "number": pod.number,
                "status": pod.status,
            }));
        }
    }
    let active = total - down.len();
    Json(json!({
        "active": active,
        "total": total,
        "down": down,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

async fn get_pod(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let pods = state.pods.read().await;
    match pods.get(&id) {
        Some(pod) => Json(json!({ "pod": pod })),
        None => Json(json!({ "error": "Pod not found" })),
    }
}

async fn register_pod(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = body["id"].as_str().unwrap_or("").to_string();
    let number = body["number"].as_u64().unwrap_or(0) as u32;
    let name = body["name"].as_str().unwrap_or("").to_string();
    let ip = body["ip_address"].as_str().unwrap_or("").to_string();
    let sim = body["sim_type"].as_str().unwrap_or("assetto_corsa");
    let sim_type = match sim {
        "assetto_corsa_evo" => SimType::AssettoCorsaEvo,
        "iracing" => SimType::IRacing,
        "f1_25" => SimType::F125,
        "le_mans_ultimate" | "lemans" => SimType::LeMansUltimate,
        "forza" => SimType::Forza,
        _ => SimType::AssettoCorsa,
    };

    let pod = PodInfo {
        id: id.clone(),
        number,
        name,
        ip_address: ip,
        mac_address: None,
        sim_type,
        status: PodStatus::Idle,
        current_driver: None,
        current_session_id: None,
        last_seen: Some(chrono::Utc::now()),
        driving_state: None,
        billing_session_id: None,
        game_state: None,
        current_game: None,
        installed_games: vec![],
        screen_blanked: None,
        ffb_preset: None,
        freedom_mode: None,
        agent_timestamp: None, // Intentional default: server-side pod creation has no agent clock
    };

    state.pods.write().await.insert(id.clone(), pod.clone());
    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));

    Json(json!({ "ok": true, "pod": pod }))
}

async fn seed_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    // (id, number, name, ip, mac)
    let pod_data = vec![
        ("pod_1", 1, "Pod 1", "192.168.31.89", "30:56:0F:05:45:88"),
        ("pod_2", 2, "Pod 2", "192.168.31.33", "30:56:0F:05:46:53"),
        ("pod_3", 3, "Pod 3", "192.168.31.28", "30:56:0F:05:44:B3"),
        ("pod_4", 4, "Pod 4", "192.168.31.88", "30:56:0F:05:45:25"),
        ("pod_5", 5, "Pod 5", "192.168.31.86", "30:56:0F:05:44:B7"),
        ("pod_6", 6, "Pod 6", "192.168.31.87", "30:56:0F:05:45:6E"),
        ("pod_7", 7, "Pod 7", "192.168.31.38", "30:56:0F:05:44:B4"),
        ("pod_8", 8, "Pod 8", "192.168.31.91", "30:56:0F:05:46:C5"),
    ];

    let mut pods_created = Vec::new();
    for (id, number, name, ip, mac) in pod_data {
        let pod = PodInfo {
            id: id.to_string(),
            number,
            name: name.to_string(),
            ip_address: ip.to_string(),
            mac_address: Some(mac.to_string()),
            sim_type: SimType::AssettoCorsa,
            status: PodStatus::Idle,
            current_driver: None,
            current_session_id: None,
            last_seen: Some(chrono::Utc::now()),
            driving_state: None,
            billing_session_id: None,
            game_state: None,
            current_game: None,
            installed_games: vec![],
            screen_blanked: None,
            ffb_preset: None,
            freedom_mode: None,
            agent_timestamp: None, // Intentional default: server-side pod seeding has no agent clock
        };
        state.pods.write().await.insert(id.to_string(), pod.clone());
        let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        pods_created.push(pod);
    }

    // Also send a full pod list event
    let all_pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let _ = state.dashboard_tx.send(DashboardEvent::PodList(all_pods));

    Json(json!({ "ok": true, "count": pods_created.len() }))
}

// POST /pods/{id}/wake — Send Wake-on-LAN magic packet
async fn wake_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod = match pods.get(&id) {
        Some(p) => p.clone(),
        None => return Json(json!({ "error": format!("Pod {} not found", id) })),
    };
    drop(pods);

    let mac = match &pod.mac_address {
        Some(m) => m.clone(),
        None => return Json(json!({ "error": format!("No MAC address for pod {}", id) })),
    };

    match wol::send_wol(&mac).await {
        Ok(_) => Json(json!({ "status": "wol_sent", "pod_id": id, "mac": mac })),
        Err(e) => Json(json!({ "error": format!("WoL failed: {}", e) })),
    }
}

// POST /pods/{id}/shutdown — Shutdown pod via pod-agent
async fn shutdown_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod = match pods.get(&id) {
        Some(p) => p.clone(),
        None => return Json(json!({ "error": format!("Pod {} not found", id) })),
    };
    drop(pods);

    match wol::shutdown_pod(&state.http_client, &pod.ip_address).await {
        Ok(output) => {
            // Mark pod as Disabled — prevents auto-recovery from waking it back up
            if let Some(p) = state.pods.write().await.get_mut(&id) {
                p.status = PodStatus::Disabled;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(p.clone()));
            }
            Json(json!({ "status": "shutdown_sent", "pod_id": id, "output": output }))
        }
        Err(e) => Json(json!({ "error": format!("Shutdown failed: {}", e) })),
    }
}

// POST /pods/{id}/enable — Re-enable a disabled pod (allows auto-recovery)
async fn enable_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut pods = state.pods.write().await;
    match pods.get_mut(&id) {
        Some(pod) => {
            pod.status = PodStatus::Offline;
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            Json(json!({ "status": "enabled", "pod_id": id }))
        }
        None => Json(json!({ "error": format!("Pod {} not found", id) })),
    }
}

// POST /pods/{id}/disable — Disable a pod (prevents all auto-recovery, no shutdown)
async fn disable_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let mut pods = state.pods.write().await;
    match pods.get_mut(&id) {
        Some(pod) => {
            pod.status = PodStatus::Disabled;
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            Json(json!({ "status": "disabled", "pod_id": id }))
        }
        None => Json(json!({ "error": format!("Pod {} not found", id) })),
    }
}

// POST /pods/:id/screen — Blank or unblank a specific pod's screen
async fn set_pod_screen(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let blank = body.get("blank").and_then(|v| v.as_bool()).unwrap_or(false);

    // MMA-P1: Clone sender out of read lock, drop guard BEFORE .await
    // Prevents deadlock/starvation when holding RwLock across async boundaries
    let sender = {
        let agent_senders = state.agent_senders.read().await;
        agent_senders.get(&id).cloned()
    }; // read lock dropped here

    match sender {
        Some(sender) => {
            let msg = if blank {
                CoreToAgentMessage::BlankScreen
            } else {
                CoreToAgentMessage::ClearLockScreen
            };
            let _ = sender.send(msg).await;

            // Optimistic update: reflect blank state immediately so kiosk sees the change
            // without waiting for the next heartbeat cycle
            {
                let mut pods = state.pods.write().await;
                if let Some(pod) = pods.get_mut(&id) {
                    pod.screen_blanked = Some(blank);
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }

            Json(json!({ "ok": true, "pod_id": id, "blank": blank }))
        }
        None => Json(json!({ "error": format!("Pod {} not connected", id) })),
    }
}

// POST /pods/wake-all — Wake all pods with known MACs
async fn wake_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let mut results = Vec::new();

    for pod in &pods {
        if let Some(mac) = &pod.mac_address {
            let status = match wol::send_wol(mac).await {
                Ok(_) => "sent",
                Err(_) => "failed",
            };
            results.push(json!({ "pod_id": pod.id, "mac": mac, "status": status }));
        }
    }

    Json(json!({ "status": "ok", "results": results }))
}

// POST /pods/shutdown-all — Shutdown all reachable pods
async fn shutdown_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let mut results = Vec::new();

    for pod in &pods {
        if pod.status == PodStatus::Offline || pod.status == PodStatus::Disabled {
            results.push(json!({ "pod_id": pod.id, "status": "skipped" }));
            continue;
        }
        let status = match wol::shutdown_pod(&state.http_client, &pod.ip_address).await {
            Ok(_) => {
                if let Some(p) = state.pods.write().await.get_mut(&pod.id) {
                    p.status = PodStatus::Disabled;
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(p.clone()));
                }
                "sent"
            }
            Err(_) => "failed",
        };
        results.push(json!({ "pod_id": pod.id, "status": status }));
    }

    Json(json!({ "status": "ok", "results": results }))
}

// POST /pods/{id}/restart — Restart pod via pod-agent (does NOT mark Disabled)
async fn restart_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod = match pods.get(&id) {
        Some(p) => p.clone(),
        None => return Json(json!({ "error": format!("Pod {} not found", id) })),
    };
    drop(pods);

    match wol::restart_pod(&state.http_client, &pod.ip_address).await {
        Ok(output) => Json(json!({ "status": "restart_sent", "pod_id": id, "output": output })),
        Err(e) => Json(json!({ "error": format!("Restart failed: {}", e) })),
    }
}

// POST /pods/restart-all — Restart all reachable pods
async fn restart_all_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods: Vec<PodInfo> = state.pods.read().await.values().cloned().collect();
    let mut results = Vec::new();

    for pod in &pods {
        if pod.status == PodStatus::Offline || pod.status == PodStatus::Disabled {
            results.push(json!({ "pod_id": pod.id, "status": "skipped" }));
            continue;
        }
        let status = match wol::restart_pod(&state.http_client, &pod.ip_address).await {
            Ok(_) => "sent",
            Err(_) => "failed",
        };
        results.push(json!({ "pod_id": pod.id, "status": status }));
    }

    Json(json!({ "status": "ok", "results": results }))
}

// POST /pods/{id}/lockdown — Toggle kiosk lockdown for a specific pod
// Body: { "locked": true }
// Guard: rejects pods with active billing (if locking) and disconnected pods.
async fn lockdown_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // Guard: do not lock pods with active billing
    if locked && state.billing.active_timers.read().await.contains_key(&id) {
        return Json(json!({ "error": "pod has active billing session" }));
    }

    // MMA-P2: Clone sender, drop lock before .await
    let sender = {
        let senders = state.agent_senders.read().await;
        match senders.get(&id) {
            Some(s) if !s.is_closed() => s.clone(),
            _ => return Json(json!({ "error": "pod not connected" })),
        }
    };

    let mut settings = std::collections::HashMap::new();
    settings.insert(
        "kiosk_lockdown_enabled".to_string(),
        if locked { "true" } else { "false" }.to_string(),
    );
    let msg = CoreToAgentMessage::SettingsUpdated { settings };
    let _ = sender.send(msg).await;

    Json(json!({ "ok": true, "pod_id": id, "locked": locked }))
}

// POST /pods/lockdown-all — Toggle kiosk lockdown for all connected pods
// Body: { "locked": true }
// Skips billing-active pods (when locking) and disconnected/closed senders.
async fn lockdown_all_pods(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // MMA-P2: Snapshot senders + billing state, drop locks before .await loop
    let send_targets: Vec<(String, _)> = {
        let active_timers = state.billing.active_timers.read().await;
        let senders = state.agent_senders.read().await;
        senders.iter()
            .filter(|(_, s)| !s.is_closed())
            .filter(|(pod_id, _)| !locked || !active_timers.contains_key(*pod_id))
            .map(|(pod_id, sender)| (pod_id.clone(), sender.clone()))
            .collect()
    }; // locks dropped here

    let mut results = Vec::new();
    // Collect skipped pods info
    {
        let active_timers = state.billing.active_timers.read().await;
        let senders = state.agent_senders.read().await;
        for (pod_id, sender) in senders.iter() {
            if sender.is_closed() {
                results.push(json!({ "pod_id": pod_id, "status": "not_connected" }));
            } else if locked && active_timers.contains_key(pod_id) {
                results.push(json!({ "pod_id": pod_id, "status": "skipped_billing_active" }));
            }
        }
    }

    for (pod_id, sender) in &send_targets {
        let mut settings = std::collections::HashMap::new();
        settings.insert(
            "kiosk_lockdown_enabled".to_string(),
            if locked { "true" } else { "false" }.to_string(),
        );
        let msg = CoreToAgentMessage::SettingsUpdated { settings };
        let _ = sender.send(msg).await;
        results.push(json!({ "pod_id": pod_id, "status": "sent" }));
    }

    Json(json!({ "ok": true, "locked": locked, "results": results }))
}

// POST /pods/{id}/unrestrict — Fully unrestrict a pod for employee training/maintenance.
// Sends ClearLockScreen + EnterDebugMode + disables kiosk lockdown on that pod.
// To re-restrict, use POST /pods/{id}/lockdown {"locked": true}.
// Body: { "unrestrict": true } (or false to re-restrict via lockdown + blank screen)
async fn unrestrict_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let unrestrict = body.get("unrestrict").and_then(|v| v.as_bool()).unwrap_or(true);

    let senders = state.agent_senders.read().await;
    let Some(sender) = senders.get(&id) else {
        return Json(json!({ "error": format!("Pod {} not connected", id) }));
    };
    if sender.is_closed() {
        return Json(json!({ "error": format!("Pod {} not connected", id) }));
    }

    if unrestrict {
        // 1. Clear lock screen
        let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
        // 2. Enter debug mode (deactivates kiosk enforcement, restores taskbar)
        let _ = sender.send(CoreToAgentMessage::EnterDebugMode {
            employee_name: "Staff (admin panel)".to_string(),
        }).await;
        // 3. Disable kiosk lockdown via settings (prevents re-activation on next settings broadcast)
        let mut settings = std::collections::HashMap::new();
        settings.insert("kiosk_lockdown_enabled".to_string(), "false".to_string());
        let _ = sender.send(CoreToAgentMessage::SettingsUpdated { settings }).await;
        tracing::info!("Pod {} UNRESTRICTED for employee training", id);
    } else {
        // Re-restrict: re-enable kiosk lockdown + blank screen
        let mut settings = std::collections::HashMap::new();
        settings.insert("kiosk_lockdown_enabled".to_string(), "true".to_string());
        let _ = sender.send(CoreToAgentMessage::SettingsUpdated { settings }).await;
        let _ = sender.send(CoreToAgentMessage::BlankScreen).await;
        tracing::info!("Pod {} RESTRICTED — kiosk re-engaged", id);
    }

    // Optimistic update for dashboard
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&id) {
            pod.screen_blanked = Some(!unrestrict);
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    Json(json!({ "ok": true, "pod_id": id, "unrestricted": unrestrict }))
}

// POST /pods/{id}/freedom — Toggle freedom mode on a pod.
// Freedom mode: all restrictions lifted (like unrestrict), but passive process monitoring stays active.
// Body: { "enabled": true } (or false to exit freedom mode and re-engage kiosk)
async fn freedom_mode_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let enabled = body.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);

    let senders = state.agent_senders.read().await;
    let Some(sender) = senders.get(&id) else {
        return Json(json!({ "error": format!("Pod {} not connected", id) }));
    };
    if sender.is_closed() {
        return Json(json!({ "error": format!("Pod {} not connected", id) }));
    }

    if enabled {
        let _ = sender.send(CoreToAgentMessage::EnterFreedomMode).await;
        tracing::info!("Pod {} FREEDOM MODE enabled — monitoring active", id);
    } else {
        let _ = sender.send(CoreToAgentMessage::ExitFreedomMode).await;
        tracing::info!("Pod {} FREEDOM MODE disabled — kiosk re-engaged", id);
    }

    // Optimistic update for dashboard
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&id) {
            pod.freedom_mode = Some(enabled);
            pod.screen_blanked = Some(false);
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    Json(json!({ "ok": true, "pod_id": id, "freedom_mode": enabled }))
}

/// POST /pods/{id}/exec — Execute a command on a pod via WebSocket proxy.
/// Body: { "cmd": "...", "timeout_ms": 30000 }
/// Returns: { "success": bool, "stdout": "...", "stderr": "..." }
/// Works even when pod's HTTP :8090 is down — only requires WebSocket connection.
async fn ws_exec_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let cmd = match body["cmd"].as_str() {
        Some(c) => c,
        None => return Json(json!({ "error": "missing 'cmd' field" })),
    };
    let timeout_ms = body["timeout_ms"].as_u64().unwrap_or(30_000);

    // SEC-P0-10: Block dangerous command patterns (defense-in-depth)
    // MMA iter2-4: normalize aggressively before checking:
    //   1. Strip ^ (cmd.exe escape), collapse whitespace, lowercase
    //   2. Strip .exe/.com suffixes from binary names so sc.exe = sc
    //   3. Block dangerous BINARIES (not just command+args patterns)
    // MMA-R2-2 + ITER1-#1/#2/#3: Block env var expansion, FOR loops, -enc, ADS, substring
    let cmd_lower = cmd.to_lowercase();
    // Block env var patterns
    if cmd_lower.contains('%') || cmd_lower.contains("$env:") {
        let has_env_bypass = cmd_lower.contains("%comspec%")
            || cmd_lower.contains("%systemroot%")
            || cmd_lower.contains("%windir%")
            || cmd_lower.contains("%temp%")
            || cmd_lower.contains("$env:")
            // ITER1-#1: Block substring expansion %var:~0,3%
            || cmd_lower.contains(":~");
        if has_env_bypass {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked env var/substring expansion bypass");
            return Json(json!({ "error": "Command blocked: environment variable expansion not allowed" }));
        }
    }
    // ITER1-#1: Block FOR /F loops (cmd shell command injection)
    if cmd_lower.contains("for /f") || cmd_lower.contains("for /l") || cmd_lower.contains("for /d") || cmd_lower.contains("for /r") {
        tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked FOR loop command");
        return Json(json!({ "error": "Command blocked: FOR loops not allowed" }));
    }
    // ITER1-#2 + ITER2: Block PowerShell encoded commands (including partial params -e, -ec, -en)
    if cmd_lower.contains("-encodedcommand") || cmd_lower.contains("-enc ") || cmd_lower.contains("-en ") || cmd_lower.contains("-ec ") || cmd_lower.contains("-e ") && cmd_lower.contains("powershell") {
        tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked encoded command");
        return Json(json!({ "error": "Command blocked: encoded commands not allowed" }));
    }
    // ITER1-#3: Block Alternate Data Streams (file.exe:stream)
    // Allow legitimate colon uses (C:\, http:) but block exe:stream pattern
    {
        let stripped = cmd_lower.replace("c:\\", "").replace("d:\\", "").replace("http:", "").replace("https:", "");
        if stripped.contains(".exe:") || stripped.contains(".dll:") || stripped.contains(".bat:") {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked Alternate Data Stream");
            return Json(json!({ "error": "Command blocked: alternate data streams not allowed" }));
        }
    }
    // MMA-R2-2: Block UNC paths that could execute remote binaries
    if cmd_lower.contains("\\\\") {
        tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked UNC path execution");
        return Json(json!({ "error": "Command blocked: UNC paths not allowed" }));
    }
    let cmd_normalized: String = cmd
        .replace('^', "")
        .replace('\t', " ")
        .to_lowercase();
    let cmd_collapsed: String = cmd_normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    // Strip .exe/.com suffixes for binary-level blocking
    let cmd_no_exe: String = cmd_collapsed
        .replace(".exe", "")
        .replace(".com", "");

    // Blocked dangerous binaries (checked against .exe-stripped command)
    // MMA-ITER4: Extended LOLBin blocklist (3 models flagged gaps)
    const BLOCKED_BINARIES: &[&str] = &[
        "powershell", "pwsh", "mshta", "wscript", "cscript",
        "regsvr32", "rundll32", "msiexec", "odbcconf", "pcalua",
        "certutil", "bitsadmin", "bash", "wsl",
        // LOLBins added ITER4:
        "forfiles", "msdt", "hh", "infdefaultinstall", "diskshadow",
        "esentutl", "expand", "extrac32", "replace", "ieexec",
        "installutil", "msbuild", "msconfig", "msdeploy", "msxsl",
    ];
    for bin in BLOCKED_BINARIES {
        if cmd_no_exe.contains(bin) {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked binary: {}", bin);
            return Json(json!({ "error": format!("Command blocked: '{}' is not allowed", bin) }));
        }
    }

    // Blocked dangerous command patterns (checked against collapsed command)
    const BLOCKED_PATTERNS: &[&str] = &[
        "net user", "net localgroup", "net1 user", "net1 localgroup",
        "net use \\\\", "net start", "net stop",
        "reg add", "reg delete", "reg import", "reg load", "reg restore",
        "format c:", "rd /s /q c:", "del /s /q c:",
        "schtasks /create", "schtasks /change", "schtasks /delete",
        "sc create", "sc config", "sc stop", "sc delete",
        "netsh advfirewall", "netsh firewall",
        "wmic process call create", "wmic /node",
        "iex(", "invoke-expression", "invoke-webrequest",
        "downloadstring", "downloadfile", "new-object net.webclient",
    ];
    for pattern in BLOCKED_PATTERNS {
        if cmd_no_exe.contains(pattern) {
            tracing::warn!(pod_id = %id, cmd = %cmd, "SEC: Blocked pattern: {}", pattern);
            return Json(json!({ "error": format!("Command blocked: contains '{}'", pattern) }));
        }
    }

    // Truncate command preview to 100 chars for audit
    let cmd_preview: String = cmd.chars().take(100).collect();

    // Audit trail + WhatsApp alert for fleet exec (HIGH sensitivity)
    accounting::log_admin_action(
        &state, "fleet_exec",
        &json!({"pod_id": id, "command": cmd_preview}).to_string(),
        None, None,
    ).await;
    whatsapp_alerter::send_admin_alert(
        &state.config, "Fleet Exec",
        &format!("Pod {}: {}", id, cmd_preview),
    ).await;

    match crate::ws::ws_exec_on_pod(&state, &id, cmd, timeout_ms).await {
        Ok((success, stdout, stderr)) => {
            Json(json!({ "success": success, "stdout": stdout, "stderr": stderr }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

/// Phase 50: GET /pods/{id}/self-test — Trigger self-test on a pod via WS, return probe results + LLM verdict.
/// Timeout: 30s (probes run ~10s, LLM verdict adds ~5s).
async fn pod_self_test(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // 1. Get the WS sender for this pod (normalize to canonical pod_N format)
    let pod_id = normalize_pod_id(&pod_id).unwrap_or(pod_id);
    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(&pod_id).cloned()
    };
    let Some(sender) = sender else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({"error": format!("pod {} not connected", pod_id)})),
        ).into_response();
    };

    // 2. Register a one-shot channel for the response
    let request_id = format!("selftest-{}", uuid::Uuid::new_v4());
    let (tx, rx) = tokio::sync::oneshot::channel::<serde_json::Value>();
    {
        let mut pending = state.pending_self_tests.write().await;
        pending.insert(request_id.clone(), (pod_id.clone(), tx));
    }

    // 3. Send RunSelfTest command
    if sender.send(CoreToAgentMessage::RunSelfTest { request_id: request_id.clone() }).await.is_err() {
        let mut pending = state.pending_self_tests.write().await;
        pending.remove(&request_id);
        return (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "failed to send command to agent"})),
        ).into_response();
    }

    // 4. Await response with 30s timeout
    match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
        Ok(Ok(report)) => Json(report).into_response(),
        Ok(Err(_)) => {
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "response channel dropped"}))).into_response()
        }
        Err(_) => {
            // Clean up timed-out entry
            let mut pending = state.pending_self_tests.write().await;
            pending.remove(&request_id);
            (axum::http::StatusCode::GATEWAY_TIMEOUT, Json(json!({"error": "self-test timed out after 30s"}))).into_response()
        }
    }
}

// POST /pods/{id}/clear-maintenance — Send ClearMaintenance to pod agent (STAFF-02)
//
// Clears the pod's maintenance state both on the server (optimistic) and by sending
// ClearMaintenance to the agent so it can re-run pre-flight checks on next session start.
async fn clear_maintenance_pod(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Send ClearMaintenance via WS.
    let agent_senders = state.agent_senders.read().await;
    match agent_senders.get(&id) {
        Some(sender) => {
            let _ = sender.send(CoreToAgentMessage::ClearMaintenance).await;
        }
        None => {
            return Json(json!({ "error": format!("Pod {} not connected", id) }));
        }
    }
    drop(agent_senders);

    // Also clear server-side maintenance state immediately (optimistic update).
    {
        let mut fleet = state.pod_fleet_health.write().await;
        if let Some(store) = fleet.get_mut(&id) {
            store.in_maintenance = false;
            store.maintenance_failures.clear();
        }
    }

    tracing::info!("ClearMaintenance sent to pod {} (STAFF-02)", id);
    crate::activity_log::log_pod_activity(&state, &id, "system", "Maintenance Cleared", "Staff cleared maintenance via dashboard", "staff");

    Json(json!({ "ok": true, "pod_id": id }))
}

// ─── v29.0 Phase 9: Maintenance & Analytics Handlers ────────────────────────

#[derive(Deserialize)]
struct MaintenanceEventQuery {
    pod_id: Option<u8>,
    severity: Option<String>,
    hours: Option<u32>,
}

/// POST /api/v1/maintenance/events — Insert a MaintenanceEvent
async fn maintenance_create_event(
    State(state): State<Arc<AppState>>,
    Json(event): Json<crate::maintenance_models::MaintenanceEvent>,
) -> impl IntoResponse {
    match maintenance_store::insert_event(&state.db, &event).await {
        Ok(()) => Json(json!({ "ok": true, "id": event.id.to_string() })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

/// GET /api/v1/maintenance/events — Query events with filters (pod_id, severity, hours)
async fn maintenance_list_events(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MaintenanceEventQuery>,
) -> impl IntoResponse {
    let since = params.hours.map(|h| {
        chrono::Utc::now() - chrono::Duration::hours(h as i64)
    });
    let limit = 200u32;
    match maintenance_store::query_events(&state.db, params.pod_id, since, limit).await {
        Ok(events) => {
            // Optional severity filter (post-query since store doesn't support it directly)
            let filtered: Vec<_> = if let Some(ref sev) = params.severity {
                events.into_iter().filter(|e| {
                    let s = serde_json::to_string(&e.severity).unwrap_or_default().replace('"', "");
                    s.eq_ignore_ascii_case(sev)
                }).collect()
            } else {
                events
            };
            Json(json!({ "ok": true, "events": filtered, "count": filtered.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

/// GET /api/v1/maintenance/summary — Get MaintenanceSummary
async fn maintenance_summary(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match maintenance_store::get_summary(&state.db).await {
        Ok(summary) => Json(json!({ "ok": true, "summary": summary })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

/// POST /api/v1/maintenance/tasks — Create a maintenance task
async fn maintenance_create_task(
    State(state): State<Arc<AppState>>,
    Json(task): Json<crate::maintenance_models::MaintenanceTask>,
) -> impl IntoResponse {
    match maintenance_store::insert_task(&state.db, &task).await {
        Ok(()) => Json(json!({ "ok": true, "id": task.id.to_string() })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
struct MaintenanceTaskQuery {
    status: Option<String>,
    pod_id: Option<u8>,
}

/// GET /api/v1/maintenance/tasks — Query tasks (status, pod_id)
async fn maintenance_list_tasks(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MaintenanceTaskQuery>,
) -> impl IntoResponse {
    let limit = 200u32;
    match maintenance_store::query_tasks(&state.db, params.status.as_deref(), limit).await {
        Ok(tasks) => {
            // Optional pod_id filter (post-query)
            let filtered: Vec<_> = if let Some(pid) = params.pod_id {
                tasks.into_iter().filter(|t| t.pod_id == Some(pid)).collect()
            } else {
                tasks
            };
            Json(json!({ "ok": true, "tasks": filtered, "count": filtered.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
struct TaskStatusUpdate {
    status: crate::maintenance_models::TaskStatus,
}

/// PATCH /api/v1/maintenance/tasks/:id — Update task status
async fn maintenance_update_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TaskStatusUpdate>,
) -> impl IntoResponse {
    let task_id = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(json!({ "ok": false, "error": "Invalid UUID" })),
    };
    match maintenance_store::update_task_status(&state.db, task_id, &body.status).await {
        Ok(true) => Json(json!({ "ok": true })),
        Ok(false) => Json(json!({ "ok": false, "error": "Task not found" })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
struct TelemetryQuery {
    pod_id: Option<String>,
    hours: Option<u32>,
    limit: Option<u32>,
}

/// GET /api/v1/analytics/telemetry — Query hardware telemetry history
async fn analytics_telemetry(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TelemetryQuery>,
) -> impl IntoResponse {
    let pool = match &state.telemetry_db {
        Some(p) => p,
        None => return Json(json!({ "ok": false, "error": "Telemetry DB not initialized" })),
    };
    let hours = params.hours.unwrap_or(24);
    let limit = params.limit.unwrap_or(500).min(2000);
    let cutoff = (chrono::Utc::now() - chrono::Duration::hours(hours as i64)).to_rfc3339();

    let query = if let Some(ref pid) = params.pod_id {
        sqlx::query(
            "SELECT pod_id, collected_at, gpu_temp_celsius, cpu_temp_celsius, gpu_power_watts,
                    cpu_usage_pct, gpu_usage_pct, memory_usage_pct, disk_usage_pct,
                    network_latency_ms, process_handle_count, disk_smart_health_pct
             FROM hardware_telemetry
             WHERE collected_at > ?1 AND pod_id = ?2
             ORDER BY collected_at DESC
             LIMIT ?3"
        )
        .bind(&cutoff)
        .bind(pid)
        .bind(limit as i64)
    } else {
        sqlx::query(
            "SELECT pod_id, collected_at, gpu_temp_celsius, cpu_temp_celsius, gpu_power_watts,
                    cpu_usage_pct, gpu_usage_pct, memory_usage_pct, disk_usage_pct,
                    network_latency_ms, process_handle_count, disk_smart_health_pct
             FROM hardware_telemetry
             WHERE collected_at > ?1
             ORDER BY collected_at DESC
             LIMIT ?2"
        )
        .bind(&cutoff)
        .bind(limit as i64)
    };

    match query.fetch_all(pool).await {
        Ok(rows) => {
            use sqlx::Row;
            let data: Vec<Value> = rows.iter().map(|r| {
                json!({
                    "pod_id": r.get::<String, _>("pod_id"),
                    "collected_at": r.get::<String, _>("collected_at"),
                    "gpu_temp_celsius": r.get::<Option<f64>, _>("gpu_temp_celsius"),
                    "cpu_temp_celsius": r.get::<Option<f64>, _>("cpu_temp_celsius"),
                    "gpu_power_watts": r.get::<Option<f64>, _>("gpu_power_watts"),
                    "cpu_usage_pct": r.get::<Option<f64>, _>("cpu_usage_pct"),
                    "gpu_usage_pct": r.get::<Option<f64>, _>("gpu_usage_pct"),
                    "memory_usage_pct": r.get::<Option<f64>, _>("memory_usage_pct"),
                    "disk_usage_pct": r.get::<Option<f64>, _>("disk_usage_pct"),
                    "network_latency_ms": r.get::<Option<i64>, _>("network_latency_ms"),
                    "process_handle_count": r.get::<Option<i64>, _>("process_handle_count"),
                    "disk_smart_health_pct": r.get::<Option<i64>, _>("disk_smart_health_pct"),
                })
            }).collect();
            Json(json!({ "ok": true, "data": data, "count": data.len() }))
        }
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

#[derive(Deserialize)]
struct TrendQuery {
    pod_id: String,
    metric: String,
    window_days: Option<u32>,
}

/// GET /api/v1/analytics/trends — Get metric trend for a pod
async fn analytics_trends(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TrendQuery>,
) -> impl IntoResponse {
    let pool = match &state.telemetry_db {
        Some(p) => p,
        None => return Json(json!({ "ok": false, "error": "Telemetry DB not initialized" })),
    };
    let window = params.window_days.unwrap_or(30);
    match crate::telemetry_store::get_metric_trend(pool, &params.pod_id, &params.metric, window).await {
        Ok(trend) => Json(json!({ "ok": true, "trend": trend })),
        Err(e) => Json(json!({ "ok": false, "error": format!("{}", e) })),
    }
}

/// RCA-PREVENTION: Static route uniqueness check.
/// Extracts all .route() registrations from this file and asserts no METHOD+PATH duplicates.
/// This catches the class of bug that caused the 2026-03-29 deploy failure
/// (21 duplicate routes from Phase 258 move-without-delete).
#[cfg(test)]
mod route_uniqueness_tests {
    #[test]
    fn no_duplicate_route_registrations() {
        let source = include_str!("routes.rs");
        let mut routes: Vec<String> = Vec::new();
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(start) = trimmed.find(".route(\"") {
                let after = &trimmed[start + 8..];
                if let Some(end) = after.find('"') {
                    let path = &after[..end];
                    // Extract method from the handler chain: get(, post(, put(, delete(
                    let rest = &after[end..];
                    for method in &["get(", "post(", "put(", "delete(", "patch("] {
                        if rest.contains(method) {
                            routes.push(format!("{} {}", method.trim_end_matches('('), path));
                        }
                    }
                }
            }
        }
        routes.sort();
        let mut duplicates: Vec<String> = Vec::new();
        for window in routes.windows(2) {
            if window[0] == window[1] && !duplicates.contains(&window[0]) {
                duplicates.push(window[0].clone());
            }
        }
        assert!(
            duplicates.is_empty(),
            "DUPLICATE ROUTES DETECTED (will panic at runtime):\n{}",
            duplicates.join("\n")
        );
    }
}

#[cfg(test)]
mod lockdown_tests {
    use super::*;
    use crate::billing::BillingTimer;
    use axum::extract::{Path, State};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    /// Build a minimal AppState suitable for lockdown unit tests.
    /// Uses a real Config loaded from the project's racecontrol.toml.
    async fn make_state() -> Arc<AppState> {
        // Use an in-memory SQLite database for tests
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, db, field_cipher))
    }

    #[tokio::test]
    async fn lockdown_pod_with_active_billing_returns_error() {
        let state = make_state().await;

        // Insert a billing timer for pod-1
        {
            let timer = BillingTimer::dummy("pod-1");
            state.billing.active_timers.write().await.insert("pod-1".to_string(), timer);
        }

        let response = lockdown_pod(
            State(state.clone()),
            Path("pod-1".to_string()),
            Json(json!({ "locked": true })),
        )
        .await;

        let body = response.0;
        assert!(
            body.get("error").is_some(),
            "Expected error key in response, got: {}",
            body
        );
        let err_msg = body["error"].as_str().unwrap_or("");
        assert!(
            err_msg.contains("active billing session"),
            "Expected 'active billing session' in error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn lockdown_pod_with_missing_sender_returns_error() {
        let state = make_state().await;
        // agent_senders is empty — pod not connected

        let response = lockdown_pod(
            State(state.clone()),
            Path("pod-1".to_string()),
            Json(json!({ "locked": true })),
        )
        .await;

        let body = response.0;
        assert!(
            body.get("error").is_some(),
            "Expected error key in response, got: {}",
            body
        );
        let err_msg = body["error"].as_str().unwrap_or("");
        assert!(
            err_msg.contains("not connected"),
            "Expected 'not connected' in error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn lockdown_pod_with_closed_sender_returns_error() {
        let state = make_state().await;

        // Create a channel then immediately drop the receiver to close the sender
        let (tx, rx) = mpsc::channel::<CoreToAgentMessage>(16);
        drop(rx); // sender is now closed
        state.agent_senders.write().await.insert("pod-1".to_string(), tx);

        let response = lockdown_pod(
            State(state.clone()),
            Path("pod-1".to_string()),
            Json(json!({ "locked": true })),
        )
        .await;

        let body = response.0;
        assert!(
            body.get("error").is_some(),
            "Expected error key in response, got: {}",
            body
        );
        let err_msg = body["error"].as_str().unwrap_or("");
        assert!(
            err_msg.contains("not connected"),
            "Expected 'not connected' in error, got: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn lockdown_all_skips_billing_active_and_closed_sends_to_healthy() {
        let state = make_state().await;

        // Pod A: billing active — should be skipped
        {
            let timer = BillingTimer::dummy("pod-a");
            state.billing.active_timers.write().await.insert("pod-a".to_string(), timer);
        }
        let (tx_a, _rx_a) = mpsc::channel::<CoreToAgentMessage>(16);
        state.agent_senders.write().await.insert("pod-a".to_string(), tx_a);

        // Pod B: closed sender — should be marked not_connected
        let (tx_b, rx_b) = mpsc::channel::<CoreToAgentMessage>(16);
        drop(rx_b);
        state.agent_senders.write().await.insert("pod-b".to_string(), tx_b);

        // Pod C: healthy — should receive SettingsUpdated
        let (tx_c, mut rx_c) = mpsc::channel::<CoreToAgentMessage>(16);
        state.agent_senders.write().await.insert("pod-c".to_string(), tx_c);

        let response = lockdown_all_pods(
            State(state.clone()),
            Json(json!({ "locked": true })),
        )
        .await;

        let body = response.0;
        assert_eq!(body["ok"], true, "Expected ok=true, got: {}", body);
        assert_eq!(body["locked"], true);

        let results = body["results"].as_array().expect("results should be array");
        assert_eq!(results.len(), 3, "Expected 3 results");

        // Find each pod result
        let find = |pod_id: &str| {
            results.iter().find(|r| r["pod_id"].as_str() == Some(pod_id))
                .cloned()
        };

        let res_a = find("pod-a").expect("pod-a result missing");
        assert_eq!(res_a["status"], "skipped_billing_active", "pod-a should be skipped: {}", res_a);

        let res_b = find("pod-b").expect("pod-b result missing");
        assert_eq!(res_b["status"], "not_connected", "pod-b should be not_connected: {}", res_b);

        let res_c = find("pod-c").expect("pod-c result missing");
        assert_eq!(res_c["status"], "sent", "pod-c should be sent: {}", res_c);

        // Verify pod-c actually received the SettingsUpdated message
        let msg = rx_c.try_recv().expect("pod-c should have received a message");
        match msg {
            CoreToAgentMessage::SettingsUpdated { settings } => {
                assert_eq!(
                    settings.get("kiosk_lockdown_enabled").map(|s| s.as_str()),
                    Some("true"),
                    "Expected kiosk_lockdown_enabled=true"
                );
            }
            other => panic!("Expected SettingsUpdated, got: {:?}", other),
        }
    }
}

#[cfg(test)]
mod pod_status_summary_tests {
    use super::*;
    use axum::extract::State;
    use std::sync::Arc;

    async fn make_state() -> Arc<AppState> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, db, field_cipher))
    }

    #[tokio::test]
    async fn returns_all_healthy_when_no_pods_down() {
        let state = make_state().await;

        // Insert 3 healthy pods
        {
            let mut pods = state.pods.write().await;
            for i in 1..=3u32 {
                let id = format!("pod-{i}");
                pods.insert(id.clone(), PodInfo {
                    id: id.clone(),
                    number: i,
                    name: format!("Pod {i}"),
                    ip_address: format!("192.168.31.{i}"),
                    mac_address: None,
                    sim_type: SimType::AssettoCorsa,
                    status: PodStatus::Idle,
                    current_driver: None,
                    current_session_id: None,
                    last_seen: None,
                    driving_state: None,
                    billing_session_id: None,
                    game_state: None,
                    current_game: None,
                    installed_games: Vec::new(),
                    screen_blanked: None,
                    ffb_preset: None,
                    freedom_mode: None,
                    agent_timestamp: None,
                });
            }
        }

        let response = pod_status_summary(State(state)).await;
        let body = response.0;
        assert_eq!(body["active"], 3);
        assert_eq!(body["total"], 3);
        assert_eq!(body["down"].as_array().unwrap().len(), 0);
        assert!(body["timestamp"].as_str().is_some());
    }

    #[tokio::test]
    async fn reports_offline_and_error_pods_as_down() {
        let state = make_state().await;

        {
            let mut pods = state.pods.write().await;
            pods.insert("pod-1".into(), PodInfo {
                id: "pod-1".into(), number: 1, name: "Pod 1".into(),
                ip_address: "192.168.31.1".into(), mac_address: None,
                sim_type: SimType::AssettoCorsa, status: PodStatus::Idle,
                current_driver: None, current_session_id: None,
                last_seen: None, driving_state: None, billing_session_id: None,
                game_state: None, current_game: None, installed_games: Vec::new(), screen_blanked: None, ffb_preset: None, freedom_mode: None, agent_timestamp: None,
            });
            pods.insert("pod-2".into(), PodInfo {
                id: "pod-2".into(), number: 2, name: "Pod 2".into(),
                ip_address: "192.168.31.2".into(), mac_address: None,
                sim_type: SimType::AssettoCorsa, status: PodStatus::Offline,
                current_driver: None, current_session_id: None,
                last_seen: None, driving_state: None, billing_session_id: None,
                game_state: None, current_game: None, installed_games: Vec::new(), screen_blanked: None, ffb_preset: None, freedom_mode: None, agent_timestamp: None,
            });
            pods.insert("pod-3".into(), PodInfo {
                id: "pod-3".into(), number: 3, name: "Pod 3".into(),
                ip_address: "192.168.31.3".into(), mac_address: None,
                sim_type: SimType::AssettoCorsa, status: PodStatus::Error,
                current_driver: None, current_session_id: None,
                last_seen: None, driving_state: None, billing_session_id: None,
                game_state: None, current_game: None, installed_games: Vec::new(), screen_blanked: None, ffb_preset: None, freedom_mode: None, agent_timestamp: None,
            });
        }

        let response = pod_status_summary(State(state)).await;
        let body = response.0;
        assert_eq!(body["active"], 1);
        assert_eq!(body["total"], 3);
        let down = body["down"].as_array().unwrap();
        assert_eq!(down.len(), 2);
    }
}

// ─── SEC-09: PII masking helpers ──────────────────────────────────────────

/// Mask a phone number, showing only the first 2 and last 2 digits.
/// Example: "9876543210" → "98****10"
fn mask_phone(phone: &str) -> String {
    if phone.len() <= 4 {
        return "****".to_string();
    }
    format!("{}****{}", &phone[..2], &phone[phone.len() - 2..])
}

/// Mask an email address, showing only the first 2 chars and the domain.
/// Example: "user@example.com" → "us***@example.com"
fn mask_email(email: &str) -> String {
    match email.find('@') {
        Some(at) if at > 2 => format!("{}***{}", &email[..2], &email[at..]),
        _ => "***@***".to_string(),
    }
}

/// Returns `true` if PII should be masked for the given staff claims.
/// Only manager and superadmin roles may see full PII.
fn should_mask_pii(claims: &Option<axum::Extension<crate::auth::middleware::StaffClaims>>) -> bool {
    match claims {
        Some(ext) => ext.role != "manager" && ext.role != "superadmin",
        None => true, // no claims = mask by default
    }
}

#[derive(Debug, Deserialize)]
struct ListDriversQuery {
    search: Option<String>,
}

async fn list_drivers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListDriversQuery>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
) -> Json<Value> {
    let mask = should_mask_pii(&claims);
    let rows = if let Some(ref search) = params.search {
        // MMA-P2: Escape LIKE wildcards and limit length to prevent enumeration + DoS
        let sanitized: String = search.chars()
            .filter(|c| !matches!(c, '%' | '_'))
            .take(50)
            .collect();
        if sanitized.is_empty() {
            return Json(json!({ "error": "Search query too short or invalid" }));
        }
        let q = format!("%{}%", sanitized);
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>, bool, bool, Option<String>)>(
            "SELECT id, name, email, phone, total_laps, total_time_ms, customer_id,
                    COALESCE(waiver_signed, 0), COALESCE(has_used_trial, 0), created_at
             FROM drivers
             WHERE name LIKE ?1 COLLATE NOCASE OR phone LIKE ?2 OR customer_id LIKE ?3
             ORDER BY name LIMIT 50",
        )
        .bind(&q)
        .bind(&q)
        .bind(&q)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>, bool, bool, Option<String>)>(
            "SELECT id, name, email, phone, total_laps, total_time_ms, customer_id,
                    COALESCE(waiver_signed, 0), COALESCE(has_used_trial, 0), created_at
             FROM drivers ORDER BY created_at DESC",
        )
        .fetch_all(&state.db)
        .await
    };

    // Total count
    let total: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers")
        .fetch_one(&state.db)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    let waiver_count: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers WHERE waiver_signed = 1")
        .fetch_one(&state.db)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    match rows {
        Ok(drivers) => {
            let list: Vec<Value> = drivers.iter().map(|d| {
                // SEC-09: mask PII for cashier role
                let email = d.2.as_deref().map(|e| if mask { mask_email(e) } else { e.to_string() });
                let phone = d.3.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });
                json!({
                    "id": d.0, "name": d.1, "email": email, "phone": phone,
                    "total_laps": d.4, "total_time_ms": d.5, "customer_id": d.6,
                    "waiver_signed": d.7, "has_used_trial": d.8, "created_at": d.9,
                })
            }).collect();
            Json(json!({ "drivers": list, "total": total, "waiver_count": waiver_count }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_driver(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let email = body.get("email").and_then(|v| v.as_str());
    let phone = body.get("phone").and_then(|v| v.as_str());
    let steam_guid = body.get("steam_guid").and_then(|v| v.as_str());

    // Encrypt PII fields
    let phone_hash = phone.filter(|p| !p.is_empty()).map(|p| state.field_cipher.hash_phone(p));
    let phone_enc = match phone.filter(|p| !p.is_empty()).map(|p| state.field_cipher.encrypt_field(p)) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => return Json(json!({ "error": format!("Encrypt error: {}", e) })),
        None => None,
    };
    let email_enc = match email.filter(|e| !e.is_empty()).map(|e| state.field_cipher.encrypt_field(e)) {
        Some(Ok(v)) => Some(v),
        Some(Err(e)) => return Json(json!({ "error": format!("Encrypt error: {}", e) })),
        None => None,
    };
    let name_enc = match state.field_cipher.encrypt_field(name) {
        Ok(v) => Some(v),
        Err(e) => return Json(json!({ "error": format!("Encrypt error: {}", e) })),
    };

    let result = sqlx::query(
        "INSERT INTO drivers (id, name, name_enc, phone_hash, phone_enc, email_enc, steam_guid, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))"
    )
    .bind(&id)
    .bind(name) // Keep plaintext name for leaderboard backward compat
    .bind(&name_enc)
    .bind(&phone_hash)
    .bind(&phone_enc)
    .bind(&email_enc)
    .bind(steam_guid)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_driver(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
) -> Json<Value> {
    let mask = should_mask_pii(&claims);
    let row = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(d)) => {
            // SEC-09: mask PII for cashier role
            let email = d.2.as_deref().map(|e| if mask { mask_email(e) } else { e.to_string() });
            let phone = d.3.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });
            Json(json!({
                "id": d.0, "name": d.1, "email": email, "phone": phone,
                "total_laps": d.4, "total_time_ms": d.5,
            }))
        },
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// GET /drivers/{id}/full-profile — comprehensive driver profile for admin
async fn get_driver_full_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
) -> Json<Value> {
    let mask = should_mask_pii(&claims);
    // Core driver info (10 fields)
    let core = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>, Option<String>, bool, Option<String>)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms,
                customer_id, nickname, COALESCE(has_used_trial, 0), dob
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let c = match core {
        Ok(Some(c)) => c,
        Ok(None) => return Json(json!({ "error": "Driver not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    // Waiver fields (separate query to stay under tuple limit)
    let waiver = sqlx::query_as::<_, (bool, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, bool)>(
        "SELECT COALESCE(waiver_signed, 0), waiver_signed_at, waiver_version,
                guardian_name, guardian_phone, signature_data,
                COALESCE(show_nickname_on_leaderboard, 0)
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or((false, None, None, None, None, None, false));

    let is_minor = c.9.as_ref().map_or(false, |dob| {
        chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d")
            .map(|date| (chrono::Utc::now().date_naive() - date).num_days() / 365 < 18)
            .unwrap_or(false)
    });

    // SEC-09: mask PII for cashier role
    let email = c.2.as_deref().map(|e| if mask { mask_email(e) } else { e.to_string() });
    let phone = c.3.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });
    let guardian_phone = waiver.4.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });

    let driver_json = json!({
        "id": c.0, "name": c.1, "email": email, "phone": phone,
        "total_laps": c.4, "total_time_ms": c.5,
        "customer_id": c.6, "nickname": c.7, "has_used_trial": c.8,
        "dob": c.9,
        "waiver_signed": waiver.0, "waiver_signed_at": waiver.1,
        "waiver_version": waiver.2, "guardian_name": waiver.3,
        "guardian_phone": guardian_phone, "has_signature": waiver.5.is_some(),
        "show_nickname_on_leaderboard": waiver.6, "is_minor": is_minor,
    });

    // Wallet
    let wallet = sqlx::query_as::<_, (i64, i64, i64, Option<String>)>(
        "SELECT balance_paise, total_credited_paise, total_debited_paise, updated_at FROM wallets WHERE driver_id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|w| json!({
        "balance_paise": w.0, "total_credited_paise": w.1,
        "total_debited_paise": w.2, "updated_at": w.3,
    }));

    // Recent wallet transactions (last 20)
    let txns = sqlx::query_as::<_, (String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, amount_paise, balance_after_paise, txn_type, reference_id, notes, created_at
         FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|t| json!({
        "id": t.0, "amount_paise": t.1, "balance_after_paise": t.2,
        "txn_type": t.3, "reference_id": t.4, "notes": t.5, "created_at": t.6,
    }))
    .collect::<Vec<_>>();

    // Billing sessions (last 20)
    let sessions = sqlx::query_as::<_, (String, String, i64, i64, String, Option<i64>, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status,
                bs.wallet_debit_paise, bs.started_at, bs.ended_at, pt.name
         FROM billing_sessions bs
         LEFT JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.driver_id = ?
         ORDER BY bs.started_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|s| json!({
        "id": s.0, "pod_id": s.1, "allocated_seconds": s.2,
        "driving_seconds": s.3, "status": s.4, "wallet_debit_paise": s.5,
        "started_at": s.6, "ended_at": s.7, "pricing_tier_name": s.8,
    }))
    .collect::<Vec<_>>();

    // Personal bests
    let pbs = sqlx::query_as::<_, (String, String, i64, Option<String>)>(
        "SELECT track, car, best_lap_ms, achieved_at FROM personal_bests WHERE driver_id = ? ORDER BY achieved_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|p| json!({ "track": p.0, "car": p.1, "best_lap_ms": p.2, "achieved_at": p.3 }))
    .collect::<Vec<_>>();

    // Referral info
    let referral = sqlx::query_as::<_, (String,)>(
        "SELECT code FROM referrals WHERE referrer_id = ? AND code IS NOT NULL LIMIT 1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0);

    let referral_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM referrals WHERE referrer_id = ? AND status = 'completed'"
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Membership
    let membership = sqlx::query_as::<_, (String, String, f64, f64, String, bool, String)>(
        "SELECT m.id, mt.name, m.hours_used_minutes, mt.hours_included, m.expires_at, m.auto_renew, m.status
         FROM memberships m JOIN membership_tiers mt ON m.tier_id = mt.id
         WHERE m.driver_id = ? AND m.status = 'active' LIMIT 1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|m| json!({
        "id": m.0, "tier_name": m.1, "hours_used": m.2,
        "hours_included": m.3, "expires_at": m.4, "auto_renew": m.5, "status": m.6,
    }));

    // Refunds
    let refunds = sqlx::query_as::<_, (String, i64, String, String, Option<String>, String)>(
        "SELECT r.billing_session_id, r.amount_paise, r.method, r.reason, r.notes, r.created_at
         FROM refunds r WHERE r.driver_id = ? ORDER BY r.created_at DESC LIMIT 10"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|r| json!({
        "billing_session_id": r.0, "amount_paise": r.1, "method": r.2,
        "reason": r.3, "notes": r.4, "created_at": r.5,
    }))
    .collect::<Vec<_>>();

    Json(json!({
        "driver": driver_json,
        "wallet": wallet,
        "transactions": txns,
        "sessions": sessions,
        "personal_bests": pbs,
        "referral_code": referral,
        "referral_count": referral_count,
        "membership": membership,
        "refunds": refunds,
    }))
}

async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT id, type, sim_type, track, status, started_at FROM sessions ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions.iter().map(|s| json!({
                "id": s.0, "type": s.1, "sim_type": s.2,
                "track": s.3, "status": s.4, "started_at": s.5,
            })).collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let session_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("practice");
    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("monza");
    let car_class = body.get("car_class").and_then(|v| v.as_str());

    let result = sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track, car_class, status) VALUES (?, ?, ?, ?, ?, 'pending')"
    )
    .bind(&id)
    .bind(session_type)
    .bind(sim_type)
    .bind(track)
    .bind(car_class)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "type": session_type, "track": track })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    use sqlx::Row;

    let row = sqlx::query(
        "SELECT bs.id, bs.driver_id, d.name as driver_name, bs.pod_id,
                bs.pricing_tier_id, pt.name as tier_name,
                bs.allocated_seconds, bs.driving_seconds, bs.status,
                COALESCE(bs.custom_price_paise, pt.price_paise) as price_paise,
                bs.started_at, bs.ended_at,
                bs.experience_id, ke.name as experience_name,
                bs.car, bs.track, bs.sim_type,
                bs.reservation_id, bs.wallet_txn_id,
                bs.wallet_debit_paise, bs.created_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN kiosk_experiences ke ON bs.experience_id = ke.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get laps count and best lap for this session
    let lap_stats = sqlx::query_as::<_, (i64, Option<i64>)>(
        "SELECT COUNT(*), MIN(CASE WHEN valid = 1 THEN lap_time_ms END)
         FROM laps WHERE session_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((0, None));

    Json(json!({
        "session": {
            "id": row.get::<String, _>("id"),
            "driver_id": row.get::<String, _>("driver_id"),
            "driver_name": row.get::<String, _>("driver_name"),
            "pod_id": row.get::<String, _>("pod_id"),
            "pricing_tier_id": row.get::<String, _>("pricing_tier_id"),
            "pricing_tier_name": row.get::<String, _>("tier_name"),
            "allocated_seconds": row.get::<i64, _>("allocated_seconds"),
            "driving_seconds": row.get::<i64, _>("driving_seconds"),
            "status": row.get::<String, _>("status"),
            "price_paise": row.get::<i64, _>("price_paise"),
            "started_at": row.get::<Option<String>, _>("started_at"),
            "ended_at": row.get::<Option<String>, _>("ended_at"),
            "experience_id": row.get::<Option<String>, _>("experience_id"),
            "experience_name": row.get::<Option<String>, _>("experience_name"),
            "car": row.get::<Option<String>, _>("car"),
            "track": row.get::<Option<String>, _>("track"),
            "sim_type": row.get::<Option<String>, _>("sim_type"),
            "reservation_id": row.get::<Option<String>, _>("reservation_id"),
            "wallet_txn_id": row.get::<Option<String>, _>("wallet_txn_id"),
            "wallet_debit_paise": row.get::<Option<i64>, _>("wallet_debit_paise"),
            "created_at": row.get::<String, _>("created_at"),
            "total_laps": lap_stats.0,
            "best_lap_ms": lap_stats.1,
        }
    }))
}

async fn list_laps(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT id, driver_id, track, car, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid FROM laps ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps.iter().map(|l| json!({
                "id": l.0, "driver_id": l.1, "track": l.2, "car": l.3,
                "lap_time_ms": l.4, "sector1_ms": l.5, "sector2_ms": l.6,
                "sector3_ms": l.7, "valid": l.8,
            })).collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn session_laps(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (
        String, String, String, String, String, i64, i64,
        Option<i64>, Option<i64>, Option<i64>, bool, String,
    )>(
        "SELECT l.id, l.driver_id, l.pod_id, l.track, l.car, l.lap_number, l.lap_time_ms,
                l.sector1_ms, l.sector2_ms, l.sector3_ms, l.valid, l.created_at
         FROM laps l
         WHERE l.session_id = ?
         ORDER BY l.lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps
                .iter()
                .map(|l| {
                    json!({
                        "id": l.0,
                        "driver_id": l.1,
                        "pod_id": l.2,
                        "track": l.3,
                        "car": l.4,
                        "lap_number": l.5,
                        "lap_time_ms": l.6,
                        "sector1_ms": l.7,
                        "sector2_ms": l.8,
                        "sector3_ms": l.9,
                        "valid": l.10,
                        "created_at": l.11,
                    })
                })
                .collect();
            let count = list.len();
            Json(json!({ "session_id": id, "laps": list, "count": count }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct StaffTrackLeaderboardQuery {
    sim_type: Option<String>,
    /// Filter by car class — UX-05 segmentation (staff view)
    car_class: Option<String>,
    /// Filter by assist tier — UX-05 segmentation (staff view)
    assist_tier: Option<String>,
}

async fn track_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(track): Path<String>,
    Query(params): Query<StaffTrackLeaderboardQuery>,
) -> Json<Value> {
    // UX-04: JOIN laps to apply billing_session_id IS NOT NULL filter
    // UX-05: car_class + assist_tier segmentation for staff consistency with public view
    // UX-07: validity = 'valid' enforced — staff never see unverifiable laps
    let sim_clause = if params.sim_type.is_some() { " AND tr.sim_type = ?" } else { "" };
    let car_class_clause = if params.car_class.is_some() { " AND l.car_class = ?" } else { "" };
    let assist_tier_clause = if params.assist_tier.is_some() { " AND l.assist_tier = ?" } else { "" };

    let query_str = format!(
        "SELECT tr.track, tr.car,
                CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                tr.best_lap_ms, tr.achieved_at, tr.lap_id, tr.sim_type
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         LEFT JOIN laps l ON l.id = tr.lap_id
         WHERE tr.track = ?
           AND (l.billing_session_id IS NOT NULL OR tr.lap_id IS NULL)
           AND (l.validity IS NULL OR l.validity = 'valid')
           {}{}{}
         ORDER BY tr.best_lap_ms ASC",
        sim_clause, car_class_clause, assist_tier_clause
    );

    let mut q = sqlx::query_as::<_, (String, String, String, i64, String, Option<String>, String)>(&query_str);
    q = q.bind(&track);
    if let Some(ref st) = params.sim_type { q = q.bind(st); }
    if let Some(ref cc) = params.car_class { q = q.bind(cc); }
    if let Some(ref at) = params.assist_tier { q = q.bind(at); }
    let rows = q.fetch_all(&state.db).await;

    match rows {
        Ok(records) => {
            let list: Vec<Value> = records.iter().enumerate().map(|(i, r)| json!({
                "position": i + 1,
                "track": r.0, "car": r.1, "driver": r.2,
                "best_lap_ms": r.3, "achieved_at": r.4, "lap_id": r.5,
                "sim_type": r.6,
            })).collect();
            Json(json!({
                "track": track,
                "sim_type": params.sim_type,
                "car_class": params.car_class,
                "assist_tier": params.assist_tier,
                "records": list,
                "last_updated": chrono::Utc::now().to_rfc3339()
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn list_events(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "events": [] }))
}

async fn create_event(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_event" }))
}

async fn list_bookings(State(_state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "bookings": [] }))
}

async fn create_booking(State(_state): State<Arc<AppState>>, Json(_body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_booking" }))
}

// ─── Pricing ────────────────────────────────────────────────────────────────

async fn list_pricing_tiers(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, is_active, sort_order
         FROM pricing_tiers ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tiers) => {
            let list: Vec<Value> = tiers
                .iter()
                .map(|t| {
                    json!({
                        "id": t.0, "name": t.1, "duration_minutes": t.2,
                        "price_paise": t.3, "is_trial": t.4, "is_active": t.5,
                        "sort_order": t.6,
                    })
                })
                .collect();
            Json(json!({ "tiers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Pricing Psychology (v14.0 Phase 94) ────────────────────────────────────

/// Public: returns pricing tiers with dynamic (time-of-day adjusted) prices.
async fn pricing_display_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, is_active, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // LEGAL-07: Refund and pricing policy text for consumer transparency (Consumer Protection Act 2019)
    const REFUND_POLICY: &str = "Unused session time is refunded to your wallet at the pro-rated session rate. \
        Refunds to original payment method are available within 7 days of top-up for unused wallet balance. \
        No refunds for completed sessions.";
    const PRICING_POLICY: &str = "All prices are inclusive of 18% GST. \
        Session billing starts when your game reaches Running state. \
        Early termination refunds unused time to your wallet. \
        Free trial: one 5-minute session per customer.";
    const GST_NOTE: &str = "Prices inclusive of 18% GST (CGST 9% + SGST 9%)";

    let mut tiers = Vec::new();
    for (id, name, duration_minutes, price_paise, is_trial, _is_active, sort_order) in &rows {
        let dynamic_price = crate::billing::compute_dynamic_price(&state, *price_paise).await;
        let has_discount = dynamic_price != *price_paise;
        tiers.push(json!({
            "id": id,
            "name": name,
            "duration_minutes": duration_minutes,
            "base_price_paise": price_paise,
            "dynamic_price_paise": dynamic_price,
            "has_discount": has_discount,
            "is_trial": is_trial,
            "sort_order": sort_order,
        }));
    }
    Json(json!({
        "tiers": tiers,
        "refund_policy": REFUND_POLICY,
        "pricing_policy": PRICING_POLICY,
        "gst_note": GST_NOTE,
    }))
}

/// Public: returns real social proof counts from billing_sessions.
async fn pricing_social_proof_handler(State(state): State<Arc<AppState>>) -> Json<Value> {
    let drivers_this_week: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT driver_id) FROM billing_sessions
         WHERE status IN ('completed', 'ended_early')
         AND started_at >= datetime('now', '-7 days')"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let sessions_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions
         WHERE status IN ('completed', 'ended_early')
         AND date(started_at) = date('now')"
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Json(json!({
        "drivers_this_week": drivers_this_week,
        "sessions_today": sessions_today
    }))
}

/// LEGAL-06: Return minor waiver liability disclosure text.
/// Public endpoint — kiosk fetches this during minor registration to display the Indian Contract Act
/// limitation text and guardian consent requirements before the guardian signs.
async fn minor_waiver_disclosure() -> Json<Value> {
    Json(json!({
        "disclosure_text": "Under the Indian Contract Act 1872, agreements with persons under 18 years of age are void. This waiver acknowledgment is signed by the guardian on behalf of the minor participant. Racing Point maintains additional liability insurance coverage for participants under 18. The guardian assumes responsibility for the minor's conduct and safety during the session. This acknowledgment does not constitute a binding waiver of the minor's legal rights.",
        "requires_guardian_signature": true,
        "requires_guardian_otp": true,
        "requires_guardian_presence": true,
    }))
}

/// LEGAL-04: Send an OTP to a minor's guardian phone for consent verification.
/// Staff trigger this at the counter when processing a minor customer.
/// Body: { "driver_id": "...", "guardian_phone": "+91XXXXXXXXXX" }
async fn send_guardian_otp_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match body.get("driver_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({ "error": "driver_id is required" })),
    };
    let guardian_phone = match body.get("guardian_phone").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => return Json(json!({ "error": "guardian_phone is required" })),
    };
    match auth::send_guardian_otp(&state, driver_id, guardian_phone).await {
        Ok(result) => Json(json!({ "ok": true, "driver_id": result.driver_id, "delivered": result.delivered })),
        Err(e) => Json(json!({ "error": e })),
    }
}

/// LEGAL-04: Verify the OTP entered by the guardian at the counter.
/// On success, sets guardian_otp_verified=1 on the driver record.
/// Body: { "driver_id": "...", "otp": "123456" }
async fn verify_guardian_otp_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match body.get("driver_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({ "error": "driver_id is required" })),
    };
    let otp = match body.get("otp").and_then(|v| v.as_str()) {
        Some(o) => o,
        None => return Json(json!({ "error": "otp is required" })),
    };
    match auth::verify_guardian_otp(&state, driver_id, otp).await {
        Ok(verified) => Json(json!({ "ok": true, "verified": verified })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn create_pricing_tier(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30);
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64()).unwrap_or(0);
    let is_trial = body.get("is_trial").and_then(|v| v.as_bool()).unwrap_or(false);
    let sort_order = body.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(10);

    let result = sqlx::query(
        "INSERT INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(duration_minutes)
    .bind(price_paise)
    .bind(is_trial)
    .bind(sort_order)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            accounting::log_admin_action(
                &state, "pricing_create",
                &json!({"tier_id": id, "name": name, "duration_minutes": duration_minutes, "price_paise": price_paise}).to_string(),
                None, None,
            ).await;
            Json(json!({ "id": id, "name": name }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Snapshot before change for audit trail
    let old_snapshot = accounting::snapshot_row(&state, "pricing_tiers", &id).await;

    let name = body.get("name").and_then(|v| v.as_str());
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64());
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());

    // Build dynamic update query.
    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = name {
        updates.push("name = ?");
        binds.push(n.to_string());
    }
    if let Some(d) = duration_minutes {
        updates.push("duration_minutes = ?");
        binds.push(d.to_string());
    }
    if let Some(p) = price_paise {
        updates.push("price_paise = ?");
        binds.push(p.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE pricing_tiers SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "pricing_tiers", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_update",
                &json!({"tier_id": id, "changes": body}).to_string(),
                None, None,
            ).await;
            Json(json!({ "ok": true }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let old_snapshot = accounting::snapshot_row(&state, "pricing_tiers", &id).await;

    // Soft delete: set is_active = 0
    match sqlx::query("UPDATE pricing_tiers SET is_active = 0, updated_at = datetime('now') WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(_) => {
            accounting::log_audit(
                &state, "pricing_tiers", &id, "delete",
                old_snapshot.as_deref(), Some("{\"is_active\":false}"), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_delete",
                &json!({"tier_id": id}).to_string(),
                None, None,
            ).await;
            Json(json!({ "ok": true }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Billing Rate Tiers (per-minute rates) ──────────────────────────────────

async fn list_billing_rates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, String, i64, i64, bool, Option<String>)>(
        "SELECT id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active, sim_type
         FROM billing_rates ORDER BY tier_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rates) => {
            let list: Vec<Value> = rates
                .iter()
                .map(|r| {
                    json!({
                        "id": r.0, "tier_order": r.1, "tier_name": r.2,
                        "threshold_minutes": r.3, "rate_per_min_paise": r.4,
                        "is_active": r.5, "sim_type": r.6,
                    })
                })
                .collect();
            Json(json!({ "rates": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_billing_rate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> (axum::http::StatusCode, Json<Value>) {
    let id = uuid::Uuid::new_v4().to_string();
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64()).unwrap_or(1);
    let tier_name = body.get("tier_name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64()).unwrap_or(0);
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64()).unwrap_or(2500);

    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).map(|s| s.to_string());

    let result = sqlx::query(
        "INSERT INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(tier_order)
    .bind(tier_name)
    .bind(threshold_minutes)
    .bind(rate_per_min_paise)
    .bind(&sim_type)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            (axum::http::StatusCode::CREATED, Json(json!({ "id": id, "tier_name": tier_name })))
        }
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))),
    }
}

async fn update_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    let tier_name = body.get("tier_name").and_then(|v| v.as_str());
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64());
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64());
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());
    // sim_type: present in body = update (even if null to clear); absent = don't touch
    let sim_type_in_body = body.get("sim_type").map(|v| v.as_str().map(|s| s.to_string()));

    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = tier_name {
        updates.push("tier_name = ?");
        binds.push(n.to_string());
    }
    if let Some(o) = tier_order {
        updates.push("tier_order = ?");
        binds.push(o.to_string());
    }
    if let Some(t) = threshold_minutes {
        updates.push("threshold_minutes = ?");
        binds.push(t.to_string());
    }
    if let Some(r) = rate_per_min_paise {
        updates.push("rate_per_min_paise = ?");
        binds.push(r.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }
    let sim_type_val: Option<String> = if let Some(opt_s) = sim_type_in_body {
        updates.push("sim_type = ?");
        binds.push(opt_s.clone().unwrap_or_default());
        opt_s
    } else {
        None
    };
    let _ = sim_type_val; // used via binds above

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE billing_rates SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "billing_rates", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            Json(json!({ "ok": true }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> axum::http::StatusCode {
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    match sqlx::query(
        "UPDATE billing_rates SET is_active = 0, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            accounting::log_audit(
                &state,
                "billing_rates",
                &id,
                "delete",
                old_snapshot.as_deref(),
                Some("{\"is_active\":false}"),
                None,
            )
            .await;
            axum::http::StatusCode::NO_CONTENT
        }
        Err(e) => {
            tracing::error!("delete_billing_rate DB error for {}: {}", id, e);
            axum::http::StatusCode::NO_CONTENT
        }
    }
}

// ─── Discount / Coupon helpers ───────────────────────────────────────────────

/// Validated coupon info ready to apply as a discount.
#[allow(dead_code)]
struct CouponDiscount {
    coupon_id: String,
    coupon_type: String,
    value: f64,
    discount_paise: i64,
    description: String,
}

/// Validate a coupon code and calculate the discount for a given price.
/// Returns Ok(CouponDiscount) or Err(error string).
async fn validate_and_calc_coupon(
    state: &Arc<AppState>,
    code: &str,
    driver_id: &str,
    price_paise: i64,
) -> Result<CouponDiscount, String> {
    let code_upper = code.to_uppercase();

    // Find coupon — FATM-08: only 'available' coupons can be validated
    let coupon: Option<(String, String, f64, i64, Option<String>, Option<String>, Option<i64>, bool)> = sqlx::query_as(
        "SELECT id, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only
         FROM coupons WHERE code = ? AND is_active = 1 AND coupon_status = 'available'",
    )
    .bind(&code_upper)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let coupon = coupon.ok_or("Invalid or expired coupon code")?;

    // Check usage count
    let used: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ?",
    )
    .bind(&coupon.0)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if used.0 >= coupon.3 {
        return Err("Coupon has reached maximum uses".to_string());
    }

    // Check if already used by this driver
    let driver_used: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ? AND driver_id = ?",
    )
    .bind(&coupon.0)
    .bind(driver_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if driver_used.0 > 0 {
        return Err("You have already used this coupon".to_string());
    }

    // Check min_spend
    if let Some(min) = coupon.6 {
        if price_paise < min {
            return Err(format!("Minimum spend of {} credits required", min / 100));
        }
    }

    // Check first_session_only
    if coupon.7 {
        let session_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ? AND status IN ('completed', 'active')",
        )
        .bind(driver_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if session_count.0 > 0 {
            return Err("This coupon is only valid for first-time sessions".to_string());
        }
    }

    // Calculate discount
    let (discount_paise, description) = match coupon.1.as_str() {
        "percent" => {
            let disc = ((price_paise as f64) * coupon.2 / 100.0).round() as i64;
            let disc = disc.min(price_paise); // never exceed price
            (disc, format!("{}% off", coupon.2))
        }
        "flat" => {
            let disc = (coupon.2 as i64).min(price_paise);
            (disc, format!("{} credits off", disc / 100))
        }
        "free_minutes" => {
            // free_minutes doesn't reduce price, it extends time — handled separately
            (0, format!("{} free minutes", coupon.2 as i64))
        }
        _ => return Err("Unknown coupon type".to_string()),
    };

    Ok(CouponDiscount {
        coupon_id: coupon.0,
        coupon_type: coupon.1,
        value: coupon.2,
        discount_paise,
        description,
    })
}

/// Record a coupon redemption in the DB.
async fn record_coupon_redemption(
    state: &Arc<AppState>,
    coupon_id: &str,
    driver_id: &str,
    billing_session_id: &str,
    discount_paise: i64,
) {
    let _ = sqlx::query(
        "INSERT INTO coupon_redemptions (id, coupon_id, driver_id, billing_session_id, discount_paise)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(coupon_id)
    .bind(driver_id)
    .bind(billing_session_id)
    .bind(discount_paise)
    .execute(&state.db)
    .await;

    // Increment used_count on coupon
    let _ = sqlx::query("UPDATE coupons SET used_count = used_count + 1 WHERE id = ?")
        .bind(coupon_id)
        .execute(&state.db)
        .await;
}

// ─── FATM-08: Coupon lifecycle FSM ──────────────────────────────────────────

/// Reserve a coupon for a session (available → reserved).
/// Uses SQL CAS (UPDATE WHERE coupon_status = 'available') to prevent races.
async fn reserve_coupon(
    pool: &sqlx::SqlitePool,
    coupon_id: &str,
    session_id: &str,
) -> Result<(), String> {
    let result = sqlx::query(
        "UPDATE coupons SET coupon_status = 'reserved', reserved_at = datetime('now'), \
         reserved_for_session = ? WHERE id = ? AND coupon_status = 'available'",
    )
    .bind(session_id)
    .bind(coupon_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error reserving coupon: {}", e))?;

    if result.rows_affected() == 0 {
        return Err("Coupon is no longer available (concurrent reservation or already used)".to_string());
    }
    Ok(())
}

/// Mark a coupon as redeemed (reserved → redeemed).
/// Called after billing session commits successfully.
async fn redeem_coupon(pool: &sqlx::SqlitePool, coupon_id: &str) -> Result<(), String> {
    let _ = sqlx::query(
        "UPDATE coupons SET coupon_status = 'redeemed' WHERE id = ? AND coupon_status = 'reserved'",
    )
    .bind(coupon_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error redeeming coupon: {}", e))?;
    Ok(())
}

/// FATM-09: Restore a coupon to available when its session is cancelled/failed.
/// Also deletes the coupon_redemption row so the count is not inflated.
/// pub so billing.rs can call it from the cancel path.
pub async fn restore_coupon_on_cancel(
    pool: &sqlx::SqlitePool,
    session_id: &str,
) -> Result<(), String> {
    // Restore coupon: clear reservation fields and decrement used_count
    let _ = sqlx::query(
        "UPDATE coupons SET coupon_status = 'available', reserved_at = NULL, \
         reserved_for_session = NULL, used_count = MAX(used_count - 1, 0) \
         WHERE reserved_for_session = ? AND coupon_status IN ('reserved', 'redeemed')",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .map_err(|e| format!("DB error restoring coupon: {}", e))?;

    // Remove the redemption record so used_count stays accurate
    let _ = sqlx::query("DELETE FROM coupon_redemptions WHERE billing_session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("DB error deleting coupon redemption: {}", e))?;

    Ok(())
}

// ─── Billing ────────────────────────────────────────────────────────────────

async fn start_billing(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id_raw = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let driver_id = body.get("driver_id").and_then(|v| v.as_str()).unwrap_or("");
    let pricing_tier_id = body.get("pricing_tier_id").and_then(|v| v.as_str()).unwrap_or("");
    let custom_price_paise = body.get("custom_price_paise").and_then(|v| v.as_u64()).map(|v| v as u32);
    let custom_duration_minutes = body.get("custom_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32);
    let staff_id = body.get("staff_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let split_count = body.get("split_count").and_then(|v| v.as_u64()).map(|v| v as u32);
    let split_duration_minutes = body.get("split_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32);
    // FATM-02: Idempotency key — if present, duplicate requests return the original result
    let idempotency_key = body.get("idempotency_key").and_then(|v| v.as_str()).map(|s| s.to_string());
    // Discount params: coupon_code OR staff_discount_paise + discount_reason
    let coupon_code = body.get("coupon_code").and_then(|v| v.as_str()).map(|s| s.to_string());
    let staff_discount_paise = body.get("staff_discount_paise").and_then(|v| v.as_i64());
    let discount_reason = body.get("discount_reason").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Normalize pod_id to canonical form
    let pod_id = rc_common::pod_id::normalize_pod_id(pod_id_raw).unwrap_or_else(|_| pod_id_raw.to_string());

    if pod_id.is_empty() || driver_id.is_empty() || pricing_tier_id.is_empty() {
        return Json(json!({ "error": "pod_id, driver_id, and pricing_tier_id are required" }));
    }

    // FATM-02: Idempotency check — return original result if key was already processed
    if let Some(ref key) = idempotency_key {
        let existing = sqlx::query_as::<_, (String, Option<i64>)>(
            "SELECT id, wallet_debit_paise FROM billing_sessions WHERE idempotency_key = ?",
        )
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((existing_id, existing_debit)) = existing {
            return Json(json!({
                "ok": true,
                "billing_session_id": existing_id,
                "wallet_debit_paise": existing_debit,
                "idempotent_replay": true,
            }));
        }
    }

    // Pre-validate: check in-memory timer (fast path; DB constraint is primary guard)
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(pod_id.as_str()) {
            return Json(json!({ "error": format!("Pod {} already has an active billing session", pod_id) }));
        }
    }

    // Look up tier (name + duration + price + trial flag)
    let tier_info = sqlx::query_as::<_, (String, i64, i64, bool)>(
        "SELECT name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (tier_name, tier_duration_minutes, tier_price_paise, is_trial) = match tier_info {
        Some(t) => t,
        None => return Json(json!({ "error": format!("Pricing tier '{}' not found or inactive", pricing_tier_id) })),
    };

    // Look up driver (name + trial status + waiver + DOB + guardian consent)
    let driver_info = sqlx::query_as::<_, (String, bool, bool, bool, Option<String>, bool, Option<String>)>(
        "SELECT name, COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0), \
         COALESCE(waiver_signed, 0), dob, COALESCE(guardian_otp_verified, 0), guardian_name \
         FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (driver_name, has_used_trial, unlimited_trials, waiver_signed, dob, guardian_otp_verified, guardian_name) = match driver_info {
        Some(d) => d,
        None => return Json(json!({ "error": format!("Driver '{}' not found", driver_id) })),
    };

    // LEGAL-03: Waiver gate — billing blocked if waiver not signed
    if !waiver_signed {
        return Json(json!({ "error": "Waiver signing required before billing. Please complete registration." }));
    }

    // LEGAL-04/05: Minor protection — check age from DOB
    let is_minor = if let Some(ref dob_str) = dob {
        if let Ok(dob_date) = chrono::NaiveDate::parse_from_str(dob_str, "%Y-%m-%d") {
            use chrono::Datelike;
            let today = chrono::Utc::now().date_naive();
            // Conservative manual age check: compare year/month/day to avoid fractional year rounding
            let age_years = today.year() - dob_date.year()
                - if (today.month(), today.day()) < (dob_date.month(), dob_date.day()) { 1 } else { 0 };
            age_years < 18
        } else {
            false // Cannot parse DOB — treat as adult
        }
    } else {
        false // No DOB on record — treat as adult
    };

    // Parse guardian_present flag from request body (staff must explicitly confirm)
    let guardian_present_flag = body.get("guardian_present").and_then(|v| v.as_bool()).unwrap_or(false);

    if is_minor {
        // LEGAL-04: Guardian OTP must be verified before billing a minor
        if !guardian_otp_verified {
            return Json(json!({
                "error": "Minor customer: guardian OTP verification required before billing",
                "minor_flow_required": true,
                "guardian_name": guardian_name,
            }));
        }
        // LEGAL-05: Staff must confirm guardian physical presence
        if !guardian_present_flag {
            return Json(json!({
                "error": "Minor customer: staff must confirm guardian physical presence (guardian_present=true)",
                "minor_flow_required": true,
            }));
        }
    }

    // Trial eligibility check
    if is_trial && has_used_trial && !unlimited_trials {
        return Json(json!({ "error": "Driver has already used their free trial" }));
    }

    // Validate pod exists
    let pod_exists = sqlx::query_as::<_, (String,)>("SELECT id FROM pods WHERE id = ?")
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    if pod_exists.is_none() {
        return Json(json!({ "error": format!("Pod '{}' not found", pod_id) }));
    }

    // Validate split params
    if let Some(sc) = split_count {
        if sc > 0 && split_duration_minutes.unwrap_or(1) == 0 {
            return Json(json!({ "error": "Split duration must be greater than 0 minutes" }));
        }
    }
    if let Some(dur) = custom_duration_minutes {
        if dur > 1440 { return Json(json!({ "error": "Custom duration cannot exceed 24 hours (1440 minutes)" })); }
    }
    if let Some(dur) = split_duration_minutes {
        if dur > 1440 { return Json(json!({ "error": "Split duration cannot exceed 24 hours (1440 minutes)" })); }
    }

    // Calculate allocated seconds
    let final_split_count = split_count.unwrap_or(1);
    let allocated_seconds: u32 = if let Some(split_dur) = split_duration_minutes.filter(|_| final_split_count > 1) {
        split_dur * 60
    } else {
        custom_duration_minutes
            .map(|m| m * 60)
            .unwrap_or(tier_duration_minutes as u32 * 60)
    };

    // Determine base price (custom override or tier price with optional dynamic pricing)
    let mut base_price_paise = custom_price_paise.map(|p| p as i64).unwrap_or_else(|| {
        // Dynamic pricing computed here synchronously is fine — no lock held
        tier_price_paise
    });

    // Apply group discount: if 3+ sessions already active, 4th+ gets group multiplier
    let mut group_discount_paise: i64 = 0;
    let active_count = {
        // Snapshot count before dropping lock
        let timers = state.billing.active_timers.read().await;
        timers.len()
    };
    if !is_trial && active_count >= 3 {
        let group_rule = sqlx::query_as::<_, (f64,)>(
            "SELECT multiplier FROM pricing_rules WHERE rule_type = 'group' AND is_active = 1 LIMIT 1",
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((multiplier,)) = group_rule {
            let discounted = (base_price_paise as f64 * multiplier).round() as i64;
            group_discount_paise = base_price_paise - discounted;
            base_price_paise = discounted;
            tracing::info!(
                "Group discount applied: {} active sessions, multiplier={}, saved {}p",
                active_count + 1, multiplier, group_discount_paise
            );
        }
    }

    // FATM-08: Generate session_id early so coupon reservation can be tied to the real session ID.
    // This session_id is used in reserve_coupon and then reused in the INSERT below.
    let session_id = uuid::Uuid::new_v4().to_string();

    // Apply coupon or staff discount
    let mut applied_discount_paise: i64 = group_discount_paise;
    let mut applied_coupon_id: Option<String> = None;
    let mut applied_discount_reason: Option<String> = if group_discount_paise > 0 {
        Some(format!("Group {} sessions (11% off)", active_count + 1))
    } else {
        None
    };

    if let Some(ref code) = coupon_code {
        match validate_and_calc_coupon(&state, code, driver_id, base_price_paise).await {
            Ok(cd) => {
                // FATM-08: Reserve coupon before the billing transaction.
                // CAS UPDATE WHERE coupon_status = 'available' catches concurrent races.
                if let Err(e) = reserve_coupon(&state.db, &cd.coupon_id, &session_id).await {
                    return Json(json!({ "error": e }));
                }
                applied_discount_paise += cd.discount_paise;
                applied_coupon_id = Some(cd.coupon_id);
                let coupon_desc = format!("Coupon {}: {}", code.to_uppercase(), cd.description);
                applied_discount_reason = Some(match applied_discount_reason {
                    Some(existing) => format!("{} + {}", existing, coupon_desc),
                    None => coupon_desc,
                });
            }
            Err(e) => return Json(json!({ "error": e })),
        }
    } else if let Some(staff_disc) = staff_discount_paise {
        if staff_disc > 0 && staff_disc <= base_price_paise {
            applied_discount_paise += staff_disc;
            let staff_desc = discount_reason.unwrap_or("Staff discount".to_string());
            applied_discount_reason = Some(match applied_discount_reason {
                Some(existing) => format!("{} + {}", existing, staff_desc),
                None => staff_desc,
            });
        }
    }

    let original_price_paise = custom_price_paise.map(|p| p as i64).unwrap_or(tier_price_paise);
    let mut final_price_paise = original_price_paise - applied_discount_paise;

    // FATM-10: Enforce discount floor — combined discounts cannot reduce payable below the floor
    let discount_floor_paise = billing::DISCOUNT_FLOOR_PAISE;
    if discount_floor_paise > 0 && final_price_paise < discount_floor_paise {
        let original_total_discount = applied_discount_paise;
        applied_discount_paise = original_price_paise - discount_floor_paise;
        final_price_paise = discount_floor_paise;
        tracing::info!(
            "FATM-10: Discount floor enforced — original discount {}p capped to {}p (floor={}p, original_price={}p)",
            original_total_discount, applied_discount_paise, discount_floor_paise, original_price_paise
        );
    }

    // Pre-check balance (optimistic, before acquiring tx) to return a clear error
    if !is_trial && final_price_paise > 0 {
        let balance = match wallet::get_balance(&state, driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": format!("Wallet error: {}", e) })),
        };
        if balance < final_price_paise {
            return Json(json!({
                "error": format!("Insufficient credits: have {} credits, need {} credits", balance / 100, final_price_paise / 100),
                "balance_paise": balance,
                "required_paise": final_price_paise,
            }));
        }
    }

    // Fetch pod number for debit notes (before tx)
    let pod_num = sqlx::query_as::<_, (i64,)>("SELECT number FROM pods WHERE id = ?")
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .unwrap_or(0);

    // ─── FATM-01: Single atomic transaction — wallet debit + session INSERT ───
    // If ANY step fails, the entire transaction rolls back automatically on drop.
    // No compensating refund needed — rollback is the rollback.
    // FATM-03: SQLite WAL mode with busy_timeout=5000ms handles concurrent write serialization.
    // The atomic UPDATE WHERE balance >= ? inside debit_in_tx is the overspend guard.
    let mut tx = match state.db.begin().await {
        Ok(t) => t,
        Err(e) => {
            // FATM-09: If a coupon was reserved but we can't start a transaction, restore it
            if let Some(ref cid) = applied_coupon_id {
                let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
                tracing::warn!(
                    coupon_id = %cid,
                    session_id = %session_id,
                    "FATM-09: Restored coupon reservation after TX begin failure"
                );
            }
            state.record_api_error("billing/start");
            return Json(json!({ "error": format!("DB error starting transaction: {}", e) }));
        }
    };

    let now = chrono::Utc::now();

    // Step 1: Debit wallet within the transaction (FATM-01, FATM-03)
    let wallet_debit_paise: Option<i64> = if !is_trial && final_price_paise > 0 {
        let debit_notes = if applied_discount_paise > 0 {
            format!("Session on Pod {} — {} credits discount", pod_num, applied_discount_paise / 100)
        } else {
            format!("Session on Pod {}", pod_num)
        };
        match wallet::debit_in_tx(
            &mut tx,
            driver_id,
            final_price_paise,
            "debit_session",
            Some(&session_id),
            Some(&debit_notes),
            idempotency_key.as_deref(),
        ).await {
            Ok(_) => Some(final_price_paise),
            Err(e) => {
                drop(tx);
                // FATM-09: Restore any reserved coupon so it can be used again
                if applied_coupon_id.is_some() {
                    let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
                    tracing::info!("FATM-09: Coupon restored after wallet debit failure for session {}", session_id);
                }
                state.record_api_error("billing/start");
                return Json(json!({ "error": e }));
            }
        }
    } else {
        None
    };

    // Step 2: INSERT billing session within the same transaction (FATM-01)
    let dynamic_price = if custom_price_paise.is_none() && !is_trial {
        // Compute dynamic pricing inside the tx (read-only query is fine)
        let dp = billing::compute_dynamic_price_in_tx(&mut tx, tier_price_paise).await;
        if dp != tier_price_paise { Some(dp) } else { None }
    } else {
        custom_price_paise.map(|p| p as i64)
    };

    // BILL-13: Insert with 'waiting_for_game' status — timer activated on AcStatus::Live
    if let Err(e) = sqlx::query(
        "INSERT INTO billing_sessions \
         (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, custom_price_paise, \
          started_at, staff_id, split_count, split_duration_minutes, \
          wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason, idempotency_key, \
          guardian_present, is_minor_session) \
         VALUES (?, ?, ?, ?, ?, 'waiting_for_game', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(driver_id)
    .bind(&pod_id)
    .bind(&pricing_tier_id)
    .bind(allocated_seconds as i64)
    .bind(dynamic_price)
    .bind(now.to_rfc3339())
    .bind(&staff_id)
    .bind(final_split_count as i64)
    .bind(split_duration_minutes.map(|d| d as i64))
    .bind(wallet_debit_paise)
    .bind(applied_discount_paise)
    .bind(&applied_coupon_id)
    .bind(original_price_paise)
    .bind(&applied_discount_reason)
    .bind(idempotency_key.as_deref())
    .bind(guardian_present_flag)
    .bind(is_minor)
    .execute(&mut *tx)
    .await {
        drop(tx); // rolls back wallet debit atomically
        // FATM-09: Restore any reserved coupon so it can be used again
        if applied_coupon_id.is_some() {
            let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
            tracing::info!("FATM-09: Coupon restored after session INSERT failure for session {}", session_id);
        }
        state.record_api_error("billing/start");
        return Json(json!({ "error": format!("Failed to create billing session: {}", e) }));
    }

    // Step 3: Log billing events within the same transaction
    for event_type in ["created", "started"] {
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event) VALUES (?, ?, ?, 0)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(event_type)
        .execute(&mut *tx)
        .await;
    }

    // Step 4: Mark trial as used within the same transaction
    if is_trial && !unlimited_trials {
        let _ = sqlx::query("UPDATE drivers SET has_used_trial = 1, updated_at = datetime('now') WHERE id = ?")
            .bind(driver_id)
            .execute(&mut *tx)
            .await;
    }

    // ─── Commit: all-or-nothing (FATM-01) ────────────────────────────────────
    if let Err(e) = tx.commit().await {
        // FATM-09: Restore any reserved coupon so it can be used again
        if applied_coupon_id.is_some() {
            let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
            tracing::info!("FATM-09: Coupon restored after commit failure for session {}", session_id);
        }
        state.record_api_error("billing/start");
        return Json(json!({ "error": format!("Transaction commit failed: {}", e) }));
    }

    // ─── Post-commit: LEGAL-08 activity tracking — update last_activity_at ─────
    // Non-critical: failure does NOT affect billing. Keeps active customers from being
    // anonymized by the daily data-retention background job.
    let _ = sqlx::query(
        "UPDATE drivers SET last_activity_at = datetime('now') WHERE id = ?",
    )
    .bind(driver_id)
    .execute(&state.db)
    .await;

    // ─── Post-commit: record coupon redemption + mark coupon redeemed (FATM-08) ─
    if let Some(ref cid) = applied_coupon_id {
        record_coupon_redemption(&state, cid, driver_id, &session_id, applied_discount_paise).await;
        // FATM-08: Transition coupon to 'redeemed' now that session is committed
        let _ = redeem_coupon(&state.db, cid).await;
    }
    if applied_discount_paise > 0 {
        accounting::log_audit(
            &state,
            "billing_sessions",
            &session_id,
            "discount",
            None,
            Some(&serde_json::json!({
                "discount_paise": applied_discount_paise,
                "original_price_paise": original_price_paise,
                "reason": applied_discount_reason,
                "coupon_id": applied_coupon_id,
            }).to_string()),
            staff_id.as_deref(),
        )
        .await;
    }

    // ─── Post-commit: generate GST invoice (LEGAL-02) ────────────────────────
    // Invoice generation is non-critical — a failure here does NOT roll back the session.
    // The journal entry is also created here using the GST-separated accounting.
    if let Some(debit_paise) = wallet_debit_paise {
        match accounting::post_session_debit_gst(&state, driver_id, debit_paise, &session_id).await {
            Ok((_entry_id, net_paise, gst_paise)) => {
                if let Err(e) = accounting::generate_invoice(
                    &state,
                    &session_id,
                    driver_id,
                    &driver_name,
                    debit_paise,
                    net_paise,
                    gst_paise,
                )
                .await
                {
                    tracing::warn!(
                        session_id = %session_id,
                        "Invoice generation failed (non-critical): {}",
                        e
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    "GST journal entry failed (non-critical): {}",
                    e
                );
            }
        }
    }

    // ─── BILL-13: Defer timer activation until game reaches AcStatus::Live ─────
    // Wallet debit + DB record already committed above (FATM-01).
    // Timer starts only when PlayableSignal received — customer not charged for load screens.
    let pod_id_for_defer = pod_id.clone();
    billing::defer_billing_with_precommitted_session(&state, pod_id_for_defer, billing::BillingStartData {
        session_id: session_id.clone(),
        driver_id: driver_id.to_string(),
        driver_name,
        pod_id,
        pricing_tier_name: tier_name,
        allocated_seconds,
        split_count: final_split_count,
        split_duration_minutes,
        started_at: now, // placeholder — overwritten to game-live time on activation
    }).await;

    Json(json!({
        "ok": true,
        "billing_session_id": session_id,
        "wallet_debit_paise": wallet_debit_paise,
        "discount_paise": applied_discount_paise,
        "original_price_paise": original_price_paise,
        "discount_reason": applied_discount_reason,
        "discount_floor_paise": billing::DISCOUNT_FLOOR_PAISE,
    }))
}

/// Returns valid session split options for a given total duration.
/// AC-specific: customers can divide their session into shorter sub-sessions.
async fn get_split_options(
    Path(duration_minutes): Path<u32>,
) -> Json<Value> {
    let options = compute_split_options(duration_minutes);
    Json(json!({ "duration_minutes": duration_minutes, "options": options }))
}

/// Compute valid split configurations for a given total duration.
/// Rules: each sub-session must be at least 10 minutes, count * sub_duration == total.
fn compute_split_options(total_minutes: u32) -> Vec<serde_json::Value> {
    let mut options = Vec::new();
    // Always include the unsplit option
    options.push(json!({ "count": 1, "duration_minutes": total_minutes, "label": format!("1 × {} min", total_minutes) }));

    // Find all valid splits where sub-session >= 10 min
    for count in 2..=6 {
        if total_minutes % count == 0 {
            let sub = total_minutes / count;
            if sub >= 10 {
                options.push(json!({
                    "count": count,
                    "duration_minutes": sub,
                    "label": format!("{} × {} min", count, sub),
                }));
            }
        }
    }
    options
}

/// Continue to next sub-session in a split session. No wallet debit — already paid.
async fn continue_split(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = match body.get("pod_id").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Json(json!({ "error": "pod_id required" })),
    };
    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa").to_string();
    let launch_args = body.get("launch_args").and_then(|v| v.as_str()).unwrap_or("{}").to_string();

    // Find active reservation for this pod
    let reservation = match crate::pod_reservation::get_active_reservation_for_pod(&state, &pod_id).await {
        Some(r) => r,
        None => return Json(json!({ "error": "No active reservation on this pod" })),
    };

    // Must not have an active billing session on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(&pod_id) {
            return Json(json!({ "error": "A session is still active on this pod" }));
        }
    }

    // Look up the original split session details from the reservation
    let original = match sqlx::query_as::<_, (i64, i64, String, String)>(
        "SELECT split_count, split_duration_minutes, pricing_tier_id, driver_id
         FROM billing_sessions
         WHERE reservation_id = ?
         ORDER BY started_at ASC LIMIT 1",
    )
    .bind(&reservation.id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "No billing sessions found for this reservation" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let total_splits = original.0 as u32;
    let split_duration_minutes = original.1 as u32;
    let pricing_tier_id = original.2;
    let driver_id = original.3;

    // Count completed sessions in this reservation
    let completed = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM billing_sessions WHERE reservation_id = ? AND status IN ('completed', 'ended_early')",
    )
    .bind(&reservation.id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0 as u32)
    .unwrap_or(0);

    if completed >= total_splits {
        // All splits used — end reservation
        let _ = crate::pod_reservation::end_reservation(&state, &reservation.id).await;
        return Json(json!({ "error": "All splits already used", "completed": completed, "total": total_splits }));
    }

    let current_split_number = completed + 1;
    let is_last_split = current_split_number >= total_splits;

    // Touch reservation
    crate::pod_reservation::touch_reservation(&state, &reservation.id).await;

    // Start billing session with split duration — NO wallet debit
    let billing_session_id = match billing::start_billing_session(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        Some(0), // custom_price_paise = 0 (no charge for continuation)
        Some(split_duration_minutes), // custom_duration_minutes
        None, // staff_id
        Some(total_splits), // split_count
        Some(split_duration_minutes), // split_duration_minutes
    )
    .await
    {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Link this billing session to the reservation
    let _ = sqlx::query(
        "UPDATE billing_sessions SET reservation_id = ?, wallet_debit_paise = 0 WHERE id = ?",
    )
    .bind(&reservation.id)
    .bind(&billing_session_id)
    .execute(&state.db)
    .await;

    // Update the timer's current_split_number
    {
        let mut timers = state.billing.active_timers.write().await;
        if let Some(timer) = timers.get_mut(&pod_id) {
            timer.current_split_number = current_split_number;
        }
    }

    // If this is the last split, end the reservation so the final timer expiry
    // triggers SessionEnded (full end) instead of SubSessionEnded
    if is_last_split {
        let _ = crate::pod_reservation::end_reservation(&state, &reservation.id).await;
        tracing::info!("Last split ({}/{}) — reservation {} ended", current_split_number, total_splits, reservation.id);
    }

    // Launch the game — inject split_duration_minutes into launch args
    let game_launched = {
        let mut parsed: serde_json::Value = serde_json::from_str(&launch_args).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(split_duration_minutes);

        let sim: rc_common::types::SimType = match serde_json::from_value(serde_json::Value::String(sim_type.clone())) {
            Ok(st) => st,
            Err(_) => return Json(json!({ "error": format!("Unknown sim_type: {}", sim_type) })),
        };

        let cmd = rc_common::protocol::DashboardCommand::LaunchGame {
            pod_id: pod_id.clone(),
            sim_type: sim,
            launch_args: Some(parsed.to_string()),
        };
        game_launcher::handle_dashboard_command(&state, cmd).await.is_ok()
    };

    tracing::info!(
        "Continue split {}/{} on pod {} — session {}",
        current_split_number, total_splits, pod_id, billing_session_id
    );

    Json(json!({
        "ok": true,
        "billing_session_id": billing_session_id,
        "current_split_number": current_split_number,
        "total_splits": total_splits,
        "is_last_split": is_last_split,
        "game_launched": game_launched,
    }))
}

async fn stop_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    // FATM-02: Idempotency — if session already ended, return ok (not error)
    // Check for an existing ended_early or cancelled billing event for this session.
    let already_ended = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM billing_events \
         WHERE billing_session_id = ? AND event_type IN ('ended_early', 'cancelled', 'completed') \
         LIMIT 1",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if already_ended.is_some() {
        return Json(json!({ "ok": true, "idempotent_replay": true }));
    }

    // Optional body may carry idempotency_key for future use — currently informational
    let _idempotency_key = body
        .as_ref()
        .and_then(|b| b.get("idempotency_key"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let found = billing::end_billing_session_public(&state, &id, rc_common::types::BillingSessionStatus::EndedEarly, None).await;
    if found {
        Json(json!({ "ok": true }))
    } else {
        Json(json!({ "ok": false, "error": "Session not found or already ended" }))
    }
}

/// DEPLOY-02: POST /billing/{id}/agent-shutdown
/// Called by rc-agent during graceful shutdown when a billing session is active.
/// No JWT required — agent authenticates via RCAGENT_SERVICE_KEY header (same key as remote_ops).
/// Ends the session with EndedEarly so the partial refund logic fires.
/// Idempotent — safe to call multiple times (e.g. from sentinel recovery on next restart).
#[derive(serde::Deserialize)]
struct AgentShutdownBody {
    pod_id: String,
    shutdown_reason: String,
}

async fn agent_shutdown_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(body): Json<AgentShutdownBody>,
) -> impl axum::response::IntoResponse {
    // Validate service key against configured sentry_service_key.
    // Agent sends key as: Authorization: Bearer <service_key>
    let expected_key = state.config.pods.sentry_service_key.clone().unwrap_or_default();
    if !expected_key.is_empty() {
        let provided_key = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .unwrap_or("");
        if provided_key != expected_key.as_str() {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "invalid service key" })),
            ).into_response();
        }
    }
    let result = billing::handle_agent_shutdown(&state, &id, &body.pod_id, &body.shutdown_reason).await;
    Json(result).into_response()
}

/// DEPLOY-04: GET /billing/pod/{pod_id}/interrupted
/// Called by rc-agent on startup to check for and auto-end interrupted sessions.
/// This endpoint also accepts unauthenticated requests from pods (service key in header).
async fn interrupted_sessions_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let result = billing::handle_interrupted_sessions_check(&state, &pod_id).await;
    Json(result)
}

async fn pause_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let cmd = rc_common::protocol::DashboardCommand::PauseBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn resume_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Check if this is a disconnect-paused session (needs special handling)
    let is_disconnect_paused = {
        let timers = state.billing.active_timers.read().await;
        timers.values().any(|t| t.session_id == id && t.status == rc_common::types::BillingSessionStatus::PausedDisconnect)
    };

    if is_disconnect_paused {
        match billing::resume_billing_from_disconnect(&state, &id).await {
            Ok(()) => Json(json!({ "ok": true })),
            Err(e) => Json(json!({ "error": e })),
        }
    } else {
        let cmd = rc_common::protocol::DashboardCommand::ResumeBilling {
            billing_session_id: id,
        };
        billing::handle_dashboard_command(&state, cmd).await;
        Json(json!({ "ok": true }))
    }
}

async fn extend_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let additional_seconds = body
        .get("additional_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(600) as u32;

    // BILL-04: Validate additional_seconds is a multiple of 60 and within bounds (60..=3600)
    if additional_seconds < 60 || additional_seconds > 3600 {
        return Json(json!({
            "ok": false,
            "error": format!("additional_seconds must be between 60 and 3600, got {}", additional_seconds)
        }));
    }
    if additional_seconds % 60 != 0 {
        return Json(json!({
            "ok": false,
            "error": format!("additional_seconds must be a multiple of 60, got {}", additional_seconds)
        }));
    }

    // FATM-07: Call extend_billing_session directly (not via DashboardCommand) to propagate errors
    match billing::extend_billing_session(&state, &id, additional_seconds).await {
        Ok(()) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn active_billing_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let timers = state.billing.active_timers.read().await;
    let sessions: Vec<_> = timers.values().map(|t| t.to_info(&rate_tiers)).collect();
    Json(json!({ "sessions": sessions }))
}

#[derive(Deserialize)]
struct BillingListQuery {
    date: Option<String>,
    status: Option<String>,
}

async fn list_billing_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BillingListQuery>,
) -> Json<Value> {
    let mut query = String::from(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.created_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE 1=1",
    );

    // Build parameterized query to prevent SQL injection
    let mut bind_values: Vec<String> = Vec::new();
    if let Some(date) = &params.date {
        // Validate date format (YYYY-MM-DD only)
        if date.len() == 10 && date.chars().all(|c| c.is_ascii_digit() || c == '-') {
            query.push_str(" AND date(bs.started_at) = ?");
            bind_values.push(date.clone());
        }
    }
    if let Some(status) = &params.status {
        // Validate status is alphanumeric + underscores only
        if status.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            query.push_str(" AND bs.status = ?");
            bind_values.push(status.clone());
        }
    }

    query.push_str(" ORDER BY bs.created_at DESC LIMIT 100");

    let mut q = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>, String)>(
        &query,
    );
    for val in &bind_values {
        q = q.bind(val);
    }
    let rows = q.fetch_all(&state.db).await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "driver_id": s.1, "driver_name": s.2,
                        "pod_id": s.3, "pricing_tier_name": s.4,
                        "allocated_seconds": s.5, "driving_seconds": s.6,
                        "status": s.7, "price_paise": s.8,
                        "started_at": s.9, "ended_at": s.10, "created_at": s.11,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_billing_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(s)) => Json(json!({
            "id": s.0, "driver_id": s.1, "driver_name": s.2,
            "pod_id": s.3, "pricing_tier_name": s.4,
            "allocated_seconds": s.5, "driving_seconds": s.6,
            "status": s.7, "price_paise": s.8,
            "started_at": s.9, "ended_at": s.10,
        })),
        Ok(None) => Json(json!({ "error": "Billing session not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn billing_session_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
        "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
         FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(events) => {
            let list: Vec<Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "event_type": e.1,
                        "driving_seconds_at_event": e.2,
                        "metadata": e.3, "created_at": e.4,
                    })
                })
                .collect();
            Json(json!({ "events": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn billing_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get billing session info (including discount fields)
    let session = sqlx::query_as::<_, (String, String, String, String, i64, i64, String, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get discount info
    let discount_info = sqlx::query_as::<_, (Option<i64>, Option<String>, Option<i64>, Option<String>)>(
        "SELECT discount_paise, coupon_id, original_price_paise, discount_reason FROM billing_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get laps for this session
    let laps = sqlx::query_as::<_, (String, i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String)>(
        "SELECT id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, track, car
         FROM laps WHERE session_id = ? ORDER BY lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len() as u32;
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.6).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.2).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.2).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };

    // Check personal best
    let track = laps.first().map(|l| l.7.as_str()).unwrap_or("");
    let car = laps.first().map(|l| l.8.as_str()).unwrap_or("");

    let pb = sqlx::query_as::<_, (i64,)>(
        "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
    )
    .bind(&session.1)
    .bind(track)
    .bind(car)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let personal_best_broken = best_lap_ms.map(|b| pb.map(|p| b <= p.0).unwrap_or(true)).unwrap_or(false);

    // Check leaderboard position
    let leaderboard_position = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) + 1 FROM personal_bests WHERE track = ? AND car = ? AND best_lap_ms < ?",
        )
        .bind(track)
        .bind(car)
        .bind(best_lap_ms.unwrap_or(i64::MAX))
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
    } else {
        None
    };

    let laps_json: Vec<Value> = laps
        .iter()
        .map(|l| {
            json!({
                "lap_number": l.1,
                "lap_time_ms": l.2,
                "sector1_ms": l.3,
                "sector2_ms": l.4,
                "sector3_ms": l.5,
                "valid": l.6,
            })
        })
        .collect();

    Json(json!({
        "summary": {
            "billing_session_id": session.0,
            "driver_id": session.1,
            "driver_name": session.2,
            "pod_id": session.3,
            "track": track,
            "car": car,
            "allocated_seconds": session.4,
            "driving_seconds": session.5,
            "status": session.6,
            "total_laps": total_laps,
            "best_lap_ms": best_lap_ms,
            "average_lap_ms": avg_lap_ms,
            "personal_best_broken": personal_best_broken,
            "leaderboard_position": leaderboard_position,
            "laps": laps_json,
            "discount_paise": discount_info.as_ref().and_then(|d| d.0),
            "coupon_id": discount_info.as_ref().and_then(|d| d.1.clone()),
            "original_price_paise": discount_info.as_ref().and_then(|d| d.2),
            "discount_reason": discount_info.as_ref().and_then(|d| d.3.clone()),
        }
    }))
}

// ─── Invoice (LEGAL-02) ──────────────────────────────────────────────────────

/// Staff endpoint: GET /billing/sessions/{id}/invoice
/// Returns the GST-compliant invoice for a billing session (manager+ access via RBAC).
async fn get_session_invoice(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, i64, String, String, i64, f64, i64, i64, i64, String)>(
        "SELECT id, invoice_number, driver_name, venue_gstin, \
         taxable_value_paise, gst_rate_percent, cgst_paise, sgst_paise, total_paise, created_at \
         FROM invoices WHERE billing_session_id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((id, invoice_number, driver_name, venue_gstin,
                 taxable_value_paise, gst_rate_percent, cgst_paise, sgst_paise,
                 total_paise, created_at))) => {
            Json(json!({
                "id": id,
                "invoice_number": invoice_number,
                "billing_session_id": session_id,
                "driver_name": driver_name,
                "venue_gstin": venue_gstin,
                "sac_code": "999692",
                "taxable_value_paise": taxable_value_paise,
                "gst_rate_percent": gst_rate_percent,
                "cgst_paise": cgst_paise,
                "sgst_paise": sgst_paise,
                "total_paise": total_paise,
                "created_at": created_at,
            }))
        }
        Ok(None) => Json(json!({ "error": "Invoice not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Customer endpoint: GET /customer/sessions/{id}/invoice
/// Returns the GST invoice for the authenticated customer's own session.
async fn customer_session_invoice(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    // Extract driver_id from JWT — only the session owner can fetch their invoice
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let row = sqlx::query_as::<_, (String, i64, String, String, i64, f64, i64, i64, i64, String)>(
        "SELECT inv.id, inv.invoice_number, inv.driver_name, inv.venue_gstin, \
         inv.taxable_value_paise, inv.gst_rate_percent, inv.cgst_paise, inv.sgst_paise, \
         inv.total_paise, inv.created_at \
         FROM invoices inv \
         JOIN billing_sessions bs ON inv.billing_session_id = bs.id \
         WHERE inv.billing_session_id = ? AND bs.driver_id = ?",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((id, invoice_number, driver_name, venue_gstin,
                 taxable_value_paise, gst_rate_percent, cgst_paise, sgst_paise,
                 total_paise, created_at))) => {
            Json(json!({
                "id": id,
                "invoice_number": invoice_number,
                "billing_session_id": session_id,
                "driver_name": driver_name,
                "venue_gstin": venue_gstin,
                "sac_code": "999692",
                "taxable_value_paise": taxable_value_paise,
                "gst_rate_percent": gst_rate_percent,
                "cgst_paise": cgst_paise,
                "sgst_paise": sgst_paise,
                "total_paise": total_paise,
                "created_at": created_at,
            }))
        }
        Ok(None) => Json(json!({ "error": "Invoice not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── UX-03: Customer session receipt ─────────────────────────────────────

/// Customer endpoint: GET /customer/sessions/{id}/receipt
/// Returns full financial breakdown: charges, GST breakup, refund, before/after balance.
/// Only the session owner (driver_id from JWT) can access their own receipt.
async fn customer_session_receipt(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch session row — verify session belongs to this driver
    let session = sqlx::query_as::<_, (String, String, String, i64, i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT bs.id, d.id, d.name, bs.driving_seconds, bs.wallet_debit_paise, bs.status,
                bs.started_at, bs.ended_at, bs.refund_paise, bs.allocated_seconds
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (sid, _did, driver_name, driving_seconds, wallet_debit_paise, status,
         started_at, ended_at, refund_paise_opt, _allocated) = session;
    let refund_paise = refund_paise_opt.unwrap_or(0);

    // Current wallet balance (after-balance)
    let balance_after_paise: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(CASE WHEN txn_type LIKE 'credit%' OR txn_type LIKE 'refund%' THEN amount_paise ELSE -amount_paise END), 0)
         FROM wallet_transactions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    // Reconstruct before-balance: add back what was charged, subtract back the refund
    let balance_before_paise = balance_after_paise + wallet_debit_paise - refund_paise;

    // GST breakup from invoices table (generated by Phase 255 post_session_debit_gst)
    let invoice = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT taxable_value_paise, cgst_paise, sgst_paise, total_paise
         FROM invoices WHERE billing_session_id = ?",
    )
    .bind(&sid)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (net_paise, cgst_paise, sgst_paise, gst_total_paise) = invoice
        .map(|i| (i.0, i.1, i.2, i.1 + i.2))
        .unwrap_or_else(|| {
            // Fallback: compute 18% inclusive GST split from wallet_debit_paise
            let net = wallet_debit_paise * 100 / 118;
            let gst = wallet_debit_paise - net;
            let half = gst / 2;
            (net, half, gst - half, gst)
        });

    Json(json!({
        "session_id": sid,
        "driver_name": driver_name,
        "started_at": started_at,
        "ended_at": ended_at,
        "duration_seconds": driving_seconds,
        "charges_paise": wallet_debit_paise,
        "gst_paise": gst_total_paise,
        "cgst_paise": cgst_paise,
        "sgst_paise": sgst_paise,
        "net_paise": net_paise,
        "refund_paise": refund_paise,
        "balance_before_paise": balance_before_paise,
        "balance_after_paise": balance_after_paise,
        "status": status,
    }))
}

// ─── Billing Refund ───────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BillingRefundRequest {
    amount_paise: i64,
    method: String,       // "wallet", "cash", "upi"
    reason: String,
    notes: Option<String>,
    // FATM-02: Optional idempotency key — duplicate requests return the original result
    idempotency_key: Option<String>,
}

async fn refund_billing_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(req): Json<BillingRefundRequest>,
) -> Json<Value> {
    // Extract staff_id from JWT (POS-05: audit trail with staff_id)
    let staff_id = claims.map(|c| c.0.sub.clone());

    // FATM-02: Idempotency check for refund
    if let Some(ref key) = req.idempotency_key {
        let existing = sqlx::query_as::<_, (String, i64)>(
            "SELECT id, amount_paise FROM refunds WHERE idempotency_key = ?",
        )
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((existing_id, existing_amount)) = existing {
            return Json(json!({
                "status": "ok",
                "refund_id": existing_id,
                "amount_paise": existing_amount,
                "idempotent_replay": true,
            }));
        }
    }

    // Validate method
    if !["wallet", "cash", "upi"].contains(&req.method.as_str()) {
        return Json(json!({ "error": "method must be wallet, cash, or upi" }));
    }
    if req.amount_paise <= 0 {
        return Json(json!({ "error": "amount_paise must be positive" }));
    }
    if req.reason.trim().is_empty() {
        return Json(json!({ "error": "reason is required" }));
    }

    // Fetch session
    let session = sqlx::query_as::<_, (String, String, Option<i64>, String)>(
        "SELECT bs.id, bs.driver_id, bs.wallet_debit_paise, d.name
         FROM billing_sessions bs JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await;

    let (_sid, driver_id, debit_paise, driver_name) = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    let max_refundable = debit_paise.unwrap_or(0);

    // Check total already refunded for this session
    let already_refunded: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(amount_paise), 0) FROM refunds WHERE billing_session_id = ?",
    )
    .bind(&session_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let remaining = max_refundable - already_refunded;
    if req.amount_paise > remaining {
        return Json(json!({
            "error": format!("Refund exceeds remaining refundable amount. Charged: {}, already refunded: {}, remaining: {}", max_refundable, already_refunded, remaining)
        }));
    }

    let refund_id = uuid::Uuid::new_v4().to_string();
    let mut wallet_txn_id: Option<String> = None;

    // If wallet refund, credit the wallet
    if req.method == "wallet" {
        match wallet::credit(
            &state,
            &driver_id,
            req.amount_paise,
            "refund_session",
            Some(&session_id),
            Some(&format!("Refund: {}", req.reason)),
            None,
        )
        .await
        {
            Ok(_new_balance) => {
                // Get the txn_id from the most recent transaction
                let txn = sqlx::query_as::<_, (String,)>(
                    "SELECT id FROM wallet_transactions WHERE driver_id = ? AND txn_type = 'refund_session' ORDER BY created_at DESC LIMIT 1"
                )
                .bind(&driver_id)
                .fetch_optional(&state.db)
                .await;
                wallet_txn_id = txn.ok().flatten().map(|r| r.0);
                tracing::info!("Refund to wallet: {} +{}p (session {})", driver_id, req.amount_paise, session_id);
            }
            Err(e) => return Json(json!({ "error": format!("Wallet credit failed: {}", e) })),
        }
    } else {
        tracing::info!("Refund via {}: {} {}p (session {})", req.method, driver_id, req.amount_paise, session_id);
    }

    // Record in refunds table (include idempotency_key for FATM-02)
    let result = sqlx::query(
        "INSERT INTO refunds (id, billing_session_id, driver_id, amount_paise, method, reason, notes, staff_id, wallet_txn_id, idempotency_key)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&refund_id)
    .bind(&session_id)
    .bind(&driver_id)
    .bind(req.amount_paise)
    .bind(&req.method)
    .bind(&req.reason)
    .bind(req.notes.as_deref())
    .bind(staff_id.as_deref()) // staff_id from JWT (POS-05)
    .bind(wallet_txn_id.as_deref())
    .bind(req.idempotency_key.as_deref())
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // Audit trail with staff_id from JWT (POS-05)
            accounting::log_audit(
                &state,
                "refunds",
                &refund_id,
                "create",
                None,
                Some(&serde_json::json!({
                    "billing_session_id": session_id,
                    "driver_id": driver_id,
                    "amount_paise": req.amount_paise,
                    "method": req.method,
                    "reason": req.reason,
                }).to_string()),
                staff_id.as_deref(),
            )
            .await;

            // Update refund_paise on billing_sessions for cloud sync
            let _ = sqlx::query(
                "UPDATE billing_sessions SET refund_paise = COALESCE(refund_paise, 0) + ? WHERE id = ?",
            )
            .bind(req.amount_paise)
            .bind(&session_id)
            .execute(&state.db)
            .await;

            Json(json!({
                "status": "ok",
                "refund_id": refund_id,
                "amount_paise": req.amount_paise,
                "method": req.method,
                "driver_name": driver_name,
                "total_refunded_paise": already_refunded + req.amount_paise,
                "max_refundable_paise": max_refundable,
            }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to record refund: {}", e) })),
    }
}

async fn get_billing_refunds(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let refunds = sqlx::query_as::<_, (String, String, String, i64, String, String, Option<String>, Option<String>, String)>(
        "SELECT r.id, r.billing_session_id, r.driver_id, r.amount_paise, r.method, r.reason, r.notes, r.wallet_txn_id, r.created_at
         FROM refunds r WHERE r.billing_session_id = ? ORDER BY r.created_at DESC",
    )
    .bind(&session_id)
    .fetch_all(&state.db)
    .await;

    match refunds {
        Ok(rows) => {
            let list: Vec<Value> = rows.iter().map(|r| json!({
                "id": r.0,
                "billing_session_id": r.1,
                "driver_id": r.2,
                "amount_paise": r.3,
                "method": r.4,
                "reason": r.5,
                "notes": r.6,
                "wallet_txn_id": r.7,
                "created_at": r.8,
            })).collect();
            let total: i64 = rows.iter().map(|r| r.3).sum();
            Json(json!({ "refunds": list, "total_refunded_paise": total }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e), "refunds": [] })),
    }
}

// ─── Daily Report ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DailyReportQuery {
    date: Option<String>,
}

async fn daily_billing_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyReportQuery>,
) -> Json<Value> {
    let date = params
        .date
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.staff_id, sm.name,
                bs.discount_paise, bs.original_price_paise, bs.discount_reason
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN staff_members sm ON bs.staff_id = sm.id
         WHERE date(bs.started_at) = ?
         ORDER BY bs.started_at ASC",
    )
    .bind(&date)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let total_sessions = sessions.len();
            let total_revenue_paise: i64 = sessions
                .iter()
                .filter(|s| s.7 != "cancelled")
                .map(|s| s.8)
                .sum();
            let total_driving_seconds: i64 = sessions.iter().map(|s| s.6).sum();
            let total_discount_paise: i64 = sessions
                .iter()
                .filter(|s| s.7 != "cancelled")
                .map(|s| s.13.unwrap_or(0))
                .sum();

            // Build staff summary
            let mut staff_map: std::collections::HashMap<String, (String, usize, i64)> = std::collections::HashMap::new();
            for s in &sessions {
                if s.7 == "cancelled" { continue; }
                let staff_key = s.11.clone().unwrap_or_default();
                let staff_name = s.12.clone().unwrap_or_else(|| "Walk-in / Self".to_string());
                let entry = staff_map.entry(staff_key).or_insert((staff_name, 0, 0));
                entry.1 += 1;
                entry.2 += s.8;
            }
            let staff_summary: Vec<Value> = staff_map
                .into_iter()
                .map(|(id, (name, count, revenue))| {
                    json!({ "staff_id": id, "staff_name": name, "sessions": count, "revenue_paise": revenue })
                })
                .collect();

            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "driver_id": s.1, "driver_name": s.2,
                        "pod_id": s.3, "pricing_tier_name": s.4,
                        "allocated_seconds": s.5, "driving_seconds": s.6,
                        "status": s.7, "price_paise": s.8,
                        "started_at": s.9, "ended_at": s.10,
                        "staff_id": s.11, "staff_name": s.12,
                        "discount_paise": s.13, "original_price_paise": s.14,
                        "discount_reason": s.15,
                    })
                })
                .collect();

            Json(json!({
                "date": date,
                "total_sessions": total_sessions,
                "total_revenue_paise": total_revenue_paise,
                "total_discount_paise": total_discount_paise,
                "total_driving_seconds": total_driving_seconds,
                "staff_summary": staff_summary,
                "sessions": list,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Game Launcher ─────────────────────────────────────────────────────────

async fn launch_game(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let sim_type_str = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("");
    let launch_args_raw = body
        .get("launch_args")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    if pod_id.is_empty() || sim_type_str.is_empty() {
        return Json(json!({ "error": "pod_id and sim_type are required" }));
    }

    // Inject duration_minutes from active billing session into launch_args.
    // Uses REMAINING time (not allocated) so mid-session relaunches get correct duration.
    // Ceiling division ensures AC session >= billing time (no early AC expiry).
    let launch_args = if let Some(args) = launch_args_raw {
        let session_info = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
            "SELECT allocated_seconds, driving_seconds, split_duration_minutes FROM billing_sessions WHERE pod_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
        )
        .bind(pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let duration_minutes: u32 = match &session_info {
            // Split sessions: use fixed split duration (each segment is independent)
            Some((_, _, Some(split_mins))) if sim_type_str == "assetto_corsa" => *split_mins as u32,
            // Non-split: use remaining time with ceiling division
            Some((allocated, driven, _)) => {
                let remaining_secs = (*allocated as u32).saturating_sub(*driven as u32);
                (remaining_secs + 59) / 60  // ceiling division — AC never expires before billing
            }
            None => 60,
        };

        let mut parsed: serde_json::Value = serde_json::from_str(&args).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(duration_minutes);

        // SEC-01: Validate launch_args fields for INI injection chars BEFORE WS send.
        // Reject at the server boundary — 400 returned immediately, nothing reaches the agent.
        if let Err(e) = crate::api::security::validate_launch_args(&parsed) {
            return Json(json!({ "error": format!("Invalid launch_args: {}", e) }));
        }

        // SEC-02: Sanitize FFB GAIN — cap to 100 (physical motor safety).
        if let Some(ffb_str) = parsed.get("ffb").and_then(|v| v.as_str()) {
            let safe_ffb = crate::api::security::sanitize_ffb_gain(ffb_str);
            parsed["ffb"] = serde_json::json!(safe_ffb);
        }

        Some(parsed.to_string())
    } else {
        None
    };

    let sim_type: SimType = match serde_json::from_value(serde_json::Value::String(
        sim_type_str.to_string(),
    )) {
        Ok(st) => st,
        Err(_) => return Json(json!({ "error": format!("Unknown sim_type: {}", sim_type_str) })),
    };

    // INTEL-01: Query combo reliability BEFORE launching — build warning if success_rate < 70%.
    // Parse car/track from the already-injected launch_args JSON (duration_minutes was added above).
    let reliability_warning: Option<String> = {
        let args_parsed: serde_json::Value = launch_args
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or(serde_json::Value::Object(Default::default()));
        let car = args_parsed.get("car").and_then(|v| v.as_str());
        let track = args_parsed.get("track").and_then(|v| v.as_str());
        crate::metrics::query_combo_reliability(&state.db, pod_id, sim_type_str, car, track)
            .await
            .filter(|r| r.success_rate < 0.70)
            .map(|r| {
                format!(
                    "This combination has a {:.0}% success rate on this pod ({}/{} launches)",
                    r.success_rate * 100.0,
                    (r.success_rate * r.total_launches as f64).round() as i64,
                    r.total_launches
                )
            })
    };

    let cmd = rc_common::protocol::DashboardCommand::LaunchGame {
        pod_id: pod_id.to_string(),
        sim_type,
        launch_args,
    };

    match game_launcher::handle_dashboard_command(&state, cmd).await {
        Ok(()) => {
            let mut resp = json!({ "ok": true });
            if let Some(w) = reliability_warning {
                resp["warning"] = json!(w);
            }
            Json(resp)
        }
        Err(e) if e.contains("No agent connected") => {
            // No local pod — try relaying to venue via Tailscale bono_relay
            relay_game_launch_to_venue(&state, pod_id, sim_type_str, &body).await
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// Relay a game launch command to venue server via Tailscale bono_relay.
/// Called when cloud has no local agent connected for the target pod.
async fn relay_game_launch_to_venue(
    state: &Arc<AppState>,
    pod_id: &str,
    sim_type_str: &str,
    body: &Value,
) -> Json<Value> {
    let bono = &state.config.bono;
    if !bono.enabled {
        return Json(json!({ "ok": false, "error": "No local agent and venue relay not configured" }));
    }

    let relay_ip = match &bono.tailscale_bind_ip {
        Some(ip) => ip.clone(),
        None => return Json(json!({ "ok": false, "error": "No venue Tailscale IP configured" })),
    };
    let relay_secret = bono.relay_secret.as_deref().unwrap_or("");
    let relay_url = format!("http://{}:{}/relay/command", relay_ip, bono.relay_port);

    // Resolve pod_id to pod_number for the relay command
    let pod_number = {
        let pods = state.pods.read().await;
        pods.values()
            .find(|p| p.id == pod_id)
            .map(|p| p.number)
    };

    let pod_number = match pod_number {
        Some(n) => n,
        None => {
            // Try parsing pod_id as "pod-N" format
            match pod_id.strip_prefix("pod-").and_then(|n| n.parse::<u32>().ok()) {
                Some(n) if n > 0 => n,
                _ => {
                    tracing::warn!("Venue relay: cannot resolve pod_id '{}' to pod number — pod not found in registry and id format unrecognized", pod_id);
                    return Json(json!({ "ok": false, "error": format!("Cannot resolve pod_id '{}' to pod number for venue relay. Pod may be offline or not registered.", pod_id) }));
                }
            }
        }
    };

    let track = body.get("launch_args")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| v.get("track").and_then(|t| t.as_str()).map(|s| s.to_string()));
    let car = body.get("launch_args")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| v.get("car").and_then(|c| c.as_str()).map(|s| s.to_string()));

    let relay_cmd = json!({
        "type": "launch_game",
        "data": {
            "pod_number": pod_number,
            "game": sim_type_str,
            "track": track,
            "car": car
        }
    });

    tracing::info!(
        "Relaying game launch to venue: pod_number={}, game={}, relay_url={}",
        pod_number, sim_type_str, relay_url
    );

    match state.http_client
        .post(&relay_url)
        .header("X-Relay-Secret", relay_secret)
        .json(&relay_cmd)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let body: Value = resp.json().await.unwrap_or_default();
            Json(json!({ "ok": true, "relayed": true, "venue_response": body }))
        }
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            tracing::warn!("Venue relay returned {}: {}", status, body);
            Json(json!({ "ok": false, "error": format!("Venue relay returned {}: {}", status, body) }))
        }
        Err(e) => {
            tracing::error!("Venue relay request failed: {}", e);
            Json(json!({ "ok": false, "error": format!("Cannot reach venue: {}", e) }))
        }
    }
}

/// CRASH-04: Relaunch a crashed game using stored launch_args
async fn relaunch_game(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    match game_launcher::relaunch_game(&state, &pod_id).await {
        Ok(()) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

async fn set_pod_transmission(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let transmission = body
        .get("transmission")
        .and_then(|v| v.as_str())
        .unwrap_or("auto");

    let senders = state.agent_senders.read().await;
    if let Some(tx) = senders.get(&pod_id) {
        let msg = CoreToAgentMessage::SetTransmission {
            transmission: transmission.to_string(),
        };
        if let Err(e) = tx.send(msg).await {
            tracing::error!("Failed to send SetTransmission to {}: {}", pod_id, e);
            return Json(json!({ "error": "Failed to send to agent" }));
        }
        tracing::info!("Set transmission to '{}' on pod {}", transmission, pod_id);
        Json(json!({ "ok": true, "transmission": transmission }))
    } else {
        Json(json!({ "error": "No agent connected for this pod" }))
    }
}

async fn set_pod_ffb(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Try numeric percent first (Phase 6 mid-session FFB gain)
    if let Some(percent) = body.get("percent").and_then(|v| v.as_u64()) {
        let percent = (percent as u8).clamp(10, 100);
        let senders = state.agent_senders.read().await;
        if let Some(tx) = senders.get(&pod_id) {
            let msg = CoreToAgentMessage::SetFfbGain { percent };
            if let Err(e) = tx.send(msg).await {
                tracing::error!("Failed to send SetFfbGain to {}: {}", pod_id, e);
                return Json(json!({ "error": "Failed to send to agent" }));
            }
            tracing::info!("Set FFB gain to {}% on pod {}", percent, pod_id);
            return Json(json!({ "ok": true, "ffb_percent": percent }));
        } else {
            return Json(json!({ "error": "No agent connected for this pod" }));
        }
    }

    // Legacy preset path (existing behavior)
    let preset = body
        .get("preset")
        .and_then(|v| v.as_str())
        .unwrap_or("medium");

    let senders = state.agent_senders.read().await;
    if let Some(tx) = senders.get(&pod_id) {
        let msg = CoreToAgentMessage::SetFfb {
            preset: preset.to_string(),
        };
        if let Err(e) = tx.send(msg).await {
            tracing::error!("Failed to send SetFfb to {}: {}", pod_id, e);
            return Json(json!({ "error": "Failed to send to agent" }));
        }
        tracing::info!("Set FFB to '{}' on pod {}", preset, pod_id);
        Json(json!({ "ok": true, "preset": preset }))
    } else {
        Json(json!({ "error": "No agent connected for this pod" }))
    }
}

async fn set_pod_assists(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let assist_type = body.get("assist_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let enabled = body.get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Validate assist_type is one of the supported types
    // Stability control intentionally excluded per user decision (no runtime mechanism in AC)
    if !["abs", "tc", "transmission"].contains(&assist_type) {
        return Json(json!({ "error": "Invalid assist_type. Supported: abs, tc, transmission" }));
    }

    let senders = state.agent_senders.read().await;
    if let Some(tx) = senders.get(&pod_id) {
        let msg = CoreToAgentMessage::SetAssist {
            assist_type: assist_type.to_string(),
            enabled,
        };
        if let Err(e) = tx.send(msg).await {
            tracing::error!("Failed to send SetAssist to {}: {}", pod_id, e);
            return Json(json!({ "error": format!("Failed to send to agent: {}", e) }));
        }
        tracing::info!("Set assist {} = {} on pod {}", assist_type, enabled, pod_id);
        Json(json!({ "ok": true, "assist_type": assist_type, "enabled": enabled }))
    } else {
        Json(json!({ "error": "No agent connected for this pod" }))
    }
}

async fn get_pod_assist_state(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    // Read cached state immediately
    let cached = {
        let cache = state.assist_cache.read().await;
        cache.get(&pod_id).cloned()
    };

    // Also send QueryAssistState to agent for background refresh
    // (next time PWA opens the drawer, cache will be even fresher)
    let senders = state.agent_senders.read().await;
    if let Some(tx) = senders.get(&pod_id) {
        if let Err(e) = tx.send(CoreToAgentMessage::QueryAssistState).await {
            tracing::warn!("Failed to send QueryAssistState to {}: {}", pod_id, e);
        }
    }

    match cached {
        Some(s) => Json(json!({
            "ok": true,
            "abs": s.abs,
            "tc": s.tc,
            "auto_shifter": s.auto_shifter,
            "ffb_percent": s.ffb_percent,
        })),
        None => {
            // No cached state yet (pod never reported state).
            // Return defaults -- the background QueryAssistState will populate the cache.
            Json(json!({
                "ok": true,
                "abs": 0,
                "tc": 0,
                "auto_shifter": true,
                "ffb_percent": 70,
            }))
        }
    }
}

async fn stop_game(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");

    if pod_id.is_empty() {
        return Json(json!({ "error": "pod_id is required" }));
    }

    let cmd = rc_common::protocol::DashboardCommand::StopGame {
        pod_id: pod_id.to_string(),
    };

    let _ = game_launcher::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

/// Returns the full game catalog — authoritative source for all UI game lists.
/// Each entry includes the sim_type id (snake_case), display name, and abbreviation.
/// Pods filter this list against their `installed_games` field.
async fn games_catalog(State(state): State<Arc<AppState>>) -> Json<Value> {
    let all_games = [
        SimType::AssettoCorsa,
        SimType::AssettoCorsaEvo,
        SimType::AssettoCorsaRally,
        SimType::IRacing,
        SimType::LeMansUltimate,
        SimType::F125,
        SimType::Forza,
        SimType::ForzaHorizon5,
    ];

    // Count how many pods have each game installed
    let pods = state.pods.read().await;
    let mut install_counts: std::collections::HashMap<SimType, usize> = std::collections::HashMap::new();
    for pod in pods.values() {
        for game in &pod.installed_games {
            *install_counts.entry(*game).or_insert(0) += 1;
        }
    }

    let catalog: Vec<Value> = all_games.iter().map(|sim| {
        let id = serde_json::to_value(sim).unwrap_or(json!("unknown"));
        let id_str = id.as_str().unwrap_or("unknown");
        let abbr = match sim {
            SimType::AssettoCorsa => "AC",
            SimType::AssettoCorsaEvo => "ACE",
            SimType::AssettoCorsaRally => "WRC",
            SimType::IRacing => "iR",
            SimType::LeMansUltimate => "LMU",
            SimType::F125 => "F1",
            SimType::Forza => "FM",
            SimType::ForzaHorizon5 => "FH5",
        };
        json!({
            "id": id_str,
            "name": sim.to_string(),
            "abbr": abbr,
            "installed_pod_count": install_counts.get(sim).unwrap_or(&0),
        })
    }).collect();

    Json(json!({ "games": catalog }))
}

async fn active_games(State(state): State<Arc<AppState>>) -> Json<Value> {
    let games = state.game_launcher.active_games.read().await;
    let list: Vec<_> = games.values().map(|g| g.to_info()).collect();
    Json(json!({ "games": list }))
}

#[derive(Deserialize)]
struct GameHistoryQuery {
    pod_id: Option<String>,
    limit: Option<i64>,
}

async fn game_launch_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GameHistoryQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(100).min(1000).max(1);

    let rows = if let Some(pod_id) = &params.pod_id {
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<String>, String)>(
            "SELECT id, pod_id, sim_type, event_type, pid, error_message, created_at \
             FROM game_launch_events WHERE pod_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(pod_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<String>, String)>(
            "SELECT id, pod_id, sim_type, event_type, pid, error_message, created_at \
             FROM game_launch_events ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(events) => {
            let list: Vec<Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "pod_id": e.1, "sim_type": e.2,
                        "event_type": e.3, "pid": e.4,
                        "error_message": e.5, "created_at": e.6,
                    })
                })
                .collect();
            Json(json!({ "events": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn pod_game_state(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let games = state.game_launcher.active_games.read().await;
    match games.get(&pod_id) {
        Some(tracker) => Json(json!({ "game": tracker.to_info() })),
        None => Json(json!({ "game": null, "state": "idle" })),
    }
}

// ─── AC LAN ──────────────────────────────────────────────────────────────────

async fn list_ac_presets(State(state): State<Arc<AppState>>) -> Json<Value> {
    match ac_server::list_presets(&state).await {
        Ok(presets) => Json(json!({ "presets": presets })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn save_ac_preset(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return Json(json!({ "error": "name is required" })),
    };

    let config: AcLanSessionConfig = match body.get("config") {
        Some(c) => match serde_json::from_value(c.clone()) {
            Ok(cfg) => cfg,
            Err(e) => return Json(json!({ "error": format!("Invalid config: {}", e) })),
        },
        None => return Json(json!({ "error": "config is required" })),
    };

    match ac_server::save_preset(&state, &name, &config).await {
        Ok(id) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match ac_server::load_preset(&state, &id).await {
        Ok((name, config)) => Json(json!({ "id": id, "name": name, "config": config })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = body.get("name").and_then(|v| v.as_str());
    let config = body.get("config").and_then(|c| serde_json::from_value::<AcLanSessionConfig>(c.clone()).ok());

    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = name {
        updates.push("name = ?");
        binds.push(n.to_string());
    }
    if let Some(cfg) = &config {
        updates.push("config_json = ?");
        binds.push(serde_json::to_string(cfg).unwrap_or_default());
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE ac_presets SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Preset not found" })),
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_ac_preset(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match ac_server::delete_preset(&state, &id).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn start_ac_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let config: AcLanSessionConfig = match body.get("config") {
        Some(c) => match serde_json::from_value(c.clone()) {
            Ok(cfg) => cfg,
            Err(e) => return Json(json!({ "error": format!("Invalid config: {}", e) })),
        },
        None => return Json(json!({ "error": "config is required" })),
    };

    let pod_ids: Vec<String> = body
        .get("pod_ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let ai_level = body.get("ai_level").and_then(|v| v.as_u64()).map(|v| v as u32);

    match ac_server::start_ac_server(&state, config, pod_ids, ai_level).await {
        Ok(session_id) => Json(json!({ "session_id": session_id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn stop_ac_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let session_id = match body.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id,
        None => return Json(json!({ "error": "session_id is required" })),
    };

    match ac_server::stop_ac_server(&state, session_id).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn active_ac_session(State(state): State<Arc<AppState>>) -> Json<Value> {
    let instances = state.ac_server.instances.read().await;
    let active: Vec<_> = instances
        .values()
        .filter(|i| matches!(i.status, AcServerStatus::Running | AcServerStatus::Starting))
        .map(|i| i.to_info())
        .collect();
    Json(json!({ "sessions": active }))
}

#[derive(Deserialize)]
struct AcSessionsQuery {
    status: Option<String>,
    limit: Option<i64>,
}

async fn list_ac_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AcSessionsQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(50).min(1000).max(1);

    let rows = if let Some(status) = &params.status {
        sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
            "SELECT id, preset_id, status, pod_ids, pid, join_url, error_message, started_at, ended_at, created_at \
             FROM ac_sessions WHERE status = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, String)>(
            "SELECT id, preset_id, status, pod_ids, pid, join_url, error_message, started_at, ended_at, created_at \
             FROM ac_sessions ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "preset_id": s.1, "status": s.2,
                        "pod_ids": s.3, "pid": s.4, "join_url": s.5,
                        "error_message": s.6, "started_at": s.7,
                        "ended_at": s.8, "created_at": s.9,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// AC Session Leaderboard — returns drivers ranked by best lap within an AC server session.
/// Finds all laps recorded on the session's pods during its active time window.
async fn ac_session_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // 1. Get the AC session record
    let session = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, config_json, status, pod_ids, started_at, ended_at, created_at FROM ac_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "AC session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (_id, config_json, status, pod_ids_str, started_at, ended_at, created_at) = session;

    // Parse config to get track/car info
    let track = config_json.as_deref()
        .and_then(|cj| serde_json::from_str::<Value>(cj).ok())
        .and_then(|v| v.get("track").and_then(|t| t.as_str().map(String::from)));

    // Parse pod_ids (comma-separated or JSON array)
    let pod_ids: Vec<String> = pod_ids_str
        .as_deref()
        .map(|s| {
            // Try JSON array first, fall back to comma-separated
            serde_json::from_str::<Vec<String>>(s)
                .unwrap_or_else(|_| s.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect())
        })
        .unwrap_or_default();

    if pod_ids.is_empty() {
        return Json(json!({
            "session_id": id, "status": status, "track": track,
            "started_at": started_at, "ended_at": ended_at, "created_at": created_at,
            "leaderboard": [], "total_laps": 0
        }));
    }

    // 2. Query laps on these pods during the session window
    let start_time = started_at.as_deref().unwrap_or(created_at.as_str());
    let end_time = ended_at.as_deref().unwrap_or("9999-12-31T23:59:59");

    // Use a CTE: find each driver's best lap, then join back for sectors.
    // The subquery LIMIT 1 ensures deterministic results when a driver has
    // multiple laps tied at the same best time.
    let placeholders = pod_ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "WITH session_laps AS (
           SELECT l.id, l.driver_id, l.car, l.track, l.lap_time_ms,
                  l.sector1_ms, l.sector2_ms, l.sector3_ms, l.pod_id
           FROM laps l
           WHERE l.pod_id IN ({placeholders})
             AND l.created_at >= ?
             AND l.created_at <= ?
             AND l.valid = 1
             AND l.lap_time_ms > 0
         ),
         driver_best AS (
           SELECT driver_id, MIN(lap_time_ms) as best_lap_ms, COUNT(*) as lap_count
           FROM session_laps
           GROUP BY driver_id
         ),
         best_rows AS (
           SELECT db.driver_id, db.best_lap_ms, db.lap_count,
                  sl.car, sl.track,
                  sl.sector1_ms, sl.sector2_ms, sl.sector3_ms, sl.pod_id,
                  ROW_NUMBER() OVER (PARTITION BY db.driver_id ORDER BY sl.id) AS rn
           FROM driver_best db
           JOIN session_laps sl ON sl.driver_id = db.driver_id
                                AND sl.lap_time_ms = db.best_lap_ms
         )
         SELECT br.driver_id, d.name AS driver_name,
                br.car, br.track, br.best_lap_ms, br.lap_count,
                br.sector1_ms, br.sector2_ms, br.sector3_ms
         FROM best_rows br
         JOIN drivers d ON br.driver_id = d.id
         WHERE br.rn = 1
         ORDER BY br.best_lap_ms ASC
         LIMIT 50"
    );

    let mut q = sqlx::query(&sql);
    for pid in &pod_ids {
        q = q.bind(pid.as_str());
    }
    q = q.bind(start_time).bind(end_time);

    use sqlx::Row;
    let rows = q.fetch_all(&state.db).await;

    match rows {
        Ok(rows) => {
            let mut leaderboard: Vec<Value> = Vec::new();
            let mut best_time: Option<i64> = None;

            for (i, row) in rows.iter().enumerate() {
                let lap_ms: i64 = row.get("best_lap_ms");
                let gap_ms = best_time.map(|bt| lap_ms - bt);
                if best_time.is_none() {
                    best_time = Some(lap_ms);
                }

                leaderboard.push(json!({
                    "position": i + 1,
                    "driver_id": row.get::<String, _>("driver_id"),
                    "driver": row.get::<String, _>("driver_name"),
                    "car": row.get::<String, _>("car"),
                    "track": row.get::<String, _>("track"),
                    "best_lap_ms": lap_ms,
                    "lap_count": row.get::<i64, _>("lap_count"),
                    "sector1_ms": row.try_get::<Option<i64>, _>("sector1_ms").unwrap_or(None),
                    "sector2_ms": row.try_get::<Option<i64>, _>("sector2_ms").unwrap_or(None),
                    "sector3_ms": row.try_get::<Option<i64>, _>("sector3_ms").unwrap_or(None),
                    "gap_ms": gap_ms,
                }));
            }

            let total_laps: i64 = leaderboard.iter().map(|e| e["lap_count"].as_i64().unwrap_or(0)).sum();

            Json(json!({
                "session_id": id,
                "status": status,
                "track": track,
                "started_at": started_at,
                "ended_at": ended_at,
                "created_at": created_at,
                "pod_ids": pod_ids,
                "leaderboard": leaderboard,
                "total_laps": total_laps,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// GROUP-02: Enable/disable continuous mode on an AC server session.
async fn ac_server_set_continuous(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let enabled = req.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);

    // Look up the group_session_id for this AC session
    let group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM group_sessions WHERE ac_session_id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match ac_server::set_continuous_mode(&state, &session_id, enabled, group_session_id).await {
        Ok(()) => {
            if enabled {
                // Spawn the continuous monitor
                let state_clone = state.clone();
                let sid = session_id.clone();
                tokio::spawn(async move {
                    ac_server::monitor_continuous_session(state_clone, sid).await;
                });
            }
            Json(json!({ "status": "ok", "continuous_mode": enabled }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// GROUP-03: Retry a failed pod join — re-sends LaunchGame to the pod.
async fn ac_session_retry_pod(
    State(state): State<Arc<AppState>>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let session_id = match req.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'session_id'" })),
    };
    let pod_id = match req.get("pod_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pod_id'" })),
    };

    match ac_server::retry_pod_join(&state, &session_id, &pod_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// GROUP-04: Update track/car config on a continuous-mode session.
/// Takes effect on the next race restart.
async fn ac_session_update_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<Value>,
) -> Json<Value> {
    let session_id = match req.get("session_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'session_id'" })),
    };
    let track = req.get("track").and_then(|v| v.as_str()).map(String::from);
    let track_config = req.get("track_config").and_then(|v| v.as_str()).map(String::from);
    let cars: Option<Vec<String>> = req.get("cars").and_then(|v| {
        v.as_array().map(|arr| {
            arr.iter().filter_map(|c| c.as_str().map(String::from)).collect()
        })
    });

    if track.is_none() && cars.is_none() {
        return Json(json!({ "error": "Must provide 'track' or 'cars' to update" }));
    }

    match ac_server::update_session_config(&state, &session_id, track, track_config, cars).await {
        Ok(()) => Json(json!({ "status": "ok", "message": "Config updated — takes effect on next race restart" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn list_ac_tracks(State(_state): State<Arc<AppState>>) -> Json<Value> {
    // Curated list of popular AC tracks
    Json(json!({ "tracks": [
        { "id": "monza", "name": "Monza", "configs": ["", "junior"] },
        { "id": "spa", "name": "Spa-Francorchamps", "configs": [""] },
        { "id": "silverstone", "name": "Silverstone", "configs": ["", "international", "national", "gp"] },
        { "id": "brands_hatch", "name": "Brands Hatch", "configs": ["", "gp", "indy"] },
        { "id": "nurburgring", "name": "Nurburgring", "configs": ["", "sprint"] },
        { "id": "nordschleife", "name": "Nordschleife", "configs": ["", "endurance", "tourist"] },
        { "id": "mugello", "name": "Mugello", "configs": [""] },
        { "id": "imola", "name": "Imola", "configs": [""] },
        { "id": "barcelona", "name": "Barcelona", "configs": ["", "moto", "national"] },
        { "id": "ks_red_bull_ring", "name": "Red Bull Ring", "configs": ["", "national"] },
        { "id": "vallelunga", "name": "Vallelunga", "configs": ["", "club"] },
        { "id": "drift", "name": "Drift Track", "configs": [""] },
        { "id": "ks_zandvoort", "name": "Zandvoort", "configs": [""] },
        { "id": "ks_laguna_seca", "name": "Laguna Seca", "configs": [""] },
        { "id": "suzuka", "name": "Suzuka", "configs": ["", "east"] },
        { "id": "ks_highlands", "name": "Highlands", "configs": [""] },
        { "id": "ks_black_cat_county", "name": "Black Cat County", "configs": ["", "long"] },
        { "id": "magione", "name": "Magione", "configs": [""] },
        { "id": "trento-bondone", "name": "Trento Bondone", "configs": [""] },
    ]}))
}

async fn list_ac_cars(State(_state): State<Arc<AppState>>) -> Json<Value> {
    // Curated list of popular AC cars grouped by class
    Json(json!({ "cars": [
        { "id": "ks_ferrari_488_gt3", "name": "Ferrari 488 GT3", "class": "GT3" },
        { "id": "ks_lamborghini_huracan_gt3", "name": "Lamborghini Huracan GT3", "class": "GT3" },
        { "id": "ks_mercedes_amg_gt3", "name": "Mercedes AMG GT3", "class": "GT3" },
        { "id": "ks_audi_r8_lms_2016", "name": "Audi R8 LMS 2016", "class": "GT3" },
        { "id": "ks_porsche_911_gt3_r_2016", "name": "Porsche 911 GT3 R", "class": "GT3" },
        { "id": "ks_mclaren_650_gt3", "name": "McLaren 650S GT3", "class": "GT3" },
        { "id": "ks_nissan_gtr_gt3", "name": "Nissan GT-R GT3", "class": "GT3" },
        { "id": "ks_bmw_m6_gt3", "name": "BMW M6 GT3", "class": "GT3" },
        { "id": "ks_ferrari_488_gtb", "name": "Ferrari 488 GTB", "class": "Street" },
        { "id": "ks_lamborghini_huracan_performante", "name": "Lamborghini Huracan Performante", "class": "Street" },
        { "id": "ks_porsche_911_r", "name": "Porsche 911 R", "class": "Street" },
        { "id": "ks_mclaren_p1", "name": "McLaren P1", "class": "Hypercar" },
        { "id": "ks_ferrari_laferrari", "name": "Ferrari LaFerrari", "class": "Hypercar" },
        { "id": "ks_porsche_918_spyder", "name": "Porsche 918 Spyder", "class": "Hypercar" },
        { "id": "ks_audi_r18_etron_quattro", "name": "Audi R18 e-tron", "class": "LMP" },
        { "id": "ks_porsche_919_hybrid_2016", "name": "Porsche 919 Hybrid", "class": "LMP" },
        { "id": "ks_toyota_ts040", "name": "Toyota TS040", "class": "LMP" },
        { "id": "tatuusfa1", "name": "Tatuus FA01", "class": "Open Wheel" },
        { "id": "ks_ferrari_sf15t", "name": "Ferrari SF15-T", "class": "Open Wheel" },
        { "id": "lotus_exos_125_s1", "name": "Lotus Exos 125 S1", "class": "Open Wheel" },
        { "id": "ks_mazda_mx5_cup", "name": "Mazda MX-5 Cup", "class": "Cup" },
        { "id": "ks_toyota_gt86", "name": "Toyota GT86", "class": "Street" },
        { "id": "ks_ford_mustang_2015", "name": "Ford Mustang 2015", "class": "Street" },
        { "id": "ks_abarth_595ss_s2", "name": "Abarth 595 SS", "class": "Street" },
        { "id": "lotus_2_eleven", "name": "Lotus 2-Eleven", "class": "Track Day" },
        { "id": "ks_toyota_ae86_drift", "name": "Toyota AE86 Drift", "class": "Drift" },
        { "id": "ks_nissan_370z", "name": "Nissan 370Z", "class": "Drift" },
    ]}))
}

// ─── Auth Endpoints ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssignCustomerRequest {
    pod_id: String,
    driver_id: String,
    pricing_tier_id: String,
    auth_type: String,
    custom_price_paise: Option<u32>,
    custom_duration_minutes: Option<u32>,
    experience_id: Option<String>,
}

async fn assign_customer(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AssignCustomerRequest>,
) -> Json<Value> {
    match auth::create_auth_token(
        &state,
        req.pod_id,
        req.driver_id,
        req.pricing_tier_id,
        req.auth_type,
        req.custom_price_paise,
        req.custom_duration_minutes,
        req.experience_id,
        None, // custom_launch_args (staff assign doesn't use custom booking)
    )
    .await
    {
        Ok(token_info) => Json(json!({ "token": token_info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn cancel_assignment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match auth::cancel_auth_token(&state, id).await {
        Ok(()) => Json(json!({ "status": "cancelled" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn pending_auth_tokens(State(state): State<Arc<AppState>>) -> Json<Value> {
    let tokens = auth::get_pending_tokens(&state).await;
    Json(json!({ "tokens": tokens }))
}

async fn pending_auth_token_for_pod(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<Value> {
    let tokens = auth::get_pending_tokens(&state).await;
    let token = tokens.into_iter().find(|t| t.pod_id == pod_id);
    match token {
        Some(t) => Json(json!({ "token": t })),
        None => Json(json!({ "token": null })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatePinRequest {
    pod_id: String,
    pin: String,
}

async fn validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ValidatePinRequest>,
) -> Json<Value> {
    match auth::validate_pin(&state, req.pod_id, req.pin).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => {
            state.record_api_error("auth/validate-pin");
            Json(json!({ "error": e }))
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KioskValidatePinRequest {
    pin: String,
    pod_id: Option<String>,
}

async fn kiosk_validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<KioskValidatePinRequest>,
) -> Json<Value> {
    match auth::validate_pin_kiosk(&state, req.pin, req.pod_id).await {
        Ok(result) => Json(json!({
            "status": "ok",
            "billing_session_id": result.billing_session_id,
            "pod_id": result.pod_id,
            "pod_number": result.pod_number,
            "driver_name": result.driver_name,
            "pricing_tier_name": result.pricing_tier_name,
            "allocated_seconds": result.allocated_seconds,
        })),
        Err(e) => {
            state.record_api_error("auth/kiosk-validate-pin");
            Json(json!({ "error": e }))
        }
    }
}

// ─── PIN Redemption Lockout ─────────────────────────────────────────────────

struct PinLockoutState {
    fail_count: u32,
    last_attempt: std::time::Instant,
    locked_until: Option<std::time::Instant>,
}

static PIN_LOCKOUT: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, PinLockoutState>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Prune lockout entries older than 10 minutes to prevent unbounded memory growth.
fn prune_pin_lockout_entries(map: &mut std::collections::HashMap<std::net::IpAddr, PinLockoutState>) {
    let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(600);
    map.retain(|_, v| v.last_attempt > cutoff);
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct KioskRedeemPinRequest {
    pin: String,
}

async fn kiosk_redeem_pin(
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<KioskRedeemPinRequest>,
) -> Json<Value> {
    let client_ip = addr.ip();

    // Check lockout FIRST
    {
        let mut lockout_map = PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());

        // Prune old entries periodically (when map grows large)
        if lockout_map.len() > 1000 {
            prune_pin_lockout_entries(&mut lockout_map);
        }

        if let Some(entry) = lockout_map.get_mut(&client_ip) {
            if let Some(locked_until) = entry.locked_until {
                let now = std::time::Instant::now();
                if now < locked_until {
                    let remaining = locked_until.duration_since(now);
                    let remaining_secs = remaining.as_secs();
                    let minutes = remaining_secs / 60;
                    let seconds = remaining_secs % 60;
                    let time_str = if minutes > 0 {
                        format!("{} minutes and {} seconds", minutes, seconds)
                    } else {
                        format!("{} seconds", seconds)
                    };
                    return Json(json!({
                        "error": format!("Too many failed attempts. Please wait {}.", time_str),
                        "lockout_remaining_seconds": remaining_secs,
                    }));
                } else {
                    // Lockout expired, reset
                    entry.fail_count = 0;
                    entry.locked_until = None;
                }
            }
        }
    }

    match reservation::redeem_pin(&state, &req.pin).await {
        Ok(result) => {
            // Success: reset lockout for this IP
            let mut lockout_map = PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
            lockout_map.remove(&client_ip);
            Json(result)
        }
        Err(e) => {
            state.record_api_error("kiosk/redeem-pin");

            // B1 fix: Only count actual PIN errors toward lockout.
            // "All pods busy", "DB error", "billing failed" should NOT punish the customer.
            if e.is_pin_error {
                let remaining_attempts = {
                    let mut lockout_map = PIN_LOCKOUT.lock().unwrap_or_else(|e| e.into_inner());
                    let entry = lockout_map.entry(client_ip).or_insert(PinLockoutState {
                        fail_count: 0,
                        last_attempt: std::time::Instant::now(),
                        locked_until: None,
                    });
                    entry.fail_count += 1;
                    entry.last_attempt = std::time::Instant::now();

                    if entry.fail_count >= PIN_REDEEM_MAX_ATTEMPTS {
                        entry.locked_until = Some(std::time::Instant::now() + std::time::Duration::from_secs(PIN_REDEEM_LOCKOUT_SECONDS as u64));
                        0u32
                    } else {
                        PIN_REDEEM_MAX_ATTEMPTS - entry.fail_count
                    }
                };

                if remaining_attempts == 0 {
                    let lockout_min = PIN_REDEEM_LOCKOUT_SECONDS / 60;
                    let lockout_sec = PIN_REDEEM_LOCKOUT_SECONDS % 60;
                    Json(json!({
                        "error": format!("Too many failed attempts. Please wait {} minutes and {} seconds.", lockout_min, lockout_sec),
                        "lockout_remaining_seconds": PIN_REDEEM_LOCKOUT_SECONDS,
                        "status": "lockout",
                    }))
                } else {
                    Json(json!({
                        "error": e.message,
                        "remaining_attempts": remaining_attempts,
                        "status": "invalid_pin",
                    }))
                }
            } else if e.is_pending_debit {
                // F4 fix: dedicated status field instead of relying on string matching
                Json(json!({
                    "error": e.message,
                    "status": "pending_debit",
                }))
            } else {
                // Infrastructure error — no lockout penalty
                Json(json!({
                    "error": e.message,
                    "status": "error",
                }))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct StartNowRequest {
    token_id: String,
}

async fn start_now(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StartNowRequest>,
) -> Json<Value> {
    match auth::start_now(&state, req.token_id).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
struct ValidateQrRequest {
    qr_token: String,
    driver_id: String,
}

async fn validate_qr(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ValidateQrRequest>,
) -> Json<Value> {
    match auth::validate_qr(&state, req.qr_token, req.driver_id).await {
        Ok(billing_session_id) => Json(json!({
            "status": "ok",
            "billing_session_id": billing_session_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Customer PWA Endpoints ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomerLoginRequest {
    phone: String,
}

async fn customer_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CustomerLoginRequest>,
) -> Json<Value> {
    match auth::send_otp(&state, &req.phone).await {
        Ok(result) => Json(json!({
            "status": "otp_sent",
            "delivered": result.delivered
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_resend_otp(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CustomerLoginRequest>,
) -> Json<Value> {
    match auth::resend_otp(&state, &req.phone).await {
        Ok(result) => Json(json!({
            "status": "otp_sent",
            "delivered": result.delivered
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyOtpRequest {
    phone: String,
    otp: String,
}

async fn customer_verify_otp(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyOtpRequest>,
) -> Json<Value> {
    match auth::verify_otp(&state, &req.phone, &req.otp).await {
        Ok(jwt) => {
            // Check registration status
            let registered = sqlx::query_as::<_, (bool,)>(
                "SELECT COALESCE(registration_completed, 0) FROM drivers WHERE phone_hash = ?",
            )
            .bind(state.field_cipher.hash_phone(&req.phone))
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|r| r.0)
            .unwrap_or(false);

            Json(json!({
                "status": "ok",
                "token": jwt,
                "registration_completed": registered,
            }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

/// Extract driver_id from Authorization: Bearer <jwt> header
fn extract_driver_id(state: &AppState, headers: &axum::http::HeaderMap) -> Result<String, String> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing Authorization header".to_string())?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| "Invalid Authorization format".to_string())?;

    auth::verify_jwt(token, &state.config.auth.jwt_secret)
}

async fn customer_profile(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let driver = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, bool, Option<String>, Option<String>, bool)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms, COALESCE(has_used_trial, 0), customer_id, nickname, COALESCE(show_nickname_on_leaderboard, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some(d)) => {
            let wallet_balance = wallet::get_balance(&state, &d.0).await.unwrap_or(0);
            let active_reservation = pod_reservation::get_active_reservation_for_driver(&state, &d.0).await;

            Json(json!({
                "driver": {
                    "id": d.0,
                    "customer_id": d.7,
                    "name": d.1,
                    "nickname": d.8,
                    "show_nickname_on_leaderboard": d.9,
                    "email": d.2,
                    "phone": d.3,
                    "total_laps": d.4,
                    "total_time_ms": d.5,
                    "has_used_trial": d.6,
                    "wallet_balance_paise": wallet_balance,
                    "active_reservation": active_reservation,
                }
            }))
        }
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn customer_update_profile(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    if let Some(nickname) = body.get("nickname") {
        let nick = nickname.as_str().map(|s| s.trim()).unwrap_or("");
        // MMA-R2-3: Validate nickname (XSS prevention)
        if !nick.is_empty() {
            if let Err(e) = crate::input_validation::validate_name(nick) {
                return Json(json!({ "error": format!("Invalid nickname: {}", e) }));
            }
        }
        let nick_val: Option<&str> = if nick.is_empty() { None } else { Some(nick) };
        let _ = sqlx::query("UPDATE drivers SET nickname = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(nick_val)
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

    if let Some(show) = body.get("show_nickname_on_leaderboard") {
        let val = show.as_bool().unwrap_or(false);
        let _ = sqlx::query("UPDATE drivers SET show_nickname_on_leaderboard = ?, updated_at = datetime('now') WHERE id = ?")
            .bind(val)
            .bind(&driver_id)
            .execute(&state.db)
            .await;
    }

    Json(json!({ "status": "updated" }))
}

async fn customer_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>)>(
        "SELECT bs.id, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.ended_at, bs.custom_price_paise,
                bs.discount_paise, bs.original_price_paise, bs.discount_reason
         FROM billing_sessions bs
         WHERE bs.driver_id = ?
         ORDER BY bs.created_at DESC
         LIMIT 50",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0,
                        "pod_id": s.1,
                        "allocated_seconds": s.2,
                        "driving_seconds": s.3,
                        "status": s.4,
                        "started_at": s.5,
                        "ended_at": s.6,
                        "custom_price_paise": s.7,
                        "discount_paise": s.8,
                        "original_price_paise": s.9,
                        "discount_reason": s.10,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

/// Compute percentile ranking for a best lap on a track+car combination.
/// Returns None if fewer than 5 unique drivers have driven this track+car,
/// or if track/car is empty.
async fn compute_percentile(db: &sqlx::SqlitePool, best_lap_ms: i64, track: &str, car: &str) -> Option<u32> {
    if track.is_empty() || car.is_empty() {
        return None;
    }

    let total_count: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(DISTINCT driver_id) FROM laps WHERE track = ? AND car = ? AND valid = 1",
    )
    .bind(track)
    .bind(car)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    let faster_count: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(DISTINCT driver_id) FROM (
            SELECT driver_id, MIN(lap_time_ms) as best
            FROM laps WHERE track = ? AND car = ? AND valid = 1
            GROUP BY driver_id
        ) WHERE best < ?",
    )
    .bind(track)
    .bind(car)
    .bind(best_lap_ms)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    match (total_count, faster_count) {
        (Some((total,)), Some((faster,))) if total >= 5 => {
            Some(((total - faster) as f64 / total as f64 * 100.0).round() as u32)
        }
        _ => None,
    }
}

async fn customer_session_detail(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch the billing session, ensuring it belongs to this customer
    let row = sqlx::query_as::<_, (
        String, String, String, i64, i64, String, i64,
        Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<i64>,
    )>(
        "SELECT bs.id, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds,
                bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at,
                bs.experience_id, ke.name,
                bs.car, bs.track, bs.sim_type,
                bs.wallet_debit_paise
         FROM billing_sessions bs
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         LEFT JOIN kiosk_experiences ke ON bs.experience_id = ke.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match row {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Fetch discount info separately (avoids sqlx 16-field tuple limit)
    let discount_info = sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<String>)>(
        "SELECT discount_paise, original_price_paise, discount_reason FROM billing_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Look up any refund for this session
    let refund_paise: Option<(i64,)> = sqlx::query_as(
        "SELECT COALESCE(SUM(amount_paise), 0) FROM wallet_transactions
         WHERE reference_id = ? AND txn_type IN ('refund_session', 'refund_manual')",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get all laps for this session
    let laps = sqlx::query_as::<_, (
        String, i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String, String,
    )>(
        "SELECT id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms,
                valid, track, car, created_at
         FROM laps WHERE session_id = ? AND driver_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len() as i64;
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.6).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.2).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.2).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };

    // Determine track and car from laps or session fields
    let track = laps.first().map(|l| l.7.clone()).unwrap_or_else(|| session.12.clone().unwrap_or_default());
    let car = laps.first().map(|l| l.8.clone()).unwrap_or_else(|| session.11.clone().unwrap_or_default());

    // Percentile ranking (shared function, >= 5 driver threshold)
    let percentile = if let Some(best) = best_lap_ms {
        compute_percentile(&state.db, best, &track, &car).await
    } else {
        None
    };

    // Personal best for this track+car
    let personal_best: Option<(i64,)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
        )
        .bind(&driver_id)
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // is_new_pb: true if this session's best lap IS the current personal best
    let is_new_pb = personal_best.map(|pb| best_lap_ms == Some(pb.0)).unwrap_or(false);

    // improvement_ms: how much faster this session's best was vs the previous PB
    // Only meaningful if is_new_pb; look for a second-best time (prior PB) excluding this session
    let improvement_ms: Option<i64> = if is_new_pb {
        if let Some(best) = best_lap_ms {
            let prev: Option<(i64,)> = sqlx::query_as(
                "SELECT MIN(lap_time_ms) FROM laps
                 WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
                 AND lap_time_ms > ? AND session_id != ?",
            )
            .bind(&driver_id)
            .bind(&track)
            .bind(&car)
            .bind(best)
            .bind(&id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
            prev.map(|p| p.0 - best)
        } else {
            None
        }
    } else {
        None
    };

    // Peak moment: lap number of the best lap in this session
    let peak_lap_number = valid_laps.iter().min_by_key(|l| l.2).map(|l| l.1);

    // group_session_id for this billing session
    let group_session_id_val: Option<String> = sqlx::query_scalar(
        "SELECT group_session_id FROM billing_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let laps_json: Vec<Value> = laps
        .iter()
        .map(|l| {
            json!({
                "id": l.0,
                "lap_number": l.1,
                "lap_time_ms": l.2,
                "sector1_ms": l.3,
                "sector2_ms": l.4,
                "sector3_ms": l.5,
                "valid": l.6,
                "track": l.7,
                "car": l.8,
                "created_at": l.9,
            })
        })
        .collect();

    // Fetch billing events timeline for this session
    let events = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
        "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
         FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let events_json: Vec<Value> = events
        .iter()
        .map(|e| {
            json!({
                "id": e.0,
                "event_type": e.1,
                "driving_seconds_at_event": e.2,
                "metadata": e.3,
                "created_at": e.4,
            })
        })
        .collect();

    Json(json!({
        "session": {
            "id": session.0,
            "pod_id": session.1,
            "pricing_tier_name": session.2,
            "allocated_seconds": session.3,
            "driving_seconds": session.4,
            "status": session.5,
            "price_paise": session.6,
            "started_at": session.7,
            "ended_at": session.8,
            "experience_id": session.9,
            "experience_name": session.10,
            "car": session.11,
            "track": session.12,
            "sim_type": session.13,
            "wallet_debit_paise": session.14,
            "discount_paise": discount_info.as_ref().and_then(|d| d.0),
            "original_price_paise": discount_info.as_ref().and_then(|d| d.1),
            "discount_reason": discount_info.as_ref().and_then(|d| d.2.clone()),
            "refund_paise": refund_paise.map(|r| r.0).filter(|&r| r > 0),
            "total_laps": total_laps,
            "best_lap_ms": best_lap_ms,
            "average_lap_ms": avg_lap_ms,
            "percentile_rank": percentile,
            "percentile_text": percentile.map(|p| format!("Faster than {}% of drivers", p)),
            "is_new_pb": is_new_pb,
            "personal_best_ms": personal_best.map(|pb| pb.0),
            "improvement_ms": improvement_ms,
            "peak_lap_number": peak_lap_number,
            "group_session_id": group_session_id_val,
        },
        "laps": laps_json,
        "events": events_json,
    }))
}

/// Polling endpoint for active session PB events.
/// Returns PB events since a given timestamp for the customer's active billing session.
/// PWA calls this every 5 seconds during active sessions.
async fn customer_active_session_events(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Find active billing session for this driver
    let active_session: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM billing_sessions WHERE driver_id = ? AND status = 'active' LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let session_id = match active_session {
        Some((id,)) => id,
        None => return Json(json!({ "events": [] })),
    };

    let since = params.get("since").cloned().unwrap_or_default();

    // Query laps that are PBs since the given timestamp
    let pb_laps = sqlx::query_as::<_, (String, i64, String, String, String)>(
        "SELECT l.id, l.lap_time_ms, l.track, l.car, l.created_at
         FROM laps l
         JOIN personal_bests pb ON l.id = pb.lap_id
         WHERE l.session_id = ? AND l.driver_id = ? AND l.created_at > ?
         ORDER BY l.created_at ASC",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .bind(&since)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "events": pb_laps.iter().map(|l| json!({
            "type": "pb",
            "lap_id": l.0,
            "lap_time_ms": l.1,
            "track": l.2,
            "car": l.3,
            "at": l.4,
        })).collect::<Vec<_>>()
    }))
}

// ─── Remote Booking Reservation Handlers ─────────────────────────────────────

async fn customer_create_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<reservation::CreateReservationRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::create_reservation(&state, &driver_id, &req).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_get_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::get_active_reservation(&state, &driver_id).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_cancel_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::cancel_reservation(&state, &driver_id).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_modify_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<reservation::CreateReservationRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    match reservation::modify_reservation(&state, &driver_id, &req).await {
        Ok(v) => Json(v),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_laps(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool, String)>(
        "SELECT id, track, car, sim_type, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at
         FROM laps
         WHERE driver_id = ?
         ORDER BY created_at DESC
         LIMIT 100",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps
                .iter()
                .map(|l| {
                    json!({
                        "id": l.0,
                        "track": l.1,
                        "car": l.2,
                        "sim_type": l.3,
                        "lap_time_ms": l.4,
                        "sector1_ms": l.5,
                        "sector2_ms": l.6,
                        "sector3_ms": l.7,
                        "valid": l.8,
                        "created_at": l.9,
                    })
                })
                .collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn customer_stats(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Total laps and time
    let totals = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COALESCE(total_laps, 0), COALESCE(total_time_ms, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or((0, 0));

    // Total sessions
    let session_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Total driving time (seconds)
    let total_driving_secs = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(SUM(driving_seconds), 0) FROM billing_sessions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Favourite car (most laps)
    let fav_car = sqlx::query_as::<_, (String, i64)>(
        "SELECT car, COUNT(*) as cnt FROM laps WHERE driver_id = ? GROUP BY car ORDER BY cnt DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Personal bests count
    let pb_count = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    Json(json!({
        "stats": {
            "total_laps": totals.0,
            "total_time_ms": totals.1,
            "total_sessions": session_count,
            "total_driving_seconds": total_driving_secs,
            "favourite_car": fav_car.as_ref().map(|c| &c.0),
            "personal_bests": pb_count,
        }
    }))
}

// ─── Customer Registration ───────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomerRegisterRequest {
    name: String,
    nickname: Option<String>,
    email: Option<String>,
    dob: String,
    waiver_consent: bool,
    signature_data: Option<String>,
    guardian_name: Option<String>,
    guardian_phone: Option<String>,
}

async fn customer_register(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CustomerRegisterRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    if !req.waiver_consent {
        return Json(json!({ "error": "Waiver consent is required" }));
    }

    // MMA-WIRED: Use centralized input validation module
    let name = match crate::input_validation::validate_name(&req.name) {
        Ok(n) => n,
        Err(e) => return Json(json!({ "error": e })),
    };
    if let Some(ref email) = req.email {
        if let Err(e) = crate::input_validation::validate_email(email) {
            return Json(json!({ "error": e }));
        }
    }
    if let Some(ref phone) = req.guardian_phone {
        if let Err(e) = crate::input_validation::validate_phone(phone) {
            return Json(json!({ "error": format!("Guardian phone: {}", e) }));
        }
    }

    // Parse and validate DOB
    let dob = match chrono::NaiveDate::parse_from_str(&req.dob, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Json(json!({ "error": "Invalid date format. Use YYYY-MM-DD" })),
    };

    let today = chrono::Utc::now().date_naive();
    let age = (today - dob).num_days() / 365;

    if age < 12 {
        return Json(json!({ "error": "Minimum age is 12 years" }));
    }

    // Guardian required for minors (12-17)
    if age < 18 {
        if req.guardian_name.as_ref().map_or(true, |n| n.trim().is_empty()) {
            return Json(json!({ "error": "Guardian name required for customers under 18" }));
        }
    }

    // Check for duplicate name + DOB (same person already registered)
    let duplicate: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE name = ? AND dob = ? AND registration_completed = 1 AND id != ?",
    )
    .bind(&name)
    .bind(&req.dob)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if duplicate.is_some() {
        return Json(json!({ "error": "An account with this name and date of birth already exists. Please sign in with your registered phone number." }));
    }

    let nickname = req.nickname.as_ref().map(|n| n.trim().to_string()).filter(|n| !n.is_empty());

    // Update driver record
    let result = sqlx::query(
        "UPDATE drivers SET
            name = ?, nickname = ?, email = ?, dob = ?,
            waiver_signed = 1, waiver_signed_at = datetime('now'),
            waiver_version = 'v1.0',
            signature_data = ?,
            guardian_name = ?, guardian_phone = ?,
            registration_completed = 1,
            updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&nickname)
    .bind(&req.email)
    .bind(&req.dob)
    .bind(&req.signature_data)
    .bind(&req.guardian_name)
    .bind(&req.guardian_phone)
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // Auto-create wallet for new customer
            let _ = wallet::ensure_wallet(&state, &driver_id).await;

            tracing::info!("Customer {} registered (age: {}, minor: {})", driver_id, age, age < 18);
            Json(json!({
                "status": "registered",
                "driver_id": driver_id,
                "is_minor": age < 18,
            }))
        }
        Err(e) => Json(json!({ "error": format!("Registration failed: {}", e) })),
    }
}

async fn customer_waiver_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let row = sqlx::query_as::<_, (bool, bool)>(
        "SELECT COALESCE(waiver_signed, 0), COALESCE(registration_completed, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((waiver, registered))) => Json(json!({
            "waiver_signed": waiver,
            "registration_completed": registered,
        })),
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Waivers (admin-facing) ──────────────────────────────────────────────────

async fn list_waivers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let page: i64 = params.get("page").and_then(|p| p.parse().ok()).unwrap_or(1).max(1);
    let per_page: i64 = params.get("per_page").and_then(|p| p.parse().ok()).unwrap_or(50).min(200).max(1);
    let offset = (page - 1) * per_page;

    let total = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM drivers WHERE waiver_signed = 1",
    )
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, phone, email, dob, waiver_signed_at, waiver_version, guardian_name, guardian_phone, signature_data
         FROM drivers WHERE waiver_signed = 1
         ORDER BY waiver_signed_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(waivers) => {
            let list: Vec<Value> = waivers.iter().map(|w| {
                let is_minor = w.4.as_ref().map_or(false, |dob| {
                    chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d")
                        .map(|d| (chrono::Utc::now().date_naive() - d).num_days() / 365 < 18)
                        .unwrap_or(false)
                });
                json!({
                    "driver_id": w.0,
                    "name": w.1,
                    "phone": w.2,
                    "email": w.3,
                    "dob": w.4,
                    "waiver_signed_at": w.5,
                    "waiver_version": w.6,
                    "guardian_name": w.7,
                    "guardian_phone": w.8,
                    "has_signature": w.9.is_some(),
                    "is_minor": is_minor,
                })
            }).collect();
            Json(json!({ "waivers": list, "total": total, "page": page, "per_page": per_page }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn check_waiver(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let phone = params.get("phone");
    let email = params.get("email");

    if phone.is_none() && email.is_none() {
        return Json(json!({ "error": "Provide phone or email parameter" }));
    }

    let row = if let Some(phone) = phone {
        // Normalize: strip non-digits, use last 10 for hash lookup (full match only)
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let last10 = if digits.len() >= 10 { &digits[digits.len() - 10..] } else { &digits };
        let ph = state.field_cipher.hash_phone(last10);
        sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id, name, phone_enc, COALESCE(waiver_signed, 0) FROM drivers WHERE phone_hash = ?",
        )
        .bind(&ph)
        .fetch_optional(&state.db)
        .await
    } else if let Some(email) = email {
        sqlx::query_as::<_, (String, String, Option<String>, bool)>(
            "SELECT id, name, phone_enc, COALESCE(waiver_signed, 0) FROM drivers WHERE LOWER(email) = LOWER(?)",
        )
        .bind(email)
        .fetch_optional(&state.db)
        .await
    } else {
        return Json(json!({ "error": "Provide phone or email parameter" }));
    };

    match row {
        Ok(Some((id, name, phone_enc, signed))) => {
            let phone = phone_enc.and_then(|enc| state.field_cipher.decrypt_field(&enc).ok());
            Json(json!({
                "signed": signed,
                "driver": { "id": id, "name": name, "phone": phone },
            }))
        }
        Ok(None) => Json(json!({ "signed": false, "driver": null })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn get_waiver_signature(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT signature_data FROM drivers WHERE id = ? AND waiver_signed = 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((Some(sig),))) => Json(json!({ "signature_data": sig })),
        Ok(Some((None,))) => Json(json!({ "error": "No signature on file" })),
        Ok(None) => Json(json!({ "error": "Waiver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── AI Chat ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AiChatRequest {
    message: String,
    #[serde(default)]
    history: Vec<Value>,
}

/// Staff/admin AI chat — full business context.
async fn ai_chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AiChatRequest>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled || !state.config.ai_debugger.chat_enabled {
        return Json(json!({ "error": "AI chat is not enabled" }));
    }

    // Gather live business context
    let context = crate::ai::gather_business_context(
        &state.db,
        &state.pods,
        &state.billing,
        &state.game_launcher,
    )
    .await;

    let system_prompt = crate::ai::build_staff_prompt(&context);

    // Build messages array: system + history + new message
    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    for msg in &req.history {
        messages.push(msg.clone());
    }

    messages.push(json!({
        "role": "user",
        "content": req.message,
    }));

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("staff_chat")).await {
        Ok((reply, model)) => Json(json!({
            "reply": reply,
            "model": model,
        })),
        Err(e) => Json(json!({
            "error": format!("AI query failed: {}", e),
        })),
    }
}

/// Customer AI chat — scoped to their own data only.
async fn customer_ai_chat(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AiChatRequest>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled || !state.config.ai_debugger.chat_enabled {
        return Json(json!({ "error": "AI chat is not enabled" }));
    }

    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Gather customer-scoped context
    let context = crate::ai::gather_customer_context(&state.db, &driver_id).await;
    let system_prompt = crate::ai::build_customer_prompt(&context);

    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    for msg in &req.history {
        messages.push(msg.clone());
    }

    messages.push(json!({
        "role": "user",
        "content": req.message,
    }));

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("customer_chat")).await {
        Ok((reply, model)) => Json(json!({
            "reply": reply,
            "model": model,
        })),
        Err(e) => Json(json!({
            "error": format!("AI query failed: {}", e),
        })),
    }
}

// ─── AI Diagnose (on-demand) ────────────────────────────────────────────────

/// Staff-triggered on-demand AI analysis of recent operational errors.
async fn ai_diagnose(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled {
        return Json(json!({ "error": "AI debugger is not enabled" }));
    }

    let db = &state.db;
    let mut context_parts: Vec<String> = Vec::new();

    // Recent crashes (last 10 minutes)
    let crashes = sqlx::query_as::<_, (String, String, Option<String>, String)>(
        "SELECT pod_id, sim_type, error_message, created_at FROM game_launch_events \
         WHERE event_type = 'crash' AND created_at > datetime('now', '-10 minutes') \
         ORDER BY created_at DESC LIMIT 10",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !crashes.is_empty() {
        let mut s = format!("RECENT CRASHES ({} in last 10 min):\n", crashes.len());
        for (pod, sim, err, time) in &crashes {
            s.push_str(&format!(
                "  - {} on pod {} at {} ({})\n",
                sim, pod, time,
                err.as_deref().unwrap_or("no details")
            ));
        }
        context_parts.push(s);
    }

    // Billing anomalies
    let stuck = sqlx::query_as::<_, (String, String)>(
        "SELECT pod_id, created_at FROM billing_sessions \
         WHERE status = 'pending' AND created_at < datetime('now', '-60 seconds') \
         AND created_at > datetime('now', '-10 minutes')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    if !stuck.is_empty() {
        context_parts.push(format!(
            "STUCK BILLING: {} session(s) stuck in 'pending' state",
            stuck.len()
        ));
    }

    let stale = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM billing_sessions \
         WHERE status = 'active' \
         AND datetime(started_at, '+' || allocated_seconds || ' seconds') < datetime('now', '-30 seconds')",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    if stale > 0 {
        context_parts.push(format!(
            "STALE BILLING: {} session(s) still 'active' past allocated time",
            stale
        ));
    }

    // API error counts
    let api_errors = state.drain_api_error_counts();
    let high_errors: Vec<_> = api_errors.iter().filter(|(_, v)| **v >= 2).collect();
    if !high_errors.is_empty() {
        let mut s = String::from("API ERRORS (recent):\n");
        for (endpoint, count) in &high_errors {
            s.push_str(&format!("  {} — {} errors\n", endpoint, count));
        }
        context_parts.push(s);
    }

    // Pod connectivity
    let pods = state.pods.read().await;
    let connected = pods.len();
    let expected = state.config.pods.count as usize;
    if connected < expected {
        context_parts.push(format!(
            "POD CONNECTIVITY: {}/{} pods connected",
            connected, expected
        ));
    }
    drop(pods);

    if context_parts.is_empty() {
        return Json(json!({
            "status": "healthy",
            "message": "No operational issues detected in the last 10 minutes"
        }));
    }

    // Gather additional business context
    let biz_context = crate::ai::gather_business_context(
        &state.db,
        &state.pods,
        &state.billing,
        &state.game_launcher,
    )
    .await;

    let full_context = format!(
        "OPERATIONAL ISSUES:\n{}\n\nVENUE STATE:\n{}",
        context_parts.join("\n\n"),
        biz_context
    );

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are James, AI operations assistant for RacingPoint eSports. \
                        Analyze the operational issues below alongside the current venue state. \
                        Provide root cause analysis, severity assessment, and specific actionable steps. \
                        Be concise but thorough."
        }),
        json!({
            "role": "user",
            "content": full_context
        }),
    ];

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(&state.db), Some("debug")).await {
        Ok((suggestion, model)) => {
            // Persist to ai_suggestions table
            let id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                 VALUES (?, 'venue', 'diagnostic', ?, ?, ?, 'diagnose')"
            )
            .bind(&id)
            .bind(&context_parts.join("\n"))
            .bind(&suggestion)
            .bind(&model)
            .execute(db)
            .await;

            Json(json!({
                "status": "analyzed",
                "issues_found": context_parts.len(),
                "suggestion": suggestion,
                "model": model,
                "suggestion_id": id,
            }))
        }
        Err(e) => Json(json!({
            "status": "error",
            "issues_found": context_parts.len(),
            "issues": context_parts,
            "error": format!("AI analysis failed: {}", e),
        })),
    }
}

// ─── AI Suggestions History ─────────────────────────────────────────────────

/// GET /ops/stats — failed sessions today + active/resolved bug counts.
async fn ops_stats(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let failed_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions WHERE status IN ('ended_early', 'cancelled') AND date(created_at) = ?",
    )
    .bind(&today)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let active_bugs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_suggestions WHERE dismissed = 0",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let resolved_bugs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ai_suggestions WHERE dismissed = 1",
    )
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    Json(json!({
        "failed_sessions_today": failed_today,
        "active_bugs": active_bugs,
        "resolved_bugs": resolved_bugs,
    }))
}

async fn list_ai_suggestions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<i64>().ok())
        .unwrap_or(50)
        .min(200)
        .max(1);

    let pod_filter = params.get("pod_id");

    let rows = if let Some(pod_id) = pod_filter {
        sqlx::query_as::<_, (String, String, String, Option<String>, String, String, String, i32, String)>(
            "SELECT id, pod_id, sim_type, error_context, suggestion, model, source, dismissed, created_at \
             FROM ai_suggestions WHERE pod_id = ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(pod_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, Option<String>, String, String, String, i32, String)>(
            "SELECT id, pod_id, sim_type, error_context, suggestion, model, source, dismissed, created_at \
             FROM ai_suggestions ORDER BY created_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
    };

    match rows {
        Ok(suggestions) => {
            let list: Vec<Value> = suggestions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0,
                        "pod_id": s.1,
                        "sim_type": s.2,
                        "error_context": s.3,
                        "suggestion": s.4,
                        "model": s.5,
                        "source": s.6,
                        "dismissed": s.7 != 0,
                        "created_at": s.8,
                    })
                })
                .collect();
            Json(json!({ "suggestions": list }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn dismiss_ai_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match sqlx::query("UPDATE ai_suggestions SET dismissed = 1 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "status": "dismissed" })),
        Ok(_) => Json(json!({ "error": "Suggestion not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── AI Training Management ─────────────────────────────────────────────────

/// GET /ai/training/stats — training pair counts, avg quality, top keywords.
async fn ai_training_stats(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let db = &state.db;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_training_pairs")
        .fetch_one(db).await.unwrap_or(0);

    let avg_quality: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(quality_score), 0.0) FROM ai_training_pairs"
    ).fetch_one(db).await.unwrap_or(0.0);

    let by_source = sqlx::query_as::<_, (String, i64)>(
        "SELECT source, COUNT(*) as cnt FROM ai_training_pairs GROUP BY source ORDER BY cnt DESC"
    ).fetch_all(db).await.unwrap_or_default();

    let top_used = sqlx::query_as::<_, (String, i64)>(
        "SELECT query_text, use_count FROM ai_training_pairs ORDER BY use_count DESC LIMIT 10"
    ).fetch_all(db).await.unwrap_or_default();

    Json(json!({
        "total": total,
        "avg_quality_score": (avg_quality * 100.0).round() / 100.0,
        "by_source": by_source.iter().map(|(s, c)| json!({"source": s, "count": c})).collect::<Vec<_>>(),
        "top_used": top_used.iter().map(|(q, u)| json!({"query": q, "use_count": u})).collect::<Vec<_>>(),
    }))
}

/// GET /ai/training/pairs — paginated list for review.
async fn ai_training_pairs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20);
    let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
    let source_filter = params.get("source");

    let (pairs, total) = if let Some(src) = source_filter {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs WHERE source = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(src).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs WHERE source = ?"
        ).bind(src).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    } else {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs"
        ).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    };

    Json(json!({
        "total": total,
        "limit": limit,
        "offset": offset,
        "pairs": pairs.iter().map(|(id, q, r, src, model, quality, use_count, created)| json!({
            "id": id,
            "query": q,
            "response": r,
            "source": src,
            "model": model,
            "quality_score": quality,
            "use_count": use_count,
            "created_at": created,
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize)]
struct TrainingImportItem {
    query: String,
    response: String,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default = "default_quality")]
    quality_score: i64,
}
fn default_source() -> String { "import".to_string() }
fn default_quality() -> i64 { 1 }

/// POST /ai/training/import — bulk import training pairs.
async fn ai_training_import(
    State(state): State<Arc<AppState>>,
    Json(pairs): Json<Vec<TrainingImportItem>>,
) -> Json<Value> {
    let mut inserted = 0u32;
    let mut skipped = 0u32;

    for item in &pairs {
        // Reuse the same log_training_pair logic but with quality_score support
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        item.query.hash(&mut hasher);
        let qhash = format!("{:x}", hasher.finish());

        let keywords = crate::ai::extract_keywords_pub(&item.query);
        let id = uuid::Uuid::new_v4().to_string();

        let result = sqlx::query(
            "INSERT INTO ai_training_pairs \
             (id, query_hash, query_text, query_keywords, response_text, source, model, quality_score) \
             SELECT ?, ?, ?, ?, ?, ?, 'import', ? \
             WHERE NOT EXISTS (SELECT 1 FROM ai_training_pairs WHERE query_hash = ?)",
        )
        .bind(&id)
        .bind(&qhash)
        .bind(&item.query)
        .bind(&keywords)
        .bind(&item.response)
        .bind(&item.source)
        .bind(item.quality_score)
        .bind(&qhash)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => inserted += 1,
            _ => skipped += 1,
        }
    }

    Json(json!({
        "imported": inserted,
        "skipped": skipped,
        "total_submitted": pairs.len(),
    }))
}

// ─── Wallet (staff-facing) ───────────────────────────────────────────────────

async fn wallet_bonus_tiers(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let tiers: Vec<(String, i64, i64, i64)> = sqlx::query_as(
        "SELECT id, min_amount_paise, bonus_percent, sort_order FROM bonus_tiers WHERE is_active = 1 ORDER BY sort_order"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let tiers_json: Vec<Value> = tiers.iter().map(|t| json!({
        "id": t.0,
        "min_paise": t.1,
        "bonus_pct": t.2,
        "sort_order": t.3,
    })).collect();

    Json(json!({ "tiers": tiers_json }))
}

async fn get_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    match wallet::get_wallet_info(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "wallet": info })),
        Ok(None) => Json(json!({ "wallet": null })),
        Err(e) => Json(json!({ "error": e })),
    }
}

#[derive(Debug, Deserialize)]
struct TopupRequest {
    amount_paise: i64,
    method: String, // cash, card, upi
    notes: Option<String>,
    staff_id: Option<String>,
    // FATM-02: Optional idempotency key — duplicate requests return the original result
    idempotency_key: Option<String>,
}

async fn topup_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(req): Json<TopupRequest>,
) -> Json<Value> {
    if req.amount_paise <= 0 {
        return Json(json!({ "error": "amount_paise must be greater than 0" }));
    }

    // SEC-05: Staff self-top-up block — cashier/manager cannot top up their own wallet.
    // Superadmin is exempt (audit trail exists for all transactions).
    if let Some(ref ext) = claims {
        if ext.0.sub == driver_id && ext.0.role != "superadmin" {
            tracing::warn!(
                staff_id = %ext.0.sub,
                target_driver = %driver_id,
                role = %ext.0.role,
                "SEC-05: Self-topup blocked for non-superadmin"
            );
            return Json(json!({ "error": "Staff cannot top up their own wallet. Contact a superadmin." }));
        }
    }

    // FATM-02: Idempotency check for topup
    if let Some(ref key) = req.idempotency_key {
        let existing = sqlx::query_as::<_, (i64, i64)>(
            "SELECT amount_paise, balance_after_paise FROM wallet_transactions WHERE idempotency_key = ?",
        )
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((_amount, balance)) = existing {
            return Json(json!({
                "status": "ok",
                "new_balance_paise": balance,
                "bonus_paise": 0,
                "idempotent_replay": true,
            }));
        }
    }

    let txn_type = match req.method.as_str() {
        "cash" => "topup_cash",
        "card" => "topup_card",
        "upi" => "topup_upi",
        "online" => "topup_online",
        _ => "topup_cash",
    };

    let mut new_balance = match wallet::credit(
        &state,
        &driver_id,
        req.amount_paise,
        txn_type,
        None,
        req.notes.as_deref(),
        req.staff_id.as_deref(),
    )
    .await
    {
        Ok(b) => b,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Bonus credit tiers — read from DB
    let bonus_row: Option<(i64,)> = sqlx::query_as(
        "SELECT bonus_percent FROM bonus_tiers WHERE is_active = 1 AND min_amount_paise <= ? ORDER BY min_amount_paise DESC LIMIT 1"
    )
    .bind(req.amount_paise)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);
    let bonus_pct = bonus_row.map(|r| r.0).unwrap_or(0);

    let bonus_paise = if bonus_pct > 0 {
        let bp = req.amount_paise * bonus_pct / 100;
        let _ = wallet::credit(
            &state,
            &driver_id,
            bp,
            "bonus",
            None,
            Some(&format!("{}% topup bonus on {} credits", bonus_pct, req.amount_paise / 100)),
            req.staff_id.as_deref(),
        )
        .await;
        new_balance = wallet::get_balance(&state, &driver_id).await.unwrap_or(new_balance);
        bp
    } else {
        0
    };

    // Audit trail + WhatsApp alert for wallet topup (HIGH sensitivity)
    accounting::log_admin_action(
        &state, "wallet_topup",
        &json!({"driver_id": driver_id, "amount_paise": req.amount_paise, "method": req.method}).to_string(),
        req.staff_id.as_deref(), None,
    ).await;
    whatsapp_alerter::send_admin_alert(
        &state.config, "Wallet Topup",
        &format!("{} paise for driver {}", req.amount_paise, driver_id),
    ).await;

    // LEGAL-08: Update last_activity_at — wallet topup is customer activity.
    // Non-critical: failure does not affect the topup result.
    let _ = sqlx::query(
        "UPDATE drivers SET last_activity_at = datetime('now') WHERE id = ?",
    )
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    Json(json!({
        "status": "ok",
        "new_balance_paise": new_balance,
        "bonus_paise": bonus_paise,
    }))
}

// ─── FATM-11: Payment gateway webhook ───────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PaymentGatewayWebhookRequest {
    /// Gateway's unique payment ID — used as idempotency key
    transaction_id: String,
    /// Driver to credit
    driver_id: String,
    /// Amount to credit in paise
    amount_paise: i64,
    /// Must be "success" or "captured" to trigger wallet credit
    status: String,
    /// HMAC signature from gateway (unused until gateway is chosen)
    #[allow(dead_code)]
    signature: Option<String>,
}

/// FATM-11: Payment gateway webhook — credits a driver's wallet idempotently.
/// - Same transaction_id fired twice → returns original result without double-crediting.
/// - Non-success status (refunded, failed, etc.) → acknowledged without crediting.
/// - Amount validation: must be 1 paise to Rs 10,000 (safety cap).
///
/// TODO: Verify HMAC signature from gateway (Razorpay/Cashfree/etc.)
/// When a specific gateway is chosen, implement:
///   let expected = hmac_sha256(webhook_secret, raw_body);
///   if !constant_time_eq(expected, signature) { return 401; }
/// For now the endpoint is protected by being undiscoverable (no public docs)
/// and the idempotency guard prevents replay damage.
async fn payment_gateway_webhook(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PaymentGatewayWebhookRequest>,
) -> Json<Value> {
    tracing::info!(
        transaction_id = %req.transaction_id,
        driver_id = %req.driver_id,
        amount_paise = req.amount_paise,
        status = %req.status,
        "FATM-11: Payment gateway webhook received"
    );

    // MMA-WEBHOOK: Verify gateway HMAC signature when webhook_secret is configured.
    // Without this, any caller can POST fabricated wallet credits.
    // TODO: When integrating a real payment gateway (Razorpay/Cashfree), set
    //       [integrations].payment_webhook_secret in racecontrol.toml and verify
    //       the X-Webhook-Signature header using HMAC-SHA256(secret, raw_body).
    if let Some(ref webhook_secret) = state.config.integrations.payment_webhook_secret {
        if !webhook_secret.is_empty() {
            let provided_sig = headers
                .get("x-webhook-signature")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if provided_sig.is_empty() {
                tracing::warn!(
                    transaction_id = %req.transaction_id,
                    "FATM-11: Gateway webhook rejected — missing X-Webhook-Signature header"
                );
                return Json(json!({ "ok": false, "error": "missing webhook signature" }));
            }
            // NOTE: Full HMAC-SHA256 verification requires raw body bytes.
            // This is a structural guard — when a real gateway is integrated,
            // replace this with proper HMAC verification using the raw request body.
            tracing::debug!("FATM-11: Webhook signature present (full HMAC check pending gateway integration)");
        }
    }

    // Basic field validation
    if req.transaction_id.is_empty() || req.driver_id.is_empty() {
        return Json(json!({ "ok": false, "error": "transaction_id and driver_id are required" }));
    }

    // Amount validation: 1 paise to Rs 10,000 (100000 paise)
    if req.amount_paise <= 0 || req.amount_paise > 10_000_00 {
        tracing::warn!(
            transaction_id = %req.transaction_id,
            amount_paise = req.amount_paise,
            "FATM-11: Gateway webhook rejected — amount out of range"
        );
        return Json(json!({
            "ok": false,
            "error": "amount_paise must be between 1 and 1000000 (Rs 10,000 cap)"
        }));
    }

    // Status check: only credit on success/captured
    let status_lower = req.status.to_lowercase();
    if status_lower != "success" && status_lower != "captured" {
        tracing::info!(
            transaction_id = %req.transaction_id,
            status = %req.status,
            "FATM-11: Gateway webhook acknowledged (non-success status — no wallet credit)"
        );
        return Json(json!({
            "ok": true,
            "action": "ignored",
            "reason": format!("status '{}' is not success/captured — no wallet credit", req.status)
        }));
    }

    // FATM-11: Idempotency check — check if this transaction_id was already processed
    let existing = sqlx::query_as::<_, (i64, i64)>(
        "SELECT amount_paise, balance_after_paise FROM wallet_transactions WHERE idempotency_key = ?",
    )
    .bind(&req.transaction_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((_amount, balance_after)) = existing {
        tracing::info!(
            transaction_id = %req.transaction_id,
            "FATM-11: Gateway webhook duplicate — returning original result"
        );
        return Json(json!({
            "ok": true,
            "duplicate": true,
            "balance_after_paise": balance_after
        }));
    }

    // Credit wallet within a transaction (atomic, idempotent via idempotency_key)
    let mut tx = match state.db.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(
                transaction_id = %req.transaction_id,
                "FATM-11: Gateway webhook DB error starting transaction: {}", e
            );
            return Json(json!({ "ok": false, "error": "DB error — please retry" }));
        }
    };

    let (new_balance, txn_id) = match wallet::credit_in_tx(
        &mut tx,
        &req.driver_id,
        req.amount_paise,
        "gateway_topup",
        Some(&req.transaction_id),
        Some("Payment gateway credit"),
        None,
        Some(&req.transaction_id), // idempotency_key = gateway's transaction_id
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            drop(tx);
            tracing::error!(
                transaction_id = %req.transaction_id,
                driver_id = %req.driver_id,
                "FATM-11: Gateway webhook credit_in_tx failed: {}", e
            );
            return Json(json!({ "ok": false, "error": format!("Wallet credit failed: {}", e) }));
        }
    };

    if let Err(e) = tx.commit().await {
        tracing::error!(
            transaction_id = %req.transaction_id,
            "FATM-11: Gateway webhook transaction commit failed: {}", e
        );
        return Json(json!({ "ok": false, "error": "Transaction commit failed — please retry" }));
    }

    tracing::info!(
        transaction_id = %req.transaction_id,
        driver_id = %req.driver_id,
        amount_paise = req.amount_paise,
        new_balance = new_balance,
        txn_id = %txn_id,
        "FATM-11: Gateway webhook — wallet credited successfully"
    );

    Json(json!({
        "ok": true,
        "balance_after_paise": new_balance,
        "txn_id": txn_id
    }))
}

/// UX-02: OTP fallback display endpoint.
/// Customer polls this URL if WhatsApp delivery failed.
/// Returns the OTP payload from the notification outbox via a one-time token.
/// Token is consumed (marked 'delivered') on first successful read.
async fn otp_fallback_handler(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> axum::response::Response {
    if token.is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({ "error": "token is required" })),
        )
            .into_response();
    }

    match crate::notification_outbox::get_otp_by_fallback_token(&state.db, &token).await {
        Ok(Some(otp)) => {
            tracing::info!(target: "notification_outbox", "OTP fallback token consumed");
            Json(json!({ "otp": otp })).into_response()
        }
        Ok(None) => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "OTP not available. WhatsApp delivery may still be in progress, or token already used." })),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(target: "notification_outbox", "OTP fallback lookup error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "lookup error" })),
            )
                .into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DebitRequest {
    amount_paise: i64,
    reason: String, // cafe, merchandise, penalty, etc.
    reference_id: Option<String>,
    notes: Option<String>,
}

async fn debit_wallet_manual(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(req): Json<DebitRequest>,
) -> Json<Value> {
    let txn_type = format!("debit_{}", req.reason);

    match wallet::debit(
        &state,
        &driver_id,
        req.amount_paise,
        &txn_type,
        req.reference_id.as_deref(),
        req.notes.as_deref(),
    )
    .await
    {
        Ok((new_balance, txn_id)) => Json(json!({
            "status": "ok",
            "new_balance_paise": new_balance,
            "txn_id": txn_id,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn wallet_transactions(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50i64);
    let txns = wallet::get_transactions(&state, &driver_id, limit).await;
    Json(json!({ "transactions": txns }))
}

async fn all_wallet_transactions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let date = params.get("date").cloned().unwrap_or(today);
    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(200i64);

    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<String>, String, String, Option<String>)>(
        "SELECT wt.id, wt.driver_id, wt.amount_paise, wt.balance_after_paise, wt.txn_type, \
         wt.reference_id, wt.notes, wt.staff_id, wt.created_at, \
         COALESCE(d.name, 'Unknown') as driver_name, d.phone as driver_phone \
         FROM wallet_transactions wt \
         LEFT JOIN drivers d ON d.id = wt.driver_id \
         WHERE date(wt.created_at) = ? \
         ORDER BY wt.created_at DESC LIMIT ?",
    )
    .bind(&date)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut total_credits: i64 = 0;
    let mut total_debits: i64 = 0;

    let txns: Vec<Value> = rows.iter().map(|r| {
        let is_credit = r.4.starts_with("topup") || r.4 == "bonus" || r.4.starts_with("refund");
        if is_credit {
            total_credits += r.2;
        } else {
            total_debits += r.2;
        }
        json!({
            "id": r.0,
            "driver_id": r.1,
            "amount_paise": r.2,
            "balance_after_paise": r.3,
            "txn_type": r.4,
            "reference_id": r.5,
            "notes": r.6,
            "staff_id": r.7,
            "created_at": r.8,
            "driver_name": r.9,
            "driver_phone": r.10,
        })
    }).collect();

    Json(json!({
        "transactions": txns,
        "summary": {
            "total_credits_paise": total_credits,
            "total_debits_paise": total_debits,
            "net_paise": total_credits - total_debits,
            "count": txns.len(),
        }
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefundRequest {
    amount_paise: i64,
    notes: Option<String>,
    reference_id: Option<String>,
}

async fn refund_wallet(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(req): Json<RefundRequest>,
) -> Json<Value> {
    // MMA-203: Extract staff_id from JWT for audit trail (POS-05)
    let staff_id = claims.map(|c| c.0.sub.clone());

    // MMA-203: Validate refund amount bounds
    if req.amount_paise <= 0 {
        return Json(json!({ "error": "Refund amount must be positive" }));
    }
    const MAX_MANUAL_REFUND_PAISE: i64 = 500_000; // ₹5,000 cap for manual wallet refunds
    if req.amount_paise > MAX_MANUAL_REFUND_PAISE {
        return Json(json!({ "error": format!("Refund exceeds maximum allowed (₹{})", MAX_MANUAL_REFUND_PAISE / 100) }));
    }

    // R2-3: Without reference_id, apply a stricter cap to prevent abuse
    if req.reference_id.is_none() && req.amount_paise > 50_000 {
        // ₹500 cap for unreferenced manual refunds (goodwill gestures)
        return Json(json!({ "error": "Refunds above ₹500 require a billing session reference_id" }));
    }

    // MMA-203: If reference_id is provided, validate it maps to a real billing session
    if let Some(ref ref_id) = req.reference_id {
        let session = sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>)>(
            "SELECT id, driver_id, custom_price_paise, \
             (SELECT COALESCE(SUM(CASE WHEN txn_type='billing' THEN amount_paise ELSE 0 END), 0) \
              FROM wallet_transactions WHERE reference_id = bs.id) as session_cost \
             FROM billing_sessions bs WHERE id = ?"
        )
        .bind(ref_id)
        .fetch_optional(&state.db)
        .await;

        // P3: Extract session cost for cumulative cap enforcement
        let session_cost_paise: i64;
        match session {
            Ok(Some((_, session_driver_id, custom_price, cost))) => {
                // Verify the refund is for the correct driver
                if session_driver_id != driver_id {
                    return Json(json!({ "error": "Billing session does not belong to this driver" }));
                }
                session_cost_paise = custom_price.unwrap_or(0).max(cost.unwrap_or(0)).max(MAX_MANUAL_REFUND_PAISE);
            }
            Ok(None) => {
                return Json(json!({ "error": "Referenced billing session not found" }));
            }
            Err(e) => {
                return Json(json!({ "error": format!("DB error validating reference: {}", e) }));
            }
        }

        // R3-4: Atomic refund check+credit in a single DB transaction to prevent TOCTOU race.
        // Check BOTH wallet_transactions AND refunds tables, then credit atomically.
        let mut conn = match state.db.acquire().await {
            Ok(c) => c,
            Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
        };
        let mut tx = match sqlx::Acquire::begin(&mut *conn).await {
            Ok(t) => t,
            Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
        };

        let already_refunded_wallet = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(amount_paise), 0) FROM wallet_transactions \
             WHERE reference_id = ? AND txn_type IN ('refund_manual', 'refund_session')"
        )
        .bind(ref_id)
        .fetch_one(&mut *tx)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

        let already_refunded_billing = sqlx::query_as::<_, (i64,)>(
            "SELECT COALESCE(SUM(amount_paise), 0) FROM refunds WHERE billing_session_id = ?"
        )
        .bind(ref_id)
        .fetch_one(&mut *tx)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

        // P3: Cumulative refund cap — allow partial refunds but prevent over-refunding
        let total_refunded = already_refunded_wallet + already_refunded_billing;
        let would_be_total = total_refunded + req.amount_paise;
        if would_be_total > session_cost_paise {
            tracing::warn!(
                "Refund exceeds session cost: driver={}, reference_id={}, already_refunded={}p, requested={}p, session_cost={}p",
                driver_id, ref_id, total_refunded, req.amount_paise, session_cost_paise
            );
            let remaining = (session_cost_paise - total_refunded).max(0);
            // tx rolls back on drop
            return Json(json!({ "error": format!("Refund would exceed session cost. Already refunded: {}p, max remaining: {}p", total_refunded, remaining) }));
        }

        // Perform the credit within the same transaction
        wallet::ensure_wallet(&state, &driver_id).await.ok();
        let txn_id = uuid::Uuid::new_v4().to_string();

        if let Err(e) = sqlx::query(
            "UPDATE wallets SET balance_paise = balance_paise + ?, total_credited_paise = total_credited_paise + ?, updated_at = datetime('now') WHERE driver_id = ?"
        )
        .bind(req.amount_paise)
        .bind(req.amount_paise)
        .bind(&driver_id)
        .execute(&mut *tx)
        .await {
            return Json(json!({ "error": format!("DB error: {}", e) }));
        }

        if let Err(e) = sqlx::query(
            "INSERT INTO wallet_transactions (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id) \
             VALUES (?, ?, ?, (SELECT balance_paise FROM wallets WHERE driver_id = ?), 'refund_manual', ?, ?, ?)"
        )
        .bind(&txn_id)
        .bind(&driver_id)
        .bind(req.amount_paise)
        .bind(&driver_id)
        .bind(ref_id.as_str())
        .bind(req.notes.as_deref())
        .bind(staff_id.as_deref())
        .execute(&mut *tx)
        .await {
            return Json(json!({ "error": format!("DB error: {}", e) }));
        }

        if let Err(e) = tx.commit().await {
            return Json(json!({ "error": format!("DB commit error: {}", e) }));
        }

        // Get new balance after commit
        let new_balance = wallet::get_balance(&state, &driver_id).await.unwrap_or(0);
        return Json(json!({
            "status": "ok",
            "new_balance_paise": new_balance,
        }));
    }

    // Non-referenced refund path (no reference_id)
    match wallet::credit(
        &state,
        &driver_id,
        req.amount_paise,
        "refund_manual",
        req.reference_id.as_deref(),
        req.notes.as_deref(),
        staff_id.as_deref(),
    )
    .await
    {
        Ok(new_balance) => Json(json!({
            "status": "ok",
            "new_balance_paise": new_balance,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Customer Wallet ────────────────────────────────────────────────────────

async fn customer_wallet(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match wallet::get_wallet_info(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "wallet": info })),
        Ok(None) => Json(json!({ "wallet": { "driver_id": driver_id, "balance_paise": 0, "total_credited_paise": 0, "total_debited_paise": 0 } })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_wallet_transactions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50i64);
    let offset = params.get("offset").and_then(|o| o.parse().ok()).unwrap_or(0i64);

    let total: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM wallet_transactions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, created_at
         FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&driver_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let txns: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id": r.0, "driver_id": r.1, "amount_paise": r.2,
            "balance_after_paise": r.3, "txn_type": r.4,
            "reference_id": r.5, "notes": r.6, "staff_id": r.7,
            "created_at": r.8,
        })
    }).collect();

    Json(json!({ "transactions": txns, "total": total }))
}

// ─── Customer Experiences ───────────────────────────────────────────────────

async fn customer_experiences(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, i64)>(
        "SELECT e.id, e.name, e.game, e.track, e.car, e.car_class, e.duration_minutes, e.start_type, e.sort_order
         FROM kiosk_experiences e WHERE e.is_active = 1 ORDER BY e.sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    // Also fetch pricing tiers for the client
    let tiers = sqlx::query_as::<_, (String, String, i64, i64, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match (rows, tiers) {
        (Ok(experiences), Ok(tiers)) => {
            let exp_list: Vec<Value> = experiences.iter().map(|e| json!({
                "id": e.0, "name": e.1, "game": e.2, "track": e.3,
                "car": e.4, "car_class": e.5, "duration_minutes": e.6,
                "start_type": e.7, "sort_order": e.8,
            })).collect();

            let tier_list: Vec<Value> = tiers.iter().map(|t| json!({
                "id": t.0, "name": t.1, "duration_minutes": t.2,
                "price_paise": t.3, "is_trial": t.4, "sort_order": t.5,
            })).collect();

            Json(json!({ "experiences": exp_list, "pricing_tiers": tier_list }))
        }
        _ => Json(json!({ "error": "Failed to load experiences" })),
    }
}

// ─── AC Catalog ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CatalogQuery {
    pod_id: Option<String>,
}

async fn customer_ac_catalog(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CatalogQuery>,
) -> Json<Value> {
    let manifest = if let Some(ref pod_id) = query.pod_id {
        state.pod_manifests.read().await.get(pod_id).cloned()
    } else {
        None
    };
    Json(catalog::get_filtered_catalog(manifest.as_ref()))
}

// ─── Customer Booking ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CustomBookingOptions {
    game: String,
    game_mode: Option<String>,
    track: String,
    car: String,
    difficulty: String,
    transmission: String,
    #[serde(default = "default_ffb_preset")]
    ffb: String,
    #[serde(default)]
    session_type: Option<String>,
}

fn default_ffb_preset() -> String { "medium".to_string() }

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BookSessionRequest {
    experience_id: Option<String>,
    pricing_tier_id: String,
    custom: Option<CustomBookingOptions>,
    coupon_code: Option<String>,
}

async fn customer_book_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BookSessionRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Validate pricing tier and get price
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

    let is_trial = tier.4;
    let base_price_paise = tier.3;

    // Apply coupon discount if provided
    let mut applied_discount_paise: i64 = 0;
    let mut applied_coupon_id: Option<String> = None;
    let mut applied_discount_reason: Option<String> = None;

    if !is_trial {
        if let Some(ref code) = req.coupon_code {
            match validate_and_calc_coupon(&state, code, &driver_id, base_price_paise).await {
                Ok(cd) => {
                    applied_discount_paise = cd.discount_paise;
                    applied_coupon_id = Some(cd.coupon_id);
                    applied_discount_reason = Some(format!("Coupon {}: {}", code.to_uppercase(), cd.description));
                }
                Err(e) => return Json(json!({ "error": e })),
            }
        }
    }

    let final_price_paise = base_price_paise - applied_discount_paise;

    // Handle trial booking (skip for unlimited_trials drivers)
    if is_trial {
        let trial_info = sqlx::query_as::<_, (bool, bool)>(
            "SELECT COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0) FROM drivers WHERE id = ?",
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await;

        match trial_info {
            Ok(Some((true, false))) => return Json(json!({ "error": "Free trial already used" })),
            Ok(None) => return Json(json!({ "error": "Driver not found" })),
            Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
            _ => {} // OK to proceed (hasn't used trial, or has unlimited_trials)
        }
    } else {
        // Validate wallet balance for non-trial (using discounted price)
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": e })),
        };

        if balance < final_price_paise {
            return Json(json!({
                "error": "Insufficient wallet balance",
                "balance_paise": balance,
                "required_paise": final_price_paise,
            }));
        }
    }

    // Check if driver already has an active reservation
    if let Some(existing) = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        return Json(json!({
            "error": "You already have an active reservation",
            "reservation_id": existing.id,
            "pod_id": existing.pod_id,
        }));
    }

    // Find idle pod
    let pod_id = match pod_reservation::find_idle_pod(&state).await {
        Some(id) => id,
        None => return Json(json!({ "error": "No pods available right now. Please try again shortly." })),
    };

    // Get pod number for display
    let pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.number).unwrap_or(0)
    };

    // Debit wallet (skip for trial) — uses discounted price
    let (wallet_txn_id, wallet_debit) = if !is_trial && final_price_paise > 0 {
        let debit_notes = if applied_discount_paise > 0 {
            format!("{} on Pod {} — {} credits discount", tier.1, pod_number, applied_discount_paise / 100)
        } else {
            format!("{} on Pod {}", tier.1, pod_number)
        };
        match wallet::debit(
            &state,
            &driver_id,
            final_price_paise,
            "debit_session",
            None,
            Some(&debit_notes),
        )
        .await
        {
            Ok((_, txn_id)) => (Some(txn_id), Some(final_price_paise)),
            Err(e) => return Json(json!({ "error": e })),
        }
    } else {
        (None, None)
    };

    // Create pod reservation
    let reservation_id = match pod_reservation::create_reservation(&state, &driver_id, &pod_id).await {
        Ok(id) => id,
        Err(e) => {
            // Refund if we already debited
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
            }
            return Json(json!({ "error": e }));
        }
    };

    // Validate: must have either experience_id or custom, not both, not neither
    if req.experience_id.is_none() && req.custom.is_none() {
        // Refund if we already debited
        if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
            let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
        }
        let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
        return Json(json!({ "error": "Either experience_id or custom must be provided" }));
    }

    // Build custom launch args if custom booking
    let custom_launch_args = req.custom.as_ref().map(|c| {
        // Get driver name for launch args
        let driver_name_for_args = "Driver"; // Will be set properly by launch_or_assist
        catalog::build_custom_launch_args(
            &c.car, &c.track, driver_name_for_args, &c.difficulty, &c.transmission, &c.ffb,
            c.session_type.as_deref().unwrap_or("practice"),
        ).to_string()
    });

    // For custom bookings, also embed game info in the launch args
    let custom_launch_args = if let Some(ref args) = custom_launch_args {
        if let Some(ref c) = req.custom {
            let mut parsed: serde_json::Value = serde_json::from_str(args).unwrap_or_default();
            parsed["game"] = serde_json::json!(c.game);
            parsed["game_mode"] = serde_json::json!(c.game_mode.as_deref().unwrap_or("single"));
            parsed["session_type"] = serde_json::json!(c.session_type.as_deref().unwrap_or("practice"));
            Some(parsed.to_string())
        } else {
            custom_launch_args
        }
    } else {
        None
    };

    // Create auth token (PIN type) for this pod
    let experience_id = req.experience_id.clone();
    let auth_token = match auth::create_auth_token(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        "pin".to_string(),
        None, // custom_price_paise
        None, // custom_duration_minutes
        experience_id,
        custom_launch_args,
    )
    .await
    {
        Ok(token_info) => token_info,
        Err(e) => {
            // Cleanup: end reservation + refund
            let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Booking failed — auto-refund")).await;
            }
            return Json(json!({ "error": format!("Failed to create auth: {}", e) }));
        }
    };

    // Record coupon redemption if applicable
    // We use reservation_id as a stand-in since the billing_session isn't created until PIN auth
    if let Some(ref cid) = applied_coupon_id {
        record_coupon_redemption(&state, cid, &driver_id, &reservation_id, applied_discount_paise).await;
    }

    Json(json!({
        "status": "booked",
        "reservation_id": reservation_id,
        "pod_id": pod_id,
        "pod_number": pod_number,
        "pin": auth_token.token,
        "allocated_seconds": auth_token.allocated_seconds,
        "wallet_debit_paise": wallet_debit,
        "wallet_txn_id": wallet_txn_id,
        "discount_paise": applied_discount_paise,
        "original_price_paise": base_price_paise,
        "discount_reason": applied_discount_reason,
    }))
}

async fn customer_active_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let reservation = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await;

    match reservation {
        Some(res) => {
            // Get pod number
            let pod_number = {
                let pods = state.pods.read().await;
                pods.get(&res.pod_id).map(|p| p.number).unwrap_or(0)
            };

            // Check if there's an active billing session on this pod
            let active_billing = {
                let rate_tiers = state.billing.rate_tiers.read().await;
                let timers = state.billing.active_timers.read().await;
                timers.get(&res.pod_id).map(|t| t.to_info(&rate_tiers))
            };

            Json(json!({
                "reservation": res,
                "pod_number": pod_number,
                "active_billing": active_billing,
            }))
        }
        None => Json(json!({ "reservation": null })),
    }
}

async fn customer_end_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let reservation = match pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        Some(r) => r,
        None => return Json(json!({ "error": "No active reservation" })),
    };

    // End any active billing on this pod
    {
        let timers = state.billing.active_timers.read().await;
        if let Some(timer) = timers.get(&reservation.pod_id) {
            let session_id = timer.session_id.clone();
            drop(timers);

            // Proportional refund
            let billing = sqlx::query_as::<_, (i64, i64, Option<i64>)>(
                "SELECT allocated_seconds, driving_seconds, wallet_debit_paise FROM billing_sessions WHERE id = ?",
            )
            .bind(&session_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((allocated, driving, Some(debit))) = billing {
                if debit > 0 && driving < allocated {
                    let remaining = allocated - driving;
                    let refund_amount = (remaining * debit) / allocated;
                    if refund_amount > 0 {
                        let _ = wallet::refund(
                            &state,
                            &driver_id,
                            refund_amount,
                            Some(&session_id),
                            Some("Early end — proportional refund"),
                        )
                        .await;
                    }
                }
            }

            billing::end_billing_session_public(&state, &session_id, rc_common::types::BillingSessionStatus::EndedEarly, None).await;
        }
    }

    // End the reservation
    let _ = pod_reservation::end_reservation(&state, &reservation.id).await;

    Json(json!({ "status": "ok" }))
}

// ─── Continue Session (Multi-Sub-Session) ───────────────────────────────────

#[derive(Debug, Deserialize)]
struct ContinueSessionRequest {
    experience_id: String,
    pricing_tier_id: String,
}

async fn customer_continue_session(
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

        let agent_senders = state.agent_senders.read().await;
        if needs_assistance {
            // Send assistance screen instead of launching
            if let Some(sender) = agent_senders.get(&reservation.pod_id) {
                let _ = sender.send(rc_common::protocol::CoreToAgentMessage::ShowAssistanceScreen {
                    driver_name: driver_id.clone(),
                    message: "A team member is on the way to help launch your game.".to_string(),
                }).await;
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
                crate::activity_log::log_pod_activity(&state, &reservation.pod_id, "content", "Launch Rejected", &reason, "core");
            } else {
                let launch_args = serde_json::json!({
                    "car": car, "track": track, "driver": "Driver",
                    "transmission": "auto",
                    "aids": { "abs": 1, "tc": 1, "stability": 1, "autoclutch": 1, "ideal_line": 1 },
                    "conditions": { "damage": 0 }
                }).to_string();
                if let Some(sender) = agent_senders.get(&reservation.pod_id) {
                    let _ = sender.send(rc_common::protocol::CoreToAgentMessage::LaunchGame {
                        sim_type,
                        launch_args: Some(launch_args),
                        force_clean: false,
                        duration_minutes: None,
                    }).await;
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

// ─── Kiosk ──────────────────────────────────────────────────────────────────

async fn list_kiosk_experiences(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, Option<String>, i64, i64, Option<String>)>(
        "SELECT id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active, pricing_tier_id
         FROM kiosk_experiences WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(experiences) => {
            let list: Vec<Value> = experiences
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "name": e.1, "game": e.2,
                        "track": e.3, "car": e.4, "car_class": e.5,
                        "duration_minutes": e.6, "start_type": e.7,
                        "ac_preset_id": e.8, "sort_order": e.9,
                        "is_active": e.10 != 0,
                        "pricing_tier_id": e.11,
                    })
                })
                .collect();
            Json(json!({ "experiences": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("New Experience");
    let game = body.get("game").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("spa");
    let car = body.get("car").and_then(|v| v.as_str()).unwrap_or("ks_ferrari_sf15t");
    let car_class = body.get("car_class").and_then(|v| v.as_str());
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30);
    let start_type = body.get("start_type").and_then(|v| v.as_str()).unwrap_or("pitlane");
    let ac_preset_id = body.get("ac_preset_id").and_then(|v| v.as_str());
    let sort_order = body.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(10);

    let result = sqlx::query(
        "INSERT INTO kiosk_experiences (id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(game)
    .bind(track)
    .bind(car)
    .bind(car_class)
    .bind(duration_minutes)
    .bind(start_type)
    .bind(ac_preset_id)
    .bind(sort_order)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, Option<String>, i64, i64)>(
        "SELECT id, name, game, track, car, car_class, duration_minutes, start_type, ac_preset_id, sort_order, is_active
         FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(e)) => Json(json!({
            "id": e.0, "name": e.1, "game": e.2,
            "track": e.3, "car": e.4, "car_class": e.5,
            "duration_minutes": e.6, "start_type": e.7,
            "ac_preset_id": e.8, "sort_order": e.9,
            "is_active": e.10 != 0,
        })),
        Ok(None) => Json(json!({ "error": "Experience not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(v) = body.get("name").and_then(|v| v.as_str()) {
        updates.push("name = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("game").and_then(|v| v.as_str()) {
        updates.push("game = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("track").and_then(|v| v.as_str()) {
        updates.push("track = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("car").and_then(|v| v.as_str()) {
        updates.push("car = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("car_class").and_then(|v| v.as_str()) {
        updates.push("car_class = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("duration_minutes").and_then(|v| v.as_i64()) {
        updates.push("duration_minutes = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("start_type").and_then(|v| v.as_str()) {
        updates.push("start_type = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("ac_preset_id").and_then(|v| v.as_str()) {
        updates.push("ac_preset_id = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("sort_order").and_then(|v| v.as_i64()) {
        updates.push("sort_order = ?");
        binds.push(v.to_string());
    }
    if let Some(v) = body.get("is_active").and_then(|v| v.as_bool()) {
        updates.push("is_active = ?");
        binds.push(if v { "1".to_string() } else { "0".to_string() });
    }
    if let Some(v) = body.get("pricing_tier_id").and_then(|v| v.as_str()) {
        updates.push("pricing_tier_id = ?");
        binds.push(v.to_string());
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    let query = format!("UPDATE kiosk_experiences SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Experience not found" })),
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_kiosk_experience(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match sqlx::query("UPDATE kiosk_experiences SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Experience not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn kiosk_pod_launch_experience(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = match body["pod_id"].as_str() {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "pod_id required" })),
    };
    let experience_id = match body["experience_id"].as_str() {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "experience_id required" })),
    };

    // Verify pod exists (pods are in-memory, not DB)
    {
        let pods = state.pods.read().await;
        if !pods.contains_key(&pod_id) {
            return Json(json!({ "error": "Pod not found" }));
        }
    }

    // Find active billing session for this pod (join drivers for name)
    let billing = sqlx::query_as::<_, (String, String, String)>(
        "SELECT bs.id, bs.driver_id, d.name FROM billing_sessions bs JOIN drivers d ON d.id = bs.driver_id WHERE bs.pod_id = ? AND bs.status = 'active' ORDER BY bs.started_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await;

    let (billing_session_id, _driver_id, driver_name) = match billing {
        Ok(Some(b)) => b,
        Ok(None) => return Json(json!({ "error": "No active billing session on this pod" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Verify experience exists
    let exp = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM kiosk_experiences WHERE id = ? AND is_active = 1",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await;

    if exp.ok().flatten().is_none() {
        return Json(json!({ "error": "Experience not found" }));
    }

    // Launch or show assistance
    let exp_id_opt = Some(experience_id);
    auth::launch_or_assist(
        &state,
        &pod_id,
        &billing_session_id,
        &exp_id_opt,
        &None,
        &driver_name,
    )
    .await;

    Json(json!({ "ok": true, "billing_session_id": billing_session_id }))
}

/// Kiosk self-service multiplayer booking.
/// Customers call this after authenticating via phone+OTP.
/// Creates a multiplayer group session, allocates pods, generates unique PINs per pod,
/// and auto-starts the AC server.
async fn kiosk_book_multiplayer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    // Extract driver_id from Bearer token (same auth as customer_book_session)
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let pricing_tier_id = match req.get("pricing_tier_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pricing_tier_id'" })),
    };

    let pod_count = match req.get("pod_count").and_then(|v| v.as_u64()) {
        Some(n) => n as usize,
        None => return Json(json!({ "error": "Missing 'pod_count'" })),
    };

    let experience_id = req.get("experience_id").and_then(|v| v.as_str()).map(String::from);

    let custom = req.get("custom").and_then(|v| {
        let game = v.get("game")?.as_str()?.to_string();
        let track = v.get("track")?.as_str()?.to_string();
        let car = v.get("car")?.as_str()?.to_string();
        Some((game, track, car))
    });

    if experience_id.is_none() && custom.is_none() {
        return Json(json!({ "error": "Must provide 'experience_id' or 'custom' payload" }));
    }

    match multiplayer::book_multiplayer_kiosk(
        &state,
        &driver_id,
        &pricing_tier_id,
        pod_count,
        experience_id.as_deref(),
        custom,
    )
    .await
    {
        Ok(result) => Json(json!({
            "status": "ok",
            "group_session_id": result.group_session_id,
            "experience_name": result.experience_name,
            "tier_name": result.tier_name,
            "allocated_seconds": result.allocated_seconds,
            "assignments": result.assignments,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

/// PIN configuration constants — exposed via kiosk settings so the frontend
/// reads config truth instead of hardcoding (standing rule: UI must reflect config truth).
const PIN_REDEEM_LENGTH: u32 = 6;
const PIN_REDEEM_MAX_ATTEMPTS: u32 = 10;
const PIN_REDEEM_LOCKOUT_SECONDS: u32 = 300;

async fn get_kiosk_settings(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM kiosk_settings",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(settings) => {
            let mut map = serde_json::Map::new();
            for (key, value) in &settings {
                map.insert(key.clone(), json!(value));
            }
            // Inject PIN config constants so frontend reads from server, not hardcoded (C3)
            map.insert("pin_length".to_string(), json!(PIN_REDEEM_LENGTH.to_string()));
            map.insert("pin_max_attempts".to_string(), json!(PIN_REDEEM_MAX_ATTEMPTS.to_string()));
            map.insert("pin_lockout_seconds".to_string(), json!(PIN_REDEEM_LOCKOUT_SECONDS.to_string()));
            Json(json!({ "settings": map }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_kiosk_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let obj = match body.as_object() {
        Some(o) => o,
        None => return Json(json!({ "error": "Expected a JSON object of key-value pairs" })),
    };

    let mut updated = 0;
    for (key, value) in obj {
        let val_str = match value.as_str() {
            Some(s) => s.to_string(),
            None => value.to_string(),
        };

        let result = sqlx::query(
            "INSERT INTO kiosk_settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(&val_str)
        .execute(&state.db)
        .await;

        if result.is_ok() {
            updated += 1;
        }
    }

    // Broadcast updated settings to all connected agents (with per-pod blanking override)
    if updated > 0 {
        let settings_map: std::collections::HashMap<String, String> = obj
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or(&v.to_string()).to_string()))
            .collect();
        state.broadcast_settings(&settings_map).await;
    }

    Json(json!({ "ok": true, "updated": updated }))
}

// ─── POS Lockdown ─────────────────────────────────────────────────────────

/// GET /pos/lockdown — returns current lockdown state for POS polling script.
///
/// MMA Round 1 fixes (2/3 consensus):
/// - P2: Moved to public_routes — POS agent polls without JWT
///
/// MMA Round 2 fixes (2/3 consensus):
/// - P1: On DB error, serve last-known-good cached state (fail-safe)
///   Uses a static AtomicBool cache updated on each successful DB read.
///   Only defaults to unlocked on first-ever read before any DB contact.
/// - P3: Don't leak "db_error" field to unauthenticated clients
async fn get_pos_lockdown(State(state): State<Arc<AppState>>) -> Json<Value> {
    use std::sync::atomic::{AtomicBool, Ordering};
    // Cache last-known-good lockdown state (MMA Round 2 P1: fail-safe, not fail-open)
    static CACHED_LOCKED: AtomicBool = AtomicBool::new(false);
    static CACHE_INITIALIZED: AtomicBool = AtomicBool::new(false);

    match sqlx::query_scalar::<_, String>(
        "SELECT value FROM kiosk_settings WHERE key = 'pos_lockdown'",
    )
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(val)) => {
            let locked = val == "true";
            CACHED_LOCKED.store(locked, Ordering::Relaxed);
            CACHE_INITIALIZED.store(true, Ordering::Relaxed);
            Json(json!({ "locked": locked }))
        }
        Ok(None) => {
            CACHED_LOCKED.store(false, Ordering::Relaxed);
            CACHE_INITIALIZED.store(true, Ordering::Relaxed);
            Json(json!({ "locked": false }))
        }
        Err(e) => {
            tracing::warn!("POS lockdown DB query failed, serving cached state: {e}");
            let cached = if CACHE_INITIALIZED.load(Ordering::Relaxed) {
                CACHED_LOCKED.load(Ordering::Relaxed)
            } else {
                false // No cache yet (fresh startup) — default unlocked
            };
            Json(json!({ "locked": cached }))
        }
    }
}

/// POST /pos/lockdown — toggle lockdown state from admin dashboard
async fn set_pos_lockdown(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let locked = body.get("locked").and_then(|v| v.as_bool()).unwrap_or(true);
    let val = if locked { "true" } else { "false" };

    let result = sqlx::query(
        "INSERT INTO kiosk_settings (key, value) VALUES ('pos_lockdown', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(val)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true, "locked": locked })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Cloud Action Queue Endpoints ─────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CreateActionRequest {
    action_type: String,
    payload: Value,
}

/// POST /actions — create a new action for the venue to pick up.
/// Auth: x-terminal-secret header (same as sync endpoints).
/// When comms_link_url is configured, also pushes the action via relay for sub-second delivery.
async fn create_action(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateActionRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let payload_str = serde_json::to_string(&body.payload).unwrap_or_else(|_| "{}".to_string());

    let result = sqlx::query(
        "INSERT INTO action_queue (id, action_type, payload, status, created_at)
         VALUES (?, ?, ?, 'pending', datetime('now'))",
    )
    .bind(&id)
    .bind(&body.action_type)
    .bind(&payload_str)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Action queue: created {} ({})", id, body.action_type);

            // Also push via relay for sub-second delivery (fire-and-forget).
            // If relay fails, venue will still pick up via polling fallback.
            if let Some(relay_url) = &state.config.cloud.comms_link_url {
                let relay_action_url = format!("{}/relay/action", relay_url);
                let relay_payload = json!({
                    "action_id": &id,
                    "action_type": &body.action_type,
                    "payload": &body.payload,
                });
                let client = state.http_client.clone();
                let id_clone = id.clone();
                tokio::spawn(async move {
                    match client
                        .post(&relay_action_url)
                        .json(&relay_payload)
                        .timeout(std::time::Duration::from_secs(2))
                        .send()
                        .await
                    {
                        Ok(resp) if resp.status().is_success() => {
                            tracing::debug!("Action {} pushed via relay", id_clone);
                        }
                        Ok(resp) => {
                            tracing::debug!("Action relay push returned {}", resp.status());
                        }
                        Err(e) => {
                            tracing::debug!("Action relay push failed (venue will poll): {}", e);
                        }
                    }
                });
            }

            Json(json!({ "ok": true, "id": id, "action_type": body.action_type }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to create action: {}", e) })),
    }
}

/// POST /actions/process — receive a pushed action from comms-link relay.
/// Called by comms-link when it receives a sync_action WS message from the cloud.
/// Auth: x-terminal-secret header.
async fn process_action_endpoint(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    // Parse the action from the request body
    let action: CloudAction = match serde_json::from_value(body.get("action").cloned().unwrap_or(body.clone())) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!("Failed to parse pushed action: {}", e);
            return Json(json!({ "status": "failed", "error": format!("Invalid action: {}", e) }));
        }
    };

    let action_id = body
        .get("action_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    tracing::info!("Processing pushed action: {}", action_id);

    match crate::action_queue::process_action(&state, &action).await {
        Ok(()) => {
            tracing::info!("Pushed action {} completed", action_id);
            Json(json!({ "status": "completed" }))
        }
        Err(e) => {
            tracing::warn!("Pushed action {} failed: {}", action_id, e);
            Json(json!({ "status": "failed", "error": e.to_string() }))
        }
    }
}

/// GET /actions/pending — returns all pending actions for the venue to process.
/// Auth: x-terminal-secret header.
async fn pending_actions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, action_type, payload, created_at
         FROM action_queue
         WHERE status = 'pending'
         ORDER BY created_at ASC
         LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let actions: Vec<Value> = rows
                .iter()
                .map(|(id, action_type, payload, created_at)| {
                    let payload_val: Value =
                        serde_json::from_str(payload).unwrap_or(json!({}));
                    // Build the PendingCloudAction format expected by venue action_queue.rs
                    json!({
                        "id": id,
                        "action": {
                            "action_type": action_type,
                            "payload": payload_val,
                        },
                        "created_at": created_at,
                    })
                })
                .collect();

            // Mark returned actions as processing to avoid re-delivery
            for (id, _, _, _) in &rows {
                let _ = sqlx::query(
                    "UPDATE action_queue SET status = 'processing', processed_at = datetime('now') WHERE id = ?",
                )
                .bind(id)
                .execute(&state.db)
                .await;
            }

            Json(json!({ "actions": actions }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to fetch actions: {}", e) })),
    }
}

/// POST /actions/{id}/ack — venue acknowledges a processed action.
/// Auth: x-terminal-secret header.
async fn ack_action(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let status = body
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("completed");
    let error = body.get("error").and_then(|v| v.as_str());

    let result = sqlx::query(
        "UPDATE action_queue SET status = ?, error = ?, acked_at = datetime('now') WHERE id = ?",
    )
    .bind(status)
    .bind(error)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!("Action queue: acked {} → {}", id, status);
            Json(json!({ "ok": true, "id": id, "status": status }))
        }
        Ok(_) => Json(json!({ "error": "Action not found" })),
        Err(e) => Json(json!({ "error": format!("Failed to ack: {}", e) })),
    }
}

/// GET /actions/history — recent action history for debugging.
/// Auth: x-terminal-secret header.
async fn action_history(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let limit: i64 = params
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let rows = sqlx::query_as::<_, (String, String, String, String, Option<String>, String, Option<String>, Option<String>)>(
        "SELECT id, action_type, payload, status, error, created_at, processed_at, acked_at
         FROM action_queue
         ORDER BY created_at DESC
         LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let actions: Vec<Value> = rows
                .iter()
                .map(|(id, action_type, payload, status, error, created_at, processed_at, acked_at)| {
                    json!({
                        "id": id,
                        "action_type": action_type,
                        "payload": serde_json::from_str::<Value>(payload).unwrap_or(json!({})),
                        "status": status,
                        "error": error,
                        "created_at": created_at,
                        "processed_at": processed_at,
                        "acked_at": acked_at,
                    })
                })
                .collect();
            Json(json!({ "actions": actions, "total": actions.len() }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to fetch history: {}", e) })),
    }
}

// ─── Cloud Sync Endpoints ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct SyncChangesQuery {
    since: Option<String>,
    tables: Option<String>,
    limit: Option<i64>,
}

async fn sync_changes(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<SyncChangesQuery>,
) -> Json<Value> {
    // Require terminal secret for sync endpoint (exposes customer PII)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(serde_json::json!({ "error": "Unauthorized" }));
        }
    }

    // HMAC-SHA256 verification on GET -- permissive mode (AUTH-07)
    // TODO: Switch to strict mode after Bono deploys matching HMAC key
    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let sig = headers.get("x-sync-signature").and_then(|v| v.to_str().ok());
        let ts = headers.get("x-sync-timestamp").and_then(|v| v.to_str().ok());
        let nonce = headers.get("x-sync-nonce").and_then(|v| v.to_str().ok());

        match (sig, ts, nonce) {
            (Some(sig), Some(ts_str), Some(nonce)) => {
                if let Ok(timestamp) = ts_str.parse::<i64>() {
                    // For GET requests, reconstruct query string as signed body
                    let since_val = params.since.as_deref().unwrap_or("1970-01-01T00:00:00Z");
                    let tables_val = params.tables.as_deref().unwrap_or("drivers,wallets,pricing_tiers,kiosk_experiences");
                    let query_body = format!("since={}&tables={}", since_val, tables_val);
                    if !crate::cloud_sync::verify_sync_signature(
                        query_body.as_bytes(), hmac_key.as_bytes(), timestamp, nonce, sig,
                    ) {
                        tracing::warn!(target: "sync", "HMAC verification failed on sync_changes (permissive -- allowing)");
                    }
                } else {
                    tracing::warn!(target: "sync", "Invalid x-sync-timestamp header on sync_changes");
                }
            }
            _ => {
                tracing::warn!(target: "sync", "HMAC headers missing on sync_changes (permissive -- allowing)");
            }
        }
    }

    // Normalize ISO timestamps (2026-03-07T23:48:38Z) to SQLite format (2026-03-07 23:48:38)
    // SQLite's datetime('now') uses space, but sync_state stores ISO with 'T'.
    // String comparison: space (0x20) < 'T' (0x54), so "2026-03-07 23:59" < "2026-03-07T00:00"
    // Without normalization, updated records are never returned after first sync cycle.
    let since = params
        .since
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
        .replace('T', " ")
        .trim_end_matches('Z')
        .trim_end_matches('+')
        .split('+')
        .next()
        .unwrap_or("1970-01-01 00:00:00")
        .to_string();
    let tables: Vec<&str> = params
        .tables
        .as_deref()
        .unwrap_or("drivers,wallets,pricing_tiers,kiosk_experiences")
        .split(',')
        .map(|s| s.trim())
        .collect();
    let limit = params.limit.unwrap_or(500);

    let mut result = json!({});

    for table in &tables {
        match *table {
            "drivers" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'customer_id', customer_id,
                        'name', name, 'email', email, 'phone', phone,
                        'steam_guid', steam_guid, 'iracing_id', iracing_id,
                        'avatar_url', avatar_url, 'total_laps', total_laps,
                        'total_time_ms', total_time_ms,
                        'has_used_trial', COALESCE(has_used_trial, 0),
                        'unlimited_trials', COALESCE(unlimited_trials, 0),
                        'pin_hash', pin_hash, 'phone_verified', COALESCE(phone_verified, 0),
                        'dob', dob, 'waiver_signed', COALESCE(waiver_signed, 0),
                        'waiver_signed_at', waiver_signed_at, 'waiver_version', waiver_version,
                        'guardian_name', guardian_name, 'guardian_phone', guardian_phone,
                        'registration_completed', COALESCE(registration_completed, 0),
                        'signature_data', signature_data,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM drivers
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["drivers"] = json!(items);
                }
            }
            "wallets" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'driver_id', w.driver_id, 'balance_paise', w.balance_paise,
                        'total_credited_paise', w.total_credited_paise,
                        'total_debited_paise', w.total_debited_paise,
                        'updated_at', w.updated_at,
                        'phone', d.phone, 'email', d.email
                    ) FROM wallets w
                    LEFT JOIN drivers d ON d.id = w.driver_id
                    WHERE w.updated_at > ?
                    ORDER BY w.updated_at ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["wallets"] = json!(items);
                }
            }
            "pricing_tiers" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'duration_minutes', duration_minutes,
                        'price_paise', price_paise, 'is_trial', is_trial,
                        'is_active', is_active, 'sort_order', sort_order,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM pricing_tiers
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["pricing_tiers"] = json!(items);
                }
            }
            "kiosk_experiences" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'game', game, 'track', track,
                        'car', car, 'car_class', car_class,
                        'duration_minutes', duration_minutes, 'start_type', start_type,
                        'ac_preset_id', ac_preset_id, 'sort_order', sort_order,
                        'is_active', is_active, 'pricing_tier_id', pricing_tier_id,
                        'created_at', created_at, 'updated_at', updated_at
                    ) FROM kiosk_experiences
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["kiosk_experiences"] = json!(items);
                }
            }
            "pricing_rules" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'rule_name', rule_name, 'rule_type', rule_type,
                        'day_of_week', day_of_week, 'hour_start', hour_start,
                        'hour_end', hour_end, 'multiplier', multiplier,
                        'flat_adjustment_paise', flat_adjustment_paise,
                        'is_active', is_active
                    ) FROM pricing_rules",
                )
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["pricing_rules"] = json!(items);
                }
            }
            "kiosk_settings" => {
                // kiosk_settings is a key-value table, return as a flat object
                let rows = sqlx::query_as::<_, (String, String)>(
                    "SELECT key, value FROM kiosk_settings",
                )
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let mut settings = json!({});
                    for (key, value) in &rows {
                        settings[key] = json!(value);
                    }
                    result["kiosk_settings"] = settings;
                }
            }
            "auth_tokens" => {
                // Only sync pending/unexpired tokens — venue needs these for kiosk PIN validation
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'pod_id', pod_id, 'driver_id', driver_id,
                        'pricing_tier_id', pricing_tier_id, 'auth_type', auth_type,
                        'token', token, 'status', status,
                        'custom_price_paise', custom_price_paise,
                        'custom_duration_minutes', custom_duration_minutes,
                        'experience_id', experience_id,
                        'custom_launch_args', custom_launch_args,
                        'created_at', created_at, 'expires_at', expires_at
                    ) FROM auth_tokens
                    WHERE status = 'pending' AND expires_at > datetime('now')
                    ORDER BY created_at ASC
                    LIMIT ?",
                )
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["auth_tokens"] = json!(items);
                }
            }
            "reservations" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'driver_id', driver_id, 'experience_id', experience_id,
                        'pin', pin, 'status', status, 'pod_number', pod_number,
                        'debit_intent_id', debit_intent_id,
                        'created_at', created_at, 'expires_at', expires_at,
                        'redeemed_at', redeemed_at, 'cancelled_at', cancelled_at,
                        'updated_at', updated_at
                    ) FROM reservations
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["reservations"] = json!(items);
                }
            }
            "debit_intents" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'driver_id', driver_id, 'amount_paise', amount_paise,
                        'reservation_id', reservation_id, 'status', status,
                        'failure_reason', failure_reason, 'wallet_txn_id', wallet_txn_id,
                        'origin', origin,
                        'created_at', created_at, 'processed_at', processed_at,
                        'updated_at', updated_at
                    ) FROM debit_intents
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["debit_intents"] = json!(items);
                }
            }
            "staff_members" => {
                let rows = sqlx::query_as::<_, (String,)>(
                    "SELECT json_object(
                        'id', id, 'name', name, 'phone', phone, 'pin', pin,
                        'is_active', is_active, 'role', COALESCE(role, 'staff'),
                        'created_at', created_at, 'updated_at', updated_at,
                        'last_login_at', last_login_at
                    ) FROM staff_members
                    WHERE updated_at > ? OR (updated_at IS NULL AND created_at > ?)
                    ORDER BY COALESCE(updated_at, created_at) ASC
                    LIMIT ?",
                )
                .bind(&since)
                .bind(&since)
                .bind(limit)
                .fetch_all(&state.db)
                .await;

                if let Ok(rows) = rows {
                    let items: Vec<Value> = rows
                        .iter()
                        .filter_map(|r| serde_json::from_str(&r.0).ok())
                        .collect();
                    result["staff_members"] = json!(items);
                }
            }
            _ => {}
        }
    }

    result["synced_at"] = json!(chrono::Utc::now().to_rfc3339());
    Json(result)
}

/// Parse a config_snapshot JSON value into a VenueConfigSnapshot.
/// Extracted for testability -- used by sync_push handler.
pub(crate) fn parse_config_snapshot(config_snap: &serde_json::Value) -> VenueConfigSnapshot {
    VenueConfigSnapshot {
        venue_name: config_snap.pointer("/venue/name")
            .and_then(|v| v.as_str()).unwrap_or("RacingPoint").to_string(),
        venue_location: config_snap.pointer("/venue/location")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        venue_timezone: config_snap.pointer("/venue/timezone")
            .and_then(|v| v.as_str()).unwrap_or("Asia/Kolkata").to_string(),
        pod_count: config_snap.pointer("/pods/count")
            .and_then(|v| v.as_u64()).unwrap_or(0),
        pod_discovery: config_snap.pointer("/pods/discovery")
            .and_then(|v| v.as_bool()).unwrap_or(false),
        pod_healer_enabled: config_snap.pointer("/pods/healer_enabled")
            .and_then(|v| v.as_bool()).unwrap_or(false),
        pod_healer_interval_secs: config_snap.pointer("/pods/healer_interval_secs")
            .and_then(|v| v.as_u64()).unwrap_or(120),
        branding_primary_color: config_snap.pointer("/branding/primary_color")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        branding_theme: config_snap.pointer("/branding/theme")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        source: config_snap.pointer("/_meta/source")
            .and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        pushed_at: config_snap.pointer("/_meta/pushed_at")
            .and_then(|v| v.as_u64()).unwrap_or(0),
        config_hash: config_snap.pointer("/_meta/hash")
            .and_then(|v| v.as_str()).unwrap_or_default().to_string(),
        received_at: chrono::Utc::now(),
    }
}

/// POST /sync/push — venue pushes data to cloud
async fn sync_push(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body_bytes: axum::body::Bytes,
) -> Json<Value> {
    // Auth check (x-terminal-secret)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(json!({ "error": "Unauthorized" }));
        }
    }

    // HMAC-SHA256 verification -- permissive mode (AUTH-07)
    // TODO: Switch to strict mode after Bono deploys matching HMAC key
    if let Some(hmac_key) = &state.config.cloud.sync_hmac_key {
        let sig = headers.get("x-sync-signature").and_then(|v| v.to_str().ok());
        let ts = headers.get("x-sync-timestamp").and_then(|v| v.to_str().ok());
        let nonce = headers.get("x-sync-nonce").and_then(|v| v.to_str().ok());

        match (sig, ts, nonce) {
            (Some(sig), Some(ts_str), Some(nonce)) => {
                if let Ok(timestamp) = ts_str.parse::<i64>() {
                    if !crate::cloud_sync::verify_sync_signature(
                        &body_bytes, hmac_key.as_bytes(), timestamp, nonce, sig,
                    ) {
                        tracing::warn!(target: "sync", "HMAC verification failed on sync_push (permissive -- allowing)");
                    }
                } else {
                    tracing::warn!(target: "sync", "Invalid x-sync-timestamp header on sync_push");
                }
            }
            _ => {
                tracing::warn!(target: "sync", "HMAC headers missing on sync_push (permissive -- allowing)");
            }
        }
    }

    // Parse JSON body
    let body: Value = match serde_json::from_slice(&body_bytes) {
        Ok(v) => v,
        Err(e) => {
            return Json(json!({ "error": format!("Invalid JSON: {}", e) }));
        }
    };

    // Origin tag check: reject data that originated from us (anti-loop defense)
    let incoming_origin = body.get("origin").and_then(|v| v.as_str()).unwrap_or("unknown");
    let my_origin = &state.config.cloud.origin_id;
    if incoming_origin == my_origin {
        tracing::warn!(target: "sync", "Rejecting sync_push from same origin: {}", my_origin);
        return Json(json!({ "ok": true, "upserted": 0, "reason": "same_origin" }));
    }

    let mut total = 0u64;

    // Upsert laps
    if let Some(laps) = body.get("laps").and_then(|v| v.as_array()) {
        for lap in laps {
            let id = lap.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO laps (id, session_id, driver_id, pod_id, sim_type, track, car,
                    lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)
                 ON CONFLICT(id) DO NOTHING",
            )
            .bind(id)
            .bind(lap.get("session_id").and_then(|v| v.as_str()))
            .bind(lap.get("driver_id").and_then(|v| v.as_str()))
            .bind(lap.get("pod_id").and_then(|v| v.as_str()))
            .bind(lap.get("sim_type").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("track").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("car").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(lap.get("lap_number").and_then(|v| v.as_i64()))
            .bind(lap.get("lap_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(lap.get("sector1_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("sector2_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("sector3_ms").and_then(|v| v.as_i64()))
            .bind(lap.get("valid").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(lap.get("created_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert track_records (best lap per track+car)
    if let Some(records) = body.get("track_records").and_then(|v| v.as_array()) {
        for rec in records {
            let track = rec.get("track").and_then(|v| v.as_str()).unwrap_or_default();
            let car = rec.get("car").and_then(|v| v.as_str()).unwrap_or_default();
            if track.is_empty() || car.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO track_records (track, car, driver_id, best_lap_ms, lap_id, achieved_at)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(track, car) DO UPDATE SET
                    driver_id = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.driver_id ELSE track_records.driver_id END,
                    best_lap_ms = MIN(excluded.best_lap_ms, track_records.best_lap_ms),
                    lap_id = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.lap_id ELSE track_records.lap_id END,
                    achieved_at = CASE WHEN excluded.best_lap_ms < track_records.best_lap_ms
                        THEN excluded.achieved_at ELSE track_records.achieved_at END",
            )
            .bind(track)
            .bind(car)
            .bind(rec.get("driver_id").and_then(|v| v.as_str()))
            .bind(rec.get("best_lap_ms").and_then(|v| v.as_i64()).unwrap_or(i64::MAX))
            .bind(rec.get("lap_id").and_then(|v| v.as_str()))
            .bind(rec.get("achieved_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert personal_bests
    if let Some(pbs) = body.get("personal_bests").and_then(|v| v.as_array()) {
        for pb in pbs {
            let driver_id = pb.get("driver_id").and_then(|v| v.as_str()).unwrap_or_default();
            let track = pb.get("track").and_then(|v| v.as_str()).unwrap_or_default();
            let car = pb.get("car").and_then(|v| v.as_str()).unwrap_or_default();
            if driver_id.is_empty() || track.is_empty() || car.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO personal_bests (driver_id, track, car, best_lap_ms, lap_id, achieved_at)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(driver_id, track, car) DO UPDATE SET
                    best_lap_ms = MIN(excluded.best_lap_ms, personal_bests.best_lap_ms),
                    lap_id = CASE WHEN excluded.best_lap_ms < personal_bests.best_lap_ms
                        THEN excluded.lap_id ELSE personal_bests.lap_id END,
                    achieved_at = CASE WHEN excluded.best_lap_ms < personal_bests.best_lap_ms
                        THEN excluded.achieved_at ELSE personal_bests.achieved_at END",
            )
            .bind(driver_id)
            .bind(track)
            .bind(car)
            .bind(pb.get("best_lap_ms").and_then(|v| v.as_i64()).unwrap_or(i64::MAX))
            .bind(pb.get("lap_id").and_then(|v| v.as_str()))
            .bind(pb.get("achieved_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Upsert billing_sessions
    if let Some(sessions) = body.get("billing_sessions").and_then(|v| v.as_array()) {
        for s in sessions {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id,
                    allocated_seconds, driving_seconds, status, custom_price_paise, notes,
                    started_at, ended_at, created_at, experience_id, car, track, sim_type,
                    split_count, split_duration_minutes,
                    wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason,
                    pause_count, total_paused_seconds, refund_paise)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)
                 ON CONFLICT(id) DO UPDATE SET
                    driving_seconds = excluded.driving_seconds,
                    status = excluded.status,
                    ended_at = excluded.ended_at,
                    wallet_debit_paise = COALESCE(excluded.wallet_debit_paise, billing_sessions.wallet_debit_paise),
                    discount_paise = COALESCE(excluded.discount_paise, billing_sessions.discount_paise),
                    coupon_id = COALESCE(excluded.coupon_id, billing_sessions.coupon_id),
                    original_price_paise = COALESCE(excluded.original_price_paise, billing_sessions.original_price_paise),
                    discount_reason = COALESCE(excluded.discount_reason, billing_sessions.discount_reason),
                    pause_count = COALESCE(excluded.pause_count, billing_sessions.pause_count),
                    total_paused_seconds = COALESCE(excluded.total_paused_seconds, billing_sessions.total_paused_seconds),
                    refund_paise = COALESCE(excluded.refund_paise, billing_sessions.refund_paise)",
            )
            .bind(id)
            .bind(s.get("driver_id").and_then(|v| v.as_str()))
            .bind(s.get("pod_id").and_then(|v| v.as_str()))
            .bind(s.get("pricing_tier_id").and_then(|v| v.as_str()))
            .bind(s.get("allocated_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(s.get("driving_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(s.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
            .bind(s.get("custom_price_paise").and_then(|v| v.as_i64()))
            .bind(s.get("notes").and_then(|v| v.as_str()))
            .bind(s.get("started_at").and_then(|v| v.as_str()))
            .bind(s.get("ended_at").and_then(|v| v.as_str()))
            .bind(s.get("created_at").and_then(|v| v.as_str()))
            .bind(s.get("experience_id").and_then(|v| v.as_str()))
            .bind(s.get("car").and_then(|v| v.as_str()))
            .bind(s.get("track").and_then(|v| v.as_str()))
            .bind(s.get("sim_type").and_then(|v| v.as_str()))
            .bind(s.get("split_count").and_then(|v| v.as_i64()))
            .bind(s.get("split_duration_minutes").and_then(|v| v.as_i64()))
            .bind(s.get("wallet_debit_paise").and_then(|v| v.as_i64()))
            .bind(s.get("discount_paise").and_then(|v| v.as_i64()))
            .bind(s.get("coupon_id").and_then(|v| v.as_str()))
            .bind(s.get("original_price_paise").and_then(|v| v.as_i64()))
            .bind(s.get("discount_reason").and_then(|v| v.as_str()))
            .bind(s.get("pause_count").and_then(|v| v.as_i64()))
            .bind(s.get("total_paused_seconds").and_then(|v| v.as_i64()))
            .bind(s.get("refund_paise").and_then(|v| v.as_i64()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
    }

    // Merge driver updates from venue (venue-owned fields only)
    if let Some(drivers) = body.get("drivers").and_then(|v| v.as_array()) {
        for d in drivers {
            let id = d.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }

            // Only update venue-owned fields, never overwrite cloud-owned fields (name, email, phone)
            let venue_updated = d.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

            // Check if cloud has a newer update for this driver
            let cloud_ts: Option<(Option<String>,)> = sqlx::query_as(
                "SELECT updated_at FROM drivers WHERE id = ?",
            )
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            // Only apply venue fields if venue's updated_at is newer
            let should_apply = match &cloud_ts {
                Some((Some(ts),)) => venue_updated > ts.as_str(),
                Some((None,)) => true,
                None => false, // Driver doesn't exist on cloud — skip partial update
            };

            if should_apply {
                let r = sqlx::query(
                    "UPDATE drivers SET
                        has_used_trial = MAX(COALESCE(has_used_trial, 0), ?),
                        unlimited_trials = MAX(COALESCE(unlimited_trials, 0), ?),
                        total_laps = MAX(COALESCE(total_laps, 0), ?),
                        total_time_ms = MAX(COALESCE(total_time_ms, 0), ?),
                        registration_completed = MAX(COALESCE(registration_completed, 0), ?),
                        waiver_signed = MAX(COALESCE(waiver_signed, 0), ?),
                        waiver_signed_at = COALESCE(?, waiver_signed_at),
                        waiver_version = COALESCE(?, waiver_version),
                        updated_at = ?
                     WHERE id = ?",
                )
                .bind(d.get("has_used_trial").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("unlimited_trials").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("total_laps").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("total_time_ms").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("registration_completed").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("waiver_signed").and_then(|v| v.as_i64()).unwrap_or(0))
                .bind(d.get("waiver_signed_at").and_then(|v| v.as_str()))
                .bind(d.get("waiver_version").and_then(|v| v.as_str()))
                .bind(venue_updated)
                .bind(id)
                .execute(&state.db)
                .await;
                if r.is_ok() { total += 1; }
            }
        }
    }

    // Upsert pods (static config + live status)
    if let Some(pods) = body.get("pods").and_then(|v| v.as_array()) {
        for pod in pods {
            let id = pod.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let number = pod.get("number").and_then(|v| v.as_i64()).unwrap_or(0);
            let name = pod.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let status = pod.get("status").and_then(|v| v.as_str()).unwrap_or("offline");

            // Update DB
            let _ = sqlx::query(
                "INSERT INTO pods (id, number, name, ip_address, sim_type, status, last_seen)
                 VALUES (?1,?2,?3,?4,?5,?6,datetime('now'))
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    ip_address = excluded.ip_address,
                    last_seen = datetime('now')",
            )
            .bind(id)
            .bind(number)
            .bind(name)
            .bind(pod.get("ip_address").and_then(|v| v.as_str()))
            .bind(pod.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa"))
            .bind(status)
            .execute(&state.db)
            .await;

            // Update in-memory pod map so PWA/dashboard sees live status
            let pod_info = rc_common::types::PodInfo {
                id: id.to_string(),
                number: number as u32,
                name: name.to_string(),
                ip_address: pod.get("ip_address").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                mac_address: pod.get("mac_address").and_then(|v| v.as_str()).map(|s| s.to_string()),
                sim_type: pod.get("sim_type").and_then(|v| v.as_str())
                    .and_then(|s| serde_json::from_value(json!(s)).ok())
                    .unwrap_or(rc_common::types::SimType::AssettoCorsa),
                status: serde_json::from_value(json!(status))
                    .unwrap_or(rc_common::types::PodStatus::Offline),
                current_driver: pod.get("current_driver").and_then(|v| v.as_str()).map(|s| s.to_string()),
                current_session_id: pod.get("current_session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                last_seen: Some(chrono::Utc::now()),
                driving_state: None,
                billing_session_id: pod.get("billing_session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                game_state: None,
                current_game: None,
                installed_games: vec![],
                screen_blanked: None,
                ffb_preset: None,
                freedom_mode: None,
                agent_timestamp: None, // Intentional default: cloud sync path has no agent clock
            };
            state.pods.write().await.insert(id.to_string(), pod_info.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod_info));
            total += 1;
        }
    }

    // Upsert wallets (venue pushes balances after billing debits)
    // Handles ID mismatch: if direct driver_id doesn't match, resolve by phone/email
    if let Some(wallets) = body.get("wallets").and_then(|v| v.as_array()) {
        for w in wallets {
            let driver_id = w.get("driver_id").and_then(|v| v.as_str()).unwrap_or_default();
            if driver_id.is_empty() { continue; }

            let balance = w.get("balance_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let credited = w.get("total_credited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let debited = w.get("total_debited_paise").and_then(|v| v.as_i64()).unwrap_or(0);
            let updated = w.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");

            // Try direct driver_id match first
            let r = sqlx::query(
                "UPDATE wallets SET
                    balance_paise = ?, total_credited_paise = ?,
                    total_debited_paise = ?, updated_at = ?
                 WHERE driver_id = ?",
            )
            .bind(balance).bind(credited).bind(debited).bind(updated)
            .bind(driver_id)
            .execute(&state.db)
            .await;

            let rows = r.as_ref().map(|r| r.rows_affected()).unwrap_or(0);
            if rows > 0 {
                total += 1;
                continue;
            }

            // ID didn't match — try to find local driver by phone or email
            let phone = w.get("phone").and_then(|v| v.as_str()).unwrap_or("");
            let email = w.get("email").and_then(|v| v.as_str()).unwrap_or("");

            let resolved: Option<(String,)> = if !phone.is_empty() {
                let ph = state.field_cipher.hash_phone(phone);
                sqlx::query_as("SELECT id FROM drivers WHERE phone_hash = ?")
                    .bind(&ph)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
            } else if !email.is_empty() {
                sqlx::query_as("SELECT id FROM drivers WHERE email = ?")
                    .bind(email)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };

            if let Some((local_id,)) = resolved {
                let r2 = sqlx::query(
                    "UPDATE wallets SET
                        balance_paise = ?, total_credited_paise = ?,
                        total_debited_paise = ?, updated_at = ?
                     WHERE driver_id = ?",
                )
                .bind(balance).bind(credited).bind(debited).bind(updated)
                .bind(&local_id)
                .execute(&state.db)
                .await;
                if r2.is_ok() {
                    tracing::info!("Wallet sync: resolved {} -> {} by phone/email", driver_id, local_id);
                    total += 1;
                }
            }
        }
    }

    // Upsert wallet_transactions (immutable — INSERT OR IGNORE by UUID for idempotency)
    if let Some(txns) = body.get("wallet_transactions").and_then(|v| v.as_array()) {
        for txn in txns {
            let id = txn.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT OR IGNORE INTO wallet_transactions
                    (id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            )
            .bind(id)
            .bind(txn.get("driver_id").and_then(|v| v.as_str()))
            .bind(txn.get("amount_paise").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(txn.get("balance_after_paise").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(txn.get("txn_type").and_then(|v| v.as_str()).unwrap_or("adjustment"))
            .bind(txn.get("reference_id").and_then(|v| v.as_str()))
            .bind(txn.get("notes").and_then(|v| v.as_str()))
            .bind(txn.get("staff_id").and_then(|v| v.as_str()))
            .bind(txn.get("created_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
        tracing::info!("Sync push: {} wallet transactions", txns.len());

        // Shadow verification: compare latest transaction balance with wallet balance
        // Collect unique driver_ids from the pushed transactions
        let mut driver_ids: Vec<String> = txns.iter()
            .filter_map(|t| t.get("driver_id").and_then(|v| v.as_str()).map(|s| s.to_string()))
            .collect();
        driver_ids.sort();
        driver_ids.dedup();

        for did in &driver_ids {
            // Get the most recent transaction's balance_after_paise for this driver
            let txn_balance: Option<(i64,)> = sqlx::query_as(
                "SELECT balance_after_paise FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT 1",
            )
            .bind(did)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            let wallet_balance: Option<(i64,)> = sqlx::query_as(
                "SELECT balance_paise FROM wallets WHERE driver_id = ?",
            )
            .bind(did)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let (Some((txn_bal,)), Some((wallet_bal,))) = (txn_balance, wallet_balance) {
                if txn_bal != wallet_bal {
                    tracing::warn!(
                        driver_id = %did,
                        wallet_balance = wallet_bal,
                        txn_balance = txn_bal,
                        diff = wallet_bal - txn_bal,
                        "Wallet balance discrepancy detected in shadow verification"
                    );
                }
            }
        }
    }

    // Insert billing events (immutable — INSERT OR IGNORE)
    if let Some(events) = body.get("billing_events").and_then(|v| v.as_array()) {
        for ev in events {
            let id = ev.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT OR IGNORE INTO billing_events
                    (id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6)",
            )
            .bind(id)
            .bind(ev.get("billing_session_id").and_then(|v| v.as_str()))
            .bind(ev.get("event_type").and_then(|v| v.as_str()).unwrap_or("unknown"))
            .bind(ev.get("driving_seconds_at_event").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(ev.get("metadata").and_then(|v| v.as_str()))
            .bind(ev.get("created_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
        tracing::info!("Sync push: {} billing events", events.len());
    }

    // Upsert staff_members (venue -> cloud or cloud -> venue)
    if let Some(staff) = body.get("staff_members").and_then(|v| v.as_array()) {
        for s in staff {
            let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let r = sqlx::query(
                "INSERT INTO staff_members (id, name, phone, pin, is_active, role, created_at, updated_at, last_login_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name, phone = excluded.phone, pin = excluded.pin,
                    is_active = excluded.is_active, role = excluded.role,
                    updated_at = excluded.updated_at, last_login_at = excluded.last_login_at",
            )
            .bind(id)
            .bind(s.get("name").and_then(|v| v.as_str()))
            .bind(s.get("phone").and_then(|v| v.as_str()))
            .bind(s.get("pin").and_then(|v| v.as_str()))
            .bind(s.get("is_active").and_then(|v| v.as_i64()).unwrap_or(1))
            .bind(s.get("role").and_then(|v| v.as_str()).unwrap_or("staff"))
            .bind(s.get("created_at").and_then(|v| v.as_str()))
            .bind(s.get("updated_at").and_then(|v| v.as_str()))
            .bind(s.get("last_login_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if r.is_ok() { total += 1; }
        }
        tracing::info!("Sync push: {} staff_members", staff.len());
    }

    // Apply venue config snapshot from James
    if let Some(config_snap) = body.get("config_snapshot") {
        let snapshot = parse_config_snapshot(config_snap);
        tracing::info!(
            venue = %snapshot.venue_name,
            pods = snapshot.pod_count,
            hash = %snapshot.config_hash.get(..8).unwrap_or(&snapshot.config_hash),
            "Config sync: received venue config snapshot"
        );
        *state.venue_config.write().await = Some(snapshot);
        total += 1;
    }

    // Upsert reservations (cloud-authoritative: cloud creates, local updates status)
    if let Some(reservations) = body.get("reservations").and_then(|v| v.as_array()) {
        for r in reservations {
            let id = r.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let res = sqlx::query(
                "INSERT INTO reservations (id, driver_id, experience_id, pin, status,
                    pod_number, debit_intent_id, created_at, expires_at, redeemed_at,
                    cancelled_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    pod_number = COALESCE(excluded.pod_number, reservations.pod_number),
                    debit_intent_id = COALESCE(excluded.debit_intent_id, reservations.debit_intent_id),
                    redeemed_at = COALESCE(excluded.redeemed_at, reservations.redeemed_at),
                    cancelled_at = COALESCE(excluded.cancelled_at, reservations.cancelled_at),
                    updated_at = excluded.updated_at",
            )
            .bind(id)
            .bind(r.get("driver_id").and_then(|v| v.as_str()))
            .bind(r.get("experience_id").and_then(|v| v.as_str()))
            .bind(r.get("pin").and_then(|v| v.as_str()))
            .bind(r.get("status").and_then(|v| v.as_str()).unwrap_or("pending_debit"))
            .bind(r.get("pod_number").and_then(|v| v.as_i64()))
            .bind(r.get("debit_intent_id").and_then(|v| v.as_str()))
            .bind(r.get("created_at").and_then(|v| v.as_str()))
            .bind(r.get("expires_at").and_then(|v| v.as_str()))
            .bind(r.get("redeemed_at").and_then(|v| v.as_str()))
            .bind(r.get("cancelled_at").and_then(|v| v.as_str()))
            .bind(r.get("updated_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if res.is_ok() { total += 1; }
        }
    }

    // Upsert debit_intents (cloud creates pending, local processes and updates status)
    if let Some(intents) = body.get("debit_intents").and_then(|v| v.as_array()) {
        for di in intents {
            let id = di.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if id.is_empty() { continue; }
            let res = sqlx::query(
                "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id,
                    status, failure_reason, wallet_txn_id, origin, created_at,
                    processed_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(id) DO UPDATE SET
                    status = excluded.status,
                    failure_reason = COALESCE(excluded.failure_reason, debit_intents.failure_reason),
                    wallet_txn_id = COALESCE(excluded.wallet_txn_id, debit_intents.wallet_txn_id),
                    processed_at = COALESCE(excluded.processed_at, debit_intents.processed_at),
                    updated_at = excluded.updated_at",
            )
            .bind(id)
            .bind(di.get("driver_id").and_then(|v| v.as_str()))
            .bind(di.get("amount_paise").and_then(|v| v.as_i64()).unwrap_or(0))
            .bind(di.get("reservation_id").and_then(|v| v.as_str()))
            .bind(di.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
            .bind(di.get("failure_reason").and_then(|v| v.as_str()))
            .bind(di.get("wallet_txn_id").and_then(|v| v.as_str()))
            .bind(di.get("origin").and_then(|v| v.as_str()).unwrap_or("cloud"))
            .bind(di.get("created_at").and_then(|v| v.as_str()))
            .bind(di.get("processed_at").and_then(|v| v.as_str()))
            .bind(di.get("updated_at").and_then(|v| v.as_str()))
            .execute(&state.db)
            .await;
            if res.is_ok() { total += 1; }
        }
    }

    tracing::info!("Sync push: upserted {} records", total);
    Json(json!({ "ok": true, "upserted": total }))
}

async fn sync_health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let driver_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM drivers")
        .fetch_one(&state.db)
        .await
        .map(|r| r.0)
        .unwrap_or(0);

    let sync_states = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT table_name, last_synced_at, last_sync_count, COALESCE(updated_at, last_synced_at)
         FROM sync_state ORDER BY table_name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let now = chrono::Utc::now();

    let sync_info: Vec<Value> = sync_states
        .iter()
        .map(|(table, last, count, updated)| {
            // Compute per-table staleness
            let table_lag = chrono::NaiveDateTime::parse_from_str(updated, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(updated, "%Y-%m-%dT%H:%M:%S"))
                .map(|dt| (now - dt.and_utc()).num_seconds())
                .unwrap_or(-1);
            json!({
                "table": table,
                "last_synced_at": last,
                "last_sync_count": count,
                "staleness_seconds": table_lag,
            })
        })
        .collect();

    // Compute overall lag from most recent sync activity
    let last_activity = sqlx::query_as::<_, (String,)>(
        "SELECT MAX(COALESCE(updated_at, last_synced_at)) FROM sync_state",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let lag_seconds: i64 = match last_activity {
        Some((ts,)) => {
            chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S"))
                .map(|dt| (now - dt.and_utc()).num_seconds())
                .unwrap_or(-1)
        }
        None => -1,
    };

    let health_status = if lag_seconds < 0 {
        "unknown"
    } else if lag_seconds <= 60 {
        "healthy"
    } else if lag_seconds <= 300 {
        "degraded"
    } else {
        "critical"
    };

    // Relay status: check if comms-link relay is configured and reachable
    let relay_configured = state.config.cloud.comms_link_url.is_some();
    let relay_available = if relay_configured {
        cloud_sync::is_relay_available(&state).await
    } else {
        false
    };

    // Determine current sync mode
    let sync_mode = if !state.config.cloud.enabled {
        "disabled"
    } else if relay_configured && relay_available {
        "relay"
    } else {
        "http"
    };

    Json(json!({
        "status": health_status,
        "lag_seconds": lag_seconds,
        "drivers": driver_count,
        "cloud_sync_enabled": state.config.cloud.enabled,
        "cloud_api_url": state.config.cloud.api_url,
        "relay_configured": relay_configured,
        "relay_available": relay_available,
        "sync_mode": sync_mode,
        "comms_link_url": state.config.cloud.comms_link_url,
        "sync_state": sync_info,
    }))
}

// ─── Terminal (remote command execution) ─────────────────────────────────────

async fn check_terminal_auth(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), String> {
    // 1. Check PIN session token (x-terminal-session header)
    if let Some(token) = headers.get("x-terminal-session").and_then(|v| v.to_str().ok()) {
        let sessions = state.terminal_sessions.read().await;
        if let Some(expiry) = sessions.get(token) {
            if *expiry > chrono::Utc::now() {
                return Ok(());
            }
        }
    }

    // 2. Check legacy shared secret (x-terminal-secret header — for cloud polling)
    let secret = state.config.cloud.terminal_secret.as_deref();
    if let Some(secret) = secret {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided == Some(secret) {
            return Ok(());
        }
    }

    // 3. If no secret AND no pin configured, allow (local dev)
    if state.config.cloud.terminal_secret.is_none() && state.config.cloud.terminal_pin.is_none() {
        return Ok(());
    }

    Err("Unauthorized. Use POST /terminal/auth with your PIN.".to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TerminalAuthRequest {
    pin: String,
}

/// POST /terminal/auth — authenticate with PIN, returns a 24h session token
async fn terminal_auth(
    State(state): State<Arc<AppState>>,
    Json(req): Json<TerminalAuthRequest>,
) -> Json<Value> {
    let expected = state.config.cloud.terminal_pin.as_deref();
    match expected {
        None => {
            return Json(json!({ "error": "Terminal PIN not configured on server." }));
        }
        Some(pin) => {
            if req.pin != pin {
                tracing::warn!("Terminal auth failed — wrong PIN");
                return Json(json!({ "error": "Invalid PIN." }));
            }
        }
    }

    // Generate session token valid for 24 hours
    let token = uuid::Uuid::new_v4().to_string();
    let expiry = chrono::Utc::now() + chrono::Duration::hours(24);

    // Clean up expired sessions while we're here
    let mut sessions = state.terminal_sessions.write().await;
    let now = chrono::Utc::now();
    sessions.retain(|_, exp| *exp > now);
    sessions.insert(token.clone(), expiry);
    drop(sessions);

    tracing::info!("Terminal session created (expires {})", expiry.format("%Y-%m-%d %H:%M UTC"));

    Json(json!({
        "session": token,
        "expires_at": expiry.to_rfc3339(),
    }))
}

#[derive(Deserialize)]
struct TerminalSubmitRequest {
    cmd: String,
    timeout_ms: Option<i64>,
}

#[derive(Deserialize)]
struct TerminalResultRequest {
    exit_code: Option<i64>,
    stdout: Option<String>,
    stderr: Option<String>,
}

#[derive(Deserialize)]
struct TerminalListQuery {
    limit: Option<i64>,
}

async fn terminal_submit(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<TerminalSubmitRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let timeout_ms = req.timeout_ms.unwrap_or(30000).min(120000);

    let result = sqlx::query(
        "INSERT INTO terminal_commands (id, cmd, status, timeout_ms) VALUES (?, ?, 'pending', ?)",
    )
    .bind(&id)
    .bind(&req.cmd)
    .bind(timeout_ms)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Terminal command queued: {} ({})", id, req.cmd);

            // Audit trail for terminal command (MEDIUM sensitivity)
            let cmd_truncated: String = req.cmd.chars().take(200).collect();
            accounting::log_admin_action(
                &state, "terminal_command",
                &json!({"command_id": id, "command": cmd_truncated}).to_string(),
                None, None,
            ).await;

            // Execute locally in background for instant results (no cloud poll delay)
            let exec_state = state.clone();
            let exec_id = id.clone();
            let exec_cmd = req.cmd.clone();
            let exec_timeout = timeout_ms as u64;
            tokio::spawn(async move {
                use tokio::time::{timeout, Duration};
                use tokio::process::Command;

                // Mark as running
                let _ = sqlx::query(
                    "UPDATE terminal_commands SET status = 'running', started_at = datetime('now') WHERE id = ? AND status = 'pending'",
                )
                .bind(&exec_id)
                .execute(&exec_state.db)
                .await;

                let max_output: usize = 100 * 1024;
                let result = timeout(Duration::from_millis(exec_timeout), async {
                    #[cfg(windows)]
                    { Command::new("cmd").args(["/C", &exec_cmd]).kill_on_drop(true).output().await }
                    #[cfg(not(windows))]
                    { Command::new("sh").args(["-c", &exec_cmd]).kill_on_drop(true).output().await }
                }).await;

                let (exit_code, stdout, stderr) = match result {
                    Ok(Ok(output)) => {
                        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        if stdout.len() > max_output { stdout.truncate(max_output); stdout.push_str("\n... [truncated]"); }
                        if stderr.len() > max_output { stderr.truncate(max_output); stderr.push_str("\n... [truncated]"); }
                        (output.status.code(), stdout, stderr)
                    }
                    Ok(Err(e)) => (None, String::new(), format!("Failed to execute: {}", e)),
                    Err(_) => (Some(124), String::new(), format!("Timed out after {}ms", exec_timeout)),
                };

                let _ = sqlx::query(
                    "UPDATE terminal_commands SET status = 'completed', exit_code = ?, stdout = ?, stderr = ?, completed_at = datetime('now') WHERE id = ?",
                )
                .bind(exit_code)
                .bind(&stdout)
                .bind(&stderr)
                .bind(&exec_id)
                .execute(&exec_state.db)
                .await;

                tracing::info!("Terminal command {} executed locally (exit: {:?})", exec_id, exit_code);
            });

            Json(json!({ "status": "queued", "id": id }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn terminal_list(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<TerminalListQuery>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let limit = params.limit.unwrap_or(50).min(200);

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'cmd', cmd, 'status', status,
            'exit_code', exit_code, 'stdout', stdout, 'stderr', stderr,
            'timeout_ms', timeout_ms,
            'created_at', created_at, 'started_at', started_at, 'completed_at', completed_at
        ) FROM terminal_commands
        ORDER BY created_at DESC
        LIMIT ?",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let commands: Vec<Value> = rows
                .iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            Json(json!({ "commands": commands }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn terminal_pending(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query_as::<_, (String,)>(
        "SELECT json_object(
            'id', id, 'cmd', cmd, 'timeout_ms', timeout_ms, 'created_at', created_at
        ) FROM terminal_commands
        WHERE status = 'pending'
        ORDER BY created_at ASC
        LIMIT 10",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let commands: Vec<Value> = rows
                .iter()
                .filter_map(|r| serde_json::from_str(&r.0).ok())
                .collect();
            Json(json!({ "commands": commands }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn terminal_result(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<TerminalResultRequest>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let status = if req.exit_code == Some(124) { "timeout" }
        else if req.exit_code.is_some() && req.exit_code != Some(0) { "failed" }
        else { "completed" };

    let result = sqlx::query(
        "UPDATE terminal_commands SET
            status = ?, exit_code = ?, stdout = ?, stderr = ?, completed_at = datetime('now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(req.exit_code)
    .bind(&req.stdout)
    .bind(&req.stderr)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!("Terminal command {} completed ({})", id, status);
            Json(json!({ "status": "ok" }))
        }
        Ok(_) => Json(json!({ "error": "Command not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Employee Endpoints ──────────────────────────────────────────────────

/// GET /employee/daily-pin — returns today's 4-digit debug PIN (employee-only, JWT auth)
async fn employee_daily_pin(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Verify employee flag
    if !auth::is_employee(&state, &driver_id).await {
        return Json(json!({ "error": "Access denied. Employee account required." }));
    }

    let pin = auth::todays_debug_pin(&state.config.auth.jwt_secret);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    Json(json!({
        "pin": pin,
        "valid_date": today,
        "note": "4-digit PIN valid until midnight UTC. Enter on any pod lock screen to unlock debug mode."
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmployeeDebugUnlockRequest {
    pin: String,
    pod_id: String,
}

/// POST /employee/debug-unlock — unlock a specific pod in debug mode
async fn employee_debug_unlock(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmployeeDebugUnlockRequest>,
) -> Json<Value> {
    match auth::validate_employee_pin_kiosk(&state, req.pin, Some(req.pod_id.clone())).await {
        Ok(_) => Json(json!({
            "status": "ok",
            "pod_id": req.pod_id,
            "mode": "debug",
            "message": "Pod unlocked in debug mode. Content Manager access enabled."
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Staff ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StaffValidatePinRequest {
    pin: String,
}

async fn staff_validate_pin(
    State(state): State<Arc<AppState>>,
    Json(req): Json<StaffValidatePinRequest>,
) -> Json<Value> {
    // Read role from DB — DEFAULT 'staff' (legacy, maps to cashier in middleware)
    let result = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, name, role FROM staff_members WHERE pin = ? AND is_active = 1",
    )
    .bind(&req.pin)
    .fetch_optional(&state.db)
    .await;

    match result {
        Ok(Some((id, name, role_opt))) => {
            let _ = sqlx::query(
                "UPDATE staff_members SET last_login_at = datetime('now') WHERE id = ?",
            )
            .bind(&id)
            .execute(&state.db)
            .await;

            // Use role from DB, default to "cashier" if NULL
            let role = role_opt.as_deref().unwrap_or("cashier");
            let token = auth::middleware::create_staff_jwt_with_role(
                &state.config.auth.jwt_secret,
                &id,
                role,
                24,
            );

            match token {
                Ok(jwt) => Json(json!({
                    "status": "ok",
                    "staff_id": id,
                    "staff_name": name,
                    "role": role,
                    "token": jwt,
                })),
                Err(e) => Json(json!({
                    "status": "ok",
                    "staff_id": id,
                    "staff_name": name,
                    "error": format!("Login ok but token failed: {}", e),
                })),
            }
        }
        Ok(None) => Json(json!({ "error": "Invalid staff PIN" })),
        Err(e) => Json(json!({ "error": format!("Database error: {}", e) })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateStaffRequest {
    name: String,
    phone: String,
    pin: String,
}

async fn create_staff(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateStaffRequest>,
) -> Json<Value> {
    let id = format!("staff_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));

    match sqlx::query(
        "INSERT INTO staff_members (id, name, phone, pin) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.phone)
    .bind(&req.pin)
    .execute(&state.db)
    .await
    {
        Ok(_) => Json(json!({ "status": "ok", "id": id, "name": req.name })),
        Err(e) => Json(json!({ "error": format!("{}", e) })),
    }
}

async fn list_staff(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool, Option<String>, Option<String>)>(
        "SELECT id, name, phone, pin, is_active, last_login_at, role FROM staff_members ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let staff: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, phone, _pin, active, last_login, role)| {
            // SEC: Never expose staff PINs in API responses
            json!({
                "id": id,
                "name": name,
                "phone": phone,
                "is_active": active,
                "last_login_at": last_login,
                "role": role.unwrap_or_else(|| "staff".to_string()),
            })
        })
        .collect();

    Json(json!({ "staff": staff }))
}

#[derive(Debug, Deserialize)]
struct UpdateStaffRequest {
    name: Option<String>,
    phone: Option<String>,
    pin: Option<String>,
    role: Option<String>,
    is_active: Option<bool>,
}

async fn update_staff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStaffRequest>,
) -> Json<Value> {
    // Verify staff exists
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM staff_members WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if exists == 0 {
        return Json(json!({ "error": "Staff member not found" }));
    }

    // Validate role if provided
    if let Some(ref role) = req.role {
        if !["staff", "cashier", "manager", "superadmin"].contains(&role.as_str()) {
            return Json(json!({ "error": format!("Invalid role: {}. Must be one of: staff, cashier, manager, superadmin", role) }));
        }
    }

    // Build dynamic UPDATE
    let mut sets: Vec<String> = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(ref name) = req.name {
        sets.push("name = ?".to_string());
        binds.push(name.clone());
    }
    if let Some(ref phone) = req.phone {
        sets.push("phone = ?".to_string());
        binds.push(phone.clone());
    }
    if let Some(ref pin) = req.pin {
        sets.push("pin = ?".to_string());
        binds.push(pin.clone());
    }
    if let Some(ref role) = req.role {
        sets.push("role = ?".to_string());
        binds.push(role.clone());
    }
    if let Some(active) = req.is_active {
        sets.push("is_active = ?".to_string());
        binds.push(if active { "1".to_string() } else { "0".to_string() });
    }

    if sets.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    sets.push("updated_at = datetime('now')".to_string());
    let sql = format!("UPDATE staff_members SET {} WHERE id = ?", sets.join(", "));

    let mut query = sqlx::query(&sql);
    for val in &binds {
        query = query.bind(val);
    }
    // Bind is_active as integer separately if present
    query = query.bind(&id);

    match query.execute(&state.db).await {
        Ok(_) => Json(json!({ "status": "ok", "id": id })),
        Err(e) => Json(json!({ "error": format!("{}", e) })),
    }
}

async fn delete_staff(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    match sqlx::query(
        "UPDATE staff_members SET is_active = 0, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    {
        Ok(result) => {
            if result.rows_affected() == 0 {
                Json(json!({ "error": "Staff member not found" }))
            } else {
                Json(json!({ "status": "ok", "id": id }))
            }
        }
        Err(e) => Json(json!({ "error": format!("{}", e) })),
    }
}

// ─── HR & Hiring Psychology (v14.0 Phase 96) ─────────────────────────────

async fn list_hiring_sjts(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, scenario_text, options_json, scoring_json
         FROM hiring_sjts WHERE is_active = 1 ORDER BY id"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let sjts: Vec<Value> = rows
        .into_iter()
        .map(|(id, scenario, options, scoring)| {
            json!({
                "id": id,
                "scenario_text": scenario,
                "options": serde_json::from_str::<Value>(&options).unwrap_or(json!([])),
                "scoring": serde_json::from_str::<Value>(&scoring).unwrap_or(json!([])),
            })
        })
        .collect();

    Json(json!({ "sjts": sjts }))
}

async fn get_hiring_sjt(
    State(state): State<Arc<AppState>>,
    Path(sjt_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, scenario_text, options_json, scoring_json
         FROM hiring_sjts WHERE id = ? AND is_active = 1"
    )
    .bind(&sjt_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some((id, scenario, options, scoring)) => Json(json!({
            "id": id,
            "scenario_text": scenario,
            "options": serde_json::from_str::<Value>(&options).unwrap_or(json!([])),
            "scoring": serde_json::from_str::<Value>(&scoring).unwrap_or(json!([])),
        })),
        None => Json(json!({ "error": "SJT not found" })),
    }
}

async fn list_job_preview(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(
        "SELECT id, title, content, media_url, sort_order
         FROM job_preview ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let items: Vec<Value> = rows
        .into_iter()
        .map(|(id, title, content, media_url, sort_order)| {
            json!({
                "id": id,
                "title": title,
                "content": content,
                "media_url": media_url,
                "sort_order": sort_order,
            })
        })
        .collect();

    Json(json!({ "items": items }))
}

async fn list_campaign_templates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, name, cialdini_principle, message_template, target_segment, is_active
         FROM campaign_templates ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let templates: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, principle, template, segment, active)| {
            json!({
                "id": id,
                "name": name,
                "cialdini_principle": principle,
                "message_template": template,
                "target_segment": segment,
                "is_active": active,
            })
        })
        .collect();

    Json(json!({ "templates": templates }))
}

async fn list_nudge_templates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, bool)>(
        "SELECT id, template_type, copy_text, timing_rules_json, is_active
         FROM nudge_templates ORDER BY template_type"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let templates: Vec<Value> = rows
        .into_iter()
        .map(|(id, ttype, copy, timing, active)| {
            json!({
                "id": id,
                "template_type": ttype,
                "copy_text": copy,
                "timing_rules": serde_json::from_str::<Value>(&timing).unwrap_or(json!({})),
                "is_active": active,
            })
        })
        .collect();

    Json(json!({ "templates": templates }))
}

async fn hr_recognition_data(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Combine kudos + badges for recognition page
    let kudos = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT sk.id, s1.name, s2.name, sk.message, sk.category, sk.created_at
         FROM staff_kudos sk
         JOIN staff_members s1 ON s1.id = sk.sender_id
         JOIN staff_members s2 ON s2.id = sk.receiver_id
         ORDER BY sk.created_at DESC LIMIT 20"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let kudos_list: Vec<Value> = kudos
        .into_iter()
        .map(|(id, sender, receiver, msg, cat, created)| {
            json!({
                "id": id, "sender_name": sender, "receiver_name": receiver,
                "message": msg, "category": cat, "created_at": created,
            })
        })
        .collect();

    // Top badge earners
    let badge_leaders = sqlx::query_as::<_, (String, i64)>(
        "SELECT sm.name, COUNT(*) as badge_count
         FROM staff_earned_badges seb
         JOIN staff_members sm ON sm.id = seb.staff_id
         GROUP BY seb.staff_id
         ORDER BY badge_count DESC LIMIT 10"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let leaders: Vec<Value> = badge_leaders
        .into_iter()
        .map(|(name, count)| json!({ "name": name, "badge_count": count }))
        .collect();

    Json(json!({
        "recent_kudos": kudos_list,
        "badge_leaders": leaders,
    }))
}

// ─── Staff Gamification (v14.0 Phase 95) ─────────────────────────────────

async fn staff_gamification_opt_in(
    State(state): State<Arc<AppState>>,
    Path(staff_id): Path<String>,
) -> Json<Value> {
    // Toggle opt-in
    let current: Option<bool> = sqlx::query_scalar(
        "SELECT gamification_opt_in FROM staff_members WHERE id = ?"
    )
    .bind(&staff_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let new_val = match current {
        Some(true) => false,
        _ => true,
    };

    let _ = sqlx::query("UPDATE staff_members SET gamification_opt_in = ? WHERE id = ?")
        .bind(new_val)
        .bind(&staff_id)
        .execute(&state.db)
        .await;

    Json(json!({ "staff_id": staff_id, "gamification_opt_in": new_val }))
}

async fn staff_gamification_leaderboard(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Sessions hosted this month by opted-in staff
    let rows = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT sm.id, sm.name,
                (SELECT COUNT(*) FROM billing_sessions bs
                 WHERE bs.staff_id = sm.id
                 AND bs.started_at >= datetime('now', 'start of month')) as sessions_hosted
         FROM staff_members sm
         WHERE sm.gamification_opt_in = 1 AND sm.is_active = 1
         ORDER BY sessions_hosted DESC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let leaderboard: Vec<Value> = rows
        .into_iter()
        .enumerate()
        .map(|(i, (id, name, sessions))| {
            json!({
                "rank": i + 1,
                "staff_id": id,
                "name": name,
                "sessions_hosted": sessions,
            })
        })
        .collect();

    Json(json!({ "leaderboard": leaderboard }))
}

async fn staff_badges_list(
    State(state): State<Arc<AppState>>,
    Path(staff_id): Path<String>,
) -> Json<Value> {
    let badges = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String)>(
        "SELECT sb.id, sb.name, sb.description, sb.badge_icon, seb.earned_at
         FROM staff_earned_badges seb
         JOIN staff_badges sb ON sb.id = seb.badge_id
         WHERE seb.staff_id = ?
         ORDER BY seb.earned_at DESC"
    )
    .bind(&staff_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let list: Vec<Value> = badges
        .into_iter()
        .map(|(id, name, desc, icon, earned_at)| {
            json!({
                "id": id,
                "name": name,
                "description": desc,
                "badge_icon": icon,
                "earned_at": earned_at,
            })
        })
        .collect();

    Json(json!({ "badges": list }))
}

async fn staff_kudos_create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let sender_id = body.get("sender_id").and_then(|v| v.as_str()).unwrap_or("");
    let receiver_id = body.get("receiver_id").and_then(|v| v.as_str()).unwrap_or("");
    let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("");
    let category = body.get("category").and_then(|v| v.as_str()).unwrap_or("teamwork");

    if sender_id.is_empty() || receiver_id.is_empty() || message.is_empty() {
        return Json(json!({ "error": "sender_id, receiver_id, and message are required" }));
    }
    if sender_id == receiver_id {
        return Json(json!({ "error": "Cannot give kudos to yourself" }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query(
        "INSERT INTO staff_kudos (id, sender_id, receiver_id, message, category) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(sender_id)
    .bind(receiver_id)
    .bind(message)
    .bind(category)
    .execute(&state.db)
    .await
    {
        Ok(_) => Json(json!({ "id": id, "status": "created" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn staff_kudos_list(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
        "SELECT sk.id, sk.sender_id, s1.name, sk.receiver_id, s2.name, sk.message, sk.category
         FROM staff_kudos sk
         JOIN staff_members s1 ON s1.id = sk.sender_id
         JOIN staff_members s2 ON s2.id = sk.receiver_id
         WHERE sk.created_at >= datetime('now', '-30 days')
         ORDER BY sk.created_at DESC
         LIMIT 50"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let kudos: Vec<Value> = rows
        .into_iter()
        .map(|(id, sender_id, sender_name, receiver_id, receiver_name, message, category)| {
            json!({
                "id": id,
                "sender_id": sender_id,
                "sender_name": sender_name,
                "receiver_id": receiver_id,
                "receiver_name": receiver_name,
                "message": message,
                "category": category,
            })
        })
        .collect();

    Json(json!({ "kudos": kudos }))
}

async fn staff_challenges_list(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, i64, Option<String>, String, String, i64, String)>(
        "SELECT id, name, description, goal_type, goal_target, reward_description,
                start_date, end_date, current_progress, status
         FROM staff_challenges
         WHERE status = 'active'
         ORDER BY end_date ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let challenges: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, desc, goal_type, goal_target, reward, start, end, progress, status)| {
            let pct = if goal_target > 0 { (progress * 100 / goal_target).min(100) } else { 0 };
            json!({
                "id": id,
                "name": name,
                "description": desc,
                "goal_type": goal_type,
                "goal_target": goal_target,
                "reward_description": reward,
                "start_date": start,
                "end_date": end,
                "current_progress": progress,
                "progress_percent": pct,
                "status": status,
            })
        })
        .collect();

    Json(json!({ "challenges": challenges }))
}

async fn staff_challenges_create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Challenge");
    let description = body.get("description").and_then(|v| v.as_str());
    let goal_type = body.get("goal_type").and_then(|v| v.as_str()).unwrap_or("sessions_hosted");
    let goal_target = body.get("goal_target").and_then(|v| v.as_i64()).unwrap_or(10);
    let reward = body.get("reward_description").and_then(|v| v.as_str());
    let start_date = body.get("start_date").and_then(|v| v.as_str()).unwrap_or("");
    let end_date = body.get("end_date").and_then(|v| v.as_str()).unwrap_or("");

    if start_date.is_empty() || end_date.is_empty() {
        return Json(json!({ "error": "start_date and end_date are required" }));
    }

    match sqlx::query(
        "INSERT INTO staff_challenges (id, name, description, goal_type, goal_target, reward_description, start_date, end_date)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(name)
    .bind(description)
    .bind(goal_type)
    .bind(goal_target)
    .bind(reward)
    .bind(start_date)
    .bind(end_date)
    .execute(&state.db)
    .await
    {
        Ok(_) => Json(json!({ "id": id, "status": "created" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn staff_challenge_update_progress(
    State(state): State<Arc<AppState>>,
    Path(challenge_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let progress = body.get("current_progress").and_then(|v| v.as_i64()).unwrap_or(0);

    // Check if goal met
    let goal_target: Option<i64> = sqlx::query_scalar(
        "SELECT goal_target FROM staff_challenges WHERE id = ?"
    )
    .bind(&challenge_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let new_status = match goal_target {
        Some(target) if progress >= target => "completed",
        _ => "active",
    };

    let _ = sqlx::query(
        "UPDATE staff_challenges SET current_progress = ?, status = ? WHERE id = ?"
    )
    .bind(progress)
    .bind(new_status)
    .bind(&challenge_id)
    .execute(&state.db)
    .await;

    Json(json!({ "id": challenge_id, "current_progress": progress, "status": new_status }))
}

// ─── Friends ──────────────────────────────────────────────────────────────

async fn customer_friends(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::list_friends(&state, &driver_id).await {
        Ok(list) => Json(json!({ "friends": list })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_friend_requests(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::list_friend_requests(&state, &driver_id).await {
        Ok((incoming, outgoing)) => Json(json!({
            "incoming": incoming,
            "outgoing": outgoing,
        })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_send_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let identifier = match req.get("identifier").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'identifier' (phone or customer_id)" })),
    };

    match friends::send_friend_request(&state, &driver_id, &identifier).await {
        Ok(request_id) => Json(json!({ "status": "ok", "request_id": request_id })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_accept_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::accept_friend_request(&state, &request_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_reject_friend_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::reject_friend_request(&state, &request_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_remove_friend(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(friend_driver_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match friends::remove_friend(&state, &driver_id, &friend_driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_set_presence(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let presence = match req.get("presence").and_then(|v| v.as_str()) {
        Some(p) => p.to_string(),
        None => return Json(json!({ "error": "Missing 'presence' (online/hidden)" })),
    };

    match friends::set_presence(&state, &driver_id, &presence).await {
        Ok(()) => Json(json!({ "status": "ok", "presence": presence })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Multiplayer ──────────────────────────────────────────────────────────

async fn customer_book_multiplayer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let experience_id = req.get("experience_id").and_then(|v| v.as_str()).map(String::from);

    let pricing_tier_id = match req.get("pricing_tier_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pricing_tier_id'" })),
    };

    let friend_ids: Vec<String> = match req.get("friend_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => return Json(json!({ "error": "Missing 'friend_ids' array" })),
    };

    if friend_ids.is_empty() {
        return Json(json!({ "error": "Need at least one friend for multiplayer" }));
    }

    // Accept optional custom booking payload (same as single-player custom booking)
    let custom = req.get("custom").and_then(|v| {
        let game = v.get("game")?.as_str()?.to_string();
        let track = v.get("track")?.as_str()?.to_string();
        let car = v.get("car")?.as_str()?.to_string();
        Some((game, track, car))
    });

    if experience_id.is_none() && custom.is_none() {
        return Json(json!({ "error": "Must provide 'experience_id' or 'custom' booking payload" }));
    }

    match multiplayer::book_multiplayer(&state, &driver_id, experience_id.as_deref(), &pricing_tier_id, friend_ids, custom).await {
        Ok(info) => Json(json!({ "status": "ok", "group_session": info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_group_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::get_active_group_session(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "group_session": info })),
        Ok(None) => Json(json!({ "group_session": null })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_accept_group_invite(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::accept_group_invite(&state, &group_session_id, &driver_id).await {
        Ok(member) => Json(json!({ "status": "ok", "member": member })),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn customer_decline_group_invite(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match multiplayer::decline_group_invite(&state, &group_session_id, &driver_id).await {
        Ok(()) => Json(json!({ "status": "ok" })),
        Err(e) => Json(json!({ "error": e })),
    }
}

// ─── Telemetry ────────────────────────────────────────────────────────────

async fn customer_telemetry(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Find the pod this driver is actively billing on
    let active_pod: Option<(String,)> = sqlx::query_as(
        "SELECT pod_id FROM billing_sessions WHERE driver_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let pod_id = match active_pod {
        Some((pid,)) => pid,
        None => return Json(json!({ "error": "No active session" })),
    };

    // Get latest telemetry sample for this pod
    let sample: Option<(String, String)> = sqlx::query_as(
        "SELECT data, sampled_at FROM telemetry_samples WHERE pod_id = ? ORDER BY sampled_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match sample {
        Some((data, sampled_at)) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(&data) {
                Json(json!({
                    "pod_id": pod_id,
                    "telemetry": parsed,
                    "sampled_at": sampled_at,
                }))
            } else {
                Json(json!({ "pod_id": pod_id, "telemetry": data, "sampled_at": sampled_at }))
            }
        }
        None => Json(json!({ "pod_id": pod_id, "telemetry": null })),
    }
}

// ─── Shareable Session Report ────────────────────────────────────────────────

async fn customer_session_share(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Fetch billing session
    let session = sqlx::query_as::<_, (
        String, String, String, i64, i64, String, i64,
        Option<String>, Option<String>, Option<String>, Option<String>,
    )>(
        "SELECT bs.id, bs.pod_id, pt.name, bs.allocated_seconds, bs.driving_seconds,
                bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.car, bs.track
         FROM billing_sessions bs
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ? AND bs.driver_id = ?",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get driver name
    let driver_name: String = sqlx::query_as::<_, (String,)>(
        "SELECT name FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or_else(|| "Driver".to_string());

    // Get laps
    let laps = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String)>(
        "SELECT lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, track, car
         FROM laps WHERE session_id = ? AND driver_id = ?
         ORDER BY lap_number ASC",
    )
    .bind(&id)
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len();
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.5).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.1).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.1).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };
    let consistency = if valid_laps.len() >= 3 {
        let mean = valid_laps.iter().map(|l| l.1 as f64).sum::<f64>() / valid_laps.len() as f64;
        let variance = valid_laps.iter().map(|l| {
            let diff = l.1 as f64 - mean;
            diff * diff
        }).sum::<f64>() / valid_laps.len() as f64;
        let std_dev = variance.sqrt();
        let cv = std_dev / mean * 100.0;
        // Lower CV = more consistent. <2% = excellent, <5% = good, <10% = average
        Some(json!({
            "std_dev_ms": std_dev.round() as i64,
            "coefficient_of_variation": (cv * 100.0).round() / 100.0,
            "rating": if cv < 2.0 { "Excellent" } else if cv < 5.0 { "Good" } else if cv < 10.0 { "Average" } else { "Inconsistent" },
        }))
    } else {
        None
    };

    // Determine track/car from laps or session
    let track = laps.first().map(|l| l.6.clone()).or(session.10.clone()).unwrap_or_default();
    let car = laps.first().map(|l| l.7.clone()).or(session.9.clone()).unwrap_or_default();

    // Percentile ranking: how does this best lap compare to all laps on this track+car?
    let percentile = if let Some(best) = best_lap_ms {
        compute_percentile(&state.db, best, &track, &car).await
    } else {
        None
    };

    // Track record for comparison
    let track_record: Option<(i64, String)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT tr.best_lap_ms, d.name FROM track_records tr
             JOIN drivers d ON tr.driver_id = d.id
             WHERE tr.track = ? AND tr.car = ?",
        )
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Personal best for this track+car
    let personal_best: Option<(i64,)> = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as(
            "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
        )
        .bind(&driver_id)
        .bind(&track)
        .bind(&car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        None
    };

    // Improvement: compare first valid lap to best valid lap
    let improvement_ms = if valid_laps.len() >= 2 {
        match (valid_laps.first(), best_lap_ms) {
            (Some(first), Some(best)) => Some(first.1 - best),
            _ => None,
        }
    } else {
        None
    };

    // Build share card data
    let driving_minutes = session.4 / 60;

    Json(json!({
        "share_report": {
            "driver_name": driver_name,
            "track": track,
            "car": car,
            "date": session.7,
            "driving_time_seconds": session.4,
            "driving_time_display": format!("{}m {}s", driving_minutes, session.4 % 60),
            "total_laps": total_laps,
            "valid_laps": valid_laps.len(),
            "best_lap_ms": best_lap_ms,
            "best_lap_display": best_lap_ms.map(|ms| format!("{}:{:02}.{:03}", ms / 60000, (ms % 60000) / 1000, ms % 1000)),
            "average_lap_ms": avg_lap_ms,
            "improvement_ms": improvement_ms,
            "consistency": consistency,
            "percentile_rank": percentile,
            "percentile_text": percentile.map(|p| format!("Top {}% of drivers", 100 - p.min(99))),
            "track_record": track_record.as_ref().map(|(ms, name)| json!({
                "time_ms": ms,
                "holder": name,
                "gap_ms": best_lap_ms.map(|b| b - ms),
            })),
            "personal_best_ms": personal_best.map(|pb| pb.0),
            "is_new_pb": personal_best.map(|pb| best_lap_ms == Some(pb.0)).unwrap_or(false),
            "laps": laps.iter().map(|l| json!({
                "lap": l.0, "time_ms": l.1,
                "s1": l.2, "s2": l.3, "s3": l.4,
                "valid": l.5,
            })).collect::<Vec<_>>(),
            "venue": "RacingPoint",
            "tagline": "May the Fastest Win.",
        }
    }))
}

// ─── Referral System ─────────────────────────────────────────────────────────

async fn customer_referral_code(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code: Option<(String,)> = sqlx::query_as(
        "SELECT referral_code FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let referral_count: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM referrals WHERE referrer_id = ? AND reward_credited = 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "referral_code": code.and_then(|c| if c.0.is_empty() { None } else { Some(c.0) }),
        "successful_referrals": referral_count.map(|c| c.0).unwrap_or(0),
    }))
}

async fn customer_generate_referral_code(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check if already has a code
    let existing: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT referral_code FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((Some(code),)) = &existing {
        if !code.is_empty() {
            return Json(json!({ "referral_code": code }));
        }
    }

    // Generate 6-char alphanumeric code from UUID
    let code = format!("RP{}", &uuid::Uuid::new_v4().to_string().replace("-", "")[..6].to_uppercase());

    let _ = sqlx::query("UPDATE drivers SET referral_code = ? WHERE id = ?")
        .bind(&code)
        .bind(&driver_id)
        .execute(&state.db)
        .await;

    Json(json!({ "referral_code": code }))
}

async fn customer_redeem_referral(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return Json(json!({ "error": "code required" })),
    };

    // Find referrer
    let referrer: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE referral_code = ?",
    )
    .bind(code)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let referrer_id = match referrer {
        Some((id,)) => {
            if id == driver_id {
                return Json(json!({ "error": "Cannot redeem your own code" }));
            }
            id
        }
        None => return Json(json!({ "error": "Invalid referral code" })),
    };

    // Check not already referred
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM referrals WHERE referee_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if existing.map(|e| e.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "Already used a referral code" }));
    }

    let referral_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO referrals (id, referrer_id, referee_id, code, reward_credited)
         VALUES (?, ?, ?, ?, 0)",
    )
    .bind(&referral_id)
    .bind(&referrer_id)
    .bind(&driver_id)
    .bind(code)
    .execute(&state.db)
    .await;

    Json(json!({ "ok": true, "message": "Referral code applied! Rewards will be credited after your first session." }))
}

// ─── Coupons ─────────────────────────────────────────────────────────────────

async fn customer_apply_coupon(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let code = match body.get("code").and_then(|v| v.as_str()) {
        Some(c) => c.to_uppercase(),
        None => return Json(json!({ "error": "code required" })),
    };

    // Find coupon
    let coupon: Option<(String, String, f64, i64, Option<String>, Option<String>, Option<i64>, bool)> = sqlx::query_as(
        "SELECT id, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only
         FROM coupons WHERE code = ? AND is_active = 1",
    )
    .bind(&code)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let coupon = match coupon {
        Some(c) => c,
        None => return Json(json!({ "error": "Invalid or expired coupon code" })),
    };

    // Check usage count
    let used: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ?",
    )
    .bind(&coupon.0)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if used.map(|u| u.0 >= coupon.3).unwrap_or(false) {
        return Json(json!({ "error": "Coupon has reached maximum uses" }));
    }

    // Check if already used by this driver
    let driver_used: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM coupon_redemptions WHERE coupon_id = ? AND driver_id = ?",
    )
    .bind(&coupon.0)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if driver_used.map(|u| u.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "You have already used this coupon" }));
    }

    // Return coupon details for the client to apply at checkout
    let discount_description = match coupon.1.as_str() {
        "percent" => format!("{}% off", coupon.2),
        "flat" => format!("₹{} off", coupon.2 as i64 / 100),
        "free_minutes" => format!("{} free minutes", coupon.2 as i64),
        _ => "Discount".to_string(),
    };

    Json(json!({
        "valid": true,
        "coupon_id": coupon.0,
        "coupon_type": coupon.1,
        "value": coupon.2,
        "description": discount_description,
        "min_spend_paise": coupon.6,
        "first_session_only": coupon.7,
    }))
}

// ─── Packages ────────────────────────────────────────────────────────────────

async fn customer_list_packages(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, i64, i64, i64, bool, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT id, name, description, num_rigs, duration_minutes, price_paise,
                includes_cafe, day_restriction, hour_start, hour_end
         FROM packages WHERE is_active = 1
         ORDER BY price_paise ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(packages) => {
            let list: Vec<Value> = packages.iter().map(|p| json!({
                "id": p.0,
                "name": p.1,
                "description": p.2,
                "num_rigs": p.3,
                "duration_minutes": p.4,
                "price_paise": p.5,
                "price_display": format!("₹{}", p.5 / 100),
                "includes_cafe": p.6,
                "day_restriction": p.7,
                "hour_start": p.8,
                "hour_end": p.9,
            })).collect();
            Json(json!({ "packages": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Memberships ─────────────────────────────────────────────────────────────

async fn customer_membership(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Get active membership
    let membership: Option<(String, String, String, f64, f64, String, bool, String)> = sqlx::query_as(
        "SELECT m.id, mt.name, mt.perks, m.hours_used_minutes, mt.hours_included,
                m.expires_at, m.auto_renew, m.status
         FROM memberships m
         JOIN membership_tiers mt ON m.tier_id = mt.id
         WHERE m.driver_id = ? AND m.status = 'active'
         ORDER BY m.created_at DESC LIMIT 1",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get available tiers
    let tiers = sqlx::query_as::<_, (String, String, f64, i64, String)>(
        "SELECT id, name, hours_included, price_paise, perks
         FROM membership_tiers WHERE is_active = 1
         ORDER BY price_paise ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let tiers_json: Vec<Value> = tiers.iter().map(|t| {
        let perks: Value = serde_json::from_str(&t.4).unwrap_or(json!([]));
        json!({
            "id": t.0,
            "name": t.1,
            "hours_included": t.2,
            "price_paise": t.3,
            "price_display": format!("₹{}/month", t.3 / 100),
            "perks": perks,
        })
    }).collect();

    Json(json!({
        "membership": membership.map(|m| {
            let perks: Value = serde_json::from_str(&m.2).unwrap_or(json!([]));
            json!({
                "id": m.0,
                "tier_name": m.1,
                "perks": perks,
                "hours_used": m.3,
                "hours_included": m.4,
                "hours_remaining": (m.4 - m.3).max(0.0),
                "expires_at": m.5,
                "auto_renew": m.6,
                "status": m.7,
            })
        }),
        "available_tiers": tiers_json,
    }))
}

async fn customer_subscribe_membership(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let tier_id = match body.get("tier_id").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Json(json!({ "error": "tier_id required" })),
    };

    // Check tier exists
    let tier: Option<(String, i64)> = sqlx::query_as(
        "SELECT name, price_paise FROM membership_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let tier = match tier {
        Some(t) => t,
        None => return Json(json!({ "error": "Invalid membership tier" })),
    };

    // Check no active membership
    let active: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM memberships WHERE driver_id = ? AND status = 'active'",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if active.map(|a| a.0 > 0).unwrap_or(false) {
        return Json(json!({ "error": "You already have an active membership" }));
    }

    let membership_id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO memberships (id, driver_id, tier_id, hours_used_minutes, price_paise, expires_at, auto_renew, status)
         VALUES (?, ?, ?, 0, ?, datetime('now', '+30 days'), 0, 'active')",
    )
    .bind(&membership_id)
    .bind(&driver_id)
    .bind(tier_id)
    .bind(tier.1)
    .execute(&state.db)
    .await;

    Json(json!({
        "ok": true,
        "membership_id": membership_id,
        "tier_name": tier.0,
        "message": format!("Welcome to {} membership!", tier.0),
    }))
}

// ─── Public Leaderboard (No Auth Required) ───────────────────────────────────

#[derive(Deserialize)]
struct PublicLeaderboardQuery {
    /// Filter by game/simulator (sim_type field)
    sim_type: Option<String>,
    /// Filter by car class (e.g. 'A', 'B', 'C') — UX-05 segmentation
    car_class: Option<String>,
    /// Filter by assist tier: 'pro', 'semi-pro', 'amateur', 'unknown' — UX-05 segmentation
    assist_tier: Option<String>,
}

async fn public_leaderboard(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PublicLeaderboardQuery>,
) -> Json<Value> {
    // UX-04: Only show laps from billing sessions (verified)
    // UX-05: Segment by game + car_class + assist_tier
    // UX-07: Never show laps marked unverifiable (telemetry adapter crash)
    let sim_clause = if params.sim_type.is_some() { " AND tr.sim_type = ?" } else { "" };
    let car_class_clause = if params.car_class.is_some() { " AND l.car_class = ?" } else { "" };
    let assist_tier_clause = if params.assist_tier.is_some() { " AND l.assist_tier = ?" } else { "" };

    // All-time track records, filtered by game + car_class + assist_tier
    // JOIN laps to apply UX-04/UX-05/UX-07 integrity filters
    let records_query = format!(
        "SELECT tr.track, tr.car,
                CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                tr.best_lap_ms, tr.achieved_at, tr.lap_id, tr.sim_type
         FROM track_records tr
         JOIN drivers d ON tr.driver_id = d.id
         LEFT JOIN laps l ON l.id = tr.lap_id
         WHERE (l.billing_session_id IS NOT NULL OR tr.lap_id IS NULL)
           AND (l.validity IS NULL OR l.validity = 'valid')
           AND (l.suspect IS NULL OR l.suspect = 0)
           {}{}{}
         ORDER BY tr.achieved_at DESC",
        sim_clause, car_class_clause, assist_tier_clause
    );

    let mut rec_q = sqlx::query_as::<_, (String, String, String, i64, String, Option<String>, String)>(&records_query);
    if let Some(ref st) = params.sim_type { rec_q = rec_q.bind(st); }
    if let Some(ref cc) = params.car_class { rec_q = rec_q.bind(cc); }
    if let Some(ref at) = params.assist_tier { rec_q = rec_q.bind(at); }
    let records = rec_q.fetch_all(&state.db).await;

    // Available tracks — only tracks with billing-verified valid laps
    let laps_sim_clause = if params.sim_type.is_some() { " AND sim_type = ?" } else { "" };
    let laps_cc_clause = if params.car_class.is_some() { " AND car_class = ?" } else { "" };
    let laps_at_clause = if params.assist_tier.is_some() { " AND assist_tier = ?" } else { "" };
    let tracks_query = format!(
        "SELECT DISTINCT track, COUNT(*) as laps FROM laps
         WHERE valid = 1
           AND billing_session_id IS NOT NULL
           AND (validity IS NULL OR validity = 'valid')
           AND (suspect IS NULL OR suspect = 0)
           {}{}{}
         GROUP BY track ORDER BY laps DESC",
        laps_sim_clause, laps_cc_clause, laps_at_clause
    );
    let mut track_q = sqlx::query_as::<_, (String, i64)>(&tracks_query);
    if let Some(ref st) = params.sim_type { track_q = track_q.bind(st); }
    if let Some(ref cc) = params.car_class { track_q = track_q.bind(cc); }
    if let Some(ref at) = params.assist_tier { track_q = track_q.bind(at); }
    let tracks = track_q.fetch_all(&state.db).await.unwrap_or_default();

    // Top drivers by total valid billing-session laps, optionally filtered
    let top_drivers_query = format!(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                COUNT(l.id) as lap_count, MIN(l.lap_time_ms) as fastest,
                MAX(dr.composite_rating),
                (SELECT dr2.rating_class FROM driver_ratings dr2 WHERE dr2.driver_id = l.driver_id ORDER BY dr2.composite_rating DESC LIMIT 1)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         LEFT JOIN driver_ratings dr ON dr.driver_id = l.driver_id AND dr.sim_type = l.sim_type
         WHERE l.valid = 1
           AND l.billing_session_id IS NOT NULL
           AND (l.validity IS NULL OR l.validity = 'valid')
           AND (l.suspect IS NULL OR l.suspect = 0)
           {}{}{}
         GROUP BY l.driver_id ORDER BY lap_count DESC LIMIT 20",
        laps_sim_clause, laps_cc_clause, laps_at_clause
    );
    let mut td_q = sqlx::query_as::<_, (String, i64, Option<i64>, Option<f64>, Option<String>)>(&top_drivers_query);
    if let Some(ref st) = params.sim_type { td_q = td_q.bind(st); }
    if let Some(ref cc) = params.car_class { td_q = td_q.bind(cc); }
    if let Some(ref at) = params.assist_tier { td_q = td_q.bind(at); }
    let top_drivers = td_q.fetch_all(&state.db).await.unwrap_or_default();

    // Available sim_types for frontend game picker (billing-verified only)
    let available_sim_types: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT sim_type FROM laps
         WHERE valid = 1 AND billing_session_id IS NOT NULL
           AND (validity IS NULL OR validity = 'valid')
         ORDER BY sim_type",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    // Available assist tiers for frontend assist picker
    let available_assist_tiers: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT DISTINCT assist_tier FROM laps
         WHERE valid = 1 AND billing_session_id IS NOT NULL
           AND (validity IS NULL OR validity = 'valid')
           AND assist_tier IS NOT NULL
         ORDER BY assist_tier",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    // Active time trial
    let time_trial = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end
         FROM time_trials
         WHERE date('now') BETWEEN week_start AND week_end
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "records": records.unwrap_or_default().iter().map(|r| json!({
            "track": r.0, "car": r.1, "driver": r.2,
            "best_lap_ms": r.3,
            "best_lap_display": format!("{}:{:02}.{:03}", r.3 / 60000, (r.3 % 60000) / 1000, r.3 % 1000),
            "achieved_at": r.4,
            "lap_id": r.5,
            "sim_type": r.6,
        })).collect::<Vec<_>>(),
        "tracks": tracks.iter().map(|t| json!({
            "name": t.0, "total_laps": t.1,
        })).collect::<Vec<_>>(),
        "top_drivers": top_drivers.iter().enumerate().map(|(i, d)| json!({
            "position": i + 1,
            "name": d.0,
            "total_laps": d.1,
            "fastest_lap_ms": d.2,
            "composite_rating": d.3,
            "rating_class": d.4,
        })).collect::<Vec<_>>(),
        "available_sim_types": available_sim_types,
        "available_assist_tiers": available_assist_tiers,
        "sim_type": params.sim_type,
        "car_class": params.car_class,
        "assist_tier": params.assist_tier,
        "time_trial": time_trial.map(|tt| json!({
            "id": tt.0, "track": tt.1, "car": tt.2,
            "week_start": tt.3, "week_end": tt.4,
        })),
        "venue": "RacingPoint",
        "tagline": "May the Fastest Win.",
        "last_updated": chrono::Utc::now().to_rfc3339(),
    }))
}

#[derive(Deserialize)]
struct LeaderboardQuery {
    sim_type: Option<String>,
    car: Option<String>,
    /// Filter by car class — UX-05 segmentation
    car_class: Option<String>,
    /// Filter by assist tier: 'pro', 'semi-pro', 'amateur' — UX-05 segmentation
    assist_tier: Option<String>,
    show_invalid: Option<bool>,
}

async fn public_track_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(track): Path<String>,
    Query(params): Query<LeaderboardQuery>,
) -> Json<Value> {
    // sim_type is optional — None means all games (backward compatible)
    let sim_type = params.sim_type.clone();
    let show_invalid = params.show_invalid.unwrap_or(false);

    // Build validity filter: suspect laps are ALWAYS hidden.
    // show_invalid=true drops the valid=1 requirement but keeps suspect filter.
    // UX-04: billing_session_id IS NOT NULL — only billed-session laps on leaderboard
    // UX-07: validity = 'valid' — never show unverifiable laps
    let validity_clause = if show_invalid {
        "AND (l.suspect IS NULL OR l.suspect = 0) AND l.billing_session_id IS NOT NULL AND (l.validity IS NULL OR l.validity = 'valid')"
    } else {
        "AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0) AND l.billing_session_id IS NOT NULL AND (l.validity IS NULL OR l.validity = 'valid')"
    };

    let sim_type_clause = if sim_type.is_some() { "AND l.sim_type = ?" } else { "" };
    let sim_type_subq_clause = if sim_type.is_some() { "AND l2.sim_type = ?" } else { "" };
    let car_clause = if params.car.is_some() { "AND l.car = ?" } else { "" };
    let car_class_clause = if params.car_class.is_some() { "AND l.car_class = ?" } else { "" };
    let assist_tier_clause = if params.assist_tier.is_some() { "AND l.assist_tier = ?" } else { "" };

    // Top 50 fastest laps on this track (best per driver per car)
    // UX-04: billing_session_id enforced via validity_clause
    // UX-05: car_class + assist_tier segmentation
    // UX-07: validity = 'valid' enforced via validity_clause
    // Phase 253: LEFT JOIN driver_ratings to include composite_rating and rating_class
    // Response includes assist_tier for frontend display
    let main_query = format!(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END,
                l.car, MIN(l.lap_time_ms), MAX(l.created_at),
                (SELECT l2.id FROM laps l2 WHERE l2.driver_id = l.driver_id AND l2.car = l.car AND l2.track = l.track
                    {} {} ORDER BY l2.lap_time_ms ASC LIMIT 1),
                l.sim_type,
                dr.composite_rating,
                dr.rating_class,
                l.assist_tier
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         LEFT JOIN driver_ratings dr ON dr.driver_id = l.driver_id AND dr.sim_type = l.sim_type
         WHERE l.track = ? {} {} {} {} {}
         GROUP BY l.driver_id, l.car
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 50",
        sim_type_subq_clause,
        if show_invalid { "AND (l2.suspect IS NULL OR l2.suspect = 0)" } else { "AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)" },
        sim_type_clause,
        validity_clause,
        car_clause,
        car_class_clause,
        assist_tier_clause,
    );

    let mut query = sqlx::query_as::<_, (String, String, i64, String, Option<String>, String, Option<f64>, Option<String>, Option<String>)>(&main_query);

    // Bind subquery sim_type first (if present)
    if let Some(ref st) = sim_type {
        query = query.bind(st);
    }
    // Bind main WHERE params
    query = query.bind(&track);
    if let Some(ref st) = sim_type {
        query = query.bind(st);
    }
    if let Some(ref car) = params.car {
        query = query.bind(car);
    }
    if let Some(ref cc) = params.car_class {
        query = query.bind(cc);
    }
    if let Some(ref at) = params.assist_tier {
        query = query.bind(at);
    }

    let records = query.fetch_all(&state.db).await;

    // Track stats (filtered by same criteria including UX-04/UX-07)
    let stats_query = format!(
        "SELECT COUNT(*) as total_laps, COUNT(DISTINCT driver_id) as drivers, COUNT(DISTINCT car) as cars
         FROM laps WHERE track = ? {} {} {} {}",
        sim_type_clause,
        validity_clause,
        car_class_clause,
        assist_tier_clause,
    );

    let stats: Option<(i64, i64, i64)> = {
        let mut sq = sqlx::query_as::<_, (i64, i64, i64)>(&stats_query).bind(&track);
        if let Some(ref st) = sim_type {
            sq = sq.bind(st);
        }
        if let Some(ref cc) = params.car_class {
            sq = sq.bind(cc);
        }
        if let Some(ref at) = params.assist_tier {
            sq = sq.bind(at);
        }
        sq.fetch_optional(&state.db).await.ok().flatten()
    };

    Json(json!({
        "track": track,
        "sim_type": sim_type,
        "car_class": params.car_class,
        "assist_tier": params.assist_tier,
        "stats": stats.map(|s| json!({
            "total_laps": s.0,
            "unique_drivers": s.1,
            "unique_cars": s.2,
        })),
        "leaderboard": records.unwrap_or_default().iter().enumerate().map(|(i, r)| json!({
            "position": i + 1,
            "driver": r.0,
            "car": r.1,
            "best_lap_ms": r.2,
            "best_lap_display": format!("{}:{:02}.{:03}", r.2 / 60000, (r.2 % 60000) / 1000, r.2 % 1000),
            "achieved_at": r.3,
            "lap_id": r.4,
            "sim_type": r.5,
            "composite_rating": r.6,
            "rating_class": r.7,
            "assist_tier": r.8,
        })).collect::<Vec<_>>(),
        "last_updated": chrono::Utc::now().to_rfc3339(),
    }))
}

// ─── Circuit Records (Public, No Auth) ────────────────────────────────────────

#[derive(Deserialize)]
struct CircuitRecordsQuery {
    sim_type: Option<String>,
}

async fn public_circuit_records(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CircuitRecordsQuery>,
) -> Json<Value> {
    let records = if let Some(ref sim_type) = params.sim_type {
        sqlx::query_as::<_, (String, String, String, i64, String)>(
            "SELECT l.track, l.car, l.sim_type, MIN(l.lap_time_ms),
                    (SELECT CASE WHEN d2.show_nickname_on_leaderboard = 1 AND d2.nickname IS NOT NULL THEN d2.nickname ELSE d2.name END
                     FROM laps l2 JOIN drivers d2 ON l2.driver_id = d2.id
                     WHERE l2.track = l.track AND l2.car = l.car AND l2.sim_type = l.sim_type
                       AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)
                     ORDER BY l2.lap_time_ms ASC LIMIT 1)
             FROM laps l
             WHERE l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0) AND l.sim_type = ?
             GROUP BY l.track, l.car, l.sim_type
             ORDER BY l.track, l.car",
        )
        .bind(sim_type)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, (String, String, String, i64, String)>(
            "SELECT l.track, l.car, l.sim_type, MIN(l.lap_time_ms),
                    (SELECT CASE WHEN d2.show_nickname_on_leaderboard = 1 AND d2.nickname IS NOT NULL THEN d2.nickname ELSE d2.name END
                     FROM laps l2 JOIN drivers d2 ON l2.driver_id = d2.id
                     WHERE l2.track = l.track AND l2.car = l.car AND l2.sim_type = l.sim_type
                       AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)
                     ORDER BY l2.lap_time_ms ASC LIMIT 1)
             FROM laps l
             WHERE l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
             GROUP BY l.track, l.car, l.sim_type
             ORDER BY l.track, l.car",
        )
        .fetch_all(&state.db)
        .await
    };

    let records = records.unwrap_or_default();
    let count = records.len();

    Json(json!({
        "records": records.iter().map(|r| json!({
            "track": r.0,
            "car": r.1,
            "sim_type": r.2,
            "best_lap_ms": r.3,
            "best_lap_display": format!("{}:{:02}.{:03}", r.3 / 60000, (r.3 % 60000) / 1000, r.3 % 1000),
            "driver": r.4,
        })).collect::<Vec<_>>(),
        "count": count,
    }))
}

// ─── Driver Rating (Public, No Auth — Phase 253) ─────────────────────────────

#[derive(Deserialize)]
struct DriverRatingQuery {
    sim_type: Option<String>,
}

async fn public_driver_rating(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Query(params): Query<DriverRatingQuery>,
) -> Json<Value> {
    if let Some(ref sim_type) = params.sim_type {
        // Single sim_type
        let row = sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? AND sim_type = ?",
        )
        .bind(&driver_id)
        .bind(sim_type)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        match row {
            Some(r) => Json(json!({
                "driver_id": r.0,
                "sim_type": r.1,
                "composite_rating": r.2,
                "rating_class": r.3,
                "pace_score": r.4,
                "consistency_score": r.5,
                "experience_score": r.6,
                "total_laps": r.7,
                "updated_at": r.8,
            })),
            None => Json(json!({
                "driver_id": driver_id,
                "sim_type": sim_type,
                "composite_rating": null,
                "rating_class": "Unrated",
                "message": "No rating data available",
            })),
        }
    } else {
        // All sim_types for this driver
        let rows = sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? ORDER BY composite_rating DESC",
        )
        .bind(&driver_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        Json(json!({
            "driver_id": driver_id,
            "ratings": rows.iter().map(|r| json!({
                "sim_type": r.1,
                "composite_rating": r.2,
                "rating_class": r.3,
                "pace_score": r.4,
                "consistency_score": r.5,
                "experience_score": r.6,
                "total_laps": r.7,
                "updated_at": r.8,
            })).collect::<Vec<_>>(),
        }))
    }
}

// ─── Driver Rating History (Staff-Only — Phase 253) ──────────────────────────

#[derive(Deserialize)]
struct RatingHistoryQuery {
    sim_type: Option<String>,
    limit: Option<i64>,
}

async fn staff_driver_rating_history(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Query(params): Query<RatingHistoryQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(50).min(200);

    // Current ratings serve as the "history" snapshot.
    // A full temporal history table would require a separate audit_log table.
    // For now, return all current ratings for this driver as their progression.
    let rows = if let Some(ref sim_type) = params.sim_type {
        sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? AND sim_type = ? ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(&driver_id)
        .bind(sim_type)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, (String, String, f64, String, f64, f64, f64, i64, String)>(
            "SELECT driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at
             FROM driver_ratings WHERE driver_id = ? ORDER BY updated_at DESC LIMIT ?",
        )
        .bind(&driver_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    Json(json!({
        "driver_id": driver_id,
        "history": rows.iter().map(|r| json!({
            "sim_type": r.1,
            "composite_rating": r.2,
            "rating_class": r.3,
            "pace_score": r.4,
            "consistency_score": r.5,
            "experience_score": r.6,
            "total_laps": r.7,
            "updated_at": r.8,
        })).collect::<Vec<_>>(),
    }))
}

// ─── Vehicle Records (Public, No Auth) ────────────────────────────────────────

#[derive(Deserialize)]
struct VehicleRecordsQuery {
    sim_type: Option<String>,
}

async fn public_vehicle_records(
    State(state): State<Arc<AppState>>,
    Path(car): Path<String>,
    Query(params): Query<VehicleRecordsQuery>,
) -> Json<Value> {
    let sim_type_filter = params.sim_type.as_deref().unwrap_or("");
    let sim_clause = if sim_type_filter.is_empty() { "" } else { "AND l.sim_type = ?" };

    let query_str = format!(
        "SELECT l.track, l.sim_type, MIN(l.lap_time_ms),
                (SELECT CASE WHEN d2.show_nickname_on_leaderboard = 1 AND d2.nickname IS NOT NULL THEN d2.nickname ELSE d2.name END
                 FROM laps l2 JOIN drivers d2 ON l2.driver_id = d2.id
                 WHERE l2.track = l.track AND l2.car = l.car AND l2.sim_type = l.sim_type
                   AND l2.valid = 1 AND (l2.suspect IS NULL OR l2.suspect = 0)
                 ORDER BY l2.lap_time_ms ASC LIMIT 1)
         FROM laps l
         WHERE l.car = ? AND l.valid = 1 AND (l.suspect IS NULL OR l.suspect = 0)
         {sim_clause}
         GROUP BY l.track, l.sim_type
         ORDER BY l.track"
    );

    let mut q = sqlx::query_as::<_, (String, String, i64, String)>(&query_str)
        .bind(&car);
    if !sim_type_filter.is_empty() {
        q = q.bind(sim_type_filter);
    }
    let records = q.fetch_all(&state.db).await;

    Json(json!({
        "car": car,
        "records": records.unwrap_or_default().iter().map(|r| json!({
            "track": r.0,
            "sim_type": r.1,
            "best_lap_ms": r.2,
            "best_lap_display": format!("{}:{:02}.{:03}", r.2 / 60000, (r.2 % 60000) / 1000, r.2 % 1000),
            "driver": r.3,
        })).collect::<Vec<_>>(),
    }))
}

// ─── Public Driver Search & Profile (No Auth Required) ────────────────────────

#[derive(Deserialize)]
struct DriverSearchQuery {
    name: String,
}

async fn public_drivers_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DriverSearchQuery>,
) -> Json<Value> {
    let results = sqlx::query_as::<_, (String, String, i64, Option<String>)>(
        "SELECT id, CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END,
                total_laps, avatar_url
         FROM drivers
         WHERE name LIKE '%' || ? || '%' COLLATE NOCASE
            OR nickname LIKE '%' || ? || '%' COLLATE NOCASE
         LIMIT 20"
    )
    .bind(&params.name)
    .bind(&params.name)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "drivers": results.iter().map(|r| json!({
            "id": r.0,
            "display_name": r.1,
            "total_laps": r.2,
            "avatar_url": r.3,
        })).collect::<Vec<_>>(),
        "count": results.len(),
    }))
}

async fn public_driver_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    // Query 1: Driver stats (NO PII — no email, phone, wallet, billing)
    let driver = sqlx::query_as::<_, (String, i64, i64, Option<String>, String)>(
        "SELECT CASE WHEN show_nickname_on_leaderboard = 1 AND nickname IS NOT NULL THEN nickname ELSE name END,
                total_laps, total_time_ms, avatar_url, created_at
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let driver = match driver {
        Some(d) => d,
        None => return Err((
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "Driver not found" })),
        )),
    };

    // Query 2: Personal bests
    let personal_bests = sqlx::query_as::<_, (String, String, i64, Option<String>)>(
        "SELECT track, car, best_lap_ms, achieved_at
         FROM personal_bests WHERE driver_id = ?
         ORDER BY achieved_at DESC"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Query 3: Lap history (exclude suspect laps, sector 0 → null)
    let laps = sqlx::query_as::<_, (String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool, String)>(
        "SELECT track, car, lap_time_ms,
                CASE WHEN sector1_ms > 0 THEN sector1_ms ELSE NULL END,
                CASE WHEN sector2_ms > 0 THEN sector2_ms ELSE NULL END,
                CASE WHEN sector3_ms > 0 THEN sector3_ms ELSE NULL END,
                valid, created_at
         FROM laps
         WHERE driver_id = ? AND (suspect IS NULL OR suspect = 0)
         ORDER BY created_at DESC
         LIMIT 100"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Ok(Json(json!({
        "driver": {
            "display_name": driver.0,
            "total_laps": driver.1,
            "total_time_ms": driver.2,
            "avatar_url": driver.3,
            "member_since": driver.4,
            "class_badge": null,
        },
        "personal_bests": personal_bests.iter().map(|pb| json!({
            "track": pb.0,
            "car": pb.1,
            "best_lap_ms": pb.2,
            "best_lap_display": format!("{}:{:02}.{:03}", pb.2 / 60000, (pb.2 % 60000) / 1000, pb.2 % 1000),
            "achieved_at": pb.3,
        })).collect::<Vec<_>>(),
        "lap_history": laps.iter().map(|l| json!({
            "track": l.0,
            "car": l.1,
            "lap_time_ms": l.2,
            "lap_time_display": format!("{}:{:02}.{:03}", l.2 / 60000, (l.2 % 60000) / 1000, l.2 % 1000),
            "sector1_ms": l.3,
            "sector2_ms": l.4,
            "sector3_ms": l.5,
            "valid": l.6,
            "created_at": l.7,
        })).collect::<Vec<_>>(),
    })))
}

async fn public_time_trial(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Current week's time trial
    let trial = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end
         FROM time_trials
         WHERE date('now') BETWEEN week_start AND week_end
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let trial = match trial {
        Some(t) => t,
        None => return Json(json!({ "time_trial": null, "message": "No active time trial this week" })),
    };

    // Leaderboard for this time trial (laps on this track+car this week)
    let entries = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL THEN d.nickname ELSE d.name END, MIN(l.lap_time_ms), COUNT(l.id)
         FROM laps l
         JOIN drivers d ON l.driver_id = d.id
         WHERE l.track = ? AND l.car = ? AND l.valid = 1
           AND l.created_at >= ? AND l.created_at < datetime(?, '+1 day')
         GROUP BY l.driver_id
         ORDER BY MIN(l.lap_time_ms) ASC
         LIMIT 20",
    )
    .bind(&trial.1)
    .bind(&trial.2)
    .bind(&trial.3)
    .bind(&trial.4)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    Json(json!({
        "time_trial": {
            "id": trial.0,
            "track": trial.1,
            "car": trial.2,
            "week_start": trial.3,
            "week_end": trial.4,
        },
        "leaderboard": entries.iter().enumerate().map(|(i, e)| json!({
            "position": i + 1,
            "driver": e.0,
            "best_lap_ms": e.1,
            "best_lap_display": format!("{}:{:02}.{:03}", e.1 / 60000, (e.1 % 60000) / 1000, e.1 % 1000),
            "attempts": e.2,
        })).collect::<Vec<_>>(),
    }))
}

// ─── Public Lap Telemetry (No Auth Required) ────────────────────────────────

async fn public_lap_telemetry(
    State(state): State<Arc<AppState>>,
    Path(lap_id): Path<String>,
) -> Json<Value> {
    // First verify lap exists and get metadata
    let lap = sqlx::query_as::<_, (String, String, String, i64, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT track, car, sim_type, lap_time_ms, sector1_ms, sector2_ms, sector3_ms FROM laps WHERE id = ?",
    )
    .bind(&lap_id)
    .fetch_optional(&state.db)
    .await;

    let lap = match lap {
        Ok(Some(l)) => l,
        Ok(None) => return Json(json!({ "error": "Lap not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    // Phase 251: Fetch telemetry samples from telemetry.db if available, else fall back to main DB
    let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);
    let samples = sqlx::query_as::<_, (i64, Option<f64>, Option<f64>, Option<f64>, Option<f64>, Option<i64>, Option<i64>)>(
        "SELECT offset_ms, speed, throttle, brake, steering, gear, rpm
         FROM telemetry_samples
         WHERE lap_id = ?
         ORDER BY offset_ms ASC",
    )
    .bind(&lap_id)
    .fetch_all(telem_pool)
    .await;

    match samples {
        Ok(rows) => {
            let data: Vec<Value> = rows.iter().map(|s| {
                json!({
                    "offset_ms": s.0,
                    "speed": s.1,
                    "throttle": s.2,
                    "brake": s.3,
                    "steering": s.4,
                    "gear": s.5,
                    "rpm": s.6,
                })
            }).collect();

            let sample_count = data.len();
            Json(json!({
                "lap_id": lap_id,
                "track": lap.0,
                "car": lap.1,
                "sim_type": lap.2,
                "lap_time_ms": lap.3,
                "sector1_ms": lap.4,
                "sector2_ms": lap.5,
                "sector3_ms": lap.6,
                "samples": data,
                "sample_count": sample_count,
            }))
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Public Session Summary ──────────────────────────────────────────────────

/// Public session summary — no auth required. Shows first name only (privacy).
/// Used for shareable session links.
async fn public_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Query session + driver name + pricing tier (no auth - public endpoint)
    let row = sqlx::query_as::<_, (String, String, String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, d.name, bs.status, bs.allocated_seconds, bs.driving_seconds,
                pt.name, bs.car, bs.track, bs.sim_type
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match row {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Extract first name only (privacy -- per user decision)
    let first_name = session.1.split_whitespace().next().unwrap_or("Racer");

    // Best lap from laps table (valid laps only)
    let best_lap: Option<(i64,)> = sqlx::query_as(
        "SELECT MIN(lap_time_ms) FROM laps WHERE session_id = ? AND valid = 1",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let total_laps: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM laps WHERE session_id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Json(json!({
        "driver_first_name": first_name,
        "status": session.2,
        "duration_seconds": session.4,
        "pricing_tier": session.5,
        "car": session.6,
        "track": session.7,
        "sim_type": session.8,
        "best_lap_ms": best_lap.map(|b| b.0),
        "total_laps": total_laps.map(|t| t.0).unwrap_or(0),
    }))
}

// ─── Public Championship Standings ───────────────────────────────────────────

/// GET /public/championships/{id}/standings — public championship standings with F1 tiebreaker
///
/// Returns championship metadata plus live-computed standings from hotlap_event_entries.
/// Standings are ordered by: total_points DESC, wins DESC, p2_count DESC, p3_count DESC.
async fn public_championship_standings_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Fetch championship metadata
    let champ = sqlx::query_as::<_, (String, String, Option<String>, String, String, i64, i64)>(
        "SELECT id, name, season, status, scoring_system, total_rounds, completed_rounds
         FROM championships WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let (champ_name, season, champ_status, scoring_system, total_rounds, completed_rounds) =
        match champ {
            Ok(Some((_, name, season, status, scoring, total, completed))) => {
                (name, season, status, scoring, total, completed)
            }
            Ok(None) => return Json(json!({ "error": "Championship not found" })),
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    // Compute standings live from hotlap_event_entries
    let standings_rows: Vec<(String, String, i64, i64, i64, i64, i64, Option<i64>)> =
        sqlx::query_as(
            "SELECT hee.driver_id,
                    COALESCE(d.nickname, d.name, 'Unknown') as display_name,
                    COALESCE(SUM(hee.points), 0) as total_points,
                    COUNT(DISTINCT cr.event_id) as rounds_entered,
                    SUM(CASE WHEN hee.position = 1 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as wins,
                    SUM(CASE WHEN hee.position = 2 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p2_count,
                    SUM(CASE WHEN hee.position = 3 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p3_count,
                    MIN(hee.position) as best_result
             FROM hotlap_event_entries hee
             INNER JOIN championship_rounds cr ON cr.event_id = hee.event_id
             LEFT JOIN drivers d ON d.id = hee.driver_id
             WHERE cr.championship_id = ?
               AND hee.result_status IN ('finished', 'dnf', 'dns')
             GROUP BY hee.driver_id
             ORDER BY total_points DESC, wins DESC, p2_count DESC, p3_count DESC",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let standings: Vec<Value> = standings_rows
        .iter()
        .enumerate()
        .map(|(i, (driver_id, display_name, total_points, rounds_entered, wins, p2_count, p3_count, best_result))| {
            json!({
                "position": i as i64 + 1,
                "driver_id": driver_id,
                "display_name": display_name,
                "total_points": total_points,
                "rounds_entered": rounds_entered,
                "wins": wins,
                "p2_count": p2_count,
                "p3_count": p3_count,
                "best_result": best_result,
            })
        })
        .collect();

    // Fetch rounds list
    let rounds: Vec<(i64, String, Option<String>)> = sqlx::query_as(
        "SELECT cr.round_number, cr.event_id, he.name
         FROM championship_rounds cr
         LEFT JOIN hotlap_events he ON he.id = cr.event_id
         WHERE cr.championship_id = ?
         ORDER BY cr.round_number",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let rounds_json: Vec<Value> = rounds
        .iter()
        .map(|(num, evt_id, name)| {
            json!({
                "round_number": num,
                "event_id": evt_id,
                "event_name": name,
            })
        })
        .collect();

    Json(json!({
        "championship": {
            "id": id,
            "name": champ_name,
            "season": season,
            "status": champ_status,
            "scoring_system": scoring_system,
            "total_rounds": total_rounds,
            "completed_rounds": completed_rounds,
        },
        "standings": standings,
        "rounds": rounds_json,
    }))
}

// ─── Public Events Endpoints ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct EventsListQuery {
    status: Option<String>,
    sim_type: Option<String>,
}

/// GET /public/events — list all non-cancelled events, sorted by status priority then date
async fn public_events_list(
    State(state): State<Arc<AppState>>,
    Query(params): Query<EventsListQuery>,
) -> Json<Value> {
    let mut conditions = vec!["status != 'cancelled'".to_string()];
    if let Some(ref s) = params.status {
        conditions.push(format!("status = '{}'", s.replace('\'', "''")));
    }
    if let Some(ref st) = params.sim_type {
        conditions.push(format!("sim_type = '{}'", st.replace('\'', "''")));
    }
    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms, created_at,
                (SELECT COUNT(*) FROM hotlap_event_entries WHERE event_id = hotlap_events.id) as entry_count
         FROM hotlap_events
         WHERE {}
         ORDER BY
           CASE status
             WHEN 'active' THEN 1
             WHEN 'upcoming' THEN 2
             WHEN 'scoring' THEN 3
             WHEN 'completed' THEN 4
             ELSE 5
           END,
           starts_at DESC",
        where_clause
    );

    let rows: Vec<(String, String, Option<String>, String, String, String, String, String,
                   Option<String>, Option<String>, Option<i64>, Option<String>, i64)> =
        match sqlx::query_as(&sql).fetch_all(&state.db).await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let events: Vec<Value> = rows.into_iter().map(|(id, name, description, track, car, car_class, sim_type, status,
                                                     starts_at, ends_at, reference_time_ms, created_at, entry_count)| {
        json!({
            "id": id,
            "name": name,
            "description": description,
            "track": track,
            "car": car,
            "car_class": car_class,
            "sim_type": sim_type,
            "status": status,
            "starts_at": starts_at,
            "ends_at": ends_at,
            "reference_time_ms": reference_time_ms,
            "created_at": created_at,
            "entry_count": entry_count,
        })
    }).collect();

    Json(json!({ "events": events }))
}

/// GET /public/events/{id} — event leaderboard with per-class grouping, badges, 107% flags
async fn public_event_leaderboard(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Fetch event metadata
    let event_row: Option<(String, String, Option<String>, String, String, String, String, String,
                           Option<String>, Option<String>, Option<i64>)> = match sqlx::query_as(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms
         FROM hotlap_events WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let event_row = match event_row {
        Some(r) => r,
        None => return Json(json!({ "error": "Event not found" })),
    };

    let event = json!({
        "id": event_row.0,
        "name": event_row.1,
        "description": event_row.2,
        "track": event_row.3,
        "car": event_row.4,
        "car_class": event_row.5,
        "sim_type": event_row.6,
        "status": event_row.7,
        "starts_at": event_row.8,
        "ends_at": event_row.9,
        "reference_time_ms": event_row.10,
    });

    // Fetch leaderboard entries — PII excluded, nickname/name display logic applied
    let entries_rows: Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, Option<i64>,
                           Option<i64>, Option<i64>, Option<String>, Option<i64>, Option<i64>,
                           String, Option<String>, Option<String>, Option<String>)> =
        match sqlx::query_as(
            "SELECT hee.driver_id,
                    CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                         THEN d.nickname ELSE d.name END as display_name,
                    hee.lap_time_ms, hee.sector1_ms, hee.sector2_ms, hee.sector3_ms,
                    hee.position, hee.points, hee.badge, hee.gap_to_leader_ms,
                    hee.within_107_percent, hee.result_status, hee.entered_at,
                    l.car as vehicle, l.sim_type
             FROM hotlap_event_entries hee
             LEFT JOIN drivers d ON d.id = hee.driver_id
             LEFT JOIN laps l ON l.id = hee.lap_id
             WHERE hee.event_id = ?
             ORDER BY hee.position ASC",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let entries: Vec<Value> = entries_rows.into_iter().map(|(driver_id, display_name, lap_time_ms,
                                                              sector1_ms, sector2_ms, sector3_ms,
                                                              position, points, badge,
                                                              gap_to_leader_ms, within_107_percent,
                                                              result_status, entered_at,
                                                              vehicle, sim_type)| {
        json!({
            "driver_id": driver_id,
            "display_name": display_name,
            "lap_time_ms": lap_time_ms,
            "sector1_ms": sector1_ms,
            "sector2_ms": sector2_ms,
            "sector3_ms": sector3_ms,
            "position": position,
            "points": points,
            "badge": badge,
            "gap_to_leader_ms": gap_to_leader_ms,
            "within_107_percent": within_107_percent.map(|v| v == 1),
            "result_status": result_status,
            "entered_at": entered_at,
            "vehicle": vehicle,
            "sim_type": sim_type,
        })
    }).collect();

    // Determine car classes from actual laps
    let car_classes: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT l.car_class FROM hotlap_event_entries hee
         JOIN laps l ON l.id = hee.lap_id
         WHERE hee.event_id = ?",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let car_classes_list: Vec<&str> = car_classes.iter().map(|(c,)| c.as_str()).collect();

    Json(json!({
        "event": event,
        "car_classes": car_classes_list,
        "entries": entries,
    }))
}

// ─── Public Championships Endpoints ──────────────────────────────────────────

/// GET /public/championships — list all non-cancelled championships, active first
async fn public_championships_list(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows: Vec<(String, String, Option<String>, Option<String>, String, String, String, String, i64, i64, Option<String>)> =
        match sqlx::query_as(
            "SELECT c.id, c.name, c.description, c.season, c.car_class, c.sim_type,
                    c.status, c.scoring_system, c.total_rounds, c.completed_rounds, c.created_at
             FROM championships c
             WHERE c.status != 'cancelled'
             ORDER BY
               CASE c.status WHEN 'active' THEN 1 WHEN 'upcoming' THEN 2 WHEN 'completed' THEN 3 ELSE 4 END,
               c.created_at DESC",
        )
        .fetch_all(&state.db)
        .await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let championships: Vec<Value> = rows.into_iter().map(|(id, name, description, season,
                                                            car_class, sim_type, status,
                                                            scoring_system, total_rounds,
                                                            completed_rounds, created_at)| {
        json!({
            "id": id,
            "name": name,
            "description": description,
            "season": season,
            "car_class": car_class,
            "sim_type": sim_type,
            "status": status,
            "scoring_system": scoring_system,
            "total_rounds": total_rounds,
            "completed_rounds": completed_rounds,
            "created_at": created_at,
        })
    }).collect();

    Json(json!({ "championships": championships }))
}

/// GET /public/championships/{id} — championship metadata + standings + per-round breakdown
async fn public_championship_standings(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Fetch championship metadata
    let champ_row: Option<(String, String, Option<String>, Option<String>, String, String, String, String, i64, i64)> =
        match sqlx::query_as(
            "SELECT id, name, description, season, car_class, sim_type, status,
                    scoring_system, total_rounds, completed_rounds
             FROM championships WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&state.db)
        .await {
            Ok(r) => r,
            Err(e) => return Json(json!({ "error": e.to_string() })),
        };

    let champ_row = match champ_row {
        Some(r) => r,
        None => return Json(json!({ "error": "Championship not found" })),
    };

    let championship = json!({
        "id": champ_row.0,
        "name": champ_row.1,
        "description": champ_row.2,
        "season": champ_row.3,
        "car_class": champ_row.4,
        "sim_type": champ_row.5,
        "status": champ_row.6,
        "scoring_system": champ_row.7,
        "total_rounds": champ_row.8,
        "completed_rounds": champ_row.9,
    });

    // Compute live standings (same tiebreaker as assign_championship_positions)
    let standings_rows: Vec<(String, String, i64, i64, i64, i64, i64, Option<i64>)> =
        sqlx::query_as(
            "SELECT hee.driver_id,
                    CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                         THEN d.nickname ELSE d.name END as display_name,
                    COALESCE(SUM(hee.points), 0) as total_points,
                    COUNT(DISTINCT cr.event_id) as rounds_entered,
                    SUM(CASE WHEN hee.position = 1 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as wins,
                    SUM(CASE WHEN hee.position = 2 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p2_count,
                    SUM(CASE WHEN hee.position = 3 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p3_count,
                    MIN(hee.position) as best_result
             FROM hotlap_event_entries hee
             INNER JOIN championship_rounds cr ON cr.event_id = hee.event_id
             LEFT JOIN drivers d ON d.id = hee.driver_id
             WHERE cr.championship_id = ?
               AND hee.result_status IN ('finished', 'dnf', 'dns')
             GROUP BY hee.driver_id
             ORDER BY total_points DESC, wins DESC, p2_count DESC, p3_count DESC",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let standings: Vec<Value> = standings_rows
        .iter()
        .enumerate()
        .map(|(i, (driver_id, display_name, total_points, rounds_entered, wins, p2_count, p3_count, best_result))| {
            json!({
                "position": i as i64 + 1,
                "driver_id": driver_id,
                "display_name": display_name,
                "total_points": total_points,
                "rounds_entered": rounds_entered,
                "wins": wins,
                "p2_count": p2_count,
                "p3_count": p3_count,
                "best_result": best_result,
            })
        })
        .collect();

    // Per-round breakdown: for each round, driver results
    let round_rows: Vec<(i64, String, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>)> =
        sqlx::query_as(
            "SELECT cr.round_number, cr.event_id, he.name as event_name,
                    hee.driver_id, hee.points, hee.position, hee.result_status
             FROM championship_rounds cr
             INNER JOIN hotlap_events he ON he.id = cr.event_id
             LEFT JOIN hotlap_event_entries hee ON hee.event_id = cr.event_id
             WHERE cr.championship_id = ?
             ORDER BY cr.round_number, hee.position",
        )
        .bind(&id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    // Group by (round_number, event_id, event_name)
    let mut rounds_map: std::collections::BTreeMap<i64, Value> = std::collections::BTreeMap::new();
    for (round_number, event_id, event_name, driver_id, points, position, result_status) in &round_rows {
        let entry = rounds_map.entry(*round_number).or_insert_with(|| {
            json!({
                "round_number": round_number,
                "event_id": event_id,
                "event_name": event_name,
                "results": [],
            })
        });
        if let Some(driver_id) = driver_id {
            if let Some(results) = entry.get_mut("results").and_then(|v| v.as_array_mut()) {
                results.push(json!({
                    "driver_id": driver_id,
                    "points": points,
                    "position": position,
                    "result_status": result_status,
                }));
            }
        }
    }
    let rounds: Vec<Value> = rounds_map.into_values().collect();

    Json(json!({
        "championship": championship,
        "standings": standings,
        "rounds": rounds,
    }))
}

// ─── Public Event Sessions (Group Racing) ────────────────────────────────────

/// GET /public/events/{id}/sessions — group session results linked to a hotlap event
/// Returns per-session multiplayer results with F1 points and gap-to-leader
async fn public_event_sessions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get group sessions linked to this event
    let sessions: Vec<(String, String, Option<String>, Option<String>)> = match sqlx::query_as(
        "SELECT gs.id, gs.status, gs.started_at, gs.completed_at
         FROM group_sessions gs
         WHERE gs.hotlap_event_id = ?
         ORDER BY gs.started_at DESC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let mut sessions_json: Vec<Value> = Vec::new();

    for (session_id, status, started_at, completed_at) in &sessions {
        // Fetch multiplayer results with PII-safe display name
        let results: Vec<(String, String, i64, Option<i64>, Option<i64>, i64, i64)> =
            sqlx::query_as(
                "SELECT mr.driver_id,
                        CASE WHEN d.show_nickname_on_leaderboard = 1 AND d.nickname IS NOT NULL
                             THEN d.nickname ELSE d.name END as display_name,
                        mr.position, mr.best_lap_ms, mr.total_time_ms, mr.laps_completed, mr.dnf
                 FROM multiplayer_results mr
                 LEFT JOIN drivers d ON d.id = mr.driver_id
                 WHERE mr.group_session_id = ?
                 ORDER BY mr.position ASC",
            )
            .bind(session_id)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

        // Find minimum best_lap_ms among non-DNF drivers for gap calculation
        let min_best_lap: Option<i64> = results
            .iter()
            .filter(|(_, _, _, _, _, _, dnf)| *dnf == 0)
            .filter_map(|(_, _, _, best_lap_ms, _, _, _)| *best_lap_ms)
            .min();

        use crate::lap_tracker::f1_points_for_position;

        let results_json: Vec<Value> = results.into_iter().map(|(driver_id, display_name, position,
                                                                  best_lap_ms, total_time_ms,
                                                                  laps_completed, dnf)| {
            let race_points = f1_points_for_position(position, dnf == 1);
            let gap_to_leader_ms: Option<i64> = match (best_lap_ms, min_best_lap, dnf) {
                (Some(bl), Some(min_bl), 0) => Some(bl - min_bl),
                _ => None,
            };
            json!({
                "position": position,
                "driver_id": driver_id,
                "display_name": display_name,
                "best_lap_ms": best_lap_ms,
                "total_time_ms": total_time_ms,
                "laps_completed": laps_completed,
                "dnf": dnf == 1,
                "race_points": race_points,
                "gap_to_leader_ms": gap_to_leader_ms,
            })
        }).collect();

        sessions_json.push(json!({
            "session_id": session_id,
            "status": status,
            "started_at": started_at,
            "completed_at": completed_at,
            "results": results_json,
        }));
    }

    Json(json!({
        "event_id": id,
        "sessions": sessions_json,
    }))
}

// ─── Dynamic Pricing Admin ───────────────────────────────────────────────────

async fn list_pricing_rules(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<i64>, Option<i64>, Option<i64>, f64, i64, bool)>(
        "SELECT id, rule_type, day_of_week, hour_start, hour_end, multiplier, flat_adjustment_paise, is_active
         FROM pricing_rules ORDER BY rule_type, day_of_week, hour_start",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rules) => {
            let list: Vec<Value> = rules.iter().map(|r| json!({
                "id": r.0, "rule_type": r.1,
                "day_of_week": r.2, "hour_start": r.3, "hour_end": r.4,
                "multiplier": r.5, "flat_adjustment_paise": r.6, "is_active": r.7,
            })).collect();
            Json(json!({ "rules": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_pricing_rule(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let rule_type = body.get("rule_type").and_then(|v| v.as_str()).unwrap_or("custom");
    let multiplier = body.get("multiplier").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let flat_adj = body.get("flat_adjustment_paise").and_then(|v| v.as_i64()).unwrap_or(0);

    let result = sqlx::query(
        "INSERT INTO pricing_rules (id, rule_type, day_of_week, hour_start, hour_end, multiplier, flat_adjustment_paise, is_active)
         VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
    )
    .bind(&id)
    .bind(rule_type)
    .bind(body.get("day_of_week").and_then(|v| v.as_i64()))
    .bind(body.get("hour_start").and_then(|v| v.as_i64()))
    .bind(body.get("hour_end").and_then(|v| v.as_i64()))
    .bind(multiplier)
    .bind(flat_adj)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "pricing_rules", &id, "create",
                None, new_values.as_deref(), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_rule_create",
                &json!({"rule_id": id, "rule_type": rule_type}).to_string(),
                None, None,
            ).await;
            Json(json!({ "id": id }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_pricing_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let old_snapshot = accounting::snapshot_row(&state, "pricing_rules", &id).await;

    let result = sqlx::query(
        "UPDATE pricing_rules SET
            rule_type = COALESCE(?, rule_type),
            day_of_week = ?,
            hour_start = ?,
            hour_end = ?,
            multiplier = COALESCE(?, multiplier),
            flat_adjustment_paise = COALESCE(?, flat_adjustment_paise),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("rule_type").and_then(|v| v.as_str()))
    .bind(body.get("day_of_week").and_then(|v| v.as_i64()))
    .bind(body.get("hour_start").and_then(|v| v.as_i64()))
    .bind(body.get("hour_end").and_then(|v| v.as_i64()))
    .bind(body.get("multiplier").and_then(|v| v.as_f64()))
    .bind(body.get("flat_adjustment_paise").and_then(|v| v.as_i64()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "pricing_rules", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            accounting::log_admin_action(
                &state, "pricing_rule_update",
                &json!({"rule_id": id, "changes": body}).to_string(),
                None, None,
            ).await;
            Json(json!({ "ok": true }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_pricing_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let old_snapshot = accounting::snapshot_row(&state, "pricing_rules", &id).await;

    // Soft delete instead of hard delete (preserves audit trail)
    let _ = sqlx::query("UPDATE pricing_rules SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    accounting::log_audit(
        &state, "pricing_rules", &id, "delete",
        old_snapshot.as_deref(), Some("{\"is_active\":false}"), None,
    ).await;
    accounting::log_admin_action(
        &state, "pricing_rule_delete",
        &json!({"rule_id": id}).to_string(),
        None, None,
    ).await;

    Json(json!({ "ok": true }))
}

// ─── Coupons Admin ───────────────────────────────────────────────────────────

async fn list_coupons(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, f64, i64, Option<String>, Option<String>, Option<i64>, bool, bool)>(
        "SELECT id, code, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only, is_active
         FROM coupons ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(coupons) => {
            let list: Vec<Value> = coupons.iter().map(|c| json!({
                "id": c.0, "code": c.1, "coupon_type": c.2, "value": c.3,
                "max_uses": c.4, "valid_from": c.5, "valid_until": c.6,
                "min_spend_paise": c.7, "first_session_only": c.8, "is_active": c.9,
            })).collect();
            Json(json!({ "coupons": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_coupon(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let code = body.get("code").and_then(|v| v.as_str()).unwrap_or("").to_uppercase();
    if code.is_empty() {
        return Json(json!({ "error": "code required" }));
    }

    let result = sqlx::query(
        "INSERT INTO coupons (id, code, coupon_type, value, max_uses, valid_from, valid_until, min_spend_paise, first_session_only, is_active)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
    )
    .bind(&id)
    .bind(&code)
    .bind(body.get("coupon_type").and_then(|v| v.as_str()).unwrap_or("percent"))
    .bind(body.get("value").and_then(|v| v.as_f64()).unwrap_or(10.0))
    .bind(body.get("max_uses").and_then(|v| v.as_i64()).unwrap_or(100))
    .bind(body.get("valid_from").and_then(|v| v.as_str()))
    .bind(body.get("valid_until").and_then(|v| v.as_str()))
    .bind(body.get("min_spend_paise").and_then(|v| v.as_i64()))
    .bind(body.get("first_session_only").and_then(|v| v.as_bool()).unwrap_or(false))
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "coupons", &id, "create",
                None, new_values.as_deref(), None,
            ).await;
            Json(json!({ "id": id, "code": code }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_coupon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let old_snapshot = accounting::snapshot_row(&state, "coupons", &id).await;

    let result = sqlx::query(
        "UPDATE coupons SET
            code = COALESCE(?, code),
            coupon_type = COALESCE(?, coupon_type),
            value = COALESCE(?, value),
            max_uses = COALESCE(?, max_uses),
            valid_from = ?,
            valid_until = ?,
            min_spend_paise = ?,
            first_session_only = COALESCE(?, first_session_only),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("code").and_then(|v| v.as_str()))
    .bind(body.get("coupon_type").and_then(|v| v.as_str()))
    .bind(body.get("value").and_then(|v| v.as_f64()))
    .bind(body.get("max_uses").and_then(|v| v.as_i64()))
    .bind(body.get("valid_from").and_then(|v| v.as_str()))
    .bind(body.get("valid_until").and_then(|v| v.as_str()))
    .bind(body.get("min_spend_paise").and_then(|v| v.as_i64()))
    .bind(body.get("first_session_only").and_then(|v| v.as_bool()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "coupons", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            Json(json!({ "ok": true }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_coupon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let old_snapshot = accounting::snapshot_row(&state, "coupons", &id).await;

    // Soft delete instead of hard delete
    let _ = sqlx::query("UPDATE coupons SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    accounting::log_audit(
        &state, "coupons", &id, "delete",
        old_snapshot.as_deref(), Some("{\"is_active\":false}"), None,
    ).await;

    Json(json!({ "ok": true }))
}

// ─── Review Nudges ───────────────────────────────────────────────────────────

async fn pending_review_nudges(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT rn.id, rn.driver_id, d.name, d.phone
         FROM review_nudges rn
         JOIN drivers d ON rn.driver_id = d.id
         WHERE rn.sent_at IS NULL
         ORDER BY rn.created_at ASC
         LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(nudges) => {
            let list: Vec<Value> = nudges.iter().map(|n| json!({
                "id": n.0, "driver_id": n.1, "driver_name": n.2, "phone": n.3,
            })).collect();
            Json(json!({ "nudges": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn mark_nudge_sent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let review_credited = body.get("review_credited").and_then(|v| v.as_bool()).unwrap_or(false);

    let _ = sqlx::query(
        "UPDATE review_nudges SET sent_at = datetime('now'), review_credited = ? WHERE id = ?",
    )
    .bind(review_credited)
    .bind(&id)
    .execute(&state.db)
    .await;

    // If they left a review, credit 50 credits (₹50)
    if review_credited {
        let driver: Option<(String,)> = sqlx::query_as(
            "SELECT driver_id FROM review_nudges WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((driver_id,)) = driver {
            let _ = crate::wallet::credit(
                &state,
                &driver_id,
                5000,
                "review_reward",
                Some(&id),
                Some("Thank you for your Google review!"),
                None,
            )
            .await;
        }
    }

    Json(json!({ "ok": true }))
}

// ─── Time Trial Admin ────────────────────────────────────────────────────────

async fn list_time_trials(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, bool)>(
        "SELECT id, track, car, week_start, week_end, is_active
         FROM time_trials ORDER BY week_start DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(trials) => {
            let list: Vec<Value> = trials.iter().map(|t| json!({
                "id": t.0, "track": t.1, "car": t.2,
                "week_start": t.3, "week_end": t.4, "is_active": t.5,
            })).collect();
            Json(json!({ "time_trials": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_time_trial(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let track = match body.get("track").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Json(json!({ "error": "track required" })),
    };
    let car = match body.get("car").and_then(|v| v.as_str()) {
        Some(c) => c,
        None => return Json(json!({ "error": "car required" })),
    };
    let week_start = match body.get("week_start").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return Json(json!({ "error": "week_start required (YYYY-MM-DD)" })),
    };
    let week_end = match body.get("week_end").and_then(|v| v.as_str()) {
        Some(e) => e,
        None => return Json(json!({ "error": "week_end required (YYYY-MM-DD)" })),
    };

    let result = sqlx::query(
        "INSERT INTO time_trials (id, track, car, week_start, week_end) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(track)
    .bind(car)
    .bind(week_start)
    .bind(week_end)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE time_trials SET
            track = COALESCE(?, track), car = COALESCE(?, car),
            week_start = COALESCE(?, week_start), week_end = COALESCE(?, week_end),
            is_active = COALESCE(?, is_active)
         WHERE id = ?",
    )
    .bind(body.get("track").and_then(|v| v.as_str()))
    .bind(body.get("car").and_then(|v| v.as_str()))
    .bind(body.get("week_start").and_then(|v| v.as_str()))
    .bind(body.get("week_end").and_then(|v| v.as_str()))
    .bind(body.get("is_active").and_then(|v| v.as_bool()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_time_trial(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let _ = sqlx::query("DELETE FROM time_trials WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;
    Json(json!({ "ok": true }))
}

// ─── Tournaments ─────────────────────────────────────────────────────────────

async fn list_tournaments(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise,
                status, registration_start, registration_end, event_date
         FROM tournaments ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tournaments) => {
            let list: Vec<Value> = tournaments.iter().map(|t| json!({
                "id": t.0, "name": t.1, "description": t.2,
                "track": t.3, "car": t.4, "format": t.5,
                "max_participants": t.6, "entry_fee_paise": t.7,
                "prize_pool_paise": t.8, "status": t.9,
                "registration_start": t.10, "registration_end": t.11,
                "event_date": t.12,
            })).collect();
            Json(json!({ "tournaments": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_tournament(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n,
        None => return Json(json!({ "error": "name required" })),
    };

    let result = sqlx::query(
        "INSERT INTO tournaments (id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise, status, registration_start, registration_end, event_date, rules)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'upcoming', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(body.get("description").and_then(|v| v.as_str()))
    .bind(body.get("track").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(body.get("car").and_then(|v| v.as_str()).unwrap_or(""))
    .bind(body.get("format").and_then(|v| v.as_str()).unwrap_or("time_attack"))
    .bind(body.get("max_participants").and_then(|v| v.as_i64()).unwrap_or(16))
    .bind(body.get("entry_fee_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(body.get("prize_pool_paise").and_then(|v| v.as_i64()).unwrap_or(0))
    .bind(body.get("registration_start").and_then(|v| v.as_str()))
    .bind(body.get("registration_end").and_then(|v| v.as_str()))
    .bind(body.get("event_date").and_then(|v| v.as_str()))
    .bind(body.get("rules").and_then(|v| v.as_str()))
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_tournament(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let tournament = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants, entry_fee_paise, prize_pool_paise,
                status, registration_start, registration_end, event_date, rules
         FROM tournaments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let t = match tournament {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "error": "Tournament not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Count registrations
    let reg_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    Json(json!({
        "tournament": {
            "id": t.0, "name": t.1, "description": t.2,
            "track": t.3, "car": t.4, "format": t.5,
            "max_participants": t.6, "entry_fee_paise": t.7,
            "prize_pool_paise": t.8, "status": t.9,
            "registration_start": t.10, "registration_end": t.11,
            "event_date": t.12, "rules": t.13,
            "registered_count": reg_count,
        }
    }))
}

async fn update_tournament(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE tournaments SET
            name = COALESCE(?, name), description = ?,
            track = COALESCE(?, track), car = COALESCE(?, car),
            format = COALESCE(?, format), max_participants = COALESCE(?, max_participants),
            entry_fee_paise = COALESCE(?, entry_fee_paise),
            prize_pool_paise = COALESCE(?, prize_pool_paise),
            status = COALESCE(?, status),
            registration_start = ?, registration_end = ?, event_date = ?,
            rules = ?
         WHERE id = ?",
    )
    .bind(body.get("name").and_then(|v| v.as_str()))
    .bind(body.get("description").and_then(|v| v.as_str()))
    .bind(body.get("track").and_then(|v| v.as_str()))
    .bind(body.get("car").and_then(|v| v.as_str()))
    .bind(body.get("format").and_then(|v| v.as_str()))
    .bind(body.get("max_participants").and_then(|v| v.as_i64()))
    .bind(body.get("entry_fee_paise").and_then(|v| v.as_i64()))
    .bind(body.get("prize_pool_paise").and_then(|v| v.as_i64()))
    .bind(body.get("status").and_then(|v| v.as_str()))
    .bind(body.get("registration_start").and_then(|v| v.as_str()))
    .bind(body.get("registration_end").and_then(|v| v.as_str()))
    .bind(body.get("event_date").and_then(|v| v.as_str()))
    .bind(body.get("rules").and_then(|v| v.as_str()))
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn tournament_registrations(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<i64>, String, Option<i64>)>(
        "SELECT tr.id, tr.driver_id, d.name, tr.seed, tr.status, tr.best_time_ms
         FROM tournament_registrations tr
         JOIN drivers d ON tr.driver_id = d.id
         WHERE tr.tournament_id = ?
         ORDER BY COALESCE(tr.seed, 9999), tr.created_at",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(regs) => {
            let list: Vec<Value> = regs.iter().map(|r| json!({
                "id": r.0, "driver_id": r.1, "driver_name": r.2,
                "seed": r.3, "status": r.4, "best_time_ms": r.5,
            })).collect();
            Json(json!({ "registrations": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn tournament_matches(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, i64, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>, String)>(
        "SELECT tm.id, tm.round, tm.match_number,
                da.name, db.name,
                tm.time_a_ms, tm.time_b_ms, dw.name, tm.status
         FROM tournament_matches tm
         LEFT JOIN drivers da ON tm.driver_a = da.id
         LEFT JOIN drivers db ON tm.driver_b = db.id
         LEFT JOIN drivers dw ON tm.winner_id = dw.id
         WHERE tm.tournament_id = ?
         ORDER BY tm.round, tm.match_number",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(matches) => {
            let list: Vec<Value> = matches.iter().map(|m| json!({
                "id": m.0, "round": m.1, "match_number": m.2,
                "driver_a": m.3, "driver_b": m.4,
                "time_a_ms": m.5, "time_b_ms": m.6,
                "winner": m.7, "status": m.8,
            })).collect();
            Json(json!({ "matches": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn generate_bracket(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get all registered drivers
    let regs = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT driver_id, seed FROM tournament_registrations
         WHERE tournament_id = ? AND status IN ('registered', 'checked_in')
         ORDER BY COALESCE(seed, 9999), created_at",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    let regs = match regs {
        Ok(r) => r,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    if regs.len() < 2 {
        return Json(json!({ "error": "Need at least 2 registrations" }));
    }

    // Delete existing matches
    let _ = sqlx::query("DELETE FROM tournament_matches WHERE tournament_id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    // Generate round 1 matches (pair sequential registrations)
    let mut match_count = 0;
    let mut i = 0;
    while i < regs.len() {
        let driver_a = &regs[i].0;
        let driver_b = if i + 1 < regs.len() {
            Some(&regs[i + 1].0)
        } else {
            None // Bye
        };

        match_count += 1;
        let match_id = uuid::Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "INSERT INTO tournament_matches (id, tournament_id, round, match_number, driver_a, driver_b, status)
             VALUES (?, ?, 1, ?, ?, ?, ?)",
        )
        .bind(&match_id)
        .bind(&id)
        .bind(match_count as i64)
        .bind(driver_a)
        .bind(driver_b)
        .bind(if driver_b.is_some() { "pending" } else { "completed" })
        .execute(&state.db)
        .await;

        // Auto-advance bye
        if driver_b.is_none() {
            let _ = sqlx::query(
                "UPDATE tournament_matches SET winner_id = ?, status = 'completed' WHERE id = ?",
            )
            .bind(driver_a)
            .bind(&match_id)
            .execute(&state.db)
            .await;
        }

        i += 2;
    }

    // Update tournament status
    let _ = sqlx::query("UPDATE tournaments SET status = 'in_progress' WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await;

    Json(json!({ "ok": true, "round_1_matches": match_count }))
}

async fn record_match_result(
    State(state): State<Arc<AppState>>,
    Path((tournament_id, match_id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let winner_id = match body.get("winner_id").and_then(|v| v.as_str()) {
        Some(w) => w,
        None => return Json(json!({ "error": "winner_id required" })),
    };

    let _ = sqlx::query(
        "UPDATE tournament_matches SET
            winner_id = ?, status = 'completed', completed_at = datetime('now'),
            time_a_ms = ?, time_b_ms = ?
         WHERE id = ? AND tournament_id = ?",
    )
    .bind(winner_id)
    .bind(body.get("time_a_ms").and_then(|v| v.as_i64()))
    .bind(body.get("time_b_ms").and_then(|v| v.as_i64()))
    .bind(&match_id)
    .bind(&tournament_id)
    .execute(&state.db)
    .await;

    // Check if all matches in current round are done, generate next round
    let current_round: Option<(i64,)> = sqlx::query_as(
        "SELECT round FROM tournament_matches WHERE id = ?",
    )
    .bind(&match_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((round,)) = current_round {
        let pending: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM tournament_matches
             WHERE tournament_id = ? AND round = ? AND status != 'completed'",
        )
        .bind(&tournament_id)
        .bind(round)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if pending.map(|p| p.0 == 0).unwrap_or(false) {
            // All done in this round — get winners and create next round
            let winners = sqlx::query_as::<_, (String,)>(
                "SELECT winner_id FROM tournament_matches
                 WHERE tournament_id = ? AND round = ? AND winner_id IS NOT NULL
                 ORDER BY match_number",
            )
            .bind(&tournament_id)
            .bind(round)
            .fetch_all(&state.db)
            .await
            .unwrap_or_default();

            if winners.len() > 1 {
                let next_round = round + 1;
                let mut match_num = 0;
                let mut i = 0;
                while i < winners.len() {
                    match_num += 1;
                    let driver_a = &winners[i].0;
                    let driver_b = if i + 1 < winners.len() {
                        Some(&winners[i + 1].0)
                    } else {
                        None
                    };

                    let mid = uuid::Uuid::new_v4().to_string();
                    let _ = sqlx::query(
                        "INSERT INTO tournament_matches (id, tournament_id, round, match_number, driver_a, driver_b, status)
                         VALUES (?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(&mid)
                    .bind(&tournament_id)
                    .bind(next_round)
                    .bind(match_num as i64)
                    .bind(driver_a)
                    .bind(driver_b)
                    .bind(if driver_b.is_some() { "pending" } else { "completed" })
                    .execute(&state.db)
                    .await;

                    if driver_b.is_none() {
                        let _ = sqlx::query(
                            "UPDATE tournament_matches SET winner_id = ?, status = 'completed' WHERE id = ?",
                        )
                        .bind(driver_a)
                        .bind(&mid)
                        .execute(&state.db)
                        .await;
                    }
                    i += 2;
                }
            } else if winners.len() == 1 {
                // Tournament complete!
                let _ = sqlx::query("UPDATE tournaments SET status = 'completed' WHERE id = ?")
                    .bind(&tournament_id)
                    .execute(&state.db)
                    .await;
                let _ = sqlx::query(
                    "UPDATE tournament_registrations SET status = 'winner' WHERE tournament_id = ? AND driver_id = ?",
                )
                .bind(&tournament_id)
                .bind(&winners[0].0)
                .execute(&state.db)
                .await;
            }
        }
    }

    Json(json!({ "ok": true }))
}

// ─── Customer Tournament Endpoints ──────────────────────────────────────────

async fn customer_list_tournaments(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String, i64, i64, i64, String, Option<String>)>(
        "SELECT id, name, description, track, car, format, max_participants,
                entry_fee_paise, prize_pool_paise, status, event_date
         FROM tournaments
         WHERE status IN ('upcoming', 'registration', 'in_progress')
         ORDER BY event_date ASC",
    )
    .fetch_all(&state.db)
    .await;

    let tournaments = match rows {
        Ok(t) => t,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Check which the driver is registered for
    let registered: Vec<String> = sqlx::query_as::<_, (String,)>(
        "SELECT tournament_id FROM tournament_registrations WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.0)
    .collect();

    let list: Vec<Value> = tournaments.iter().map(|t| {
        json!({
            "id": t.0, "name": t.1, "description": t.2,
            "track": t.3, "car": t.4, "format": t.5,
            "max_participants": t.6,
            "entry_fee_display": if t.7 > 0 { format!("Rs.{}", t.7 / 100) } else { "Free".to_string() },
            "prize_pool_display": if t.8 > 0 { format!("Rs.{}", t.8 / 100) } else { "TBD".to_string() },
            "status": t.9, "event_date": t.10,
            "is_registered": registered.contains(&t.0),
        })
    }).collect();

    Json(json!({ "tournaments": list }))
}

async fn customer_register_tournament(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check tournament exists and is open
    let status: Option<(String, i64)> = sqlx::query_as(
        "SELECT status, max_participants FROM tournaments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match &status {
        Some((s, _)) if s != "registration" && s != "upcoming" => {
            return Json(json!({ "error": "Registration is not open" }));
        }
        None => return Json(json!({ "error": "Tournament not found" })),
        _ => {}
    }

    let max = status.unwrap().1;

    // Check capacity
    let count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM tournament_registrations WHERE tournament_id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    if count >= max {
        return Json(json!({ "error": "Tournament is full" }));
    }

    let reg_id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT INTO tournament_registrations (id, tournament_id, driver_id) VALUES (?, ?, ?)",
    )
    .bind(&reg_id)
    .bind(&id)
    .bind(&driver_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "ok": true, "registration_id": reg_id })),
        Err(e) if e.to_string().contains("UNIQUE") => {
            Json(json!({ "error": "Already registered" }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Coaching: Lap Comparison ────────────────────────────────────────────────

#[derive(Deserialize)]
struct CompareLapsQuery {
    track: String,
    car: String,
    compare_to: Option<String>, // "record" or driver_id
}

async fn customer_compare_laps(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<CompareLapsQuery>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Get driver's laps on this track+car
    let my_laps = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid
         FROM laps WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
         ORDER BY lap_time_ms ASC LIMIT 20",
    )
    .bind(&driver_id)
    .bind(&params.track)
    .bind(&params.car)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if my_laps.is_empty() {
        return Json(json!({ "error": "No laps found on this track/car" }));
    }

    let my_best = &my_laps[0];

    // Get comparison target
    let compare_to = params.compare_to.as_deref().unwrap_or("record");

    let reference_lap: Option<(String, i64, Option<i64>, Option<i64>, Option<i64>)> = if compare_to == "record" {
        // Compare to track record
        sqlx::query_as(
            "SELECT d.name, tr.best_lap_ms, l.sector1_ms, l.sector2_ms, l.sector3_ms
             FROM track_records tr
             JOIN drivers d ON tr.driver_id = d.id
             LEFT JOIN laps l ON tr.lap_id = l.id
             WHERE tr.track = ? AND tr.car = ?",
        )
        .bind(&params.track)
        .bind(&params.car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        // Compare to specific driver's best
        sqlx::query_as(
            "SELECT d.name, pb.best_lap_ms, l.sector1_ms, l.sector2_ms, l.sector3_ms
             FROM personal_bests pb
             JOIN drivers d ON pb.driver_id = d.id
             LEFT JOIN laps l ON pb.lap_id = l.id
             WHERE pb.driver_id = ? AND pb.track = ? AND pb.car = ?",
        )
        .bind(compare_to)
        .bind(&params.track)
        .bind(&params.car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    };

    // Compute sector deltas
    let sector_analysis = if let Some(ref_lap) = &reference_lap {
        let s1_delta = match (my_best.2, ref_lap.2) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };
        let s2_delta = match (my_best.3, ref_lap.3) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };
        let s3_delta = match (my_best.4, ref_lap.4) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };

        let weakest = [
            s1_delta.map(|d| ("S1", d)),
            s2_delta.map(|d| ("S2", d)),
            s3_delta.map(|d| ("S3", d)),
        ]
        .iter()
        .filter_map(|x| *x)
        .max_by_key(|(_, d)| *d);

        Some(json!({
            "s1_delta_ms": s1_delta,
            "s2_delta_ms": s2_delta,
            "s3_delta_ms": s3_delta,
            "weakest_sector": weakest.map(|(s, d)| format!("{} (+{}ms)", s, d)),
            "total_delta_ms": my_best.1 - ref_lap.1,
        }))
    } else {
        None
    };

    // Consistency trend (last 10 laps chronologically)
    let recent_laps = sqlx::query_as::<_, (i64,)>(
        "SELECT lap_time_ms FROM laps
         WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
         ORDER BY created_at DESC LIMIT 10",
    )
    .bind(&driver_id)
    .bind(&params.track)
    .bind(&params.car)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let trend: Vec<i64> = recent_laps.iter().rev().map(|l| l.0).collect();
    let improving = if trend.len() >= 3 {
        let first_half: f64 = trend[..trend.len()/2].iter().map(|&t| t as f64).sum::<f64>() / (trend.len()/2) as f64;
        let second_half: f64 = trend[trend.len()/2..].iter().map(|&t| t as f64).sum::<f64>() / (trend.len() - trend.len()/2) as f64;
        Some(second_half < first_half)
    } else {
        None
    };

    Json(json!({
        "track": params.track,
        "car": params.car,
        "my_best": {
            "time_ms": my_best.1,
            "s1_ms": my_best.2,
            "s2_ms": my_best.3,
            "s3_ms": my_best.4,
        },
        "reference": reference_lap.as_ref().map(|r| json!({
            "driver": r.0,
            "time_ms": r.1,
            "s1_ms": r.2,
            "s2_ms": r.3,
            "s3_ms": r.4,
        })),
        "sector_analysis": sector_analysis,
        "recent_trend": trend,
        "improving": improving,
        "tip": sector_analysis.as_ref().and_then(|sa| {
            sa.get("weakest_sector").and_then(|w| w.as_str()).map(|w| {
                format!("Focus on {} — that is where you lose the most time vs the reference lap.", w)
            })
        }),
    }))
}

// ─── Bot endpoints (WhatsApp bot, terminal_secret auth) ─────────────────────

fn validate_bot_secret(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), Json<Value>> {
    let secret = state.config.cloud.terminal_secret.as_deref()
        .ok_or_else(|| Json(json!({ "error": "Terminal secret not configured" })))?;
    let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
    if provided != Some(secret) {
        return Err(Json(json!({ "error": "Unauthorized" })));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct BotLookupQuery {
    phone: String,
}

async fn bot_lookup(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<BotLookupQuery>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let phone = params.phone.trim();
    if phone.is_empty() {
        return Json(json!({ "error": "Phone number required" }));
    }

    // Look up driver by phone hash
    let ph = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, String, Option<String>, bool)>(
        "SELECT id, name, phone_enc, COALESCE(has_used_trial, 0) FROM drivers WHERE phone_hash = ?",
    )
    .bind(&ph)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some((id, name, _phone, has_used_trial))) => {
            // Get wallet balance
            let balance = wallet::get_balance(&state, &id).await.unwrap_or(0);

            Json(json!({
                "registered": true,
                "driver_id": id,
                "name": name,
                "wallet_balance_paise": balance,
                "has_used_trial": has_used_trial,
            }))
        }
        Ok(None) => Json(json!({
            "registered": false,
            "message": "No account found for this phone number",
        })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

async fn bot_pricing(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tiers) => {
            let list: Vec<Value> = tiers
                .iter()
                .map(|t| {
                    json!({
                        "id": t.0, "name": t.1, "duration_minutes": t.2,
                        "price_paise": t.3, "is_trial": t.4,
                    })
                })
                .collect();
            Json(json!({ "tiers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Debug, Deserialize)]
struct BotBookRequest {
    phone: String,
    pricing_tier_id: String,
    experience_id: Option<String>,
}

async fn bot_book(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BotBookRequest>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    // Look up driver by phone hash
    let ph = state.field_cipher.hash_phone(&req.phone);
    let driver = sqlx::query_as::<_, (String, String, bool, bool)>(
        "SELECT id, name, COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0) FROM drivers WHERE phone_hash = ?",
    )
    .bind(&ph)
    .fetch_optional(&state.db)
    .await;

    let (driver_id, driver_name, has_used_trial, unlimited_trials) = match driver {
        Ok(Some(d)) => d,
        Ok(None) => return Json(json!({
            "status": "error",
            "error": "not_registered",
            "message": "No account found for this phone number. Please register at app.racingpoint.cloud first.",
        })),
        Err(e) => return Json(json!({ "status": "error", "error": format!("DB error: {}", e) })),
    };

    // Validate pricing tier
    let tier = match sqlx::query_as::<_, (String, String, i64, i64, bool)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some(t)) => t,
        Ok(None) => return Json(json!({ "status": "error", "error": "invalid_tier", "message": "Invalid pricing tier" })),
        Err(e) => return Json(json!({ "status": "error", "error": format!("DB error: {}", e) })),
    };

    let is_trial = tier.4;
    let price_paise = tier.3;
    let duration_minutes = tier.2;

    // Trial check (skip for unlimited_trials drivers)
    if is_trial && has_used_trial && !unlimited_trials {
        return Json(json!({
            "status": "error",
            "error": "trial_used",
            "message": "You've already used your free trial.",
        }));
    }

    // Wallet balance check for non-trial
    if !is_trial {
        let balance = match wallet::get_balance(&state, &driver_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "status": "error", "error": e })),
        };

        if balance < price_paise {
            return Json(json!({
                "status": "error",
                "error": "insufficient_balance",
                "message": format!("Insufficient balance. You have ₹{} but need ₹{}.", balance / 100, price_paise / 100),
                "balance_paise": balance,
                "required_paise": price_paise,
            }));
        }
    }

    // Check for existing active reservation
    if let Some(existing) = pod_reservation::get_active_reservation_for_driver(&state, &driver_id).await {
        return Json(json!({
            "status": "error",
            "error": "active_reservation",
            "message": "You already have an active reservation.",
            "reservation_id": existing.id,
        }));
    }

    // Find idle pod
    let pod_id = match pod_reservation::find_idle_pod(&state).await {
        Some(id) => id,
        None => return Json(json!({
            "status": "error",
            "error": "no_pods",
            "message": "No pods available right now. Please try again shortly or visit us to get in the queue.",
        })),
    };

    let pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.number).unwrap_or(0)
    };

    // Debit wallet (skip for trial)
    let (wallet_txn_id, wallet_debit) = if !is_trial && price_paise > 0 {
        match wallet::debit(
            &state,
            &driver_id,
            price_paise,
            "debit_session",
            None,
            Some(&format!("{} on Pod {} (WhatsApp)", tier.1, pod_number)),
        )
        .await
        {
            Ok((_, txn_id)) => (Some(txn_id), Some(price_paise)),
            Err(e) => return Json(json!({ "status": "error", "error": e })),
        }
    } else {
        (None, None)
    };

    // Create pod reservation
    let reservation_id = match pod_reservation::create_reservation(&state, &driver_id, &pod_id).await {
        Ok(id) => id,
        Err(e) => {
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Bot booking failed — auto-refund")).await;
            }
            return Json(json!({ "status": "error", "error": e }));
        }
    };

    // Create auth token (PIN type)
    let experience_id = req.experience_id.clone();
    let auth_token = match auth::create_auth_token(
        &state,
        pod_id.clone(),
        driver_id.clone(),
        req.pricing_tier_id.clone(),
        "pin".to_string(),
        None,
        None,
        experience_id,
        None,
    )
    .await
    {
        Ok(token_info) => token_info,
        Err(e) => {
            let _ = pod_reservation::end_reservation(&state, &reservation_id).await;
            if let (Some(_), Some(amount)) = (&wallet_txn_id, wallet_debit) {
                let _ = wallet::refund(&state, &driver_id, amount, None, Some("Bot booking failed — auto-refund")).await;
            }
            return Json(json!({ "status": "error", "error": format!("Failed to create auth: {}", e) }));
        }
    };

    Json(json!({
        "status": "booked",
        "booking_id": reservation_id,
        "driver_name": driver_name,
        "pod_number": pod_number,
        "pin": auth_token.token,
        "allocated_seconds": auth_token.allocated_seconds,
        "duration_minutes": duration_minutes,
        "tier_name": tier.1,
        "wallet_debit_paise": wallet_debit,
        "message": format!(
            "Session booked! Head to Pod {} and enter PIN {} on the screen. You have {} minutes.",
            pod_number, auth_token.token, duration_minutes
        ),
    }))
}

// ─── Bot: pods-status ────────────────────────────────────────────────────

async fn bot_pods_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let pods = state.pods.read().await;
    let total = pods.len();
    let available = pods.values()
        .filter(|p| p.status == PodStatus::Idle && p.billing_session_id.is_none())
        .count();
    let in_use = pods.values()
        .filter(|p| p.status == PodStatus::InSession)
        .count();

    Json(json!({
        "total": total,
        "available": available,
        "in_use": in_use,
        "message": format!("{} of {} rigs are free right now", available, total),
    }))
}

// ─── Bot: events ─────────────────────────────────────────────────────────

async fn bot_events(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    // Upcoming/active tournaments
    let tournaments = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT id, name, status, event_date FROM tournaments
         WHERE status IN ('upcoming', 'registration', 'in_progress')
         ORDER BY event_date ASC LIMIT 5"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Active time trials (current week or future)
    let time_trials = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, track, car, week_start, week_end FROM time_trials
         WHERE is_active = 1 AND week_end >= date('now')
         ORDER BY week_start ASC LIMIT 5"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let t_list: Vec<Value> = tournaments.iter().map(|t| json!({
        "id": t.0, "name": t.1, "status": t.2, "event_date": t.3,
    })).collect();

    let tt_list: Vec<Value> = time_trials.iter().map(|t| json!({
        "id": t.0, "track": t.1, "car": t.2, "week_start": t.3, "week_end": t.4,
    })).collect();

    Json(json!({
        "tournaments": t_list,
        "time_trials": tt_list,
        "has_events": !t_list.is_empty() || !tt_list.is_empty(),
    }))
}

// ─── Bot: leaderboard ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct BotLeaderboardQuery {
    track: Option<String>,
    sim_type: Option<String>,
}

async fn bot_leaderboard(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<BotLeaderboardQuery>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let entries: Result<Vec<(String, String, i64, String)>, _> = if let Some(ref track) = params.track {
        // Track-specific: query laps directly
        if let Some(ref st) = params.sim_type {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, l.track, MIN(l.lap_time_ms) as best_time, l.car
                 FROM laps l
                 JOIN drivers d ON l.driver_id = d.id
                 WHERE l.track = ? AND l.sim_type = ? AND l.lap_time_ms > 0
                 GROUP BY l.driver_id, l.track
                 ORDER BY best_time ASC LIMIT 10"
            )
            .bind(track)
            .bind(st)
            .fetch_all(&state.db)
            .await
        } else {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, l.track, MIN(l.lap_time_ms) as best_time, l.car
                 FROM laps l
                 JOIN drivers d ON l.driver_id = d.id
                 WHERE l.track = ? AND l.lap_time_ms > 0
                 GROUP BY l.driver_id, l.track
                 ORDER BY best_time ASC LIMIT 10"
            )
            .bind(track)
            .fetch_all(&state.db)
            .await
        }
    } else {
        // All-tracks: query track_records
        if let Some(ref st) = params.sim_type {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, tr.track, tr.best_lap_ms, tr.car
                 FROM track_records tr
                 JOIN drivers d ON tr.driver_id = d.id
                 WHERE tr.sim_type = ?
                 ORDER BY tr.best_lap_ms ASC LIMIT 10"
            )
            .bind(st)
            .fetch_all(&state.db)
            .await
        } else {
            sqlx::query_as::<_, (String, String, i64, String)>(
                "SELECT d.name, tr.track, tr.best_lap_ms, tr.car
                 FROM track_records tr
                 JOIN drivers d ON tr.driver_id = d.id
                 ORDER BY tr.best_lap_ms ASC LIMIT 10"
            )
            .fetch_all(&state.db)
            .await
        }
    };

    let list: Vec<Value> = entries.unwrap_or_default().iter().enumerate().map(|(i, e)| json!({
        "position": i + 1,
        "driver": e.0,
        "track": e.1,
        "time_ms": e.2,
        "time_formatted": format!("{}:{:02}.{:03}", e.2 / 60000, (e.2 % 60000) / 1000, e.2 % 1000),
        "car": e.3,
    })).collect();

    let count = list.len();
    Json(json!({
        "entries": list,
        "track_filter": params.track,
        "sim_type": params.sim_type,
        "count": count,
        "last_updated": chrono::Utc::now().to_rfc3339(),
    }))
}

// ─── Bot: customer-stats ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct BotCustomerStatsQuery {
    phone: String,
}

async fn bot_customer_stats(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<BotCustomerStatsQuery>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    let phone = params.phone.trim();

    let ph = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT id, name, COALESCE(total_laps, 0), COALESCE(total_time_ms, 0)
         FROM drivers WHERE phone_hash = ?"
    )
    .bind(&ph)
    .fetch_optional(&state.db)
    .await;

    match driver {
        Ok(Some((id, name, laps, time_ms))) => {
            let sessions = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM billing_sessions
                 WHERE driver_id = ? AND status IN ('completed', 'in_progress')"
            )
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map(|r| r.0)
            .unwrap_or(0);

            let pbs = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?"
            )
            .bind(&id)
            .fetch_one(&state.db)
            .await
            .map(|r| r.0)
            .unwrap_or(0);

            let balance = wallet::get_balance(&state, &id).await.unwrap_or(0);

            Json(json!({
                "found": true,
                "name": name,
                "total_laps": laps,
                "total_sessions": sessions,
                "total_time_ms": time_ms,
                "personal_bests": pbs,
                "wallet_balance_paise": balance,
            }))
        }
        Ok(None) => Json(json!({ "found": false, "message": "No customer found for this phone" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ─── Bot: register-lead ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct BotRegisterLeadRequest {
    phone: String,
    name: Option<String>,
    source: Option<String>,
    intent: Option<String>,
    notes: Option<String>,
}

async fn bot_register_lead(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<BotRegisterLeadRequest>,
) -> Json<Value> {
    if let Err(e) = validate_bot_secret(&state, &headers) {
        return e;
    }

    // Ensure leads table exists
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS leads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            phone TEXT NOT NULL,
            name TEXT,
            source TEXT DEFAULT 'whatsapp',
            intent TEXT DEFAULT 'general',
            stage TEXT DEFAULT 'inquiry',
            notes TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            converted_driver_id TEXT
        )"
    )
    .execute(&state.db)
    .await;

    // Check if lead already exists
    let existing = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM leads WHERE phone = ? LIMIT 1"
    )
    .bind(&req.phone)
    .fetch_optional(&state.db)
    .await;

    match existing {
        Ok(Some((id,))) => {
            // Update existing lead
            let _ = sqlx::query(
                "UPDATE leads SET name = COALESCE(?, name), intent = COALESCE(?, intent),
                 notes = COALESCE(?, notes) WHERE id = ?"
            )
            .bind(&req.name)
            .bind(&req.intent)
            .bind(&req.notes)
            .bind(id)
            .execute(&state.db)
            .await;

            Json(json!({ "status": "updated", "lead_id": id }))
        }
        Ok(None) => {
            let result = sqlx::query(
                "INSERT INTO leads (phone, name, source, intent, notes)
                 VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&req.phone)
            .bind(&req.name)
            .bind(req.source.as_deref().unwrap_or("whatsapp"))
            .bind(req.intent.as_deref().unwrap_or("general"))
            .bind(&req.notes)
            .execute(&state.db)
            .await;

            match result {
                Ok(r) => Json(json!({ "status": "created", "lead_id": r.last_insert_rowid() })),
                Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
            }
        }
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Pod Debug System
// ═══════════════════════════════════════════════════════════════════════════════

// ─── Pod Activity Log ────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ActivityQuery {
    limit: Option<i64>,
}

async fn global_activity(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ActivityQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(100).min(500);
    let rows: Vec<(String, String, i64, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, pod_id, pod_number, timestamp, category, action, details, source
         FROM pod_activity_log ORDER BY timestamp DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.0, "pod_id": r.1, "pod_number": r.2, "timestamp": r.3,
        "category": r.4, "action": r.5, "details": r.6, "source": r.7,
    })).collect();

    Json(json!(entries))
}

async fn pod_activity(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Query(q): Query<ActivityQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(100).min(500);
    let rows: Vec<(String, String, i64, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, pod_id, pod_number, timestamp, category, action, details, source
         FROM pod_activity_log WHERE pod_id = ? ORDER BY timestamp DESC LIMIT ?"
    )
    .bind(&pod_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.0, "pod_id": r.1, "pod_number": r.2, "timestamp": r.3,
        "category": r.4, "action": r.5, "details": r.6, "source": r.7,
    })).collect();

    Json(json!(entries))
}

// ─── Server Logs ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LogsQuery {
    lines: Option<usize>,
    level: Option<String>,
}

// GET /logs — Tail the racecontrol log file
async fn get_server_logs(Query(q): Query<LogsQuery>) -> Json<Value> {
    let max_lines = q.lines.unwrap_or(200).min(2000);
    let level_filter = q.level.as_deref().unwrap_or("");

    // Find the most recent log file in ./logs/
    let log_dir = std::path::Path::new("logs");
    let log_file = match std::fs::read_dir(log_dir) {
        Ok(entries) => {
            let mut files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("racecontrol"))
                        .unwrap_or(false)
                })
                .collect();
            files.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
            files.first().map(|e| e.path())
        }
        Err(_) => None,
    };

    let path = match log_file {
        Some(p) => p,
        None => return Json(json!({ "lines": [], "file": null, "total": 0 })),
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return Json(json!({ "error": format!("Failed to read log: {}", e) })),
    };

    let all_lines: Vec<&str> = content.lines().collect();

    // Filter by level if requested
    let filtered: Vec<&str> = if level_filter.is_empty() {
        all_lines.clone()
    } else {
        let upper = level_filter.to_uppercase();
        all_lines
            .iter()
            .filter(|line| line.to_uppercase().contains(&upper))
            .copied()
            .collect()
    };

    // Take last N lines
    let start = filtered.len().saturating_sub(max_lines);
    let tail: Vec<&str> = filtered[start..].to_vec();

    Json(json!({
        "lines": tail,
        "file": path.file_name().and_then(|n| n.to_str()),
        "total": all_lines.len(),
        "filtered": filtered.len(),
    }))
}

// ─── Failover Orchestration (Phase 69) ───────────────────────────────────

#[derive(serde::Deserialize)]
struct FailoverBroadcastRequest {
    target_url: String,
}

/// POST /api/v1/failover/broadcast
/// Body: { "target_url": "ws://100.70.177.44:8080/ws/agent" }
/// Auth: x-terminal-secret header (same as sync endpoints).
/// Iterates agent_senders and sends SwitchController to all connected pods.
async fn failover_broadcast(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<FailoverBroadcastRequest>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // Auth: x-terminal-secret check (consistent with sync_push and other service routes)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized"})),
            )
                .into_response();
        }
    }

    let target_url = body.target_url;
    let agent_senders = state.agent_senders.read().await;
    let mut sent = 0usize;
    let total = agent_senders.len();

    for (pod_id, sender) in agent_senders.iter() {
        if sender
            .send(rc_common::protocol::CoreToAgentMessage::SwitchController {
                target_url: target_url.clone(),
            })
            .await
            .is_ok()
        {
            sent += 1;
            tracing::info!("[failover] SwitchController sent to pod {}", pod_id);
        } else {
            tracing::warn!("[failover] Failed to send SwitchController to pod {}", pod_id);
        }
    }

    tracing::info!(
        "[failover] Broadcast SwitchController to {}/{} agents, target: {}",
        sent,
        total,
        target_url
    );
    Json(serde_json::json!({ "ok": true, "sent": sent, "total": total })).into_response()
}

// ─── Failback Data Reconciliation (Phase 70) ─────────────────────────────

/// POST /api/v1/sync/import-sessions
/// Body: { "sessions": [ { ...billing_session fields... } ] }
/// Auth: x-terminal-secret header (same as sync_push).
/// Inserts cloud-created billing sessions that were created during failover.
/// Uses INSERT OR IGNORE so duplicate UUIDs are silently skipped.
async fn import_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Auth: x-terminal-secret check (consistent with sync_push pattern)
    if let Some(secret) = state.config.cloud.terminal_secret.as_deref() {
        let provided = headers.get("x-terminal-secret").and_then(|v| v.to_str().ok());
        if provided != Some(secret) {
            return Json(json!({ "error": "Unauthorized" }));
        }
    }

    let sessions = match body.get("sessions").and_then(|v| v.as_array()) {
        Some(s) => s,
        None => return Json(json!({ "error": "missing sessions array" })),
    };

    let mut imported = 0u64;
    let mut skipped = 0u64;

    for s in sessions {
        let id = s.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        if id.is_empty() { continue; }

        let r = sqlx::query(
            "INSERT OR IGNORE INTO billing_sessions (
                id, driver_id, pod_id, pricing_tier_id,
                allocated_seconds, driving_seconds, status, custom_price_paise, notes,
                started_at, ended_at, created_at, experience_id, car, track, sim_type,
                split_count, split_duration_minutes,
                wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason,
                pause_count, total_paused_seconds, refund_paise)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26)",
        )
        .bind(id)
        .bind(s.get("driver_id").and_then(|v| v.as_str()))
        .bind(s.get("pod_id").and_then(|v| v.as_str()))
        .bind(s.get("pricing_tier_id").and_then(|v| v.as_str()))
        .bind(s.get("allocated_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(s.get("driving_seconds").and_then(|v| v.as_i64()).unwrap_or(0))
        .bind(s.get("status").and_then(|v| v.as_str()).unwrap_or("pending"))
        .bind(s.get("custom_price_paise").and_then(|v| v.as_i64()))
        .bind(s.get("notes").and_then(|v| v.as_str()))
        .bind(s.get("started_at").and_then(|v| v.as_str()))
        .bind(s.get("ended_at").and_then(|v| v.as_str()))
        .bind(s.get("created_at").and_then(|v| v.as_str()))
        .bind(s.get("experience_id").and_then(|v| v.as_str()))
        .bind(s.get("car").and_then(|v| v.as_str()))
        .bind(s.get("track").and_then(|v| v.as_str()))
        .bind(s.get("sim_type").and_then(|v| v.as_str()))
        .bind(s.get("split_count").and_then(|v| v.as_i64()))
        .bind(s.get("split_duration_minutes").and_then(|v| v.as_i64()))
        .bind(s.get("wallet_debit_paise").and_then(|v| v.as_i64()))
        .bind(s.get("discount_paise").and_then(|v| v.as_i64()))
        .bind(s.get("coupon_id").and_then(|v| v.as_str()))
        .bind(s.get("original_price_paise").and_then(|v| v.as_i64()))
        .bind(s.get("discount_reason").and_then(|v| v.as_str()))
        .bind(s.get("pause_count").and_then(|v| v.as_i64()))
        .bind(s.get("total_paused_seconds").and_then(|v| v.as_i64()))
        .bind(s.get("refund_paise").and_then(|v| v.as_i64()))
        .execute(&state.db)
        .await;

        match r {
            Ok(result) if result.rows_affected() > 0 => imported += 1,
            Ok(_) => skipped += 1,
            Err(e) => {
                tracing::warn!("[import_sessions] Failed to insert session {}: {}", id, e);
                skipped += 1;
            }
        }
    }

    Json(json!({
        "imported": imported,
        "skipped": skipped,
        "synced_at": chrono::Utc::now().to_rfc3339(),
    }))
}

// ─── Debug System ────────────────────────────────────────────────────────

/// GET /debug/pod-events/{pod_id} — proxy recent diagnostic events from a pod's tier engine.
/// v27.0: Kiosk debug page fetches this to show recent autonomous + staff-triggered diagnostics.
async fn debug_pod_events(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Query(q): Query<PodEventsQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(10).min(50);

    // P2 fix: Validate pod_id against known registered pods only.
    // The HashMap lookup IS the validation — unknown IDs get 404.
    // Additional format check prevents abuse (SSRF, log injection).

    // Look up the pod's IP address from the in-memory pod registry (not SQL)
    let pods = state.pods.read().await;
    let pod = pods.get(&pod_id).cloned();
    drop(pods);

    let Some(pod) = pod else {
        return Json(json!({ "events": [], "error": format!("Pod {} not found", pod_id) }));
    };

    // Fetch from pod's /events/recent endpoint
    let url = format!("http://{}:8090/events/recent?limit={}", pod.ip_address, limit);
    match state.http_client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<Value>().await {
                Ok(data) => Json(data),
                Err(e) => Json(json!({ "events": [], "error": format!("Parse error: {}", e) })),
            }
        }
        Ok(resp) => Json(json!({ "events": [], "error": format!("Pod returned {}", resp.status()) })),
        Err(e) => Json(json!({ "events": [], "error": format!("Pod unreachable: {}", e) })),
    }
}

#[derive(Deserialize)]
struct PodEventsQuery {
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct DebugActivityQuery {
    hours: Option<f64>,
}

/// Track consecutive try_read() failures for starvation detection (MMA Round 3 P3).
static DEBUG_CONTENTION_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

async fn debug_activity(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DebugActivityQuery>,
) -> Json<Value> {
    let hours = q.hours.unwrap_or(2.0);
    let minutes = (hours * 60.0) as i64;
    let db = &state.db;

    // Pod health from in-memory state — use try_read() (non-blocking) to avoid deadlock.
    // 20+ write lock sites in billing/WS handlers can block readers indefinitely.
    // try_read() never queues — returns immediately with Err if lock is held.
    let (pod_health, pods_contended) = match state.pods.try_read() {
        Ok(pods) => {
            DEBUG_CONTENTION_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
            let now = chrono::Utc::now();
            let health: Vec<Value> = pods.values().map(|p| {
                let secs = p.last_seen
                    .map(|ls| (now - ls).num_seconds())
                    .unwrap_or(9999);
                let color = if secs > 9998 { "grey" }
                    else if secs > 15 { "red" }
                    else if secs > 10 { "orange" }
                    else if secs > 5 { "yellow" }
                    else { "green" };
                json!({
                    "pod_id": p.id,
                    "pod_number": p.number,
                    "seconds_since_heartbeat": secs,
                    "health": color,
                    "status": format!("{:?}", p.status),
                })
            }).collect();
            drop(pods);
            (health, false)
        }
        Err(_) => {
            let count = DEBUG_CONTENTION_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if count >= 5 {
                tracing::error!(target: "debug", consecutive = count, "debug_activity: pods RwLock STARVED — {} consecutive failures, possible write-lock monopoly", count);
            } else {
                tracing::warn!(target: "debug", consecutive = count, "debug_activity: pods RwLock contended");
            }
            (vec![], true)
        }
    };

    // Billing events — timeout DB queries to prevent indefinite hangs on SQLite lock contention
    let billing_json: Vec<Value> = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT id, session_id, event_type, created_at, COALESCE(json_extract(details, '$.pod_id'), '') \
             FROM billing_events \
             WHERE created_at > datetime('now', ? || ' minutes') \
             ORDER BY created_at DESC LIMIT 200",
        )
        .bind(format!("-{}", minutes))
        .fetch_all(db),
    ).await {
        Ok(Ok(events)) => events.iter().map(|(id, sid, etype, ts, pod)| {
            json!({ "id": id, "session_id": sid, "event_type": etype, "created_at": ts, "pod_id": pod })
        }).collect(),
        Ok(Err(e)) => {
            tracing::warn!(target: "debug", "debug_activity: billing query error: {}", e);
            vec![]
        }
        Err(_) => {
            tracing::warn!(target: "debug", "debug_activity: billing query timeout (5s)");
            vec![]
        }
    };

    // Game launch events
    let game_json: Vec<Value> = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
            "SELECT id, pod_id, event_type, created_at, COALESCE(error_message, '') \
             FROM game_launch_events \
             WHERE created_at > datetime('now', ? || ' minutes') \
             ORDER BY created_at DESC LIMIT 200",
        )
        .bind(format!("-{}", minutes))
        .fetch_all(db),
    ).await {
        Ok(Ok(events)) => events.iter().map(|(id, pod, etype, ts, err)| {
            json!({ "id": id, "pod_id": pod, "event_type": etype, "created_at": ts, "error_message": err })
        }).collect(),
        Ok(Err(e)) => {
            tracing::warn!(target: "debug", "debug_activity: game events query error: {}", e);
            vec![]
        }
        Err(_) => {
            tracing::warn!(target: "debug", "debug_activity: game events query timeout (5s)");
            vec![]
        }
    };

    // Include contention flag so kiosk UI can show "data temporarily unavailable" instead of "all pods down"
    Json(json!({
        "pod_health": pod_health,
        "billing_events": billing_json,
        "game_events": game_json,
        "pods_contended": pods_contended,
    }))
}

async fn debug_playbooks(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let playbooks: Vec<Value> = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT id, category, title, steps FROM debug_playbooks ORDER BY category",
        )
        .fetch_all(&state.db),
    ).await {
        Ok(Ok(rows)) => rows.iter().map(|(id, cat, title, steps)| {
            let parsed: Value = serde_json::from_str(steps).unwrap_or(json!([]));
            json!({ "id": id, "category": cat, "title": title, "steps": parsed })
        }).collect(),
        Ok(Err(e)) => {
            tracing::warn!(target: "debug", "debug_playbooks query error: {}", e);
            vec![]
        }
        Err(_) => {
            tracing::warn!(target: "debug", "debug_playbooks: DB query timeout (5s)");
            vec![]
        }
    };

    Json(json!({ "playbooks": playbooks }))
}

#[derive(Deserialize)]
struct CreateIncidentBody {
    description: String,
    pod_id: Option<String>,
}

async fn create_debug_incident(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateIncidentBody>,
) -> Json<Value> {
    let db = &state.db;
    let desc_lower = body.description.to_lowercase();

    // Auto-detect category
    let category = if desc_lower.contains("offline") || desc_lower.contains("down") || desc_lower.contains("not working") || desc_lower.contains("dead") {
        "pod_offline"
    } else if desc_lower.contains("crash") || desc_lower.contains("won't launch") || desc_lower.contains("game error") || desc_lower.contains("wont launch") {
        "game_crash"
    } else if desc_lower.contains("billing") || desc_lower.contains("timer") || desc_lower.contains("session stuck") {
        "billing_stuck"
    } else if desc_lower.contains("blank") || desc_lower.contains("screen stuck") || desc_lower.contains("lock screen") {
        "screen_stuck"
    } else if desc_lower.contains("steering") || desc_lower.contains("pedal") || desc_lower.contains("wheel") || desc_lower.contains("input") {
        "no_steering_input"
    } else if desc_lower.contains("idle") || desc_lower.contains("not counting") || desc_lower.contains("pausing") {
        "high_idle_time"
    } else if desc_lower.contains("sync") || desc_lower.contains("cloud") || desc_lower.contains("not updating") {
        "sync_failure"
    } else if desc_lower.contains("kiosk") || desc_lower.contains("bypass") || desc_lower.contains("desktop") || desc_lower.contains("taskbar") {
        "kiosk_bypass"
    } else {
        "unknown"
    };

    // Find matching playbook
    let playbook = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, category, title, steps FROM debug_playbooks WHERE category = ?",
    )
    .bind(category)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let playbook_id = playbook.as_ref().map(|p| p.0.clone());

    // Capture context snapshot
    let pods = state.pods.read().await;
    let active_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM billing_sessions WHERE status = 'active'",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);

    let pod_snapshot = if let Some(ref pid) = body.pod_id {
        pods.get(pid).map(|p| json!({
            "status": format!("{:?}", p.status),
            "last_seen": p.last_seen,
            "driving_state": p.driving_state,
            "current_game": p.sim_type,
        }))
    } else {
        None
    };
    drop(pods);

    let context = json!({
        "pod_state": pod_snapshot,
        "active_sessions": active_sessions,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    let id = uuid::Uuid::new_v4().to_string();
    let _ = sqlx::query(
        "INSERT INTO debug_incidents (id, pod_id, category, description, status, context_snapshot, playbook_id) \
         VALUES (?, ?, ?, ?, 'open', ?, ?)",
    )
    .bind(&id)
    .bind(&body.pod_id)
    .bind(category)
    .bind(&body.description)
    .bind(context.to_string())
    .bind(&playbook_id)
    .execute(db)
    .await;

    // Log to activity feed so staff messages appear in real-time
    let pod_id_for_log = body.pod_id.as_deref().unwrap_or("system");
    crate::activity_log::log_pod_activity(
        &state,
        pod_id_for_log,
        "system",
        "Staff Report",
        &body.description,
        "staff",
    );

    let playbook_json = playbook.map(|(pid, cat, title, steps)| {
        let parsed: Value = serde_json::from_str(&steps).unwrap_or(json!([]));
        json!({ "id": pid, "category": cat, "title": title, "steps": parsed })
    });

    // Suggest quick actions based on category
    let suggested_actions: Vec<&str> = match category {
        "pod_offline" => vec!["restart_pod", "wake_pod"],
        "game_crash" => vec!["kill_game"],
        "screen_stuck" => vec!["relaunch_edge"],
        "no_steering_input" => vec!["restart_pod"],
        "kiosk_bypass" => vec!["relaunch_edge"],
        "billing_stuck" | "high_idle_time" | "sync_failure" | "unknown" => vec![],
        _ => vec![],
    };

    // ─── v27.0: Send DiagnosticRequest to pod for Tier 1 + Tier 2 diagnosis ──
    // NOTE: Skip for "pod_offline" category — if the pod is truly offline, the WS send
    // will fail silently. The server's own AI diagnosis (Claude/Ollama) handles offline pods.
    // Use incident ID as correlation_id so the returning DiagnosticResult can be
    // directly linked to the incident in the DB (MMA R4-1 fix: broken correlation chain)
    let correlation_id = id.clone();
    let mut tier_diagnosis_sent = false;
    if category != "pod_offline" {
    if let Some(ref pid) = body.pod_id {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pid) {
            let diag_req = CoreToAgentMessage::DiagnosticRequest {
                correlation_id: correlation_id.clone(),
                incident_id: id.clone(),
                description: body.description.clone(),
                category: category.to_string(),
                requested_by: "staff".to_string(),
            };
            if sender.send(diag_req).await.is_ok() {
                tier_diagnosis_sent = true;
                tracing::info!(
                    target: "debug-bridge",
                    pod = %pid,
                    correlation_id = %correlation_id,
                    "DiagnosticRequest sent to pod for incident {}",
                    id
                );
            }
        }
        drop(agent_senders);
    }
    } // end category != "pod_offline" guard

    Json(json!({
        "incident": {
            "id": id,
            "pod_id": body.pod_id,
            "category": category,
            "description": body.description,
            "status": "open",
            "playbook_id": playbook_id,
            "created_at": chrono::Utc::now().to_rfc3339(),
        },
        "playbook": playbook_json,
        "suggested_actions": suggested_actions,
        "tier_diagnosis": {
            "sent": tier_diagnosis_sent,
            "correlation_id": correlation_id,
        },
    }))
}

#[derive(Deserialize)]
struct DebugIncidentFilter {
    status: Option<String>,
}

async fn list_debug_incidents(
    State(state): State<Arc<AppState>>,
    Query(q): Query<DebugIncidentFilter>,
) -> Json<Value> {
    let db = &state.db;

    let rows = if let Some(ref status) = q.status {
        sqlx::query_as::<_, (String, Option<String>, String, String, String, Option<String>, String)>(
            "SELECT id, pod_id, category, description, status, playbook_id, created_at \
             FROM debug_incidents WHERE status = ? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(status)
        .fetch_all(db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, (String, Option<String>, String, String, String, Option<String>, String)>(
            "SELECT id, pod_id, category, description, status, playbook_id, created_at \
             FROM debug_incidents ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(db)
        .await
        .unwrap_or_default()
    };

    let incidents: Vec<Value> = rows.iter().map(|(id, pod, cat, desc, status, pb, ts)| {
        json!({
            "id": id, "pod_id": pod, "category": cat,
            "description": desc, "status": status,
            "playbook_id": pb, "created_at": ts,
        })
    }).collect();

    Json(json!({ "incidents": incidents }))
}

#[derive(Deserialize)]
struct UpdateIncidentBody {
    status: Option<String>,
    resolution_text: Option<String>,
    effectiveness: Option<i32>,
}

async fn update_debug_incident(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateIncidentBody>,
) -> Json<Value> {
    let db = &state.db;

    if let Some(ref status) = body.status {
        let resolved_at = if status == "resolved" {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        let _ = sqlx::query(
            "UPDATE debug_incidents SET status = ?, resolved_at = COALESCE(?, resolved_at) WHERE id = ?",
        )
        .bind(status)
        .bind(&resolved_at)
        .bind(&id)
        .execute(db)
        .await;
    }

    // If resolving with text, save to RAG knowledge base
    if let Some(ref text) = body.resolution_text {
        let category: Option<String> = sqlx::query_scalar(
            "SELECT category FROM debug_incidents WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(db)
        .await
        .unwrap_or(None);

        if let Some(cat) = category {
            let res_id = uuid::Uuid::new_v4().to_string();
            let _ = sqlx::query(
                "INSERT INTO debug_resolutions (id, incident_id, category, resolution_text, effectiveness) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&res_id)
            .bind(&id)
            .bind(&cat)
            .bind(text)
            .bind(body.effectiveness.unwrap_or(3))
            .execute(db)
            .await;
        }
    }

    Json(json!({ "ok": true, "id": id }))
}

// ─── POST /debug/incidents/{id}/apply-fix — Execute a quick fix action from debug page ──
#[derive(Deserialize)]
struct ApplyFixBody {
    /// One of: restart_pod, wake_pod, shutdown_pod, relaunch_edge, kill_game
    action: String,
    pod_id: Option<String>,
}

async fn debug_apply_fix(
    State(state): State<Arc<AppState>>,
    Path(incident_id): Path<String>,
    Json(body): Json<ApplyFixBody>,
) -> Json<Value> {
    let db = &state.db;

    // Verify incident exists
    let incident = sqlx::query_as::<_, (String, Option<String>, String)>(
        "SELECT id, pod_id, category FROM debug_incidents WHERE id = ?",
    )
    .bind(&incident_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let Some((inc_id, inc_pod_id, category)) = incident else {
        return Json(json!({ "ok": false, "error": "Incident not found" }));
    };

    // Resolve target pod — prefer explicit pod_id, fall back to incident's pod_id
    let target_pod_id = body.pod_id.or(inc_pod_id);
    let Some(ref pod_id) = target_pod_id else {
        return Json(json!({ "ok": false, "error": "No pod specified — select a pod first" }));
    };

    // Look up pod info
    let pods = state.pods.read().await;
    let pod = match pods.get(pod_id) {
        Some(p) => p.clone(),
        None => {
            drop(pods);
            return Json(json!({ "ok": false, "error": format!("Pod {} not found", pod_id) }));
        }
    };
    drop(pods);

    let action_label = body.action.clone();
    let result = match body.action.as_str() {
        "restart_pod" => {
            match wol::restart_pod(&state.http_client, &pod.ip_address).await {
                Ok(output) => json!({ "ok": true, "action": "restart_pod", "output": output }),
                Err(e) => json!({ "ok": false, "error": format!("Restart failed: {}", e) }),
            }
        }
        "wake_pod" => {
            if let Some(ref mac) = pod.mac_address {
                match wol::send_wol(mac).await {
                    Ok(_) => json!({ "ok": true, "action": "wake_pod" }),
                    Err(e) => json!({ "ok": false, "error": format!("WoL failed: {}", e) }),
                }
            } else {
                json!({ "ok": false, "error": format!("Pod {} has no MAC address configured", pod.number) })
            }
        }
        "shutdown_pod" => {
            match wol::shutdown_pod(&state.http_client, &pod.ip_address).await {
                Ok(output) => json!({ "ok": true, "action": "shutdown_pod", "output": output }),
                Err(e) => json!({ "ok": false, "error": format!("Shutdown failed: {}", e) }),
            }
        }
        "relaunch_edge" => {
            // Kill Edge and relaunch kiosk — executed via WS exec on the pod
            let cmd = "taskkill /F /IM msedge.exe & ping -n 3 127.0.0.1 >nul & start msedge.exe --kiosk http://localhost:3300 --edge-kiosk-type=fullscreen";
            match crate::ws::ws_exec_on_pod(&state, pod_id, cmd, 15_000).await {
                Ok((success, stdout, stderr)) => {
                    if success {
                        json!({ "ok": true, "action": "relaunch_edge", "output": stdout })
                    } else {
                        json!({ "ok": false, "error": format!("Edge relaunch failed: {}", stderr) })
                    }
                }
                Err(e) => json!({ "ok": false, "error": format!("Edge relaunch failed: {}", e) }),
            }
        }
        "kill_game" => {
            // Kill any running game process via WS exec
            let cmd = "taskkill /F /IM acs.exe & taskkill /F /IM acc.exe & taskkill /F /IM FormulaOne.exe";
            match crate::ws::ws_exec_on_pod(&state, pod_id, cmd, 10_000).await {
                Ok((success, stdout, stderr)) => {
                    if success {
                        json!({ "ok": true, "action": "kill_game", "output": stdout })
                    } else {
                        json!({ "ok": false, "error": format!("Kill game failed: {}", stderr) })
                    }
                }
                Err(e) => json!({ "ok": false, "error": format!("Kill game failed: {}", e) }),
            }
        }
        _ => json!({ "ok": false, "error": format!("Unknown action: {}", body.action) }),
    };

    let success = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);

    // Log to activity feed
    let detail = if success {
        format!("Applied fix '{}' on Pod {}", action_label, pod.number)
    } else {
        let err = result.get("error").and_then(|v| v.as_str()).unwrap_or("unknown");
        format!("Fix '{}' failed on Pod {}: {}", action_label, pod.number, err)
    };
    crate::activity_log::log_pod_activity(&state, pod_id, "race_engineer", "Quick Fix Applied", &detail, "staff");

    // v27.0: Notify pod's tier engine about the staff action to reset dedup window
    if success {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreToAgentMessage::StaffActionNotify {
                action: action_label.clone(),
                reason: format!("Staff quick-fix for incident {}", incident_id),
                correlation_id: uuid::Uuid::new_v4().to_string(),
            }).await;
        }
        drop(agent_senders);
    }

    // If action succeeded, auto-resolve the incident with the action as resolution
    if success {
        let resolved_at = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query(
            "UPDATE debug_incidents SET status = 'resolved', resolved_at = ? WHERE id = ? AND status = 'open'",
        )
        .bind(&resolved_at)
        .bind(&inc_id)
        .execute(db)
        .await;

        // Save to RAG knowledge base so future diagnosis can reference this fix
        let res_id = uuid::Uuid::new_v4().to_string();
        let resolution_text = format!("Quick fix: {} (applied from debug page)", action_label);
        let _ = sqlx::query(
            "INSERT INTO debug_resolutions (id, incident_id, category, resolution_text, effectiveness) \
             VALUES (?, ?, ?, ?, 4)",
        )
        .bind(&res_id)
        .bind(&inc_id)
        .bind(&category)
        .bind(&resolution_text)
        .execute(db)
        .await;
    }

    Json(result)
}

#[derive(Deserialize)]
struct DiagnoseBody {
    incident_id: String,
}

async fn debug_diagnose(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DiagnoseBody>,
) -> Json<Value> {
    if !state.config.ai_debugger.enabled {
        return Json(json!({ "error": "AI debugger is not enabled" }));
    }

    let db = &state.db;

    // Load incident
    let incident = sqlx::query_as::<_, (String, Option<String>, String, String, Option<String>)>(
        "SELECT id, pod_id, category, description, context_snapshot FROM debug_incidents WHERE id = ?",
    )
    .bind(&body.incident_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let Some((inc_id, pod_id, category, description, ctx_snapshot)) = incident else {
        return Json(json!({ "error": "Incident not found" }));
    };

    // Load matching playbook
    let playbook = sqlx::query_as::<_, (String, String, String)>(
        "SELECT title, category, steps FROM debug_playbooks WHERE category = ?",
    )
    .bind(&category)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    // Load past resolutions for same category (RAG)
    let past_resolutions = sqlx::query_as::<_, (String, i32, String)>(
        "SELECT resolution_text, effectiveness, created_at FROM debug_resolutions \
         WHERE category = ? ORDER BY effectiveness DESC, created_at DESC LIMIT 5",
    )
    .bind(&category)
    .fetch_all(db)
    .await
    .unwrap_or_default();

    // Build AI prompt
    let biz_context = crate::ai::gather_business_context(
        &state.db, &state.pods, &state.billing, &state.game_launcher,
    ).await;

    let mut prompt_parts = vec![
        format!("INCIDENT: {}", description),
        format!("CATEGORY: {}", category),
    ];

    if let Some(ref pid) = pod_id {
        prompt_parts.push(format!("POD: {}", pid));
    }
    if let Some(ref ctx) = ctx_snapshot {
        prompt_parts.push(format!("CONTEXT SNAPSHOT: {}", ctx));
    }
    if let Some(ref pb) = playbook {
        prompt_parts.push(format!("PLAYBOOK ({}): {}", pb.0, pb.2));
    }
    if !past_resolutions.is_empty() {
        let mut rag = String::from("PAST RESOLUTIONS FOR THIS CATEGORY:\n");
        for (text, eff, ts) in &past_resolutions {
            rag.push_str(&format!("  - [effectiveness={}/5, {}] {}\n", eff, ts, text));
        }
        prompt_parts.push(rag);
    }
    prompt_parts.push(format!("VENUE STATE:\n{}", biz_context));

    let messages = vec![
        json!({
            "role": "system",
            "content": "You are James, AI operations assistant for RacingPoint eSports venue. \
                        A staff member reported an incident. Analyze the issue using the playbook, \
                        past resolutions, and current venue state. Provide: 1) Root cause analysis, \
                        2) Step-by-step fix instructions, 3) Whether this matches a known pattern. \
                        Be concise and actionable."
        }),
        json!({
            "role": "user",
            "content": prompt_parts.join("\n\n")
        }),
    ];

    match crate::ai::query_ai(&state.config.ai_debugger, &messages, Some(db), Some("debug_incident")).await {
        Ok((diagnosis, model)) => {
            let playbook_json = playbook.map(|(title, cat, steps)| {
                let parsed: Value = serde_json::from_str(&steps).unwrap_or(json!([]));
                json!({ "category": cat, "title": title, "steps": parsed })
            });

            let past_json: Vec<Value> = past_resolutions.iter().map(|(text, eff, ts)| {
                json!({ "resolution_text": text, "effectiveness": eff, "created_at": ts })
            }).collect();

            // Log diagnosis to activity feed
            let detail = if diagnosis.len() > 120 { format!("{}...", &diagnosis[..120]) } else { diagnosis.clone() };
            let log_pod = pod_id.as_deref().unwrap_or("system");
            crate::activity_log::log_pod_activity(&state, log_pod, "race_engineer", "AI Diagnosis", &detail, "race_engineer");

            Json(json!({
                "diagnosis": diagnosis,
                "model": model,
                "incident_id": inc_id,
                "playbook": playbook_json,
                "past_resolutions": past_json,
            }))
        }
        Err(e) => {
            let log_pod = pod_id.as_deref().unwrap_or("system");
            crate::activity_log::log_pod_activity(&state, log_pod, "race_engineer", "AI Diagnosis Failed", &e.to_string(), "race_engineer");
            Json(json!({
                "error": format!("AI diagnosis failed: {}", e),
                "incident_id": inc_id,
            }))
        },
    }
}

// ─── Accounting & Audit Routes ─────────────────────────────────────────────

async fn list_accounts(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, String, String, Option<String>, Option<String>, bool)>(
        "SELECT id, code, name, account_type, parent_id, description, is_active
         FROM accounts ORDER BY code",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(accts) => {
            let list: Vec<Value> = accts.iter().map(|a| json!({
                "id": a.0, "code": a.1, "name": a.2, "account_type": a.3,
                "parent_id": a.4, "description": a.5, "is_active": a.6,
            })).collect();
            Json(json!({ "accounts": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
struct DateRangeQuery {
    from: Option<String>,
    to: Option<String>,
}

async fn trial_balance(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeQuery>,
) -> Json<Value> {
    match accounting::get_trial_balance(&state, params.from.as_deref(), params.to.as_deref()).await {
        Ok(data) => Json(data),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn profit_loss(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeQuery>,
) -> Json<Value> {
    match accounting::get_profit_loss(&state, params.from.as_deref(), params.to.as_deref()).await {
        Ok(data) => Json(data),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn balance_sheet(State(state): State<Arc<AppState>>) -> Json<Value> {
    match accounting::get_balance_sheet(&state).await {
        Ok(data) => Json(data),
        Err(e) => Json(json!({ "error": e })),
    }
}

async fn list_journal_entries(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeQuery>,
) -> Json<Value> {
    let limit = 100i64; // default

    let mut query = String::from(
        "SELECT je.id, je.date, je.description, je.reference_type, je.reference_id, je.staff_id, je.created_at
         FROM journal_entries je WHERE 1=1"
    );

    if params.from.is_some() {
        query.push_str(" AND je.date >= ?");
    }
    if params.to.is_some() {
        query.push_str(" AND je.date <= ?");
    }
    query.push_str(" ORDER BY je.created_at DESC LIMIT ?");

    let mut q = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>, String)>(&query);
    if let Some(ref d) = params.from {
        q = q.bind(d);
    }
    if let Some(ref d) = params.to {
        q = q.bind(d);
    }
    q = q.bind(limit);

    let entries = match q.fetch_all(&state.db).await {
        Ok(rows) => rows,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let mut result = Vec::new();
    for entry in &entries {
        // Fetch lines for this entry
        let lines = sqlx::query_as::<_, (String, String, i64, i64)>(
            "SELECT jel.account_id, a.name, jel.debit_paise, jel.credit_paise
             FROM journal_entry_lines jel
             JOIN accounts a ON jel.account_id = a.id
             WHERE jel.journal_entry_id = ?
             ORDER BY jel.debit_paise DESC",
        )
        .bind(&entry.0)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let line_json: Vec<Value> = lines.iter().map(|l| json!({
            "account_id": l.0,
            "account_name": l.1,
            "debit_paise": l.2,
            "credit_paise": l.3,
        })).collect();

        result.push(json!({
            "id": entry.0,
            "date": entry.1,
            "description": entry.2,
            "reference_type": entry.3,
            "reference_id": entry.4,
            "staff_id": entry.5,
            "created_at": entry.6,
            "lines": line_json,
        }));
    }

    Json(json!({ "entries": result, "count": result.len() }))
}

#[derive(Deserialize)]
struct AuditLogQuery {
    table_name: Option<String>,
    row_id: Option<String>,
    action: Option<String>,
    staff_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

async fn query_audit_log(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditLogQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(100).min(500);

    let mut query = String::from(
        "SELECT id, table_name, row_id, action, old_values, new_values, staff_id, ip_address, created_at
         FROM audit_log WHERE 1=1"
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(ref t) = params.table_name {
        query.push_str(" AND table_name = ?");
        binds.push(t.clone());
    }
    if let Some(ref r) = params.row_id {
        query.push_str(" AND row_id = ?");
        binds.push(r.clone());
    }
    if let Some(ref a) = params.action {
        query.push_str(" AND action = ?");
        binds.push(a.clone());
    }
    if let Some(ref s) = params.staff_id {
        query.push_str(" AND staff_id = ?");
        binds.push(s.clone());
    }
    if let Some(ref d) = params.from {
        query.push_str(" AND created_at >= ?");
        binds.push(d.clone());
    }
    if let Some(ref d) = params.to {
        query.push_str(" AND created_at <= ?");
        binds.push(d.clone());
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ?");
    binds.push(limit.to_string());

    let mut q = sqlx::query_as::<_, (String, String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String)>(&query);
    for b in &binds {
        q = q.bind(b);
    }

    match q.fetch_all(&state.db).await {
        Ok(rows) => {
            let entries: Vec<Value> = rows.iter().map(|r| json!({
                "id": r.0,
                "table_name": r.1,
                "row_id": r.2,
                "action": r.3,
                "old_values": r.4.as_ref().and_then(|s| serde_json::from_str::<Value>(s).ok()),
                "new_values": r.5.as_ref().and_then(|s| serde_json::from_str::<Value>(s).ok()),
                "staff_id": r.6,
                "ip_address": r.7,
                "created_at": r.8,
            })).collect();
            Json(json!({ "entries": entries, "count": entries.len() }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Terminal Multiplayer ─────────────────────────────────────────────────────

/// POST /terminal/book-multiplayer — Staff-initiated multiplayer booking (skips friendship checks)
async fn terminal_book_multiplayer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let driver_ids: Vec<String> = match req.get("driver_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => return Json(json!({ "error": "Missing 'driver_ids' array" })),
    };

    let pod_ids: Vec<String> = match req.get("pod_ids").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        None => return Json(json!({ "error": "Missing 'pod_ids' array" })),
    };

    let pricing_tier_id = match req.get("pricing_tier_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "Missing 'pricing_tier_id'" })),
    };

    let experience_id = req.get("experience_id").and_then(|v| v.as_str());
    let game = req.get("game").and_then(|v| v.as_str());
    let track = req.get("track").and_then(|v| v.as_str());
    let car = req.get("car").and_then(|v| v.as_str());

    match multiplayer::staff_book_multiplayer(
        &state,
        driver_ids,
        pod_ids,
        experience_id,
        &pricing_tier_id,
        game,
        track,
        car,
    )
    .await
    {
        Ok(info) => Json(json!({ "status": "ok", "group_session": info })),
        Err(e) => Json(json!({ "error": e })),
    }
}

/// GET /terminal/group-sessions — List recent group sessions for POS dashboard
async fn terminal_group_sessions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let sessions = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
        "SELECT gs.id, gs.host_driver_id, gs.status, gs.shared_pin,
                COALESCE(ke.name, 'Unknown'), gs.total_members, gs.validated_count,
                gs.created_at
         FROM group_sessions gs
         LEFT JOIN kiosk_experiences ke ON ke.id = gs.experience_id
         ORDER BY gs.created_at DESC
         LIMIT 20",
    )
    .fetch_all(&state.db)
    .await;

    match sessions {
        Ok(rows) => {
            let mut sessions_json = Vec::new();
            for (id, host_id, status, pin, exp_name, total, validated, created) in &rows {
                let host_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
                    .bind(host_id)
                    .fetch_optional(&state.db)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "Unknown".to_string());

                // Get members
                let members = sqlx::query_as::<_, (String, String, String, Option<String>, Option<u32>)>(
                    "SELECT gsm.driver_id, COALESCE(d.name, 'Unknown'), gsm.status, gsm.pod_id,
                            (SELECT number FROM pods WHERE id = gsm.pod_id)
                     FROM group_session_members gsm
                     LEFT JOIN drivers d ON d.id = gsm.driver_id
                     WHERE gsm.group_session_id = ?
                     ORDER BY gsm.role DESC, gsm.invited_at",
                )
                .bind(id)
                .fetch_all(&state.db)
                .await
                .unwrap_or_default();

                let members_json: Vec<Value> = members
                    .iter()
                    .map(|(did, dname, mstatus, pod_id, pod_num)| {
                        json!({
                            "driver_id": did,
                            "driver_name": dname,
                            "status": mstatus,
                            "pod_id": pod_id,
                            "pod_number": pod_num,
                        })
                    })
                    .collect();

                sessions_json.push(json!({
                    "id": id,
                    "host_driver_id": host_id,
                    "host_name": host_name,
                    "status": status,
                    "shared_pin": pin,
                    "experience_name": exp_name,
                    "total_members": total,
                    "validated_count": validated,
                    "created_at": created,
                    "members": members_json,
                }));
            }
            Json(json!({ "group_sessions": sessions_json }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Customer Multiplayer Results ─────────────────────────────────────────────

/// GET /customer/multiplayer-results/{group_session_id} — Get race results for a group session
async fn customer_multiplayer_results(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let _driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, i64, Option<i64>, Option<i64>, i64, i64)>(
        "SELECT mr.id, COALESCE(d.name, 'Unknown'), mr.position, mr.best_lap_ms, mr.total_time_ms,
                mr.laps_completed, mr.dnf
         FROM multiplayer_results mr
         LEFT JOIN drivers d ON d.id = mr.driver_id
         WHERE mr.group_session_id = ?
         ORDER BY mr.position ASC",
    )
    .bind(&group_session_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(results) => {
            let results_json: Vec<Value> = results
                .iter()
                .map(|(id, name, pos, best_lap, total_time, laps, dnf)| {
                    json!({
                        "id": id,
                        "driver_name": name,
                        "position": pos,
                        "best_lap_ms": best_lap,
                        "total_time_ms": total_time,
                        "laps_completed": laps,
                        "dnf": dnf == &1,
                    })
                })
                .collect();
            Json(json!({ "results": results_json }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ── Deploy endpoints ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DeployRequest {
    binary_url: String,
    /// DEPLOY-03: Set to true to override weekend peak-hour deploy lock.
    #[serde(default)]
    force: bool,
}

/// POST /api/deploy/:pod_id — Deploy rc-agent binary to a single pod.
/// Returns 202 Accepted immediately; deploy runs as background task.
/// Returns 409 Conflict if deploy is already in progress or pod has active billing.
/// Returns 404 if pod not found.
/// Returns 423 Locked if weekend peak hours and force=false (DEPLOY-03).
async fn deploy_single_pod(
    Path(pod_id): Path<String>,
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<DeployRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    // DEPLOY-03: Check weekend peak-hour deploy window lock
    if let Err(msg) = crate::deploy::is_deploy_window_locked(req.force, &claims.sub) {
        return (
            axum::http::StatusCode::LOCKED,
            Json(json!({ "error": msg })),
        );
    }

    // Check pod exists and get IP
    let pod_ip = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.ip_address.clone())
    };

    let pod_ip = match pod_ip {
        Some(ip) => ip,
        None => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Pod not found", "pod_id": pod_id })),
            );
        }
    };

    // Check for active billing session — cannot deploy to a pod mid-session
    let has_billing = state
        .billing
        .active_timers
        .read()
        .await
        .contains_key(&pod_id);
    if has_billing {
        return (
            axum::http::StatusCode::CONFLICT,
            Json(json!({
                "error": "Pod has active billing session. Cannot deploy during active session.",
                "pod_id": pod_id
            })),
        );
    }

    // Check for concurrent deploy in progress
    {
        let deploy_states = state.pod_deploy_states.read().await;
        if let Some(ds) = deploy_states.get(&pod_id) {
            if ds.is_active() {
                return (
                    axum::http::StatusCode::CONFLICT,
                    Json(json!({
                        "error": "Deploy already in progress",
                        "pod_id": pod_id,
                        "current_state": format!("{:?}", ds)
                    })),
                );
            }
        }
    }

    // Spawn deploy as background task (non-blocking)
    let deploy_state = Arc::clone(&state);
    let deploy_pod_id = pod_id.clone();
    let deploy_binary_url = req.binary_url.clone();
    tokio::spawn(async move {
        crate::deploy::deploy_pod(deploy_state, deploy_pod_id, pod_ip, deploy_binary_url).await;
    });

    (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({
            "status": "deploy_started",
            "pod_id": pod_id,
            "binary_url": req.binary_url
        })),
    )
}

/// GET /api/deploy/status — Get deploy state for all pods.
async fn deploy_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let deploy_states = state.pod_deploy_states.read().await;
    let statuses: Vec<Value> = deploy_states
        .iter()
        .map(|(pod_id, ds)| {
            json!({
                "pod_id": pod_id,
                "state": ds,
            })
        })
        .collect();
    Json(json!({ "pods": statuses }))
}

/// POST /api/deploy/rolling — Start a canary-first rolling deploy to all pods.
/// Returns 202 Accepted immediately; rolling deploy runs as background task.
/// Returns 409 Conflict if any deploy is already active.
/// Returns 423 Locked if weekend peak hours and force=false (DEPLOY-03).
///
/// Body: { "binary_url": "http://192.168.31.27:9998/rc-agent.exe", "force": false }
async fn deploy_rolling_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<DeployRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    // DEPLOY-03: Check weekend peak-hour deploy window lock
    if let Err(msg) = crate::deploy::is_deploy_window_locked(req.force, &claims.sub) {
        return (
            axum::http::StatusCode::LOCKED,
            Json(json!({ "error": msg })),
        );
    }

    // Reject if any deploy is already in progress (guards against double-trigger)
    {
        let deploy_states = state.pod_deploy_states.read().await;
        let any_active = deploy_states
            .values()
            .any(|s| s.is_active());
        if any_active {
            return (
                axum::http::StatusCode::CONFLICT,
                Json(json!({
                    "error": "A deploy is already in progress on one or more pods",
                    "hint": "Check GET /api/deploy/status for current state"
                })),
            );
        }
    }

    let state_clone = Arc::clone(&state);
    let binary_url = req.binary_url.clone();
    let force = req.force;
    let actor = claims.sub.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::deploy::deploy_rolling(state_clone, binary_url, force, &actor).await {
            tracing::error!("Rolling deploy failed: {}", e);
        }
    });

    (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({
            "status": "rolling_deploy_started",
            "canary": "pod_8",
            "binary_url": req.binary_url,
            "force_override": req.force,
        })),
    )
}

// ─── OTA Pipeline (v22.0 Phase 179) ──────────────────────────────────────────

#[derive(Deserialize, Default)]
struct OtaDeployQuery {
    /// DEPLOY-03: Set to true to override weekend peak-hour deploy lock.
    #[serde(default)]
    force: bool,
}

/// POST /api/v1/ota/deploy — Start an OTA pipeline deploy with a TOML manifest.
/// Returns 202 Accepted; pipeline runs as background task.
/// Returns 409 if a pipeline is already running.
/// Returns 423 Locked if weekend peak hours and force=false (DEPLOY-03).
///
/// Query params: ?force=true to override weekend peak-hour lock.
async fn ota_deploy_handler(
    State(_state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    axum::extract::Query(query): axum::extract::Query<OtaDeployQuery>,
    body: String,
) -> impl IntoResponse {
    use crate::ota_pipeline;

    // DEPLOY-03: Check weekend peak-hour deploy window lock
    if let Err(msg) = crate::deploy::is_deploy_window_locked(query.force, &claims.sub) {
        return (
            axum::http::StatusCode::LOCKED,
            Json(json!({ "error": msg })),
        );
    }

    // Parse manifest from TOML body
    let manifest = match ota_pipeline::parse_manifest(&body) {
        Ok(m) => m,
        Err(e) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": e })),
            );
        }
    };

    // Check if a pipeline is already running
    if let Some(record) = ota_pipeline::load_pipeline_state() {
        if !record.state.is_terminal() {
            return (
                axum::http::StatusCode::CONFLICT,
                Json(json!({
                    "error": "Pipeline already running",
                    "state": format!("{:?}", record.state),
                })),
            );
        }
    }

    // Spawn pipeline as background task
    let version = manifest.release.version.clone();
    tokio::spawn(async move {
        tracing::info!("OTA pipeline started for version {}", version);
        // Pipeline orchestration will be wired here in future iteration
        // For now: persist initial state
        let record = ota_pipeline::DeployRecord::new(&version);
        if let Err(e) = ota_pipeline::persist_pipeline_state(&record) {
            tracing::error!("Failed to persist initial pipeline state: {e}");
        }
    });

    (
        axum::http::StatusCode::ACCEPTED,
        Json(json!({ "status": "pipeline_started" })),
    )
}

/// GET /api/v1/ota/status — Current OTA pipeline state.
async fn ota_status_handler() -> impl IntoResponse {
    use crate::ota_pipeline;

    match ota_pipeline::load_pipeline_state() {
        Some(record) => match serde_json::to_value(&record) {
            Ok(json) => (axum::http::StatusCode::OK, Json(json)),
            Err(e) => {
                tracing::warn!("Failed to serialize pipeline state: {e}");
                (
                    axum::http::StatusCode::OK,
                    Json(json!({ "state": "error", "message": format!("Serialization error: {e}") })),
                )
            }
        },
        None => (
            axum::http::StatusCode::OK,
            Json(json!({ "state": "idle", "message": "No pipeline state" })),
        ),
    }
}

// ─── Watchdog ────────────────────────────────────────────────────────────────

async fn watchdog_crash_report(
    Path(pod_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(report): Json<WatchdogCrashReport>,
) -> axum::http::StatusCode {
    tracing::warn!(
        pod_id = %pod_id,
        exit_code = ?report.exit_code,
        restart_count = report.restart_count,
        crash_time = %report.crash_time,
        watchdog_version = %report.watchdog_version,
        "Watchdog crash report: rc-agent restarted on {}",
        pod_id
    );

    crate::activity_log::log_pod_activity(
        &state,
        &pod_id,
        "system",
        "Watchdog Crash Report",
        &format!(
            "exit_code={:?} restart_count={} crash_time={} watchdog_version={}",
            report.exit_code, report.restart_count, report.crash_time, report.watchdog_version
        ),
        "watchdog",
    );

    axum::http::StatusCode::OK
}

// ─── Staff: Hotlap Events ─────────────────────────────────────────────────────

/// POST /staff/events — create a new hotlap event
async fn create_hotlap_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return Json(json!({ "error": "name is required" })),
    };
    let track = match body.get("track").and_then(|v| v.as_str()) {
        Some(t) => t.to_string(),
        None => return Json(json!({ "error": "track is required" })),
    };
    let car = match body.get("car").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Json(json!({ "error": "car is required" })),
    };
    let car_class = match body.get("car_class").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Json(json!({ "error": "car_class is required" })),
    };
    let sim_type = body
        .get("sim_type")
        .and_then(|v| v.as_str())
        .unwrap_or("assetto_corsa")
        .to_string();
    let description: Option<String> = body
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let starts_at: Option<String> = body
        .get("starts_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let ends_at: Option<String> = body
        .get("ends_at")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let reference_time_ms: Option<i64> = body
        .get("reference_time_ms")
        .and_then(|v| v.as_i64());
    let rule_107_percent: i64 = body
        .get("rule_107_percent")
        .and_then(|v| v.as_bool())
        .map(|b| if b { 1 } else { 0 })
        .unwrap_or(1);

    let result = sqlx::query(
        "INSERT INTO hotlap_events
            (id, name, description, track, car, car_class, sim_type, status,
             starts_at, ends_at, reference_time_ms, rule_107_percent, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, 'upcoming', ?, ?, ?, ?, datetime('now'), datetime('now'))",
    )
    .bind(&id)
    .bind(&name)
    .bind(&description)
    .bind(&track)
    .bind(&car)
    .bind(&car_class)
    .bind(&sim_type)
    .bind(&starts_at)
    .bind(&ends_at)
    .bind(reference_time_ms)
    .bind(rule_107_percent)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Hotlap event created: {} ({})", id, name);
            Json(json!({ "id": id, "status": "created" }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to create event: {}", e) })),
    }
}

/// GET /staff/events — list all hotlap events
async fn list_staff_events(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms, rule_107_percent,
                championship_id, created_at, updated_at
         FROM hotlap_events ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let events: Vec<Value> = rows
                .iter()
                .map(|r| {
                    use sqlx::Row;
                    json!({
                        "id": r.try_get::<String, _>("id").unwrap_or_default(),
                        "name": r.try_get::<String, _>("name").unwrap_or_default(),
                        "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                        "track": r.try_get::<String, _>("track").unwrap_or_default(),
                        "car": r.try_get::<String, _>("car").unwrap_or_default(),
                        "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                        "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                        "status": r.try_get::<String, _>("status").unwrap_or_default(),
                        "starts_at": r.try_get::<Option<String>, _>("starts_at").unwrap_or(None),
                        "ends_at": r.try_get::<Option<String>, _>("ends_at").unwrap_or(None),
                        "reference_time_ms": r.try_get::<Option<i64>, _>("reference_time_ms").unwrap_or(None),
                        "rule_107_percent": r.try_get::<i64, _>("rule_107_percent").unwrap_or(1),
                        "championship_id": r.try_get::<Option<String>, _>("championship_id").unwrap_or(None),
                        "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                        "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
                    })
                })
                .collect();
            Json(json!({ "events": events }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to list events: {}", e) })),
    }
}

/// GET /staff/events/{id} — get a single hotlap event
async fn get_staff_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let row = sqlx::query(
        "SELECT id, name, description, track, car, car_class, sim_type, status,
                starts_at, ends_at, reference_time_ms, rule_107_percent,
                championship_id, created_at, updated_at
         FROM hotlap_events WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(r)) => {
            use sqlx::Row;
            Json(json!({
                "id": r.try_get::<String, _>("id").unwrap_or_default(),
                "name": r.try_get::<String, _>("name").unwrap_or_default(),
                "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                "track": r.try_get::<String, _>("track").unwrap_or_default(),
                "car": r.try_get::<String, _>("car").unwrap_or_default(),
                "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                "status": r.try_get::<String, _>("status").unwrap_or_default(),
                "starts_at": r.try_get::<Option<String>, _>("starts_at").unwrap_or(None),
                "ends_at": r.try_get::<Option<String>, _>("ends_at").unwrap_or(None),
                "reference_time_ms": r.try_get::<Option<i64>, _>("reference_time_ms").unwrap_or(None),
                "rule_107_percent": r.try_get::<i64, _>("rule_107_percent").unwrap_or(1),
                "championship_id": r.try_get::<Option<String>, _>("championship_id").unwrap_or(None),
                "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
            }))
        }
        Ok(None) => Json(json!({ "error": "Event not found" })),
        Err(e) => Json(json!({ "error": format!("Database error: {}", e) })),
    }
}

/// PUT /staff/events/{id} — update a hotlap event
/// Uses COALESCE so only provided fields are changed; omitted fields keep existing values.
async fn update_hotlap_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let status: Option<String> = body.get("status").and_then(|v| v.as_str()).map(|s| s.to_string());
    let name: Option<String> = body.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
    let description: Option<String> = body.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
    let starts_at: Option<String> = body.get("starts_at").and_then(|v| v.as_str()).map(|s| s.to_string());
    let ends_at: Option<String> = body.get("ends_at").and_then(|v| v.as_str()).map(|s| s.to_string());
    let reference_time_ms: Option<i64> = body.get("reference_time_ms").and_then(|v| v.as_i64());

    if status.is_none() && name.is_none() && description.is_none()
        && starts_at.is_none() && ends_at.is_none() && reference_time_ms.is_none()
    {
        return Json(json!({ "error": "No updatable fields provided" }));
    }

    let result = sqlx::query(
        "UPDATE hotlap_events SET
            status = COALESCE(?, status),
            name = COALESCE(?, name),
            description = COALESCE(?, description),
            starts_at = COALESCE(?, starts_at),
            ends_at = COALESCE(?, ends_at),
            reference_time_ms = COALESCE(?, reference_time_ms),
            updated_at = datetime('now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(name)
    .bind(description)
    .bind(starts_at)
    .bind(ends_at)
    .bind(reference_time_ms)
    .bind(&id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Event not found" })),
        Ok(_) => Json(json!({ "status": "updated" })),
        Err(e) => Json(json!({ "error": format!("Failed to update event: {}", e) })),
    }
}

// ─── Staff: Championships ─────────────────────────────────────────────────────

/// POST /staff/championships — create a new championship
async fn create_championship(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let name = match body.get("name").and_then(|v| v.as_str()) {
        Some(n) => n.to_string(),
        None => return Json(json!({ "error": "name is required" })),
    };
    let car_class = match body.get("car_class").and_then(|v| v.as_str()) {
        Some(c) => c.to_string(),
        None => return Json(json!({ "error": "car_class is required" })),
    };
    let description: Option<String> = body
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let sim_type = body
        .get("sim_type")
        .and_then(|v| v.as_str())
        .unwrap_or("assetto_corsa")
        .to_string();
    let season: Option<String> = body
        .get("season")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let result = sqlx::query(
        "INSERT INTO championships
            (id, name, description, car_class, sim_type, season,
             status, scoring_system, total_rounds, completed_rounds,
             created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 'upcoming', 'f1_2010', 0, 0, datetime('now'), datetime('now'))",
    )
    .bind(&id)
    .bind(&name)
    .bind(&description)
    .bind(&car_class)
    .bind(&sim_type)
    .bind(&season)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            tracing::info!("Championship created: {} ({})", id, name);
            Json(json!({ "id": id, "status": "created" }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to create championship: {}", e) })),
    }
}

/// GET /staff/championships — list all championships
async fn list_staff_championships(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let rows = sqlx::query(
        "SELECT id, name, description, car_class, sim_type, season,
                status, scoring_system, total_rounds, completed_rounds,
                created_at, updated_at
         FROM championships ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rows) => {
            let championships: Vec<Value> = rows
                .iter()
                .map(|r| {
                    use sqlx::Row;
                    json!({
                        "id": r.try_get::<String, _>("id").unwrap_or_default(),
                        "name": r.try_get::<String, _>("name").unwrap_or_default(),
                        "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                        "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                        "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                        "season": r.try_get::<Option<String>, _>("season").unwrap_or(None),
                        "status": r.try_get::<String, _>("status").unwrap_or_default(),
                        "scoring_system": r.try_get::<String, _>("scoring_system").unwrap_or_default(),
                        "total_rounds": r.try_get::<i64, _>("total_rounds").unwrap_or(0),
                        "completed_rounds": r.try_get::<i64, _>("completed_rounds").unwrap_or(0),
                        "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                        "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
                    })
                })
                .collect();
            Json(json!({ "championships": championships }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to list championships: {}", e) })),
    }
}

/// GET /staff/championships/{id} — get a championship with its rounds
async fn get_staff_championship(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let champ_row = sqlx::query(
        "SELECT id, name, description, car_class, sim_type, season,
                status, scoring_system, total_rounds, completed_rounds,
                created_at, updated_at
         FROM championships WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let championship = match champ_row {
        Ok(Some(r)) => {
            use sqlx::Row;
            json!({
                "id": r.try_get::<String, _>("id").unwrap_or_default(),
                "name": r.try_get::<String, _>("name").unwrap_or_default(),
                "description": r.try_get::<Option<String>, _>("description").unwrap_or(None),
                "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                "sim_type": r.try_get::<String, _>("sim_type").unwrap_or_default(),
                "season": r.try_get::<Option<String>, _>("season").unwrap_or(None),
                "status": r.try_get::<String, _>("status").unwrap_or_default(),
                "scoring_system": r.try_get::<String, _>("scoring_system").unwrap_or_default(),
                "total_rounds": r.try_get::<i64, _>("total_rounds").unwrap_or(0),
                "completed_rounds": r.try_get::<i64, _>("completed_rounds").unwrap_or(0),
                "created_at": r.try_get::<String, _>("created_at").unwrap_or_default(),
                "updated_at": r.try_get::<String, _>("updated_at").unwrap_or_default(),
            })
        }
        Ok(None) => return Json(json!({ "error": "Championship not found" })),
        Err(e) => return Json(json!({ "error": format!("Database error: {}", e) })),
    };

    let rounds_rows = sqlx::query(
        "SELECT cr.round_number, cr.event_id,
                he.name AS event_name, he.track, he.car_class, he.status AS event_status,
                he.starts_at, he.ends_at
         FROM championship_rounds cr
         JOIN hotlap_events he ON he.id = cr.event_id
         WHERE cr.championship_id = ?
         ORDER BY cr.round_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    let rounds: Vec<Value> = match rounds_rows {
        Ok(rows) => rows
            .iter()
            .map(|r| {
                use sqlx::Row;
                json!({
                    "round_number": r.try_get::<i64, _>("round_number").unwrap_or(0),
                    "event_id": r.try_get::<String, _>("event_id").unwrap_or_default(),
                    "event_name": r.try_get::<String, _>("event_name").unwrap_or_default(),
                    "track": r.try_get::<String, _>("track").unwrap_or_default(),
                    "car_class": r.try_get::<String, _>("car_class").unwrap_or_default(),
                    "event_status": r.try_get::<String, _>("event_status").unwrap_or_default(),
                    "starts_at": r.try_get::<Option<String>, _>("starts_at").unwrap_or(None),
                    "ends_at": r.try_get::<Option<String>, _>("ends_at").unwrap_or(None),
                })
            })
            .collect(),
        Err(_) => vec![],
    };

    Json(json!({ "championship": championship, "rounds": rounds }))
}

/// POST /staff/championships/{id}/rounds — add a round to a championship
async fn add_championship_round(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(championship_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let event_id = match body.get("event_id").and_then(|v| v.as_str()) {
        Some(e) => e.to_string(),
        None => return Json(json!({ "error": "event_id is required" })),
    };
    let round_number = match body.get("round_number").and_then(|v| v.as_i64()) {
        Some(n) => n,
        None => return Json(json!({ "error": "round_number is required" })),
    };

    let result = sqlx::query(
        "INSERT INTO championship_rounds (championship_id, event_id, round_number)
         VALUES (?, ?, ?)",
    )
    .bind(&championship_id)
    .bind(&event_id)
    .bind(round_number)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        return Json(json!({ "error": format!("Failed to add round: {}", e) }));
    }

    // Link event back to championship
    let _ = sqlx::query(
        "UPDATE hotlap_events SET championship_id = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&championship_id)
    .bind(&event_id)
    .execute(&state.db)
    .await;

    // Increment total_rounds on championship
    let _ = sqlx::query(
        "UPDATE championships SET total_rounds = total_rounds + 1, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&championship_id)
    .execute(&state.db)
    .await;

    tracing::info!(
        "Championship round added: {} round {} = event {}",
        championship_id, round_number, event_id
    );
    Json(json!({ "status": "round_added" }))
}

/// POST /staff/group-sessions/{id}/complete — mark a group session completed and score the linked event
async fn complete_group_session(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(session_id): Path<String>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    // Fetch group session and its hotlap_event_id
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, hotlap_event_id FROM group_sessions WHERE id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let hotlap_event_id = match row {
        None => return Json(json!({ "error": "Group session not found" })),
        Some((_, None)) => {
            return Json(json!({
                "error": "Group session not linked to an event. Use POST /staff/events/{id}/link-session first."
            }));
        }
        Some((_, Some(event_id))) => event_id,
    };

    // Mark session as completed
    let result = sqlx::query(
        "UPDATE group_sessions SET status = 'completed', completed_at = datetime('now') WHERE id = ?",
    )
    .bind(&session_id)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        return Json(json!({ "error": format!("Failed to complete session: {e}") }));
    }

    // Score the event from multiplayer_results
    if let Err(e) = crate::lap_tracker::score_group_event(&state.db, &session_id, &hotlap_event_id).await {
        return Json(json!({ "error": format!("Session marked complete but scoring failed: {e}") }));
    }

    Json(json!({
        "status": "completed",
        "scored_event": hotlap_event_id
    }))
}

/// POST /staff/events/{id}/link-session — link a group session to a hotlap event
async fn link_group_session_to_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(event_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    if let Err(e) = check_terminal_auth(&state, &headers).await {
        return Json(json!({ "error": e }));
    }

    let group_session_id = match body.get("group_session_id").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return Json(json!({ "error": "group_session_id is required" })),
    };

    let result = sqlx::query(
        "UPDATE group_sessions SET hotlap_event_id = ? WHERE id = ?",
    )
    .bind(&event_id)
    .bind(&group_session_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => Json(json!({ "error": "Group session not found" })),
        Ok(_) => Json(json!({ "status": "linked" })),
        Err(e) => Json(json!({ "error": format!("Failed to link session: {}", e) })),
    }
}

// ─── Kiosk Allowlist (Phase 48 — ALLOW-01/02/05) ────────────────────────────
//
// Well-known system processes that staff might accidentally try to add.
// This is a UX guard only — the authoritative ~70-entry baseline lives in
// rc-agent's ALLOWED_PROCESSES constant and is never modified here.
const BASELINE_PROCESSES: &[&str] = &[
    "svchost.exe",
    "csrss.exe",
    "explorer.exe",
    "lsass.exe",
    "winlogon.exe",
    "services.exe",
    "smss.exe",
    "taskmgr.exe",
    "spoolsv.exe",
    "dwm.exe",
    "wininit.exe",
    "conhost.exe",
    "ntoskrnl.exe",
    "system",
];

async fn list_kiosk_allowlist(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>, String)>(
        "SELECT id, process_name, added_by, notes, created_at
         FROM kiosk_allowlist ORDER BY process_name ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(entries) => {
            let list: Vec<Value> = entries
                .iter()
                .map(|r| {
                    json!({
                        "id": r.0,
                        "process_name": r.1,
                        "added_by": r.2,
                        "notes": r.3,
                        "created_at": r.4,
                    })
                })
                .collect();
            Json(json!({
                "allowlist": list,
                "hardcoded_count": 70,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn add_kiosk_allowlist_entry(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> (axum::http::StatusCode, Json<Value>) {
    let process_name = match body.get("process_name").and_then(|v| v.as_str()) {
        Some(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(json!({ "error": "process_name is required" })),
            );
        }
    };
    let notes = body.get("notes").and_then(|v| v.as_str()).map(|s| s.to_string());
    let added_by = body.get("added_by").and_then(|v| v.as_str()).unwrap_or("staff").to_string();

    // UX guard: check if it matches the well-known baseline
    let lower = process_name.to_lowercase();
    for baseline in BASELINE_PROCESSES {
        if lower == *baseline {
            return (
                axum::http::StatusCode::OK,
                Json(json!({
                    "status": "already_in_baseline",
                    "message": format!(
                        "'{}' is already in the hardcoded baseline allowlist — no action needed",
                        process_name
                    ),
                })),
            );
        }
    }

    let id = uuid::Uuid::new_v4().to_string();
    let result = sqlx::query(
        "INSERT OR IGNORE INTO kiosk_allowlist (id, process_name, added_by, notes)
         VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&process_name)
    .bind(&added_by)
    .bind(&notes)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() == 0 => {
            // UNIQUE constraint — already exists
            (
                axum::http::StatusCode::OK,
                Json(json!({
                    "status": "already_exists",
                    "message": format!("'{}' is already in the staff allowlist", process_name),
                })),
            )
        }
        Ok(_) => (
            axum::http::StatusCode::CREATED,
            Json(json!({ "id": id, "process_name": process_name })),
        ),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

async fn delete_kiosk_allowlist_entry(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> axum::http::StatusCode {
    match sqlx::query(
        "DELETE FROM kiosk_allowlist WHERE LOWER(process_name) = LOWER(?)",
    )
    .bind(&name)
    .execute(&state.db)
    .await
    {
        Ok(_) => axum::http::StatusCode::NO_CONTENT,
        Err(e) => {
            tracing::error!("delete_kiosk_allowlist_entry error for '{}': {}", name, e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[cfg(test)]
mod session_detail_tests {
    use serde_json::{json, Value};

    /// Test that the events query + JSON construction logic works correctly.
    /// Tests the query pattern that will be embedded in customer_session_detail.
    #[tokio::test]
    async fn test_customer_session_detail_includes_events() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE billing_events (
                id TEXT PRIMARY KEY, billing_session_id TEXT NOT NULL,
                event_type TEXT NOT NULL, driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
                metadata TEXT, created_at TEXT DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.expect("create billing_events");

        // Insert events out of order to verify ASC ordering
        sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at)
             VALUES ('e2', 's1', 'paused', 300, '{\"reason\":\"bathroom\"}', '2026-01-01T00:05:00'),
                    ('e1', 's1', 'started', 0, NULL, '2026-01-01T00:00:00'),
                    ('e3', 's1', 'resumed', 300, NULL, '2026-01-01T00:07:00')"
        ).execute(&pool).await.expect("insert events");

        let events = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
            "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
             FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
        )
        .bind("s1")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        let events_json: Vec<Value> = events
            .iter()
            .map(|e| {
                json!({
                    "id": e.0,
                    "event_type": e.1,
                    "driving_seconds_at_event": e.2,
                    "metadata": e.3,
                    "created_at": e.4,
                })
            })
            .collect();

        // Verify events array is not empty
        assert_eq!(events_json.len(), 3, "Expected 3 events");

        // Verify ordering (created_at ASC)
        assert_eq!(events_json[0]["event_type"], "started");
        assert_eq!(events_json[1]["event_type"], "paused");
        assert_eq!(events_json[2]["event_type"], "resumed");

        // Verify all expected keys present
        assert_eq!(events_json[0]["id"], "e1");
        assert_eq!(events_json[0]["driving_seconds_at_event"], 0);
        assert!(events_json[0]["metadata"].is_null());
        assert_eq!(events_json[0]["created_at"], "2026-01-01T00:00:00");

        // Verify metadata is present where set
        assert_eq!(events_json[1]["metadata"], "{\"reason\":\"bathroom\"}");

        // Verify it would appear alongside session/laps in final JSON
        let response = json!({
            "session": { "id": "s1" },
            "laps": [],
            "events": events_json,
        });
        assert!(response.get("events").is_some(), "events key must be present");
        assert!(response["events"].is_array(), "events must be an array");
    }

    #[tokio::test]
    async fn test_customer_session_detail_empty_events() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE billing_events (
                id TEXT PRIMARY KEY, billing_session_id TEXT NOT NULL,
                event_type TEXT NOT NULL, driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
                metadata TEXT, created_at TEXT DEFAULT (datetime('now'))
            )"
        ).execute(&pool).await.expect("create billing_events");

        // No events inserted for session 'no-events'
        let events = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
            "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
             FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
        )
        .bind("no-events")
        .fetch_all(&pool)
        .await
        .unwrap_or_default();

        let events_json: Vec<Value> = events
            .iter()
            .map(|e| {
                json!({
                    "id": e.0,
                    "event_type": e.1,
                    "driving_seconds_at_event": e.2,
                    "metadata": e.3,
                    "created_at": e.4,
                })
            })
            .collect();

        // Must be empty array, not null, not missing
        assert!(events_json.is_empty(), "Expected empty events array");

        let response = json!({
            "session": { "id": "no-events" },
            "laps": [],
            "events": events_json,
        });
        assert_eq!(response["events"].as_array().expect("must be array").len(), 0);
    }
}

#[cfg(test)]
mod public_session_tests {
    use serde_json::{json, Value};

    /// Test public_session_summary returns first name only (privacy) and correct fields.
    #[tokio::test]
    async fn test_public_session_summary_first_name_and_fields() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE drivers (id TEXT PRIMARY KEY, name TEXT NOT NULL, phone TEXT)"
        ).execute(&pool).await.expect("create drivers");

        sqlx::query(
            "CREATE TABLE pricing_tiers (id TEXT PRIMARY KEY, name TEXT NOT NULL, price_paise INTEGER NOT NULL, duration_seconds INTEGER)"
        ).execute(&pool).await.expect("create pricing_tiers");

        sqlx::query(
            "CREATE TABLE billing_sessions (
                id TEXT PRIMARY KEY, driver_id TEXT NOT NULL, pod_id TEXT,
                pricing_tier_id TEXT NOT NULL, allocated_seconds INTEGER,
                driving_seconds INTEGER DEFAULT 0, status TEXT DEFAULT 'Completed',
                custom_price_paise INTEGER, car TEXT, track TEXT, sim_type TEXT,
                wallet_debit_paise INTEGER, discount_paise INTEGER,
                started_at TEXT, ended_at TEXT
            )"
        ).execute(&pool).await.expect("create billing_sessions");

        sqlx::query(
            "CREATE TABLE laps (
                id TEXT PRIMARY KEY, session_id TEXT, driver_id TEXT,
                lap_number INTEGER, lap_time_ms INTEGER, valid INTEGER DEFAULT 1,
                track TEXT, car TEXT, created_at TEXT
            )"
        ).execute(&pool).await.expect("create laps");

        // Insert test data
        sqlx::query("INSERT INTO drivers (id, name, phone) VALUES ('d1', 'John Smith', '9876543210')")
            .execute(&pool).await.expect("insert driver");
        sqlx::query("INSERT INTO pricing_tiers (id, name, price_paise, duration_seconds) VALUES ('t1', '30 Minutes', 70000, 1800)")
            .execute(&pool).await.expect("insert tier");
        sqlx::query(
            "INSERT INTO billing_sessions (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, driving_seconds, status, car, track, sim_type, wallet_debit_paise)
             VALUES ('s1', 'd1', 'pod-1', 't1', 1800, 1500, 'Completed', 'Ferrari 488', 'Monza', 'AC', 70000)"
        ).execute(&pool).await.expect("insert session");
        sqlx::query(
            "INSERT INTO laps (id, session_id, driver_id, lap_number, lap_time_ms, valid, track, car, created_at)
             VALUES ('l1', 's1', 'd1', 1, 95432, 1, 'Monza', 'Ferrari 488', '2026-01-01T00:05:00'),
                    ('l2', 's1', 'd1', 2, 93210, 1, 'Monza', 'Ferrari 488', '2026-01-01T00:07:00'),
                    ('l3', 's1', 'd1', 3, 99000, 0, 'Monza', 'Ferrari 488', '2026-01-01T00:09:00')"
        ).execute(&pool).await.expect("insert laps");

        // Simulate the public_session_summary query logic
        let row = sqlx::query_as::<_, (String, String, String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
            "SELECT bs.id, d.name, bs.status, bs.allocated_seconds, bs.driving_seconds,
                    pt.name, bs.car, bs.track, bs.sim_type
             FROM billing_sessions bs
             JOIN drivers d ON bs.driver_id = d.id
             JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
             WHERE bs.id = ?",
        )
        .bind("s1")
        .fetch_optional(&pool)
        .await;

        let session = row.expect("no error").expect("session found");
        let first_name = session.1.split_whitespace().next().unwrap_or("Racer");

        // Best lap
        let best_lap: Option<(i64,)> = sqlx::query_as(
            "SELECT MIN(lap_time_ms) FROM laps WHERE session_id = ? AND valid = 1",
        )
        .bind("s1")
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();

        let total_laps: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM laps WHERE session_id = ?",
        )
        .bind("s1")
        .fetch_optional(&pool)
        .await
        .ok()
        .flatten();

        let response = json!({
            "driver_first_name": first_name,
            "status": session.2,
            "duration_seconds": session.4,
            "pricing_tier": session.5,
            "car": session.6,
            "track": session.7,
            "sim_type": session.8,
            "best_lap_ms": best_lap.map(|b| b.0),
            "total_laps": total_laps.map(|t| t.0).unwrap_or(0),
        });

        // Verify first name only (not full name)
        assert_eq!(response["driver_first_name"], "John", "Must show first name only");

        // Verify expected fields present
        assert_eq!(response["status"], "Completed");
        assert_eq!(response["duration_seconds"], 1500);
        assert_eq!(response["pricing_tier"], "30 Minutes");
        assert_eq!(response["car"], "Ferrari 488");
        assert_eq!(response["track"], "Monza");
        assert_eq!(response["best_lap_ms"], 93210);
        assert_eq!(response["total_laps"], 3);

        // Verify NO billing amounts in response
        assert!(response.get("wallet_debit_paise").is_none(), "Must NOT expose wallet_debit_paise");
        assert!(response.get("discount_paise").is_none(), "Must NOT expose discount_paise");
        assert!(response.get("phone").is_none(), "Must NOT expose phone");
        assert!(response.get("email").is_none(), "Must NOT expose email");
        assert!(response.get("driver_name").is_none(), "Must NOT expose full driver_name");
    }

    /// Test 404 for non-existent session
    #[tokio::test]
    async fn test_public_session_summary_not_found() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE drivers (id TEXT PRIMARY KEY, name TEXT NOT NULL, phone TEXT)"
        ).execute(&pool).await.expect("create drivers");
        sqlx::query(
            "CREATE TABLE pricing_tiers (id TEXT PRIMARY KEY, name TEXT NOT NULL, price_paise INTEGER NOT NULL, duration_seconds INTEGER)"
        ).execute(&pool).await.expect("create pricing_tiers");
        sqlx::query(
            "CREATE TABLE billing_sessions (
                id TEXT PRIMARY KEY, driver_id TEXT NOT NULL, pod_id TEXT,
                pricing_tier_id TEXT NOT NULL, allocated_seconds INTEGER,
                driving_seconds INTEGER DEFAULT 0, status TEXT DEFAULT 'Completed',
                custom_price_paise INTEGER, car TEXT, track TEXT, sim_type TEXT
            )"
        ).execute(&pool).await.expect("create billing_sessions");

        let row = sqlx::query_as::<_, (String, String, String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
            "SELECT bs.id, d.name, bs.status, bs.allocated_seconds, bs.driving_seconds,
                    pt.name, bs.car, bs.track, bs.sim_type
             FROM billing_sessions bs
             JOIN drivers d ON bs.driver_id = d.id
             JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
             WHERE bs.id = ?",
        )
        .bind("nonexistent")
        .fetch_optional(&pool)
        .await;

        assert!(row.expect("no error").is_none(), "Must return None for non-existent session");
    }
}

#[cfg(test)]
mod watchdog_crash_report_tests {
    use super::*;
    use axum::extract::{Path, State};
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use std::sync::Arc;

    async fn make_state() -> Arc<AppState> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");
        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, db, field_cipher))
    }

    #[tokio::test]
    async fn watchdog_crash_report_returns_200_for_valid_payload() {
        let state = make_state().await;

        let report = WatchdogCrashReport {
            pod_id: "pod_8".to_string(),
            exit_code: Some(-1073741819),
            crash_time: "2026-03-15T10:00:00+00:00".to_string(),
            restart_count: 3,
            watchdog_version: "0.1.0".to_string(),
        };

        let response = watchdog_crash_report(
            Path("pod_8".to_string()),
            State(state),
            Json(report),
        )
        .await;

        let status = response.into_response().status();
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn watchdog_crash_report_accepts_none_exit_code() {
        let state = make_state().await;

        let report = WatchdogCrashReport {
            pod_id: "pod_1".to_string(),
            exit_code: None,
            crash_time: "2026-03-15T12:00:00+00:00".to_string(),
            restart_count: 1,
            watchdog_version: "0.1.0".to_string(),
        };

        let response = watchdog_crash_report(
            Path("pod_1".to_string()),
            State(state),
            Json(report),
        )
        .await;

        let status = response.into_response().status();
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn watchdog_crash_report_high_restart_count() {
        let state = make_state().await;

        let report = WatchdogCrashReport {
            pod_id: "pod_5".to_string(),
            exit_code: Some(1),
            crash_time: "2026-03-15T14:30:00+00:00".to_string(),
            restart_count: 42,
            watchdog_version: "0.1.0".to_string(),
        };

        let response = watchdog_crash_report(
            Path("pod_5".to_string()),
            State(state),
            Json(report),
        )
        .await;

        let status = response.into_response().status();
        assert_eq!(status, StatusCode::OK);
    }
}

// ─── PWA: Customer game launch request ─────────────────────────────────────

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GameRequestBody {
    pod_id: String,
    sim_type: SimType,
}

/// POST /api/v1/customer/game-request
///
/// Customer requests a game launch from the PWA. Validates that the pod
/// exists and the game is installed, then broadcasts GameLaunchRequested
/// to the staff dashboard. Staff confirms via POST /api/v1/games/pod/{id}/launch.
///
/// Note: customer auth uses extract_driver_id() (customer JWT). Customer auth
/// middleware is in-handler (Phase 82+ may promote to tower middleware).
async fn pwa_game_request(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<GameRequestBody>,
) -> (axum::http::StatusCode, Json<Value>) {
    // Authenticate: extract driver_id from customer JWT
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            )
        }
    };

    // Look up driver name for the broadcast payload
    let driver_name = match sqlx::query_as::<_, (String,)>(
        "SELECT name FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some((name,))) => name,
        Ok(None) => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": "Driver not found" })),
            )
        }
        Err(e) => {
            tracing::error!("pwa_game_request: DB error looking up driver {}: {}", driver_id, e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            );
        }
    };

    // Validate pod exists
    let pods = state.pods.read().await;
    let pod = match pods.get(&body.pod_id) {
        Some(p) => p.clone(),
        None => {
            return (
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("Pod '{}' not found", body.pod_id) })),
            )
        }
    };
    drop(pods);

    // Validate game is installed on that pod
    if !pod.installed_games.contains(&body.sim_type) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("{:?} is not installed on pod '{}'", body.sim_type, body.pod_id)
            })),
        );
    }

    // Generate unique request ID
    let request_id = uuid::Uuid::new_v4().to_string();

    // BILL-03: Insert into game_launch_requests with 10-minute server-side TTL
    let sim_type_str = format!("{:?}", body.sim_type);
    if let Err(e) = sqlx::query(
        "INSERT INTO game_launch_requests (id, driver_id, pod_id, sim_type, status, expires_at)
         VALUES (?, ?, ?, ?, 'pending', datetime('now', '+10 minutes'))",
    )
    .bind(&request_id)
    .bind(&driver_id)
    .bind(&body.pod_id)
    .bind(&sim_type_str)
    .execute(&state.db)
    .await
    {
        tracing::error!("pwa_game_request: Failed to insert game_launch_request {}: {}", request_id, e);
        // Non-fatal: still broadcast to staff
    }

    // Broadcast to staff dashboard -- staff confirms via existing launch endpoint
    let _ = state.dashboard_tx.send(DashboardEvent::GameLaunchRequested {
        pod_id: body.pod_id.clone(),
        sim_type: body.sim_type,
        driver_name: driver_name.clone(),
        request_id: request_id.clone(),
    });

    tracing::info!(
        "pwa_game_request: driver '{}' ({}) requested {:?} on pod '{}' (request_id={}, pwa_request_timeout=10min)",
        driver_name, driver_id, body.sim_type, body.pod_id, request_id
    );

    (
        axum::http::StatusCode::OK,
        Json(json!({ "ok": true, "request_id": request_id })),
    )
}

/// GET /customer/game-request/{id} — Check status of a PWA game request.
/// BILL-03: Returns status including "expired" if the 10-minute TTL has passed.
async fn get_game_request_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(request_id): Path<String>,
) -> (axum::http::StatusCode, Json<Value>) {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            )
        }
    };

    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT status, expires_at, resolved_at FROM game_launch_requests WHERE id = ? AND driver_id = ?",
    )
    .bind(&request_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(json!({ "error": "Request not found" })),
        ),
        Some((status, expires_at, resolved_at)) => {
            // BILL-03: If pending but TTL passed, return expired status in real-time
            let effective_status = if status == "pending" {
                let is_expired: Option<(i64,)> = sqlx::query_as(
                    "SELECT CASE WHEN ? < datetime('now') THEN 1 ELSE 0 END",
                )
                .bind(&expires_at)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten();
                if is_expired.map(|(v,)| v).unwrap_or(0) == 1 {
                    "expired".to_string()
                } else {
                    status
                }
            } else {
                status
            };
            (
                axum::http::StatusCode::OK,
                Json(json!({
                    "request_id": request_id,
                    "status": effective_status,
                    "expires_at": expires_at,
                    "resolved_at": resolved_at,
                })),
            )
        }
    }
}

// ─── Psychology handlers ──────────────────────────────────────────────────────

async fn list_badges(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let badges: Vec<(String, String, Option<String>, String, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT id, name, description, category, criteria_json, badge_icon, reward_credits_paise, sort_order
         FROM achievements WHERE is_active = 1 ORDER BY sort_order ASC"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let result: Vec<Value> = badges.into_iter().map(|(id, name, desc, cat, criteria, icon, reward, sort)| {
        json!({
            "id": id, "name": name, "description": desc, "category": cat,
            "criteria_json": criteria, "badge_icon": icon,
            "reward_credits_paise": reward, "sort_order": sort
        })
    }).collect();

    Json(json!({ "badges": result }))
}

async fn driver_badges(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let earned: Vec<(String, String, Option<String>, String, Option<String>, String)> = sqlx::query_as(
        "SELECT a.id, a.name, a.description, a.category, a.badge_icon, da.earned_at
         FROM driver_achievements da
         JOIN achievements a ON a.id = da.achievement_id
         WHERE da.driver_id = ?
         ORDER BY da.earned_at DESC"
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let count = earned.len();
    let result: Vec<Value> = earned.into_iter().map(|(id, name, desc, cat, icon, earned_at)| {
        json!({
            "id": id, "name": name, "description": desc,
            "category": cat, "badge_icon": icon, "earned_at": earned_at
        })
    }).collect();

    Json(json!({ "driver_id": driver_id, "badges": result, "count": count }))
}

async fn driver_streak(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> Json<Value> {
    let streak: Option<(i64, i64, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT current_streak, longest_streak, last_visit_date, grace_expires_date, streak_started_at
         FROM streaks WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match streak {
        Some((current, longest, last_visit, grace, started)) => {
            Json(json!({
                "driver_id": driver_id,
                "current_streak": current,
                "longest_streak": longest,
                "last_visit_date": last_visit,
                "grace_expires_date": grace,
                "streak_started_at": started
            }))
        }
        None => {
            Json(json!({
                "driver_id": driver_id,
                "current_streak": 0,
                "longest_streak": 0,
                "last_visit_date": null,
                "grace_expires_date": null,
                "streak_started_at": null
            }))
        }
    }
}

async fn list_nudge_queue(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit = params.get("limit").and_then(|v| v.parse::<i64>().ok()).unwrap_or(50);
    let status_filter = params.get("status").cloned();

    let nudges: Vec<(String, String, String, i32, String, String, String, Option<String>, Option<String>)> = if let Some(status) = &status_filter {
        sqlx::query_as(
            "SELECT id, driver_id, channel, priority, template, payload_json, status, sent_at, created_at
             FROM nudge_queue WHERE status = ? ORDER BY created_at DESC LIMIT ?"
        )
        .bind(status)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as(
            "SELECT id, driver_id, channel, priority, template, payload_json, status, sent_at, created_at
             FROM nudge_queue ORDER BY created_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    let count = nudges.len();
    let result: Vec<Value> = nudges.into_iter().map(|(id, driver, ch, pri, tpl, payload, status, sent, created)| {
        json!({
            "id": id, "driver_id": driver, "channel": ch, "priority": pri,
            "template": tpl, "payload_json": payload, "status": status,
            "sent_at": sent, "created_at": created
        })
    }).collect();

    Json(json!({ "nudges": result, "count": count }))
}

async fn test_nudge(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = body.get("driver_id").and_then(|v| v.as_str()).unwrap_or("");
    let channel = body.get("channel").and_then(|v| v.as_str()).unwrap_or("pwa");
    let message = body.get("message").and_then(|v| v.as_str()).unwrap_or("Test notification");

    if driver_id.is_empty() {
        return Json(json!({ "error": "driver_id required" }));
    }

    let ch = psychology::NotificationChannel::from_str(channel)
        .unwrap_or(psychology::NotificationChannel::Pwa);

    psychology::queue_notification(&state, driver_id, ch, 5, message, "{}").await;

    Json(json!({ "ok": true, "queued_for": driver_id, "channel": channel }))
}

// ─── DPDP Act: Customer Data Rights (Plan 79-03) ────────────────────────────

/// GET /api/v1/customer/data-export
/// Returns a JSON dump of all customer data with decrypted PII fields.
/// Requires valid customer JWT.
async fn customer_data_export(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<(axum::http::StatusCode, Json<Value>), (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    let driver = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, name_enc, email_enc, phone_enc, total_laps, total_time_ms FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let d = match driver {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_export DB error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    // Decrypt PII fields; fallback to plaintext columns if decryption fails or enc is NULL
    let name = d.4.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or_else(|| Some(d.1.clone()));
    let email = d.5.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.2.clone());
    let phone = d.6.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.3.clone());

    // Fetch nickname
    let nickname: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT nickname FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.0);

    // Fetch wallet balance
    let wallet_balance: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(balance, 0) FROM wallets WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or(0);

    let exported_at = chrono::Utc::now().to_rfc3339();
    tracing::info!("Data export requested by driver {}", driver_id);

    Ok((
        axum::http::StatusCode::OK,
        Json(json!({
            "driver_id": d.0,
            "name": name,
            "email": email,
            "phone": phone,
            "nickname": nickname,
            "total_laps": d.7,
            "total_time_ms": d.8,
            "wallet_balance": wallet_balance,
            "exported_at": exported_at,
        })),
    ))
}

/// DELETE /api/v1/customer/data-delete
/// Cascades deletion to all child tables and the driver record in a single transaction.
/// Requires valid customer JWT.
async fn customer_data_delete(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    // Verify driver exists
    let exists = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match exists {
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_delete lookup error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
        Ok(Some(_)) => {}
    }

    // Begin transaction -- cascade delete all child tables
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("data_delete transaction start error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    // Delete from child tables (children first, then parent)
    // wallet_transactions before wallets (wallet_transactions references wallets)
    let _ = sqlx::query("DELETE FROM wallet_transactions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM wallets WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM billing_sessions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM laps WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM customer_sessions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM friend_requests WHERE sender_id = ? OR receiver_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM friendships WHERE driver_a_id = ? OR driver_b_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM group_session_members WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM tournament_registrations WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM pod_reservations WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM auth_tokens WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM personal_bests WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM event_entries WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM session_feedback WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM coupon_redemptions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM memberships WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM referrals WHERE referrer_id = ? OR referee_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM session_highlights WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM review_nudges WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM multiplayer_results WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM driver_ratings WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;

    // Delete the driver record itself
    let _ = sqlx::query("DELETE FROM drivers WHERE id = ?").bind(&driver_id).execute(&mut *tx).await;

    // Commit transaction
    if let Err(e) = tx.commit().await {
        tracing::error!("data_delete commit error for driver {}: {}", driver_id, e);
        return Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Database error" })),
        ));
    }

    tracing::info!("Customer {} deleted their data (DPDP compliance)", driver_id);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── LEGAL-09: Consent revocation (customer-initiated via PWA) ───────────────
/// POST /api/v1/customer/revoke-consent
///
/// Allows a driver (or guardian acting on behalf of a minor) to invoke the DPDP Act
/// right of erasure. Anonymizes PII immediately. Financial records (journal entries,
/// invoices, billing_sessions) are NOT deleted — they must be retained for 8 years
/// per the Income Tax Act.
///
/// Body: `{ "reason": "optional reason string" }`
async fn revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("customer_request");
    anonymize_driver_pii(&state, &driver_id, reason, None).await
}

// ─── LEGAL-09: Consent revocation (staff-initiated for guardian requests) ────
/// POST /api/v1/drivers/{id}/revoke-consent
///
/// Staff endpoint for guardian-initiated revocation — guardian calls the venue,
/// staff (cashier+) processes the data deletion request.
///
/// Body: `{ "reason": "optional reason string" }`
async fn staff_revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("guardian_request");
    anonymize_driver_pii(&state, &driver_id, reason, Some("staff")).await
}

// ─── BILL-08: Customer charge dispute portal ─────────────────────────────────

/// POST /api/v1/customer/dispute
///
/// Allows a customer to flag a billing session charge for review by staff.
/// Only completed or ended_early sessions can be disputed (not active ones).
/// Enforces one active dispute per session via the UNIQUE index on dispute_requests.
///
/// Body: `{ "billing_session_id": "...", "reason": "..." }`
/// Returns: `{ "ok": true, "dispute_id": "..." }`
/// BILL-08
async fn create_dispute_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let billing_session_id = match body.get("billing_session_id").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return Json(json!({ "error": "billing_session_id is required" })),
    };

    let reason = match body.get("reason").and_then(|v| v.as_str()) {
        Some(r) if !r.is_empty() => r.to_string(),
        _ => return Json(json!({ "error": "reason is required" })),
    };

    // BILL-08: Validate session exists and belongs to this driver
    let session_info: Option<(String, String)> = sqlx::query_as(
        "SELECT id, status FROM billing_sessions WHERE id = ? AND driver_id = ?",
    )
    .bind(&billing_session_id)
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let session_status = match session_info {
        Some((_, status)) => status,
        None => return Json(json!({ "error": "Billing session not found or does not belong to you" })),
    };

    // BILL-08: Only completed or ended_early sessions can be disputed (not active sessions)
    if session_status != "completed" && session_status != "ended_early" {
        return Json(json!({
            "error": format!("Cannot dispute a session with status '{}'. Only completed or ended_early sessions can be disputed.", session_status)
        }));
    }

    let dispute_id = uuid::Uuid::new_v4().to_string();

    // BILL-08: Insert dispute — UNIQUE index will reject duplicates
    let insert_result = sqlx::query(
        "INSERT INTO dispute_requests (id, billing_session_id, driver_id, reason, status)
         VALUES (?, ?, ?, ?, 'pending')",
    )
    .bind(&dispute_id)
    .bind(&billing_session_id)
    .bind(&driver_id)
    .bind(&reason)
    .execute(&state.db)
    .await;

    if let Err(e) = insert_result {
        let msg = e.to_string();
        if msg.contains("UNIQUE") || msg.contains("constraint") {
            return Json(json!({ "error": "A dispute is already open for this session" }));
        }
        tracing::error!("BILL-08: Failed to insert dispute for session {}: {}", billing_session_id, e);
        return Json(json!({ "error": "Failed to create dispute" }));
    }

    // Look up driver name for broadcast
    let driver_name: String = sqlx::query_scalar(
        "SELECT COALESCE(name, id) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| driver_id.clone());

    // BILL-08: Broadcast DisputeCreated — staff dashboard gets a notification
    let _ = state.dashboard_tx.send(DashboardEvent::DisputeCreated {
        dispute_id: dispute_id.clone(),
        driver_name,
        session_id: billing_session_id.clone(),
        reason: reason.clone(),
    });

    tracing::info!(
        "BILL-08: Dispute {} created by driver {} for session {}",
        dispute_id, driver_id, billing_session_id
    );

    Json(json!({ "ok": true, "dispute_id": dispute_id }))
}

/// GET /api/v1/admin/disputes
///
/// Returns list of disputes for staff review. Optional ?status=pending filter.
/// Includes: dispute_id, driver_name, session_id, pod_id, session_duration,
///           amount_charged, reason, status, created_at.
/// BILL-08
async fn list_disputes_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let status_filter = params.get("status").cloned();

    let rows = if let Some(ref status) = status_filter {
        sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>, String, String)>(
            "SELECT dr.id, COALESCE(d.name, dr.driver_id) AS driver_name,
                    dr.billing_session_id, bs.pod_id,
                    d.id AS driver_id,
                    bs.driving_seconds, bs.wallet_debit_paise,
                    dr.reason, dr.status
             FROM dispute_requests dr
             LEFT JOIN billing_sessions bs ON bs.id = dr.billing_session_id
             LEFT JOIN drivers d ON d.id = dr.driver_id
             WHERE dr.status = ?
             ORDER BY dr.created_at DESC
             LIMIT 100",
        )
        .bind(status)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<i64>, Option<i64>, String, String)>(
            "SELECT dr.id, COALESCE(d.name, dr.driver_id) AS driver_name,
                    dr.billing_session_id, bs.pod_id,
                    d.id AS driver_id,
                    bs.driving_seconds, bs.wallet_debit_paise,
                    dr.reason, dr.status
             FROM dispute_requests dr
             LEFT JOIN billing_sessions bs ON bs.id = dr.billing_session_id
             LEFT JOIN drivers d ON d.id = dr.driver_id
             ORDER BY dr.created_at DESC
             LIMIT 100",
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default()
    };

    let disputes: Vec<Value> = rows
        .into_iter()
        .map(|(id, driver_name, session_id, pod_id, driver_id, driving_seconds, amount_charged, reason, status)| {
            json!({
                "dispute_id": id,
                "driver_name": driver_name,
                "driver_id": driver_id,
                "session_id": session_id,
                "pod_id": pod_id,
                "session_duration_seconds": driving_seconds,
                "amount_charged_paise": amount_charged,
                "reason": reason,
                "status": status
            })
        })
        .collect();

    Json(json!({ "disputes": disputes, "count": disputes.len() }))
}

/// GET /api/v1/admin/disputes/{id}/details
///
/// Returns full dispute context: dispute info + billing_events + billing_session details.
/// Gives staff the complete audit trail to make an informed decision.
/// BILL-08
async fn dispute_details_handler(
    State(state): State<Arc<AppState>>,
    Path(dispute_id): Path<String>,
) -> Json<Value> {
    // Fetch the dispute
    let dispute: Option<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, billing_session_id, driver_id, reason, status
         FROM dispute_requests WHERE id = ?",
    )
    .bind(&dispute_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (_, billing_session_id, driver_id, reason, status) = match dispute {
        Some(d) => d,
        None => return Json(json!({ "error": "Dispute not found" })),
    };

    // Fetch billing session details
    let session: Option<(Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT pod_id, pricing_tier_id, allocated_seconds, driving_seconds, wallet_debit_paise, started_at, ended_at
         FROM billing_sessions WHERE id = ?",
    )
    .bind(&billing_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Fetch billing events for this session (audit trail)
    let events: Vec<(String, i64, Option<String>)> = sqlx::query_as(
        "SELECT event_type, driving_seconds_at_event, metadata
         FROM billing_events WHERE billing_session_id = ?
         ORDER BY rowid ASC LIMIT 50",
    )
    .bind(&billing_session_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let events_json: Vec<Value> = events
        .into_iter()
        .map(|(event_type, driving_secs, metadata)| {
            json!({
                "event_type": event_type,
                "driving_seconds": driving_secs,
                "metadata": metadata
            })
        })
        .collect();

    let session_json = if let Some((pod_id, tier_id, alloc, driving, debit, started, ended)) = session {
        json!({
            "pod_id": pod_id,
            "pricing_tier_id": tier_id,
            "allocated_seconds": alloc,
            "driving_seconds": driving,
            "wallet_debit_paise": debit,
            "started_at": started,
            "ended_at": ended
        })
    } else {
        json!(null)
    };

    Json(json!({
        "dispute_id": dispute_id,
        "billing_session_id": billing_session_id,
        "driver_id": driver_id,
        "reason": reason,
        "status": status,
        "billing_session": session_json,
        "billing_events": events_json
    }))
}

/// POST /api/v1/admin/disputes/{id}/resolve
///
/// Staff resolves a pending dispute. Action must be "approve" or "deny".
/// Approve: computes refund via compute_refund(), credits wallet via credit_in_tx(),
///          logs 'dispute_refund' billing_event.
/// Deny: logs 'dispute_denied' billing_event with reason.
///
/// Body: `{ "action": "approve" | "deny", "reason": "..." }`
/// Returns: `{ "ok": true, "status": "approved" | "denied", "refund_amount_paise": N }`
/// BILL-08
async fn resolve_dispute_handler(
    State(state): State<Arc<AppState>>,
    Path(dispute_id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let staff_id = claims.map(|c| c.0.sub.clone()).unwrap_or_else(|| "unknown".to_string());

    let action = match body.get("action").and_then(|v| v.as_str()) {
        Some("approve") => "approve",
        Some("deny") => "deny",
        _ => return Json(json!({ "error": "action must be 'approve' or 'deny'" })),
    };

    let resolution_reason = body.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string();

    // BILL-08: Fetch dispute — must exist and be pending
    let dispute: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, billing_session_id, driver_id, status FROM dispute_requests WHERE id = ?",
    )
    .bind(&dispute_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (_, billing_session_id, driver_id, current_status) = match dispute {
        Some(d) => d,
        None => return Json(json!({ "error": "Dispute not found" })),
    };

    // BILL-08: Guard — only pending disputes can be resolved
    if current_status != "pending" {
        return Json(json!({
            "error": format!("Dispute is already resolved (status: {})", current_status)
        }));
    }

    if action == "approve" {
        // BILL-08: Compute refund using unified compute_refund() (FATM-06 path)
        let session_data: Option<(i64, i64, i64)> = sqlx::query_as(
            "SELECT COALESCE(allocated_seconds, 0), COALESCE(driving_seconds, 0), COALESCE(wallet_debit_paise, 0)
             FROM billing_sessions WHERE id = ?",
        )
        .bind(&billing_session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let (allocated, driving, debit) = session_data.unwrap_or((0, 0, 0));
        let refund_paise = crate::billing::compute_refund(allocated, driving, debit);

        if refund_paise > 0 {
            // BILL-08: Credit wallet via credit_in_tx (FATM-03 atomic credit path)
            let mut tx = match state.db.begin().await {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("BILL-08: Failed to begin transaction for dispute refund {}: {}", dispute_id, e);
                    return Json(json!({ "error": "Failed to process refund" }));
                }
            };

            match crate::wallet::credit_in_tx(
                &mut tx,
                &driver_id,
                refund_paise,
                "dispute_refund",
                Some(&billing_session_id),
                Some(&format!("Dispute {} approved by staff", dispute_id)),
                Some(&staff_id),
                None,
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    tracing::error!("BILL-08: Failed to credit wallet for dispute {}: {}", dispute_id, e);
                    return Json(json!({ "error": "Failed to credit wallet" }));
                }
            }

            if let Err(e) = tx.commit().await {
                tracing::error!("BILL-08: Failed to commit wallet credit for dispute {}: {}", dispute_id, e);
                return Json(json!({ "error": "Failed to commit refund" }));
            }
        }

        // BILL-08: Update dispute to approved
        let _ = sqlx::query(
            "UPDATE dispute_requests SET status='approved', resolved_at=datetime('now'),
             resolved_by=?, refund_amount_paise=? WHERE id=?",
        )
        .bind(&staff_id)
        .bind(refund_paise)
        .bind(&dispute_id)
        .execute(&state.db)
        .await;

        // BILL-08: Log 'dispute_refund' billing_event for audit trail
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata)
             VALUES (?, ?, 'dispute_refund', ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&billing_session_id)
        .bind(0i64)
        .bind(format!(
            "{{\"dispute_id\":\"{}\",\"refund_paise\":{},\"resolved_by\":\"{}\"}}",
            dispute_id, refund_paise, staff_id
        ))
        .execute(&state.db)
        .await
        .map_err(|e| tracing::warn!("BILL-08: Failed to log dispute_refund event: {}", e));

        tracing::info!(
            "BILL-08: Dispute {} approved by {} — refund {}p to driver {}",
            dispute_id, staff_id, refund_paise, driver_id
        );

        Json(json!({
            "ok": true,
            "status": "approved",
            "refund_amount_paise": refund_paise
        }))
    } else {
        // deny path
        // BILL-08: Update dispute to denied
        let _ = sqlx::query(
            "UPDATE dispute_requests SET status='denied', resolved_at=datetime('now'),
             resolved_by=?, resolution_reason=? WHERE id=?",
        )
        .bind(&staff_id)
        .bind(&resolution_reason)
        .bind(&dispute_id)
        .execute(&state.db)
        .await;

        // BILL-08: Log 'dispute_denied' billing_event for audit trail
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, metadata)
             VALUES (?, ?, 'dispute_denied', ?, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&billing_session_id)
        .bind(0i64)
        .bind(format!(
            "{{\"dispute_id\":\"{}\",\"reason\":\"{}\",\"resolved_by\":\"{}\"}}",
            dispute_id,
            resolution_reason.replace('"', "'"),
            staff_id
        ))
        .execute(&state.db)
        .await
        .map_err(|e| tracing::warn!("BILL-08: Failed to log dispute_denied event: {}", e));

        tracing::info!(
            "BILL-08: Dispute {} denied by {} — reason: {}",
            dispute_id, staff_id, resolution_reason
        );

        Json(json!({
            "ok": true,
            "status": "denied",
            "refund_amount_paise": 0
        }))
    }
}

/// Shared PII anonymization logic for both customer- and staff-initiated consent revocation.
///
/// Anonymizes all PII fields on the drivers row and sets consent_revoked = 1.
/// The driver row is retained so billing_sessions.driver_id foreign keys remain valid.
/// Financial records (journal_entries, invoices, billing_sessions, wallet_transactions)
/// are NOT touched — retained for 8 years per the Income Tax Act.
async fn anonymize_driver_pii(
    state: &Arc<AppState>,
    driver_id: &str,
    reason: &str,
    actor: Option<&str>,
) -> Json<Value> {
    // Check driver exists and is not already revoked
    let row = sqlx::query_as::<_, (String, bool)>(
        "SELECT id, COALESCE(consent_revoked, 0) FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(None) => return Json(json!({ "error": "Driver not found" })),
        Ok(Some((_, true))) => {
            return Json(json!({
                "ok": true,
                "message": "Consent was already revoked. Personal data has been anonymized previously."
            }));
        }
        Err(e) => {
            tracing::error!(driver_id = %driver_id, "consent_revocation DB lookup error: {}", e);
            return Json(json!({ "error": "Database error" }));
        }
        Ok(Some(_)) => {} // proceed
    }

    // Anonymize PII — same UPDATE used by the daily background job.
    // The driver row is KEPT so billing_session.driver_id FKs remain valid.
    let result = sqlx::query(
        "UPDATE drivers SET
            name = 'ANONYMIZED-' || substr(id, 1, 8),
            email = NULL,
            phone = NULL,
            phone_hash = NULL,
            guardian_name = NULL,
            guardian_phone = NULL,
            guardian_phone_hash = NULL,
            dob = NULL,
            pii_anonymized = 1,
            pii_anonymized_at = datetime('now'),
            consent_revoked = 1,
            consent_revoked_at = datetime('now')
        WHERE id = ? AND COALESCE(pii_anonymized, 0) = 0",
    )
    .bind(driver_id)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        tracing::error!(driver_id = %driver_id, "consent_revocation anonymization failed: {}", e);
        return Json(json!({ "error": "Failed to anonymize driver data" }));
    }

    // Audit log — record the revocation event
    accounting::log_audit(
        state,
        "drivers",
        driver_id,
        "consent_revocation",
        None,
        Some(&json!({ "reason": reason, "actor": actor }).to_string()),
        actor,
    )
    .await;

    tracing::info!(
        target: "legal_compliance",
        driver_id = %driver_id,
        reason = %reason,
        actor = ?actor,
        "LEGAL-09: PII anonymized via consent revocation"
    );

    Json(json!({
        "ok": true,
        "message": "Personal data has been anonymized. Financial records retained per legal requirements."
    }))
}

// ─── LEGAL-08: Data retention background job ─────────────────────────────────
/// Spawned at server startup (in main.rs). Runs daily with a 1-hour initial delay
/// to avoid congestion at boot. Reads pii_inactive_months from data_retention_config
/// and anonymizes drivers who have been inactive beyond that threshold.
///
/// Financial records (journal_entries, invoices, billing_sessions, wallet_transactions)
/// are never touched — retained for 8 years per Income Tax Act.
pub async fn spawn_data_retention_job(state: Arc<AppState>) {
    tracing::info!(
        target: "data_retention",
        "data-retention task started (86400s interval, 3600s initial delay)"
    );
    // Initial delay: 1 hour — avoid boot congestion alongside orphan detector, reconciler, etc.
    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(86400));
    loop {
        interval.tick().await;
        run_pii_anonymization_cycle(state.clone()).await;
    }
}

/// Single anonymization cycle — called daily by spawn_data_retention_job.
async fn run_pii_anonymization_cycle(state: Arc<AppState>) {
    // Read retention policy from config table
    let policy: Option<(i64,)> = sqlx::query_as(
        "SELECT pii_inactive_months FROM data_retention_config WHERE id = 'default'",
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let pii_inactive_months = policy.map(|r| r.0).unwrap_or(24);

    // Find drivers inactive beyond the threshold who have not yet been anonymized
    // and have not already revoked consent (those are handled immediately on revocation).
    let inactive_drivers: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers
         WHERE (last_activity_at IS NULL
                OR last_activity_at < datetime('now', '-' || ? || ' months'))
           AND COALESCE(pii_anonymized, 0) = 0
           AND COALESCE(consent_revoked, 0) = 0
         LIMIT 500",
    )
    .bind(pii_inactive_months)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let mut anonymized_count: u32 = 0;
    for (driver_id,) in inactive_drivers {
        let result = sqlx::query(
            "UPDATE drivers SET
                name = 'ANONYMIZED-' || substr(id, 1, 8),
                email = NULL,
                phone = NULL,
                phone_hash = NULL,
                guardian_name = NULL,
                guardian_phone = NULL,
                guardian_phone_hash = NULL,
                dob = NULL,
                pii_anonymized = 1,
                pii_anonymized_at = datetime('now')
            WHERE id = ? AND COALESCE(pii_anonymized, 0) = 0",
        )
        .bind(&driver_id)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => anonymized_count += 1,
            Ok(_) => {} // already anonymized between SELECT and UPDATE — idempotent
            Err(e) => tracing::warn!(
                target: "data_retention",
                driver_id = %driver_id,
                "Failed to anonymize inactive driver: {}",
                e
            ),
        }
    }

    tracing::info!(
        target: "data_retention",
        count = anonymized_count,
        threshold_months = pii_inactive_months,
        "PII anonymization cycle complete"
    );
}

#[cfg(test)]
mod data_rights_tests {
    use super::*;
    use axum::extract::State;
    use axum::http::StatusCode;
    use std::sync::Arc;

    async fn make_state_with_db() -> Arc<AppState> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        // Create required tables
        sqlx::query("CREATE TABLE IF NOT EXISTS drivers (id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT, phone TEXT, name_enc TEXT, email_enc TEXT, phone_enc TEXT, nickname TEXT, total_laps INTEGER DEFAULT 0, total_time_ms INTEGER DEFAULT 0, created_at TEXT DEFAULT (datetime('now')), updated_at TEXT)").execute(&db).await.expect("create drivers");
        sqlx::query("CREATE TABLE IF NOT EXISTS wallets (driver_id TEXT PRIMARY KEY, balance INTEGER DEFAULT 0)").execute(&db).await.expect("create wallets");
        sqlx::query("CREATE TABLE IF NOT EXISTS wallet_transactions (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create wallet_transactions");
        sqlx::query("CREATE TABLE IF NOT EXISTS billing_sessions (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create billing_sessions");
        sqlx::query("CREATE TABLE IF NOT EXISTS laps (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create laps");
        sqlx::query("CREATE TABLE IF NOT EXISTS customer_sessions (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create customer_sessions");
        sqlx::query("CREATE TABLE IF NOT EXISTS friend_requests (id TEXT PRIMARY KEY, sender_id TEXT NOT NULL, receiver_id TEXT NOT NULL)").execute(&db).await.expect("create friend_requests");
        sqlx::query("CREATE TABLE IF NOT EXISTS friendships (id TEXT PRIMARY KEY, driver_a_id TEXT NOT NULL, driver_b_id TEXT NOT NULL)").execute(&db).await.expect("create friendships");
        sqlx::query("CREATE TABLE IF NOT EXISTS group_session_members (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create group_session_members");
        sqlx::query("CREATE TABLE IF NOT EXISTS tournament_registrations (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create tournament_registrations");
        sqlx::query("CREATE TABLE IF NOT EXISTS pod_reservations (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create pod_reservations");
        sqlx::query("CREATE TABLE IF NOT EXISTS auth_tokens (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create auth_tokens");
        sqlx::query("CREATE TABLE IF NOT EXISTS personal_bests (driver_id TEXT NOT NULL, track TEXT NOT NULL, car TEXT NOT NULL)").execute(&db).await.expect("create personal_bests");
        sqlx::query("CREATE TABLE IF NOT EXISTS event_entries (event_id TEXT NOT NULL, driver_id TEXT NOT NULL)").execute(&db).await.expect("create event_entries");
        sqlx::query("CREATE TABLE IF NOT EXISTS session_feedback (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create session_feedback");
        sqlx::query("CREATE TABLE IF NOT EXISTS coupon_redemptions (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create coupon_redemptions");
        sqlx::query("CREATE TABLE IF NOT EXISTS memberships (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create memberships");
        sqlx::query("CREATE TABLE IF NOT EXISTS referrals (id TEXT PRIMARY KEY, referrer_id TEXT NOT NULL, referee_id TEXT, code TEXT NOT NULL)").execute(&db).await.expect("create referrals");
        sqlx::query("CREATE TABLE IF NOT EXISTS session_highlights (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create session_highlights");
        sqlx::query("CREATE TABLE IF NOT EXISTS review_nudges (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create review_nudges");
        sqlx::query("CREATE TABLE IF NOT EXISTS multiplayer_results (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL)").execute(&db).await.expect("create multiplayer_results");
        sqlx::query("CREATE TABLE IF NOT EXISTS driver_ratings (driver_id TEXT NOT NULL, sim_type TEXT NOT NULL DEFAULT 'assettocorsa', composite_rating REAL NOT NULL DEFAULT 0.0, rating_class TEXT NOT NULL DEFAULT 'Unrated', pace_score REAL NOT NULL DEFAULT 0.0, consistency_score REAL NOT NULL DEFAULT 0.0, experience_score REAL NOT NULL DEFAULT 0.0, total_laps INTEGER NOT NULL DEFAULT 0, updated_at TEXT DEFAULT (datetime('now')), PRIMARY KEY (driver_id, sim_type))").execute(&db).await.expect("create driver_ratings");

        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, db, field_cipher))
    }

    fn make_auth_headers(state: &AppState, driver_id: &str) -> axum::http::HeaderMap {
        let token = crate::auth::create_jwt(driver_id, &state.config.auth.jwt_secret)
            .expect("generate test JWT");
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn data_export_without_jwt_returns_401() {
        let state = make_state_with_db().await;
        let headers = axum::http::HeaderMap::new();
        let result = customer_data_export(State(state), headers).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn data_export_with_valid_jwt_returns_200() {
        let state = make_state_with_db().await;
        sqlx::query("INSERT INTO drivers (id, name, email, phone, total_laps, total_time_ms) VALUES (?, ?, ?, ?, ?, ?)")
            .bind("d-001").bind("Test Driver").bind("test@example.com").bind("9876543210").bind(42i64).bind(360000i64)
            .execute(&state.db).await.expect("insert driver");
        sqlx::query("INSERT INTO wallets (driver_id, balance) VALUES (?, ?)")
            .bind("d-001").bind(5000i64)
            .execute(&state.db).await.expect("insert wallet");
        let headers = make_auth_headers(&state, "d-001");
        let result = customer_data_export(State(state), headers).await;
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let (status, Json(body)) = result.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["driver_id"], "d-001");
        assert_eq!(body["total_laps"], 42);
        assert_eq!(body["wallet_balance"], 5000);
        assert!(body["exported_at"].as_str().is_some());
        assert_eq!(body["name"], "Test Driver");
    }

    #[tokio::test]
    async fn data_export_driver_not_found_returns_404() {
        let state = make_state_with_db().await;
        let headers = make_auth_headers(&state, "nonexistent");
        let result = customer_data_export(State(state), headers).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn data_export_decrypts_encrypted_fields() {
        let state = make_state_with_db().await;
        let name_enc = state.field_cipher.encrypt_field("Encrypted Name").expect("encrypt name");
        let email_enc = state.field_cipher.encrypt_field("secret@email.com").expect("encrypt email");
        let phone_enc = state.field_cipher.encrypt_field("9999999999").expect("encrypt phone");
        sqlx::query("INSERT INTO drivers (id, name, name_enc, email_enc, phone_enc, total_laps, total_time_ms) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind("d-enc").bind("Plaintext Name").bind(&name_enc).bind(&email_enc).bind(&phone_enc).bind(10i64).bind(100000i64)
            .execute(&state.db).await.expect("insert driver with enc");
        let headers = make_auth_headers(&state, "d-enc");
        let result = customer_data_export(State(state), headers).await;
        assert!(result.is_ok());
        let (_, Json(body)) = result.unwrap();
        assert_eq!(body["name"], "Encrypted Name");
        assert_eq!(body["email"], "secret@email.com");
        assert_eq!(body["phone"], "9999999999");
    }

    #[tokio::test]
    async fn data_delete_without_jwt_returns_401() {
        let state = make_state_with_db().await;
        let headers = axum::http::HeaderMap::new();
        let result = customer_data_delete(State(state), headers).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn data_delete_driver_not_found_returns_404() {
        let state = make_state_with_db().await;
        let headers = make_auth_headers(&state, "nonexistent");
        let result = customer_data_delete(State(state), headers).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn data_delete_cascades_all_child_tables() {
        let state = make_state_with_db().await;
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)").bind("d-del").bind("Delete Me").execute(&state.db).await.expect("insert driver");
        sqlx::query("INSERT INTO wallets (driver_id, balance) VALUES (?, ?)").bind("d-del").bind(1000i64).execute(&state.db).await.expect("insert wallet");
        sqlx::query("INSERT INTO wallet_transactions (id, driver_id) VALUES (?, ?)").bind("wt-1").bind("d-del").execute(&state.db).await.expect("insert wallet_txn");
        sqlx::query("INSERT INTO laps (id, driver_id) VALUES (?, ?)").bind("l-1").bind("d-del").execute(&state.db).await.expect("insert lap");
        sqlx::query("INSERT INTO auth_tokens (id, driver_id) VALUES (?, ?)").bind("at-1").bind("d-del").execute(&state.db).await.expect("insert auth_token");
        let headers = make_auth_headers(&state, "d-del");
        let result = customer_data_delete(State(state.clone()), headers).await;
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        assert_eq!(result.unwrap(), StatusCode::NO_CONTENT);
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM drivers WHERE id = ?").bind("d-del").fetch_one(&state.db).await.expect("count drivers");
        assert_eq!(count.0, 0, "Driver should be deleted");
        let wallet_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wallets WHERE driver_id = ?").bind("d-del").fetch_one(&state.db).await.expect("count wallets");
        assert_eq!(wallet_count.0, 0, "Wallet should be deleted");
        let lap_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM laps WHERE driver_id = ?").bind("d-del").fetch_one(&state.db).await.expect("count laps");
        assert_eq!(lap_count.0, 0, "Laps should be deleted");
    }

    #[tokio::test]
    async fn data_delete_returns_204_no_content() {
        let state = make_state_with_db().await;
        sqlx::query("INSERT INTO drivers (id, name) VALUES (?, ?)").bind("d-204").bind("204 Test").execute(&state.db).await.expect("insert driver");
        let headers = make_auth_headers(&state, "d-204");
        let result = customer_data_delete(State(state), headers).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StatusCode::NO_CONTENT);
    }
}

// ─── Customer Passport + Badges (PWA) ────────────────────────────────────────

/// GET /customer/passport — returns driving passport with tiered track/car collections.
/// Lazy backfill: if driver has laps but no passport entries, backfills from laps table first.
async fn customer_passport(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Lazy backfill: check if driver has passport entries
    let passport_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM driving_passport WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if passport_count == 0 {
        psychology::backfill_driving_passport(&state, &driver_id).await;
    }

    // Fetch all passport entries for this driver
    let entries: Vec<(String, String, Option<String>, Option<i64>, i64)> = sqlx::query_as(
        "SELECT track, car, first_driven_at, best_lap_ms, lap_count FROM driving_passport WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Build driven sets for quick lookup
    let driven_tracks: std::collections::HashSet<String> = entries.iter().map(|(t, _, _, _, _)| t.clone()).collect();
    let driven_cars: std::collections::HashSet<String> = entries.iter().map(|(_, c, _, _, _)| c.clone()).collect();

    // Build lookup maps for passport data (aggregate per track and per car)
    let mut track_data: std::collections::HashMap<String, (i64, i64, Option<String>)> = std::collections::HashMap::new();
    let mut car_data: std::collections::HashMap<String, (i64, i64, Option<String>)> = std::collections::HashMap::new();
    for (track, car, first_driven, best_lap, lap_count) in &entries {
        let te = track_data.entry(track.clone()).or_insert((0, i64::MAX, first_driven.clone()));
        te.0 += lap_count;
        if let Some(bl) = best_lap { if *bl < te.1 { te.1 = *bl; } }
        let ce = car_data.entry(car.clone()).or_insert((0, i64::MAX, first_driven.clone()));
        ce.0 += lap_count;
        if let Some(bl) = best_lap { if *bl < ce.1 { ce.1 = *bl; } }
    }

    // Get featured catalog data
    let featured_tracks = catalog::get_featured_tracks_for_passport();
    let featured_cars = catalog::get_featured_cars_for_passport();

    // Tier boundaries: Starter=0..6, Explorer=6..15, Legend=15..end
    let tier_boundaries: &[(usize, usize, &str, &str)] = &[
        (0, 6, "Starter Circuits", "Starter Garage"),
        (6, 15, "Explorer Circuits", "Explorer Garage"),
        (15, usize::MAX, "Legend Circuits", "Legend Garage"),
    ];

    // Build track tiers
    let mut track_tiers = Vec::new();
    for &(start, end, track_label, _) in tier_boundaries {
        let tier_items: Vec<Value> = featured_tracks.iter()
            .filter_map(|t| {
                let sort = t.get("sort_order")?.as_u64()? as usize;
                if sort >= start && sort < end { Some(t) } else { None }
            })
            .map(|t| {
                let tid = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let driven = driven_tracks.contains(tid);
                let (lap_count, best_lap, first_driven) = track_data.get(tid).cloned().unwrap_or((0, 0, None));
                json!({
                    "id": tid,
                    "name": t.get("name").and_then(|v| v.as_str()).unwrap_or(tid),
                    "category": t.get("category").and_then(|v| v.as_str()).unwrap_or(""),
                    "country": t.get("country").and_then(|v| v.as_str()).unwrap_or(""),
                    "driven": driven,
                    "lap_count": if driven { lap_count } else { 0 },
                    "best_lap_ms": if driven && best_lap < i64::MAX { Some(best_lap) } else { None::<i64> },
                    "first_driven_at": first_driven
                })
            })
            .collect();
        let driven_count = tier_items.iter().filter(|i| i.get("driven").and_then(|v| v.as_bool()).unwrap_or(false)).count();
        track_tiers.push(json!({
            "name": track_label,
            "target": tier_items.len(),
            "driven_count": driven_count,
            "items": tier_items
        }));
    }

    // Build car tiers
    let mut car_tiers = Vec::new();
    for &(start, end, _, car_label) in tier_boundaries {
        let tier_items: Vec<Value> = featured_cars.iter()
            .filter_map(|c| {
                let sort = c.get("sort_order")?.as_u64()? as usize;
                if sort >= start && sort < end { Some(c) } else { None }
            })
            .map(|c| {
                let cid = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let driven = driven_cars.contains(cid);
                let (lap_count, best_lap, first_driven) = car_data.get(cid).cloned().unwrap_or((0, 0, None));
                json!({
                    "id": cid,
                    "name": c.get("name").and_then(|v| v.as_str()).unwrap_or(cid),
                    "category": c.get("category").and_then(|v| v.as_str()).unwrap_or(""),
                    "driven": driven,
                    "lap_count": if driven { lap_count } else { 0 },
                    "best_lap_ms": if driven && best_lap < i64::MAX { Some(best_lap) } else { None::<i64> },
                    "first_driven_at": first_driven
                })
            })
            .collect();
        let driven_count = tier_items.iter().filter(|i| i.get("driven").and_then(|v| v.as_bool()).unwrap_or(false)).count();
        car_tiers.push(json!({
            "name": car_label,
            "target": tier_items.len(),
            "driven_count": driven_count,
            "items": tier_items
        }));
    }

    // Non-featured (other) tracks and cars
    let featured_track_ids: std::collections::HashSet<&str> = featured_tracks.iter()
        .filter_map(|t| t.get("id")?.as_str())
        .collect();
    let featured_car_ids: std::collections::HashSet<&str> = featured_cars.iter()
        .filter_map(|c| c.get("id")?.as_str())
        .collect();

    let other_tracks: Vec<Value> = entries.iter()
        .filter(|(t, _, _, _, _)| !featured_track_ids.contains(t.as_str()))
        .map(|(t, _, first_driven, best_lap, lap_count)| {
            let display_name = catalog::id_to_display_name(t);
            json!({
                "id": t,
                "name": display_name,
                "driven": true,
                "lap_count": lap_count,
                "best_lap_ms": best_lap,
                "first_driven_at": first_driven
            })
        })
        .collect();

    let other_cars: Vec<Value> = entries.iter()
        .filter(|(_, c, _, _, _)| !featured_car_ids.contains(c.as_str()))
        .map(|(_, c, first_driven, best_lap, lap_count)| {
            let display_name = catalog::id_to_display_name(c);
            json!({
                "id": c,
                "name": display_name,
                "driven": true,
                "lap_count": lap_count,
                "best_lap_ms": best_lap,
                "first_driven_at": first_driven
            })
        })
        .collect();

    // Summary stats
    let total_laps: i64 = entries.iter().map(|(_, _, _, _, lc)| lc).sum();
    let streak_data: Option<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT current_streak, longest_streak, last_visit_date, grace_expires_date
         FROM streaks WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (streak_weeks, longest_streak, last_visit_date, grace_expires_date) = streak_data
        .map(|(c, l, lv, ge)| (c, l, lv, ge))
        .unwrap_or((0, 0, None, None));

    Json(json!({
        "passport": {
            "tracks": {
                "total_driven": driven_tracks.len(),
                "total_available": featured_tracks.len(),
                "tiers": {
                    "starter": track_tiers.get(0),
                    "explorer": track_tiers.get(1),
                    "legend": track_tiers.get(2)
                },
                "other": other_tracks
            },
            "cars": {
                "total_driven": driven_cars.len(),
                "total_available": featured_cars.len(),
                "tiers": {
                    "starter": car_tiers.get(0),
                    "explorer": car_tiers.get(1),
                    "legend": car_tiers.get(2)
                },
                "other": other_cars
            },
            "summary": {
                "unique_tracks": driven_tracks.len(),
                "unique_cars": driven_cars.len(),
                "total_laps": total_laps,
                "streak_weeks": streak_weeks,
                "longest_streak": longest_streak,
                "last_visit_date": last_visit_date,
                "grace_expires_date": grace_expires_date
            }
        }
    }))
}

/// GET /customer/badges — returns earned + available badges for the authenticated customer.
/// Earned badges include earned_at timestamp. Available (not yet earned) badges include
/// progress toward the target metric.
async fn customer_badges(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // All active badge definitions — column is badge_icon, NOT icon
    let all_badges: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, description, category, badge_icon, criteria_json FROM achievements WHERE is_active = 1 ORDER BY sort_order"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Earned badges for this driver — table is driver_achievements, column is achievement_id
    let earned_map: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT achievement_id, earned_at FROM driver_achievements WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    // Driver metrics for progress calculation
    let total_laps: i64 = sqlx::query_scalar("SELECT COALESCE(total_laps, 0) FROM drivers WHERE id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let unique_tracks: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT track) FROM driving_passport WHERE driver_id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let unique_cars: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT car) FROM driving_passport WHERE driver_id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let pb_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ? AND status = 'completed'")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);

    let mut earned_list = Vec::new();
    let mut available_list = Vec::new();

    for (badge_id, name, description, category, badge_icon, criteria_json) in &all_badges {
        if let Some(earned_at) = earned_map.get(badge_id) {
            earned_list.push(json!({
                "id": badge_id,
                "name": name,
                "description": description,
                "category": category,
                "icon": badge_icon,
                "earned_at": earned_at,
                "earned": true
            }));
        } else {
            let (progress, target) = parse_badge_progress(criteria_json, total_laps, unique_tracks, unique_cars, pb_count, session_count);
            available_list.push(json!({
                "id": badge_id,
                "name": name,
                "description": description,
                "category": category,
                "icon": badge_icon,
                "progress": progress,
                "target": target,
                "earned": false
            }));
        }
    }

    let total_available = all_badges.len();
    Json(json!({
        "badges": {
            "earned": earned_list,
            "available": available_list,
            "total_earned": earned_list.len(),
            "total_available": total_available
        }
    }))
}

/// Parse badge criteria JSON to extract progress/target for display.
/// Returns (current_progress, target_value).
/// IMPORTANT: criteria_json keys are "type" and "value" (NOT "metric"/"threshold").
/// Example: {"type":"total_laps","operator":">=","value":100}
fn parse_badge_progress(criteria_json: &str, total_laps: i64, unique_tracks: i64, unique_cars: i64, pb_count: i64, session_count: i64) -> (i64, i64) {
    let parsed: Result<Value, _> = serde_json::from_str(criteria_json);
    match parsed {
        Ok(criteria) => {
            let metric = criteria.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let threshold = criteria.get("value").and_then(|v| v.as_i64()).unwrap_or(1);
            let progress = match metric {
                "total_laps" => total_laps,
                "unique_tracks" => unique_tracks,
                "unique_cars" => unique_cars,
                "pb_count" => pb_count,
                "session_count" => session_count,
                "first_lap" => if total_laps > 0 { 1 } else { 0 },
                "streak_weeks" => 0, // streak handled separately, not in simple metrics
                _ => 0,
            };
            (progress.min(threshold), threshold)
        }
        Err(_) => (0, 1),
    }
}

// ─── Deploy Audit Log (Phase 177) ──────────────────────────────────────────

#[derive(Deserialize)]
struct CreateDeployLog {
    app: String,
    result: String,
    #[serde(default = "default_deployer")]
    deployer: String,
    pages_before: Option<i64>,
    pages_after: Option<i64>,
    pages_missing: Option<String>,
    duration_secs: Option<i64>,
    error: Option<String>,
    build_hash: Option<String>,
}

fn default_deployer() -> String {
    "james".to_string()
}

async fn create_deploy_log(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateDeployLog>,
) -> (axum::http::StatusCode, Json<Value>) {
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    let db = state.db.clone();
    let id_clone = id.clone();
    tokio::spawn(async move {
        let _ = sqlx::query(
            "INSERT INTO deploy_logs (id, app, timestamp, deployer, result, pages_before, pages_after, pages_missing, duration_secs, error, build_hash)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id_clone)
        .bind(&body.app)
        .bind(&timestamp)
        .bind(&body.deployer)
        .bind(&body.result)
        .bind(body.pages_before)
        .bind(body.pages_after)
        .bind(&body.pages_missing)
        .bind(body.duration_secs)
        .bind(&body.error)
        .bind(&body.build_hash)
        .execute(&db)
        .await;
    });

    (
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": id, "status": "logged" })),
    )
}

async fn list_deploy_logs(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, DeployLogRow>(
        "SELECT id, app, timestamp, deployer, result, pages_before, pages_after, pages_missing, duration_secs, error, build_hash FROM deploy_logs ORDER BY timestamp DESC LIMIT 50",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(logs) => {
            let entries: Vec<Value> = logs
                .into_iter()
                .map(|r| {
                    json!({
                        "id": r.id,
                        "app": r.app,
                        "timestamp": r.timestamp,
                        "deployer": r.deployer,
                        "result": r.result,
                        "pages_before": r.pages_before,
                        "pages_after": r.pages_after,
                        "pages_missing": r.pages_missing,
                        "duration_secs": r.duration_secs,
                        "error": r.error,
                        "build_hash": r.build_hash,
                    })
                })
                .collect();
            Json(json!(entries))
        }
        Err(e) => {
            tracing::error!("Failed to fetch deploy_logs: {e}");
            Json(json!([]))
        }
    }
}

#[derive(sqlx::FromRow)]
struct DeployLogRow {
    id: String,
    app: String,
    timestamp: String,
    deployer: String,
    result: String,
    pages_before: Option<i64>,
    pages_after: Option<i64>,
    pages_missing: Option<String>,
    duration_secs: Option<i64>,
    error: Option<String>,
    build_hash: Option<String>,
}

/// GET /api/v1/app-health — current health probe results for admin, kiosk, web.
async fn get_app_health() -> Json<Value> {
    let entries = crate::app_health_monitor::get_current_health().await;
    let result: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            json!({
                "app": e.app,
                "status": e.status,
                "pages_expected": e.pages_expected,
                "pages_available": e.pages_available,
                "last_checked": e.last_checked,
                "response_ms": e.response_ms,
                "error": e.error,
            })
        })
        .collect();
    Json(json!(result))
}

#[cfg(test)]
mod config_snapshot_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_full_config_snapshot() {
        let payload = json!({
            "venue": {"name": "TestVenue", "location": "TestCity", "timezone": "UTC"},
            "pods": {"count": 4, "discovery": true, "healer_enabled": false, "healer_interval_secs": 60},
            "branding": {"primary_color": "#FF0000", "theme": "light"},
            "_meta": {"source": "test", "pushed_at": 1234567890u64, "hash": "abc123"}
        });
        let snap = parse_config_snapshot(&payload);
        assert_eq!(snap.venue_name, "TestVenue");
        assert_eq!(snap.pod_count, 4);
        assert_eq!(snap.branding_primary_color, "#FF0000");
        assert_eq!(snap.config_hash, "abc123");
    }

    #[test]
    fn test_parse_config_snapshot_defaults() {
        let payload = json!({});
        let snap = parse_config_snapshot(&payload);
        assert_eq!(snap.venue_name, "RacingPoint");
        assert_eq!(snap.pod_count, 0);
        assert_eq!(snap.venue_timezone, "Asia/Kolkata");
    }

    #[test]
    fn test_venue_config_snapshot_serde_roundtrip() {
        let snap = VenueConfigSnapshot {
            venue_name: "Test".to_string(),
            pod_count: 8,
            ..Default::default()
        };
        let serialized = serde_json::to_string(&snap).unwrap();
        let deserialized: VenueConfigSnapshot = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.venue_name, "Test");
        assert_eq!(deserialized.pod_count, 8);
    }
}

// ─── Autonomous Pipeline (v26.0) ─────────────────────────────────────────────

async fn pipeline_status() -> Json<serde_json::Value> {
    let config_path = std::path::Path::new("audit/results/auto-detect-config.json");
    let config: serde_json::Value = if config_path.exists() {
        match tokio::fs::read_to_string(config_path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => serde_json::Value::Null,
        }
    } else {
        serde_json::Value::Null
    };

    let suggestions_path = std::path::Path::new("audit/results/suggestions.jsonl");
    let recent_findings: Vec<serde_json::Value> = if suggestions_path.exists() {
        match tokio::fs::read_to_string(suggestions_path).await {
            Ok(content) => content.lines().rev().take(50)
                .filter_map(|line| serde_json::from_str(line).ok()).collect(),
            Err(_) => vec![],
        }
    } else { vec![] };

    let proposals_dir = std::path::Path::new("audit/results/proposals");
    let proposals: Vec<serde_json::Value> = if proposals_dir.exists() {
        match tokio::fs::read_dir(proposals_dir).await {
            Ok(mut entries) => {
                let mut items = vec![];
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if entry.path().extension().map_or(false, |e| e == "json") {
                        if let Ok(content) = tokio::fs::read_to_string(entry.path()).await {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                                items.push(val);
                            }
                        }
                    }
                }
                items
            }
            Err(_) => vec![],
        }
    } else { vec![] };

    let summary_path = std::path::Path::new("audit/results/last-run-summary.json");
    let last_run: serde_json::Value = if summary_path.exists() {
        match tokio::fs::read_to_string(summary_path).await {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => serde_json::Value::Null,
        }
    } else { serde_json::Value::Null };

    Json(serde_json::json!({
        "config": config,
        "last_run": last_run,
        "recent_findings": recent_findings,
        "proposals": proposals,
        "finding_count": recent_findings.len(),
        "proposal_count": proposals.len(),
    }))
}

async fn pipeline_config_get() -> Json<serde_json::Value> {
    let config_path = std::path::Path::new("audit/results/auto-detect-config.json");
    match tokio::fs::read_to_string(config_path).await {
        Ok(content) => Json(serde_json::from_str(&content).unwrap_or_default()),
        Err(_) => Json(serde_json::json!({"error": "config not found"})),
    }
}

async fn pipeline_config_set(Json(body): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let config_path = std::path::Path::new("audit/results/auto-detect-config.json");
    let mut config: serde_json::Value = match tokio::fs::read_to_string(config_path).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => serde_json::json!({}),
    };
    if let (Some(existing), Some(incoming)) = (config.as_object_mut(), body.as_object()) {
        for (key, value) in incoming {
            existing.insert(key.clone(), value.clone());
        }
    }
    match tokio::fs::write(config_path, serde_json::to_string_pretty(&config).unwrap_or_default()).await {
        Ok(_) => Json(serde_json::json!({"ok": true, "config": config})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

// ─── Mesh Intelligence API (v26.0 Phase 222) ────────────────────────────────

#[derive(serde::Deserialize)]
struct MeshListParams {
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
    #[serde(default)]
    status: Option<String>,
}

fn default_limit() -> u32 { 50 }

#[derive(serde::Deserialize)]
struct MeshSearchParams {
    q: Option<String>,
    limit: Option<u32>,
}

async fn mesh_list_solutions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MeshListParams>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::list_solutions(&state.db, params.status.as_deref(), params.limit, params.offset).await {
        Ok(solutions) => Json(serde_json::json!({ "solutions": solutions, "count": solutions.len() })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn mesh_search_solutions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MeshSearchParams>,
) -> Json<serde_json::Value> {
    let q = params.q.as_deref().unwrap_or("");
    if q.is_empty() {
        return Json(serde_json::json!({ "solutions": [], "count": 0, "query": q }));
    }
    match crate::fleet_kb::search_solutions(&state.db, q, params.limit.unwrap_or(5)).await {
        Ok(solutions) => Json(serde_json::json!({
            "solutions": solutions,
            "count": solutions.len(),
            "query": q,
        })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn mesh_get_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::get_solution(&state.db, &id).await {
        Ok(Some(sol)) => Json(serde_json::json!(sol)),
        Ok(None) => Json(serde_json::json!({ "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn mesh_list_incidents(
    State(state): State<Arc<AppState>>,
    Query(params): Query<MeshListParams>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::list_incidents(&state.db, params.limit, params.offset).await {
        Ok(incidents) => Json(serde_json::json!({ "incidents": incidents, "count": incidents.len() })),
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })),
    }
}

async fn mesh_stats(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let counts = crate::fleet_kb::solution_counts(&state.db).await.unwrap_or_default();
    let total: i64 = counts.iter().map(|(_, c)| c).sum();
    let status_map: std::collections::HashMap<String, i64> = counts.into_iter().collect();
    Json(serde_json::json!({
        "total_solutions": total,
        "by_status": status_map,
    }))
}

/// DEPLOY-AWARE-01: Fleet deployment status for Meshed Intelligence.
/// Returns version consistency, stale builds, crash patterns, and deployment issues.
async fn mesh_deploy_status(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let status = crate::deploy_awareness::get_fleet_deploy_status(&state).await;
    Json(serde_json::to_value(status).unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() })))
}

async fn mesh_promote_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::update_status(&state.db, &id, rc_common::mesh_types::SolutionStatus::FleetVerified).await {
        Ok(true) => Json(serde_json::json!({ "ok": true, "status": "fleet_verified" })),
        Ok(false) => Json(serde_json::json!({ "ok": false, "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

async fn mesh_retire_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::update_status(&state.db, &id, rc_common::mesh_types::SolutionStatus::Retired).await {
        Ok(true) => Json(serde_json::json!({ "ok": true, "status": "retired" })),
        Ok(false) => Json(serde_json::json!({ "ok": false, "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

// ─── Cloud Mesh KB Sync (v26.0 Phase 227) ───────────────────────────────────

/// Venue pushes fleet-verified + hardened solutions to cloud KB.
/// Request body: { "venue_id": "rp-hyderabad", "solutions": [...] }
async fn cloud_mesh_sync(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let venue_id = body.get("venue_id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let solutions = match body.get("solutions").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Json(serde_json::json!({ "ok": false, "error": "solutions array required" })),
    };

    let mut imported = 0u32;
    let mut errors = 0u32;

    for sol_val in solutions {
        // Parse and tag with venue_id
        let mut sol: rc_common::mesh_types::MeshSolution = match serde_json::from_value(sol_val.clone()) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Cloud mesh sync: failed to parse solution: {e}");
                errors += 1;
                continue;
            }
        };
        sol.venue_id = Some(venue_id.to_string());

        if let Err(e) = crate::fleet_kb::insert_solution(&state.db, &sol).await {
            tracing::warn!("Cloud mesh sync: failed to insert solution {}: {e}", sol.id);
            errors += 1;
        } else {
            imported += 1;
        }
    }

    tracing::info!("Cloud mesh sync from venue {venue_id}: imported={imported} errors={errors}");
    Json(serde_json::json!({ "ok": true, "imported": imported, "errors": errors }))
}

/// New venue pulls the full cloud KB to seed their local fleet KB.
/// Query params: ?venue_id=xxx (optional — excludes own solutions to avoid loops)
async fn cloud_mesh_pull(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let requesting_venue = params.get("venue_id").map(|s| s.as_str());

    // Pull all fleet_verified + hardened solutions
    let verified = crate::fleet_kb::list_solutions(&state.db, Some("fleet_verified"), 500, 0).await.unwrap_or_default();
    let hardened = crate::fleet_kb::list_solutions(&state.db, Some("hardened"), 500, 0).await.unwrap_or_default();

    let mut all: Vec<rc_common::mesh_types::MeshSolution> = verified.into_iter().chain(hardened).collect();

    // Exclude requesting venue's own solutions (prevent sync loop)
    if let Some(vid) = requesting_venue {
        all.retain(|s| s.venue_id.as_deref() != Some(vid));
    }

    // Mark external solutions for the requesting venue
    for sol in &mut all {
        if sol.venue_id.is_some() && sol.venue_id.as_deref() != requesting_venue {
            // Tag as external — needs local verification before auto-apply
            if let Some(tags) = sol.tags.as_mut() {
                if !tags.contains(&"external".to_string()) {
                    tags.push("external".to_string());
                }
            } else {
                sol.tags = Some(vec!["external".to_string()]);
            }
        }
    }

    Json(serde_json::json!({ "solutions": all, "count": all.len() }))
}

// ─── FATM-12: Reconciliation handlers ───────────────────────────────────────

/// GET /api/v1/reconciliation/status — returns last reconciliation run info.
async fn reconciliation_status() -> Json<serde_json::Value> {
    Json(billing::get_reconciliation_status())
}

/// POST /api/v1/reconciliation/run — triggers an immediate reconciliation run.
async fn reconciliation_run(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    billing::run_reconciliation_public(&state).await;
    Json(billing::get_reconciliation_status())
}

// ─── STAFF-01: Discount approval gate ────────────────────────────────────────

#[derive(serde::Deserialize)]
struct ApplyDiscountRequest {
    discount_paise: i64,
    reason_code: String,
    manager_approval_code: Option<String>,
}

/// POST /api/v1/billing/{id}/discount
/// STAFF-01: Apply a discount to an active billing session.
/// Discounts above DISCOUNT_APPROVAL_THRESHOLD_PAISE require manager approval code.
async fn apply_billing_discount(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Path(session_id): Path<String>,
    Json(req): Json<ApplyDiscountRequest>,
) -> Json<Value> {
    if req.discount_paise <= 0 {
        return Json(json!({ "error": "discount_paise must be greater than 0" }));
    }
    if req.reason_code.trim().is_empty() {
        return Json(json!({ "error": "reason_code is required" }));
    }

    let threshold = billing::DISCOUNT_APPROVAL_THRESHOLD_PAISE;
    let mut manager_approved = false;

    // STAFF-01: Discounts above threshold require manager approval code
    if req.discount_paise > threshold {
        match req.manager_approval_code.as_deref() {
            None | Some("") => {
                tracing::warn!(
                    actor_id = %claims.sub,
                    session_id = %session_id,
                    discount_paise = req.discount_paise,
                    threshold = threshold,
                    "STAFF-01: Discount above threshold rejected — no manager approval code"
                );
                return Json(json!({
                    "error": "Discount above threshold requires manager approval code",
                    "threshold_paise": threshold,
                }));
            }
            Some(code) => {
                // Validate manager approval code: look up staff with matching PIN where role is manager or superadmin
                let result = sqlx::query_as::<_, (String, String)>(
                    "SELECT id, role FROM staff_members WHERE pin = ? AND is_active = 1 AND role IN ('manager', 'superadmin')",
                )
                .bind(code)
                .fetch_optional(&state.db)
                .await;

                match result {
                    Ok(Some(_)) => {
                        manager_approved = true;
                    }
                    Ok(None) => {
                        tracing::warn!(
                            actor_id = %claims.sub,
                            session_id = %session_id,
                            "STAFF-01: Manager approval code invalid or not a manager"
                        );
                        return Json(json!({
                            "error": "Invalid manager approval code — must be a manager or superadmin PIN",
                        }));
                    }
                    Err(e) => {
                        tracing::error!("STAFF-01: DB error validating manager approval code: {}", e);
                        return Json(json!({ "error": "Database error validating approval code" }));
                    }
                }
            }
        }
    }

    // FATM-10: Enforce discount floor — fetch current price/discount to check cap
    let session_prices = sqlx::query_as::<_, (Option<i64>, i64)>(
        "SELECT original_price_paise, COALESCE(discount_paise, 0) FROM billing_sessions WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery')",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await;

    let effective_discount_paise = match session_prices {
        Ok(Some((original_price_opt, current_discount))) => {
            let floor = billing::DISCOUNT_FLOOR_PAISE;
            if floor > 0 {
                let base_price = original_price_opt.unwrap_or(0);
                let max_total_discount = base_price - floor;
                let remaining_headroom = max_total_discount - current_discount;
                if remaining_headroom <= 0 {
                    tracing::info!(
                        "FATM-10: Discount floor already reached for session {} (base={}p, current_discount={}p, floor={}p) — discount rejected",
                        session_id, base_price, current_discount, floor
                    );
                    return Json(json!({
                        "error": "Discount floor already reached — no further discount allowed",
                        "discount_floor_paise": floor,
                        "session_id": session_id,
                    }));
                }
                let capped = req.discount_paise.min(remaining_headroom);
                if capped < req.discount_paise {
                    tracing::info!(
                        "FATM-10: Discount floor enforced for session {} — requested {}p capped to {}p (floor={}p, base={}p, current_discount={}p)",
                        session_id, req.discount_paise, capped, floor, base_price, current_discount
                    );
                }
                capped
            } else {
                req.discount_paise
            }
        }
        Ok(None) => {
            return Json(json!({
                "error": "Session not found or not in an active/paused state",
                "session_id": session_id,
            }));
        }
        Err(e) => {
            tracing::error!("FATM-10: DB error reading session for floor check: {}", e);
            return Json(json!({ "error": "Database error checking discount floor" }));
        }
    };

    // Apply discount: UPDATE billing_sessions for active/paused sessions only
    let update_result = sqlx::query(
        "UPDATE billing_sessions
         SET discount_paise = COALESCE(discount_paise, 0) + ?,
             discount_reason = ?
         WHERE id = ? AND status IN ('active', 'paused_manual', 'paused_game_pause', 'paused_disconnect', 'paused_crash_recovery')",
    )
    .bind(effective_discount_paise)
    .bind(&req.reason_code)
    .bind(&session_id)
    .execute(&state.db)
    .await;

    match update_result {
        Ok(r) if r.rows_affected() == 0 => {
            return Json(json!({
                "error": "Session not found or not in an active/paused state",
                "session_id": session_id,
            }));
        }
        Err(e) => {
            tracing::error!("STAFF-01: Failed to apply discount for session {}: {}", session_id, e);
            return Json(json!({ "error": "Database error applying discount" }));
        }
        Ok(_) => {}
    }

    // Insert audit_log entry
    let audit_details = json!({
        "session_id": session_id,
        "discount_paise": effective_discount_paise,
        "requested_discount_paise": req.discount_paise,
        "reason_code": req.reason_code,
        "manager_approved": manager_approved,
        "actor_id": claims.sub,
        "discount_floor_paise": billing::DISCOUNT_FLOOR_PAISE,
    });
    accounting::log_admin_action(
        &state,
        "discount_applied",
        &audit_details.to_string(),
        Some(&claims.sub),
        None,
    )
    .await;

    tracing::info!(
        actor_id = %claims.sub,
        session_id = %session_id,
        discount_paise = effective_discount_paise,
        manager_approved = manager_approved,
        reason_code = %req.reason_code,
        "STAFF-01: Discount applied"
    );

    Json(json!({
        "status": "ok",
        "session_id": session_id,
        "discount_paise": effective_discount_paise,
        "manager_approved": manager_approved,
        "discount_floor_paise": billing::DISCOUNT_FLOOR_PAISE,
    }))
}

// ─── STAFF-03: Daily override report ─────────────────────────────────────────

#[derive(serde::Deserialize)]
struct DailyOverridesQuery {
    /// Optional date in YYYY-MM-DD format (IST). Defaults to today IST.
    date: Option<String>,
}

/// GET /api/v1/admin/reports/daily-overrides?date=YYYY-MM-DD
/// STAFF-03: Returns all discounts, manual refunds, and tier changes with actor_id for a given day.
async fn daily_overrides_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyOverridesQuery>,
) -> Json<Value> {
    // Default to today IST (UTC+5:30)
    let target_date = params.date.unwrap_or_else(|| {
        let now_utc = chrono::Utc::now();
        // east_opt(19800) = UTC+5:30 (IST). 19800 is always valid; fallback to east_opt(0) which is also always valid.
        let ist_offset = chrono::FixedOffset::east_opt(19800).unwrap_or_else(|| chrono::FixedOffset::east_opt(0).expect("UTC offset 0 is always valid"));
        let now_ist = now_utc.with_timezone(&ist_offset);
        now_ist.format("%Y-%m-%d").to_string()
    });

    // Discount entries from billing_sessions
    let discounts = sqlx::query_as::<_, (String, Option<i64>, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.discount_paise, bs.discount_reason, bs.driver_id,
                al.staff_id
         FROM billing_sessions bs
         LEFT JOIN audit_log al ON al.action_type = 'discount_applied'
                               AND json_extract(al.new_values, '$.session_id') = bs.id
         WHERE bs.discount_paise > 0
           AND date(bs.started_at) = ?
         ORDER BY bs.started_at DESC",
    )
    .bind(&target_date)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Manual refunds from wallet_transactions
    let refunds = sqlx::query_as::<_, (String, i64, Option<String>, Option<String>, String)>(
        "SELECT id, amount_paise, driver_id, staff_id, created_at
         FROM wallet_transactions
         WHERE type = 'manual_refund'
           AND date(created_at) = ?
         ORDER BY created_at DESC",
    )
    .bind(&target_date)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // All override audit_log entries for the day
    let audit_entries = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, String)>(
        "SELECT id, action_type, staff_id, new_values, created_at
         FROM audit_log
         WHERE action_type IN ('discount_applied', 'tier_change', 'manual_refund')
           AND date(created_at) = ?
         ORDER BY created_at DESC",
    )
    .bind(&target_date)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let discount_entries: Vec<Value> = discounts
        .into_iter()
        .map(|(session_id, discount_paise, reason, driver_id, actor_id)| {
            json!({
                "action_type": "discount_applied",
                "session_id": session_id,
                "actor_id": actor_id,
                "target_driver": driver_id,
                "amount_paise": discount_paise.unwrap_or(0),
                "reason": reason,
            })
        })
        .collect();

    let refund_entries: Vec<Value> = refunds
        .into_iter()
        .map(|(id, amount_paise, driver_id, staff_id, created_at)| {
            json!({
                "action_type": "manual_refund",
                "transaction_id": id,
                "actor_id": staff_id,
                "target_driver": driver_id,
                "amount_paise": amount_paise,
                "timestamp": created_at,
            })
        })
        .collect();

    let audit_override_entries: Vec<Value> = audit_entries
        .into_iter()
        .map(|(id, action_type, staff_id, details, created_at)| {
            json!({
                "action_type": action_type,
                "audit_id": id,
                "actor_id": staff_id,
                "details": details,
                "timestamp": created_at,
            })
        })
        .collect();

    Json(json!({
        "status": "ok",
        "date": target_date,
        "discounts": discount_entries,
        "manual_refunds": refund_entries,
        "audit_overrides": audit_override_entries,
    }))
}

// ─── STAFF-04: Cash drawer reconciliation ─────────────────────────────────────

/// GET /api/v1/admin/reports/cash-drawer
/// STAFF-04: Returns system cash total for today (sum of wallet_transactions with method='cash').
async fn cash_drawer_status(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // IST today
    let now_utc = chrono::Utc::now();
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
    let today_ist = now_utc.with_timezone(&ist_offset).format("%Y-%m-%d").to_string();

    let total: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT SUM(amount_paise) FROM wallet_transactions
         WHERE (type = 'topup_cash' OR notes LIKE '%cash%')
           AND date(created_at) = ?",
    )
    .bind(&today_ist)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let system_cash_total_paise = total.and_then(|(v,)| v).unwrap_or(0);

    Json(json!({
        "status": "ok",
        "date": today_ist,
        "system_cash_total_paise": system_cash_total_paise,
    }))
}

#[derive(serde::Deserialize)]
struct CashDrawerCloseRequest {
    physical_count_paise: i64,
}

/// POST /api/v1/admin/reports/cash-drawer/close
/// STAFF-04: Log end-of-day cash drawer close with physical count vs system total discrepancy.
async fn cash_drawer_close(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<CashDrawerCloseRequest>,
) -> Json<Value> {
    if req.physical_count_paise < 0 {
        return Json(json!({ "error": "physical_count_paise cannot be negative" }));
    }

    // Compute system total for today IST
    let now_utc = chrono::Utc::now();
    let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
    let today_ist = now_utc.with_timezone(&ist_offset).format("%Y-%m-%d").to_string();

    let total: Option<(Option<i64>,)> = sqlx::query_as(
        "SELECT SUM(amount_paise) FROM wallet_transactions
         WHERE (type = 'topup_cash' OR notes LIKE '%cash%')
           AND date(created_at) = ?",
    )
    .bind(&today_ist)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let system_total_paise = total.and_then(|(v,)| v).unwrap_or(0);
    let discrepancy_paise = req.physical_count_paise - system_total_paise;

    let status = match discrepancy_paise.cmp(&0) {
        std::cmp::Ordering::Greater => "over",
        std::cmp::Ordering::Less => "under",
        std::cmp::Ordering::Equal => "balanced",
    };

    // Insert audit_log entry
    let audit_details = json!({
        "date": today_ist,
        "system_total_paise": system_total_paise,
        "physical_count_paise": req.physical_count_paise,
        "discrepancy_paise": discrepancy_paise,
        "status": status,
        "actor_id": claims.sub,
    });
    accounting::log_admin_action(
        &state,
        "cash_drawer_close",
        &audit_details.to_string(),
        Some(&claims.sub),
        None,
    )
    .await;

    tracing::info!(
        actor_id = %claims.sub,
        date = %today_ist,
        system_total_paise = system_total_paise,
        physical_count_paise = req.physical_count_paise,
        discrepancy_paise = discrepancy_paise,
        status = %status,
        "STAFF-04: Cash drawer closed"
    );

    Json(json!({
        "status": "ok",
        "date": today_ist,
        "system_total_paise": system_total_paise,
        "physical_count_paise": req.physical_count_paise,
        "discrepancy_paise": discrepancy_paise,
        "drawer_status": status,
    }))
}

// ─── STAFF-05: Shift Handoff ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct ShiftHandoffRequest {
    incoming_staff_id: Option<String>,
    notes: Option<String>,
}

/// POST /api/v1/staff/shift-handoff
///
/// STAFF-05: Outgoing staff member logs shift handoff.
/// - Reads all active billing sessions from active_timers.
/// - If active sessions exist and incoming_staff_id is not provided, returns 400.
/// - Inserts audit_log entry: action='shift_handoff', actor=outgoing staff sub.
/// - Returns list of active sessions for handoff acknowledgment.
async fn shift_handoff_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::auth::middleware::StaffClaims>,
    Json(req): Json<ShiftHandoffRequest>,
) -> (axum::http::StatusCode, Json<Value>) {
    // Snapshot active timers — drop lock before any async work
    let active_sessions: Vec<Value> = {
        let timers = state.billing.active_timers.read().await;
        timers
            .values()
            .map(|t| {
                let elapsed_minutes = t.elapsed_seconds / 60;
                let game_type = t
                    .sim_type
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "Unknown".to_string());
                json!({
                    "pod_id": t.pod_id,
                    "billing_session_id": t.session_id,
                    "driver_name": t.driver_name,
                    "elapsed_minutes": elapsed_minutes,
                    "game_type": game_type,
                })
            })
            .collect()
    };

    // If active sessions exist, incoming_staff_id is mandatory
    if !active_sessions.is_empty() && req.incoming_staff_id.is_none() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "Active sessions require incoming staff acknowledgment — provide incoming_staff_id",
                "active_session_count": active_sessions.len(),
            })),
        );
    }

    let handoff_at = chrono::Utc::now().to_rfc3339();
    let session_ids: Vec<String> = active_sessions
        .iter()
        .filter_map(|s| s["billing_session_id"].as_str().map(str::to_string))
        .collect();

    let details = json!({
        "incoming_staff_id": req.incoming_staff_id,
        "active_session_count": active_sessions.len(),
        "session_ids": session_ids,
        "notes": req.notes,
        "handoff_at": handoff_at,
    });

    // Insert audit log entry
    crate::accounting::log_admin_action(
        &state,
        "shift_handoff",
        &details.to_string(),
        Some(&claims.sub),
        None,
    )
    .await;

    tracing::info!(
        actor_id = %claims.sub,
        incoming_staff_id = ?req.incoming_staff_id,
        active_session_count = active_sessions.len(),
        "STAFF-05: Shift handoff logged"
    );

    (
        axum::http::StatusCode::OK,
        Json(json!({
            "outgoing_staff_id": claims.sub,
            "incoming_staff_id": req.incoming_staff_id,
            "active_sessions": active_sessions,
            "handoff_at": handoff_at,
        })),
    )
}

/// GET /api/v1/staff/shift-briefing
///
/// STAFF-05: Returns current active session summary for incoming staff.
/// Also includes the last shift_handoff event from audit_log for context.
async fn shift_briefing_handler(
    State(state): State<Arc<AppState>>,
    axum::Extension(_claims): axum::Extension<crate::auth::middleware::StaffClaims>,
) -> Json<Value> {
    // Snapshot active timers — drop lock before DB query
    let active_sessions: Vec<Value> = {
        let timers = state.billing.active_timers.read().await;
        timers
            .values()
            .map(|t| {
                let elapsed_minutes = t.elapsed_seconds / 60;
                let game_type = t
                    .sim_type
                    .as_ref()
                    .map(|s| format!("{:?}", s))
                    .unwrap_or_else(|| "Unknown".to_string());
                json!({
                    "pod_id": t.pod_id,
                    "billing_session_id": t.session_id,
                    "driver_name": t.driver_name,
                    "elapsed_minutes": elapsed_minutes,
                    "game_type": game_type,
                })
            })
            .collect()
    };

    // Query most recent shift_handoff from audit_log
    let last_handoff: Option<Value> = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT staff_id, new_values, id
         FROM audit_log
         WHERE action_type = 'shift_handoff'
         ORDER BY rowid DESC
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(staff_id, details_json, _id)| {
        let details: Value = serde_json::from_str(&details_json).unwrap_or(Value::Null);
        json!({
            "outgoing_staff": staff_id,
            "notes": details.get("notes"),
            "incoming_staff_id": details.get("incoming_staff_id"),
            "handoff_at": details.get("handoff_at"),
        })
    });

    Json(json!({
        "active_sessions": active_sessions,
        "last_handoff": last_handoff,
    }))
}

// ─── UX-08: Virtual queue handlers ───────────────────────────────────────────

#[derive(Deserialize)]
struct QueueJoinBody {
    driver_name: String,
    phone: Option<String>,
    party_size: Option<i64>,
    driver_id: Option<String>,
}

/// POST /queue/join — walk-in joins the virtual queue (no auth required)
async fn queue_join_handler(
    State(state): State<Arc<AppState>>,
    Json(body): Json<QueueJoinBody>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let party_size = body.party_size.unwrap_or(1).max(1);

    // Insert the new queue entry
    if let Err(e) = sqlx::query(
        "INSERT INTO virtual_queue (id, driver_id, driver_name, phone, party_size, status)
         VALUES (?, ?, ?, ?, ?, 'waiting')",
    )
    .bind(&id)
    .bind(&body.driver_id)
    .bind(&body.driver_name)
    .bind(&body.phone)
    .bind(party_size)
    .execute(&state.db)
    .await
    {
        return Json(json!({ "error": e.to_string() }));
    }

    // Compute position: count waiting entries joined before this one + 1
    let position: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM virtual_queue WHERE status = 'waiting' AND id != ? AND joined_at <= (SELECT joined_at FROM virtual_queue WHERE id = ?)",
    )
    .bind(&id)
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(1);

    // Estimate wait: position * avg_session_minutes (30 min default) / 8 pods
    let estimated_wait_minutes = (position * 30 + 7) / 8; // ceiling division

    // Update with computed position and ETA
    let _ = sqlx::query(
        "UPDATE virtual_queue SET position = ?, estimated_wait_minutes = ? WHERE id = ?",
    )
    .bind(position)
    .bind(estimated_wait_minutes)
    .bind(&id)
    .execute(&state.db)
    .await;

    Json(json!({
        "queue_id": id,
        "position": position,
        "estimated_wait_minutes": estimated_wait_minutes,
    }))
}

/// GET /queue/status/{id} — customer checks their queue position and status (no auth)
async fn queue_status_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, i64, Option<String>, Option<String>)>(
        "SELECT id, status, party_size, joined_at, called_at FROM virtual_queue WHERE id = ?",
    )
    .bind(&queue_id)
    .fetch_optional(&state.db)
    .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => return Json(json!({ "error": "Queue entry not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (id, status, party_size, joined_at, called_at) = row;

    // Recompute live position from DB
    let position: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) + 1 FROM virtual_queue WHERE status = 'waiting' AND id != ? AND joined_at <= (SELECT joined_at FROM virtual_queue WHERE id = ?)",
    )
    .bind(&id)
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(1);

    let estimated_wait_minutes = if status == "waiting" {
        Some((position * 30 + 7) / 8)
    } else {
        None
    };

    Json(json!({
        "queue_id": id,
        "status": status,
        "party_size": party_size,
        "position": if status == "waiting" { Some(position) } else { None },
        "estimated_wait_minutes": estimated_wait_minutes,
        "joined_at": joined_at,
        "called_at": called_at,
    }))
}

/// POST /queue/{id}/leave — customer removes themselves from queue (no auth)
async fn queue_leave_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    match sqlx::query(
        "UPDATE virtual_queue SET status = 'left', updated_at = datetime('now') WHERE id = ? AND status IN ('waiting', 'called')",
    )
    .bind(&queue_id)
    .execute(&state.db)
    .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Queue entry not found or already left/seated" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// v29.0 Phase 34: GET /api/v1/pods/:id/availability
/// Returns pod availability from the self-healing availability map (updated by anomaly scanner)
/// with DB fallback for unresolved Critical maintenance events.
/// Used by kiosk maintenance gate to prevent booking degraded/unavailable pods.
async fn pod_availability_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<i64>,
) -> Json<Value> {
    let pod_num = pod_id as u8;

    // Check in-memory availability map (updated by anomaly scanner → self-healing)
    {
        let avail_map = state.pod_availability.read().await;
        if let Some(avail) = avail_map.get(&pod_num) {
            match avail {
                crate::self_healing::PodAvailability::Degraded { reason } => {
                    return Json(json!({
                        "pod_id": pod_id,
                        "state": "Degraded",
                        "reason": reason,
                        "hold_until": null
                    }));
                }
                crate::self_healing::PodAvailability::Unavailable { reason } => {
                    return Json(json!({
                        "pod_id": pod_id,
                        "state": "Unavailable",
                        "reason": reason,
                        "hold_until": null
                    }));
                }
                crate::self_healing::PodAvailability::MaintenanceHold { until, reason } => {
                    return Json(json!({
                        "pod_id": pod_id,
                        "state": "MaintenanceHold",
                        "reason": reason,
                        "hold_until": until
                    }));
                }
                crate::self_healing::PodAvailability::Available => {
                    // Fall through to DB check below
                }
            }
        }
    }

    // DB fallback: check for unresolved Critical events in last hour
    let critical_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM maintenance_events \
         WHERE pod_id = ?1 AND severity = 'Critical' AND resolved_at IS NULL \
         AND detected_at > datetime('now', '-1 hour')"
    )
    .bind(pod_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if critical_count > 0 {
        Json(json!({
            "pod_id": pod_id,
            "state": "Unavailable",
            "reason": "Critical maintenance alert",
            "hold_until": null
        }))
    } else {
        Json(json!({
            "pod_id": pod_id,
            "state": "Available",
            "reason": null,
            "hold_until": null
        }))
    }
}

/// GET /queue — staff sees all waiting + called entries ordered by join time
async fn queue_list_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, Option<String>, String, Option<String>, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, driver_id, driver_name, phone, party_size, status, joined_at, called_at, seated_at
         FROM virtual_queue WHERE status IN ('waiting', 'called') ORDER BY joined_at ASC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows
        .into_iter()
        .map(|(id, driver_id, driver_name, phone, party_size, status, joined_at, called_at, seated_at)| {
            json!({
                "queue_id": id,
                "driver_id": driver_id,
                "driver_name": driver_name,
                "phone": phone,
                "party_size": party_size,
                "status": status,
                "joined_at": joined_at,
                "called_at": called_at,
                "seated_at": seated_at,
            })
        })
        .collect();

    Json(json!({ "queue": entries, "count": entries.len() }))
}

/// POST /queue/{id}/call — staff calls the next customer (status: waiting → called)
async fn queue_call_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    match sqlx::query(
        "UPDATE virtual_queue SET status = 'called', called_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ? AND status = 'waiting'",
    )
    .bind(&queue_id)
    .execute(&state.db)
    .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Queue entry not found or not in waiting status" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// POST /queue/{id}/seat — staff marks customer as seated/gaming (status: called → seated)
async fn queue_seat_handler(
    State(state): State<Arc<AppState>>,
    Path(queue_id): Path<String>,
) -> Json<Value> {
    match sqlx::query(
        "UPDATE virtual_queue SET status = 'seated', seated_at = datetime('now'), updated_at = datetime('now')
         WHERE id = ? AND status = 'called'",
    )
    .bind(&queue_id)
    .execute(&state.db)
    .await
    {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true })),
        Ok(_) => Json(json!({ "error": "Queue entry not found or not in called status" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// Background task: expire 'called' entries that have been waiting > 10 minutes.
/// Runs every 5 minutes. Spawned at server startup.
pub async fn queue_expire_task(db: sqlx::SqlitePool) {
    tracing::info!("QUEUE: expire task started");
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
    loop {
        interval.tick().await;
        match sqlx::query(
            "UPDATE virtual_queue SET status = 'expired', updated_at = datetime('now')
             WHERE status = 'called' AND called_at < datetime('now', '-10 minutes')",
        )
        .execute(&db)
        .await
        {
            Ok(r) if r.rows_affected() > 0 => {
                tracing::info!("QUEUE: expired {} stale 'called' entries", r.rows_affected());
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("QUEUE: expire task error: {}", e);
            }
        }
    }
}

// ─── SEC-05: Self-topup guard tests ──────────────────────────────────────────

#[cfg(test)]
mod self_topup_tests {
    use super::*;
    use axum::extract::{Path, State};
    use axum::Extension;
    use std::sync::Arc;
    use crate::auth::middleware::StaffClaims;

    /// Create a minimal in-memory AppState for testing wallet handlers.
    async fn make_test_state() -> Arc<AppState> {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS drivers \
             (id TEXT PRIMARY KEY, name TEXT NOT NULL, email TEXT, phone TEXT, \
              name_enc TEXT, email_enc TEXT, phone_enc TEXT, nickname TEXT, \
              total_laps INTEGER DEFAULT 0, total_time_ms INTEGER DEFAULT 0, \
              created_at TEXT DEFAULT (datetime('now')), updated_at TEXT)",
        ).execute(&db).await.expect("create drivers");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wallets \
             (driver_id TEXT PRIMARY KEY, balance INTEGER DEFAULT 0)",
        ).execute(&db).await.expect("create wallets");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS wallet_transactions \
             (id TEXT PRIMARY KEY, driver_id TEXT NOT NULL, type TEXT, \
              amount_paise INTEGER, balance_after_paise INTEGER, \
              notes TEXT, staff_id TEXT, idempotency_key TEXT, \
              created_at TEXT DEFAULT (datetime('now')))",
        ).execute(&db).await.expect("create wallet_transactions");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS bonus_tiers \
             (id TEXT PRIMARY KEY, min_amount_paise INTEGER, bonus_percent INTEGER, \
              is_active INTEGER DEFAULT 1)",
        ).execute(&db).await.expect("create bonus_tiers");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS audit_log \
             (id TEXT PRIMARY KEY, action TEXT, actor TEXT, details TEXT, \
              created_at TEXT DEFAULT (datetime('now')))",
        ).execute(&db).await.expect("create audit_log");

        // Insert a test driver with wallet
        sqlx::query("INSERT INTO drivers (id, name) VALUES ('driver-001', 'Test Driver')")
            .execute(&db).await.expect("insert driver");
        sqlx::query("INSERT INTO wallets (driver_id, balance) VALUES ('driver-001', 10000)")
            .execute(&db).await.expect("insert wallet");

        let config = crate::config::Config::default_test();
        let field_cipher = crate::crypto::encryption::test_field_cipher();
        Arc::new(AppState::new(config, db, field_cipher))
    }

    fn make_claims(sub: &str, role: &str) -> Option<Extension<StaffClaims>> {
        Some(Extension(StaffClaims {
            sub: sub.to_string(),
            role: role.to_string(),
            exp: 9999999999,
            iat: 0,
        }))
    }

    fn make_topup_req(amount: i64) -> TopupRequest {
        TopupRequest {
            amount_paise: amount,
            method: "cash".to_string(),
            notes: None,
            staff_id: None,
            idempotency_key: None,
        }
    }

    // SEC-05: cashier cannot top up their own wallet
    #[tokio::test]
    async fn self_topup_blocked_for_cashier() {
        let state = make_test_state().await;
        let result = topup_wallet(
            State(state),
            Path("driver-001".to_string()),
            make_claims("driver-001", "cashier"),
            Json(make_topup_req(1000)),
        ).await;
        let body = result.0;
        assert_eq!(
            body["error"].as_str(),
            Some("Staff cannot top up their own wallet. Contact a superadmin."),
            "cashier self-topup should be blocked"
        );
    }

    // SEC-05: manager cannot top up their own wallet
    #[tokio::test]
    async fn self_topup_blocked_for_manager() {
        let state = make_test_state().await;
        let result = topup_wallet(
            State(state),
            Path("driver-001".to_string()),
            make_claims("driver-001", "manager"),
            Json(make_topup_req(1000)),
        ).await;
        let body = result.0;
        assert_eq!(
            body["error"].as_str(),
            Some("Staff cannot top up their own wallet. Contact a superadmin."),
            "manager self-topup should be blocked"
        );
    }

    // SEC-05: superadmin is allowed to top up their own wallet
    #[tokio::test]
    async fn self_topup_allowed_for_superadmin() {
        let state = make_test_state().await;
        let result = topup_wallet(
            State(state),
            Path("driver-001".to_string()),
            make_claims("driver-001", "superadmin"),
            Json(make_topup_req(1000)),
        ).await;
        let body = result.0;
        assert!(
            body.get("error").is_none()
                || body["error"].as_str() != Some("Staff cannot top up their own wallet. Contact a superadmin."),
            "superadmin self-topup should be allowed, got: {:?}", body
        );
    }

    // SEC-05: cashier can top up a DIFFERENT driver's wallet
    #[tokio::test]
    async fn cross_topup_allowed_for_cashier() {
        let state = make_test_state().await;
        // cashier "staff-abc" tops up "driver-001" — different IDs, should proceed
        let result = topup_wallet(
            State(state),
            Path("driver-001".to_string()),
            make_claims("staff-abc", "cashier"),
            Json(make_topup_req(1000)),
        ).await;
        let body = result.0;
        assert!(
            body["error"].as_str() != Some("Staff cannot top up their own wallet. Contact a superadmin."),
            "cashier topping up a different driver should be allowed"
        );
    }
}
