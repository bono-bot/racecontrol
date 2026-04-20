# Phase 445: Typed API Contract (Rust→TS codegen) - Context

**Gathered:** 2026-04-21
**Status:** Ready for planning
**Mode:** `/gsd:discuss-phase --auto` (recommended defaults selected, no interactive questions)

<domain>
## Phase Boundary

Generate TypeScript types and OpenAPI spec from Rust route definitions and `rc-common` shared types. Migrate at least one frontend consumer (admin) from hand-written types in `packages/shared-types/` to generated types. Add CI gate that fails PRs where committed generated types drift from Rust source.

**In scope:**
- Add `utoipa` annotations to racecontrol routes (admin-facing surface only)
- Add `#[derive(TS)]` (via `ts-rs`) to `rc-common` structs consumed by admin
- New `gen-types` binary that emits `packages/shared-types/generated/*.ts` + `docs/openapi.generated.yaml`
- CI gate (`cargo run --bin gen-types && git diff --exit-code`)
- Migrate `racingpoint-admin` repo to import from generated types
- Dual-write period: hand-written types at `packages/shared-types/src/` coexist with generated at `packages/shared-types/generated/` for one release; generated wins on conflict

**Not in scope (explicit deferrals):**
- Full migration of kiosk, web, pwa (follow-on phases once admin proves pattern)
- WS protocol versioning in `handle_ws_message()` (separate phase — mega-hub blast radius)
- Annotating the remaining ~370 non-admin routes (incremental follow-on)
- Deleting hand-written types (removed in a later phase after one release of dual-write)
- Changing any business logic — this phase is pure tooling + refactor

</domain>

<decisions>
## Implementation Decisions

### Toolchain

- **D-01:** `utoipa` for OpenAPI annotations on axum routes.
  _Rationale: first-class axum integration, actively maintained, `utoipa-axum` crate exists. `aide` is an alternative but less mature. [auto — recommended]_
- **D-02:** `ts-rs` for TypeScript generation from Rust structs (derive-based).
  _Rationale: simpler developer ergonomics than `typeshare` (no separate config file), more active than `specta`. Works directly from `#[derive(TS)]`. [auto — recommended]_
- **D-03:** NEW `gen-types` binary in `crates/racecontrol` (feature-flagged, not in default build).
  _Rationale: isolated from hot path; runs only in CI + on-demand. [auto — recommended]_

### Migration target

- **D-04:** `racingpoint-admin` migrates first.
  _Rationale: documented API-error pain points in memory (`rcFetch`/`proxyFetch` drift, auth boundary confusion, cloud↔venue divergence). Smallest route surface (~30–40 admin routes per admin graphify corpus). Maximum value-per-line-changed. [auto — recommended]_
- **D-05:** Admin uses workspace-relative path (`file:../racecontrol/packages/shared-types`) in its `package.json`.
  _Rationale: matches existing sibling-repo layout. No npm publish required. [auto — recommended]_

### API contract surface scope

- **D-06:** Annotate admin-surface routes ONLY in this phase. Other ~370 routes stay un-annotated until follow-on phases.
  _Rationale: avoids 400-endpoint annotation fatigue; proves pattern on smallest high-value surface. [auto — recommended]_
- **D-07:** `rc-common` types derive `#[derive(TS)]` only for structs used by admin routes. Use a discovery script to enumerate them before planning.
  _Rationale: minimizes blast radius; a struct shared across venue+cloud that gets TS-derived without audit could produce a subtly different shape than hand-written. [auto — recommended]_

### Contract delivery

- **D-08:** Reuse existing `@racingpoint/types` workspace package at `packages/shared-types/`.
  _Rationale: package already exists; kiosk/web/pwa already consume it. Introducing a new package would fragment. [auto — recommended]_
- **D-09:** Generated output goes to `packages/shared-types/generated/` (separate from hand-written `src/`).
  _Rationale: clean diff during dual-write; easy deletion of hand-written when dual-write ends; git history stays legible. [auto — recommended]_
- **D-10:** `packages/shared-types/src/index.ts` re-exports from `generated/` for each migrated type; hand-written coexists for types not yet migrated.
  _Rationale: consumers see a single import path (`@racingpoint/types`) regardless of whether a type is generated-yet or still hand-written. Seamless to migrate one type at a time. [auto — recommended]_

### Dual-write + migration policy

- **D-11:** Dual-write period: one release cycle (typically 2–3 weeks for this repo).
  _Rationale: if generated types turn out to miss a hand-written edge case, rollback is `git revert` of the index.ts re-export line, not a package restructure. [auto — recommended]_
- **D-12:** For each type being migrated, audit hand-written vs generated for drift BEFORE flipping the re-export.
  _Rationale: the hand-written `.ts` may encode decisions the Rust source doesn't. Silent drift during migration is the worst-case regression. [auto — recommended]_

### Enum representation policy

- **D-13:** Externally tagged enums (serde default) for all migrated types.
  _Rationale: ts-rs handles externally-tagged cleanly; internally-tagged breaks with `#[serde(flatten)]`; adjacently-tagged is verbose on the TS side. [auto — recommended]_
- **D-14:** If any existing Rust enum uses internal/adjacent tagging, audit it — do NOT blindly convert to externally-tagged. Flag for planner.
  _Rationale: changing enum tagging changes the wire format, which breaks every existing consumer. [auto — recommended safety gate]_

### CI enforcement

- **D-15:** Strict drift check from day one: `cargo run --bin gen-types && git diff --exit-code packages/shared-types/generated/ docs/openapi.generated.yaml` — PR fails on any drift.
  _Rationale: advisory-only re-accumulates drift; hard gate is the structural fix. Matches existing `security-check.js` / `run-all.sh` pattern. [auto — recommended]_
- **D-16:** Admin repo's CI runs `tsc --noEmit` against the generated types post-migration.
  _Rationale: catches shape mismatches at admin build time, not at runtime. [auto — recommended]_
- **D-17:** Add `deploy-audit.sh` manifest item: generated-types freshness.
  _Rationale: existing DMP protocol demands a manifest item per deploy; generated-types join `rust_binary`, `frontend_rebuild`, etc. [auto — recommended]_

### WS protocol

- **D-18:** DEFER WS protocol versioning to a separate phase.
  _Rationale: `handle_ws_message()` is the graphify-verified mega-hub (107 edges racecontrol, 91 edges rc-agent). Blast radius is fleet-wide. HTTP-first is the safe path. [auto — recommended]_
- **D-19:** WS message types (`ServerMessage`, `AgentMessage`) remain hand-written in `ws-messages.ts` for this phase.
  _Rationale: untouched = no regression risk. Follow-on phase handles them with explicit protocol version handshake. [auto — recommended]_

### Success-criteria test

- **D-20:** Add a deliberate field-name mismatch regression fixture as a negative test.
  _Rationale: CONTEXT.md's original success-criteria row demands it; this locks it as an acceptance test for the planner. Example: a test `AdminUserResponse` struct with a field rename on the Rust side must make admin `tsc --noEmit` fail in CI. [auto — recommended]_

### Claude's Discretion

The following are NOT decided here — planner + researcher have flexibility:
- Exact `utoipa` vs `utoipa-axum` vs `utoipa-swagger-ui` crate selection (researcher decides based on axum version + tokio features already in lockfile)
- Whether to use `ts-rs` macro attributes (`#[ts(export_to = "...")]`) per-struct or a central export config
- File-level organization of generated `.ts` output (one file vs per-module)
- CI tool specifics (GitHub Action vs local pre-commit vs both)
- Whether to publish a version bump in `@racingpoint/types` per generation, or keep it at `1.0.0`
- Exact `deploy-audit.sh` manifest field name

### Folded Todos

None checked — `gsd-tools todo match-phase 445` not run in this auto session. Recommend planner runs it before task breakdown in case pending todos like "shared types hygiene" or "openapi drift" exist.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing in-repo infrastructure (must audit BEFORE touching)
- `packages/shared-types/src/index.ts` — Current hand-written type barrel export; 9 modules (billing, config, driver, fleet, metrics, pod, reservation, ws-messages)
- `packages/shared-types/src/billing.ts` — Billing types; highest-churn module historically
- `packages/shared-types/src/ws-messages.ts` — WS message types; DO NOT MIGRATE IN THIS PHASE (D-19)
- `packages/shared-types/package.json` — Declares `@racingpoint/types` v1.0.0 private package
- `packages/contract-tests/` — Existing vitest contract-testing harness; reuse for D-20 regression fixture
- `docs/openapi.yaml` — Hand-written OpenAPI spec (to be replaced by `docs/openapi.generated.yaml`); do NOT delete in this phase

### Source-of-truth Rust crates
- `crates/rc-common/src/` — Shared types crate; `#[derive(TS)]` additions go here
- `crates/racecontrol/src/api/` — Axum route handlers; `utoipa` annotations go here
- `crates/racecontrol/src/api/routes.rs` — Central route registry (if it exists) — planner confirms

### Consumer repos
- `../racingpoint-admin/package.json` — Sibling repo; migration target for D-04/D-05
- `kiosk/package.json`, `web/package.json`, `pwa/package.json` — Deferred consumers; NOT migrated this phase

### Governance + standards docs
- `.planning/phases/445-typed-api-contract-rust-ts-codegen/CONTEXT.md` — Original phase-scoping doc (seed for this CONTEXT.md)
- `C:/Users/bono/racingpoint/racecontrol/CLAUDE.md` — "Cross-Boundary Serialization" standing rules (§ Testing & Verification) — documents the bug class this phase eliminates
- `docs/ARCHITECTURE.md` §22 (Deploy Manifest Protocol) — adds a new manifest entry per D-17
- `ECOSYSTEM-MANIFEST.json` — 14 critical systems; 4 frontend consumers (kiosk, web, admin, pwa)
- `SWAPLOG.md` — every generated-types regeneration that reaches production must appear in SWAPLOG per project convention

### Prior-phase reference
- `.planning/phases/414-continuous-billing-session/` — blocker dependency (D-dependencies); billing FSM must stabilize before touching billing types
- `.planning/phases/415-reserved-mp-group-booking/` — may add MP route surface; coordinate schemas before annotating admin routes
- `.planning/phases/413.1-deploy-server-step4-fix-and-plan11-retry/` — Phase 413 added `GET /api/v1/pods/mesh-service-key`; admin does NOT consume it (server-only route), so it's outside 445's annotation set

### Ecosystem-coverage note
Graphify currently indexes 4 of ~17 local corpora (racecontrol, admin, comms-link, whatsapp). Eight additional corpora are parsed as graph.json (kiosk, pwa, web, rc-sentry, rc-agent, admin-api, billing-api, customer-api) but NOT exposed via MCP. Phase 445 does not depend on full graph coverage, but researcher should NOT use graphify as its only source-of-truth enumeration for types — read `crates/rc-common/src/` directly.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `@racingpoint/types` package — exists with hand-written structure (billing/config/driver/fleet/metrics/pod/reservation/ws-messages). Generated types slot in via `generated/` subdirectory, re-exported from `src/index.ts`.
- `packages/contract-tests/` — existing vitest harness for contract regression tests. Reuse for D-20 negative fixture.
- `docs/openapi.yaml` — existing hand-written OpenAPI — serves as reference for what shape admin currently expects. Cross-reference during D-12 drift audit.
- `security-check.js` pattern (mentioned in CLAUDE.md standing rules) — model for the CI drift check in D-15.

### Established Patterns
- **Serde derive is already universal** in `rc-common`: every shared struct already has `#[derive(Serialize, Deserialize)]`. Adding `#[derive(TS)]` is additive (no refactor needed on existing derives).
- **Workspace dependencies** are centralized in `Cargo.toml`. `utoipa` + `ts-rs` added once there, pulled in by `crates/racecontrol` + `crates/rc-common` only.
- **CI enforcement via bash scripts** (not pure-config CI) — `run-all.sh` Suite model; drift check becomes Suite N.
- **Sibling-repo layout** (admin at `../racingpoint-admin/`) means admin's `package.json` references via `file:../racecontrol/packages/shared-types` work on James's machine + Bono's VPS (both clone racecontrol + racingpoint-admin at the same parent dir).
- **Static CRT + incremental=false** in workspace Cargo.toml — `gen-types` binary inherits these. Build cost is a fresh full compile (~2–3 min on James). Plan for this in CI.

### Integration Points
- **Frontend `fetchApi` / `rcFetch` / `proxyFetch` call sites** — every one must eventually migrate. Planner's consumer-surface audit step enumerates them.
- **Admin's `rcFetch` wrapper** (per admin graphify corpus: hub with 9 edges) — the single point where admin calls become typed. One wrapper change ≈ all admin API calls become typed.
- **`docs/openapi.yaml` in `docs/API.md` references** — grep for every place openapi.yaml is cited; update or dual-path during migration.
- **Build pipeline: `cargo build --release --bin gen-types`** must run in CI before `tsc --noEmit` on admin. Adds one stage to CI graph.

</code_context>

<specifics>
## Specific Ideas

Drawn directly from user's 2026-04-21 session framing:

- "racecontrol needs to communicate with the local as well as Cloud. Needs to be connected with not only rc-agent but also kiosk, PWA, POS, and all the pods." — racecontrol is the hub, spokes are the typed-contract consumers.
- "If the nodes are not connected, it is not a true mesh intelligence." — this phase accepts hub-and-spoke reality; mesh is a separate problem. But it addresses the root cause of cross-boundary bug class by making spoke contracts non-drift-able.
- The `ai_difficulty: "easy"` vs `ai_level: u32` mismatch is the canonical example of the bug class this phase closes — if a regression fixture can reproduce that mismatch and the CI catches it, the phase ships.

</specifics>

<deferred>
## Deferred Ideas

- **Full migration of kiosk, web, pwa frontends** — follow-on phases, after admin proves the pattern.
- **WS protocol versioning** — `handle_ws_message()` is the mega-hub; needs its own phase with explicit version handshake and fleet-wide rollout plan.
- **Annotating the ~370 non-admin routes** — incremental follow-on phases per consumer.
- **Deleting hand-written `packages/shared-types/src/*.ts`** — only after dual-write period proves generated is complete and stable (one release cycle post-445).
- **True mesh intelligence** — user flagged this as separate aspiration; not solved by typed contracts.
- **Cloud↔venue version skew handling** — generated types embed a build_id; admin-refuses-to-render-on-mismatch is a future enhancement, not scoped here.
- **Enum tagging audit** — if any Rust enum is currently internally/adjacently tagged and needs a wire-format change, that's a separate migration phase.
- **Contract test coverage for the other 370 routes** — mentioned in `packages/contract-tests/` infra; scale-out is future work.

### Reviewed Todos (not folded)
- Todo match not run this session (auto mode skipped `gsd-tools todo match-phase 445`). Planner should run it before task breakdown.

</deferred>

---

*Phase: 445-typed-api-contract-rust-ts-codegen*
*Context gathered: 2026-04-21 (--auto mode)*
