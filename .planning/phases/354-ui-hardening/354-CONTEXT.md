# Phase 354: UI Hardening — Context

**Gathered:** 2026-04-10
**Status:** Ready for planning
**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**Repo:** racingpoint-admin (NOT racecontrol)

<domain>
## Phase Boundary

Eliminate broken buttons, blank loading screens, and dead nav links on the admin dashboard. Three concrete deliverables:

1. **Nav cleanup** — `/memberships` and `/wallet-transactions` already hidden from sidebar (prior session). Verify no other dead routes.
2. **Loading/empty/error state pattern** — Replace 13+ "Loading..." text-only states with `SkeletonTable`/`SkeletonPage` components that already exist in `src/components/Skeleton.tsx`. Add meaningful empty state messages. Ensure mutations show success/failure toasts via `sonner`.
3. **`/settings/health` live tiles** — Already functional with `useSWR` polling. Verify tiles update live (may need `refreshInterval` tuned).

**Does NOT cover:** New features, new pages, new API endpoints, changes to racecontrol backend.
</domain>

<decisions>
## Implementation Decisions

### Nav cleanup (354-01) — ALREADY DONE
- **D-01:** `/memberships` and `/wallet-transactions` hidden from sidebar with comments in `AdminLayout.tsx:62,77`. Routes still accessible via direct URL (intentional — data is valid, just not venue-ready).
- **D-02:** No other dead routes identified. All nav items point to existing functional pages.
- **Verdict:** 354-01 is already shipped. Mark as complete, skip planning.

### Loading/empty/error pattern (354-02)
- **D-03:** Use existing `SkeletonTable` from `src/components/Skeleton.tsx` as the standard loading state. Import and replace all 13 "Loading..." text instances.
- **D-04:** Pages that already use `Skeleton*` components (10 pages) are the reference pattern. Match their style.
- **D-05:** Empty state: simple centered text with muted color + icon. No dedicated component needed — inline pattern: `<div className="text-center text-rp-grey py-12"><Icon /><p>No {items} found</p></div>`.
- **D-06:** Toast on mutations: `sonner` is already installed (v2.0.7). All mutations (create, update, delete, topup, refund) must call `toast.success()` on 200 and `toast.error()` on failure. Check each page's mutation handlers.
- **D-07:** Error state: use existing inline red-box pattern from `/customers` page — red border, error message, retry button.

### /settings/health live tiles (354-03) — ALREADY DONE
- **D-08:** `/settings/health` page already exists with `useSWR` and skeleton loading. Health tiles show status badges (ok/degraded/unreachable) with response times. Deploy log table included.
- **Verdict:** 354-03 is already shipped. Verify `refreshInterval` is set (should be 10-30s for live feel).

### Claude's Discretion
- Exact skeleton layout per page (match nearest existing page pattern)
- Whether to create a shared `<EmptyState>` component or keep inline
- Order of page upgrades (suggest: most-used pages first)
</decisions>

<specifics>
## Specific Ideas

### Pages needing skeleton upgrade (from grep — "Loading..." text)
1. `leaderboard/page.tsx:139`
2. `calendar/page.tsx:38`
3. `waivers/page.tsx:131,198`
4. `packages/page.tsx:41`
5. `coupons/page.tsx:125`
6. `cafe/page.tsx:104`
7. `pricing/page.tsx:127`
8. `cafe/inventory/page.tsx:102`
9. `tournaments/page.tsx:205`
10. `memberships/page.tsx:150` (hidden from nav but still accessible)
11. `bookings/page.tsx:163`
12. `wallet-transactions/page.tsx:479` (inline loading text in modal)

### Pages already using skeletons (reference patterns)
- `fleet/page.tsx` — fleet health tiles with skeleton cards
- `page.tsx` (dashboard home) — overview cards with skeletons
- `settings/health/page.tsx` — health tiles + deploy log with skeleton table
- `sales/page.tsx`, `purchases/page.tsx`, `finance/page.tsx` — table skeletons
- `hr/page.tsx`, `hr/attendance/page.tsx`, `hr/leaves/page.tsx` — table skeletons
- `metrics/page.tsx` — chart skeletons
</specifics>

<canonical_refs>
## Canonical References

### Nav component
- `racingpoint-admin/src/components/AdminLayout.tsx:14-98` — static nav sections array
- `racingpoint-admin/src/components/AdminLayout.tsx:62` — memberships hidden
- `racingpoint-admin/src/components/AdminLayout.tsx:77` — wallet-transactions hidden

### Loading/skeleton components
- `racingpoint-admin/src/components/Skeleton.tsx` — SkeletonLine, SkeletonCard, SkeletonTable, SkeletonPage

### Toast system
- `sonner` v2.0.7 — `import { toast } from 'sonner'`
- `racingpoint-admin/src/components/Toast.tsx` — backward-compatible wrapper

### Data fetching
- `racingpoint-admin/src/lib/api/base.ts` — `rcFetch()` + `apiFetch()` with circuit breaker
- SWR pattern: `useSWR(key, fetcher, { refreshInterval: N })`

### Error pattern reference
- `racingpoint-admin/src/app/(dashboard)/customers/page.tsx` — inline red error box with retry button
</canonical_refs>

<deferred>
## Deferred Ideas

- Centralized `<ErrorBoundary>` component wrapping all dashboard pages (own phase — significant refactor)
- `error.tsx` file in app router for automatic error catching
- Automated visual regression tests for loading/empty/error states (v47.0 Phase 350+)
- Feature flag system for hiding/showing nav items dynamically
</deferred>
