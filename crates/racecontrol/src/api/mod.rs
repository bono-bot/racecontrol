pub mod debug_launches;
pub mod metrics;
pub mod metrics_prometheus;
pub mod metrics_query;
pub mod pods;
pub mod routes;
pub mod security;
pub mod survival;

// ─── Domain handler modules (Phase 380 split from routes.rs) ─────────────
pub mod accounting_handlers;
pub mod activity_routes;
pub mod admin_events;
pub mod admin_gamification;
pub mod admin_hr;
pub mod admin_tools;
pub mod ai_routes;
pub mod auth_handlers;
pub mod auth_staff;
pub mod billing_coupon;
pub mod billing_discount;
pub mod billing_invoice;
pub mod billing_session;
pub mod billing_shift;
pub mod billing_start;
pub mod billing_views;
pub mod bot_routes;
pub mod customer_auth;
pub mod customer_booking;
pub mod customer_legal;
pub mod customer_marketing;
pub mod customer_passport;
pub mod customer_register;
pub mod customer_session;
pub mod customer_social;
pub mod customer_wallet;
pub mod debug_system;
pub mod deploy_handlers;
pub mod driver_routes;
pub mod events_query;
pub mod game_ac;
pub mod game_launch;
pub mod game_state;
pub mod health_misc;
pub mod kiosk_config;
pub mod kiosk_handlers;
pub mod leaderboard_events;
pub mod leaderboard_public;
pub mod mesh_intelligence;
pub mod pod_exec;
pub mod pod_mgmt;
pub mod pod_queue;
pub mod pricing_routes;
pub mod psychology_routes;
pub mod pwa_game_request;
pub mod staff_crud;
pub mod sync_actions;
pub mod sync_cloud;
pub mod sync_failover;
pub mod terminal_handlers;
pub mod tournament_admin;
pub mod tournament_core;
pub mod waiver_routes;
pub mod wallet_ops;
pub mod wallet_staff;

use axum::Router;
use std::sync::Arc;

use crate::state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .nest("/api/v1", routes::api_routes(state.clone()))
        .with_state(state)
}
