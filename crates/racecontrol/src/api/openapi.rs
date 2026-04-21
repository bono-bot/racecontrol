// crates/racecontrol/src/api/openapi.rs
// Phase 445 Wave 2b -- ApiDoc umbrella referencing 42 admin handler paths.
//
// CRITICAL (RESEARCH Pitfall 3): this module MUST NOT be referenced
// from the live server's Router in main.rs. utoipa-axum's OpenApiRouter
// is a different type than axum::Router; mixing them breaks route
// registration. The umbrella ApiDoc below is ONLY consumed by
// crates/racecontrol/src/bin/gen_types.rs (via ApiDoc::openapi()).
//
// Decision IDs: D-01 (utoipa umbrella), D-03 (gen-types binary), D-06
// (admin-surface scope), Pitfall 3 (do NOT wire into live axum Router),
// Pitfall 6 (determinism gate).

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Racing Point racecontrol API",
        version = "1.0.0",
        description = "Generated from utoipa annotations. Admin-surface subset (Phase 445 D-06).",
    ),
    tags(
        (name = "admin",         description = "Admin panel surface"),
        (name = "fleet",         description = "Fleet health + status"),
        (name = "billing",       description = "Billing sessions + pricing"),
        (name = "games",         description = "Game catalog + launch"),
        (name = "drivers",       description = "Driver/customer management"),
        (name = "pods",          description = "Pod operations"),
        (name = "staff",         description = "Staff + gamification"),
        (name = "deploy",        description = "Deploy + rolling updates"),
        (name = "mesh",          description = "Mesh intelligence stats"),
        (name = "wallet",        description = "Wallet bonus tiers + topup presets"),
        (name = "kiosk",         description = "Kiosk experiences + settings"),
        (name = "tournaments",   description = "Tournaments"),
        (name = "customer",      description = "Customer packages + membership"),
        (name = "coupons",       description = "Coupons"),
        (name = "presets",       description = "Pricing presets"),
        (name = "cafe",          description = "Cafe promos"),
        (name = "hr",            description = "HR recognition"),
        (name = "time_trials",   description = "Time trials"),
        (name = "config",        description = "Config audit + push"),
        (name = "ai_chat",       description = "AI chat"),
        (name = "pricing",       description = "Pricing tiers + rules"),
        (name = "activity",      description = "Activity log"),
    ),
    paths(
        // Fleet
        crate::fleet_health_api::fleet_health_handler,
        // Billing
        crate::api::billing_views::active_billing_sessions,
        crate::api::billing_start::start_billing,
        crate::api::pricing_billing_rates::list_billing_rates,
        // Pricing
        crate::api::pricing_routes::list_pricing_tiers,
        crate::api::tournament_admin::list_pricing_rules,
        // Games
        crate::api::game_launch::launch_game,
        crate::api::game_launch::stop_game,
        crate::api::game_launch::games_catalog,
        crate::api::game_state::active_games,
        // Drivers / Pods / Staff
        crate::api::driver_routes::list_drivers,
        crate::api::pod_mgmt::list_pods,
        crate::api::staff_crud::list_staff,
        // Pod bulk ops
        crate::api::pod_mgmt_bulk::wake_all_pods,
        crate::api::pod_mgmt_bulk::shutdown_all_pods,
        crate::api::pod_mgmt_bulk::restart_all_pods,
        crate::api::pod_mgmt_bulk::lockdown_all_pods,
        // Staff gamification
        crate::api::admin_gamification::staff_gamification_leaderboard,
        crate::api::admin_gamification::staff_kudos_list,
        crate::api::admin_gamification::staff_challenges_list,
        // Deploy
        crate::api::deploy_handlers::deploy_single_pod,
        crate::api::deploy_handlers::deploy_status,
        crate::api::deploy_handlers::deploy_rolling_handler,
        // Mesh
        crate::api::mesh_intelligence::mesh_stats,
        // Wallet
        crate::api::wallet_staff::wallet_bonus_tiers,
        crate::api::wallet_staff::wallet_topup_presets,
        // Kiosk
        crate::api::kiosk_handlers::list_kiosk_experiences,
        crate::api::kiosk_handlers::get_kiosk_settings,
        // Tournaments / Time trials / Coupons
        crate::api::tournament_core::list_tournaments,
        crate::api::tournament_timetrial::list_time_trials,
        crate::api::tournament_admin::list_coupons,
        // Customer
        // customer_list_packages lives in customer_referral.rs, a pub(crate) #[path]
        // submodule of customer_marketing. utoipa's `__path_<fn>` sibling struct is
        // generated next to the original fn, so we must reach through the nested mod.
        crate::api::customer_marketing::customer_referral::customer_list_packages,
        crate::api::customer_marketing::customer_membership,
        // Presets
        crate::preset_library::list_presets,
        // Cafe
        crate::cafe_promos::list_cafe_promos,
        // HR
        crate::api::admin_hr::hr_recognition_data,
        // Config push
        crate::config_push::config_push_handlers::push_config,
        crate::config_push::config_push_handlers::get_audit_log,
        crate::config_push::config_push_full::get_pod_config_handler,
        crate::api::kiosk_config::list_kiosk_allowlist,
        // AI chat
        crate::api::ai_routes::ai_chat,
        // Activity
        crate::api::activity_routes::global_activity,
    ),
)]
pub struct ApiDoc;
