# Phase 94: Pricing & Conversion - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase adds psychology-driven pricing display, real-time pod scarcity indicators, social proof using actual venue data, and a commitment ladder with WhatsApp nudges to the customer-facing booking experience (kiosk + web PWA).

</domain>

<decisions>
## Implementation Decisions

### Pricing Display & Anchoring
- 3-tier pricing display lives in both kiosk booking wizard AND web PWA /book page for consistent customer touchpoints
- Middle "value" tier visually emphasized with "Most Popular" badge, Racing Red (#E10600) border, and slightly larger card — classic decoy anchoring
- Prices update dynamically using existing `pricing_rules` table (peak/off-peak multipliers already implemented in billing.rs)
- Anchor display uses strikethrough original price + bold current price for anchoring effect

### Pod Scarcity & Social Proof
- Real-time pod availability shown as "X of 8 pods available now" with color gradient (green→yellow→red) using live fleet health data
- Social proof displays "Y drivers raced this week" + "Z sessions today" from real billing_sessions data — actual counts, never fabricated
- Social proof placed below pricing tiers on booking page — visible during decision moment
- Zero availability shows "All pods in use — next slot likely in ~Xmin" with waitlist CTA — loss-framed scarcity

### Commitment Ladder & Nudges
- Ladder steps: Trial → Single Session → Package (5-pack) → Membership — matches existing pricing_tiers
- Next-step nudges delivered via post-session WhatsApp through psychology engine nudge_queue — e.g., "You've done 3 sessions! Save 20% with a 5-pack"
- Ladder position tracked via new `commitment_ladder` column on drivers table (enum: trial/single/package/member)
- Nudge triggers: after 2nd single session (→ package nudge) or after 3rd package use (→ membership nudge) — natural escalation points

### Claude's Discretion
- API endpoint naming and response structure
- Component file organization within kiosk/web apps
- Exact color gradient thresholds for pod availability indicator

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/psychology.rs` — notification engine with nudge_queue, throttling, multi-channel dispatch (Phase 89)
- `crates/racecontrol/src/billing.rs` — `compute_dynamic_price()` with pricing_rules table (peak/off-peak multipliers)
- `pricing_tiers` table — already has id, name, duration_minutes, is_active
- `pricing_rules` table — day_of_week, hour_start/end, multiplier, flat_adjustment
- Fleet health endpoint: `GET /api/v1/fleet/health` — real-time pod status with ws_connected, http_reachable
- `kiosk/src/components/SetupWizard.tsx` — existing booking wizard component
- `web/src/app/billing/page.tsx` — existing billing page
- `packages/shared-types/` — shared TypeScript types across kiosk/web/admin

### Established Patterns
- Next.js App Router with server components + client islands
- SWR for data fetching in frontend apps
- Sonner for toast notifications
- Racing Red #E10600 as primary brand color
- Tailwind CSS for styling

### Integration Points
- Backend: new API endpoints on racecontrol :8080 for pricing display + social proof stats
- Kiosk: enhance SetupWizard with pricing tier selection
- Web PWA: enhance /book page with pricing + social proof
- Psychology engine: commitment ladder nudges via existing nudge_queue
- drivers table: new commitment_ladder column

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches based on existing codebase patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
