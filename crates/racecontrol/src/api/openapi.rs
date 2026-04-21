// crates/racecontrol/src/api/openapi.rs
// Phase 445 Wave 1 skeleton -- Plan 02b fills in admin handler paths.
//
// CRITICAL (RESEARCH Pitfall 3): this module MUST NOT be referenced
// from the live server's Router in main.rs. utoipa-axum's OpenApiRouter
// is a different type than axum::Router; mixing them breaks route
// registration. The umbrella ApiDoc below is ONLY consumed by
// crates/racecontrol/src/bin/gen_types.rs (via ApiDoc::openapi()).
//
// Decision IDs: D-03 (gen-types binary), D-06 (admin-surface scope),
// Pitfall 3 (do NOT wire into live axum Router).

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Racing Point racecontrol API",
        version = "1.0.0",
        description = "Generated from utoipa annotations. Admin-surface subset (Phase 445 D-06).",
    ),
    tags(
        (name = "admin", description = "Admin panel surface"),
        (name = "fleet", description = "Fleet health + status"),
        (name = "billing", description = "Billing sessions + pricing"),
        (name = "games", description = "Game catalog + launch"),
        (name = "drivers", description = "Driver/customer management"),
    ),
)]
pub struct ApiDoc;

// Plan 02b will populate this with ~43 admin handler routes via
// utoipa_axum::OpenApiRouter::new().routes(routes!(...)). For Wave 1,
// the empty placeholder is sufficient to prove the build chain compiles
// with utoipa + utoipa-axum behind the gen-types feature gate.
pub fn admin_openapi_router_placeholder() -> &'static str {
    "Plan 02b fills admin routes here"
}
