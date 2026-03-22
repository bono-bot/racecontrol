# Phase 151: Menu Display - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Cafe menu rendering in POS (staff billing flow) and PWA (customer-facing). Items grouped by category, images/descriptions in PWA, unavailable items hidden. Consumes the public `/api/v1/cafe/menu` endpoint from Phase 149.

</domain>

<decisions>
## Implementation Decisions

### POS Menu Display
- New "Cafe" tab/section in the existing POS billing flow — staff sees cafe items alongside gaming options
- Category tabs + item grid layout — tap category to filter, tap item to add to order
- Show item name and selling price (formatted as rupees from paise)
- Unavailable items hidden (already filtered by public menu endpoint)

### PWA Menu Display
- Category sections with card grid — each category is a header, items shown as cards with image, name, price, description
- "Cafe" tab in bottom navigation or prominent section on PWA home page
- Images served from `/static/cafe-images/` (Phase 150 infrastructure)
- Unavailable items hidden (already filtered server-side)

### Claude's Discretion
- Exact card sizing, spacing, and responsive breakpoints
- Loading skeleton/placeholder while menu loads
- Empty state when no items in a category
- Image fallback when no image uploaded

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `GET /api/v1/cafe/menu` — public endpoint returning available items with category info
- `web/src/lib/api.ts` — existing `api.listCafeItems()` and types
- `pwa/` — Next.js PWA app (customer-facing)
- `kiosk/` — POS kiosk app

### Integration Points
- POS: extend kiosk app with cafe menu section
- PWA: add cafe menu page/tab
- Both consume the same public menu API endpoint

</code_context>

<specifics>
## Specific Ideas

No specific requirements — follow Racing Point brand identity (red #E10600, black #1A1A1A, Montserrat font).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
