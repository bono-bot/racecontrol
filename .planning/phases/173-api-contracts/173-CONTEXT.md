# Phase 173: API Contracts - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Document all API boundaries, extract shared TypeScript types for kiosk and admin, generate OpenAPI specs for racecontrol REST API, add contract tests that break on drift, and set up CI to enforce on every PR.

</domain>

<decisions>
## Implementation Decisions

### Shared Types Architecture
- Shared TS types live in `packages/shared-types/` in the racecontrol monorepo — single source of truth
- Kiosk and admin consume via TypeScript path aliases (`@racingpoint/types`) in tsconfig — no build step needed
- Extract the 4 most-used API shapes first: Pod status, billing session, fleet health, driver
- Rust→TS drift handled by manual sync with contract test that compares Rust struct fields to TS interface — fails on mismatch

### OpenAPI Strategy
- Hand-write `openapi.yaml` from existing route handlers (Rust/Axum has no reliable auto-gen)
- Swagger UI HTML served from `web/public/api-docs/` via web dashboard (:3200)
- Spec covers ALL `/api/v1/` endpoints: billing, pods, fleet, sessions, drivers, leaderboards, config

### Contract Tests & CI
- Jest/Vitest tests that import shared types and fetch from racecontrol API — verify response matches type shape
- GitHub Actions workflow on racecontrol repo — runs contract tests on every PR, blocks merge on failure
- Tests fail on any field drift: adding/removing/renaming a field in Rust response without updating shared TS type

### Claude's Discretion
- Exact TS interface definitions (derived from Rust structs)
- OpenAPI YAML structure and endpoint ordering
- GitHub Actions workflow configuration details
- Contract test file organization

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- racecontrol Rust route handlers define response shapes (crates/racecontrol/src/)
- Kiosk already has inline type definitions for API responses (C:/RacingPoint/kiosk/)
- Admin uses server-side proxy /api/rc/[...path] (racingpoint-admin)
- Web dashboard serves on :3200 (web/ directory in racecontrol)

### Established Patterns
- Axum handlers return JSON via serde serialization
- Next.js apps use fetch() to call racecontrol API
- TypeScript path aliases already used in admin (tsconfig paths)

### Integration Points
- racecontrol :8080 — all API endpoints
- kiosk :3300 — consumes pod, billing, fleet APIs
- admin :3200 (web dashboard) — consumes all APIs
- comms-link :8766 — relay exec/chain/health APIs
- rc-agent :8090 — health, exec, files endpoints

</code_context>

<specifics>
## Specific Ideas

- API boundary document should include: endpoint path, HTTP method, request body shape, response shape, auth requirement, which consumer calls it
- Contract tests can be simple: fetch endpoint, JSON.parse response, assert all expected keys exist with correct types
- OpenAPI spec can start from the fleet/health endpoint (most used) and work outward

</specifics>

<deferred>
## Deferred Ideas

- Auto-generation of TS types from Rust using ts-rs crate (too much setup for this milestone)
- Consumer-driven contract testing with Pact (overkill for current scale)
- API versioning strategy (v2 endpoints)

</deferred>
