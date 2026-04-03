# Phase 320: Kiosk Game Filtering - Context

**Gathered:** 2026-04-03
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped via autonomous mode)

<domain>
## Phase Boundary

Customers on each pod only see games and AC combos that are actually available on that specific pod. The kiosk game picker filters by pod_game_inventory — no silent launch failures from showing unavailable content. AC presets with combo_valid=false show "Unavailable" badge.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion. Key constraints:
- Kiosk app is at kiosk/ in the monorepo (port 3300)
- Game picker component needs to read installed_games from the pod's heartbeat/WS state
- Filter the game selection list client-side based on what's installed on THIS pod
- AC combos: check combo_valid from server preset data, show "Unavailable" badge if invalid
- Inventory changes reflected within 30s (existing polling interval)
- No flicker mid-browse — debounce inventory updates
- This is the highest customer-impact phase — validate carefully

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `kiosk/src/app/` — Next.js kiosk pages
- `kiosk/src/components/GamePickerPanel.tsx` — existing game selection component (already has installedGames prop filter at line 53-58)
- `kiosk/src/lib/api.ts` — API client
- Server: GET /api/v1/presets (returns fleet_validity), GET /api/v1/fleet/game-matrix

</code_context>

<specifics>
## Specific Ideas

GamePickerPanel already receives installedGames as a prop and filters on it (line 53-58). The gap is populating this prop from pod_game_inventory rather than the in-memory-only PodInfo field.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
