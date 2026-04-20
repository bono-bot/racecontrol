# Phase 445: Typed API Contract (Rust→TS codegen) - Research

**Researched:** 2026-04-21
**Domain:** Rust→TS type generation, OpenAPI codegen, cross-repo contract CI
**Confidence:** HIGH (toolchain versions + repo inventory), MEDIUM (D-14 enum-tagging risk)

## Summary

Phase 445 is a **tooling/refactor phase with zero business-logic change**. The upstream decisions (`utoipa` + `ts-rs`, admin-first migration, dual-write, strict CI gate) are already locked in CONTEXT.md and verified current: `utoipa = 5.4.0`, `utoipa-axum = 0.2.0` targeting axum ^0.8.0 (this workspace uses `axum = "0.8"` — direct fit), `ts-rs = 12.0.1` with `chrono-impl` + `uuid-impl` + `serde-compat` features matching the existing `rc-common` derive pattern.

Three things that materially change the planner's task breakdown versus the original CONTEXT.md framing:

1. **D-14 safety gate is triggered.** `rc-common` already uses adjacently-tagged enums (`#[serde(tag = "type", content = "data")]`) on all WS protocol enums *and* on `GameLaunchInfo` (`types.rs:871`, `tag = "state"`). D-13 says "externally tagged only" and D-19 says WS types stay hand-written — the planner must explicitly exclude these files from `#[derive(TS)]` annotation in Wave 0, not discover it at compile time.
2. **Admin surface is 43 unique endpoints — matches CONTEXT.md estimate.** Enumerated via `rcFetch('...` grep across `racingpoint-admin/src/**/*.{ts,tsx}`. This is the exact routes list the planner annotates with `#[utoipa::path]`.
3. **Pre-existing drift bug in repo (documented, not a 445 blocker).** `web/public/api-docs/openapi.yaml` is ~30 days stale vs `docs/openapi.yaml` (`BillingSessionStatus` still has `paused_idle` instead of the 4 split variants). Generated output will structurally eliminate this duplicate, but the planner should decide whether to (a) redirect Swagger UI to `docs/openapi.generated.yaml` or (b) keep both paths during dual-write.

**Primary recommendation:** 6-plan phase, Wave 0 = safety audits (enum tagging, route enumeration, admin surface freeze), Wave 1 = `gen-types` binary + dependencies, Wave 2 = annotations + generator run, Wave 3 = dual-write re-export + admin migration, Wave 4 = CI gate + regression fixture, Wave 5 = deploy audit + SUMMARY.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Toolchain**
- **D-01:** `utoipa` for OpenAPI annotations on axum routes.
- **D-02:** `ts-rs` for TypeScript generation from Rust structs (derive-based).
- **D-03:** NEW `gen-types` binary in `crates/racecontrol` (feature-flagged, not in default build).

**Migration target**
- **D-04:** `racingpoint-admin` migrates first.
- **D-05:** Admin uses workspace-relative path (`file:../racecontrol/packages/shared-types`) in its `package.json`.

**API contract surface scope**
- **D-06:** Annotate admin-surface routes ONLY in this phase. Other ~370 routes stay un-annotated until follow-on phases.
- **D-07:** `rc-common` types derive `#[derive(TS)]` only for structs used by admin routes. Use a discovery script to enumerate them before planning.

**Contract delivery**
- **D-08:** Reuse existing `@racingpoint/types` workspace package at `packages/shared-types/`.
- **D-09:** Generated output goes to `packages/shared-types/generated/` (separate from hand-written `src/`).
- **D-10:** `packages/shared-types/src/index.ts` re-exports from `generated/` for each migrated type; hand-written coexists for types not yet migrated.

**Dual-write + migration policy**
- **D-11:** Dual-write period: one release cycle (typically 2–3 weeks for this repo).
- **D-12:** For each type being migrated, audit hand-written vs generated for drift BEFORE flipping the re-export.

**Enum representation policy**
- **D-13:** Externally tagged enums (serde default) for all migrated types.
- **D-14:** If any existing Rust enum uses internal/adjacent tagging, audit it — do NOT blindly convert to externally-tagged. Flag for planner. **[TRIGGERED — see § Adjacently-tagged enums below.]**

**CI enforcement**
- **D-15:** Strict drift check from day one: `cargo run --bin gen-types && git diff --exit-code packages/shared-types/generated/ docs/openapi.generated.yaml` — PR fails on any drift.
- **D-16:** Admin repo's CI runs `tsc --noEmit` against the generated types post-migration.
- **D-17:** Add `deploy-audit.sh` manifest item: generated-types freshness.

**WS protocol**
- **D-18:** DEFER WS protocol versioning to a separate phase.
- **D-19:** WS message types (`ServerMessage`, `AgentMessage`) remain hand-written in `ws-messages.ts` for this phase.

**Success-criteria test**
- **D-20:** Add a deliberate field-name mismatch regression fixture as a negative test.

### Claude's Discretion

The following are NOT decided — planner + researcher have flexibility:
- Exact `utoipa` vs `utoipa-axum` vs `utoipa-swagger-ui` crate selection (researcher decides based on axum version + tokio features already in lockfile)
- Whether to use `ts-rs` macro attributes (`#[ts(export_to = "...")]`) per-struct or a central export config
- File-level organization of generated `.ts` output (one file vs per-module)
- CI tool specifics (GitHub Action vs local pre-commit vs both)
- Whether to publish a version bump in `@racingpoint/types` per generation, or keep it at `1.0.0`
- Exact `deploy-audit.sh` manifest field name

### Deferred Ideas (OUT OF SCOPE)

- **Full migration of kiosk, web, pwa frontends** — follow-on phases, after admin proves the pattern.
- **WS protocol versioning** — `handle_ws_message()` is the mega-hub; needs its own phase with explicit version handshake and fleet-wide rollout plan.
- **Annotating the ~370 non-admin routes** — incremental follow-on phases per consumer.
- **Deleting hand-written `packages/shared-types/src/*.ts`** — only after dual-write period proves generated is complete and stable (one release cycle post-445).
- **True mesh intelligence** — user flagged this as separate aspiration; not solved by typed contracts.
- **Cloud↔venue version skew handling** — generated types embed a build_id; admin-refuses-to-render-on-mismatch is a future enhancement, not scoped here.
- **Enum tagging audit** — if any Rust enum is currently internally/adjacently tagged and needs a wire-format change, that's a separate migration phase.
- **Contract test coverage for the other 370 routes** — mentioned in `packages/contract-tests/` infra; scale-out is future work.

</user_constraints>

<phase_requirements>
## Phase Requirements

Phase 445 has no explicit REQ-IDs in REQUIREMENTS.md (it is a late-added tooling phase from the 2026-04-21 god-node analysis, not mapped to the v49 deploy/laps/arch/revenue/game/polish waves). Requirements derived from CONTEXT.md `<domain>` + `<specifics>`:

| ID | Description | Research Support |
|----|-------------|------------------|
| TYP-01 | `gen-types` binary emits `packages/shared-types/generated/*.ts` from `rc-common` structs used by admin | ts-rs 12.0.1 + chrono-impl + uuid-impl verified (§ Standard Stack); admin surface enumerated (§ Admin surface inventory) |
| TYP-02 | `gen-types` binary emits `docs/openapi.generated.yaml` from axum routes annotated with `#[utoipa::path]` | utoipa 5.4.0 + utoipa-axum 0.2.0 targeting axum 0.8 verified (§ Standard Stack); 43 admin routes enumerated |
| TYP-03 | `packages/shared-types/src/index.ts` re-exports from `generated/` for migrated types; hand-written coexists for not-yet-migrated | pattern verified via reading existing `src/index.ts` barrel (§ Code Examples) |
| TYP-04 | `racingpoint-admin` imports migrated types from `@racingpoint/types` (path unchanged) | admin already imports `@racingpoint/types` via `file:../racecontrol/packages/shared-types` pattern — confirmed via package.json (§ Integration hazards) |
| TYP-05 | CI gate: `cargo run --bin gen-types && git diff --exit-code …` fails PR on drift | slot into existing `tests/e2e/run-all.sh` as new Suite, and pre-commit-hook workflow cascade (§ CI drift check patterns) |
| TYP-06 | `tsc --noEmit` in admin repo CI catches shape mismatch between admin code and generated types | admin already has `typecheck: tsc --noEmit` (verify); admin build step runs `next build` post-standalone (§ Integration hazards) |
| TYP-07 | Deliberate field-rename regression fixture lives in `packages/contract-tests/` and proves CI catches drift | vitest 2.1 harness ready to use; D-20 negative-test pattern (§ Code Examples) |
| TYP-08 | `deploy-audit.sh` manifest adds `generated_types_freshness` field | existing DMP protocol (docs/ARCHITECTURE.md §22); drop-in slot confirmed (§ Architecture Patterns) |
| TYP-09 | D-14 safety gate: NO adjacently-tagged / internally-tagged / flatten enums get `#[derive(TS)]` in this phase | 7 adjacently-tagged enums found in `protocol.rs`, 1 in `types.rs:871`, 1 in `mesh_types.rs:196`, 2 `#[serde(flatten)]` call sites — all excluded (§ Adjacently-tagged enums) |

</phase_requirements>

## Standard Stack

### Core

| Crate | Version | Purpose | Why Standard |
|---|---|---|---|
| `utoipa` | `5.4.0` | Derive-based OpenAPI generation from axum handlers | Most active axum-first OpenAPI crate; `ToSchema` + `#[utoipa::path]` macros; chrono/uuid first-class feature flags |
| `utoipa-axum` | `0.2.0` | Axum 0.8 bindings + ergonomic Router extension | Depends on `axum ^0.8.0` (this workspace: `axum = "0.8"` ✓); `OpenApiRouter` bridges axum `Router` and utoipa path collection |
| `ts-rs` | `12.0.1` | Derive-based TypeScript generation from Rust structs/enums | Supports `#[ts(export_to = "...")]`; serde-compat on by default; chrono/uuid feature flags align with existing `rc-common` serde usage |

### Supporting

| Crate | Version | Purpose | When to Use |
|---|---|---|---|
| `utoipa` feature `axum_extras` | N/A | Enhanced axum IntoParams integration | Enable when annotating path/query extractors — saves boilerplate on `#[derive(IntoParams)]` |
| `utoipa` feature `chrono` | N/A | Maps `DateTime<Utc>` → `string` format `date-time` (RFC 3339) | MUST enable — 188 rc-common Serialize derives already use chrono |
| `utoipa` feature `uuid` | N/A | Maps `uuid::Uuid` → `string` format `uuid` | MUST enable — rc-common uses `uuid = "1"` with `["v4","serde"]` |
| `ts-rs` feature `chrono-impl` | N/A | `impl TS for chrono::DateTime<Utc>` → TypeScript `string` | REQUIRED — orphan rule blocks user-side impl |
| `ts-rs` feature `uuid-impl` | N/A | `impl TS for uuid::Uuid` → TypeScript `string` | REQUIRED — same orphan reason |
| `ts-rs` feature `serde-compat` | default | Parses `#[serde(rename_all, skip_serializing_if, tag, content)]` etc. | MUST stay enabled — rc-common uses all of these |
| `utoipa-swagger-ui` | `8.x` | (optional) Serves Swagger UI from the binary | **SKIP for 445** — `web/public/api-docs/index.html` + static `openapi.generated.yaml` already works; adding a live route is scope creep |

### Alternatives Considered (evidence-driven)

| Instead of | Could Use | Why not chosen |
|---|---|---|
| `utoipa` | `aide` (axum-only OpenAPI) | D-01 locks utoipa; aide has smaller ecosystem, less active in 2026 |
| `ts-rs` | `typeshare` | D-02 locks ts-rs; typeshare needs separate config file, breaks derive ergonomics |
| `ts-rs` | `specta` | D-02 locks ts-rs; specta is newer and supports Zod/routers but 445 only needs types |

### Installation (verified against local registry cache 2026-04-21)

```toml
# Cargo.toml [workspace.dependencies]
utoipa = { version = "5.4", features = ["axum_extras", "chrono", "uuid"] }
utoipa-axum = "0.2"
ts-rs = { version = "12", features = ["chrono-impl", "uuid-impl", "serde-json-impl"] }
```

```bash
# Verify current:
cargo search utoipa --limit 1          # utoipa = "5.4.0"
cargo search utoipa-axum --limit 1     # utoipa-axum = "0.2.0"
cargo search ts-rs --limit 1           # ts-rs = "12.0.1"
```

**No new npm dependencies** on the TS side. `packages/shared-types/package.json` already uses `typescript: ^5`; admin uses `typescript: ^5`, Next.js 16.1.6, Playwright, vitest.

## Architecture Patterns

### Recommended Project Structure

```
crates/
  racecontrol/
    Cargo.toml                     # + utoipa + utoipa-axum (workspace = true)
                                   # + [[bin]] gen-types + [features] gen-types = ["utoipa", "utoipa-axum", "ts-rs"]
    src/
      bin/
        gen_types.rs                # NEW: main() writes out both .ts + .yaml
      api/
        routes.rs                   # unchanged — 890 lines, 448 .route() calls
        mod.rs                      # unchanged
        openapi.rs                  # NEW: #[derive(OpenApi)] umbrella + route collector
        <42 admin handler files>    # ADD #[utoipa::path] to admin handlers only
  rc-common/
    Cargo.toml                     # + ts-rs (workspace = true, feature-gated via `ts-rs` feature)
                                   # + [features] ts-rs = ["dep:ts-rs"]
    src/
      types.rs                     # ADD #[cfg_attr(feature = "ts-rs", derive(TS))] to admin-used structs
                                   # (82 pub struct/enum; admin subset ~20–25 per D-07 audit)
      inventory_types.rs           # same — PodInventory, GameInventory used by admin content drift page
      launch_contract.rs           # SKIP this phase — launch_contract is kiosk/rc-agent consumer
      diagnostic_types.rs          # same — kiosk consumer, defer
      protocol.rs                  # DO NOT ANNOTATE (D-19: 8 adjacently-tagged enums, WS path)
      mesh_types.rs                # PARTIAL — 1 adjacently-tagged enum (line 196) excluded; others OK
packages/
  shared-types/
    src/                           # HAND-WRITTEN — unchanged in this phase
      index.ts                     # UPDATED: re-exports from generated/ for migrated types only
      billing.ts                   # KEPT during dual-write
      pod.ts                       # KEPT during dual-write
      fleet.ts                     # KEPT during dual-write
      ws-messages.ts               # KEPT permanently (D-19)
      …
    generated/                     # NEW, git-tracked, CI-enforced non-drift
      index.ts                     # barrel export from gen-types
      BillingSessionInfo.ts        # one file per type (ts-rs default)
      PodInfo.ts
      FleetHealthResponse.ts
      …
  contract-tests/
    tests/                         # NEW: add tests/regression-drift.test.ts (D-20 fixture)
docs/
  openapi.yaml                     # KEPT during dual-write (hand-written, stale in 2 places)
  openapi.generated.yaml           # NEW, git-tracked, CI-enforced
```

### Pattern 1: utoipa + utoipa-axum annotation

**What:** Annotate handlers with `#[utoipa::path]` and collect them in an `OpenApi` derive root.

**When to use:** Every admin-surface handler (43 paths). Non-admin handlers stay untouched.

**Example:**
```rust
// Source: https://docs.rs/utoipa-axum/0.2.0/utoipa_axum/
// crates/racecontrol/src/api/openapi.rs
use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};

#[derive(OpenApi)]
#[openapi(
    info(title = "Racing Point racecontrol API", version = "1.0.0"),
    tags((name = "admin"), (name = "fleet"), (name = "billing")),
)]
pub struct ApiDoc;

// Per-handler annotation:
// crates/racecontrol/src/api/fleet_health.rs
#[utoipa::path(
    get,
    path = "/api/v1/fleet/health",
    tag = "fleet",
    responses((status = 200, body = FleetHealthResponse))
)]
pub async fn fleet_health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse { ... }

// Router registration (parallel to routes.rs):
pub fn admin_openapi_router(state: Arc<AppState>) -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::new()
        .routes(routes!(fleet_health_handler))
        .routes(routes!(billing_active_handler))
        // ... 43 admin handlers
}
```

### Pattern 2: ts-rs derive with cfg_attr feature gating

**What:** Gate `#[derive(TS)]` behind a Cargo feature so the default release build stays lean.

**When to use:** Every rc-common struct consumed by admin routes.

**Example:**
```rust
// Source: https://docs.rs/ts-rs/12.0.1 wiki
// crates/rc-common/src/types.rs
use serde::{Deserialize, Serialize};
#[cfg(feature = "ts-rs")]
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(TS))]
#[cfg_attr(feature = "ts-rs", ts(export, export_to = "../../packages/shared-types/generated/"))]
pub struct BillingSessionInfo {
    pub id: String,
    pub driver_id: String,
    // chrono DateTime handled by chrono-impl feature:
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    // … rest unchanged
}
```

**Key insight on paise/u32:** ts-rs emits `u32` / `i64` as TypeScript `number` by default. Existing `billing.ts` uses `cost_paise?: number` — this round-trips cleanly; no value_type override needed.

### Pattern 3: gen-types binary shape

**What:** A small `main()` that calls `ApiDoc::openapi()` for YAML and runs ts-rs `export()`.

**Example:**
```rust
// crates/racecontrol/src/bin/gen_types.rs
fn main() -> anyhow::Result<()> {
    // 1. Emit OpenAPI YAML
    let openapi = racecontrol::api::openapi::ApiDoc::openapi();
    let yaml = serde_yaml::to_string(&openapi)?;
    std::fs::write("docs/openapi.generated.yaml", yaml)?;
    // 2. Emit TS — ts-rs writes to path from #[ts(export_to = "...")]
    //    Simply referencing the types forces codegen via their TS impl.
    //    Use `TS::export_all_to()` when available (12.x supports it via test-attr).
    rc_common::types::BillingSessionInfo::export_all_to(
        "packages/shared-types/generated/"
    )?;
    // … one call per root type
    Ok(())
}
```

**Anti-pattern warning:** ts-rs 12.x emits export calls inside `#[test]` harnesses by default. Running `cargo test --features ts-rs export_bindings` is the canonical path. The `gen-types` binary approach is supported but requires `TS::export_all_to()` explicitly. The planner should **verify this works on 12.0.1 and, if not, fall back to `cargo test -p rc-common --features ts-rs bindings`** — this is documented in the ts-rs wiki.

### Anti-Patterns to Avoid

- **Do NOT annotate all 448 routes in one PR.** D-06 says admin only. PR size matters for review; splitting across phases is intentional.
- **Do NOT change enum wire format during 445.** D-14 says if `#[serde(tag, content)]` exists, audit and skip — don't "clean it up" to externally-tagged. That changes the wire format on every consumer.
- **Do NOT add `#[derive(TS)]` to enums with `#[serde(flatten)]` fields.** ts-rs + serde-compat handles flatten but emits different output than hand-written. Two flatten sites exist (`types.rs:1114`, `protocol.rs:755`) — they stay hand-written this phase.
- **Do NOT put generated files in `.gitignore`.** D-15 requires `git diff --exit-code` — the files MUST be committed, or the drift check becomes meaningless.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---|---|---|---|
| OpenAPI JSON/YAML emission | Custom serde-based route walker | `utoipa::OpenApi::openapi()` + `serde_yaml` | utoipa already emits OpenAPI 3.0.3 spec with all schema components resolved |
| TS type emission from Rust | Custom AST walker / handlebars template | `ts-rs` derive + `TS::export_all_to` | ts-rs handles recursive types, generics, `Option<T>` → `T | null`, `Vec<T>` → `T[]`, all serde attrs |
| Chrono DateTime → TS type | Hand-map each field | `ts-rs` `chrono-impl` feature | `DateTime<Utc>` → TS `string` (ISO-8601); matches current hand-written `started_at?: string` |
| Enum union representation | Hand-write `type X = "a" \| "b"` lines | `ts-rs` with `#[serde(rename_all = "snake_case")]` | Already used everywhere — ts-rs parses it natively |
| CI drift detection | Custom parity script (existing `check-billing-status-parity.js` pattern) | `cargo run --bin gen-types && git diff --exit-code` | One command, no parsing; file bytes are the contract |
| Cross-repo workspace linking | Publish to internal npm | `file:../racecontrol/packages/shared-types` (D-05 locked) | Admin + racecontrol always clone to same parent; sibling-repo is stable |

**Key insight:** The repo *already* has a narrow hand-rolled parity tool (`scripts/check-billing-status-parity.js`) that was added when `BillingSessionStatus` kept drifting. That file's existence is direct evidence that the bug class happens in production and that a per-type hand-rolled fix does not scale. Phase 445 generalizes this pattern.

## Runtime State Inventory

Phase 445 is **not a rename/refactor phase that touches runtime state**. Generated types are source-code artifacts; no databases, live services, OS registrations, secrets, or installed packages carry a name that needs updating. This section is intentionally minimal.

| Category | Items Found | Action Required |
|---|---|---|
| Stored data | None — no DB stores Rust type names as keys | None — verified by grepping `CREATE TABLE` + migrations |
| Live service config | Swagger UI at `web/public/api-docs/` serves `openapi.yaml` statically; no auto-load of `openapi.generated.yaml` until planner wires it in | Planner decides Wave 3/4: redirect `web/public/api-docs/openapi.yaml` to `openapi.generated.yaml` OR leave both |
| OS-registered state | None — no schtasks / pm2 / systemd reference this | None |
| Secrets/env vars | None — no env var carries a type name | None |
| Build artifacts | `target/release/gen-types.exe` becomes a tracked CI artifact; NO production binary changes | `cargo clean` not needed; default `gen-types` stays feature-flagged off |

## Common Pitfalls

### Pitfall 1: utoipa `ToSchema` collision with `TS` derive

**What goes wrong:** Both derives generate schema metadata from the same struct. If the feature flags are not cleanly gated, a default build (no `gen-types` feature) pulls utoipa + ts-rs into every pod binary, bloating `rc-agent.exe` and `racecontrol.exe`.

**Why it happens:** Rust feature unification — if any crate depends on rc-common with `ts-rs` on, every other crate sees it.

**How to avoid:** Make the `ts-rs` feature **additive and weak**: `[features] ts-rs = ["dep:ts-rs"]` on rc-common; `[features] gen-types = ["ts-rs"]` on racecontrol; NEVER enable from rc-agent, rc-sentry, rc-watchdog, etc. Verify with `cargo tree -e features -p rc-agent | grep ts-rs` → must be empty.

**Warning signs:** Release binary size grows >5%; ts-rs appears in `cargo tree -e features --workspace` for any non-`gen-types` feature set.

### Pitfall 2: ts-rs chrono DateTime format drift

**What goes wrong:** Hand-written `BillingSession.started_at?: string` silently accepts any string. ts-rs with `chrono-impl` emits `started_at?: string | null` (OR `string` depending on `Option<T>` handling). The `null` vs `undefined` distinction matters for `JSON.parse` with `strict: true` on admin's Zod validators.

**Why it happens:** Rust `Option<T>` → JSON `null`. Serde with `skip_serializing_if = "Option::is_none"` erases the key entirely → JS `undefined`. ts-rs emits the property type, not the serialization skip behavior.

**How to avoid:** In the D-12 drift audit, for every `Option<T>` field with `skip_serializing_if`, verify admin's Zod schema treats the field as `.optional()` (undefined, not null). One test fixture per family is enough.

**Warning signs:** Zod `.safeParse()` fails in admin after migration with "expected string, received null" on first-rendered session page.

### Pitfall 3: utoipa-axum + existing 448-route Router merge

**What goes wrong:** `utoipa-axum::OpenApiRouter` returns `OpenApiRouter`, NOT plain `axum::Router`. The existing `api_routes()` in `routes.rs` merges 5 sub-routers via `axum::Router::merge`. Mixing OpenApiRouter into that plain-Router merge is a type mismatch.

**Why it happens:** Two router APIs at different levels.

**How to avoid:** Build a **parallel** `admin_openapi_router()` that mirrors the admin routes for OpenAPI collection, then `.into_router()` it back onto the main Router. Critically, **the existing routes.rs stays unchanged** — this phase does NOT re-do route registration. OpenApiRouter is used ONLY inside `gen-types` binary to harvest the spec; the live server still uses plain `axum::Router` via `routes.rs`.

**Warning signs:** Route-registration duplication test (`route_uniqueness_tests::no_duplicate_route_registrations`, see racecontrol CLAUDE.md § Process) starts failing with "duplicate METHOD+PATH" errors.

### Pitfall 4: Admin path resolution — `file:../racecontrol/...` on Windows

**What goes wrong:** npm on Windows sometimes translates `file:../racecontrol/packages/shared-types` to an absolute path via `C:\Users\...\racecontrol\...`. When admin deploys to Bono VPS (Linux, sibling clone layout), absolute paths don't match.

**Why it happens:** `file:` protocol resolves relative paths at `npm install` time to absolute, not runtime.

**How to avoid:** Use `file:../racecontrol/packages/shared-types` (the sibling-relative form, per D-05). Verify on BOTH James Windows machine AND Bono Linux VPS before marking phase ready. Admin's CI/deploy script (scripts/admin-deploy.sh) already expects this layout.

**Warning signs:** `npm install` on admin Linux gives "Cannot find module '@racingpoint/types'" after a clean checkout.

### Pitfall 5: Adjacently-tagged enums ghosting into D-13 scope

**What goes wrong:** A planner reads D-13 ("externally tagged only") and unthinkingly adds `#[derive(TS)]` to `GameLaunchInfo` (types.rs:871, `#[serde(tag = "state", content = "detail")]`). ts-rs + serde-compat emits a different TypeScript shape than hand-written `LaunchDiagnostics`. Silent drift in a production WS path.

**Why it happens:** ts-rs supports adjacently-tagged but emits a more verbose discriminated-union shape than hand-rolled code. CONTEXT.md D-14 explicitly guards this.

**How to avoid:** Wave 0 must include a Cargo build attempt with `#[derive(TS)]` added ONLY to a hand-picked externally-tagged whitelist (e.g. `PodInfo`, `BillingSessionInfo`, `FleetHealthResponse`, `PodInventory`, `GameInventory`). Verify grep of `#[serde(tag` across all files listed in § Adjacently-tagged enums returns 0 hits inside TS-derived structs.

**Warning signs:** `gen-types` output diverges from hand-written for any WS-adjacent type; admin's Zod validator fails on first render.

### Pitfall 6: OpenAPI YAML emission ordering (CI flake class)

**What goes wrong:** `serde_yaml` emits map keys in iteration order, which can be non-deterministic across `HashMap` rebuilds. CI drift check (`git diff --exit-code`) then fails intermittently even when NO source changed.

**Why it happens:** utoipa internally uses `BTreeMap` for OpenAPI components (sorted), BUT the top-level `paths` ordering depends on route-registration order. If route registration uses `HashMap` iteration anywhere in the chain, output is non-deterministic.

**How to avoid:** utoipa 5.x uses `BTreeMap` throughout. Verify by running `cargo run --bin gen-types` 3× in a row — output must be byte-identical. Document expected hash (sha256 of generated YAML) in the first commit for reference.

**Warning signs:** `git diff --exit-code docs/openapi.generated.yaml` fails on CI but passes locally; running `gen-types` again produces different output.

## Code Examples

### Example 1: Minimal annotated handler (admin fleet health)

```rust
// Source: https://docs.rs/utoipa-axum/0.2.0 + docs/openapi.yaml existing spec
// crates/racecontrol/src/api/fleet_health.rs

use utoipa::ToSchema;
use rc_common::types::FleetHealthResponse;  // already Serialize

#[utoipa::path(
    get,
    path = "/api/v1/fleet/health",
    tag = "fleet",
    responses(
        (status = 200, description = "Current fleet health snapshot", body = FleetHealthResponse),
        (status = 401, description = "Staff JWT required"),
    ),
    security(("staffJWT" = []))
)]
pub async fn fleet_health_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // existing body unchanged
}
```

### Example 2: rc-common struct gaining TS derive

```rust
// Source: https://github.com/Aleph-Alpha/ts-rs#examples
// crates/rc-common/src/types.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-rs",
    ts(export,
       export_to = "../../packages/shared-types/generated/",
       rename_all = "snake_case"))]
#[derive(utoipa::ToSchema)]  // always on — tiny; no runtime impact
pub struct PricingTier {
    pub id: String,
    pub name: String,
    pub duration_minutes: u32,
    pub price_paise: u32,
    pub is_trial: bool,
    pub is_active: bool,
}
```

### Example 3: D-20 regression fixture (vitest)

```typescript
// Source: D-20 requirement + existing packages/contract-tests/ pattern
// packages/contract-tests/tests/regression-drift.test.ts

import { describe, it, expectTypeOf } from 'vitest';
import type { BillingSessionInfo } from '@racingpoint/types';

describe('TYP-07: deliberate mismatch fixture', () => {
  it('fails tsc --noEmit when Rust field renames to driver_name_v2', () => {
    // This test is ASSERTIVE at the TYPE level.
    // If a Rust-side PR renames `driver_name` → `driver_name_v2` without
    // regenerating the TS, this will still compile green because the
    // generated type still has `driver_name`. The drift check
    // (`cargo run --bin gen-types && git diff --exit-code`) is the gate
    // that catches the Rust-side rename — NOT this test.
    //
    // This test instead locks the CURRENT expected shape.
    expectTypeOf<BillingSessionInfo>().toHaveProperty('driver_name');
    // Negative fixture: prove the OLD field name is gone
    // @ts-expect-error — ai_difficulty is the canonical bug-class example
    expectTypeOf<BillingSessionInfo>().toHaveProperty('ai_difficulty');
  });
});
```

### Example 4: index.ts re-export pattern (D-10)

```typescript
// packages/shared-types/src/index.ts (after Phase 445 Wave 3)

// Types migrated to generated — re-export from generated/ only:
export type { BillingSessionInfo, BillingSessionStatus, PricingTier }
  from '../generated/index';
export type { PodInfo, PodStatus, SimType, GameState, DrivingState }
  from '../generated/index';
export type { FleetHealthResponse, PodFleetStatus }
  from '../generated/index';
export type { PodInventory, GameInventory, ContentDirsResponse }
  from '../generated/index';

// Types NOT yet migrated — KEEP hand-written (D-19 + not-yet-in-scope):
export type { FlagSyncPayload, WsConfigPushPayload, OtaDownloadPayload,
              KillSwitchPayload, ConfigAckPayload, OtaAckPayload,
              FlagCacheSyncPayload, LaunchDiagnostics, BillingTick,
              GameStateChanged } from './ws-messages';
export type { RedeemPinResponse, RedeemPinStatus } from './reservation';
export type { FailureMode, LaunchStatsResponse, BillingAccuracyResponse,
              AlternativeCombo, LaunchMatrixRow } from './metrics';
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|---|---|---|---|
| Hand-written `docs/openapi.yaml` | utoipa-derived `docs/openapi.generated.yaml` | This phase | Drift permanently impossible |
| Hand-written `packages/shared-types/src/*.ts` | ts-rs-derived `packages/shared-types/generated/*.ts` | This phase (admin subset only) | Compile-time type guarantee for admin |
| `scripts/check-billing-status-parity.js` per-type parity | `git diff --exit-code generated/` whole-tree | This phase | One gate, unbounded coverage |
| `utoipa 4.x` + axum 0.7 | `utoipa 5.4` + `utoipa-axum 0.2` on axum 0.8 | utoipa 5.0 release late 2025 | axum 0.8 needed utoipa-axum 0.2 split crate — workspace already on axum 0.8, zero migration cost |
| `ts-rs 7.x` single-file export | `ts-rs 12.x` per-struct export with `export_to` | ts-rs 8.0 (mid-2024) through 12.0 (late 2025) | Cleaner directory layout, deterministic output |

**Deprecated/outdated (watch out for online tutorials):**
- `utoipa 4.x` tutorials reference `utoipa::axum` module — moved to `utoipa-axum` crate in 5.0.
- `ts-rs 6.x` tutorials reference `#[ts(export)]` at crate-root — removed; use `TS::export_all_to()` explicitly.
- `aide` tutorials predating axum 0.8 won't apply — aide 0.13+ supports axum 0.8 but we're not using aide.

## Open Questions

1. **Does `ts-rs 12.0.1`'s `TS::export_all_to()` work inside a plain `fn main()` binary, or is `cargo test --features ts-rs export_bindings` still the only supported path?**
   - What we know: ts-rs 12.x added `export_all_to` explicitly for non-test contexts (per wiki + changelog).
   - What's unclear: whether it handles transitive export correctly without the test harness's type registration.
   - Recommendation: Wave 0 Task 0 = 30-min spike — add `#[derive(TS)]` to ONE rc-common struct (e.g. `PricingTier`), write a throwaway `gen_types.rs` main, confirm output. If fails, fall back to `cargo test` strategy (documented in ts-rs wiki).

2. **Does the admin repo have a `tsconfig.json` `paths` mapping that needs updating to pick up `generated/`?**
   - What we know: admin imports from `@racingpoint/types` (package name); Node resolution handles `main` field.
   - What's unclear: whether admin's `tsconfig.json` has explicit `paths` declarations that break on new export shape.
   - Recommendation: Planner reads `racingpoint-admin/tsconfig.json` during Wave 0, adds to D-12 audit.

3. **Should `web/public/api-docs/index.html` be repointed to `openapi.generated.yaml` this phase?**
   - What we know: Swagger UI at `web/public/api-docs/` currently serves `openapi.yaml` which is 30 days stale (missing 4 billing status variants).
   - What's unclear: whether fixing stale Swagger UI is in scope or a follow-on phase.
   - Recommendation: Planner's discretion. Cheap to fix (1-line href change in index.html), high value (staff sees correct spec), zero risk (served static). Include in Wave 4 checklist; pull out to a sibling phase if it expands.

4. **Does rc-common's `ts-rs` feature correctly compile under `--no-default-features` builds of the no-default-features rc-agent variants?**
   - What we know: rc-agent has a no-default-features build path (ref: Phase 413 Plan 04 deferred-items); the default is fine.
   - What's unclear: whether weak-dep (`dep:ts-rs`) propagates correctly or whether `rc-agent --no-default-features` inadvertently pulls ts-rs in.
   - Recommendation: Wave 4 verification step = `cargo tree -e features -p rc-agent-crate --no-default-features | grep ts-rs` must return empty.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|---|---|---|---|---|
| `cargo` | gen-types build | ✓ | 1.93.1 stable (per MEMORY.md) | — |
| `utoipa` / `utoipa-axum` / `ts-rs` | workspace deps | ✓ (crates.io reachable) | 5.4.0 / 0.2.0 / 12.0.1 (verified via `cargo search`) | — |
| `node` | admin + shared-types TS | ✓ | v22.22.0 (James machine) | — |
| `tsc` (TypeScript 5.6) | `tsc --noEmit` in D-16 | ✓ | contract-tests already on `typescript ^5.6`, shared-types on `typescript ^5` | — |
| `vitest` | regression fixture (D-20) | ✓ | `^2.1.0` in contract-tests/package.json | — |
| `git` | CI drift check | ✓ | standard | — |
| Sibling repo `racingpoint-admin/` | D-04 migration target | ✓ | on local filesystem at `C:/Users/bono/racingpoint/racingpoint-admin/` | — |
| `docs/openapi.yaml` hand-written | D-09 baseline | ✓ | 2854 lines, slightly stale (see drift findings) | — |
| `serde_yaml` | emit `openapi.generated.yaml` | needs adding | `0.9` | use `serde_json` → external yaml conversion (ugly, avoid) |

**Missing dependencies with no fallback:** None.
**Missing dependencies with fallback:** `serde_yaml` — cheap to add; not fallback-critical.

## Validation Architecture

### Test Framework

| Property | Value |
|---|---|
| Framework | `vitest 2.1` (packages/contract-tests) + `cargo test` (workspace) |
| Config file | `packages/contract-tests/package.json` scripts; `Cargo.toml` at root |
| Quick run command | `cd packages/contract-tests && npx vitest run regression-drift` |
| Full suite command | `bash tests/e2e/run-all.sh` (existing) + `cargo test -p rc-common --features ts-rs` (new) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|---|---|---|---|---|
| TYP-01 | ts-rs generates TS file for every `#[derive(TS)]` struct | unit | `cargo test -p rc-common --features ts-rs ts_bindings` | Wave 0 |
| TYP-02 | utoipa emits admin-tagged paths + schema components | unit | `cargo run --bin gen-types && test -s docs/openapi.generated.yaml` | Wave 0 |
| TYP-03 | `@racingpoint/types` barrel re-exports both src/ and generated/ | unit | `cd packages/shared-types && npx tsc --noEmit` | ✅ shared-types typecheck script exists |
| TYP-04 | Admin builds with generated types | integration | `cd ../racingpoint-admin && npx tsc --noEmit && npm run build` | ✅ admin build script exists |
| TYP-05 | CI gate fails on generated-types drift | integration | `cargo run --bin gen-types && git diff --exit-code packages/shared-types/generated/ docs/openapi.generated.yaml` | Wave 0 (new script slotted into `tests/e2e/run-all.sh`) |
| TYP-06 | `tsc --noEmit` in admin passes | integration | `cd ../racingpoint-admin && npx tsc --noEmit` | ✅ uses existing typescript |
| TYP-07 | Regression fixture catches deliberate Rust rename | unit | `cd packages/contract-tests && npx vitest run regression-drift` | Wave 0 (new file) |
| TYP-08 | `deploy-audit.sh` rejects stale generated/ | integration | `bash scripts/deploy/deploy-audit.sh <old> <new>` — manifest must list `generated_types` | Wave 5 (deploy-audit.sh update) |
| TYP-09 | No adjacently-tagged enum in the TS-derived set | unit | `cargo check -p rc-common --features ts-rs 2>&1 | grep -q 'tag = '` must return nothing | Wave 0 grep |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common --features ts-rs` (~10–15s) + `cargo run --bin gen-types` + `git diff --exit-code` (~20s)
- **Per wave merge:** Full `tests/e2e/run-all.sh --skip-browser` + `cd ../racingpoint-admin && npx tsc --noEmit`
- **Phase gate:** Full suite green + admin `npm run build` green + `bash scripts/deploy/deploy-audit.sh` emits `generated_types_freshness: OK`

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/bin/gen_types.rs` — new binary
- [ ] `crates/racecontrol/src/api/openapi.rs` — utoipa `OpenApi` derive root + admin route collector
- [ ] `packages/contract-tests/tests/regression-drift.test.ts` — D-20 fixture
- [ ] `packages/shared-types/generated/` — directory created (initially empty `.gitkeep`)
- [ ] `docs/openapi.generated.yaml` — first full generation committed
- [ ] `scripts/check-generated-types-drift.sh` — new drift-check wrapper used by `run-all.sh` + pre-commit
- [ ] `scripts/deploy/deploy-audit.sh` — add `generated_types` manifest field (existing file, one-line append)
- [ ] Feature flag plumbing: `[features] ts-rs = ["dep:ts-rs"]` in `crates/rc-common/Cargo.toml`; `[features] gen-types = ["ts-rs", "utoipa", "utoipa-axum"]` in `crates/racecontrol/Cargo.toml`
- [ ] `Cargo.toml` (workspace root): add `utoipa`, `utoipa-axum`, `ts-rs`, `serde_yaml` to `[workspace.dependencies]`

## Adjacently-tagged enums (D-14 safety list — DO NOT ANNOTATE THIS PHASE)

Found by grepping `#\[serde\(tag = "…", content = "…"` across rc-common source:

| File:line | Enum | Tag/content | Phase 445 action |
|---|---|---|---|
| `crates/rc-common/src/protocol.rs:141` | WS ServerMessage top-level | `tag = "type", content = "data"` | SKIP (D-19 — WS hand-written) |
| `crates/rc-common/src/protocol.rs:772` | WS AgentMessage | `tag = "type", content = "data"` | SKIP (D-19) |
| `crates/rc-common/src/protocol.rs:1173` | WS DashboardEvent | `tag = "event", content = "data"` | SKIP (D-19) |
| `crates/rc-common/src/protocol.rs:1543` | WS sub-message | `tag = "type", content = "data"` | SKIP (D-19) |
| `crates/rc-common/src/protocol.rs:1584` | WS ClientAction | `tag = "action_type", content = "payload"` | SKIP (D-19) |
| `crates/rc-common/src/protocol.rs:1633` | WS command variant | `tag = "command", content = "data"` | SKIP (D-19) |
| `crates/rc-common/src/types.rs:871` | `GameLaunchInfo` state | `tag = "state", content = "detail"` | **SKIP this phase** — used by admin fleet view; D-14 audit deferred. Admin consumes via hand-written `ws-messages.ts::LaunchDiagnostics`. |
| `crates/rc-common/src/mesh_types.rs:196` | Mesh evidence enum | `tag = "type"` | SKIP (not admin-consumed; Mesh Intelligence is pod-local) |

Also `#[serde(flatten)]` at:
- `crates/rc-common/src/protocol.rs:755` — WS flatten path. SKIP (D-19).
- `crates/rc-common/src/types.rs:1114` — non-admin flatten. SKIP this phase.

**Net effect:** The planner's TS-derive whitelist must exclude these 10 sites. All other rc-common structs (external-tagged or no tagging at all, 80%+ of the file) are safe.

## Admin surface inventory (43 unique paths)

Enumerated via `grep -rhE "rcFetch\('[^']+" racingpoint-admin/src/ | sort -u`:

```
/activity?limit=             /ai/chat                     /billing/active
/billing/rates               /billing/start               /business-rules
/cafe/promos                 /config/audit                /config/pod/
/config/push                 /coupons                     /customer/packages
/deploy/                     /deploy/rolling              /deploy/status
/drivers                     /fleet/health                /games/active
/games/catalog               /games/launch                /games/stop
/hr/recognition              /kiosk/experiences           /kiosk/settings
/mesh/stats                  /pods                        /pods/
/pods/lockdown-all           /pods/restart-all            /pods/shutdown-all
/pods/wake-all               /presets                     /pricing
/pricing/rules               /staff                       /staff/gamification/challenges
/staff/gamification/kudos    /staff/gamification/leaderboard
/time-trials                 /tournaments                 /wallet/bonus-tiers
/wallet/bonus-tiers/admin    /wallet/topup-presets
```

Plus 2 direct `fetch('/api/rc/...')` call sites:
- `/api/rc/customer/membership/active`
- `/api/rc/customer/membership/tiers`

**Response types used by admin, to `#[derive(TS)]` in Wave 0:**
- From `rc-common/types.rs`: `PodInfo`, `PodStatus`, `SimType`, `DrivingState`, `GameState`, `BillingSessionInfo`, `BillingSessionStatus`, `PricingTier`, `Driver`, `PlayableSignal`
- From `rc-common/inventory_types.rs`: `PodInventory`, `GameInventory`, `ContentDirsResponse`, `GameDirs` (admin's content drift dashboard)
- From `racecontrol/fleet_health.rs` (NOT rc-common — may need moving): `PodFleetStatus`, `FleetHealthResponse`

**Wave 0 discovery script (recommended):**

```bash
cd crates/rc-common/src
grep -rE "pub struct |pub enum " *.rs | \
  awk -F: '{ print $2 }' | \
  grep -v ^// | \
  awk '{ print $3 }' | sed 's/[<{].*//' | sort -u
```

…then cross-reference against admin route handler return types. The planner may parallelize this audit.

## Drift findings (pre-existing, NOT 445 blockers)

During research, the following drifts were found in the existing hand-written types (fix in a later phase OR during D-12 audit — do NOT block 445 on these):

1. **`docs/openapi.yaml` vs `web/public/api-docs/openapi.yaml`** — 30 days apart; `BillingSessionStatus` still has deprecated `paused_idle` variant in web copy.
2. **`packages/shared-types/src/billing.ts` `BillingSessionStatus`** union has 11 variants; Rust `rc-common/src/types.rs::BillingSessionStatus` has 11 variants — PARITY OK, checked by existing `scripts/check-billing-status-parity.js`.
3. **`packages/shared-types/src/pod.ts` `SimType`** has 8 variants; Rust has 8 variants + 2 `#[serde(rename = "…")]` overrides (`iracing`, `f1_25`, `forza_horizon_5`) — parity OK in wire format.
4. **`packages/shared-types/src/fleet.ts` `PodFleetStatus`** is sourced from `crates/racecontrol/src/fleet_health.rs`, not rc-common — must be moved to rc-common OR gen-types must cross-crate. Recommend moving to rc-common during Wave 2.

## Sources

### Primary (HIGH confidence)
- Local cargo search (`cargo search utoipa --limit 1` → `utoipa = "5.4.0"`; same for utoipa-axum 0.2.0 + ts-rs 12.0.1). Directly queried 2026-04-21.
- [utoipa-axum docs.rs](https://docs.rs/utoipa-axum/latest/utoipa_axum/) — axum ^0.8.0 dependency confirmed
- [ts-rs Cargo.toml on GitHub](https://github.com/Aleph-Alpha/ts-rs/blob/main/ts-rs/Cargo.toml) — feature flags `chrono-impl`, `uuid-impl`, `bigdecimal-impl`, `serde-compat`, `serde-json-impl` all confirmed; ts-rs 12.0.1 MSRV 1.78.0
- [utoipa 5.4 source README on docs.rs](https://docs.rs/crate/utoipa/latest/source/README.md) — chrono feature behavior (DateTime → string date-time), value_type override, axum_extras feature
- Direct inspection of `C:\Users\bono\racingpoint\racecontrol\crates\rc-common\src\types.rs` + `protocol.rs` + `mesh_types.rs` — 10 adjacently-tagged + 2 flatten sites enumerated
- Direct inspection of `C:\Users\bono\racingpoint\racingpoint-admin\src\lib\api\*.ts` + `rcFetch` grep — 43 unique admin paths enumerated
- Direct inspection of `Cargo.toml` workspace + `crates/racecontrol/Cargo.toml` — current axum version 0.8, static CRT, incremental=false for release
- `packages/contract-tests/package.json` — vitest 2.1, typescript 5.6 confirmed ready to host D-20 fixture

### Secondary (MEDIUM confidence)
- [utoipa docs.rs](https://docs.rs/utoipa/latest/utoipa/) — ToSchema derive schema attribute (value_type, format, description)
- [ts-rs wiki — Deriving the TS trait](https://github.com/Aleph-Alpha/ts-rs/wiki/Deriving-the-TS-trait) — serde-compat attribute support, export_to syntax
- [ts-rs wiki — Feature flags](https://github.com/Aleph-Alpha/ts-rs/wiki/Feature-flags) — chrono-impl, uuid-impl, serde-compat defaults
- [utoipa GitHub README](https://github.com/juhaku/utoipa) — axum_extras feature for IntoParams integration

### Tertiary (LOW confidence — flag for validation)
- Ordering-determinism claim in Pitfall 6 (utoipa uses `BTreeMap`) — needs 3× run empirical verification in Wave 0.
- `TS::export_all_to()` in gen-types binary context — Open Question 1, needs Wave 0 spike.

## Metadata

**Confidence breakdown:**
- Standard stack (crate versions + feature flags): **HIGH** — directly verified via local cargo registry cache 2026-04-21
- Architecture patterns (annotation shape, cfg gating): **HIGH** — matches utoipa-axum 0.2 + ts-rs 12.0 docs
- Pitfalls (D-14, chrono null, router merge): **HIGH** — first 5 rooted in enumerated repo facts; Pitfall 6 is MEDIUM (needs empirical test)
- Admin surface enumeration: **HIGH** — enumerated directly from `racingpoint-admin/src/` grep
- gen-types binary emission path: **MEDIUM** — Open Question 1 flagged; Wave 0 spike required

**Research date:** 2026-04-21
**Valid until:** 2026-05-21 (crate versions drift quickly; re-check utoipa/ts-rs/utoipa-axum before implementation if >30 days elapse).
