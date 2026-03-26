# Phase 201: Frontend Integration & Type Sync - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Update all 4 frontend apps (kiosk, web dashboard, admin, PWA) to handle new billing states (WaitingForGame, CancelledNoPlayable, PausedGamePause), game states (Stopping, externally_tracked), and metrics endpoints. Sync shared TypeScript types with Rust enums. Add contract tests for type drift prevention. Rebuild and deploy all 3 Next.js apps.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — frontend integration phase. Key decisions:
- How to update shared-types package (packages/shared-types/)
- Contract test approach (vitest vs jest, snapshot vs assertion)
- Which UI components need updates per app
- How to handle WaitingForGame display on kiosk ("Loading..." state)
- Whether to add new admin pages for launch matrix or integrate into existing
- Deploy sequence for 3 Next.js apps

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `packages/shared-types/` — existing TypeScript type definitions shared across apps
- `apps/kiosk/` — Next.js kiosk app at :3300
- `apps/web/` — Next.js web dashboard at :3200
- `apps/admin/` — Next.js admin dashboard at :3201
- `apps/pwa/` — PWA app (cloud deployed)
- Existing OpenAPI spec and contract tests from v21.0

### Established Patterns
- Next.js apps use `NEXT_PUBLIC_API_URL` and `NEXT_PUBLIC_WS_URL` for backend connectivity
- WebSocket DashboardEvent messages for real-time updates
- SWR for data fetching in admin app
- Tailwind CSS for styling across all apps

### Integration Points
- BillingSessionStatus enum: add WaitingForGame, CancelledNoPlayable to TS types
- GameState enum: ensure Stopping is handled in all switch statements
- DashboardEvent: handle new event payloads (reliability warning, recovery status)
- New API endpoints: /metrics/launch-matrix, /games/alternatives
- Kiosk: "Loading..." state when WaitingForGame, countdown when Active
- Admin: launch matrix page, reliability warnings display
- Web: recovery status display, billing accuracy metrics

</code_context>

<specifics>
## Specific Ideas

- 19 requirements: SYNC-01 through SYNC-07, KIOSK-01 through KIOSK-05, WEB-01 through WEB-03, ADMIN-01 through ADMIN-04
- Type drift: Rust has 10 BillingSessionStatus variants, TS had 8 — now need all 10
- Contract tests: vitest tests that fetch /api/v1/health and validate response shape
- Kiosk "Loading...": when BillingTick has status=WaitingForGame, show loading animation instead of countdown
- Admin launch matrix: new page at /admin/launch-matrix with car×track grid
- All 3 Next.js apps must be rebuilt with correct NEXT_PUBLIC_ env vars and deployed

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
