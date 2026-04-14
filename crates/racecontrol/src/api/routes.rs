use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use super::metrics;
use super::metrics_prometheus;
use super::metrics_query;
use super::survival;
use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
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
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};


// ─── Domain module imports ────────────────────────────────────────────────
// Re-export items referenced by other crate modules (billing.rs, main.rs)
pub use super::billing_coupon::restore_coupon_on_cancel;
pub use super::customer_legal::spawn_data_retention_job;
pub use super::pod_queue::queue_expire_task;

use super::accounting_handlers::*;
use super::activity_routes::*;
use super::admin_events::*;
use super::admin_gamification::*;
use super::admin_hr::*;
use super::admin_tools::*;
use super::ai_routes::*;
use super::auth_handlers::*;
use super::auth_staff::*;
use super::billing_coupon::*;
use super::billing_discount::*;
use super::billing_invoice::*;
use super::billing_session::*;
use super::billing_shift::*;
use super::billing_start::*;
use super::billing_views::*;
use super::bot_routes::*;
use super::customer_auth::*;
use super::customer_booking::*;
use super::customer_legal::*;
use super::customer_marketing::*;
use super::customer_passport::*;
use super::customer_register::*;
use super::customer_session::*;
use super::customer_social::*;
use super::customer_wallet::*;
use super::debug_system::*;
use super::deploy_handlers::*;
use super::driver_routes::*;
use super::events_query::*;
use super::game_ac::*;
use super::game_launch::*;
use super::game_state::*;
use super::health_misc::*;
use super::kiosk_config::*;
use super::kiosk_handlers::*;
use super::leaderboard_events::*;
use super::leaderboard_public::*;
use super::mesh_intelligence::*;
use super::pod_exec::*;
use super::pod_mgmt::*;
use super::pod_queue::*;
use super::pricing_routes::*;
use super::psychology_routes::*;
use super::pwa_game_request::*;
use super::staff_crud::*;
use super::sync_actions::*;
use super::sync_cloud::*;
use super::sync_failover::*;
use super::terminal_handlers::*;
use super::tournament_admin::*;
use super::tournament_core::*;
use super::waiver_routes::*;
use super::wallet_ops::*;
use super::wallet_staff::*;

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
        .route("/auth/break-glass", post(auth::admin::break_glass))
        .layer(auth::rate_limit::auth_rate_limit_layer())
}

// ─── Tier 1: Public (no auth) ────────────────────────────────────────────

fn public_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/fleet/health", get(fleet_health::fleet_health_handler))
        .route("/sentry/crash", post(fleet_health::sentry_crash_handler))
        .route("/fleet/blocked-start", post(fleet_health::blocked_start_handler))
        .route("/telemetry/client-error", post(client_error_handler))
        // MMA-P1: Debug endpoints moved to staff_routes (below) to prevent information
        // disclosure. Kiosk debug UI must use staff JWT to access these endpoints.
        // Previously public: /debug/activity, /debug/playbooks, /debug/incidents, /debug/pod-events
        // MMA-v29: /debug/db-stats also moved to staff_routes (was leaking table names, row counts)
        .route("/guard/whitelist/{machine_id}", get(process_guard::get_whitelist_handler))
        .route("/venue", get(venue_info))
        .route("/venue/register", post(venue_register))
        .route("/customer/register", post(customer_register))
        .route("/wallet/bonus-tiers", get(wallet_bonus_tiers))
        .route("/wallet/topup-presets", get(wallet_topup_presets))
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
        // cameras/health is a simple ok/down proxy — no sensitive data. Needs to be public
        // because the portal page (/portal) fetches it without auth to show camera status dot.
        .route("/cameras/health", get(cameras_health_proxy))
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
        // Phase 288: Prometheus exposition format (PROM-01, PROM-02) — public, read-only metrics
        .route("/metrics/prometheus", get(metrics_prometheus::prometheus_handler))
        // Phase 298 PRESET-01: Preset reads are public (pods/kiosk need the list without JWT)
        .route("/presets", get(preset_library::list_presets))
        .route("/presets/{id}", get(preset_library::get_preset))
        // Phase 320 INV-03: Per-pod game inventory — public (kiosk fetches without JWT)
        .route("/fleet/pod-inventory/{pod_id}", get(pod_inventory_handler))
        // Phase 335: Spectator circuit viewer — public (TV display, no auth)
        .route("/spectator/tracks", get(spectator_list_tracks))
        .route("/spectator/track/{track_id}", get(spectator_get_track))
        .route("/spectator/positions", get(spectator_get_positions))
        // Kiosk experiences — read-only is public so standalone pod kiosk (/kiosk/pod/N)
        // can fetch the experience list without staff JWT. Write ops stay in kiosk_routes.
        // Same pattern as kiosk-allowlist, POS lockdown, presets, pod-inventory.
        .route("/kiosk/experiences", get(list_kiosk_experiences))
        // Games catalog — read-only is public so kiosk deep health + pod kiosk can fetch
        // the game list without staff JWT. Write ops (launch/stop/relaunch) stay in staff_routes.
        .route("/games/catalog", get(games_catalog))
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
        .route("/customer/racers", get(customer_list_racers).post(customer_add_racer))
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
        // GET /kiosk/experiences moved to public_routes (standalone pod kiosk needs it without JWT)
        .route("/kiosk/experiences", post(create_kiosk_experience))
        .route("/kiosk/experiences/{id}", get(get_kiosk_experience).put(update_kiosk_experience).delete(delete_kiosk_experience))
        .route("/kiosk/settings", get(get_kiosk_settings).put(update_kiosk_settings))
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
        // Phase 302: Event archive query API (system_events table — not /events which is hotlap competition)
        .route("/system-events", get(get_events))
        // MMA-P1: Debug endpoints moved from public_routes — require staff JWT
        .route("/debug/db-stats", get(debug_db_stats))
        .route("/debug/activity", get(debug_activity))
        .route("/debug/playbooks", get(debug_playbooks))
        .route("/debug/incidents", get(list_debug_incidents).post(create_debug_incident))
        .route("/debug/incidents/{id}", put(update_debug_incident))
        .route("/debug/incidents/{id}/apply-fix", post(debug_apply_fix))
        .route("/debug/diagnose", post(debug_diagnose))
        .route("/debug/pod-events/{pod_id}", get(debug_pod_events))
        // Phase 368: Launch status cards REST API (staff-JWT-gated via .layer below)
        .route(
            "/debug/launches/active",
            get(crate::api::debug_launches::debug_launches_active),
        )
        .route(
            "/debug/launches/{launch_id}/notes",
            get(crate::api::debug_launches::debug_launches_get_notes)
                .post(crate::api::debug_launches::debug_launches_post_note),
        )
        .route(
            "/debug/launches/{launch_id}/approve-fix",
            post(crate::api::debug_launches::debug_launches_approve_fix),
        )
        .route(
            "/debug/launches/{launch_id}/dismiss",
            post(crate::api::debug_launches::debug_launches_dismiss),
        )
        // Pods
        .route("/pods", get(list_pods).post(register_pod))
        .route("/pod-status-summary", get(pod_status_summary))
        // Phase 366 GLD-F-01/F-02: Fleet intelligence — composite health scores + time-of-day patterns
        .route("/fleet/intelligence", get(fleet_intelligence::fleet_intelligence_handler))
        .route("/pods/seed", post(seed_pods))
        .route("/pods/{id}", get(get_pod))
        // Phase 361-01: per-pod content inventory (games + cars + tracks) for
        // kiosk wizard filtering + admin drift detection. Staff JWT required
        // (info disclosure class — reveals fleet content posture).
        .route("/pods/{id}/inventory", get(crate::api::pods::pod_inventory_handler))
        // Phase 361-03: proxy rc-agent /debug/content-dirs with server-injected
        // pod service key. Admin browser never handles pod credentials.
        // Returns ContentDirsResponse (live disk scan for drift detection).
        .route("/debug/pod-content-dirs/{id}", get(crate::api::pods::pod_content_dirs_proxy_handler))
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
        // Act 2: Package upgrade (30→60 only, charges difference)
        .route("/billing/{id}/upgrade", post(upgrade_billing))
        // Act 3: Visit lifecycle + receipts
        .route("/visits/end/{id}", post(end_visit))
        .route("/billing/{id}/receipt", get(staff_session_receipt))
        .route("/visits/{id}/receipt", get(visit_receipt))
        // Act 3: Review/follow incentive tracking
        .route("/incentive/review/{driver_id}", get(track_review_click))
        .route("/incentive/follow/{driver_id}", get(track_follow_click))
        .route("/incentive/approve/{driver_id}", post(approve_incentive_bonus))
        // STAFF-01: Discount approval — cashier+ access, manager approval code required above threshold
        .route("/billing/{id}/discount", post(apply_billing_discount))
        .route("/billing/{id}/refund", post(refund_billing_session))
        .route("/billing/{id}/refunds", get(get_billing_refunds))
        // billing/report GET + billing/rates GET: staff-readable for POS dashboard.
        // Write operations (POST/PUT/DELETE rates) remain manager+ gated.
        .route("/billing/report/daily", get(daily_billing_report))
        .route("/billing/rates", get(list_billing_rates))
        // Feature flags: staff can read (POS obeys flags), only superadmin can write.
        .route("/flags", get(flags::list_flags))
        .route("/billing/split-options/{duration_minutes}", get(get_split_options))
        .route("/billing/continue-split", post(continue_split))
        // Game Launcher
        .route("/games/launch", post(launch_game))
        .route("/games/relaunch/{pod_id}", post(relaunch_game))
        .route("/games/stop", post(stop_game))
        // Games catalog GET moved to public_routes (kiosk deep health needs it without JWT)
        .route("/games/active", get(active_games))
        .route("/games/history", get(game_launch_history))
        .route("/launch-timeline/recent", get(get_recent_launch_timelines))
        .route("/launch-timeline/{launch_id}", get(get_launch_timeline))
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
        // Phase 334: Race weekend session progression
        .route("/games/weekend", post(create_weekend).get(list_active_weekends))
        .route("/games/weekend/{id}/status", get(get_weekend_status))
        .route("/games/weekend/{id}/stop", post(stop_weekend))
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
        .route("/wallet/{driver_id}/cash-refund", post(cash_refund_wallet))
        // Waivers (admin-facing)
        .route("/waivers", get(list_waivers))
        .route("/waivers/check", get(check_waiver))
        .route("/waivers/{driver_id}/signature", get(get_waiver_signature))
        // Guardian OTP (LEGAL-04/05) — staff sends + verifies guardian OTP for minor customers
        .route("/guardian/send-otp", post(send_guardian_otp_handler))
        .route("/guardian/verify-otp", post(verify_guardian_otp_handler))
        // Kiosk (admin-only: create/update/delete -- pod-accessible routes are in kiosk_routes())
        // kiosk experiences/settings — moved to role-gated admin section
        // Phase 298: Game preset library — write operations (read is in public_routes)
        .route("/presets", post(preset_library::create_preset))
        .route("/presets/{id}", put(preset_library::update_preset).delete(preset_library::delete_preset))
        // Config — write ops for kiosk allowlist (GET is in public_routes)
        .route("/config/kiosk-allowlist", post(add_kiosk_allowlist_entry))
        .route("/config/kiosk-allowlist/{name}", delete(delete_kiosk_allowlist_entry))
        // POS lockdown write (GET is in public_routes)
        .route("/pos/lockdown", post(set_pos_lockdown))
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
        .route("/staff/{id}/reset-pin", post(reset_staff_pin))
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
        .route("/metrics/launch-observability", get(metrics::launch_observability_handler))
        .route("/admin/launch-matrix", get(metrics::launch_matrix_handler))
        // DASH-01: Fleet game matrix — which games are installed on which pods
        .route("/fleet/game-matrix", get(game_matrix_handler))
        // DASH-02: Combo reliability list — sortable, flagged if success_rate < 70%
        .route("/admin/combo-list", get(metrics::combo_list_handler))
        // Phase 286: Metrics Query API (QAPI-01..05) — staff-only, business intelligence
        .route("/metrics/query", get(metrics_query::query_handler))
        .route("/metrics/names", get(metrics_query::names_handler))
        .route("/metrics/snapshot", get(metrics_query::snapshot_handler))
        .route("/mesh/solutions", get(mesh_list_solutions))
        .route("/mesh/solutions/search", get(mesh_search_solutions))
        .route("/mesh/solutions/{id}", get(mesh_get_solution))
        .route("/mesh/incidents", get(mesh_list_incidents))
        .route("/mesh/stats", get(mesh_stats))
        .route("/mesh/deploy-status", get(mesh_deploy_status))
        .route("/mesh/audit-check", get(mesh_audit_check))
        // cameras/health moved to public_routes — portal page needs it without JWT
        // Mesh Intelligence (v26.0) — staff write operations
        .route("/mesh/solutions/{id}/promote", post(mesh_promote_solution))
        .route("/mesh/solutions/{id}/retire", post(mesh_retire_solution))
        .route("/mesh/audit-seed", post(mesh_audit_seed))
        // ─── Model Evaluation Query (EVAL-03 / Phase 290) ────────────────────
        .route("/models/evaluations", get(list_model_evaluations))
        // ─── Model Reputation Query (MREP-04 / Phase 292) ────────────────────
        .route("/models/reputation", get(list_model_reputation))
        // ─── v29.0 Phase 9: Maintenance & Analytics ─────────────────────────
        .route("/maintenance/events", post(maintenance_create_event).get(maintenance_list_events))
        .route("/maintenance/summary", get(maintenance_summary))
        .route("/maintenance/tasks", post(maintenance_create_task).get(maintenance_list_tasks))
        .route("/maintenance/tasks/{id}", axum::routing::patch(maintenance_update_task))
        .route("/analytics/telemetry", get(analytics_telemetry))
        .route("/analytics/trends", get(analytics_trends))
        // ─── Phase 300-02: Backup Status (staff-only — backup health is internal data) ──
        .route("/backup/status", get(get_backup_status))
        // ─── Phase 299: Policy Rules Engine ──────────────────────────────────
        .route("/policy/rules", get(policy_engine::list_rules_handler).post(policy_engine::create_rule_handler))
        .route("/policy/rules/{id}", put(policy_engine::update_rule_handler).delete(policy_engine::delete_rule_handler))
        .route("/policy/eval-log", get(policy_engine::list_eval_log_handler))
        // Merge role-gated sub-routers (SEC-04: manager+, superadmin-only groups)
        .merge(
            // ── Manager+ routes ─────────────────────────────────────────────
            // Billing reports, financial accounting, audit log, rate management.
            // Cashiers cannot access financial reports or modify billing rates.
            Router::new()
                // GET /billing/report/daily and GET /billing/rates moved to staff routes for POS access.
                // Only write operations remain manager-gated here.
                .route("/billing/rates", post(create_billing_rate))
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
                // Phase 347-01: Safe staff PIN change (orchestrated cloud write + sync + dual verify)
                .route("/admin/staff/{id}/change-pin", post(change_staff_pin_safe))
                // Phase 347-01: On-demand filtered cloud pull
                .route("/admin/sync/pull-now", post(sync_pull_now_handler))
                // Phase 365: Manual trigger for AI behavior MMA batch
                .route("/admin/ai-behavior-batch/run", post(ai_behavior_batch_trigger))
                // Phase 367-01: Suspect sessions list (GLD-G-01)
                .route("/admin/suspect-sessions", get(list_suspect_sessions_handler))
                // Phase 367-01: Session telemetry heatmap drill-down (GLD-G-01)
                .route("/admin/sessions/{id}/telemetry-heatmap", get(session_telemetry_heatmap_handler))
                // Phase 367-02: On-demand pod verify (GLD-G-02)
                .route("/admin/pods/{pod_id}/verify", post(admin_verify_pod_handler))
                // Phase 367-03: Session replay (GLD-G-03)
                .route("/admin/sessions/{id}/replay", get(session_replay_handler))
                // Phase 367-04: Batch export estimate + export (GLD-G-04)
                .route("/admin/export/estimate", get(admin_export_estimate_handler))
                .route("/admin/export", get(admin_export_handler))
                .layer(axum::middleware::from_fn(require_role_manager))
        )
        .merge(
            // ── Superadmin-only routes ──────────────────────────────────────
            // System config, feature flags, deploy pipeline, OTA, pipeline config.
            // Managers cannot change system configuration.
            Router::new()
                // GET /flags moved to staff routes for POS access.
                // Only write operations remain superadmin-gated here.
                .route("/flags", post(flags::create_flag))
                .route("/flags/{name}", put(flags::update_flag))
                .route("/config/push", post(config_push::push_config))
                .route("/config/push/queue", get(config_push::get_queue))
                .route("/config/audit", get(config_push::get_audit_log))
                // Phase 296 PUSH-01/PUSH-02: Full AgentConfig per-pod storage + push
                .route("/config/pod/{pod_id}", get(config_push::get_pod_config_handler).post(config_push::set_pod_config))
                .route("/deploy/status", get(deploy_status))
                .route("/deploy/rolling", post(deploy_rolling_handler))
                .route("/deploy/{pod_id}", post(deploy_single_pod))
                .route("/ota/deploy", post(ota_deploy_handler))
                .route("/ota/status", get(ota_status_handler))
                .route("/pipeline/config", get(pipeline_config_get).post(pipeline_config_set))
                // Phase 367-05: Synthetic config mismatch test endpoint (GLD-G-05)
                .route("/internal/test/config-mismatch", post(internal_test_config_mismatch_handler))
                // Phase 304: Fleet deploy automation (canary-first, billing-drain, auto-rollback)
                .route("/fleet/deploy", post(fleet_deploy_handler))
                .route("/fleet/deploy/status", get(fleet_deploy_status_handler))
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
        // Audit seed via service key (CGP 4.1: smart pipes feed MI without staff JWT)
        .route("/mesh/audit-seed-service", post(mesh_audit_seed_service))
        // GLD-C-03: CSV telemetry fallback from rc-agent at session end (D-07/D-09)
        // Auth: X-Service-Key (sentry_service_key). Max body: 50MB.
        .route(
            "/sessions/{id}/telemetry-fallback",
            post(telemetry_fallback_handler)
                .layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
}

pub(crate) const BUILD_ID: &str = env!("GIT_HASH");





/// RCA-PREVENTION: Static route uniqueness check.
/// Extracts all .route() registrations from this file and asserts no METHOD+PATH duplicates.
/// This catches the class of bug that caused the 2026-03-29 deploy failure
/// (21 duplicate routes from Phase 258 move-without-delete).








/// Act 3: Track review link click — redirects to Google Maps, logs click.

// ─── Invoice (LEGAL-02) ──────────────────────────────────────────────────────




// ─── AC LAN ──────────────────────────────────────────────────────────────────


















// ─── Phase 365: AI Behavior MMA Batch Trigger ────────────────────────────

/// POST /api/v1/admin/ai-behavior-batch/run
/// Manually triggers one MMA batch cycle (normally runs weekly).

// ─── Staff Gamification (v14.0 Phase 95) ─────────────────────────────────


// ─── Friends ──────────────────────────────────────────────────────────────


// ─── Shareable Session Report ────────────────────────────────────────────────






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


// ═══════════════════════════════════════════════════════════════════════════════





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








/// POST /api/v1/customer/game-request
///
/// Customer requests a game launch from the PWA. Validates that the pod
/// exists and the game is installed, then broadcasts GameLaunchRequested
/// to the staff dashboard. Staff confirms via POST /api/v1/games/pod/{id}/launch.
///
/// Note: customer auth uses extract_driver_id() (customer JWT). Customer auth
/// middleware is in-handler (Phase 82+ may promote to tower middleware).

// ─── Psychology handlers ──────────────────────────────────────────────────────


// ─── DPDP Act: Customer Data Rights (Plan 79-03) ────────────────────────────


/// Shared PII anonymization logic for both customer- and staff-initiated consent revocation.
///
/// Anonymizes all PII fields on the drivers row and sets consent_revoked = 1.
/// The driver row is retained so billing_sessions.driver_id foreign keys remain valid.
/// Financial records (journal_entries, invoices, billing_sessions, wallet_transactions)
/// are NOT touched — retained for 8 years per the Income Tax Act.
pub(crate) async fn anonymize_driver_pii(
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

// ─── FATM-12: Reconciliation handlers ───────────────────────────────────────





// ─── Fleet Game Matrix (DASH-01) ─────────────────────────────────────────────
//
// Returns which games are installed on which pods, sourced from pod_game_inventory.
// Response: { games: [{ game_id, display_name, sim_type, pods: { pod_id: { installed, launchable, scanned_at } } }] }

// ─── Phase 320: Pod Inventory (INV-03, COMBO-05) ────────────────────────────

/// Convert the Rust Debug format of SimType (e.g. "AssettoCorsa") to snake_case API string.


// ─── SEC-05: Self-topup guard tests ──────────────────────────────────────────


/// Phase 318 (LAUNCH-05): launch-timeline endpoint tests.

// ─── DASH-01: Game matrix tests ────────────────────────────────────────────────


// ─── Phase 320: Pod Inventory tests (INV-03, COMBO-05) ───────────────────────


// ─── GLD-C-03: Telemetry Fallback Handler Tests ───────────────────────────────


#[cfg(test)]
#[path = "routes_tests.rs"]
mod routes_tests;
