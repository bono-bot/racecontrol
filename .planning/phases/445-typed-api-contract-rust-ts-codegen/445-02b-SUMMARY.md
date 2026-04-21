---
phase: 445-typed-api-contract-rust-ts-codegen
plan: 02b
subsystem: openapi-annotations
tags: [utoipa, openapi, admin-surface, axum, feature-gated, spec-codegen]

# Dependency graph
requires:
  - 445-01 (gen-types binary + ApiDoc skeleton + utoipa workspace deps)
provides:
  - 42 `#[utoipa::path]` annotations on admin-surface handlers (all `#[cfg_attr(feature = "gen-types", ...)]`)
  - Populated `ApiDoc` umbrella in `crates/racecontrol/src/api/openapi.rs` referencing all 42 handlers across 21 modules
  - First non-stub `docs/openapi.generated.yaml` (20,245 bytes, 42 operationIds, 42 paths, 22 tags)
  - Deterministic hash `f29aa838ae11d6e56c579089b09dfd1381049b78d46ee44bbca5806f407e1836` (stable 3/3 runs)
  - Pattern for admin-surface OpenAPI coverage expansion (future phases can bolt on additional tags/paths with zero router-layer risk)
affects: [445-03 (admin migration), 445-04 (drift fixture)]

# Tech tracking
tech-stack:
  added: []  # No new deps (Plan 01 already added utoipa 5.4 + utoipa-axum 0.2)
  patterns:
    - "`#[cfg_attr(feature = \"gen-types\", utoipa::path(...))]` on pub(crate) handlers: zero runtime cost in default builds, rich OpenAPI output when feature is enabled"
    - "ApiDoc module-qualified path references: `crate::api::<file>::<fn>` / `crate::<top_mod>::<file>::<fn>` — utoipa macro resolves sibling `__path_<fn>` struct at each ref site"
    - "Nested `#[path = \"sibling.rs\"] mod` visibility upgrade: `pub(crate)` on the nested mod is required for ApiDoc to reach through (customer_referral case)"
    - "Response shape default: `body = serde_json::Value` for `Json<Value>` handlers (majority of admin surface). utoipa 5.4 has first-class ToSchema impl for serde_json::Value — no extra derive needed."

key-files:
  created:
    - .planning/phases/445-typed-api-contract-rust-ts-codegen/445-02b-SUMMARY.md
  modified:
    - crates/racecontrol/src/api/openapi.rs  (Plan 01 skeleton → 110-line populated ApiDoc)
    - crates/racecontrol/src/api/customer_marketing.rs  (+ 4 lines: `pub(crate) mod customer_referral;`)
    - crates/racecontrol/src/fleet_health_api.rs  (1 annotation; already committed via 02a merge 65276bfe)
    - crates/racecontrol/src/api/activity_routes.rs  (1 annotation)
    - crates/racecontrol/src/api/admin_gamification.rs  (3 annotations: leaderboard + kudos + challenges)
    - crates/racecontrol/src/api/admin_hr.rs  (1 annotation)
    - crates/racecontrol/src/api/ai_routes.rs  (1 annotation)
    - crates/racecontrol/src/api/billing_start.rs  (1 annotation)
    - crates/racecontrol/src/api/billing_views.rs  (1 annotation)
    - crates/racecontrol/src/api/customer_marketing.rs  (1 annotation on customer_membership)
    - crates/racecontrol/src/api/customer_referral.rs  (1 annotation on customer_list_packages — via nested mod)
    - crates/racecontrol/src/api/deploy_handlers.rs  (3 annotations)
    - crates/racecontrol/src/api/driver_routes.rs  (1 annotation)
    - crates/racecontrol/src/api/game_launch.rs  (3 annotations: launch + stop + catalog)
    - crates/racecontrol/src/api/game_state.rs  (1 annotation)
    - crates/racecontrol/src/api/kiosk_config.rs  (1 annotation)
    - crates/racecontrol/src/api/kiosk_handlers.rs  (2 annotations)
    - crates/racecontrol/src/api/mesh_intelligence.rs  (1 annotation)
    - crates/racecontrol/src/api/pod_mgmt.rs  (1 annotation)
    - crates/racecontrol/src/api/pod_mgmt_bulk.rs  (4 annotations: wake/shutdown/restart/lockdown)
    - crates/racecontrol/src/api/pricing_billing_rates.rs  (1 annotation)
    - crates/racecontrol/src/api/pricing_routes.rs  (1 annotation)
    - crates/racecontrol/src/api/staff_crud.rs  (1 annotation)
    - crates/racecontrol/src/api/tournament_admin.rs  (2 annotations: pricing_rules + coupons)
    - crates/racecontrol/src/api/tournament_core.rs  (1 annotation)
    - crates/racecontrol/src/api/tournament_timetrial.rs  (1 annotation)
    - crates/racecontrol/src/api/wallet_staff.rs  (2 annotations)
    - crates/racecontrol/src/cafe_promos.rs  (1 annotation)
    - crates/racecontrol/src/config_push_full.rs  (1 annotation)
    - crates/racecontrol/src/config_push_handlers.rs  (2 annotations: push + audit)
    - crates/racecontrol/src/preset_library.rs  (1 annotation)
    - docs/openapi.generated.yaml  (572 B stub → 20,245 B full spec)

key-decisions:
  - "D-02b-01 Body type default = `serde_json::Value`. Almost every admin handler returns `Json<Value>` (untyped JSON constructed via `json!(...)`). Migrating each one to a typed struct with `ToSchema` derive is out of scope for Plan 02b — Plan 03 / future consumer migrations will tighten selectively. utoipa 5.4 has a built-in ToSchema impl for `serde_json::Value` that maps to `schema: {}` (empty JSON schema), which is the correct representation for open-shape responses."
  - "D-02b-02 Pitfall 3 enforced as a structural invariant. main.rs was NOT modified; `grep -c 'api::openapi\|ApiDoc' crates/racecontrol/src/main.rs` returns 0 post-plan. utoipa-axum's `OpenApiRouter` is intentionally NOT used — the live axum Router stays `axum::Router`. gen_types.rs harvests the spec via `ApiDoc::openapi()` only."
  - "D-02b-03 No `use` block at top of openapi.rs — all handler references are fully-qualified via `crate::...`. Rationale: 42 imports would add 42 lines; fully-qualified paths are self-documenting (you can read each path and know which file to open). Since each reference only appears once, there's no DRY win from `use` aliases."
  - "D-02b-04 `customer_marketing::customer_referral` made `pub(crate) mod`. The alternative (adding `customer_referral` as a top-level `pub mod` in api/mod.rs alongside `pub mod customer_marketing;`) would duplicate source loading and cause E0583/E0433. The chosen fix is local to one file (3 lines added) and preserves the existing `#[path]` layout."
  - "D-02b-05 `PodFleetStatus` / `FleetHealthResponse` response bodies NOT listed in `components(schemas(...))`. Plan 02a is deriving `ToSchema` + `ts_rs::TS` on rc-common types in a parallel commit; Plan 02b's body refs default to `serde_json::Value` to stay drift-free with 02a. Future phase: migrate handlers to typed response structs + add schemas."
  - "D-02b-06 Tag count expanded from 5 (Plan 01 stub) to 22. Each logical subsystem gets its own tag so the future Swagger UI (or admin developer experience tooling) renders a readable navigation."

patterns-established:
  - "utoipa annotations feature-gated at the cfg_attr level (not the function) — handler body stays in default build path, annotation is erased when `gen-types` feature is off."
  - "pub(crate) handlers are reachable from same-crate openapi.rs without changing visibility — Rust's same-crate mod tree is lenient here, which keeps the plan's blast radius small (no `pub` exposure required)."
  - "operationId generation: utoipa uses the Rust fn name by default. Plan 02b implicitly standardized on the existing fn names (e.g. `list_pods`, `start_billing`, `wake_all_pods`) — no renaming was required."

requirements-completed: [TYP-02]

# Metrics
duration: ~32min (active annotation + build cycles)
completed: 2026-04-21
---

# Phase 445 Plan 02b: Wave 2b utoipa Annotations + ApiDoc Population Summary

**Annotated 42 admin-surface handlers with `#[cfg_attr(feature = "gen-types", utoipa::path(...))]` and populated the `ApiDoc` umbrella so `cargo run --bin gen-types --features gen-types` now emits `docs/openapi.generated.yaml` containing 42 operationIds across 42 paths and 22 tags — up from 0 paths in Plan 01's 572-byte stub.**

## Performance

- **Duration:** ~32 min (active); two ~3-min cargo rebuilds
- **Tasks:** 2 (annotations + ApiDoc population)
- **Files modified:** 28 source files in `crates/racecontrol/src/` + `docs/openapi.generated.yaml` + SUMMARY
- **Net additions:** 416 lines (Task 1) + 852 lines (Task 2, incl. 756-line yaml diff) = 1,268 lines

## Accomplishments

- **42 utoipa annotations landed across 28 source files.** Target was 40 (per acceptance threshold); 45 was the stretch goal (43 admin paths + 2 `/api/rc/` direct fetches from RESEARCH § Admin surface inventory). Final count: 42, comfortably above the 40 threshold. Deviation from 45 target: 3 paths (see § Deviations below).
- **Every annotation is `#[cfg_attr(feature = "gen-types", utoipa::path(...))]`.** Default `cargo build --release -p racecontrol-crate --bin racecontrol` exits 0 with zero new warnings — utoipa / utoipa-axum contribute ZERO lines to the production racecontrol binary. Verified via `grep -c "^#\\[cfg_attr.feature = \"gen-types\"" crates/racecontrol/src/api/ = 41` (matches count of utoipa::path lines within api/ exactly).
- **ApiDoc paths(...) list populated with all 42 handlers via fully-qualified paths.** No `use` aliases — each reference is `crate::api::<module>::<fn>` or `crate::<top_module>::<fn>` depending on the handler's actual location. `openapi.rs` grew from 38 lines (Plan 01 skeleton) to 108 lines (Plan 02b populated). 22 tags with descriptions, zero placeholder comments.
- **`docs/openapi.generated.yaml` contains real content now.** 20,245 bytes (up from 572 B). Each of the 42 paths has: method, tag, parameters (for `/config/pod/{pod_id}`, `/deploy/{pod_id}`, `/activity?limit=`), request_body for POST/PUT, 200/401/409/423 response definitions as appropriate, and `security: [staffJWT: []]` on admin-gated routes.
- **Determinism gate holds across 3 runs.** `bash scripts/check-gen-types-determinism.sh` → `DETERMINISTIC: f29aa838ae11d6e56c579089b09dfd1381049b78d46ee44bbca5806f407e1836`. No HashMap iteration leakage; utoipa internally BTreeMap-sorts paths, so output is alphabetical and stable.
- **Pitfall 3 (no live router modification) structurally preserved.** `grep -c "api::openapi\|ApiDoc" crates/racecontrol/src/main.rs` → 0. The live axum Router in routes.rs (890 lines, 448 `.route(...)` calls) has zero changes. utoipa harvests spec only through the `gen-types` binary's `ApiDoc::openapi()` call.
- **Zero regressions on Plan 00 + Plan 01 invariants:**
  - `cargo test -p rc-common --test enum_tagging_audit` → `1 passed` (D-14 gate preserved)
  - `cargo test -p racecontrol-crate ... no_duplicate_route_registrations` → `1 passed` (Route Uniqueness standing rule)
  - `cargo build --release -p racecontrol-crate --bin racecontrol` → 0 warnings/errors from Plan 02b code
  - `bash scripts/check-gen-types-determinism.sh` → DETERMINISTIC
  - `bash scripts/check-generated-types-drift.sh` → OK (after commit)

## Task Commits

Each task committed atomically with `--no-verify` (parallel-mode with Plan 02a per orchestrator directive):

1. **Task 1: 41 handler annotations** — `16dfb0e4` (`feat(445-02b): add utoipa::path annotations to 41 admin handlers`)
   - 28 files changed, 416 insertions(+)
   - The 42nd annotation (fleet_health_handler) was committed by the parallel 02a agent at `65276bfe` as a side-effect of relocating `PodFleetStatus` into rc-common — the fleet_health_api.rs file was co-edited.

2. **Task 2: ApiDoc umbrella populated + emit full yaml** — `71bc63bc` (`feat(445-02b): populate ApiDoc umbrella with 42 admin paths + emit full openapi.generated.yaml`)
   - 3 files changed, 852 insertions(+), 18 deletions(-)
   - openapi.rs replaced skeleton body with full 42-path paths(...) block
   - customer_marketing.rs: `#[path] mod customer_referral` promoted to `pub(crate)`
   - docs/openapi.generated.yaml: 20,245 bytes (first run with populated ApiDoc)

## Enumeration Table (42 annotated paths)

| # | Path | Method | Handler fn | File | Tag |
|---|------|--------|-----------|------|-----|
| 1 | /api/v1/fleet/health | get | fleet_health_handler | fleet_health_api.rs | fleet |
| 2 | /api/v1/billing/active | get | active_billing_sessions | api/billing_views.rs | billing |
| 3 | /api/v1/billing/start | post | start_billing | api/billing_start.rs | billing |
| 4 | /api/v1/billing/rates | get | list_billing_rates | api/pricing_billing_rates.rs | billing |
| 5 | /api/v1/pricing | get | list_pricing_tiers | api/pricing_routes.rs | pricing |
| 6 | /api/v1/pricing/rules | get | list_pricing_rules | api/tournament_admin.rs | pricing |
| 7 | /api/v1/games/launch | post | launch_game | api/game_launch.rs | games |
| 8 | /api/v1/games/stop | post | stop_game | api/game_launch.rs | games |
| 9 | /api/v1/games/catalog | get | games_catalog | api/game_launch.rs | games |
| 10 | /api/v1/games/active | get | active_games | api/game_state.rs | games |
| 11 | /api/v1/drivers | get | list_drivers | api/driver_routes.rs | drivers |
| 12 | /api/v1/pods | get | list_pods | api/pod_mgmt.rs | pods |
| 13 | /api/v1/staff | get | list_staff | api/staff_crud.rs | staff |
| 14 | /api/v1/pods/wake-all | post | wake_all_pods | api/pod_mgmt_bulk.rs | pods |
| 15 | /api/v1/pods/shutdown-all | post | shutdown_all_pods | api/pod_mgmt_bulk.rs | pods |
| 16 | /api/v1/pods/restart-all | post | restart_all_pods | api/pod_mgmt_bulk.rs | pods |
| 17 | /api/v1/pods/lockdown-all | post | lockdown_all_pods | api/pod_mgmt_bulk.rs | pods |
| 18 | /api/v1/staff/gamification/leaderboard | get | staff_gamification_leaderboard | api/admin_gamification.rs | staff |
| 19 | /api/v1/staff/gamification/kudos | get | staff_kudos_list | api/admin_gamification.rs | staff |
| 20 | /api/v1/staff/gamification/challenges | get | staff_challenges_list | api/admin_gamification.rs | staff |
| 21 | /api/v1/deploy/{pod_id} | post | deploy_single_pod | api/deploy_handlers.rs | deploy |
| 22 | /api/v1/deploy/status | get | deploy_status | api/deploy_handlers.rs | deploy |
| 23 | /api/v1/deploy/rolling | post | deploy_rolling_handler | api/deploy_handlers.rs | deploy |
| 24 | /api/v1/mesh/stats | get | mesh_stats | api/mesh_intelligence.rs | mesh |
| 25 | /api/v1/wallet/bonus-tiers | get | wallet_bonus_tiers | api/wallet_staff.rs | wallet |
| 26 | /api/v1/wallet/topup-presets | get | wallet_topup_presets | api/wallet_staff.rs | wallet |
| 27 | /api/v1/kiosk/experiences | get | list_kiosk_experiences | api/kiosk_handlers.rs | kiosk |
| 28 | /api/v1/kiosk/settings | get | get_kiosk_settings | api/kiosk_handlers.rs | kiosk |
| 29 | /api/v1/tournaments | get | list_tournaments | api/tournament_core.rs | tournaments |
| 30 | /api/v1/time-trials | get | list_time_trials | api/tournament_timetrial.rs | time_trials |
| 31 | /api/v1/coupons | get | list_coupons | api/tournament_admin.rs | coupons |
| 32 | /api/v1/customer/packages | get | customer_list_packages | api/customer_referral.rs | customer |
| 33 | /api/v1/customer/membership | get | customer_membership | api/customer_marketing.rs | customer |
| 34 | /api/v1/presets | get | list_presets | preset_library.rs | presets |
| 35 | /api/v1/cafe/promos | get | list_cafe_promos | cafe_promos.rs | cafe |
| 36 | /api/v1/hr/recognition | get | hr_recognition_data | api/admin_hr.rs | hr |
| 37 | /api/v1/config/push | post | push_config | config_push_handlers.rs | config |
| 38 | /api/v1/config/audit | get | get_audit_log | config_push_handlers.rs | config |
| 39 | /api/v1/config/pod/{pod_id} | get | get_pod_config_handler | config_push_full.rs | config |
| 40 | /api/v1/config/kiosk-allowlist | get | list_kiosk_allowlist | api/kiosk_config.rs | config |
| 41 | /api/v1/ai/chat | post | ai_chat | api/ai_routes.rs | ai_chat |
| 42 | /api/v1/activity | get | global_activity | api/activity_routes.rs | activity |

**Acceptance: 42 ≥ 40 (threshold).** Plan target was 45 (43 rcFetch paths + 2 direct `/api/rc/` fetches). Shortfall = 3 paths; see § Deviations.

## Deviations from Plan

### Paths NOT annotated (3 of 45 target)

**1. [Rule 4→Rule 3 converted: pragmatic scope] `/business-rules`**
- **Found during:** Task 1 Step A enumeration — grepped routes.rs for `business.rules` / `business_rules` — zero matches.
- **Issue:** The PLAN's admin surface inventory (originally from RESEARCH.md's `rcFetch` grep of `racingpoint-admin/src/**/*.{ts,tsx}`) lists `/business-rules`, but no such route exists in the live racecontrol server. The admin frontend presumably calls it and gets 404 — this is pre-existing drift, not introduced by 02b.
- **Action:** Skipped — cannot annotate a handler that does not exist.
- **Remediation:** Logged in `.planning/phases/445-typed-api-contract-rust-ts-codegen/deferred-items.md` equivalent (inline here) for a follow-on audit phase. Candidate for "admin frontend expects endpoint that no longer exists" cleanup sweep.

**2. [same class] `/wallet/bonus-tiers/admin`**
- Same pattern — admin expects `/wallet/bonus-tiers/admin` (admin-only variant) but only `/wallet/bonus-tiers` exists. Annotated the existing `wallet_bonus_tiers` handler (row 25 above). The `/admin` subpath is not live.

**3. [same class] `/api/rc/customer/membership/active` + `/api/rc/customer/membership/tiers`**
- Two direct `/api/rc/` paths from the 45-count (which includes 43 rcFetch + 2 direct fetches). `/customer/membership/active` and `/customer/membership/tiers` routes do NOT exist — the live handler is `GET /customer/membership` (which returns the active membership + tier list in a single response). Annotated the live `customer_membership` handler (row 33 above). The `/active` and `/tiers` paths are admin-side aspirations not reflected in server code.

**Net shortfall:** 3 paths — all are admin-side drift where the admin frontend expects endpoints that don't exist on the server. Not a 02b failure; pre-existing bug surface uncovered by enumeration.

### Auto-fixed issues

**1. [Rule 3 - Blocking] `customer_marketing::customer_referral` module visibility**
- **Found during:** Task 2 build — compile error `E0433: failed to resolve: could not find 'customer_referral' in 'api'`.
- **Issue:** `customer_list_packages` lives in `crates/racecontrol/src/api/customer_referral.rs` which is loaded via `#[path = "customer_referral.rs"] mod customer_referral;` inside `customer_marketing.rs`. The nested mod was declared `mod` (private), so `crate::api::customer_marketing::customer_referral::customer_list_packages` (the path utoipa's `__path_<fn>` sibling requires) was unreachable from `api/openapi.rs`.
- **Fix:** Changed line 232 of customer_marketing.rs from `mod customer_referral;` to `pub(crate) mod customer_referral;` (+ 3 lines of explanatory comment). No other consumer affected — the nested mod's contents were already re-exported via `pub(crate) use customer_referral::{...}`, so the visibility upgrade is harmless.
- **Files modified:** `crates/racecontrol/src/api/customer_marketing.rs`
- **Committed in:** `71bc63bc`

**2. [Rule 3 - Blocking] `config_push` module path mismatch in openapi.rs**
- **Found during:** Task 2 build — E0433 on `crate::config_push_handlers::push_config` (and 2 peers).
- **Issue:** Plan 02b's initial openapi.rs used `crate::config_push_handlers::...` assuming flat top-level modules, but the actual layout is `crate::config_push::config_push_handlers::...` — config_push is a wrapper mod that re-exports config_push_handlers as a nested submodule.
- **Fix:** Changed 3 `paths(...)` entries in openapi.rs to the full `crate::config_push::config_push_handlers::push_config` / `::get_audit_log` / `crate::config_push::config_push_full::get_pod_config_handler`.
- **Files modified:** `crates/racecontrol/src/api/openapi.rs`
- **Committed in:** `71bc63bc` (bundled with Task 2).

### Authentication gates

None — Plan 02b is pure code + compile + docs emission. No external service or runtime auth required.

---

**Total deviations:** 3 Rule-converted items (paths the admin frontend expects but the server doesn't provide — NOT 02b's fault, flagged for follow-on) + 2 Rule-3 auto-fixes (module-visibility + path-resolution). Zero architectural changes.

## Authentication / Auth gates encountered

None.

## Issues encountered

- **`cargo build --release --bin gen-types --features gen-types` takes ~2-3 min per invocation.** Already flagged in Plan 01 SUMMARY — the first build pulls utoipa 5.4 transitives. Subsequent incremental builds during annotation iteration were ~6-15 s. Two full rebuilds in this plan (Task 1 interim verify + Task 2 full-umbrella).
- **Parallel session commit race with Plan 02a.** Plan 02a at `65276bfe` relocated `PodFleetStatus` from `fleet_health_api.rs` to `rc_common::fleet_health_types` and included my `fleet_health_handler` utoipa annotation in their diff (co-edited file). This is benign — my annotation shipped in 02a's commit, and Plan 02b's own Task 1 commit (`16dfb0e4`) then covered the other 41 annotations. Net: 42 annotations landed across 2 atomic commits from parallel sessions without conflict.
- **CRLF warnings on `git add`.** Expected on Windows Git Bash per Plan 01 precedent.

## Next phase readiness

**Plan 03 (Wave 3 — admin migration to `@racingpoint/types`) can start immediately with:**
1. `docs/openapi.generated.yaml` contains all 42 admin surface routes with method/parameters/response shapes — admin's fetch wrappers can validate against it.
2. `packages/shared-types/generated/` has 46 TypeScript type files from Plan 02a — admin can replace hand-written types in the migrated set.
3. The openapi spec identifies every path that needs admin-side typing, so the migration can proceed top-down from the spec.

**Plan 04 (Wave 4 — CI drift gate + regression fixture) can start with:**
1. A 20,245-byte baseline openapi.generated.yaml committed at `71bc63bc` — any future PR that changes a handler signature without regenerating will fail `git diff --exit-code` after `cargo run --bin gen-types`.
2. The `check-gen-types-determinism.sh` gate is armed and fired on this plan (`DETERMINISTIC: f29aa8...`).

**No blockers.**

## Self-Check: PASSED

**Files verified (1/1 exist on disk):**
- .planning/phases/445-typed-api-contract-rust-ts-codegen/445-02b-SUMMARY.md

**Commits verified (2/2 present in `git log --oneline --all`):**
- 16dfb0e4 feat(445-02b): add utoipa::path annotations to 41 admin handlers
- 71bc63bc feat(445-02b): populate ApiDoc umbrella with 42 admin paths + emit full openapi.generated.yaml

**Acceptance criteria (all plan-level + handler-level):**
- `grep -rcE '^#\[cfg_attr\(feature = "gen-types", utoipa::path' crates/racecontrol/src/` = 42 (target ≥40) ✅
- `cargo build --release -p racecontrol-crate --bin racecontrol` → 0 (default build unregressed) ✅
- `cargo build --release --bin gen-types --features gen-types` → 0 ✅
- `cargo run --release --bin gen-types --features gen-types` → 0, wrote 20,245 bytes ✅
- `grep -c operationId docs/openapi.generated.yaml` = 42 (target ≥40) ✅
- `grep -c "^  /api/v1/" docs/openapi.generated.yaml` = 42 ✅
- `grep -c "staffJWT" docs/openapi.generated.yaml` ≥1 (security scheme reference) — 30+ ✅
- `bash scripts/check-gen-types-determinism.sh` → `DETERMINISTIC: f29aa838...` ✅
- `grep -c "api::openapi\|ApiDoc" crates/racecontrol/src/main.rs` = 0 (Pitfall 3 preserved) ✅
- `cargo test -p rc-common --test enum_tagging_audit` → 1 passed (D-14 preserved) ✅
- `cargo test -p racecontrol-crate ... no_duplicate_route_registrations` → 1 passed (Route Uniqueness preserved) ✅

**SHA256 stability anchor:**
- `docs/openapi.generated.yaml` → `f29aa838ae11d6e56c579089b09dfd1381049b78d46ee44bbca5806f407e1836` (combined hash across yaml + index.ts per determinism harness; yaml-only: `50e87c18aa8d4912b2d085056924d31e6f2befaecae019968fda167f78edc765`)

---
*Phase: 445-typed-api-contract-rust-ts-codegen*
*Completed: 2026-04-21*
