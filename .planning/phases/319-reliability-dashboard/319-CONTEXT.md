# Phase 319: Reliability Dashboard - Context

**Gathered:** 2026-04-03
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped via autonomous mode)

<domain>
## Phase Boundary

Admin dashboard page showing fleet game matrix (which pods have which games), per-combo reliability scores with flagged unreliable combos, and launch timeline viewer for debugging. This is an internal staff tool in the existing Next.js admin app (:3201).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — autonomous mode. Key constraints:
- Uses existing Next.js admin dashboard at apps/admin/ (port 3201)
- New page at /reliability
- Fleet game matrix: 8-pod x N-game grid with install status badges from pod_game_inventory
- Combo reliability: per-combo success rates from combo_reliability table, sortable, red highlight for < 70%
- Launch timeline: expandable per-launch view with checkpoint timestamps from launch_timeline_spans
- Data fetching: SWR with 30s polling (existing pattern in admin app)
- Charts: use recharts (already in admin deps) or plain HTML table
- API endpoints already exist: GET /api/v1/presets (with fleet_validity), GET /api/v1/fleet/health, GET /api/v1/launch-timeline/{id}
- May need: GET /api/v1/fleet/game-matrix (new endpoint returning pod_game_inventory data)
- Racing Point brand: Racing Red #E10600, Asphalt Black #1A1A1A, Gunmetal Grey #5A5A5A, Card #222222
- Must load under 3s from remote browser

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `apps/admin/src/app/` — Next.js admin pages
- `apps/admin/src/components/` — shared UI components
- `apps/admin/package.json` — has recharts, swr, tailwind
- `crates/racecontrol/src/api/routes.rs` — REST endpoints
- `crates/racecontrol/src/preset_library.rs` — preset + reliability data

### Established Patterns
- Admin pages use `useSWR` for data fetching with 30s refreshInterval
- Dark theme with Racing Point brand colors
- Tables use plain HTML with Tailwind styling

</code_context>

<specifics>
## Specific Ideas

No specific requirements — refer to ROADMAP success criteria.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
