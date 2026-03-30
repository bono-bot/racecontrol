# Phase 265: Kiosk Pages - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning

<domain>
## Phase Boundary
Redesign all 5 kiosk screens with touch-optimized components. Pod selection grid, game launch flow, billing/payment view, staff tools, kiosk leaderboard. Must verify on actual pod hardware.
Requirements: KS-01..KS-05
</domain>

<decisions>
## Implementation Decisions
### Touch UX
- Minimum 44x44px touch targets everywhere
- No hover-only interactions (all must work with tap)
- Press feedback on buttons (scale or color change)
- Full-screen panels, no scrolling where possible (overflow:hidden)
- Kiosk basePath: "/kiosk" — all hrefs root-relative

### Design Direction
- Pod grid: large cards with status, game icon, countdown
- Game launch: step-by-step wizard (sim → difficulty → AI → launch)
- Billing: countdown ring, wallet balance, session timer
- Staff tools: PinPad gated, fleet status overview
- Leaderboard: animated rank changes with AnimatePresence

### Claude's Discretion
Component layouts, animation timing, touch gesture handling.
</decisions>

<code_context>
## Existing Code Insights
- kiosk/src/app/page.tsx — existing pod selection + PIN entry
- kiosk/src/app/book/page.tsx — booking flow
- kiosk/src/app/fleet/page.tsx — fleet status
- kiosk/src/app/debug/page.tsx — debug tools
- kiosk/src/app/staff/page.tsx — staff tools
- kiosk has separate component set from web (different interaction model)
</code_context>

<specifics>
- Must test on actual pod touchscreen before marking complete
- No hover:* utilities that hide content on touch devices
- Kiosk must work without keyboard (on-screen numpad for all input)
</specifics>

<deferred>
- Ambient race-mode background animation
- QR code telemetry sharing
</deferred>
