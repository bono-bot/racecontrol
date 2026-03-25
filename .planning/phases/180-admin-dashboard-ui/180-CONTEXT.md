# Phase 180: Admin Dashboard UI - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Operators can toggle feature flags per-pod or fleet-wide and trigger OTA releases from the admin dashboard, with live wave progress, pod drain status, and rollback controls visible without a terminal. Two new pages: Feature Flags and OTA Releases.

</domain>

<decisions>
## Implementation Decisions

### Page structure
- Two separate pages: `/flags` and `/ota` — consistent with existing one-page-per-feature pattern
- Both added to Sidebar.tsx nav array (under a new "Operations" group or near existing Settings)
- Both use `DashboardLayout` wrapper, `"use client"` pattern

### Feature Flags page (/flags)
- Table of all registered flags with toggle switches (one row per flag)
- Scope selector per flag: "Fleet-wide" (default) or individual pod override dropdown
- Toggle is immediate (no confirm dialog) — calls `PUT /api/v1/flags/{name}` with new value
- Toast notification on success ("Flag updated") or failure ("Failed to update")
- Flag divergence column in fleet health table: pods whose cached flags differ from server registry are highlighted with a warning badge
- Kill switch flags (kill_*) visually distinct — red toggle, warning icon

### OTA Releases page (/ota)
- Current pipeline state displayed prominently (Idle, Canary, StagedRollout, etc.)
- Stepper/timeline visualization: Wave 1 (Pod 8) → Wave 2 (1-4) → Wave 3 (5-7)
- Per-pod status within each wave: pending, deploying, draining (has active billing), complete, failed
- "Draining" pods show billing session info (driver, remaining time) so operator knows WHY the pod is waiting
- One-click rollback button — visible during active pipeline, triggers POST to rollback endpoint
- Deploy trigger: text area for TOML manifest paste + "Deploy" button → POST /api/v1/ota/deploy
- History section: last 5 deploy records from deploy-state.json

### Real-time updates
- Feature flags: WebSocket events from existing useWebSocket hook (flag changes broadcast as DashboardEvent)
- OTA pipeline: SWR polling GET /api/v1/ota/status every 3 seconds during active pipeline, 30s when idle
- Pod fleet health: existing useWebSocket hook provides real-time pod status

### Claude's Discretion
- Exact card layout and spacing within pages
- Loading skeleton implementation
- Empty state design for OTA page when no deploys have run
- Color coding for pipeline states (green=complete, yellow=in-progress, red=failed/rolling-back)
- Whether to use tabs or sections within each page

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### API endpoints (Phase 177 + 179)
- `crates/racecontrol/src/flags.rs` — Flag CRUD endpoints (GET/POST/PUT /api/v1/flags)
- `crates/racecontrol/src/ota_pipeline.rs` — OTA types (ReleaseManifest, PipelineState, DeployRecord, HealthFailure)
- `crates/racecontrol/src/api/routes.rs` lines 375-388 — Flag + OTA route registration

### Admin dashboard patterns
- `web/src/lib/api.ts` — API client pattern (fetchApi wrapper + typed api.* object)
- `web/src/components/Sidebar.tsx` — Navigation array pattern
- `web/src/components/DashboardLayout.tsx` — Page wrapper component
- `web/src/hooks/useWebSocket.ts` — Real-time data hook (socket.io)
- `web/src/app/globals.css` — Tailwind theme variables (rp-red, rp-card, rp-border)

### Standing rules
- `CLAUDE.md` §Standing Rules — Cross-process updates, UI must reflect config truth

### Requirements
- `.planning/REQUIREMENTS.md` — FF-06, SYNC-04

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `DashboardLayout` component: page wrapper with sidebar — use for both new pages
- `StatusBadge` component: colored status indicators — reuse for pipeline state badges
- `PodCard` component: pod display card — extend or adapt for per-pod deploy status
- `api` client in `web/src/lib/api.ts`: typed fetch wrapper — add flag + OTA methods
- `useWebSocket` hook: real-time pod/billing data — flag changes can piggyback on this

### Established Patterns
- All pages are client components (`"use client"`)
- API calls via `api.*` methods with typed responses
- Cards use `bg-rp-card border border-rp-border rounded-lg p-4`
- Tables use standard HTML tables with Tailwind classes
- Modals via conditional rendering (`selectedItem && <Modal />`)
- No shadcn/ui — all custom components with Tailwind

### Integration Points
- `web/src/components/Sidebar.tsx` nav array — add /flags and /ota entries
- `web/src/lib/api.ts` — add api.listFlags(), api.updateFlag(), api.getOtaStatus(), api.triggerOtaDeploy()
- `web/src/hooks/useWebSocket.ts` — may need new event types for flag_sync and ota_progress

</code_context>

<specifics>
## Specific Ideas

- Draining status must be visible — operators need to know which pods are waiting for billing sessions and why
- Kill switch flags should look dangerous — red toggles, warning styling
- Rollback should be one-click but with a brief confirmation ("Are you sure? This will revert all pods to previous binary")
- Pipeline stepper should show real-time progression, not just current state

</specifics>

<deferred>
## Deferred Ideas

- Config push UI (editing billing rates, game limits from dashboard) — could be Phase 180.1
- Deploy history timeline with git commit links — future enhancement
- Automated flag A/B testing dashboard — future milestone

</deferred>

---

*Phase: 180-admin-dashboard-ui*
*Context gathered: 2026-03-25*
