# Phase 297: Config Editor UI - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (autonomous mode)

<domain>
## Phase Boundary

Staff can view, edit, and push pod configuration from the admin app without touching files. Admin /config page with per-pod editor, diff view, one-click push, bulk ops, audit log.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion. Follow existing admin app patterns.

Key observations:
- Admin app is Next.js in racingpoint-admin repo at C:/Users/bono/racingpoint/racingpoint-admin/
- Uses 'use client', SWR, sonner toast, useAuth hook, rp-card/rp-border styles
- Fleet page pattern: single page.tsx with useSWR fetching, card-based layout
- API: Phase 296 added POST/GET /api/v1/config/pod/{pod_id} endpoints
- Config push: POST /api/v1/config/push with fields map + target_pods
- Audit log: GET /api/v1/config/audit returns audit entries
- Create /config page in (dashboard) group
- Use existing API client pattern from lib/api/

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `racingpoint-admin/src/app/(dashboard)/fleet/page.tsx` — pod card pattern
- `racingpoint-admin/src/lib/api/fleet.ts` — API client pattern
- `racingpoint-admin/src/hooks/useAuth.ts` — auth hook
- `racingpoint-admin/src/components/ConfirmDialog.tsx` — confirmation modals

### Established Patterns
- Pages in (dashboard) group with layout.tsx wrapper
- SWR for data fetching with refreshInterval
- Card-based grid layouts with rp-card/rp-border classes
- Toast notifications via sonner
- Dark theme with neutral grays and Racing Red #E10600 accent

### Integration Points
- API endpoints at server :8080 (existing proxy in next.config)
- POST /api/v1/config/pod/{pod_id} — store config
- GET /api/v1/config/pod/{pod_id} — get stored config
- POST /api/v1/config/push — push config to pods
- GET /api/v1/config/audit — audit log

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard admin UI page.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
